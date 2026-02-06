use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::{Block, Borders};

use crate::tui::components::list_selector::ListSelector;
use crate::tui::components::status_bar::StatusBar;

/// 利用可能なAWSサービス一覧
pub const SERVICE_NAMES: &[&str] = &["EC2", "ECR", "ECS", "S3", "VPC", "Secrets Manager"];

/// サービス選択画面を描画する
pub fn render(frame: &mut Frame, selected: usize) {
    let area = frame.area();
    let outer_chunks = Layout::vertical([
        Constraint::Min(1),    // 外枠（リスト）
        Constraint::Length(1), // ステータスバー
    ])
    .split(area);

    // 外枠Block
    let outer_block = Block::default()
        .title(" Select AWS Service ")
        .borders(Borders::ALL);
    let inner = outer_block.inner(outer_chunks[0]);
    frame.render_widget(outer_block, outer_chunks[0]);

    // リスト選択
    let items: Vec<String> = SERVICE_NAMES.iter().map(|s| s.to_string()).collect();
    let selector = ListSelector::new("", &items, selected);
    frame.render_widget(selector, inner);

    // ステータスバー
    let status = StatusBar::new("j/k:select  Enter:confirm  Esc:back  q:quit");
    frame.render_widget(status, outer_chunks[1]);
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
    fn render_returns_service_list_when_rendered() {
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, 0)).unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Select AWS Service"));
        assert!(content.contains("EC2"));
        assert!(content.contains("ECR"));
        assert!(content.contains("ECS"));
        assert!(content.contains("S3"));
        assert!(content.contains("VPC"));
        assert!(content.contains("Secrets Manager"));
    }

    #[test]
    fn render_returns_selected_marker_when_first_service_selected() {
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, 0)).unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("▶ EC2"));
    }

    #[test]
    fn render_returns_status_bar_when_rendered() {
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, 0)).unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("j/k:select"));
    }

    #[test]
    fn render_returns_snapshot_when_rendered() {
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, 0)).unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }
}
