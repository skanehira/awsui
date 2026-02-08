use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Row};

use crate::aws::vpc_model::{Subnet, Vpc};
use crate::tui::components::loading::Loading;
use crate::tui::components::status_bar::StatusBar;
use crate::tui::components::table::{SelectableTable, SelectableTableWidget};
use crate::tui::theme;

/// VPC詳細画面（サブネット一覧）を描画する
pub fn render(
    frame: &mut Frame,
    vpc: &Vpc,
    subnets: &[Subnet],
    selected_index: usize,
    loading: bool,
    spinner_tick: usize,
    area: Rect,
) {
    // フッターは外枠の外に配置
    let outer_chunks = Layout::vertical([
        Constraint::Min(1),    // 外枠（テーブル）
        Constraint::Length(1), // ステータスバー
    ])
    .split(area);

    // 外枠Block: タイトルにVPC IDとVPC名を表示
    let title = format!(" {} ({}) - Subnets ", vpc.name, vpc.vpc_id);
    let outer_block = Block::default().title(title).borders(Borders::ALL);
    let inner = outer_block.inner(outer_chunks[0]);
    frame.render_widget(outer_block, outer_chunks[0]);

    // メインコンテンツ
    if loading {
        let loading_widget = Loading::new("Loading subnets...", spinner_tick);
        frame.render_widget(loading_widget, inner);
    } else {
        render_table(frame, subnets, selected_index, inner);
    }

    // ステータスバー
    let keybinds = "j/k:move y:copy-id Esc:back ?:help";
    let status = StatusBar::new(keybinds);
    frame.render_widget(status, outer_chunks[1]);
}

/// テーブルを描画
fn render_table(
    frame: &mut Frame,
    subnets: &[Subnet],
    selected_index: usize,
    area: ratatui::layout::Rect,
) {
    let headers = Row::new(vec![
        "Subnet ID",
        "Name",
        "CIDR",
        "AZ",
        "Available IPs",
        "State",
        "Default",
        "Public IP",
    ])
    .style(theme::header());

    let rows: Vec<Row> = subnets.iter().map(subnet_to_row).collect();

    let widths = vec![
        Constraint::Length(26),
        Constraint::Length(20),
        Constraint::Length(18),
        Constraint::Length(16),
        Constraint::Length(14),
        Constraint::Length(12),
        Constraint::Length(8),
        Constraint::Min(10),
    ];

    let table = SelectableTable::new(headers, rows, widths);
    let widget = SelectableTableWidget::new(table, selected_index);
    frame.render_widget(widget, area);
}

/// サブネットをテーブル行に変換
fn subnet_to_row(subnet: &Subnet) -> Row<'_> {
    let state_style = match subnet.state.as_str() {
        "available" => theme::state_running(),
        "pending" => theme::state_transitioning(),
        _ => theme::inactive(),
    };

    let default_text = if subnet.is_default { "Yes" } else { "No" };
    let public_ip_text = if subnet.map_public_ip_on_launch {
        "Yes"
    } else {
        "No"
    };

    Row::new(vec![
        Line::from(subnet.subnet_id.as_str()),
        Line::from(subnet.name.as_str()),
        Line::from(subnet.cidr_block.as_str()),
        Line::from(subnet.availability_zone.as_str()),
        Line::from(subnet.available_ip_count.to_string()),
        Line::from(ratatui::text::Span::styled(
            subnet.state.as_str(),
            state_style,
        )),
        Line::from(default_text),
        Line::from(public_ip_text),
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

    fn create_test_vpc() -> Vpc {
        Vpc {
            vpc_id: "vpc-0abc1234".to_string(),
            name: "main-vpc".to_string(),
            cidr_block: "10.0.0.0/16".to_string(),
            state: "available".to_string(),
            is_default: false,
            owner_id: "123456789012".to_string(),
            tags: HashMap::new(),
        }
    }

    fn create_test_subnet(
        id: &str,
        name: &str,
        cidr: &str,
        az: &str,
        ip_count: i32,
        is_default: bool,
        public_ip: bool,
    ) -> Subnet {
        Subnet {
            subnet_id: id.to_string(),
            name: name.to_string(),
            vpc_id: "vpc-0abc1234".to_string(),
            cidr_block: cidr.to_string(),
            availability_zone: az.to_string(),
            available_ip_count: ip_count,
            state: "available".to_string(),
            is_default,
            map_public_ip_on_launch: public_ip,
        }
    }

    #[test]
    fn render_returns_title_when_vpc_detail() {
        let vpc = create_test_vpc();
        let subnets: Vec<Subnet> = vec![];
        let backend = TestBackend::new(130, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &vpc, &subnets, 0, false, 0, frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("main-vpc"));
        assert!(content.contains("vpc-0abc1234"));
        assert!(content.contains("Subnets"));
    }

    #[test]
    fn render_returns_table_headers_when_subnets_exist() {
        let vpc = create_test_vpc();
        let subnets = vec![create_test_subnet(
            "subnet-001",
            "public-1a",
            "10.0.1.0/24",
            "ap-northeast-1a",
            251,
            false,
            true,
        )];
        let backend = TestBackend::new(130, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &vpc, &subnets, 0, false, 0, frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Subnet ID"));
        assert!(content.contains("Name"));
        assert!(content.contains("CIDR"));
        assert!(content.contains("AZ"));
        assert!(content.contains("Available IPs"));
        assert!(content.contains("State"));
    }

    #[test]
    fn render_returns_subnet_data_when_subnets_provided() {
        let vpc = create_test_vpc();
        let subnets = vec![
            create_test_subnet(
                "subnet-001",
                "public-1a",
                "10.0.1.0/24",
                "ap-northeast-1a",
                251,
                false,
                true,
            ),
            create_test_subnet(
                "subnet-002",
                "private-1c",
                "10.0.2.0/24",
                "ap-northeast-1c",
                250,
                false,
                false,
            ),
        ];
        let backend = TestBackend::new(130, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &vpc, &subnets, 0, false, 0, frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("subnet-001"));
        assert!(content.contains("public-1a"));
        assert!(content.contains("10.0.1.0/24"));
        assert!(content.contains("ap-northeast-1a"));
        assert!(content.contains("251"));
        assert!(content.contains("subnet-002"));
        assert!(content.contains("private-1c"));
    }

    #[test]
    fn render_returns_loading_when_loading_state() {
        let vpc = create_test_vpc();
        let subnets: Vec<Subnet> = vec![];
        let backend = TestBackend::new(130, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &vpc, &subnets, 0, true, 0, frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Loading subnets..."));
    }

    #[test]
    fn render_returns_keybinds_when_rendered() {
        let vpc = create_test_vpc();
        let subnets: Vec<Subnet> = vec![];
        let backend = TestBackend::new(130, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &vpc, &subnets, 0, false, 0, frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("j/k:move"));
        assert!(content.contains("Esc:back"));
    }

    #[test]
    fn render_returns_snapshot_when_rendered() {
        let vpc = create_test_vpc();
        let subnets = vec![
            create_test_subnet(
                "subnet-0abc1234",
                "public-1a",
                "10.0.1.0/24",
                "ap-northeast-1a",
                251,
                false,
                true,
            ),
            create_test_subnet(
                "subnet-0def5678",
                "private-1c",
                "10.0.2.0/24",
                "ap-northeast-1c",
                250,
                true,
                false,
            ),
            create_test_subnet(
                "subnet-0ghi9012",
                "private-1d",
                "10.0.3.0/24",
                "ap-northeast-1d",
                248,
                false,
                false,
            ),
        ];
        let backend = TestBackend::new(130, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &vpc, &subnets, 0, false, 0, frame.area()))
            .unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }
}
