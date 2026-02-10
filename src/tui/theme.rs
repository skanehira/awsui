use ratatui::style::{Color, Modifier, Style};

/// アクティブ要素（タブ、フォーカス枠）
pub fn active() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

/// 非アクティブ要素
pub fn inactive() -> Style {
    Style::default().fg(Color::DarkGray)
}

/// テーブル選択行
pub fn selected() -> Style {
    Style::default()
        .fg(Color::White)
        .bg(Color::DarkGray)
        .add_modifier(Modifier::BOLD)
}

/// テーブルヘッダー
pub fn header() -> Style {
    Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
}

/// ステータスバー
pub fn status_bar() -> Style {
    Style::default().fg(Color::White).bg(Color::DarkGray)
}

/// State: running
pub fn state_running() -> Style {
    Style::default().fg(Color::Green)
}

/// State: stopped
pub fn state_stopped() -> Style {
    Style::default().fg(Color::Red)
}

/// State: pending / stopping / shutting-down
pub fn state_transitioning() -> Style {
    Style::default().fg(Color::Yellow)
}

/// State: terminated
pub fn state_terminated() -> Style {
    Style::default().fg(Color::DarkGray)
}

/// Error message
pub fn error() -> Style {
    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
}

/// Success message
pub fn success() -> Style {
    Style::default().fg(Color::Green)
}

/// Info message
pub fn info() -> Style {
    Style::default().fg(Color::Cyan)
}

/// 検索マッチ（非カレント）
pub fn search_match() -> Style {
    Style::default().bg(Color::Yellow).fg(Color::Black)
}

/// 検索マッチ（カレント）
pub fn search_match_current() -> Style {
    Style::default()
        .bg(Color::Cyan)
        .fg(Color::Black)
        .add_modifier(Modifier::BOLD)
}
