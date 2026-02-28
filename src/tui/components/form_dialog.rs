use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::app::{DangerConfirmContext, FormContext};
use crate::tui::components::dialog::centered_rect;
use crate::tui::theme;

/// フォーム入力ダイアログWidget
pub struct FormDialog<'a> {
    context: &'a FormContext,
}

impl<'a> FormDialog<'a> {
    pub fn new(context: &'a FormContext) -> Self {
        Self { context }
    }
}

impl Widget for FormDialog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // 高さ: タイトル枠(2) + フィールド毎(label+input+空行=3) + ボタン(1) + 空行(1)
        let field_count = self.context.fields.len() as u16;
        let height = (field_count * 3 + 4).min(area.height);
        let popup = centered_rect(75, height, area);
        Clear.render(popup, buf);

        let title = match self.context.kind {
            crate::app::FormKind::CreateS3Bucket => " Create S3 Bucket ",
            crate::app::FormKind::CreateSecret => " Create Secret ",
            crate::app::FormKind::UpdateSecretValue => " Update Value ",
            crate::app::FormKind::ScaleEcsService => " Scale Service ",
            crate::app::FormKind::CreateEcrRepository => " Create ECR Repository ",
            crate::app::FormKind::DownloadS3Object => " Download Object ",
            crate::app::FormKind::UploadS3Object => " Upload Object ",
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .style(theme::active());

        let inner = block.inner(popup);
        block.render(popup, buf);

        // レイアウト: フィールド群 + 空行 + ボタン行
        let mut constraints: Vec<Constraint> = Vec::new();
        for _ in &self.context.fields {
            constraints.push(Constraint::Length(1)); // label
            constraints.push(Constraint::Length(1)); // input
            constraints.push(Constraint::Length(1)); // spacing
        }
        constraints.push(Constraint::Length(1)); // button

        let chunks = Layout::vertical(constraints).split(inner);

        for (i, field) in self.context.fields.iter().enumerate() {
            let base = i * 3;

            // Label
            let required_marker = if field.required { "*" } else { "" };
            let label_style = if i == self.context.focused_field {
                theme::active()
            } else {
                theme::inactive()
            };
            let label = Paragraph::new(Line::from(vec![Span::styled(
                format!("{}{}: ", field.label, required_marker),
                label_style,
            )]));
            if base < chunks.len() {
                label.render(chunks[base], buf);
            }

            // Input field
            let input_style = if i == self.context.focused_field {
                theme::selected()
            } else {
                ratatui::style::Style::default()
            };
            let value = field.input.value();
            let display = if self.context.kind == crate::app::FormKind::UpdateSecretValue
                && field.label.starts_with("New value")
            {
                "*".repeat(value.len())
            } else {
                value.to_string()
            };
            let input_para = Paragraph::new(format!(" {}", display)).style(input_style);
            if base + 1 < chunks.len() {
                input_para.render(chunks[base + 1], buf);

                // カーソル表示 (フォーカス中のフィールドのみ)
                if i == self.context.focused_field {
                    let cursor_x = chunks[base + 1].x + 1 + field.input.visual_cursor() as u16;
                    let cursor_y = chunks[base + 1].y;
                    if cursor_x < chunks[base + 1].x + chunks[base + 1].width
                        && cursor_y < chunks[base + 1].y + chunks[base + 1].height
                    {
                        buf[(cursor_x, cursor_y)].set_style(
                            ratatui::style::Style::default()
                                .bg(ratatui::style::Color::White)
                                .fg(ratatui::style::Color::Black),
                        );
                    }
                }
            }
        }

        // ボタン行
        let button_idx = chunks.len().saturating_sub(1);
        let buttons = Line::from(vec![
            Span::styled("[ Submit (Enter) ]", theme::active()),
            Span::raw("  "),
            Span::raw("[ Cancel (Esc) ]"),
            Span::raw("  "),
            Span::raw("[ Next Field (Tab) ]"),
        ])
        .alignment(Alignment::Center);
        let button_para = Paragraph::new(buttons);
        if button_idx < chunks.len() {
            button_para.render(chunks[button_idx], buf);
        }
    }
}

/// 危険操作確認ダイアログWidget
pub struct DangerConfirmDialog<'a> {
    context: &'a DangerConfirmContext,
}

impl<'a> DangerConfirmDialog<'a> {
    pub fn new(context: &'a DangerConfirmContext) -> Self {
        Self { context }
    }
}

impl Widget for DangerConfirmDialog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup = centered_rect(60, 9, area);
        Clear.render(popup, buf);

        let block = Block::default()
            .title(" Danger Confirm ")
            .title_style(theme::error())
            .borders(Borders::ALL)
            .style(theme::error());

        let inner = block.inner(popup);
        block.render(popup, buf);

        let chunks = Layout::vertical([
            Constraint::Length(1), // 空行
            Constraint::Length(1), // メッセージ
            Constraint::Length(1), // 空行
            Constraint::Length(1), // 入力フィールド
            Constraint::Length(1), // 空行
            Constraint::Length(1), // ボタン
        ])
        .split(inner);

        // メッセージ
        let msg =
            Paragraph::new(Line::from(self.context.action.message())).alignment(Alignment::Center);
        msg.render(chunks[1], buf);

        // 入力フィールド
        let input_value = self.context.input.value();
        let matches = input_value == self.context.action.confirm_text();
        let input_style = if matches {
            theme::success()
        } else {
            ratatui::style::Style::default().fg(ratatui::style::Color::White)
        };
        let input_para =
            Paragraph::new(Line::from(format!(" > {}", input_value)).style(input_style))
                .style(theme::selected());
        input_para.render(chunks[3], buf);

        // カーソル
        let cursor_x = chunks[3].x + 3 + self.context.input.visual_cursor() as u16;
        let cursor_y = chunks[3].y;
        if cursor_x < chunks[3].x + chunks[3].width {
            buf[(cursor_x, cursor_y)].set_style(
                ratatui::style::Style::default()
                    .bg(ratatui::style::Color::White)
                    .fg(ratatui::style::Color::Black),
            );
        }

        // ボタン
        let submit_style = if matches {
            theme::active()
        } else {
            theme::inactive()
        };
        let buttons = Line::from(vec![
            Span::styled("[ Confirm (Enter) ]", submit_style),
            Span::raw("    "),
            Span::raw("[ Cancel (Esc) ]"),
        ])
        .alignment(Alignment::Center);
        let button_para = Paragraph::new(buttons);
        button_para.render(chunks[5], buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{DangerAction, DangerConfirmContext, FormContext, FormField, FormKind};
    use tui_input::Input;

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
    fn form_dialog_render_returns_title_when_create_s3_bucket() {
        let ctx = FormContext {
            kind: FormKind::CreateS3Bucket,
            fields: vec![FormField {
                label: "Bucket Name".to_string(),
                input: Input::default(),
                required: true,
            }],
            focused_field: 0,
        };
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        let dialog = FormDialog::new(&ctx);
        dialog.render(area, &mut buf);

        let content = buffer_to_string(&buf, 80, 30);
        assert!(content.contains("Create S3 Bucket"));
    }

    #[test]
    fn form_dialog_render_returns_field_label_when_rendered() {
        let ctx = FormContext {
            kind: FormKind::CreateS3Bucket,
            fields: vec![FormField {
                label: "Bucket Name".to_string(),
                input: Input::default(),
                required: true,
            }],
            focused_field: 0,
        };
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        let dialog = FormDialog::new(&ctx);
        dialog.render(area, &mut buf);

        let content = buffer_to_string(&buf, 80, 30);
        assert!(content.contains("Bucket Name*"));
    }

    #[test]
    fn form_dialog_render_returns_buttons_when_rendered() {
        let ctx = FormContext {
            kind: FormKind::CreateS3Bucket,
            fields: vec![FormField {
                label: "Name".to_string(),
                input: Input::default(),
                required: true,
            }],
            focused_field: 0,
        };
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        let dialog = FormDialog::new(&ctx);
        dialog.render(area, &mut buf);

        let content = buffer_to_string(&buf, 80, 30);
        assert!(content.contains("Submit (Enter)"));
        assert!(content.contains("Cancel (Esc)"));
        assert!(content.contains("Next Field (Tab)"));
    }

    #[test]
    fn form_dialog_render_returns_title_when_create_secret() {
        let ctx = FormContext {
            kind: FormKind::CreateSecret,
            fields: vec![
                FormField {
                    label: "Name".to_string(),
                    input: Input::default(),
                    required: true,
                },
                FormField {
                    label: "Value".to_string(),
                    input: Input::default(),
                    required: true,
                },
                FormField {
                    label: "Description".to_string(),
                    input: Input::default(),
                    required: false,
                },
            ],
            focused_field: 0,
        };
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        let dialog = FormDialog::new(&ctx);
        dialog.render(area, &mut buf);

        let content = buffer_to_string(&buf, 80, 30);
        assert!(content.contains("Create Secret"));
        assert!(content.contains("Name*"));
        assert!(content.contains("Value*"));
        assert!(content.contains("Description:"));
    }

    #[test]
    fn form_dialog_render_returns_input_value_when_typed() {
        let mut input = Input::default();
        input.handle(tui_input::InputRequest::InsertChar('t'));
        input.handle(tui_input::InputRequest::InsertChar('e'));
        input.handle(tui_input::InputRequest::InsertChar('s'));
        input.handle(tui_input::InputRequest::InsertChar('t'));
        let ctx = FormContext {
            kind: FormKind::CreateS3Bucket,
            fields: vec![FormField {
                label: "Bucket Name".to_string(),
                input,
                required: true,
            }],
            focused_field: 0,
        };
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        let dialog = FormDialog::new(&ctx);
        dialog.render(area, &mut buf);

        let content = buffer_to_string(&buf, 80, 30);
        assert!(content.contains("test"));
    }

    #[test]
    fn form_dialog_render_returns_masked_value_when_update_secret() {
        let mut input = Input::default();
        input.handle(tui_input::InputRequest::InsertChar('a'));
        input.handle(tui_input::InputRequest::InsertChar('b'));
        input.handle(tui_input::InputRequest::InsertChar('c'));
        let ctx = FormContext {
            kind: FormKind::UpdateSecretValue,
            fields: vec![FormField {
                label: "New value for 'my-key'".to_string(),
                input,
                required: true,
            }],
            focused_field: 0,
        };
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        let dialog = FormDialog::new(&ctx);
        dialog.render(area, &mut buf);

        let content = buffer_to_string(&buf, 80, 30);
        assert!(content.contains("***"));
        assert!(!content.contains("abc"));
    }

    #[test]
    fn danger_confirm_dialog_render_returns_title_when_rendered() {
        let ctx = DangerConfirmContext {
            action: DangerAction::TerminateEc2("i-001".to_string()),
            input: Input::default(),
        };
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        let dialog = DangerConfirmDialog::new(&ctx);
        dialog.render(area, &mut buf);

        let content = buffer_to_string(&buf, 80, 30);
        assert!(content.contains("Danger Confirm"));
    }

    #[test]
    fn danger_confirm_dialog_render_returns_message_when_rendered() {
        let ctx = DangerConfirmContext {
            action: DangerAction::TerminateEc2("i-001".to_string()),
            input: Input::default(),
        };
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        let dialog = DangerConfirmDialog::new(&ctx);
        dialog.render(area, &mut buf);

        let content = buffer_to_string(&buf, 80, 30);
        assert!(content.contains("i-001"));
        assert!(content.contains("terminate"));
    }

    #[test]
    fn danger_confirm_dialog_render_returns_buttons_when_rendered() {
        let ctx = DangerConfirmContext {
            action: DangerAction::DeleteS3Bucket("my-bucket".to_string()),
            input: Input::default(),
        };
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        let dialog = DangerConfirmDialog::new(&ctx);
        dialog.render(area, &mut buf);

        let content = buffer_to_string(&buf, 80, 30);
        assert!(content.contains("Confirm (Enter)"));
        assert!(content.contains("Cancel (Esc)"));
    }

    #[test]
    fn danger_confirm_dialog_render_returns_input_value_when_typed() {
        let mut input = Input::default();
        for c in "i-001".chars() {
            input.handle(tui_input::InputRequest::InsertChar(c));
        }
        let ctx = DangerConfirmContext {
            action: DangerAction::TerminateEc2("i-001".to_string()),
            input,
        };
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        let dialog = DangerConfirmDialog::new(&ctx);
        dialog.render(area, &mut buf);

        let content = buffer_to_string(&buf, 80, 30);
        assert!(content.contains("i-001"));
    }

    #[test]
    fn danger_confirm_dialog_render_returns_delete_message_when_s3_object() {
        let ctx = DangerConfirmContext {
            action: DangerAction::DeleteS3Object {
                bucket: "my-bucket".to_string(),
                key: "path/to/file.txt".to_string(),
            },
            input: Input::default(),
        };
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        let dialog = DangerConfirmDialog::new(&ctx);
        dialog.render(area, &mut buf);

        let content = buffer_to_string(&buf, 80, 30);
        assert!(content.contains("path/to/file.txt"));
        assert!(content.contains("delete"));
    }
}
