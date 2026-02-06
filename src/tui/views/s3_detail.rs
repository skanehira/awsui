use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Row};

use crate::aws::s3_model::S3Object;
use crate::tui::components::loading::Loading;
use crate::tui::components::status_bar::StatusBar;
use crate::tui::components::table::{SelectableTable, SelectableTableWidget};
use crate::tui::theme;

/// S3バケット詳細（オブジェクト一覧）画面を描画する
pub fn render(
    frame: &mut Frame,
    bucket_name: &str,
    objects: &[S3Object],
    current_prefix: &str,
    selected_index: usize,
    loading: bool,
    spinner_tick: usize,
) {
    let area = frame.area();

    // フッターは外枠の外に配置
    let outer_chunks = Layout::vertical([
        Constraint::Min(1),    // 外枠（テーブル）
        Constraint::Length(1), // フッター
    ])
    .split(area);

    // タイトル
    let title = if current_prefix.is_empty() {
        format!(" {} ", bucket_name)
    } else {
        format!(" {} / {} ", bucket_name, current_prefix)
    };

    // 外枠Block
    let outer_block = Block::default()
        .title(title)
        .title_top(Line::from(format!(" {} objects ", objects.len())).alignment(Alignment::Right))
        .borders(Borders::ALL);
    let inner = outer_block.inner(outer_chunks[0]);
    frame.render_widget(outer_block, outer_chunks[0]);

    // メインコンテンツ
    if loading {
        let loading_widget = Loading::new("Loading objects...", spinner_tick);
        frame.render_widget(loading_widget, inner);
    } else {
        render_table(frame, objects, selected_index, inner);
    }

    // ステータスバー
    let keybinds = "j/k:move Enter:open Esc:back ?:help";
    let status = StatusBar::new(keybinds);
    frame.render_widget(status, outer_chunks[1]);
}

/// テーブルを描画
fn render_table(
    frame: &mut Frame,
    objects: &[S3Object],
    selected_index: usize,
    area: ratatui::layout::Rect,
) {
    let headers =
        Row::new(vec!["Key", "Size", "Last Modified", "Storage Class"]).style(theme::header());

    let rows: Vec<Row> = objects.iter().map(|obj| object_to_row(obj)).collect();

    let widths = vec![
        Constraint::Percentage(40),
        Constraint::Length(12),
        Constraint::Length(22),
        Constraint::Min(14),
    ];

    let table = SelectableTable::new(headers, rows, widths);
    let widget = SelectableTableWidget::new(table, selected_index);
    frame.render_widget(widget, area);
}

/// S3Objectをテーブル行に変換
fn object_to_row(obj: &S3Object) -> Row<'_> {
    let icon = if obj.is_prefix {
        "\u{1F4C1} "
    } else {
        "\u{1F4C4} "
    };
    let key_display = format!("{}{}", icon, obj.key);

    let size_display = if obj.is_prefix {
        "-".to_string()
    } else {
        obj.size.map(format_size).unwrap_or_else(|| "-".to_string())
    };

    let last_modified = obj.last_modified.as_deref().unwrap_or("-");
    let storage_class = obj.storage_class.as_deref().unwrap_or("-");

    Row::new(vec![
        Line::from(key_display),
        Line::from(size_display),
        Line::from(last_modified.to_string()),
        Line::from(storage_class.to_string()),
    ])
}

/// バイトサイズを人間が読みやすい形式に変換
fn format_size(bytes: i64) -> String {
    if bytes < 0 {
        return "-".to_string();
    }
    let bytes = bytes as u64;
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    const TB: u64 = 1024 * GB;

    if bytes >= TB {
        format!("{:.1} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
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

    fn create_test_object(key: &str, size: Option<i64>, is_prefix: bool) -> S3Object {
        S3Object {
            key: key.to_string(),
            size,
            last_modified: Some("2025-01-15T10:30:00Z".to_string()),
            storage_class: Some("STANDARD".to_string()),
            is_prefix,
        }
    }

    #[test]
    fn render_returns_bucket_name_when_no_prefix() {
        let objects: Vec<S3Object> = vec![];
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, "my-bucket", &objects, "", 0, false, 0))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("my-bucket"));
    }

    #[test]
    fn render_returns_bucket_name_with_prefix_when_prefix_set() {
        let objects: Vec<S3Object> = vec![];
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, "my-bucket", &objects, "logs/2025/", 0, false, 0))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("my-bucket"));
        assert!(content.contains("logs/2025/"));
    }

    #[test]
    fn render_returns_table_headers_when_objects_exist() {
        let objects = vec![create_test_object("file.txt", Some(1024), false)];
        let backend = TestBackend::new(90, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, "my-bucket", &objects, "", 0, false, 0))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Key"));
        assert!(content.contains("Size"));
        assert!(content.contains("Last Modified"));
        assert!(content.contains("Storage Class"));
    }

    #[test]
    fn render_returns_object_data_when_objects_provided() {
        let objects = vec![
            create_test_object("data/", None, true),
            create_test_object("readme.md", Some(2048), false),
        ];
        let backend = TestBackend::new(90, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, "my-bucket", &objects, "", 0, false, 0))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("data/"));
        assert!(content.contains("readme.md"));
        assert!(content.contains("STANDARD"));
    }

    #[test]
    fn render_returns_loading_when_loading_state() {
        let objects: Vec<S3Object> = vec![];
        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, "my-bucket", &objects, "", 0, true, 0))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Loading objects..."));
    }

    #[test]
    fn render_returns_keybinds_when_normal() {
        let objects: Vec<S3Object> = vec![];
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, "my-bucket", &objects, "", 0, false, 0))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("j/k:move"));
        assert!(content.contains("Enter:open"));
        assert!(content.contains("Esc:back"));
    }

    #[test]
    fn format_size_returns_bytes_when_small() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1023), "1023 B");
    }

    #[test]
    fn format_size_returns_kb_when_kilobyte_range() {
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
    }

    #[test]
    fn format_size_returns_mb_when_megabyte_range() {
        assert_eq!(format_size(1048576), "1.0 MB");
        assert_eq!(format_size(5242880), "5.0 MB");
    }

    #[test]
    fn format_size_returns_gb_when_gigabyte_range() {
        assert_eq!(format_size(1073741824), "1.0 GB");
    }

    #[test]
    fn format_size_returns_tb_when_terabyte_range() {
        assert_eq!(format_size(1099511627776), "1.0 TB");
    }

    #[test]
    fn format_size_returns_dash_when_negative() {
        assert_eq!(format_size(-1), "-");
    }

    #[test]
    fn render_returns_snapshot_when_rendered() {
        let objects = vec![
            create_test_object("images/", None, true),
            create_test_object("logs/", None, true),
            create_test_object("config.json", Some(256), false),
            create_test_object("data.csv", Some(1048576), false),
            create_test_object("backup.tar.gz", Some(5368709120), false),
        ];
        let backend = TestBackend::new(90, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, "my-prod-bucket", &objects, "", 0, false, 0))
            .unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }

    #[test]
    fn render_returns_snapshot_with_prefix_when_inside_directory() {
        let objects = vec![
            create_test_object("sub/", None, true),
            create_test_object("file1.log", Some(4096), false),
            create_test_object("file2.log", Some(8192), false),
        ];
        let backend = TestBackend::new(90, 12);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, "my-prod-bucket", &objects, "logs/2025/", 0, false, 0))
            .unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }
}
