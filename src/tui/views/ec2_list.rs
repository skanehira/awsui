use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Row};

use crate::app::App;
use crate::aws::model::{Instance, InstanceState};
use crate::tui::components::loading::Loading;
use crate::tui::components::status_bar::render_footer;
use crate::tui::components::table::{SelectableTable, SelectableTableWidget};
use crate::tui::theme;

/// EC2インスタンス一覧画面を描画する
pub fn render(frame: &mut Frame, app: &App, spinner_tick: usize) {
    let area = frame.area();

    // フッターは外枠の外に配置
    let outer_chunks = Layout::vertical([
        Constraint::Min(1),    // 外枠（テーブル）
        Constraint::Length(1), // フッター
    ])
    .split(area);

    // 右タイトル（profile │ region │ count）
    let right_title = build_right_title(app);

    // 外枠Block
    let mut outer_block = Block::default()
        .title(" EC2 Instances ")
        .borders(Borders::ALL);
    if let Some(title) = right_title {
        outer_block = outer_block.title_top(Line::from(title).alignment(Alignment::Right));
    }
    let inner = outer_block.inner(outer_chunks[0]);
    frame.render_widget(outer_block, outer_chunks[0]);

    // メインコンテンツ
    if app.loading {
        let loading = Loading::new("Loading instances...", spinner_tick);
        frame.render_widget(loading, inner);
    } else {
        render_table(frame, app, inner);
    }

    // フッター
    render_footer(
        frame,
        outer_chunks[1],
        &app.mode,
        app.filter_input.value(),
        "j/k:move Enter:detail S:start/stop R:reboot /:filter ?:help",
    );
}

/// 右タイトル文字列を構築（profile │ region）
fn build_right_title(app: &App) -> Option<String> {
    let mut parts = Vec::new();

    if let Some(ref profile) = app.profile {
        parts.push(profile.clone());
    }
    if let Some(ref region) = app.region {
        parts.push(region.clone());
    }

    if parts.is_empty() {
        None
    } else {
        Some(format!(" {} ", parts.join(" │ ")))
    }
}

/// テーブルを描画
fn render_table(frame: &mut Frame, app: &App, area: Rect) {
    let headers =
        Row::new(vec!["Instance ID", "Name", "State", "Type", "AZ"]).style(theme::header());

    let rows: Vec<Row> = app
        .filtered_instances
        .iter()
        .map(|instance| instance_to_row(instance))
        .collect();

    let widths = vec![
        Constraint::Length(20),
        Constraint::Length(15),
        Constraint::Length(14),
        Constraint::Length(12),
        Constraint::Min(10),
    ];

    let table = SelectableTable::new(headers, rows, widths);
    let widget = SelectableTableWidget::new(table, app.selected_index);
    frame.render_widget(widget, area);
}

/// インスタンスをテーブル行に変換
fn instance_to_row(instance: &Instance) -> Row<'_> {
    let state_style = state_style(&instance.state);
    let state_text = format!("{} {}", instance.state.icon(), instance.state.as_str());

    Row::new(vec![
        Line::from(instance.instance_id.as_str()),
        Line::from(instance.name.as_str()),
        Line::from(Span::styled(state_text, state_style)),
        Line::from(instance.instance_type.as_str()),
        Line::from(instance.availability_zone.as_str()),
    ])
}

/// InstanceStateに対応するスタイルを返す
fn state_style(state: &InstanceState) -> ratatui::style::Style {
    match state {
        InstanceState::Running => theme::state_running(),
        InstanceState::Stopped => theme::state_stopped(),
        InstanceState::Pending | InstanceState::Stopping | InstanceState::ShuttingDown => {
            theme::state_transitioning()
        }
        InstanceState::Terminated => theme::state_terminated(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Mode, View};
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

    fn create_test_instance(id: &str, name: &str, state: InstanceState) -> Instance {
        Instance {
            instance_id: id.to_string(),
            name: name.to_string(),
            state,
            instance_type: "t3.micro".to_string(),
            availability_zone: "ap-northeast-1a".to_string(),
            private_ip: Some("10.0.1.1".to_string()),
            public_ip: None,
            vpc_id: Some("vpc-123".to_string()),
            subnet_id: Some("subnet-456".to_string()),
            ami_id: "ami-test".to_string(),
            key_name: None,
            platform: None,
            launch_time: None,
            security_groups: Vec::new(),
            volumes: Vec::new(),
            tags: HashMap::new(),
        }
    }

    fn app_with_instances(instances: Vec<Instance>) -> App {
        let mut app = App::new(vec!["dev".to_string()]);
        app.view = View::Ec2List;
        app.profile = Some("dev-account".to_string());
        app.region = Some("ap-northeast-1".to_string());
        app.instances = instances.clone();
        app.filtered_instances = instances;
        app
    }

    #[test]
    fn render_returns_header_when_ec2_list() {
        let app = app_with_instances(vec![]);
        let backend = TestBackend::new(70, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &app, 0)).unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("EC2 Instances"));
    }

    #[test]
    fn render_returns_table_headers_when_instances_exist() {
        let instances = vec![create_test_instance("i-001", "web", InstanceState::Running)];
        let app = app_with_instances(instances);
        let backend = TestBackend::new(70, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &app, 0)).unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Instance ID"));
        assert!(content.contains("Name"));
        assert!(content.contains("State"));
    }

    #[test]
    fn render_returns_instance_data_when_instances_provided() {
        let instances = vec![
            create_test_instance("i-001", "web-01", InstanceState::Running),
            create_test_instance("i-002", "api-01", InstanceState::Stopped),
        ];
        let app = app_with_instances(instances);
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &app, 0)).unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("i-001"));
        assert!(content.contains("web-01"));
        assert!(content.contains("running"));
        assert!(content.contains("i-002"));
        assert!(content.contains("api-01"));
        assert!(content.contains("stopped"));
    }

    #[test]
    fn render_returns_right_title_when_profile_and_region_set() {
        let instances = vec![create_test_instance("i-001", "web", InstanceState::Running)];
        let app = app_with_instances(instances);
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &app, 0)).unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("dev-account"));
        assert!(content.contains("ap-northeast-1"));
        assert!(content.contains("j/k:move"));
    }

    #[test]
    fn render_returns_loading_when_loading_state() {
        let mut app = app_with_instances(vec![]);
        app.loading = true;
        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &app, 0)).unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Loading instances..."));
    }

    #[test]
    fn render_returns_filter_input_when_filter_mode() {
        use tui_input::Input;
        let instances = vec![create_test_instance("i-001", "web", InstanceState::Running)];
        let mut app = app_with_instances(instances);
        app.mode = Mode::Filter;
        app.filter_input = Input::from("web");
        let backend = TestBackend::new(70, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &app, 0)).unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("/web"));
    }

    #[test]
    fn render_returns_snapshot_when_rendered() {
        let instances = vec![
            create_test_instance("i-0abc1234", "web-01", InstanceState::Running),
            create_test_instance("i-0def5678", "api-01", InstanceState::Stopped),
            create_test_instance("i-0ghi9012", "batch-01", InstanceState::Pending),
        ];
        let app = app_with_instances(instances);
        let backend = TestBackend::new(80, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &app, 0)).unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }

    #[test]
    fn state_style_returns_green_when_running() {
        let style = state_style(&InstanceState::Running);
        assert_eq!(style, theme::state_running());
    }

    #[test]
    fn state_style_returns_red_when_stopped() {
        let style = state_style(&InstanceState::Stopped);
        assert_eq!(style, theme::state_stopped());
    }

    #[test]
    fn state_style_returns_yellow_when_pending() {
        let style = state_style(&InstanceState::Pending);
        assert_eq!(style, theme::state_transitioning());
    }

    #[test]
    fn state_style_returns_dark_gray_when_terminated() {
        let style = state_style(&InstanceState::Terminated);
        assert_eq!(style, theme::state_terminated());
    }
}
