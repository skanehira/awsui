use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::tui::theme;

/// Profile選択用リストセレクターWidget
pub struct ListSelector<'a> {
    title: &'a str,
    items: &'a [String],
    selected: usize,
}

impl<'a> ListSelector<'a> {
    pub fn new(title: &'a str, items: &'a [String], selected: usize) -> Self {
        Self {
            title,
            items,
            selected,
        }
    }
}

impl Widget for ListSelector<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // タイトル
        let title_line = Line::from(self.title).alignment(Alignment::Center);
        let title_paragraph = Paragraph::new(title_line)
            .block(Block::default().borders(Borders::NONE))
            .style(theme::active());

        // タイトル用の領域を確保（上部3行分）
        let chunks = Layout::vertical([
            Constraint::Length(3), // タイトル
            Constraint::Min(0),    // リスト
        ])
        .split(area);

        title_paragraph.render(chunks[0], buf);

        // リストアイテム
        let list_area = chunks[1];
        for (i, item) in self.items.iter().enumerate() {
            if i as u16 >= list_area.height {
                break;
            }
            let marker = if i == self.selected { "▶ " } else { "  " };
            let style = if i == self.selected {
                theme::selected()
            } else {
                ratatui::style::Style::default()
            };
            let line = Line::from(vec![Span::styled(format!("{}{}", marker, item), style)]);
            buf.set_line(
                list_area.x + 2,
                list_area.y + i as u16,
                &line,
                list_area.width.saturating_sub(2),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_list_selector(
        title: &str,
        items: &[String],
        selected: usize,
        width: u16,
        height: u16,
    ) -> Buffer {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        let widget = ListSelector::new(title, items, selected);
        widget.render(area, &mut buf);
        buf
    }

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
    fn list_selector_render_returns_title_when_items_provided() {
        let items = vec!["dev".to_string(), "staging".to_string()];
        let buf = render_list_selector("Select AWS SSO Profile", &items, 0, 40, 10);
        let content = buffer_to_string(&buf, 40, 10);
        assert!(content.contains("Select AWS SSO Profile"));
    }

    #[test]
    fn list_selector_render_returns_selected_marker_when_first_item_selected() {
        let items = vec!["dev".to_string(), "staging".to_string()];
        let buf = render_list_selector("Title", &items, 0, 40, 10);
        let content = buffer_to_string(&buf, 40, 10);
        assert!(content.contains("▶ dev"));
    }

    #[test]
    fn list_selector_render_returns_unselected_items_without_marker() {
        let items = vec!["dev".to_string(), "staging".to_string(), "prod".to_string()];
        let buf = render_list_selector("Title", &items, 0, 40, 10);
        let content = buffer_to_string(&buf, 40, 10);
        assert!(content.contains("▶ dev"));
        assert!(content.contains("  staging"));
        assert!(content.contains("  prod"));
    }

    #[test]
    fn list_selector_render_returns_second_item_selected_when_index_one() {
        let items = vec!["dev".to_string(), "staging".to_string()];
        let buf = render_list_selector("Title", &items, 1, 40, 10);
        let content = buffer_to_string(&buf, 40, 10);
        assert!(content.contains("  dev"));
        assert!(content.contains("▶ staging"));
    }
}
