use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::tab::Tab;
use crate::tui::theme;

/// タブバーウィジェット
pub struct TabBar<'a> {
    tabs: &'a [Tab],
    active_index: usize,
}

impl<'a> TabBar<'a> {
    pub fn new(tabs: &'a [Tab], active_index: usize) -> Self {
        Self { tabs, active_index }
    }
}

impl Widget for TabBar<'_> {
    fn render(self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        let mut spans = Vec::new();

        for (i, tab) in self.tabs.iter().enumerate() {
            let title = tab.title();
            let style = if i == self.active_index {
                theme::active()
            } else {
                theme::inactive()
            };

            if i > 0 {
                spans.push(Span::raw(" "));
            }
            spans.push(Span::styled(format!(" {} ", title), style));
        }

        let line = Line::from(spans);
        Paragraph::new(line).render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::ServiceKind;
    use crate::tab::TabId;
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
    fn render_returns_active_tab_highlighted_when_single_tab() {
        let tabs = vec![Tab::new(TabId(0), ServiceKind::Ec2)];
        let backend = TestBackend::new(30, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let widget = TabBar::new(&tabs, 0);
                frame.render_widget(widget, frame.area());
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("EC2"));
    }

    #[test]
    fn render_returns_multiple_tabs_when_multiple_tabs() {
        let tabs = vec![
            Tab::new(TabId(0), ServiceKind::Ec2),
            Tab::new(TabId(1), ServiceKind::S3),
            Tab::new(TabId(2), ServiceKind::Ecs),
        ];
        let backend = TestBackend::new(30, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let widget = TabBar::new(&tabs, 1);
                frame.render_widget(widget, frame.area());
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("EC2"));
        assert!(content.contains("S3"));
        assert!(content.contains("ECS"));
    }

    #[test]
    fn render_returns_snapshot_when_three_tabs() {
        let tabs = vec![
            Tab::new(TabId(0), ServiceKind::Ec2),
            Tab::new(TabId(1), ServiceKind::S3),
            Tab::new(TabId(2), ServiceKind::Ecs),
        ];
        let backend = TestBackend::new(30, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let widget = TabBar::new(&tabs, 1);
                frame.render_widget(widget, frame.area());
            })
            .unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }
}
