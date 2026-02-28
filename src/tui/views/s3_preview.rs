use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::aws::s3_model::ObjectContent;
use crate::tui::components::loading::Loading;
use crate::tui::components::status_bar::StatusBar;
use crate::tui::theme;

/// S3オブジェクトプレビュー画面を描画する
pub fn render(
    frame: &mut Frame,
    key: &str,
    content: Option<&ObjectContent>,
    scroll: usize,
    loading: bool,
    spinner_tick: usize,
    area: Rect,
) {
    let outer_chunks = Layout::vertical([
        Constraint::Min(1),    // 外枠
        Constraint::Length(1), // フッター
    ])
    .split(area);

    let title = format!(" Preview: {} ", key);
    let outer_block = Block::default().title(title).borders(Borders::ALL);
    let inner = outer_block.inner(outer_chunks[0]);
    frame.render_widget(outer_block, outer_chunks[0]);

    if loading {
        let loading_widget = Loading::new("Loading preview...", spinner_tick);
        frame.render_widget(loading_widget, inner);
    } else if let Some(content) = content {
        // メタ情報行
        let meta = Line::from(vec![
            Span::styled("Type: ", theme::header()),
            Span::raw(&content.content_type),
            Span::raw("  "),
            Span::styled("Size: ", theme::header()),
            Span::raw(format_size(content.size)),
        ]);

        let meta_chunks = Layout::vertical([
            Constraint::Length(1), // メタ情報
            Constraint::Min(1),    // 本文
        ])
        .split(inner);

        frame.render_widget(Paragraph::new(meta), meta_chunks[0]);

        // 本文
        let lines: Vec<Line> = content
            .body
            .lines()
            .enumerate()
            .map(|(i, line)| {
                Line::from(vec![
                    Span::styled(format!("{:>4} ", i + 1), theme::inactive()),
                    Span::raw(line),
                ])
            })
            .collect();

        let para = Paragraph::new(lines)
            .scroll((scroll as u16, 0))
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::TOP));
        frame.render_widget(para, meta_chunks[1]);
    }

    let keybinds = "j/k:scroll g/G:top/bottom Esc:back ?:help";
    let status = StatusBar::new(keybinds);
    frame.render_widget(status, outer_chunks[1]);
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;

    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn render_returns_loading_when_loading() {
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, "test.txt", None, 0, true, 0, frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Loading preview..."));
        assert!(content.contains("Preview: test.txt"));
    }

    #[test]
    fn render_returns_content_when_loaded() {
        let obj = ObjectContent {
            content_type: "text/plain".to_string(),
            body: "Hello\nWorld".to_string(),
            size: 11,
        };
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, "test.txt", Some(&obj), 0, false, 0, frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Preview: test.txt"));
        assert!(content.contains("text/plain"));
        assert!(content.contains("11 B"));
        assert!(content.contains("Hello"));
        assert!(content.contains("World"));
    }

    #[test]
    fn render_returns_keybinds_when_displayed() {
        let obj = ObjectContent {
            content_type: "text/plain".to_string(),
            body: "test".to_string(),
            size: 4,
        };
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, "test.txt", Some(&obj), 0, false, 0, frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("j/k:scroll"));
        assert!(content.contains("Esc:back"));
    }

    #[test]
    fn render_returns_snapshot_when_preview_rendered() {
        let obj = ObjectContent {
            content_type: "application/json".to_string(),
            body: "{\n  \"name\": \"test\",\n  \"version\": \"1.0.0\"\n}".to_string(),
            size: 42,
        };
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, "config.json", Some(&obj), 0, false, 0, frame.area()))
            .unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }
}
