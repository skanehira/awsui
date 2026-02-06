use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::widgets::{Row, StatefulWidget, Table as RatatuiTable, TableState, Widget};

use crate::tui::theme;

/// 選択可能テーブルWidget
pub struct SelectableTable<'a> {
    headers: Row<'a>,
    rows: Vec<Row<'a>>,
    widths: Vec<Constraint>,
}

impl<'a> SelectableTable<'a> {
    pub fn new(headers: Row<'a>, rows: Vec<Row<'a>>, widths: Vec<Constraint>) -> Self {
        Self {
            headers,
            rows,
            widths,
        }
    }
}

impl StatefulWidget for SelectableTable<'_> {
    type State = TableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let table = RatatuiTable::new(self.rows, &self.widths)
            .header(self.headers.style(theme::header()))
            .row_highlight_style(theme::selected());

        StatefulWidget::render(table, area, buf, state);
    }
}

/// StatefulWidgetではなくWidgetとして使いたい場合のラッパー
pub struct SelectableTableWidget<'a> {
    table: SelectableTable<'a>,
    selected: usize,
}

impl<'a> SelectableTableWidget<'a> {
    pub fn new(table: SelectableTable<'a>, selected: usize) -> Self {
        Self { table, selected }
    }
}

impl Widget for SelectableTableWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = TableState::default().with_selected(self.selected);
        StatefulWidget::render(self.table, area, buf, &mut state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::text::Line;

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
    fn selectable_table_render_returns_headers_when_rendered() {
        let headers = Row::new(vec!["ID", "Name", "State"]);
        let rows = vec![Row::new(vec!["i-001", "web", "running"])];
        let widths = vec![
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(10),
        ];
        let table = SelectableTable::new(headers, rows, widths);
        let widget = SelectableTableWidget::new(table, 0);

        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let content = buffer_to_string(&buf, 40, 10);
        assert!(content.contains("ID"));
        assert!(content.contains("Name"));
        assert!(content.contains("State"));
    }

    #[test]
    fn selectable_table_render_returns_rows_when_data_provided() {
        let headers = Row::new(vec!["ID", "Name"]);
        let rows = vec![
            Row::new(vec!["i-001", "web"]),
            Row::new(vec!["i-002", "api"]),
        ];
        let widths = vec![Constraint::Length(10), Constraint::Length(10)];
        let table = SelectableTable::new(headers, rows, widths);
        let widget = SelectableTableWidget::new(table, 0);

        let area = Rect::new(0, 0, 30, 10);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let content = buffer_to_string(&buf, 30, 10);
        assert!(content.contains("i-001"));
        assert!(content.contains("web"));
        assert!(content.contains("i-002"));
        assert!(content.contains("api"));
    }

    #[test]
    fn selectable_table_render_returns_highlighted_row_when_selected() {
        let headers = Row::new(vec!["ID"]);
        let rows = vec![
            Row::new(vec![Line::from("i-001")]),
            Row::new(vec![Line::from("i-002")]),
        ];
        let widths = vec![Constraint::Length(10)];
        let table = SelectableTable::new(headers, rows, widths);

        let area = Rect::new(0, 0, 20, 10);
        let mut buf = Buffer::empty(area);
        let mut state = TableState::default().with_selected(1);
        StatefulWidget::render(table, area, &mut buf, &mut state);

        // i-002が含まれている行を探して、そのスタイルが selected() であることを確認
        let content = buffer_to_string(&buf, 20, 10);
        assert!(content.contains("i-002"));

        // 選択行のスタイルを検証: i-002の行を探す
        let expected = theme::selected();
        let mut found = false;
        for y in 0..10u16 {
            let row_str: String = (0..20u16)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect();
            if row_str.contains("i-002") {
                let cell = &buf[(0, y)];
                let style = cell.style();
                assert_eq!(style.fg, expected.fg);
                assert_eq!(style.bg, expected.bg);
                found = true;
                break;
            }
        }
        assert!(found, "i-002 row should be found in buffer");
    }
}
