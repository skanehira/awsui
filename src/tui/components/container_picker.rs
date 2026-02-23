use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Row, Widget};

use crate::tui::components::dialog::centered_rect;
use crate::tui::components::table::SelectableTableWidget;
use crate::ui_state::ContainerSelectState;

/// コンテナ選択ポップアップウィジェット（テーブル+あいまい検索）
pub struct ContainerPicker<'a> {
    state: &'a ContainerSelectState,
}

impl<'a> ContainerPicker<'a> {
    pub fn new(state: &'a ContainerSelectState) -> Self {
        Self { state }
    }
}

impl Widget for ContainerPicker<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // ポップアップサイズ: コンテナ数 + フィルタ入力(1行) + ヘッダー(1行) + 枠(2行)
        let height = (self.state.filtered_names.len() as u16 + 4).min(area.height);
        let popup = centered_rect(50, height, area);
        Clear.render(popup, buf);

        let block = Block::default()
            .title(" Select Container ")
            .borders(Borders::ALL)
            .style(Style::default());
        let inner = block.inner(popup);
        block.render(popup, buf);

        if inner.height < 2 {
            return;
        }

        // フィルタ入力行（1行）
        let filter_area = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        };
        let filter_value = self.state.filter_input.value();
        let filter_line = Line::from(Span::styled(
            format!("/{}", filter_value),
            Style::default().add_modifier(Modifier::DIM),
        ));
        Paragraph::new(filter_line).render(filter_area, buf);

        // テーブル領域（フィルタ行の下）
        let table_area = Rect {
            x: inner.x,
            y: inner.y + 1,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };

        let headers = Row::new(vec!["Container"]);
        let rows: Vec<Row> = self
            .state
            .filtered_names
            .iter()
            .map(|name| Row::new(vec![name.as_str()]))
            .collect();
        let widths = vec![Constraint::Percentage(100)];

        let table = crate::tui::components::table::SelectableTable::new(headers, rows, widths);
        let widget = SelectableTableWidget::new(table, self.state.selected_index);
        widget.render(table_area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use tui_input::Input;

    use crate::ui_state::ContainerSelectPurpose;

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
    fn render_returns_snapshot_when_all_containers_shown() {
        let state = ContainerSelectState {
            all_names: vec!["web".to_string(), "sidecar".to_string()],
            filtered_names: vec!["web".to_string(), "sidecar".to_string()],
            selected_index: 0,
            filter_input: Input::default(),
            purpose: ContainerSelectPurpose::ShowLogs,
        };
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let widget = ContainerPicker::new(&state);
                frame.render_widget(widget, frame.area());
            })
            .unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }

    #[test]
    fn render_returns_snapshot_when_filtered_with_selection() {
        let state = ContainerSelectState {
            all_names: vec![
                "web".to_string(),
                "sidecar".to_string(),
                "worker".to_string(),
            ],
            filtered_names: vec!["web".to_string(), "worker".to_string()],
            selected_index: 1,
            filter_input: Input::from("w"),
            purpose: ContainerSelectPurpose::EcsExec,
        };
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let widget = ContainerPicker::new(&state);
                frame.render_widget(widget, frame.area());
            })
            .unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }
}
