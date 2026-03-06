use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Song {
    pub id: u64,
    pub name: String,
    pub artists: Vec<String>,
}

#[derive(Debug)]
pub struct LoginData {
    pub user_id: String,
    pub token: String,
}

#[derive(Debug)]
pub enum WorkerEvent {
    SearchFinished(Result<Vec<Song>, String>),
    CaptchaFinished(Result<(), String>),
    LoginFinished(Result<LoginData, String>),
    DownloadProgress { downloaded: u64, total: Option<u64> },
    DownloadFinished(Result<PathBuf, String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Login,
    Main,
}

pub struct App {
    pub state: AppState,
    pub input: String,
    pub results: Vec<Song>,
    pub selected: usize,
    pub status: String,
    pub cookie_desc: String,
    pub search_running: bool,
    pub auth_running: bool,
    pub download_running: bool,
    pub level: String,
    pub download_progress: Option<(u64, Option<u64>)>,
    pub should_quit: bool,
    pub login_phone: String,
}

pub fn level_label(level: &str) -> &'static str {
    match level {
        "standard" => "标准",
        "exhigh" => "极高",
        "lossless" => "无损",
        "hires" => "Hi-Res",
        _ => "未知",
    }
}

impl App {
    pub fn new(default_level: String, cookie_desc: String, has_cookie: bool) -> Self {
        let state = if has_cookie {
            AppState::Main
        } else {
            AppState::Login
        };

        let status = if has_cookie {
            format!("就绪 | {} | 音质: {}", cookie_desc, level_label(&default_level))
        } else {
            "请先登录以使用完整功能".to_string()
        };

        Self {
            state,
            input: String::new(),
            results: Vec::new(),
            selected: 0,
            status,
            cookie_desc,
            search_running: false,
            auth_running: false,
            download_running: false,
            level: default_level,
            download_progress: None,
            should_quit: false,
            login_phone: String::new(),
        }
    }

    pub fn selected_song(&self) -> Option<Song> {
        self.results.get(self.selected).cloned()
    }

    pub fn next(&mut self) {
        if self.results.is_empty() {
            self.selected = 0;
            return;
        }
        self.selected = (self.selected + 1).min(self.results.len() - 1);
    }

    pub fn previous(&mut self) {
        if self.results.is_empty() || self.selected == 0 {
            self.selected = 0;
            return;
        }
        self.selected -= 1;
    }
}
