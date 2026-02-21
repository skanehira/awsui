use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Row, Wrap};

use crate::tui::components::dialog::centered_rect;
use crate::tui::components::status_bar::render_footer;
use crate::tui::components::table::{SelectableTable, SelectableTableWidget};
use crate::tui::theme;
use crate::ui_state::ProfileSelectorState;

/// スピナーフレーム
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// プロファイル選択画面を描画する
pub fn render(frame: &mut Frame, state: &ProfileSelectorState, spinner_tick: usize) {
    let area = frame.area();

    let outer_chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(area);

    // 外枠
    let outer_block = Block::default()
        .title(" Select Profile ")
        .borders(Borders::ALL);
    let inner = outer_block.inner(outer_chunks[0]);
    frame.render_widget(outer_block, outer_chunks[0]);

    // テーブル
    let headers = Row::new(vec!["Name", "Region", "URL"]).style(theme::header());

    let rows: Vec<Row> = state
        .filtered_profiles
        .iter()
        .map(|p| {
            Row::new(vec![
                p.name.as_str(),
                p.region.as_deref().unwrap_or(""),
                p.sso_start_url.as_str(),
            ])
        })
        .collect();

    let widths = vec![
        Constraint::Length(20),
        Constraint::Length(18),
        Constraint::Min(20),
    ];

    let table = SelectableTable::new(headers, rows, widths);
    let widget = SelectableTableWidget::new(table, state.selected_index);
    frame.render_widget(widget, inner);

    // フッター
    render_footer(
        frame,
        outer_chunks[1],
        &state.mode,
        state.filter_input.value(),
        "j/k:move  Enter:select  /:filter  q:quit",
    );

    // SSO loginダイアログ（logging_in時にオーバーレイ表示）
    if state.logging_in {
        render_sso_login_dialog(frame, state, spinner_tick);
    }
}

/// SSO loginダイアログを描画する
fn render_sso_login_dialog(frame: &mut Frame, state: &ProfileSelectorState, spinner_tick: usize) {
    let area = frame.area();
    let output_lines = state.login_output.len() as u16;
    // タイトル(1) + ボーダー(2) + 出力行 + スピナー行(1) + 空行(1) + Escヒント(1)
    let height = (output_lines + 6).max(8).min(area.height);
    let popup = centered_rect(60, height, area);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" SSO Login ")
        .borders(Borders::ALL)
        .style(theme::active());
    let dialog_inner = block.inner(popup);
    frame.render_widget(block, popup);

    let chunks = Layout::vertical([
        Constraint::Min(1),    // 出力行
        Constraint::Length(1), // スピナー
        Constraint::Length(1), // 空行
        Constraint::Length(1), // [Esc] Cancel
    ])
    .split(dialog_inner);

    // login_output行を表示
    let output_text: Vec<Line> = state
        .login_output
        .iter()
        .map(|l| Line::from(l.as_str()))
        .collect();
    let output = Paragraph::new(output_text).wrap(Wrap { trim: false });
    frame.render_widget(output, chunks[0]);

    // スピナー
    let spinner = SPINNER_FRAMES[(spinner_tick / 6) % SPINNER_FRAMES.len()];
    let spinner_line = Paragraph::new(Line::from(vec![Span::styled(
        format!("{} Waiting for authentication...", spinner),
        theme::info(),
    )]))
    .alignment(Alignment::Center);
    frame.render_widget(spinner_line, chunks[1]);

    // [Esc] Cancel
    let cancel = Paragraph::new(Line::from(Span::styled("[Esc] Cancel", theme::inactive())))
        .alignment(Alignment::Center);
    frame.render_widget(cancel, chunks[3]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SsoProfile;
    use crate::ui_state::ProfileSelectorState;
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

    fn test_profiles() -> Vec<SsoProfile> {
        vec![
            SsoProfile {
                name: "dev-account".to_string(),
                region: Some("ap-northeast-1".to_string()),
                sso_start_url: "https://dev.awsapps.com/start".to_string(),
                sso_session: None,
            },
            SsoProfile {
                name: "staging".to_string(),
                region: Some("us-east-1".to_string()),
                sso_start_url: "https://staging.awsapps.com/start".to_string(),
                sso_session: None,
            },
            SsoProfile {
                name: "production".to_string(),
                region: Some("ap-northeast-1".to_string()),
                sso_start_url: "https://prod.awsapps.com/start".to_string(),
                sso_session: None,
            },
        ]
    }

    #[test]
    fn render_returns_snapshot_when_profiles_displayed() {
        let state = ProfileSelectorState::new(test_profiles());
        let backend = TestBackend::new(70, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &state, 0)).unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }

    #[test]
    fn render_returns_snapshot_when_second_item_selected() {
        let mut state = ProfileSelectorState::new(test_profiles());
        state.selected_index = 1;
        let backend = TestBackend::new(70, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &state, 0)).unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }

    #[test]
    fn render_returns_snapshot_when_no_profiles() {
        let state = ProfileSelectorState::new(vec![]);
        let backend = TestBackend::new(70, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &state, 0)).unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }

    #[test]
    fn render_returns_snapshot_when_filter_mode() {
        use crate::ui_state::Mode;

        let mut state = ProfileSelectorState::new(test_profiles());
        state.mode = Mode::Filter;
        state.filter_input = "dev".into();
        state.apply_filter();
        let backend = TestBackend::new(70, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &state, 0)).unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }

    #[test]
    fn render_returns_snapshot_when_sso_login_dialog() {
        let mut state = ProfileSelectorState::new(test_profiles());
        state.logging_in = true;
        state.login_output = vec![
            "Attempting to automatically open the SSO authorization page in your browser."
                .to_string(),
            "https://device.sso.ap-northeast-1.amazonaws.com/".to_string(),
            "Enter code: ABCD-EFGH".to_string(),
        ];
        let backend = TestBackend::new(70, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &state, 0)).unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }

    #[test]
    fn render_returns_snapshot_when_sso_login_dialog_empty_output() {
        let mut state = ProfileSelectorState::new(test_profiles());
        state.logging_in = true;
        let backend = TestBackend::new(70, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &state, 0)).unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }
}
