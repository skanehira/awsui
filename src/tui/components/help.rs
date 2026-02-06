use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::tui::components::dialog::centered_rect;
use crate::tui::theme;

/// ヘルプポップアップWidget
pub struct HelpPopup;

impl Default for HelpPopup {
    fn default() -> Self {
        Self
    }
}

impl HelpPopup {
    pub fn new() -> Self {
        Self
    }
}

impl Widget for HelpPopup {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup = centered_rect(70, 23, area);
        Clear.render(popup, buf);

        let block = Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .style(theme::active());

        let inner = block.inner(popup);
        block.render(popup, buf);

        let sections: Vec<Line<'_>> = vec![
            Line::from(""),
            Line::from(Span::styled("  Navigation", theme::header())),
            Line::from("    j/k        Move down/up"),
            Line::from("    g/G        Go to first/last"),
            Line::from("    Ctrl+d/u   Half page down/up"),
            Line::from("    Enter      Open detail"),
            Line::from("    Esc        Go back"),
            Line::from(""),
            Line::from(Span::styled("  Actions", theme::header())),
            Line::from("    S          Start/Stop instance"),
            Line::from("    R          Reboot instance"),
            Line::from("    r          Refresh list"),
            Line::from("    y          Copy instance ID"),
            Line::from("    /          Filter instances"),
            Line::from(""),
            Line::from(Span::styled("  General", theme::header())),
            Line::from("    ?          Show this help"),
            Line::from("    q          Quit"),
            Line::from(""),
            Line::from("  Press Esc to close"),
        ];

        let help_text = Paragraph::new(sections);
        help_text.render(inner, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer_to_string(buf: &Buffer, width: u16, height: u16) -> String {
        let mut result = String::new();
        for y in 0..height {
            for x in 0..width {
                result.push_str(buf[(x, y)].symbol());
            }
            result.push('\n');
        }
        result
    }

    #[test]
    fn help_popup_render_returns_title_when_rendered() {
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        HelpPopup::new().render(area, &mut buf);

        let content = buffer_to_string(&buf, 80, 30);
        assert!(content.contains("Help"));
    }

    #[test]
    fn help_popup_render_returns_navigation_section_when_rendered() {
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        HelpPopup::new().render(area, &mut buf);

        let content = buffer_to_string(&buf, 80, 30);
        assert!(content.contains("Navigation"));
        assert!(content.contains("j/k"));
        assert!(content.contains("Move down/up"));
    }

    #[test]
    fn help_popup_render_returns_actions_section_when_rendered() {
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        HelpPopup::new().render(area, &mut buf);

        let content = buffer_to_string(&buf, 80, 30);
        assert!(content.contains("Actions"));
        assert!(content.contains("Start/Stop"));
        assert!(content.contains("Reboot"));
    }

    #[test]
    fn help_popup_render_returns_general_section_when_rendered() {
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        HelpPopup::new().render(area, &mut buf);

        let content = buffer_to_string(&buf, 80, 30);
        assert!(content.contains("General"));
        assert!(content.contains("Quit"));
        assert!(content.contains("Press Esc to close"));
    }
}
