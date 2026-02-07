use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::app::Mode;
use crate::tui::theme;

/// フッターを描画する（Filterモード時は入力表示、それ以外はステータスバー）
pub fn render_footer(
    frame: &mut Frame,
    area: Rect,
    mode: &Mode,
    filter_value: &str,
    keybinds: &str,
) {
    match mode {
        Mode::Filter => {
            let filter_line = Paragraph::new(Line::from(vec![
                Span::styled("/", theme::active()),
                Span::raw(filter_value),
            ]));
            frame.render_widget(filter_line, area);
        }
        _ => {
            let status = StatusBar::new(keybinds);
            frame.render_widget(status, area);
        }
    }
}

/// ステータスバーWidget
/// 1行: キーバインドヘルプ
pub struct StatusBar<'a> {
    keybinds: &'a str,
}

impl<'a> StatusBar<'a> {
    pub fn new(keybinds: &'a str) -> Self {
        Self { keybinds }
    }
}

impl Widget for StatusBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 {
            return;
        }

        let keybind_line = Line::from(vec![Span::styled(
            format!(" {}", self.keybinds),
            theme::status_bar(),
        )]);
        buf.set_line(area.x, area.y, &keybind_line, area.width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    fn render_status_bar(keybinds: &str, width: u16) -> Buffer {
        let area = Rect::new(0, 0, width, 1);
        let mut buf = Buffer::empty(area);
        let widget = StatusBar::new(keybinds);
        widget.render(area, &mut buf);
        buf
    }

    #[test]
    fn render_returns_keybinds_when_provided() {
        let buf = render_status_bar("j/k:move Enter:detail", 50);
        let content: String = (0..50).map(|x| buf[(x, 0)].symbol().to_string()).collect();
        assert!(content.contains("j/k:move Enter:detail"));
    }

    #[test]
    fn render_returns_empty_when_height_zero() {
        let area = Rect::new(0, 0, 50, 0);
        let mut buf = Buffer::empty(area);
        let widget = StatusBar::new("test");
        widget.render(area, &mut buf);
        // no panic = success
    }
}
