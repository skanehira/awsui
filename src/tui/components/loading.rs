use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Widget};

use crate::tui::theme;

/// スピナーフレーム
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// ローディングスピナーWidget
pub struct Loading<'a> {
    message: &'a str,
    tick: usize,
}

impl<'a> Loading<'a> {
    pub fn new(message: &'a str, tick: usize) -> Self {
        Self { message, tick }
    }
}

impl Widget for Loading<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let frame = SPINNER_FRAMES[(self.tick / 6) % SPINNER_FRAMES.len()];
        let text = format!("{} {}", frame, self.message);
        let paragraph = Paragraph::new(Line::from(text))
            .alignment(Alignment::Center)
            .style(theme::info());
        // 垂直方向で中央に配置
        let y_offset = area.height / 2;
        let centered = Rect::new(area.x, area.y + y_offset, area.width, 1);
        paragraph.render(centered, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_loading(message: &str, tick: usize, width: u16, height: u16) -> Buffer {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        let widget = Loading::new(message, tick);
        widget.render(area, &mut buf);
        buf
    }

    fn buffer_row_to_string(buf: &Buffer, y: u16, width: u16) -> String {
        (0..width)
            .map(|x| buf[(x, y)].symbol().to_string())
            .collect()
    }

    #[test]
    fn loading_render_returns_spinner_frame_when_tick_zero() {
        let buf = render_loading("Loading...", 0, 40, 10);
        let mid = buffer_row_to_string(&buf, 5, 40);
        assert!(mid.contains("⠋"));
        assert!(mid.contains("Loading..."));
    }

    #[test]
    fn loading_render_returns_different_frame_when_tick_changes() {
        // tick 18 / 6 = 3 → SPINNER_FRAMES[3] = "⠸"
        let buf = render_loading("Loading...", 18, 40, 10);
        let mid = buffer_row_to_string(&buf, 5, 40);
        assert!(mid.contains("⠸"));
    }

    #[test]
    fn loading_render_returns_wrapped_frame_when_tick_exceeds_frames() {
        // tick 60 / 6 = 10, 10 % 10 = 0 → "⠋"
        let buf = render_loading("test", 60, 40, 10);
        let mid = buffer_row_to_string(&buf, 5, 40);
        assert!(mid.contains("⠋"));
    }
}
