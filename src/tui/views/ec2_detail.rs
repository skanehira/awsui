use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Wrap};

use crate::app::{App, DetailTab, Ec2DetailField};
use crate::aws::model::Instance;
use crate::tui::components::status_bar::StatusBar;
use crate::tui::components::table::{SelectableTable, SelectableTableWidget};
use crate::tui::theme;

/// EC2インスタンス詳細画面を描画する
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let outer_chunks = Layout::vertical([
        Constraint::Min(1),    // 外枠（タブバー + コンテンツ）
        Constraint::Length(1), // ステータスバー
    ])
    .split(area);

    // 外枠Block（パンくずリスト対応）
    let title = if let Some(breadcrumb) = app.breadcrumb() {
        format!(" {} ", breadcrumb)
    } else if let Some(instance) = app.selected_instance() {
        format!(" {} ({}) ", instance.name, instance.instance_id)
    } else {
        " EC2 Detail ".to_string()
    };
    let right_title = build_right_title(app);

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

    // アクティブタブからdetail_tab/detail_tag_indexを取得
    let (detail_tab, detail_tag_index) = app
        .active_tab()
        .map(|t| (t.detail_tab.clone(), t.detail_tag_index))
        .unwrap_or((DetailTab::Overview, 0));

    // タブバー
    render_tab_bar(frame, &detail_tab, inner_chunks[0]);

    // コンテンツ
    if let Some(instance) = app.selected_instance() {
        match detail_tab {
            DetailTab::Overview => {
                render_overview(frame, instance, detail_tag_index, inner_chunks[1]);
            }
            DetailTab::Tags => render_tags(frame, instance, detail_tag_index, inner_chunks[1]),
        }
    }

    // ステータスバー
    let keybinds = match detail_tab {
        DetailTab::Overview => {
            "Tab:switch-tab j/k:select Enter:follow-link S:start/stop R:reboot y:copy-id Esc:back"
        }
        DetailTab::Tags => "Tab:switch-tab y:copy-value Esc:back",
    };
    let status = StatusBar::new(keybinds);
    frame.render_widget(status, outer_chunks[1]);
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

/// タブバーを描画
fn render_tab_bar(frame: &mut Frame, current_tab: &DetailTab, area: Rect) {
    let overview_style = if *current_tab == DetailTab::Overview {
        theme::active()
    } else {
        theme::inactive()
    };
    let tags_style = if *current_tab == DetailTab::Tags {
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
fn render_overview(frame: &mut Frame, instance: &Instance, selected_field: usize, area: Rect) {
    let chunks = Layout::vertical([
        Constraint::Min(1),    // Instance + Network
        Constraint::Length(5), // Storage
    ])
    .split(area);

    // Instance + Network（横分割）
    let h_chunks = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);

    // Instance情報
    let instance_block = Block::default().title(" Instance ").borders(Borders::ALL);
    let state_text = format!("{} {}", instance.state.icon(), instance.state.as_str());
    let instance_lines = vec![
        detail_line("ID", &instance.instance_id),
        detail_line("Name", &instance.name),
        detail_line("Type", &instance.instance_type),
        detail_line("State", &state_text),
        detail_line("AMI", &instance.ami_id),
        detail_line("Key", instance.key_name.as_deref().unwrap_or("-")),
        detail_line("Platform", instance.platform.as_deref().unwrap_or("-")),
        detail_line("Launch", instance.launch_time.as_deref().unwrap_or("-")),
    ];
    let instance_para = Paragraph::new(instance_lines)
        .block(instance_block)
        .wrap(Wrap { trim: false });
    frame.render_widget(instance_para, h_chunks[0]);

    // Network情報（リンク可能フィールド付き）
    let network_block = Block::default().title(" Network ").borders(Borders::ALL);
    let sg_text = if instance.security_groups.is_empty() {
        "-".to_string()
    } else {
        instance.security_groups.join(", ")
    };

    // リンク可能フィールドの生成
    let link_fields = Ec2DetailField::ALL;
    let network_lines = vec![
        linkable_detail_line(
            "VPC",
            instance.vpc_id.as_deref().unwrap_or("-"),
            instance.vpc_id.is_some(),
            selected_field == 0 && link_fields.contains(&Ec2DetailField::VpcId),
        ),
        linkable_detail_line(
            "Subnet",
            instance.subnet_id.as_deref().unwrap_or("-"),
            instance.subnet_id.is_some(),
            selected_field == 1 && link_fields.contains(&Ec2DetailField::SubnetId),
        ),
        detail_line("Private IP", instance.private_ip.as_deref().unwrap_or("-")),
        detail_line("Public IP", instance.public_ip.as_deref().unwrap_or("-")),
        detail_line("AZ", &instance.availability_zone),
        detail_line("SG", &sg_text),
    ];
    let network_para = Paragraph::new(network_lines)
        .block(network_block)
        .wrap(Wrap { trim: false });
    frame.render_widget(network_para, h_chunks[1]);

    // Storage
    let storage_block = Block::default().title(" Storage ").borders(Borders::ALL);
    let storage_lines: Vec<Line> = instance
        .volumes
        .iter()
        .map(|v| {
            Line::from(format!(
                "  {}  {}  {}GB  {}  {}",
                v.volume_id, v.volume_type, v.size_gb, v.device_name, v.state
            ))
        })
        .collect();
    let storage_para = Paragraph::new(if storage_lines.is_empty() {
        vec![Line::from("  No volumes")]
    } else {
        storage_lines
    })
    .block(storage_block);
    frame.render_widget(storage_para, chunks[1]);
}

/// Tagsタブを描画
fn render_tags(frame: &mut Frame, instance: &Instance, selected_index: usize, area: Rect) {
    let headers = Row::new(vec!["Key", "Value"]).style(theme::header());

    let mut sorted_tags: Vec<(&String, &String)> = instance.tags.iter().collect();
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
        Span::styled(format!("  {:<12}", label), theme::header()),
        Span::raw(value),
    ])
}

/// リンク可能フィールドの1行を生成
fn linkable_detail_line<'a>(
    label: &'a str,
    value: &'a str,
    has_link: bool,
    selected: bool,
) -> Line<'a> {
    let marker = if has_link { " →" } else { "" };
    let style = if selected {
        Style::default()
            .fg(ratatui::style::Color::Black)
            .bg(ratatui::style::Color::Cyan)
    } else {
        Style::default()
    };

    Line::from(vec![
        Span::styled(format!("  {:<12}", label), theme::header().patch(style)),
        Span::styled(format!("{}{}", value, marker), style),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aws::model::{InstanceState, Volume};
    use crate::service::ServiceKind;
    use crate::tab::{ServiceData, TabView};
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

    fn create_detail_instance() -> Instance {
        let mut tags = HashMap::new();
        tags.insert("Name".to_string(), "web-01".to_string());
        tags.insert("env".to_string(), "production".to_string());
        tags.insert("team".to_string(), "backend".to_string());

        Instance {
            instance_id: "i-0abc1234".to_string(),
            name: "web-01".to_string(),
            state: InstanceState::Running,
            instance_type: "t3.micro".to_string(),
            availability_zone: "ap-northeast-1a".to_string(),
            private_ip: Some("10.0.1.42".to_string()),
            public_ip: Some("54.210.3.15".to_string()),
            vpc_id: Some("vpc-abc123".to_string()),
            subnet_id: Some("subnet-def456".to_string()),
            ami_id: "ami-0abcdef123".to_string(),
            key_name: Some("my-keypair".to_string()),
            platform: Some("Linux/UNIX".to_string()),
            launch_time: Some("2025-01-10 09:30".to_string()),
            security_groups: vec!["sg-789012".to_string()],
            volumes: vec![
                Volume {
                    volume_id: "vol-abc123".to_string(),
                    volume_type: "gp3".to_string(),
                    size_gb: 20,
                    device_name: "/dev/xvda".to_string(),
                    state: "attached".to_string(),
                },
                Volume {
                    volume_id: "vol-def456".to_string(),
                    volume_type: "gp3".to_string(),
                    size_gb: 100,
                    device_name: "/dev/xvdf".to_string(),
                    state: "attached".to_string(),
                },
            ],
            tags,
        }
    }

    fn app_with_detail() -> App {
        let mut app = App::new("dev".to_string(), None);
        app.profile = Some("dev-account".to_string());
        app.region = Some("ap-northeast-1".to_string());
        let instance = create_detail_instance();
        app.create_tab(ServiceKind::Ec2);
        if let Some(tab) = app.active_tab_mut() {
            tab.tab_view = TabView::Detail;
            tab.loading = false;
            if let ServiceData::Ec2 { instances, .. } = &mut tab.data {
                instances.set_items(vec![instance]);
            }
            tab.selected_index = 0;
        }
        app
    }

    #[test]
    fn render_returns_tab_bar_when_overview_tab() {
        let app = app_with_detail();
        let backend = TestBackend::new(80, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &app, frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("[Overview]"));
        assert!(content.contains("[Tags]"));
    }

    #[test]
    fn render_returns_instance_info_when_overview_tab() {
        let app = app_with_detail();
        let backend = TestBackend::new(80, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &app, frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("i-0abc1234"));
        assert!(content.contains("web-01"));
        assert!(content.contains("t3.micro"));
        assert!(content.contains("running"));
    }

    #[test]
    fn render_returns_network_info_when_overview_tab() {
        let app = app_with_detail();
        let backend = TestBackend::new(80, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &app, frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("vpc-abc123"));
        assert!(content.contains("10.0.1.42"));
        assert!(content.contains("54.210.3.15"));
    }

    #[test]
    fn render_returns_storage_info_when_overview_tab() {
        let app = app_with_detail();
        let backend = TestBackend::new(80, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &app, frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("vol-abc123"));
        assert!(content.contains("gp3"));
        assert!(content.contains("20GB"));
    }

    #[test]
    fn render_returns_tags_table_when_tags_tab() {
        let mut app = app_with_detail();
        if let Some(tab) = app.active_tab_mut() {
            tab.detail_tab = DetailTab::Tags;
        }
        let backend = TestBackend::new(80, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &app, frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Key"));
        assert!(content.contains("Value"));
        assert!(content.contains("Name"));
        assert!(content.contains("web-01"));
        assert!(content.contains("env"));
        assert!(content.contains("production"));
    }

    #[test]
    fn render_returns_right_title_when_profile_and_region_set() {
        let app = app_with_detail();
        let backend = TestBackend::new(80, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &app, frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("dev-account"));
        assert!(content.contains("ap-northeast-1"));
        assert!(content.contains("Tab:switch-tab"));
    }

    #[test]
    fn render_returns_overview_snapshot_when_rendered() {
        let app = app_with_detail();
        let backend = TestBackend::new(80, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &app, frame.area()))
            .unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }

    #[test]
    fn render_returns_tags_snapshot_when_tags_tab() {
        let mut app = app_with_detail();
        if let Some(tab) = app.active_tab_mut() {
            tab.detail_tab = DetailTab::Tags;
        }
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &app, frame.area()))
            .unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }
}
