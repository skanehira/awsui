use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap};

use crate::app::{Message, MessageLevel};
use crate::tui::theme;

/// 確認ダイアログWidget
pub struct ConfirmDialog<'a> {
    message: &'a str,
}

impl<'a> ConfirmDialog<'a> {
    pub fn new(message: &'a str) -> Self {
        Self { message }
    }
}

impl Widget for ConfirmDialog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup = centered_rect(50, 7, area);
        Clear.render(popup, buf);

        let block = Block::default()
            .title(" Confirm ")
            .borders(Borders::ALL)
            .style(theme::active());

        let inner = block.inner(popup);
        block.render(popup, buf);

        let chunks = Layout::vertical([
            Constraint::Length(1), // 空行
            Constraint::Length(1), // メッセージ
            Constraint::Length(1), // 空行
            Constraint::Length(1), // ボタン
        ])
        .split(inner);

        let msg = Paragraph::new(Line::from(self.message)).alignment(Alignment::Center);
        msg.render(chunks[1], buf);

        let buttons = Line::from(vec![
            Span::styled("[ Yes (y) ]", theme::active()),
            Span::raw("    "),
            Span::raw("[ No (n) ]"),
        ])
        .alignment(Alignment::Center);
        let button_para = Paragraph::new(buttons);
        button_para.render(chunks[3], buf);
    }
}

/// メッセージダイアログWidget
pub struct MessageDialog<'a> {
    message: &'a Message,
}

impl<'a> MessageDialog<'a> {
    pub fn new(message: &'a Message) -> Self {
        Self { message }
    }

    fn title_style(level: &MessageLevel) -> ratatui::style::Style {
        match level {
            MessageLevel::Error => theme::error(),
            MessageLevel::Success => theme::success(),
            MessageLevel::Info => theme::info(),
        }
    }
}

impl Widget for MessageDialog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // メッセージの行数に応じて高さを調整
        let body_lines = self.message.body.lines().count().max(1);
        let height = (body_lines as u16 + 6).min(area.height); // title(1) + border(2) + spacing(2) + button(1)
        let popup = centered_rect(60, height, area);
        Clear.render(popup, buf);

        let title = format!(" {} ", self.message.title);
        let block = Block::default()
            .title(title)
            .title_style(Self::title_style(&self.message.level))
            .borders(Borders::ALL)
            .style(theme::active());

        let inner = block.inner(popup);
        block.render(popup, buf);

        let chunks = Layout::vertical([
            Constraint::Length(1), // 空行
            Constraint::Min(1),    // ボディ
            Constraint::Length(1), // 空行
            Constraint::Length(1), // ボタン
        ])
        .split(inner);

        let body = Paragraph::new(self.message.body.as_str())
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false });
        body.render(chunks[1], buf);

        let button = Paragraph::new(Line::from("[ OK (Enter) ]").alignment(Alignment::Center));
        button.render(chunks[3], buf);
    }
}

/// 画面中央に配置する矩形を計算
pub fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let width = (area.width * percent_x / 100).min(area.width);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height.min(area.height))
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
    fn centered_rect_returns_centered_position_when_area_provided() {
        let area = Rect::new(0, 0, 100, 30);
        let rect = centered_rect(50, 7, area);
        assert_eq!(rect.width, 50);
        assert_eq!(rect.height, 7);
        assert_eq!(rect.x, 25);
        assert_eq!(rect.y, 11);
    }

    #[test]
    fn centered_rect_returns_clamped_height_when_area_too_small() {
        let area = Rect::new(0, 0, 40, 5);
        let rect = centered_rect(50, 10, area);
        assert_eq!(rect.height, 5);
    }

    #[test]
    fn confirm_dialog_render_returns_title_when_rendered() {
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        let dialog = ConfirmDialog::new("Stop instance i-0abc1234?");
        dialog.render(area, &mut buf);

        let content = buffer_to_string(&buf, 60, 20);
        assert!(content.contains("Confirm"));
    }

    #[test]
    fn confirm_dialog_render_returns_message_when_rendered() {
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        let dialog = ConfirmDialog::new("Stop instance i-0abc1234?");
        dialog.render(area, &mut buf);

        let content = buffer_to_string(&buf, 60, 20);
        assert!(content.contains("Stop instance i-0abc1234?"));
    }

    #[test]
    fn confirm_dialog_render_returns_buttons_when_rendered() {
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        let dialog = ConfirmDialog::new("test?");
        dialog.render(area, &mut buf);

        let content = buffer_to_string(&buf, 60, 20);
        assert!(content.contains("Yes (y)"));
        assert!(content.contains("No (n)"));
    }

    #[test]
    fn message_dialog_render_returns_title_when_error() {
        let msg = Message {
            level: MessageLevel::Error,
            title: "Error".to_string(),
            body: "Something failed".to_string(),
        };
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        let dialog = MessageDialog::new(&msg);
        dialog.render(area, &mut buf);

        let content = buffer_to_string(&buf, 60, 20);
        assert!(content.contains("Error"));
        assert!(content.contains("Something failed"));
    }

    #[test]
    fn message_dialog_render_returns_ok_button_when_rendered() {
        let msg = Message {
            level: MessageLevel::Info,
            title: "Info".to_string(),
            body: "test message".to_string(),
        };
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        let dialog = MessageDialog::new(&msg);
        dialog.render(area, &mut buf);

        let content = buffer_to_string(&buf, 60, 20);
        assert!(content.contains("OK (Enter)"));
    }

    #[test]
    fn message_dialog_render_returns_success_title_when_success() {
        let msg = Message {
            level: MessageLevel::Success,
            title: "Success".to_string(),
            body: "Instance started".to_string(),
        };
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        let dialog = MessageDialog::new(&msg);
        dialog.render(area, &mut buf);

        let content = buffer_to_string(&buf, 60, 20);
        assert!(content.contains("Success"));
        assert!(content.contains("Instance started"));
    }
}
