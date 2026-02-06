use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Wrap};

use crate::aws::ecs_model::{Cluster, Service};
use crate::tui::components::loading::Loading;
use crate::tui::components::status_bar::StatusBar;
use crate::tui::components::table::{SelectableTable, SelectableTableWidget};
use crate::tui::theme;

/// ECSクラスター詳細画面を描画する
pub fn render(
    frame: &mut Frame,
    cluster: &Cluster,
    services: &[Service],
    selected_index: usize,
    loading: bool,
) {
    let area = frame.area();

    let outer_chunks = Layout::vertical([
        Constraint::Min(1),    // 外枠（クラスタ情報 + サービステーブル）
        Constraint::Length(1), // ステータスバー
    ])
    .split(area);

    let title = format!(" {} ", cluster.cluster_name);
    let outer_block = Block::default().title(title).borders(Borders::ALL);
    let inner = outer_block.inner(outer_chunks[0]);
    frame.render_widget(outer_block, outer_chunks[0]);

    // 内側レイアウト: クラスタ概要 + サービス一覧
    let inner_chunks = Layout::vertical([
        Constraint::Length(5), // クラスタ概要
        Constraint::Min(1),    // サービステーブル
    ])
    .split(inner);

    render_cluster_overview(frame, cluster, inner_chunks[0]);

    if loading {
        let loading_widget = Loading::new("Loading services...", 0);
        frame.render_widget(loading_widget, inner_chunks[1]);
    } else {
        render_services_table(frame, services, selected_index, inner_chunks[1]);
    }

    // ステータスバー
    let keybinds = "j/k:move Esc:back ?:help";
    let status = StatusBar::new(keybinds);
    frame.render_widget(status, outer_chunks[1]);
}

/// クラスター概要を描画
fn render_cluster_overview(frame: &mut Frame, cluster: &Cluster, area: Rect) {
    let overview_block = Block::default()
        .title(" Cluster Info ")
        .borders(Borders::ALL);
    let running = cluster.running_tasks_count.to_string();
    let pending = cluster.pending_tasks_count.to_string();
    let lines = vec![
        detail_line("Status", &cluster.status),
        detail_line("Running Tasks", &running),
        detail_line("Pending Tasks", &pending),
    ];
    let para = Paragraph::new(lines)
        .block(overview_block)
        .wrap(Wrap { trim: false });
    frame.render_widget(para, area);
}

/// サービス一覧テーブルを描画
fn render_services_table(
    frame: &mut Frame,
    services: &[Service],
    selected_index: usize,
    area: Rect,
) {
    let headers = Row::new(vec![
        "Service Name",
        "Status",
        "Desired",
        "Running",
        "Pending",
        "Task Definition",
        "Launch Type",
    ])
    .style(theme::header());

    let rows: Vec<Row> = services.iter().map(service_to_row).collect();

    let widths = vec![
        Constraint::Length(20),
        Constraint::Length(10),
        Constraint::Length(9),
        Constraint::Length(9),
        Constraint::Length(9),
        Constraint::Length(30),
        Constraint::Min(12),
    ];

    let table = SelectableTable::new(headers, rows, widths);
    let widget = SelectableTableWidget::new(table, selected_index);
    frame.render_widget(widget, area);
}

/// サービスをテーブル行に変換
fn service_to_row(service: &Service) -> Row<'_> {
    let status_style = match service.status.as_str() {
        "ACTIVE" => theme::state_running(),
        "DRAINING" => theme::state_transitioning(),
        "INACTIVE" => theme::state_stopped(),
        _ => theme::state_transitioning(),
    };

    Row::new(vec![
        Line::from(service.service_name.as_str()),
        Line::from(Span::styled(service.status.as_str(), status_style)),
        Line::from(service.desired_count.to_string()),
        Line::from(service.running_count.to_string()),
        Line::from(service.pending_count.to_string()),
        Line::from(service.task_definition.as_str()),
        Line::from(service.launch_type.as_deref().unwrap_or("-")),
    ])
}

/// 詳細画面の1行を生成
fn detail_line<'a>(label: &'a str, value: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("  {:<16}", label), theme::header()),
        Span::raw(value),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

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

    fn create_test_cluster() -> Cluster {
        Cluster {
            cluster_name: "web-cluster".to_string(),
            cluster_arn: "arn:aws:ecs:ap-northeast-1:123456789012:cluster/web-cluster".to_string(),
            status: "ACTIVE".to_string(),
            running_tasks_count: 10,
            pending_tasks_count: 2,
            active_services_count: 3,
            registered_container_instances_count: 4,
        }
    }

    fn create_test_service(name: &str, status: &str, desired: i32, running: i32) -> Service {
        Service {
            service_name: name.to_string(),
            service_arn: format!(
                "arn:aws:ecs:ap-northeast-1:123456789012:service/web-cluster/{}",
                name
            ),
            cluster_arn: "arn:aws:ecs:ap-northeast-1:123456789012:cluster/web-cluster".to_string(),
            status: status.to_string(),
            desired_count: desired,
            running_count: running,
            pending_count: 0,
            task_definition: "arn:aws:ecs:ap-northeast-1:123456789012:task-definition/my-task:1"
                .to_string(),
            launch_type: Some("FARGATE".to_string()),
        }
    }

    #[test]
    fn render_returns_cluster_title_when_rendered() {
        let cluster = create_test_cluster();
        let services = vec![];
        let backend = TestBackend::new(110, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &cluster, &services, 0, false))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("web-cluster"));
    }

    #[test]
    fn render_returns_cluster_overview_when_rendered() {
        let cluster = create_test_cluster();
        let services = vec![];
        let backend = TestBackend::new(110, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &cluster, &services, 0, false))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Cluster Info"));
        assert!(content.contains("ACTIVE"));
        assert!(content.contains("Running Tasks"));
    }

    #[test]
    fn render_returns_service_table_headers_when_services_exist() {
        let cluster = create_test_cluster();
        let services = vec![create_test_service("api-service", "ACTIVE", 3, 3)];
        let backend = TestBackend::new(110, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &cluster, &services, 0, false))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Service Name"));
        assert!(content.contains("Desired"));
        assert!(content.contains("Running"));
        assert!(content.contains("Pending"));
        assert!(content.contains("Task Definition"));
        assert!(content.contains("Launch Type"));
    }

    #[test]
    fn render_returns_service_data_when_services_provided() {
        let cluster = create_test_cluster();
        let services = vec![
            create_test_service("api-service", "ACTIVE", 3, 3),
            create_test_service("worker-service", "ACTIVE", 2, 2),
        ];
        let backend = TestBackend::new(110, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &cluster, &services, 0, false))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("api-service"));
        assert!(content.contains("worker-service"));
        assert!(content.contains("FARGATE"));
    }

    #[test]
    fn render_returns_loading_when_loading_state() {
        let cluster = create_test_cluster();
        let services = vec![];
        let backend = TestBackend::new(110, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &cluster, &services, 0, true))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Loading services..."));
    }

    #[test]
    fn render_returns_keybinds_when_rendered() {
        let cluster = create_test_cluster();
        let services = vec![];
        let backend = TestBackend::new(110, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &cluster, &services, 0, false))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("j/k:move"));
        assert!(content.contains("Esc:back"));
    }

    #[test]
    fn render_returns_snapshot_when_rendered() {
        let cluster = create_test_cluster();
        let services = vec![
            create_test_service("api-service", "ACTIVE", 3, 3),
            create_test_service("worker-service", "ACTIVE", 2, 1),
            create_test_service("cron-service", "DRAINING", 1, 0),
        ];
        let backend = TestBackend::new(110, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &cluster, &services, 0, false))
            .unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }

    #[test]
    fn service_to_row_returns_row_when_active_service() {
        let service = create_test_service("test", "ACTIVE", 1, 1);
        let row = service_to_row(&service);
        let _ = row;
    }

    #[test]
    fn service_to_row_returns_row_when_no_launch_type() {
        let mut service = create_test_service("test", "ACTIVE", 1, 1);
        service.launch_type = None;
        let row = service_to_row(&service);
        let _ = row;
    }
}
