use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use anyhow::{Context, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

mod client;
mod crypto;
mod models;
mod ui;

use client::MusicClient;
use models::{App, AppState, LoginData, Song, WorkerEvent, format_bytes, level_label};
use ui::render_ui;

const COOKIE_CACHE_FILE: &str = ".neteasydownload_cookie";
const LEGACY_COOKIE_CACHE_FILE: &str = ".cloudx_cookie";
const COOKIE_ENV_KEY: &str = "NETEASYDOWNLOAD_COOKIE";
const LEGACY_COOKIE_ENV_KEY: &str = "CLOUDX_COOKIE";
const LEVEL_ENV_KEY: &str = "NETEASYDOWNLOAD_LEVEL";
const LEGACY_LEVEL_ENV_KEY: &str = "CLOUDX_LEVEL";

fn main() -> Result<()> {
    let (cookie, source) = load_initial_cookie();
    let has_cookie = !cookie.trim().is_empty();
    let default_level = read_env_first_non_empty(&[LEVEL_ENV_KEY, LEGACY_LEVEL_ENV_KEY])
        .unwrap_or_else(|| "hires".to_string());
    let client = MusicClient::new(cookie.clone())?;
    let cookie_desc = format_cookie_desc(&cookie, &source);

    enable_raw_mode().context("启用 raw mode 失败")?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen).context("进入备用屏幕失败")?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("创建终端失败")?;

    let run_result = run_app(
        &mut terminal,
        client,
        default_level,
        cookie_desc,
        has_cookie,
    );

    disable_raw_mode().ok();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();

    run_result
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    client: MusicClient,
    default_level: String,
    cookie_desc: String,
    has_cookie: bool,
) -> Result<()> {
    let (tx, rx): (Sender<WorkerEvent>, Receiver<WorkerEvent>) = mpsc::channel();
    let mut app = App::new(default_level, cookie_desc, has_cookie);

    loop {
        handle_worker_events(&mut app, &rx, &client);
        terminal.draw(|f| render_ui(f, &app))?;

        if app.should_quit {
            break;
        }

        process_input_event(&mut app, &client, &tx)?;
    }

    Ok(())
}

fn process_input_event(
    app: &mut App,
    client: &MusicClient,
    tx: &Sender<WorkerEvent>,
) -> Result<()> {
    if event::poll(Duration::from_millis(120)).context("轮询终端事件失败")?
        && let Event::Key(key_event) = event::read().context("读取终端事件失败")?
    {
        handle_key_event(app, key_event, client, tx);
    }

    Ok(())
}

fn handle_key_event(
    app: &mut App,
    key_event: KeyEvent,
    client: &MusicClient,
    tx: &Sender<WorkerEvent>,
) {
    if key_event.kind != KeyEventKind::Press {
        return;
    }

    match key_event.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Enter => handle_enter_key(app, client, tx),
        KeyCode::Backspace => {
            app.input.pop();
        }
        KeyCode::Esc => app.input.clear(),
        KeyCode::Char(c) => handle_char_key(app, client, tx, c),
        KeyCode::Up => {
            if app.state == AppState::Main {
                app.previous();
            }
        }
        KeyCode::Down => {
            if app.state == AppState::Main {
                app.next();
            }
        }
        _ => {}
    }
}

fn handle_enter_key(app: &mut App, client: &MusicClient, tx: &Sender<WorkerEvent>) {
    let line = app.input.trim().to_string();
    if line.is_empty() {
        app.status = if app.state == AppState::Login {
            "请输入内容".to_string()
        } else {
            "请输入关键词".to_string()
        };
    } else {
        handle_enter_line(app, client, line, tx);
    }
    app.input.clear();
}

fn handle_char_key(app: &mut App, client: &MusicClient, tx: &Sender<WorkerEvent>, c: char) {
    if app.state == AppState::Main {
        handle_main_char_key(app, client, tx, c);
        return;
    }

    app.input.push(c);
}

fn handle_main_char_key(app: &mut App, client: &MusicClient, tx: &Sender<WorkerEvent>, c: char) {
    match c {
        'd' => trigger_download(app, client, tx),
        '1' => set_quality(app, "standard"),
        '2' => set_quality(app, "exhigh"),
        '3' => set_quality(app, "lossless"),
        '4' => set_quality(app, "hires"),
        _ => app.input.push(c),
    }
}

fn trigger_download(app: &mut App, client: &MusicClient, tx: &Sender<WorkerEvent>) {
    if app.download_running {
        app.status = "已有下载任务正在进行".to_string();
        return;
    }

    if let Some(song) = app.selected_song() {
        app.download_running = true;
        app.download_progress = Some((0, None));
        app.status = format!("开始下载: {}", song.name);
        spawn_download(client.clone(), song, app.level.clone(), tx.clone());
    } else {
        app.status = "当前没有可下载歌曲".to_string();
    }
}

fn set_quality(app: &mut App, level: &str) {
    app.level = level.to_string();
    app.status = format!("音质切换为 {}", level_label(level));
}

fn handle_enter_line(app: &mut App, client: &MusicClient, line: String, tx: &Sender<WorkerEvent>) {
    match app.state {
        AppState::Login => handle_login_enter(app, client, line, tx),
        AppState::Main => handle_main_enter(app, client, line, tx),
    }
}

fn handle_login_enter(app: &mut App, client: &MusicClient, line: String, tx: &Sender<WorkerEvent>) {
    if app.auth_running {
        app.status = "认证任务进行中，请稍后".to_string();
        return;
    }

    if app.login_phone.is_empty() {
        let phone = line.trim();
        if !is_valid_phone(phone) {
            app.status = "请输入正确的11位手机号".to_string();
            return;
        }

        app.login_phone = phone.to_string();
        app.auth_running = true;
        app.status = format!("正在发送验证码到 {phone} ...");
        spawn_send_captcha(client.clone(), phone.to_string(), tx.clone());
        return;
    }

    let captcha = line.trim();
    if !is_valid_captcha(captcha) {
        app.status = "请输入4-6位数字验证码".to_string();
        return;
    }

    app.auth_running = true;
    app.status = "正在验证登录...".to_string();
    spawn_login(
        client.clone(),
        app.login_phone.clone(),
        captcha.to_string(),
        tx.clone(),
    );
}

fn handle_main_enter(app: &mut App, client: &MusicClient, line: String, tx: &Sender<WorkerEvent>) {
    if app.search_running {
        app.status = "搜索进行中，请稍后".to_string();
        return;
    }

    app.search_running = true;
    app.status = format!("搜索中: {line}");
    spawn_search(client.clone(), line, tx.clone());
}

fn handle_worker_events(app: &mut App, rx: &Receiver<WorkerEvent>, client: &MusicClient) {
    while let Ok(event) = rx.try_recv() {
        dispatch_worker_event(app, event, client);
    }
}

fn dispatch_worker_event(app: &mut App, event: WorkerEvent, client: &MusicClient) {
    match event {
        WorkerEvent::SearchFinished(result) => on_search_finished(app, result),
        WorkerEvent::CaptchaFinished(result) => on_captcha_finished(app, result),
        WorkerEvent::LoginFinished(result) => on_login_finished(app, result, client),
        WorkerEvent::DownloadProgress { downloaded, total } => {
            on_download_progress(app, downloaded, total)
        }
        WorkerEvent::DownloadFinished(result) => on_download_finished(app, result),
    }
}

fn on_search_finished(app: &mut App, result: Result<Vec<Song>, String>) {
    app.search_running = false;
    match result {
        Ok(songs) => {
            let count = songs.len();
            app.results = songs;
            app.selected = 0;
            app.status = format!("搜索完成，共 {count} 条结果（音质: {}）", level_label(&app.level));
        }
        Err(err) => {
            app.status = format!("搜索失败: {err}");
        }
    }
}

fn on_captcha_finished(app: &mut App, result: Result<(), String>) {
    app.auth_running = false;
    app.status = match result {
        Ok(_) => "验证码发送成功，请输入验证码".to_string(),
        Err(err) => {
            app.login_phone.clear();
            format!("验证码发送失败: {err}")
        }
    };
}

fn on_login_finished(app: &mut App, result: Result<LoginData, String>, client: &MusicClient) {
    app.auth_running = false;

    match result {
        Ok(data) => {
            let cookie = build_cookie_from_token(&data.token);
            client.set_cookie(cookie.clone());
            app.cookie_desc = format_cookie_desc(&cookie, "验证码登录");
            app.state = AppState::Main;
            app.login_phone.clear();
            app.status = match save_cookie_to_cache(&cookie) {
                Ok(_) => format!("登录成功！uid={} | 音质: {}", data.user_id, level_label(&app.level)),
                Err(e) => format!("登录成功！uid={}，Cookie 保存失败: {e}", data.user_id),
            };
        }
        Err(err) => {
            app.login_phone.clear();
            app.status = format!("登录失败: {err}");
        }
    }
}

fn on_download_progress(app: &mut App, downloaded: u64, total: Option<u64>) {
    app.download_progress = Some((downloaded, total));
    app.status = if let Some(total_size) = total {
        if total_size > 0 {
            let percent = (downloaded as f64 / total_size as f64 * 100.0).min(100.0);
            format!(
                "下载中... {} / {} ({percent:.1}%)",
                format_bytes(downloaded),
                format_bytes(total_size)
            )
        } else {
            format!("下载中... {}", format_bytes(downloaded))
        }
    } else {
        format!("下载中... {}", format_bytes(downloaded))
    };
}

fn on_download_finished(app: &mut App, result: Result<PathBuf, String>) {
    app.download_running = false;
    app.download_progress = None;
    app.status = match result {
        Ok(path) => format!("下载完成: {}", path.display()),
        Err(err) => format!("下载失败: {err}"),
    };
}

fn spawn_search(client: MusicClient, keyword: String, tx: Sender<WorkerEvent>) {
    thread::spawn(move || {
        let result = client
            .search_music(&keyword, 0, 20)
            .map_err(|e| e.to_string());
        let _ = tx.send(WorkerEvent::SearchFinished(result));
    });
}

fn spawn_send_captcha(client: MusicClient, phone: String, tx: Sender<WorkerEvent>) {
    thread::spawn(move || {
        let result = client.send_captcha(&phone).map_err(|e| e.to_string());
        let _ = tx.send(WorkerEvent::CaptchaFinished(result));
    });
}

fn spawn_login(client: MusicClient, phone: String, captcha: String, tx: Sender<WorkerEvent>) {
    thread::spawn(move || {
        let result = client
            .login_with_captcha(&phone, &captcha)
            .map_err(|e| e.to_string());
        let _ = tx.send(WorkerEvent::LoginFinished(result));
    });
}

fn spawn_download(client: MusicClient, song: Song, level: String, tx: Sender<WorkerEvent>) {
    thread::spawn(move || {
        let result = (|| -> Result<PathBuf> {
            let url = client
                .get_song_url(song.id, &level)
                .with_context(|| format!("获取歌曲下载地址失败: {}", song.name))?;

            let downloads_dir = env::current_dir()
                .context("获取当前运行目录失败")?
                .join("downloads");
            fs::create_dir_all(&downloads_dir).context("创建 downloads 目录失败")?;
            let ext = guess_extension(&url);
            let mut base = sanitize_filename(&format!("{}-{}", song.name, song.artists.join("、")));
            if base.is_empty() {
                base = format!("song-{}", song.id);
            }

            let path = unique_path(&downloads_dir, &base, ext);
            client
                .download_file_with_progress(&url, &path, &tx)
                .context("文件下载失败")?;
            Ok(path)
        })()
        .map_err(|e| e.to_string());

        let _ = tx.send(WorkerEvent::DownloadFinished(result));
    });
}

fn load_initial_cookie() -> (String, String) {
    if let Some(cookie) = read_env_first_non_empty(&[COOKIE_ENV_KEY, LEGACY_COOKIE_ENV_KEY]) {
        let source = if env::var(COOKIE_ENV_KEY).is_ok() {
            COOKIE_ENV_KEY
        } else {
            LEGACY_COOKIE_ENV_KEY
        };
        return (cookie, format!("环境变量 {source}"));
    }

    let cookie_cache_path = app_dir().join(COOKIE_CACHE_FILE);
    if let Ok(cookie) = fs::read_to_string(&cookie_cache_path) {
        let trimmed = cookie.trim().to_string();
        if !trimmed.is_empty() {
            return (trimmed, format!("文件 {}", cookie_cache_path.display()));
        }
    }

    let legacy_cookie_cache_path = app_dir().join(LEGACY_COOKIE_CACHE_FILE);
    if let Ok(cookie) = fs::read_to_string(&legacy_cookie_cache_path) {
        let trimmed = cookie.trim().to_string();
        if !trimmed.is_empty() {
            return (trimmed, format!("文件 {}", legacy_cookie_cache_path.display()));
        }
    }

    (String::new(), "未设置 Cookie".to_string())
}

fn save_cookie_to_cache(cookie: &str) -> Result<()> {
    let cookie_cache_path = app_dir().join(COOKIE_CACHE_FILE);
    fs::write(&cookie_cache_path, cookie)
        .with_context(|| format!("写入 {} 失败", cookie_cache_path.display()))
}

fn app_dir() -> PathBuf {
    env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .or_else(|| env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn read_env_first_non_empty(keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        env::var(key)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn format_cookie_desc(cookie: &str, source: &str) -> String {
    if cookie.trim().is_empty() {
        "Cookie: 未设置".to_string()
    } else {
        format!("Cookie: 已设置(len={}, 来源={source})", cookie.len())
    }
}

fn build_cookie_from_token(token: &str) -> String {
    format!(
        "os=pc; osver=Microsoft-Windows-11-Home-China-build-26100-64bit; appver=3.1.23.204750; channel=netease; mode=83NN; __remember_me=true; MUSIC_U={token}"
    )
}

fn is_valid_phone(phone: &str) -> bool {
    phone.len() == 11 && phone.chars().all(|c| c.is_ascii_digit())
}

fn is_valid_captcha(captcha: &str) -> bool {
    (4..=6).contains(&captcha.len()) && captcha.chars().all(|c| c.is_ascii_digit())
}

fn sanitize_filename(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => ' ',
            _ if c.is_control() => ' ',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn guess_extension(url: &str) -> &'static str {
    let path = url.split('?').next().unwrap_or(url);
    let last_segment = path.rsplit('/').next().unwrap_or_default().to_lowercase();

    if last_segment.ends_with(".flac") {
        "flac"
    } else if last_segment.ends_with(".mp3") {
        "mp3"
    } else if last_segment.ends_with(".aac") {
        "aac"
    } else {
        "m4a"
    }
}

fn unique_path(dir: &Path, base_name: &str, ext: &str) -> PathBuf {
    let mut idx = 0usize;
    loop {
        let file_name = if idx == 0 {
            format!("{base_name}.{ext}")
        } else {
            format!("{base_name} ({idx}).{ext}")
        };
        let candidate = dir.join(file_name);
        if !candidate.exists() {
            return candidate;
        }
        idx += 1;
    }
}
