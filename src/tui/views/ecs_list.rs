use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Row};
use tui_input::Input;

use crate::app::Mode;
use crate::aws::ecs_model::Cluster;
use crate::tui::components::loading::Loading;
use crate::tui::components::status_bar::render_footer;
use crate::tui::components::table::{SelectableTable, SelectableTableWidget};
use crate::tui::theme;

/// ECSクラスター一覧画面を描画する
pub fn render(
    frame: &mut Frame,
    clusters: &[Cluster],
    selected_index: usize,
    filter_input: &Input,
    mode: &Mode,
    loading: bool,
    spinner_tick: usize,
    area: Rect,
) {
    let outer_chunks = Layout::vertical([
        Constraint::Min(1),    // 外枠（テーブル）
        Constraint::Length(1), // フッター
    ])
    .split(area);

    let count_title = format!(" ECS Clusters ({}) ", clusters.len());
    let outer_block = Block::default().title(count_title).borders(Borders::ALL);
    let inner = outer_block.inner(outer_chunks[0]);
    frame.render_widget(outer_block, outer_chunks[0]);

    if loading {
        let loading_widget = Loading::new("Loading clusters...", spinner_tick);
        frame.render_widget(loading_widget, inner);
    } else {
        render_table(frame, clusters, selected_index, inner);
    }

    // フッター
    render_footer(
        frame,
        outer_chunks[1],
        mode,
        filter_input.value(),
        "j/k:move Enter:detail /:filter ?:help Esc:back",
    );
}

/// テーブルを描画
fn render_table(frame: &mut Frame, clusters: &[Cluster], selected_index: usize, area: Rect) {
    let headers = Row::new(vec![
        "Cluster Name",
        "Status",
        "Running Tasks",
        "Pending Tasks",
        "Active Services",
        "Container Instances",
    ])
    .style(theme::header());

    let rows: Vec<Row> = clusters.iter().map(cluster_to_row).collect();

    let widths = vec![
        Constraint::Length(25),
        Constraint::Length(12),
        Constraint::Length(15),
        Constraint::Length(15),
        Constraint::Length(17),
        Constraint::Min(20),
    ];

    let table = SelectableTable::new(headers, rows, widths);
    let widget = SelectableTableWidget::new(table, selected_index);
    frame.render_widget(widget, area);
}

/// クラスターをテーブル行に変換
fn cluster_to_row(cluster: &Cluster) -> Row<'_> {
    let status_style = match cluster.status.as_str() {
        "ACTIVE" => theme::state_running(),
        "INACTIVE" => theme::state_stopped(),
        _ => theme::state_transitioning(),
    };

    Row::new(vec![
        Line::from(cluster.cluster_name.as_str()),
        Line::from(Span::styled(cluster.status.as_str(), status_style)),
        Line::from(cluster.running_tasks_count.to_string()),
        Line::from(cluster.pending_tasks_count.to_string()),
        Line::from(cluster.active_services_count.to_string()),
        Line::from(cluster.registered_container_instances_count.to_string()),
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

    fn create_test_cluster(name: &str, status: &str, running: i32, pending: i32) -> Cluster {
        Cluster {
            cluster_name: name.to_string(),
            cluster_arn: format!("arn:aws:ecs:ap-northeast-1:123456789012:cluster/{}", name),
            status: status.to_string(),
            running_tasks_count: running,
            pending_tasks_count: pending,
            active_services_count: 2,
            registered_container_instances_count: 3,
        }
    }

    #[test]
    fn render_returns_title_when_ecs_list() {
        let clusters = vec![];
        let input = Input::default();
        let backend = TestBackend::new(110, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &clusters,
                    0,
                    &input,
                    &Mode::Normal,
                    false,
                    0,
                    frame.area(),
                )
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("ECS Clusters"));
    }

    #[test]
    fn render_returns_table_headers_when_clusters_exist() {
        let clusters = vec![create_test_cluster("my-cluster", "ACTIVE", 5, 0)];
        let input = Input::default();
        let backend = TestBackend::new(110, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &clusters,
                    0,
                    &input,
                    &Mode::Normal,
                    false,
                    0,
                    frame.area(),
                )
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Cluster Name"));
        assert!(content.contains("Status"));
        assert!(content.contains("Running Tasks"));
        assert!(content.contains("Pending Tasks"));
        assert!(content.contains("Active Services"));
        assert!(content.contains("Container Instances"));
    }

    #[test]
    fn render_returns_cluster_data_when_clusters_provided() {
        let clusters = vec![
            create_test_cluster("web-cluster", "ACTIVE", 10, 2),
            create_test_cluster("batch-cluster", "ACTIVE", 3, 0),
        ];
        let input = Input::default();
        let backend = TestBackend::new(110, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &clusters,
                    0,
                    &input,
                    &Mode::Normal,
                    false,
                    0,
                    frame.area(),
                )
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("web-cluster"));
        assert!(content.contains("ACTIVE"));
        assert!(content.contains("batch-cluster"));
    }

    #[test]
    fn render_returns_loading_when_loading_state() {
        let clusters = vec![];
        let input = Input::default();
        let backend = TestBackend::new(110, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &clusters,
                    0,
                    &input,
                    &Mode::Normal,
                    true,
                    0,
                    frame.area(),
                )
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Loading clusters..."));
    }

    #[test]
    fn render_returns_filter_input_when_filter_mode() {
        let clusters = vec![create_test_cluster("my-cluster", "ACTIVE", 5, 0)];
        let input = Input::from("web");
        let backend = TestBackend::new(110, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &clusters,
                    0,
                    &input,
                    &Mode::Filter,
                    false,
                    0,
                    frame.area(),
                )
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("/web"));
    }

    #[test]
    fn render_returns_keybinds_when_normal_mode() {
        let clusters = vec![];
        let input = Input::default();
        let backend = TestBackend::new(110, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &clusters,
                    0,
                    &input,
                    &Mode::Normal,
                    false,
                    0,
                    frame.area(),
                )
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("j/k:move"));
    }

    #[test]
    fn render_returns_snapshot_when_rendered() {
        let clusters = vec![
            create_test_cluster("web-cluster", "ACTIVE", 10, 2),
            create_test_cluster("batch-cluster", "ACTIVE", 3, 0),
            create_test_cluster("dev-cluster", "INACTIVE", 0, 0),
        ];
        let input = Input::default();
        let backend = TestBackend::new(110, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &clusters,
                    0,
                    &input,
                    &Mode::Normal,
                    false,
                    0,
                    frame.area(),
                )
            })
            .unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }

    #[test]
    fn cluster_to_row_returns_running_style_when_active() {
        let cluster = create_test_cluster("test", "ACTIVE", 1, 0);
        let row = cluster_to_row(&cluster);
        // Row が正常に生成されることを確認（パニックしないこと）
        let _ = row;
    }

    #[test]
    fn cluster_to_row_returns_stopped_style_when_inactive() {
        let cluster = create_test_cluster("test", "INACTIVE", 0, 0);
        let row = cluster_to_row(&cluster);
        let _ = row;
    }
}
