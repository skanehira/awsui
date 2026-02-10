use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Wrap};

use crate::aws::ecs_model::Task;
use crate::tui::components::status_bar::StatusBar;
use crate::tui::components::table::{SelectableTable, SelectableTableWidget};
use crate::tui::theme;

/// ECSタスク詳細画面を描画する（タスク概要 + コンテナテーブル）
pub fn render(frame: &mut Frame, task: &Task, area: Rect) {
    let outer_chunks = Layout::vertical([
        Constraint::Min(1),    // 外枠（タスク情報）
        Constraint::Length(1), // ステータスバー
    ])
    .split(area);

    // タスクIDをARNの末尾から取得
    let task_id = task
        .task_arn
        .rsplit('/')
        .next()
        .unwrap_or(&task.task_arn);
    let task_id_short = if task_id.len() > 12 {
        &task_id[..12]
    } else {
        task_id
    };
    let title = format!(" {} ", task_id_short);
    let outer_block = Block::default().title(title).borders(Borders::ALL);
    let inner = outer_block.inner(outer_chunks[0]);
    frame.render_widget(outer_block, outer_chunks[0]);

    // 内側レイアウト: タスク概要 + コンテナテーブル
    let inner_chunks = Layout::vertical([
        Constraint::Length(13), // タスク概要
        Constraint::Min(1),     // コンテナテーブル
    ])
    .split(inner);

    render_task_overview(frame, task, inner_chunks[0]);
    render_containers_table(frame, task, inner_chunks[1]);

    // ステータスバー
    let keybinds = "Esc:back ?:help";
    let status = StatusBar::new(keybinds);
    frame.render_widget(status, outer_chunks[1]);
}

/// タスク概要を描画
fn render_task_overview(frame: &mut Frame, task: &Task, area: Rect) {
    let overview_block = Block::default()
        .title(" Task Overview ")
        .borders(Borders::ALL);

    let cpu_memory = format!(
        "{} / {}",
        task.cpu.as_deref().unwrap_or("-"),
        task.memory.as_deref().unwrap_or("-")
    );

    let lines = vec![
        detail_line("Status", &task.last_status),
        detail_line("Desired", &task.desired_status),
        detail_line("Health", task.health_status.as_deref().unwrap_or("-")),
        detail_line("Task Definition", &task.task_definition_arn),
        detail_line("Launch Type", task.launch_type.as_deref().unwrap_or("-")),
        detail_line("CPU / Memory", &cpu_memory),
        detail_line(
            "Platform",
            task.platform_version.as_deref().unwrap_or("-"),
        ),
        detail_line("AZ", task.availability_zone.as_deref().unwrap_or("-")),
        detail_line("Started", task.started_at.as_deref().unwrap_or("-")),
        detail_line("Task ARN", &task.task_arn),
    ];

    let para = Paragraph::new(lines)
        .block(overview_block)
        .wrap(Wrap { trim: false });
    frame.render_widget(para, area);
}

/// コンテナテーブルを描画
fn render_containers_table(frame: &mut Frame, task: &Task, area: Rect) {
    let block = Block::default()
        .title(" Containers ")
        .borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let headers = Row::new(vec!["Name", "Image", "Status", "Exit", "Health"])
        .style(theme::header());

    let rows: Vec<Row> = task
        .containers
        .iter()
        .map(|c| {
            let exit_code = c
                .exit_code
                .map(|code| code.to_string())
                .unwrap_or_else(|| "-".to_string());

            let status_style = match c.last_status.as_str() {
                "RUNNING" => theme::state_running(),
                "STOPPED" => theme::state_stopped(),
                _ => theme::state_transitioning(),
            };

            Row::new(vec![
                Line::from(c.name.as_str()),
                Line::from(c.image.as_str()),
                Line::from(Span::styled(c.last_status.as_str(), status_style)),
                Line::from(exit_code),
                Line::from(c.health_status.as_deref().unwrap_or("-")),
            ])
        })
        .collect();

    let widths = vec![
        Constraint::Length(15),
        Constraint::Length(25),
        Constraint::Length(10),
        Constraint::Length(6),
        Constraint::Min(10),
    ];

    let table = SelectableTable::new(headers, rows, widths);
    // コンテナテーブルは選択不要なので index=0 を固定
    let widget = SelectableTableWidget::new(table, 0);
    frame.render_widget(widget, inner);
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

    fn create_test_task() -> Task {
        Task {
            task_arn: "arn:aws:ecs:ap-northeast-1:123456789012:task/web-cluster/abc123def456"
                .to_string(),
            cluster_arn: "arn:aws:ecs:ap-northeast-1:123456789012:cluster/web-cluster".to_string(),
            task_definition_arn:
                "arn:aws:ecs:ap-northeast-1:123456789012:task-definition/my-task:1".to_string(),
            last_status: "RUNNING".to_string(),
            desired_status: "RUNNING".to_string(),
            cpu: Some("256".to_string()),
            memory: Some("512".to_string()),
            launch_type: Some("FARGATE".to_string()),
            platform_version: Some("1.4.0".to_string()),
            health_status: Some("HEALTHY".to_string()),
            connectivity: Some("CONNECTED".to_string()),
            availability_zone: Some("ap-northeast-1a".to_string()),
            started_at: Some("2026-02-04T20:07:00Z".to_string()),
            stopped_at: None,
            stopped_reason: None,
            containers: vec![
                Container {
                    name: "app".to_string(),
                    image: "nginx:latest".to_string(),
                    last_status: "RUNNING".to_string(),
                    exit_code: None,
                    health_status: Some("HEALTHY".to_string()),
                    reason: None,
                },
                Container {
                    name: "sidecar".to_string(),
                    image: "envoy:v1.28".to_string(),
                    last_status: "RUNNING".to_string(),
                    exit_code: None,
                    health_status: None,
                    reason: None,
                },
            ],
        }
    }

    #[test]
    fn render_returns_task_id_when_rendered() {
        let task = create_test_task();
        let backend = TestBackend::new(90, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &task, frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("abc123def456"));
    }

    #[test]
    fn render_returns_task_overview_when_rendered() {
        let task = create_test_task();
        let backend = TestBackend::new(90, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &task, frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Task Overview"));
        assert!(content.contains("RUNNING"));
        assert!(content.contains("HEALTHY"));
        assert!(content.contains("FARGATE"));
        assert!(content.contains("256 / 512"));
        assert!(content.contains("1.4.0"));
        assert!(content.contains("ap-northeast-1a"));
    }

    #[test]
    fn render_returns_container_table_when_rendered() {
        let task = create_test_task();
        let backend = TestBackend::new(90, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &task, frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Containers"));
        assert!(content.contains("app"));
        assert!(content.contains("nginx:latest"));
        assert!(content.contains("sidecar"));
        assert!(content.contains("envoy:v1.28"));
    }

    #[test]
    fn render_returns_keybinds_when_rendered() {
        let task = create_test_task();
        let backend = TestBackend::new(90, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &task, frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Esc:back"));
        assert!(content.contains("?:help"));
    }

    #[test]
    fn render_returns_snapshot_when_rendered() {
        let task = create_test_task();
        let backend = TestBackend::new(90, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &task, frame.area()))
            .unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }
}
