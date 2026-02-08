use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::app::App;
use crate::service::ServiceKind;
use crate::tui::components::status_bar::render_footer;
use crate::tui::theme;

/// ダッシュボード画面を描画する
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let outer_chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(area);

    // 外枠
    let title = build_title(app.profile.as_deref(), app.region.as_deref());
    let outer_block = Block::default().title(title).borders(Borders::ALL);
    let inner = outer_block.inner(outer_chunks[0]);
    frame.render_widget(outer_block, outer_chunks[0]);

    // コンテンツ
    let content = DashboardContent::new(
        &app.dashboard.recent_services,
        &app.dashboard.filtered_services,
        app.dashboard.selected_index,
    );
    frame.render_widget(content, inner);

    // フッター
    render_footer(
        frame,
        outer_chunks[1],
        &app.dashboard.mode,
        app.dashboard.filter_input.value(),
        "j/k:select  /:filter  Enter:open  q:quit",
    );
}

fn build_title(profile: Option<&str>, region: Option<&str>) -> String {
    let mut parts = vec![" awsui Dashboard ".to_string()];
    if let Some(p) = profile {
        parts.push(format!("| {} ", p));
    }
    if let Some(r) = region {
        parts.push(format!("| {} ", r));
    }
    parts.join("")
}

/// ダッシュボードのコンテンツウィジェット
struct DashboardContent<'a> {
    recent: &'a [ServiceKind],
    all_services: &'a [ServiceKind],
    selected_index: usize,
}

impl<'a> DashboardContent<'a> {
    fn new(
        recent: &'a [ServiceKind],
        all_services: &'a [ServiceKind],
        selected_index: usize,
    ) -> Self {
        Self {
            recent,
            all_services,
            selected_index,
        }
    }
}

impl Widget for DashboardContent<'_> {
    fn render(self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        let mut lines = Vec::new();

        // Recently Used セクション
        if !self.recent.is_empty() {
            lines.push(Line::from(Span::styled(
                "Recently Used",
                Style::default().add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(Span::raw("─────────────")));

            for (i, service) in self.recent.iter().enumerate() {
                let style = if i == self.selected_index {
                    theme::active()
                } else {
                    Style::default()
                };
                let marker = if i == self.selected_index {
                    "▶ "
                } else {
                    "  "
                };
                lines.push(Line::from(Span::styled(
                    format!("{}{}", marker, service.full_name()),
                    style,
                )));
            }

            lines.push(Line::from(""));
        }

        // All Services セクション
        lines.push(Line::from(Span::styled(
            "All Services",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::raw("────────────")));

        let recent_len = self.recent.len();
        for (i, service) in self.all_services.iter().enumerate() {
            let global_index = recent_len + i;
            let style = if global_index == self.selected_index {
                theme::active()
            } else {
                Style::default()
            };
            let marker = if global_index == self.selected_index {
                "▶ "
            } else {
                "  "
            };
            lines.push(Line::from(Span::styled(
                format!("{}{}", marker, service.full_name()),
                style,
            )));
        }

        Paragraph::new(lines).render(area, buf);
    }
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

    #[test]
    fn render_returns_snapshot_when_no_recent_services() {
        let mut app = App::new("dev".to_string(), Some("us-east-1".to_string()));
        app.dashboard.recent_services.clear();
        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &app)).unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }

    #[test]
    fn render_returns_snapshot_when_recent_services_exist() {
        let mut app = App::new("production".to_string(), Some("ap-northeast-1".to_string()));
        app.dashboard.recent_services = vec![ServiceKind::Ec2, ServiceKind::S3];

        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &app)).unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }

    #[test]
    fn selected_service_returns_recent_service_when_index_in_recent() {
        let mut app = App::new("dev".to_string(), None);
        app.dashboard.recent_services = vec![ServiceKind::S3, ServiceKind::Ecs];
        app.dashboard.selected_index = 1;
        assert_eq!(app.dashboard.selected_service(), Some(ServiceKind::Ecs));
    }

    #[test]
    fn selected_service_returns_all_service_when_index_past_recent() {
        let mut app = App::new("dev".to_string(), None);
        app.dashboard.recent_services = vec![ServiceKind::S3];
        app.dashboard.selected_index = 1; // 1 = first item in "All Services"
        assert_eq!(app.dashboard.selected_service(), Some(ServiceKind::Ec2));
    }

    #[test]
    fn item_count_returns_sum_when_recent_and_all() {
        let mut app = App::new("dev".to_string(), None);
        app.dashboard.recent_services = vec![ServiceKind::Ec2, ServiceKind::S3];
        // All Services = 6 (ServiceKind::ALL)
        assert_eq!(app.dashboard.item_count(), 8); // 2 + 6
    }
}
