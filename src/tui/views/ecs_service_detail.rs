use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Wrap};

use crate::aws::ecs_model::{Service, Task};
use crate::tui::components::loading::Loading;
use crate::tui::components::status_bar::StatusBar;
use crate::tui::components::table::{SelectableTable, SelectableTableWidget};
use crate::tui::theme;

/// ECSサービス詳細画面を描画する（サービス概要 + タスク一覧）
pub fn render(
    frame: &mut Frame,
    service: &Service,
    tasks: &[Task],
    selected_index: usize,
    loading: bool,
    spinner_tick: usize,
    area: Rect,
) {
    let outer_chunks = Layout::vertical([
        Constraint::Min(1),    // 外枠（サービス情報 + タスクテーブル）
        Constraint::Length(1), // ステータスバー
    ])
    .split(area);

    let title = format!(" {} ", service.service_name);
    let outer_block = Block::default().title(title).borders(Borders::ALL);
    let inner = outer_block.inner(outer_chunks[0]);
    frame.render_widget(outer_block, outer_chunks[0]);

    // 内側レイアウト: サービス概要 + タスク一覧
    let inner_chunks = Layout::vertical([
        Constraint::Length(14), // サービス概要
        Constraint::Min(1),     // タスクテーブル
    ])
    .split(inner);

    render_service_overview(frame, service, inner_chunks[0]);

    if loading {
        let loading_widget = Loading::new("Loading tasks...", spinner_tick);
        frame.render_widget(loading_widget, inner_chunks[1]);
    } else {
        render_tasks_table(frame, tasks, selected_index, inner_chunks[1]);
    }

    // ステータスバー
    let keybinds = "j/k:move Enter:detail Esc:back ?:help";
    let status = StatusBar::new(keybinds);
    frame.render_widget(status, outer_chunks[1]);
}

/// サービス概要を描画
fn render_service_overview(frame: &mut Frame, service: &Service, area: Rect) {
    let overview_block = Block::default()
        .title(" Service Overview ")
        .borders(Borders::ALL);

    let tasks = format!(
        "{} pending | {} running / {} desired",
        service.pending_count, service.running_count, service.desired_count
    );
    let health_check = service
        .health_check_grace_period_seconds
        .map(|s| format!("{} seconds", s))
        .unwrap_or_else(|| "-".to_string());

    let lines = vec![
        detail_line("Status", &service.status),
        detail_line("Tasks", &tasks),
        detail_line("Task Definition", &service.task_definition),
        detail_line(
            "Deploy Status",
            service.deployment_status.as_deref().unwrap_or("-"),
        ),
        detail_line("Launch Type", service.launch_type.as_deref().unwrap_or("-")),
        detail_line(
            "Scheduling",
            service.scheduling_strategy.as_deref().unwrap_or("-"),
        ),
        detail_line("Created", service.created_at.as_deref().unwrap_or("-")),
        detail_line("Health Check", &health_check),
        detail_line("Service ARN", &service.service_arn),
        detail_line("Cluster ARN", &service.cluster_arn),
    ];

    let para = Paragraph::new(lines)
        .block(overview_block)
        .wrap(Wrap { trim: false });
    frame.render_widget(para, area);
}

/// タスク一覧テーブルを描画
fn render_tasks_table(frame: &mut Frame, tasks: &[Task], selected_index: usize, area: Rect) {
    let headers = Row::new(vec![
        "Task ID",
        "Status",
        "Desired",
        "Health",
        "Launch Type",
        "Started At",
    ])
    .style(theme::header());

    let rows: Vec<Row> = tasks.iter().map(task_to_row).collect();

    let widths = vec![
        Constraint::Length(14),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(12),
        Constraint::Min(20),
    ];

    let table = SelectableTable::new(headers, rows, widths);
    let widget = SelectableTableWidget::new(table, selected_index);
    frame.render_widget(widget, area);
}

/// タスクをテーブル行に変換
fn task_to_row(task: &Task) -> Row<'_> {
    let status_style = match task.last_status.as_str() {
        "RUNNING" => theme::state_running(),
        "STOPPED" => theme::state_stopped(),
        _ => theme::state_transitioning(),
    };

    // タスクIDはARNの末尾部分を表示
    let task_id = task.task_arn.rsplit('/').next().unwrap_or(&task.task_arn);
    // 長すぎる場合は先頭12文字に切り詰め
    let task_id_short = if task_id.len() > 12 {
        &task_id[..12]
    } else {
        task_id
    };

    Row::new(vec![
        Line::from(task_id_short),
        Line::from(Span::styled(task.last_status.as_str(), status_style)),
        Line::from(task.desired_status.as_str()),
        Line::from(task.health_status.as_deref().unwrap_or("-")),
        Line::from(task.launch_type.as_deref().unwrap_or("-")),
        Line::from(task.started_at.as_deref().unwrap_or("-")),
    ])
}

/// 詳細画面の1行を生成
fn detail_line<'a>(label: &'a str, value: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("  {:<18}", label), theme::header()),
        Span::raw(value),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aws::ecs_model::Container;
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

    fn create_test_service() -> Service {
        Service {
            service_name: "api-service".to_string(),
            service_arn: "arn:aws:ecs:ap-northeast-1:123456789012:service/web-cluster/api-service"
                .to_string(),
            cluster_arn: "arn:aws:ecs:ap-northeast-1:123456789012:cluster/web-cluster".to_string(),
            status: "ACTIVE".to_string(),
            desired_count: 3,
            running_count: 3,
            pending_count: 0,
            task_definition: "arn:aws:ecs:ap-northeast-1:123456789012:task-definition/my-task:1"
                .to_string(),
            launch_type: Some("FARGATE".to_string()),
            scheduling_strategy: Some("REPLICA".to_string()),
            created_at: Some("2026-02-04T20:07:00Z".to_string()),
            health_check_grace_period_seconds: Some(0),
            deployment_status: Some("COMPLETED".to_string()),
            enable_execute_command: true,
        }
    }

    fn create_test_task(id: &str, status: &str, health: Option<&str>) -> Task {
        Task {
            task_arn: format!(
                "arn:aws:ecs:ap-northeast-1:123456789012:task/web-cluster/{}",
                id
            ),
            cluster_arn: "arn:aws:ecs:ap-northeast-1:123456789012:cluster/web-cluster".to_string(),
            task_definition_arn:
                "arn:aws:ecs:ap-northeast-1:123456789012:task-definition/my-task:1".to_string(),
            last_status: status.to_string(),
            desired_status: "RUNNING".to_string(),
            cpu: Some("256".to_string()),
            memory: Some("512".to_string()),
            launch_type: Some("FARGATE".to_string()),
            platform_version: Some("1.4.0".to_string()),
            health_status: health.map(|s| s.to_string()),
            connectivity: Some("CONNECTED".to_string()),
            availability_zone: Some("ap-northeast-1a".to_string()),
            started_at: Some("2026-02-04T20:07:00Z".to_string()),
            stopped_at: None,
            stopped_reason: None,
            containers: vec![Container {
                name: "app".to_string(),
                image: "nginx:latest".to_string(),
                last_status: "RUNNING".to_string(),
                exit_code: None,
                health_status: Some("HEALTHY".to_string()),
                reason: None,
            }],
        }
    }

    #[test]
    fn render_returns_service_title_when_rendered() {
        let service = create_test_service();
        let tasks = vec![];
        let backend = TestBackend::new(90, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &service, &tasks, 0, false, 0, frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("api-service"));
    }

    #[test]
    fn render_returns_service_overview_when_rendered() {
        let service = create_test_service();
        let tasks = vec![];
        let backend = TestBackend::new(90, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &service, &tasks, 0, false, 0, frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Service Overview"));
        assert!(content.contains("ACTIVE"));
        assert!(content.contains("FARGATE"));
        assert!(content.contains("REPLICA"));
        assert!(content.contains("COMPLETED"));
    }

    #[test]
    fn render_returns_task_table_headers_when_tasks_exist() {
        let service = create_test_service();
        let tasks = vec![create_test_task("abc123def456", "RUNNING", Some("HEALTHY"))];
        let backend = TestBackend::new(90, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &service, &tasks, 0, false, 0, frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Task ID"));
        assert!(content.contains("Status"));
        assert!(content.contains("Launch Type"));
        assert!(content.contains("Started At"));
    }

    #[test]
    fn render_returns_task_data_when_tasks_provided() {
        let service = create_test_service();
        let tasks = vec![
            create_test_task("abc123def456", "RUNNING", Some("HEALTHY")),
            create_test_task("ghi789jkl012", "RUNNING", None),
        ];
        let backend = TestBackend::new(90, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &service, &tasks, 0, false, 0, frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("abc123def456"));
        assert!(content.contains("ghi789jkl012"));
        assert!(content.contains("RUNNING"));
    }

    #[test]
    fn render_returns_loading_when_loading_state() {
        let service = create_test_service();
        let tasks = vec![];
        let backend = TestBackend::new(90, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &service, &tasks, 0, true, 0, frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Loading tasks..."));
    }

    #[test]
    fn render_returns_keybinds_when_rendered() {
        let service = create_test_service();
        let tasks = vec![];
        let backend = TestBackend::new(90, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &service, &tasks, 0, false, 0, frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("j/k:move"));
        assert!(content.contains("Enter:detail"));
        assert!(content.contains("Esc:back"));
        assert!(content.contains("?:help"));
    }

    #[test]
    fn render_returns_snapshot_when_rendered() {
        let service = create_test_service();
        let tasks = vec![
            create_test_task("abc123def456", "RUNNING", Some("HEALTHY")),
            create_test_task("ghi789jkl012", "RUNNING", None),
        ];
        let backend = TestBackend::new(90, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &service, &tasks, 0, false, 0, frame.area()))
            .unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }
}
