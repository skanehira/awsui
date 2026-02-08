use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Row};
use tui_input::Input;

use crate::app::Mode;
use crate::aws::vpc_model::Vpc;
use crate::tui::components::loading::Loading;
use crate::tui::components::status_bar::render_footer;
use crate::tui::components::table::{SelectableTable, SelectableTableWidget};
use crate::tui::theme;

/// VPC一覧画面を描画する
#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    vpcs: &[Vpc],
    selected_index: usize,
    filter_input: &Input,
    mode: &Mode,
    loading: bool,
    spinner_tick: usize,
    profile: Option<&str>,
    region: Option<&str>,
    area: Rect,
) {
    // フッターは外枠の外に配置
    let outer_chunks = Layout::vertical([
        Constraint::Min(1),    // 外枠（テーブル）
        Constraint::Length(1), // フッター
    ])
    .split(area);

    // 右タイトル（profile │ region）
    let right_title = build_right_title(profile, region);

    // 外枠Block
    let mut outer_block = Block::default().title(" VPCs ").borders(Borders::ALL);
    if let Some(title) = right_title {
        outer_block = outer_block.title_top(Line::from(title).alignment(Alignment::Right));
    }
    let inner = outer_block.inner(outer_chunks[0]);
    frame.render_widget(outer_block, outer_chunks[0]);

    // メインコンテンツ
    if loading {
        let loading_widget = Loading::new("Loading VPCs...", spinner_tick);
        frame.render_widget(loading_widget, inner);
    } else {
        render_table(frame, vpcs, selected_index, inner);
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
fn render_table(
    frame: &mut Frame,
    vpcs: &[Vpc],
    selected_index: usize,
    area: ratatui::layout::Rect,
) {
    let headers =
        Row::new(vec!["VPC ID", "Name", "CIDR", "State", "Default"]).style(theme::header());

    let rows: Vec<Row> = vpcs.iter().map(vpc_to_row).collect();

    let widths = vec![
        Constraint::Length(24),
        Constraint::Length(20),
        Constraint::Length(18),
        Constraint::Length(12),
        Constraint::Min(8),
    ];

    let table = SelectableTable::new(headers, rows, widths);
    let widget = SelectableTableWidget::new(table, selected_index);
    frame.render_widget(widget, area);
}

/// VPCをテーブル行に変換
fn vpc_to_row(vpc: &Vpc) -> Row<'_> {
    let state_style = match vpc.state.as_str() {
        "available" => theme::state_running(),
        "pending" => theme::state_transitioning(),
        _ => theme::inactive(),
    };

    let default_text = if vpc.is_default { "Yes" } else { "No" };

    Row::new(vec![
        Line::from(vpc.vpc_id.as_str()),
        Line::from(vpc.name.as_str()),
        Line::from(vpc.cidr_block.as_str()),
        Line::from(Span::styled(vpc.state.as_str(), state_style)),
        Line::from(default_text),
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

    fn create_test_vpc(id: &str, name: &str, cidr: &str, state: &str, is_default: bool) -> Vpc {
        Vpc {
            vpc_id: id.to_string(),
            name: name.to_string(),
            cidr_block: cidr.to_string(),
            state: state.to_string(),
            is_default,
            owner_id: "123456789012".to_string(),
            tags: HashMap::new(),
        }
    }

    #[test]
    fn render_returns_header_when_vpc_list() {
        let vpcs: Vec<Vpc> = vec![];
        let input = Input::default();
        let mode = Mode::Normal;
        let backend = TestBackend::new(90, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &vpcs,
                    0,
                    &input,
                    &mode,
                    false,
                    0,
                    None,
                    None,
                    frame.area(),
                )
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("VPCs"));
    }

    #[test]
    fn render_returns_table_headers_when_vpcs_exist() {
        let vpcs = vec![create_test_vpc(
            "vpc-001",
            "main-vpc",
            "10.0.0.0/16",
            "available",
            false,
        )];
        let input = Input::default();
        let mode = Mode::Normal;
        let backend = TestBackend::new(90, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &vpcs,
                    0,
                    &input,
                    &mode,
                    false,
                    0,
                    None,
                    None,
                    frame.area(),
                )
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("VPC ID"));
        assert!(content.contains("Name"));
        assert!(content.contains("CIDR"));
        assert!(content.contains("State"));
        assert!(content.contains("Default"));
    }

    #[test]
    fn render_returns_vpc_data_when_vpcs_provided() {
        let vpcs = vec![
            create_test_vpc("vpc-001", "main-vpc", "10.0.0.0/16", "available", true),
            create_test_vpc("vpc-002", "dev-vpc", "172.16.0.0/16", "available", false),
        ];
        let input = Input::default();
        let mode = Mode::Normal;
        let backend = TestBackend::new(90, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &vpcs,
                    0,
                    &input,
                    &mode,
                    false,
                    0,
                    None,
                    None,
                    frame.area(),
                )
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("vpc-001"));
        assert!(content.contains("main-vpc"));
        assert!(content.contains("10.0.0.0/16"));
        assert!(content.contains("available"));
        assert!(content.contains("Yes"));
        assert!(content.contains("vpc-002"));
        assert!(content.contains("dev-vpc"));
        assert!(content.contains("No"));
    }

    #[test]
    fn render_returns_right_title_when_profile_and_region_set() {
        let vpcs = vec![create_test_vpc(
            "vpc-001",
            "main",
            "10.0.0.0/16",
            "available",
            false,
        )];
        let input = Input::default();
        let mode = Mode::Normal;
        let backend = TestBackend::new(90, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &vpcs,
                    0,
                    &input,
                    &mode,
                    false,
                    0,
                    Some("dev-account"),
                    Some("ap-northeast-1"),
                    frame.area(),
                )
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("dev-account"));
        assert!(content.contains("ap-northeast-1"));
    }

    #[test]
    fn render_returns_loading_when_loading_state() {
        let vpcs: Vec<Vpc> = vec![];
        let input = Input::default();
        let mode = Mode::Normal;
        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &vpcs,
                    0,
                    &input,
                    &mode,
                    true,
                    0,
                    None,
                    None,
                    frame.area(),
                )
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Loading VPCs..."));
    }

    #[test]
    fn render_returns_filter_input_when_filter_mode() {
        let vpcs = vec![create_test_vpc(
            "vpc-001",
            "main",
            "10.0.0.0/16",
            "available",
            false,
        )];
        let input = Input::from("main");
        let mode = Mode::Filter;
        let backend = TestBackend::new(90, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &vpcs,
                    0,
                    &input,
                    &mode,
                    false,
                    0,
                    None,
                    None,
                    frame.area(),
                )
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("/main"));
    }

    #[test]
    fn render_returns_keybinds_when_normal_mode() {
        let vpcs: Vec<Vpc> = vec![];
        let input = Input::default();
        let mode = Mode::Normal;
        let backend = TestBackend::new(90, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &vpcs,
                    0,
                    &input,
                    &mode,
                    false,
                    0,
                    None,
                    None,
                    frame.area(),
                )
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("j/k:move"));
    }

    #[test]
    fn render_returns_snapshot_when_rendered() {
        let vpcs = vec![
            create_test_vpc("vpc-0abc1234", "main-vpc", "10.0.0.0/16", "available", true),
            create_test_vpc(
                "vpc-0def5678",
                "dev-vpc",
                "172.16.0.0/16",
                "available",
                false,
            ),
            create_test_vpc(
                "vpc-0ghi9012",
                "staging",
                "192.168.0.0/16",
                "pending",
                false,
            ),
        ];
        let input = Input::default();
        let mode = Mode::Normal;
        let backend = TestBackend::new(90, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &vpcs,
                    0,
                    &input,
                    &mode,
                    false,
                    0,
                    Some("dev-account"),
                    Some("ap-northeast-1"),
                    frame.area(),
                )
            })
            .unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }
}
