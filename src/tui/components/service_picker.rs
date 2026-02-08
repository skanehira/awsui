use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::app::ServicePickerState;
use crate::tui::components::dialog::centered_rect;
use crate::tui::theme;

/// サービスピッカーポップアップウィジェット
pub struct ServicePicker<'a> {
    state: &'a ServicePickerState,
}

impl<'a> ServicePicker<'a> {
    pub fn new(state: &'a ServicePickerState) -> Self {
        Self { state }
    }
}

impl Widget for ServicePicker<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // ポップアップサイズ: サービス数 + フィルタ入力(1行) + 枠(2行)
        let height = (self.state.filtered_services.len() as u16 + 3).min(area.height);
        let popup = centered_rect(60, height, area);
        Clear.render(popup, buf);

        let block = Block::default()
            .title(" Open Service ")
            .borders(Borders::ALL)
            .style(Style::default());
        let inner = block.inner(popup);
        block.render(popup, buf);

        let mut lines = Vec::new();

        // フィルタ入力行
        let filter_value = self.state.filter_input.value();
        lines.push(Line::from(Span::styled(
            format!("/{}", filter_value),
            Style::default().add_modifier(Modifier::DIM),
        )));

        // サービス一覧
        for (i, service) in self.state.filtered_services.iter().enumerate() {
            let style = if i == self.state.selected_index {
                theme::active()
            } else {
                Style::default()
            };
            let marker = if i == self.state.selected_index {
                "▶ "
            } else {
                "  "
            };
            lines.push(Line::from(Span::styled(
                format!("{}{}", marker, service.full_name()),
                style,
            )));
        }

        Paragraph::new(lines).render(inner, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::ServiceKind;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use tui_input::Input;

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
    fn render_returns_snapshot_when_all_services_shown() {
        let state = ServicePickerState {
            selected_index: 0,
            filter_input: Input::default(),
            filtered_services: ServiceKind::ALL.to_vec(),
        };
        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let widget = ServicePicker::new(&state);
                frame.render_widget(widget, frame.area());
            })
            .unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }

    #[test]
    fn render_returns_snapshot_when_filtered_with_selection() {
        let state = ServicePickerState {
            selected_index: 1,
            filter_input: Input::from("ec"),
            filtered_services: vec![ServiceKind::Ec2, ServiceKind::Ecr, ServiceKind::Ecs],
        };
        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let widget = ServicePicker::new(&state);
                frame.render_widget(widget, frame.area());
            })
            .unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }
}
