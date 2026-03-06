use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph},
};

use crate::models::{App, AppState};

pub fn render_ui(frame: &mut ratatui::Frame<'_>, app: &App) {
    match app.state {
        AppState::Login => render_login_ui(frame, app),
        AppState::Main => render_main_ui(frame, app),
    }
}

fn render_login_ui(frame: &mut ratatui::Frame<'_>, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let help = Paragraph::new("欢迎使用 NetEasyDownload | 请先登录 | q 退出").block(
        Block::default()
            .title("NetEasyDownload - 登录")
            .borders(Borders::ALL),
    );
    frame.render_widget(help, chunks[0]);

    let input_title = if app.login_phone.is_empty() {
        "请输入手机号"
    } else {
        "请输入验证码"
    };
    let input = Paragraph::new(app.input.as_str())
        .block(Block::default().title(input_title).borders(Borders::ALL));
    frame.render_widget(input, chunks[1]);

    let login_info = if app.login_phone.is_empty() {
        vec![
            ListItem::new("步骤 1: 输入11位手机号，按 Enter 发送验证码"),
            ListItem::new("步骤 2: 输入收到的验证码，按 Enter 完成登录"),
            ListItem::new(""),
            ListItem::new("登录后可以下载更高音质的歌曲"),
        ]
    } else {
        vec![
            ListItem::new(format!("手机号: {}", app.login_phone)),
            ListItem::new(""),
            ListItem::new("验证码已发送，请查收短信"),
            ListItem::new("输入验证码后按 Enter 完成登录"),
        ]
    };

    let list =
        List::new(login_info).block(Block::default().title("登录说明").borders(Borders::ALL));
    frame.render_widget(list, chunks[2]);

    let status = Paragraph::new(app.status.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().title("状态").borders(Borders::ALL));
    frame.render_widget(status, chunks[3]);
}

fn render_main_ui(frame: &mut ratatui::Frame<'_>, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let help = Paragraph::new("Enter=搜索歌曲 | ↑/↓ 选择 | d 下载 | 1标准 2极高 3无损 4Hi-Res | q退出").block(
        Block::default()
            .title("NetEasyDownload")
            .borders(Borders::ALL),
    );
    frame.render_widget(help, chunks[0]);

    let input = Paragraph::new(app.input.as_str()).block(
        Block::default()
            .title("输入歌曲关键词")
            .borders(Borders::ALL),
    );
    frame.render_widget(input, chunks[1]);

    render_results_list(frame, app, chunks[2]);

    let status = Paragraph::new(format!("{} | {}", app.status, app.cookie_desc))
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().title("状态").borders(Borders::ALL));
    frame.render_widget(status, chunks[3]);

    let (ratio, label) = build_download_progress(app);
    let gauge = Gauge::default()
        .block(Block::default().title("下载进度").borders(Borders::ALL))
        .gauge_style(Style::default().fg(Color::Green))
        .ratio(ratio)
        .label(label);
    frame.render_widget(gauge, chunks[4]);
}

fn render_results_list(frame: &mut ratatui::Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let list = List::new(build_result_items(app))
        .block(Block::default().title("搜索结果").borders(Borders::ALL))
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan))
        .highlight_symbol(">> ");

    let mut state = ListState::default();
    if !app.results.is_empty() {
        state.select(Some(app.selected));
    }

    frame.render_stateful_widget(list, area, &mut state);
}

fn build_result_items(app: &App) -> Vec<ListItem<'_>> {
    if app.results.is_empty() {
        return vec![ListItem::new("输入歌曲名或歌手名，按 Enter 搜索")];
    }

    app.results
        .iter()
        .map(|song| {
            ListItem::new(format!(
                "{} | {} | id={}",
                song.name,
                if song.artists.is_empty() {
                    "未知歌手".to_string()
                } else {
                    song.artists.join("/")
                },
                song.id
            ))
        })
        .collect()
}

fn build_download_progress(app: &App) -> (f64, String) {
    if let Some((downloaded, total)) = app.download_progress {
        if let Some(total_bytes) = total {
            if total_bytes > 0 {
                let ratio = (downloaded as f64 / total_bytes as f64).min(1.0);
                return (ratio, format!("{:.1}%", ratio * 100.0));
            }
            return (0.0, "未知总大小".to_string());
        }

        return (0.0, format!("已下载 {} KB", downloaded / 1024));
    }

    (0.0, "无下载任务".to_string())
}
