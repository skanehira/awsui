use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Row};
use tui_input::Input;

use crate::app::Mode;
use crate::aws::secrets_model::Secret;
use crate::tui::components::loading::Loading;
use crate::tui::components::status_bar::render_footer;
use crate::tui::components::table::{SelectableTable, SelectableTableWidget};
use crate::tui::theme;

/// シークレット一覧画面を描画する
#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    secrets: &[Secret],
    selected_index: usize,
    filter_input: &Input,
    mode: &Mode,
    loading: bool,
    profile: Option<&str>,
    region: Option<&str>,
    spinner_tick: usize,
) {
    let area = frame.area();

    // フッターは外枠の外に配置
    let outer_chunks = Layout::vertical([
        Constraint::Min(1),    // 外枠（テーブル）
        Constraint::Length(1), // フッター
    ])
    .split(area);

    // 右タイトル（profile │ region）
    let right_title = build_right_title(profile, region);

    // 外枠Block
    let mut outer_block = Block::default().title(" Secrets ").borders(Borders::ALL);
    if let Some(title) = right_title {
        outer_block = outer_block.title_top(Line::from(title).alignment(Alignment::Right));
    }
    let inner = outer_block.inner(outer_chunks[0]);
    frame.render_widget(outer_block, outer_chunks[0]);

    // メインコンテンツ
    if loading {
        let loading_widget = Loading::new("Loading secrets...", spinner_tick);
        frame.render_widget(loading_widget, inner);
    } else {
        render_table(frame, secrets, selected_index, inner);
    }

    // フッター
    render_footer(
        frame,
        outer_chunks[1],
        mode,
        filter_input.value(),
        "j/k:move Enter:detail /:filter ?:help",
    );
}

/// 右タイトル文字列を構築（profile │ region）
fn build_right_title(profile: Option<&str>, region: Option<&str>) -> Option<String> {
    let mut parts = Vec::new();

    if let Some(p) = profile {
        parts.push(p.to_string());
    }
    if let Some(r) = region {
        parts.push(r.to_string());
    }

    if parts.is_empty() {
        None
    } else {
        Some(format!(" {} ", parts.join(" │ ")))
    }
}

/// テーブルを描画
fn render_table(frame: &mut Frame, secrets: &[Secret], selected_index: usize, area: Rect) {
    let headers = Row::new(vec!["Name", "Description", "Last Changed", "Last Accessed"])
        .style(theme::header());

    let rows: Vec<Row> = secrets.iter().map(secret_to_row).collect();

    let widths = vec![
        Constraint::Length(25),
        Constraint::Length(30),
        Constraint::Length(22),
        Constraint::Min(22),
    ];

    let table = SelectableTable::new(headers, rows, widths);
    let widget = SelectableTableWidget::new(table, selected_index);
    frame.render_widget(widget, area);
}

/// シークレットをテーブル行に変換
fn secret_to_row(secret: &Secret) -> Row<'_> {
    let description = secret.description.as_deref().unwrap_or("-");
    let last_changed = secret.last_changed_date.as_deref().unwrap_or("-");
    let last_accessed = secret.last_accessed_date.as_deref().unwrap_or("-");

    Row::new(vec![
        Line::from(secret.name.as_str()),
        Line::from(description),
        Line::from(last_changed),
        Line::from(last_accessed),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::collections::HashMap;

    fn buffer_to_string(terminal: &Terminal<TestBackend>) -> String {
        let buffer = terminal.backend().buffer();
        let mut result = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                result.push_str(buffer[(x, y)].symbol());
            }
            result.push('\n');
        }
        result
    }

    fn create_test_secret(name: &str, description: Option<&str>) -> Secret {
        Secret {
            name: name.to_string(),
            arn: format!("arn:aws:secretsmanager:ap-northeast-1:123456789012:secret:{name}"),
            description: description.map(String::from),
            last_changed_date: Some("2025-01-15T10:30:00Z".to_string()),
            last_accessed_date: Some("2025-01-20T08:00:00Z".to_string()),
            tags: HashMap::new(),
        }
    }

    #[test]
    fn render_returns_header_when_secrets_list() {
        let input = Input::default();
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &[],
                    0,
                    &input,
                    &Mode::Normal,
                    false,
                    Some("dev-account"),
                    Some("ap-northeast-1"),
                    0,
                );
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Secrets"));
    }

    #[test]
    fn render_returns_table_headers_when_secrets_exist() {
        let secrets = vec![create_test_secret("my-secret", Some("A secret"))];
        let input = Input::default();
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &secrets,
                    0,
                    &input,
                    &Mode::Normal,
                    false,
                    None,
                    None,
                    0,
                );
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Name"));
        assert!(content.contains("Description"));
        assert!(content.contains("Last Changed"));
        assert!(content.contains("Last Accessed"));
    }

    #[test]
    fn render_returns_secret_data_when_secrets_provided() {
        let secrets = vec![
            create_test_secret("db-password", Some("Database password")),
            create_test_secret("api-key", Some("API key")),
        ];
        let input = Input::default();
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &secrets,
                    0,
                    &input,
                    &Mode::Normal,
                    false,
                    None,
                    None,
                    0,
                );
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("db-password"));
        assert!(content.contains("Database password"));
        assert!(content.contains("api-key"));
        assert!(content.contains("API key"));
    }

    #[test]
    fn render_returns_right_title_when_profile_and_region_set() {
        let secrets = vec![create_test_secret("my-secret", None)];
        let input = Input::default();
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &secrets,
                    0,
                    &input,
                    &Mode::Normal,
                    false,
                    Some("dev-account"),
                    Some("ap-northeast-1"),
                    0,
                );
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("dev-account"));
        assert!(content.contains("ap-northeast-1"));
        assert!(content.contains("j/k:move"));
    }

    #[test]
    fn render_returns_loading_when_loading_state() {
        let input = Input::default();
        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(frame, &[], 0, &input, &Mode::Normal, true, None, None, 0);
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Loading secrets..."));
    }

    #[test]
    fn render_returns_filter_input_when_filter_mode() {
        let secrets = vec![create_test_secret("my-secret", None)];
        let input = Input::from("db");
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &secrets,
                    0,
                    &input,
                    &Mode::Filter,
                    false,
                    None,
                    None,
                    0,
                );
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("/db"));
    }

    #[test]
    fn render_returns_snapshot_when_rendered() {
        let secrets = vec![
            create_test_secret("db-password", Some("Database password")),
            create_test_secret("api-key", Some("API key for service")),
            create_test_secret("tls-cert", None),
        ];
        let input = Input::default();
        let backend = TestBackend::new(100, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &secrets,
                    0,
                    &input,
                    &Mode::Normal,
                    false,
                    Some("dev-account"),
                    Some("ap-northeast-1"),
                    0,
                );
            })
            .unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }

    #[test]
    fn build_right_title_returns_none_when_no_profile_no_region() {
        assert!(build_right_title(None, None).is_none());
    }

    #[test]
    fn build_right_title_returns_profile_when_profile_only() {
        let title = build_right_title(Some("dev"), None);
        assert_eq!(title, Some(" dev ".to_string()));
    }

    #[test]
    fn build_right_title_returns_both_when_profile_and_region() {
        let title = build_right_title(Some("dev"), Some("ap-northeast-1"));
        assert_eq!(title, Some(" dev │ ap-northeast-1 ".to_string()));
    }
}
