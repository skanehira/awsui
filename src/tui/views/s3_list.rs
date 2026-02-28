use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Row};
use tui_input::Input;

use crate::app::Mode;
use crate::aws::s3_model::Bucket;
use crate::tui::components::loading::Loading;
use crate::tui::components::status_bar::render_footer;
use crate::tui::components::table::{SelectableTable, SelectableTableWidget};
use crate::tui::theme;

/// S3バケット一覧画面の描画に必要なプロパティ
pub struct S3ListProps<'a> {
    pub buckets: &'a [Bucket],
    pub selected_index: usize,
    pub filter_input: &'a Input,
    pub mode: &'a Mode,
    pub loading: bool,
    pub spinner_tick: usize,
    pub profile: Option<&'a str>,
    pub region: Option<&'a str>,
    pub can_delete: bool,
}

/// S3バケット一覧画面を描画する
pub fn render(frame: &mut Frame, props: &S3ListProps, area: Rect) {
    let S3ListProps {
        buckets,
        selected_index,
        filter_input,
        mode,
        loading,
        spinner_tick,
        profile,
        region,
        can_delete,
    } = props;
    let (selected_index, loading, spinner_tick) = (*selected_index, *loading, *spinner_tick);
    let (profile, region) = (*profile, *region);
    // フッターは外枠の外に配置
    let outer_chunks = Layout::vertical([
        Constraint::Min(1),    // 外枠（テーブル）
        Constraint::Length(1), // フッター
    ])
    .split(area);

    // 右タイトル（profile │ region）
    let right_title = build_right_title(profile, region);

    // 外枠Block
    let mut outer_block = Block::default().title(" S3 Buckets ").borders(Borders::ALL);
    if let Some(title) = right_title {
        outer_block = outer_block.title_top(Line::from(title).alignment(Alignment::Right));
    }
    let inner = outer_block.inner(outer_chunks[0]);
    frame.render_widget(outer_block, outer_chunks[0]);

    // メインコンテンツ
    if loading {
        let loading_widget = Loading::new("Loading buckets...", spinner_tick);
        frame.render_widget(loading_widget, inner);
    } else {
        render_table(frame, buckets, selected_index, inner);
    }

    // フッター
    render_footer(
        frame,
        outer_chunks[1],
        mode,
        filter_input.value(),
        if *can_delete {
            "j/k:move Enter:detail c:create D:delete /:filter ?:help Esc:back"
        } else {
            "j/k:move Enter:detail c:create /:filter ?:help Esc:back"
        },
    );
}

/// 右タイトル文字列を構築（profile │ region）
fn build_right_title(profile: Option<&str>, region: Option<&str>) -> Option<String> {
    let mut parts = Vec::new();

    if let Some(p) = profile {
        parts.push(p.to_string());
    }
    if let Some(r) = region {
        parts.push(r.to_string());
    }

    if parts.is_empty() {
        None
    } else {
        Some(format!(" {} ", parts.join(" │ ")))
    }
}

/// テーブルを描画
fn render_table(
    frame: &mut Frame,
    buckets: &[Bucket],
    selected_index: usize,
    area: ratatui::layout::Rect,
) {
    let headers = Row::new(vec!["Bucket Name", "Creation Date"]).style(theme::header());

    let rows: Vec<Row> = buckets.iter().map(|b| bucket_to_row(b)).collect();

    let widths = vec![Constraint::Percentage(60), Constraint::Percentage(40)];

    let table = SelectableTable::new(headers, rows, widths);
    let widget = SelectableTableWidget::new(table, selected_index);
    frame.render_widget(widget, area);
}

/// バケットをテーブル行に変換
fn bucket_to_row(bucket: &Bucket) -> Row<'_> {
    Row::new(vec![
        Line::from(bucket.name.as_str()),
        Line::from(bucket.creation_date.as_deref().unwrap_or("-")),
    ])
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

    fn create_test_bucket(name: &str, date: Option<&str>) -> Bucket {
        Bucket {
            name: name.to_string(),
            creation_date: date.map(String::from),
        }
    }

    #[test]
    fn render_returns_header_when_s3_list() {
        let buckets: Vec<Bucket> = vec![];
        let input = Input::default();
        let backend = TestBackend::new(70, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &S3ListProps {
                        buckets: &buckets,
                        selected_index: 0,
                        filter_input: &input,
                        mode: &Mode::Normal,
                        loading: false,
                        spinner_tick: 0,
                        profile: Some("dev"),
                        region: Some("ap-northeast-1"),
                        can_delete: true,
                    },
                    frame.area(),
                );
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("S3 Buckets"));
    }

    #[test]
    fn render_returns_table_headers_when_buckets_exist() {
        let buckets = vec![create_test_bucket("my-bucket", Some("2025-01-01"))];
        let input = Input::default();
        let backend = TestBackend::new(70, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &S3ListProps {
                        buckets: &buckets,
                        selected_index: 0,
                        filter_input: &input,
                        mode: &Mode::Normal,
                        loading: false,
                        spinner_tick: 0,
                        profile: None,
                        region: None,
                        can_delete: true,
                    },
                    frame.area(),
                );
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Bucket Name"));
        assert!(content.contains("Creation Date"));
    }

    #[test]
    fn render_returns_bucket_data_when_buckets_provided() {
        let buckets = vec![
            create_test_bucket("web-assets", Some("2024-06-15")),
            create_test_bucket("logs-archive", Some("2023-12-01")),
        ];
        let input = Input::default();
        let backend = TestBackend::new(70, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &S3ListProps {
                        buckets: &buckets,
                        selected_index: 0,
                        filter_input: &input,
                        mode: &Mode::Normal,
                        loading: false,
                        spinner_tick: 0,
                        profile: None,
                        region: None,
                        can_delete: true,
                    },
                    frame.area(),
                );
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("web-assets"));
        assert!(content.contains("2024-06-15"));
        assert!(content.contains("logs-archive"));
        assert!(content.contains("2023-12-01"));
    }

    #[test]
    fn render_returns_right_title_when_profile_and_region_set() {
        let buckets = vec![create_test_bucket("my-bucket", None)];
        let input = Input::default();
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &S3ListProps {
                        buckets: &buckets,
                        selected_index: 0,
                        filter_input: &input,
                        mode: &Mode::Normal,
                        loading: false,
                        spinner_tick: 0,
                        profile: Some("dev-account"),
                        region: Some("ap-northeast-1"),
                        can_delete: true,
                    },
                    frame.area(),
                );
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("dev-account"));
        assert!(content.contains("ap-northeast-1"));
        assert!(content.contains("j/k:move"));
    }

    #[test]
    fn render_returns_loading_when_loading_state() {
        let buckets: Vec<Bucket> = vec![];
        let input = Input::default();
        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &S3ListProps {
                        buckets: &buckets,
                        selected_index: 0,
                        filter_input: &input,
                        mode: &Mode::Normal,
                        loading: true,
                        spinner_tick: 0,
                        profile: None,
                        region: None,
                        can_delete: true,
                    },
                    frame.area(),
                );
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Loading buckets..."));
    }

    #[test]
    fn render_returns_filter_input_when_filter_mode() {
        let buckets = vec![create_test_bucket("my-bucket", None)];
        let input = Input::from("web");
        let backend = TestBackend::new(70, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &S3ListProps {
                        buckets: &buckets,
                        selected_index: 0,
                        filter_input: &input,
                        mode: &Mode::Filter,
                        loading: false,
                        spinner_tick: 0,
                        profile: None,
                        region: None,
                        can_delete: true,
                    },
                    frame.area(),
                );
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("/web"));
    }

    #[test]
    fn render_returns_snapshot_when_rendered() {
        let buckets = vec![
            create_test_bucket("web-assets-prod", Some("2024-01-15T10:30:00Z")),
            create_test_bucket("logs-archive", Some("2023-06-20T08:00:00Z")),
            create_test_bucket("data-pipeline", Some("2025-03-01T14:45:00Z")),
        ];
        let input = Input::default();
        let backend = TestBackend::new(80, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &S3ListProps {
                        buckets: &buckets,
                        selected_index: 0,
                        filter_input: &input,
                        mode: &Mode::Normal,
                        loading: false,
                        spinner_tick: 0,
                        profile: Some("dev-account"),
                        region: Some("ap-northeast-1"),
                        can_delete: true,
                    },
                    frame.area(),
                );
            })
            .unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }
}
