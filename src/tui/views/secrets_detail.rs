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
    Rotation,
    Versions,
    Tags,
}

/// シークレット詳細画面を描画する
#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    detail: &SecretDetail,
    tag_selected_index: usize,
    detail_tab: &SecretsDetailTab,
    value_visible: bool,
    profile: Option<&str>,
    region: Option<&str>,
    area: Rect,
) {
    let outer_chunks = Layout::vertical([
        Constraint::Min(1),    // 外枠（ヘッダー + タブバー + コンテンツ）
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

    // 内側レイアウト: ヘッダー + タブバー + コンテンツ
    let inner_chunks = Layout::vertical([
        Constraint::Length(5), // 固定ヘッダー（Secret Details 2カラム）
        Constraint::Length(1), // タブバー
        Constraint::Min(1),    // タブコンテンツ
    ])
    .split(inner);

    // ヘッダー（Secret Details 2カラム）
    render_header(frame, detail, inner_chunks[0]);

    // タブバー
    render_tab_bar(frame, detail_tab, inner_chunks[1]);

    // コンテンツ
    match detail_tab {
        SecretsDetailTab::Overview => {
            render_overview(frame, detail, value_visible, inner_chunks[2]);
        }
        SecretsDetailTab::Rotation => render_rotation(frame, detail, inner_chunks[2]),
        SecretsDetailTab::Versions => {
            render_versions(frame, detail, tag_selected_index, inner_chunks[2]);
        }
        SecretsDetailTab::Tags => {
            render_tags(frame, detail, tag_selected_index, inner_chunks[2]);
        }
    }

    // ステータスバー
    let keybinds = match detail_tab {
        SecretsDetailTab::Overview => {
            "[/]:switch-tab v:show/hide-value e:edit y:copy-arn Esc:back ?:help"
        }
        SecretsDetailTab::Rotation => "[/]:switch-tab y:copy-arn Esc:back ?:help",
        SecretsDetailTab::Versions => "[/]:switch-tab y:copy-arn Esc:back ?:help",
        SecretsDetailTab::Tags => "[/]:switch-tab y:copy-value Esc:back ?:help",
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

/// ヘッダー（Secret Details 2カラム）を描画
fn render_header(frame: &mut Frame, detail: &SecretDetail, area: Rect) {
    let block = Block::default()
        .title(" Secret Details ")
        .borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let cols =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(inner);

    // 左カラム
    let truncated_arn = truncate_str(&detail.arn, cols[0].width.saturating_sub(16) as usize);
    let left_lines = vec![
        detail_line("KMS Key", detail.kms_key_id.as_deref().unwrap_or("-")),
        detail_line("Name", &detail.name),
        detail_line("ARN", &truncated_arn),
    ];
    let left_para = Paragraph::new(left_lines).wrap(Wrap { trim: false });
    frame.render_widget(left_para, cols[0]);

    // 右カラム
    let right_lines = vec![
        detail_line("Description", detail.description.as_deref().unwrap_or("-")),
        detail_line("Type", "-"),
    ];
    let right_para = Paragraph::new(right_lines).wrap(Wrap { trim: false });
    frame.render_widget(right_para, cols[1]);
}

/// タブバーを描画
fn render_tab_bar(frame: &mut Frame, current_tab: &SecretsDetailTab, area: Rect) {
    let tab_style = |tab: &SecretsDetailTab| {
        if current_tab == tab {
            theme::active()
        } else {
            theme::inactive()
        }
    };

    let tabs = Line::from(vec![
        Span::raw(" "),
        Span::styled("[Overview]", tab_style(&SecretsDetailTab::Overview)),
        Span::raw(" "),
        Span::styled("[Rotation]", tab_style(&SecretsDetailTab::Rotation)),
        Span::raw(" "),
        Span::styled("[Versions]", tab_style(&SecretsDetailTab::Versions)),
        Span::raw(" "),
        Span::styled("[Tags]", tab_style(&SecretsDetailTab::Tags)),
    ]);
    frame.render_widget(Paragraph::new(tabs), area);
}

/// Overviewタブを描画（Secret Value セクション）
fn render_overview(frame: &mut Frame, detail: &SecretDetail, value_visible: bool, area: Rect) {
    let block = Block::default()
        .title(" Secret Value ")
        .borders(Borders::ALL);

    let content = match (&detail.secret_value, value_visible) {
        (None, _) => "  Press 'v' to retrieve and show secret value".to_string(),
        (Some(_), false) => "  ********".to_string(),
        (Some(val), true) => format!("  {}", val),
    };

    let para = Paragraph::new(content)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(para, area);
}

/// Rotationタブを描画
fn render_rotation(frame: &mut Frame, detail: &SecretDetail, area: Rect) {
    let rotation_status = if detail.rotation_enabled {
        "Enabled"
    } else {
        "Disabled"
    };

    let interval = detail
        .rotation_days
        .map(|d| format!("{} days", d))
        .unwrap_or_else(|| "-".to_string());

    let lines = vec![
        detail_line("Status", rotation_status),
        detail_line(
            "Lambda ARN",
            detail.rotation_lambda_arn.as_deref().unwrap_or("-"),
        ),
        detail_line("Interval", &interval),
        detail_line(
            "Last Rotated",
            detail.last_rotated_date.as_deref().unwrap_or("-"),
        ),
    ];

    let block = Block::default()
        .title(" Rotation Configuration ")
        .borders(Borders::ALL);
    let para = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(para, area);
}

/// Versionsタブを描画
fn render_versions(frame: &mut Frame, detail: &SecretDetail, selected_index: usize, area: Rect) {
    let headers = Row::new(vec!["Version ID", "Staging Labels"]).style(theme::header());

    let version_data: Vec<(String, String)> = detail
        .version_stages
        .iter()
        .map(|v| (v.version_id.clone(), v.staging_labels.join(", ")))
        .collect();

    let rows: Vec<Row> = version_data
        .iter()
        .map(|(id, labels)| Row::new(vec![id.as_str(), labels.as_str()]))
        .collect();

    let widths = vec![Constraint::Percentage(50), Constraint::Percentage(50)];

    let table = SelectableTable::new(headers, rows, widths);
    let widget = SelectableTableWidget::new(table, selected_index);
    frame.render_widget(widget, area);
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

/// 文字列を指定幅で切り詰める
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aws::secrets_model::SecretVersion;
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
            rotation_days: Some(30),
            last_rotated_date: Some("2025-01-10T12:00:00Z".to_string()),
            last_changed_date: Some("2025-01-15T10:30:00Z".to_string()),
            last_accessed_date: Some("2025-01-20T08:00:00Z".to_string()),
            created_date: Some("2024-06-01T09:00:00Z".to_string()),
            tags,
            version_ids: vec!["v1-abc123".to_string(), "v2-def456".to_string()],
            version_stages: vec![
                SecretVersion {
                    version_id: "v1-abc123".to_string(),
                    staging_labels: vec!["AWSPREVIOUS".to_string()],
                },
                SecretVersion {
                    version_id: "v2-def456".to_string(),
                    staging_labels: vec!["AWSCURRENT".to_string()],
                },
            ],
            secret_value: None,
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
                    false,
                    Some("dev-account"),
                    Some("ap-northeast-1"),
                    frame.area(),
                );
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("[Overview]"));
        assert!(content.contains("[Rotation]"));
        assert!(content.contains("[Versions]"));
        assert!(content.contains("[Tags]"));
    }

    #[test]
    fn render_returns_header_two_columns_when_overview_tab() {
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
                    false,
                    None,
                    None,
                    frame.area(),
                );
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Secret Details"));
        assert!(content.contains("KMS Key"));
        assert!(content.contains("Description"));
        assert!(content.contains("db-password"));
    }

    #[test]
    fn render_returns_secret_value_prompt_when_value_not_loaded() {
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
                    false,
                    None,
                    None,
                    frame.area(),
                );
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Press 'v' to retrieve and show secret value"));
    }

    #[test]
    fn render_returns_masked_value_when_value_loaded_but_hidden() {
        let mut detail = create_test_detail();
        detail.secret_value = Some("my-secret-password".to_string());
        let backend = TestBackend::new(90, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &detail,
                    0,
                    &SecretsDetailTab::Overview,
                    false,
                    None,
                    None,
                    frame.area(),
                );
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("********"));
        assert!(!content.contains("my-secret-password"));
    }

    #[test]
    fn render_returns_plain_value_when_value_visible() {
        let mut detail = create_test_detail();
        detail.secret_value = Some("my-secret-password".to_string());
        let backend = TestBackend::new(90, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &detail,
                    0,
                    &SecretsDetailTab::Overview,
                    true,
                    None,
                    None,
                    frame.area(),
                );
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("my-secret-password"));
    }

    #[test]
    fn render_returns_rotation_info_when_rotation_tab() {
        let detail = create_test_detail();
        let backend = TestBackend::new(90, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &detail,
                    0,
                    &SecretsDetailTab::Rotation,
                    false,
                    None,
                    None,
                    frame.area(),
                );
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Rotation Configuration"));
        assert!(content.contains("Enabled"));
        assert!(content.contains("30 days"));
        assert!(content.contains("Lambda ARN"));
    }

    #[test]
    fn render_returns_versions_table_when_versions_tab() {
        let detail = create_test_detail();
        let backend = TestBackend::new(90, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &detail,
                    0,
                    &SecretsDetailTab::Versions,
                    false,
                    None,
                    None,
                    frame.area(),
                );
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Version ID"));
        assert!(content.contains("Staging Labels"));
        assert!(content.contains("v1-abc123"));
        assert!(content.contains("AWSPREVIOUS"));
        assert!(content.contains("v2-def456"));
        assert!(content.contains("AWSCURRENT"));
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
                    false,
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
                    false,
                    Some("dev-account"),
                    Some("ap-northeast-1"),
                    frame.area(),
                );
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("dev-account"));
        assert!(content.contains("ap-northeast-1"));
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
                    false,
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
                    false,
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
        detail.rotation_days = None;
        let backend = TestBackend::new(90, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &detail,
                    0,
                    &SecretsDetailTab::Rotation,
                    false,
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
