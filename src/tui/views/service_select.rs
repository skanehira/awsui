use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::{Block, Borders};

use crate::app::App;
use crate::service::ServiceKind;
use crate::tui::components::list_selector::ListSelector;
use crate::tui::components::status_bar::render_footer;

/// 利用可能なAWSサービス名一覧（ServiceKindから生成）
pub fn service_names() -> Vec<String> {
    ServiceKind::ALL
        .iter()
        .map(|s| s.short_name().to_string())
        .collect()
}

/// サービス選択画面を描画する
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let outer_chunks = Layout::vertical([
        Constraint::Min(1),    // 外枠（リスト）
        Constraint::Length(1), // フッター
    ])
    .split(area);

    // 外枠Block
    let outer_block = Block::default()
        .title(" Select AWS Service ")
        .borders(Borders::ALL);
    let inner = outer_block.inner(outer_chunks[0]);
    frame.render_widget(outer_block, outer_chunks[0]);

    // リスト（ダッシュボードの状態から取得）
    let service_names: Vec<String> = app
        .dashboard
        .filtered_services
        .iter()
        .map(|s| s.short_name().to_string())
        .collect();
    let selector = ListSelector::new("", &service_names, app.dashboard.selected_index);
    frame.render_widget(selector, inner);

    // フッター
    render_footer(
        frame,
        outer_chunks[1],
        &app.dashboard.mode,
        app.dashboard.filter_input.value(),
        "j/k:select  /:filter  Enter:confirm  Esc:back  q:quit",
    );
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
    fn render_returns_snapshot_when_rendered() {
        let app = App::new("dev".to_string(), None);
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &app)).unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }
}
