use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::{Block, Borders};

use crate::app::App;
use crate::tui::components::list_selector::ListSelector;
use crate::tui::components::status_bar::StatusBar;

/// Profile選択画面を描画する
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let outer_chunks = Layout::vertical([
        Constraint::Min(1),    // 外枠（リスト）
        Constraint::Length(1), // ステータスバー
    ])
    .split(area);

    // 外枠Block
    let outer_block = Block::default()
        .title(" Select AWS SSO Profile ")
        .borders(Borders::ALL);
    let inner = outer_block.inner(outer_chunks[0]);
    frame.render_widget(outer_block, outer_chunks[0]);

    // リスト選択
    let selector = ListSelector::new("", &app.profile_names, app.profile_selected);
    frame.render_widget(selector, inner);

    // ステータスバー
    let status = StatusBar::new("j/k:select  Enter:confirm  q:quit");
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
    fn render_returns_profile_list_when_profiles_provided() {
        let app = App::new(vec![
            "dev-account".to_string(),
            "staging-account".to_string(),
            "prod-account".to_string(),
        ]);
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(frame, &app);
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Select AWS SSO Profile"));
        assert!(content.contains("dev-account"));
        assert!(content.contains("staging-account"));
        assert!(content.contains("prod-account"));
    }

    #[test]
    fn render_returns_selected_marker_when_first_profile_selected() {
        let app = App::new(vec!["dev".to_string(), "staging".to_string()]);
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(frame, &app);
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("▶ dev"));
    }

    #[test]
    fn render_returns_status_bar_when_rendered() {
        let app = App::new(vec!["dev".to_string()]);
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(frame, &app);
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("j/k:select"));
    }

    #[test]
    fn render_returns_snapshot_when_rendered() {
        let app = App::new(vec![
            "dev-account".to_string(),
            "staging-account".to_string(),
            "prod-account".to_string(),
        ]);
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(frame, &app);
            })
            .unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }
}
