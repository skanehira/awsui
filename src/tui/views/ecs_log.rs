use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::Mode;
use crate::tab::LogViewState;
use crate::tui::components::loading::Loading;
use crate::tui::components::status_bar::StatusBar;
use crate::tui::theme;

/// ECSログビューを描画する
pub fn render(
    frame: &mut Frame,
    log_state: &LogViewState,
    loading: bool,
    spinner_tick: usize,
    mode: &Mode,
    filter_value: &str,
    area: Rect,
) {
    let outer_chunks = Layout::vertical([
        Constraint::Min(1),    // 外枠（ログ表示）
        Constraint::Length(1), // ステータスバー
    ])
    .split(area);

    let title = format!(" Logs: {} ", log_state.container_name);
    let outer_block = Block::default().title(title).borders(Borders::ALL);
    let inner = outer_block.inner(outer_chunks[0]);
    frame.render_widget(outer_block, outer_chunks[0]);

    if loading && log_state.events.is_empty() {
        let loading_widget = Loading::new("Loading logs...", spinner_tick);
        frame.render_widget(loading_widget, inner);
    } else {
        render_log_content(frame, log_state, inner);
    }

    // ステータスバー
    if *mode == Mode::Filter {
        // 検索入力モード
        let search_line = format!("/{}", filter_value);
        let status = StatusBar::new(&search_line);
        frame.render_widget(status, outer_chunks[1]);
    } else {
        let live_indicator = if log_state.auto_scroll {
            "[LIVE]"
        } else {
            "[PAUSED]"
        };

        let total = log_state.events.len();
        let position = if total > 0 {
            format!("{}/{}", log_state.scroll_offset + 1, total)
        } else {
            "0/0".to_string()
        };

        let search_info = if !log_state.search_query.is_empty() {
            let match_count = log_state.search_matches.len();
            let current = log_state.current_match_index.map(|i| i + 1).unwrap_or(0);
            format!(" [{}/{}]", current, match_count)
        } else {
            String::new()
        };

        let keybinds = format!(
            "{} {}{} j/k:scroll h/l:hscroll g/G:top/bottom f:toggle-live /:search n/N:next/prev Esc:back",
            live_indicator, position, search_info,
        );
        let status = StatusBar::new(&keybinds);
        frame.render_widget(status, outer_chunks[1]);
    }
}

/// ログコンテンツを描画
///
/// scroll_offset から表示領域に収まる分だけ Line を構築する。
/// 全イベントを走査しないため、イベント数に依存しない O(visible) の描画コスト。
/// 横スクロールは Paragraph::scroll((0, x)) で適用する。
fn render_log_content(frame: &mut Frame, log_state: &LogViewState, area: Rect) {
    if log_state.events.is_empty() {
        let empty = Paragraph::new("No log events").style(theme::header());
        frame.render_widget(empty, area);
        return;
    }

    let visible_height = area.height as usize;
    let has_search = !log_state.search_query.is_empty();
    let current_match_event_idx = log_state
        .current_match_index
        .and_then(|mi| log_state.search_matches.get(mi).copied());

    let mut visible_lines: Vec<Line> = Vec::with_capacity(visible_height);

    for event_idx in log_state.scroll_offset..log_state.events.len() {
        if visible_lines.len() >= visible_height {
            break;
        }

        let event = &log_state.events[event_idx];
        let is_match = has_search && log_state.search_matches.contains(&event_idx);
        let is_current = current_match_event_idx == Some(event_idx);
        let match_style = if is_current {
            theme::search_match_current()
        } else {
            theme::search_match()
        };

        for msg_line in event.message.split('\n') {
            if visible_lines.len() >= visible_height {
                break;
            }

            if is_match {
                let msg_spans = highlight_matches(msg_line, &log_state.search_query, match_style);
                visible_lines.push(Line::from(msg_spans));
            } else {
                visible_lines.push(Line::from(msg_line));
            }
        }
    }

    let scroll_x = log_state.scroll_x as u16;
    let para = Paragraph::new(visible_lines).scroll((0, scroll_x));
    frame.render_widget(para, area);
}

/// メッセージ内の検索文字列をハイライトする
fn highlight_matches<'a>(
    message: &'a str,
    query: &str,
    style: ratatui::style::Style,
) -> Vec<Span<'a>> {
    let mut spans = Vec::new();
    let lower = message.to_lowercase();
    let mut last = 0;

    for (start, _) in lower.match_indices(query) {
        if start > last {
            spans.push(Span::raw(&message[last..start]));
        }
        spans.push(Span::styled(&message[start..start + query.len()], style));
        last = start + query.len();
    }
    if last < message.len() {
        spans.push(Span::raw(&message[last..]));
    }
    if spans.is_empty() {
        spans.push(Span::raw(message));
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aws::logs_model::LogEvent;
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

    fn create_test_log_state() -> LogViewState {
        LogViewState {
            container_name: "app".to_string(),
            log_group: "/ecs/my-service".to_string(),
            log_stream: "ecs/app/abc123".to_string(),
            events: vec![
                LogEvent {
                    timestamp: 1706000000000,
                    formatted_time: "2024-01-23T10:00:00Z".to_string(),
                    message: "Starting application...".to_string(),
                },
                LogEvent {
                    timestamp: 1706000001000,
                    formatted_time: "2024-01-23T10:00:01Z".to_string(),
                    message: "Listening on port 8080".to_string(),
                },
                LogEvent {
                    timestamp: 1706000002000,
                    formatted_time: "2024-01-23T10:00:02Z".to_string(),
                    message: "Health check passed".to_string(),
                },
            ],
            next_forward_token: None,
            auto_scroll: true,
            scroll_offset: 2,
            scroll_x: 0,
            search_query: String::new(),
            search_matches: Vec::new(),
            current_match_index: None,
        }
    }

    #[test]
    fn render_returns_container_name_when_rendered() {
        let state = create_test_log_state();
        let backend = TestBackend::new(90, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &state, false, 0, &Mode::Normal, "", frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Logs: app"));
    }

    #[test]
    fn render_returns_log_messages_when_events_exist() {
        let mut state = create_test_log_state();
        state.scroll_offset = 0;
        let backend = TestBackend::new(90, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &state, false, 0, &Mode::Normal, "", frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Starting application"));
        assert!(content.contains("Listening on port 8080"));
    }

    #[test]
    fn render_returns_live_indicator_when_auto_scroll_enabled() {
        let state = create_test_log_state();
        let backend = TestBackend::new(90, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &state, false, 0, &Mode::Normal, "", frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("[LIVE]"));
    }

    #[test]
    fn render_returns_paused_indicator_when_auto_scroll_disabled() {
        let mut state = create_test_log_state();
        state.auto_scroll = false;
        let backend = TestBackend::new(90, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &state, false, 0, &Mode::Normal, "", frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("[PAUSED]"));
    }

    #[test]
    fn render_returns_loading_when_no_events_and_loading() {
        let mut state = create_test_log_state();
        state.events.clear();
        let backend = TestBackend::new(90, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &state, true, 0, &Mode::Normal, "", frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Loading logs"));
    }

    #[test]
    fn render_returns_keybinds_when_rendered() {
        let state = create_test_log_state();
        let backend = TestBackend::new(120, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &state, false, 0, &Mode::Normal, "", frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("j/k:scroll"));
        assert!(content.contains("f:toggle-live"));
        assert!(content.contains("/:search"));
        assert!(content.contains("Esc:back"));
    }

    #[test]
    fn render_returns_position_when_events_exist() {
        let state = create_test_log_state();
        let backend = TestBackend::new(120, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &state, false, 0, &Mode::Normal, "", frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("3/3"));
    }

    #[test]
    fn render_returns_search_input_when_filter_mode() {
        let state = create_test_log_state();
        let backend = TestBackend::new(90, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &state,
                    false,
                    0,
                    &Mode::Filter,
                    "error",
                    frame.area(),
                )
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("/error"));
    }

    #[test]
    fn render_returns_search_match_count_when_search_active() {
        let mut state = create_test_log_state();
        state.search_query = "application".to_string();
        state.search_matches = vec![0];
        state.current_match_index = Some(0);
        let backend = TestBackend::new(120, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &state, false, 0, &Mode::Normal, "", frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("[1/1]"));
    }

    #[test]
    fn render_returns_snapshot_when_rendered() {
        let state = create_test_log_state();
        let backend = TestBackend::new(120, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &state, false, 0, &Mode::Normal, "", frame.area()))
            .unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }

    #[test]
    fn render_returns_no_events_when_empty() {
        let mut state = create_test_log_state();
        state.events.clear();
        let backend = TestBackend::new(90, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &state, false, 0, &Mode::Normal, "", frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("No log events"));
    }

    #[test]
    fn render_returns_multiline_message_when_newline_in_event() {
        let mut state = create_test_log_state();
        state.events = vec![LogEvent {
            timestamp: 1706000000000,
            formatted_time: "2024-01-23T10:00:00Z".to_string(),
            message: "Error occurred\n  at main.rs:42\n  at lib.rs:10".to_string(),
        }];
        state.scroll_offset = 0;
        let backend = TestBackend::new(90, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &state, false, 0, &Mode::Normal, "", frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Error occurred"));
        assert!(content.contains("at main.rs:42"));
        assert!(content.contains("at lib.rs:10"));
    }

    #[test]
    fn render_scrolls_to_matched_event_when_search_next() {
        let mut state = create_test_log_state();
        // 画面サイズより多くのイベントを追加
        state.events.clear();
        for i in 0..30 {
            state.events.push(LogEvent {
                timestamp: 1706000000000 + i * 1000,
                formatted_time: format!("2024-01-23T10:00:{:02}Z", i),
                message: if i == 25 {
                    "TARGET error found".to_string()
                } else {
                    format!("Normal log line {}", i)
                },
            });
        }
        state.search_query = "target".to_string();
        state.search_matches = vec![25];
        state.current_match_index = Some(0);
        state.scroll_offset = 25; // n/N がここにスクロールする
        state.auto_scroll = false;

        let backend = TestBackend::new(90, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &state, false, 0, &Mode::Normal, "", frame.area()))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("TARGET error found"));
    }
}
