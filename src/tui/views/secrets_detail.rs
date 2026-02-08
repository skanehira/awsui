use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Wrap};

use crate::aws::secrets_model::SecretDetail;
use crate::tui::components::status_bar::StatusBar;
use crate::tui::components::table::{SelectableTable, SelectableTableWidget};
use crate::tui::theme;

/// 詳細画面のタブ
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretsDetailTab {
    Overview,
    Tags,
}

/// シークレット詳細画面を描画する
pub fn render(
    frame: &mut Frame,
    detail: &SecretDetail,
    tag_selected_index: usize,
    detail_tab: &SecretsDetailTab,
    profile: Option<&str>,
    region: Option<&str>,
    area: Rect,
) {
    let outer_chunks = Layout::vertical([
        Constraint::Min(1),    // 外枠（タブバー + コンテンツ）
        Constraint::Length(1), // ステータスバー
    ])
    .split(area);

    // 外枠Block
    let title = format!(" {} ", detail.name);
    let right_title = build_right_title(profile, region);

    let mut outer_block = Block::default().title(title).borders(Borders::ALL);
    if let Some(rt) = right_title {
        outer_block = outer_block.title_top(Line::from(rt).alignment(Alignment::Right));
    }
    let inner = outer_block.inner(outer_chunks[0]);
    frame.render_widget(outer_block, outer_chunks[0]);

    // 内側レイアウト: タブバー + コンテンツ
    let inner_chunks = Layout::vertical([
        Constraint::Length(1), // タブバー
        Constraint::Min(1),    // コンテンツ
    ])
    .split(inner);

    // タブバー
    render_tab_bar(frame, detail_tab, inner_chunks[0]);

    // コンテンツ
    match detail_tab {
        SecretsDetailTab::Overview => render_overview(frame, detail, inner_chunks[1]),
        SecretsDetailTab::Tags => render_tags(frame, detail, tag_selected_index, inner_chunks[1]),
    }

    // ステータスバー
    let keybinds = match detail_tab {
        SecretsDetailTab::Overview => "Tab:switch-tab y:copy-arn Esc:back",
        SecretsDetailTab::Tags => "Tab:switch-tab y:copy-value Esc:back",
    };
    let status = StatusBar::new(keybinds);
    frame.render_widget(status, outer_chunks[1]);
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

/// タブバーを描画
fn render_tab_bar(frame: &mut Frame, current_tab: &SecretsDetailTab, area: Rect) {
    let overview_style = if *current_tab == SecretsDetailTab::Overview {
        theme::active()
    } else {
        theme::inactive()
    };
    let tags_style = if *current_tab == SecretsDetailTab::Tags {
        theme::active()
    } else {
        theme::inactive()
    };

    let tabs = Line::from(vec![
        Span::raw(" "),
        Span::styled("[Overview]", overview_style),
        Span::raw(" "),
        Span::styled("[Tags]", tags_style),
    ]);
    frame.render_widget(Paragraph::new(tabs), area);
}

/// Overviewタブを描画
fn render_overview(frame: &mut Frame, detail: &SecretDetail, area: Rect) {
    let rotation_status = if detail.rotation_enabled {
        "Enabled"
    } else {
        "Disabled"
    };

    let versions_count = detail.version_ids.len().to_string();
    let lines = vec![
        detail_line("Name", &detail.name),
        detail_line("ARN", &detail.arn),
        detail_line("Description", detail.description.as_deref().unwrap_or("-")),
        detail_line("KMS Key", detail.kms_key_id.as_deref().unwrap_or("-")),
        detail_line("Rotation", rotation_status),
        detail_line(
            "Rotation Fn",
            detail.rotation_lambda_arn.as_deref().unwrap_or("-"),
        ),
        detail_line(
            "Last Rotated",
            detail.last_rotated_date.as_deref().unwrap_or("-"),
        ),
        detail_line(
            "Last Changed",
            detail.last_changed_date.as_deref().unwrap_or("-"),
        ),
        detail_line(
            "Last Accessed",
            detail.last_accessed_date.as_deref().unwrap_or("-"),
        ),
        detail_line("Created", detail.created_date.as_deref().unwrap_or("-")),
        detail_line("Versions", &versions_count),
    ];

    let block = Block::default()
        .title(" Secret Info ")
        .borders(Borders::ALL);
    let para = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(para, area);
}

/// Tagsタブを描画
fn render_tags(frame: &mut Frame, detail: &SecretDetail, selected_index: usize, area: Rect) {
    let headers = Row::new(vec!["Key", "Value"]).style(theme::header());

    let mut sorted_tags: Vec<(&String, &String)> = detail.tags.iter().collect();
    sorted_tags.sort_by_key(|(k, _)| k.as_str());

    let rows: Vec<Row> = sorted_tags
        .iter()
        .map(|(k, v)| Row::new(vec![k.as_str(), v.as_str()]))
        .collect();

    let widths = vec![Constraint::Percentage(30), Constraint::Percentage(70)];

    let table = SelectableTable::new(headers, rows, widths);
    let widget = SelectableTableWidget::new(table, selected_index);
    frame.render_widget(widget, area);
}

/// 詳細画面の1行を生成
fn detail_line<'a>(label: &'a str, value: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("  {:<14}", label), theme::header()),
        Span::raw(value),
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

    fn create_test_detail() -> SecretDetail {
        let mut tags = HashMap::new();
        tags.insert("env".to_string(), "production".to_string());
        tags.insert("team".to_string(), "backend".to_string());

        SecretDetail {
            name: "db-password".to_string(),
            arn: "arn:aws:secretsmanager:ap-northeast-1:123456789012:secret:db-password-abc123"
                .to_string(),
            description: Some("Database password for production".to_string()),
            kms_key_id: Some("arn:aws:kms:ap-northeast-1:123456789012:key/my-key".to_string()),
            rotation_enabled: true,
            rotation_lambda_arn: Some(
                "arn:aws:lambda:ap-northeast-1:123456789012:function:rotate".to_string(),
            ),
            last_rotated_date: Some("2025-01-10T12:00:00Z".to_string()),
            last_changed_date: Some("2025-01-15T10:30:00Z".to_string()),
            last_accessed_date: Some("2025-01-20T08:00:00Z".to_string()),
            created_date: Some("2024-06-01T09:00:00Z".to_string()),
            tags,
            version_ids: vec!["v1-abc123".to_string(), "v2-def456".to_string()],
        }
    }

    #[test]
    fn render_returns_tab_bar_when_overview_tab() {
        let detail = create_test_detail();
        let backend = TestBackend::new(90, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &detail,
                    0,
                    &SecretsDetailTab::Overview,
                    Some("dev-account"),
                    Some("ap-northeast-1"),
                    frame.area(),
                );
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("[Overview]"));
        assert!(content.contains("[Tags]"));
    }

    #[test]
    fn render_returns_secret_info_when_overview_tab() {
        let detail = create_test_detail();
        let backend = TestBackend::new(90, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &detail,
                    0,
                    &SecretsDetailTab::Overview,
                    None,
                    None,
                    frame.area(),
                );
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("db-password"));
        assert!(content.contains("Database password"));
        assert!(content.contains("Enabled"));
    }

    #[test]
    fn render_returns_tags_table_when_tags_tab() {
        let detail = create_test_detail();
        let backend = TestBackend::new(90, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &detail,
                    0,
                    &SecretsDetailTab::Tags,
                    None,
                    None,
                    frame.area(),
                );
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Key"));
        assert!(content.contains("Value"));
        assert!(content.contains("env"));
        assert!(content.contains("production"));
        assert!(content.contains("team"));
        assert!(content.contains("backend"));
    }

    #[test]
    fn render_returns_right_title_when_profile_and_region_set() {
        let detail = create_test_detail();
        let backend = TestBackend::new(90, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &detail,
                    0,
                    &SecretsDetailTab::Overview,
                    Some("dev-account"),
                    Some("ap-northeast-1"),
                    frame.area(),
                );
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("dev-account"));
        assert!(content.contains("ap-northeast-1"));
        assert!(content.contains("Tab:switch-tab"));
    }

    #[test]
    fn render_returns_overview_snapshot_when_rendered() {
        let detail = create_test_detail();
        let backend = TestBackend::new(90, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &detail,
                    0,
                    &SecretsDetailTab::Overview,
                    Some("dev-account"),
                    Some("ap-northeast-1"),
                    frame.area(),
                );
            })
            .unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }

    #[test]
    fn render_returns_tags_snapshot_when_tags_tab() {
        let detail = create_test_detail();
        let backend = TestBackend::new(90, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &detail,
                    0,
                    &SecretsDetailTab::Tags,
                    Some("dev-account"),
                    Some("ap-northeast-1"),
                    frame.area(),
                );
            })
            .unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }

    #[test]
    fn render_returns_rotation_disabled_when_not_enabled() {
        let mut detail = create_test_detail();
        detail.rotation_enabled = false;
        detail.rotation_lambda_arn = None;
        detail.last_rotated_date = None;
        let backend = TestBackend::new(90, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &detail,
                    0,
                    &SecretsDetailTab::Overview,
                    None,
                    None,
                    frame.area(),
                );
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Disabled"));
    }

    #[test]
    fn build_right_title_returns_none_when_no_profile_no_region() {
        assert!(build_right_title(None, None).is_none());
    }

    #[test]
    fn build_right_title_returns_both_when_profile_and_region() {
        let title = build_right_title(Some("dev"), Some("ap-northeast-1"));
        assert_eq!(title, Some(" dev │ ap-northeast-1 ".to_string()));
    }
}
