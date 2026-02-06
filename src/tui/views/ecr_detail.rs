use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Wrap};

use crate::aws::ecr_model::{Image, Repository};
use crate::tui::components::loading::Loading;
use crate::tui::components::status_bar::StatusBar;
use crate::tui::components::table::{SelectableTable, SelectableTableWidget};
use crate::tui::theme;

/// ECRリポジトリ詳細画面を描画する（イメージ一覧）
#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    repository: &Repository,
    images: &[Image],
    selected_index: usize,
    loading: bool,
    spinner_tick: usize,
    profile: Option<&str>,
    region: Option<&str>,
) {
    let area = frame.area();

    let outer_chunks = Layout::vertical([
        Constraint::Min(1),    // 外枠（リポジトリ情報 + イメージテーブル）
        Constraint::Length(1), // ステータスバー
    ])
    .split(area);

    // 外枠Block
    let title = format!(" {} ", repository.repository_name);
    let right_title = build_right_title(profile, region);

    let mut outer_block = Block::default().title(title).borders(Borders::ALL);
    if let Some(rt) = right_title {
        outer_block = outer_block.title_top(Line::from(rt).alignment(Alignment::Right));
    }
    let inner = outer_block.inner(outer_chunks[0]);
    frame.render_widget(outer_block, outer_chunks[0]);

    // 内側レイアウト: リポジトリ情報 + イメージテーブル
    let inner_chunks = Layout::vertical([
        Constraint::Length(4), // リポジトリ情報
        Constraint::Min(1),    // イメージテーブル
    ])
    .split(inner);

    // リポジトリ情報
    render_repository_info(frame, repository, inner_chunks[0]);

    // イメージテーブル
    if loading {
        let loading_widget = Loading::new("Loading images...", spinner_tick);
        frame.render_widget(loading_widget, inner_chunks[1]);
    } else {
        render_image_table(frame, images, selected_index, inner_chunks[1]);
    }

    // ステータスバー
    let keybinds = "j/k:move y:copy-digest Esc:back ?:help";
    let status = StatusBar::new(keybinds);
    frame.render_widget(status, outer_chunks[1]);
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

/// リポジトリ情報を描画
fn render_repository_info(frame: &mut Frame, repository: &Repository, area: Rect) {
    let info_block = Block::default()
        .title(" Repository Info ")
        .borders(Borders::ALL);

    let lines = vec![
        detail_line("URI", &repository.repository_uri),
        detail_line("Registry", &repository.registry_id),
    ];

    let para = Paragraph::new(lines)
        .block(info_block)
        .wrap(Wrap { trim: false });
    frame.render_widget(para, area);
}

/// イメージテーブルを描画
fn render_image_table(frame: &mut Frame, images: &[Image], selected_index: usize, area: Rect) {
    let headers = Row::new(vec!["Digest", "Tags", "Size", "Pushed At"]).style(theme::header());

    let rows: Vec<Row> = images.iter().map(|image| image_to_row(image)).collect();

    let widths = vec![
        Constraint::Length(25),
        Constraint::Min(20),
        Constraint::Length(12),
        Constraint::Length(22),
    ];

    let table = SelectableTable::new(headers, rows, widths);
    let widget = SelectableTableWidget::new(table, selected_index);
    frame.render_widget(widget, area);
}

/// イメージをテーブル行に変換
fn image_to_row(image: &Image) -> Row<'_> {
    let tags_text = if image.image_tags.is_empty() {
        "<untagged>".to_string()
    } else {
        image.image_tags.join(", ")
    };

    let size_text = match image.image_size_bytes {
        Some(bytes) => format_size(bytes),
        None => "-".to_string(),
    };

    Row::new(vec![
        Line::from(truncate_digest(&image.image_digest)),
        Line::from(tags_text),
        Line::from(size_text),
        Line::from(image.pushed_at.as_deref().unwrap_or("-")),
    ])
}

/// ダイジェストを短縮表示
fn truncate_digest(digest: &str) -> String {
    if let Some(hash) = digest.strip_prefix("sha256:") {
        if hash.len() > 12 {
            format!("sha256:{}...", &hash[..12])
        } else {
            digest.to_string()
        }
    } else {
        digest.to_string()
    }
}

/// バイト数を人間が読みやすい形式に変換
fn format_size(bytes: i64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let bytes_f = bytes as f64;
    if bytes_f >= GB {
        format!("{:.1} GB", bytes_f / GB)
    } else if bytes_f >= MB {
        format!("{:.1} MB", bytes_f / MB)
    } else if bytes_f >= KB {
        format!("{:.1} KB", bytes_f / KB)
    } else {
        format!("{} B", bytes)
    }
}

/// 詳細画面の1行を生成
fn detail_line<'a>(label: &'a str, value: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("  {:<12}", label), theme::header()),
        Span::raw(value),
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

    fn create_test_repository() -> Repository {
        Repository {
            repository_name: "my-app".to_string(),
            repository_uri: "123456789012.dkr.ecr.ap-northeast-1.amazonaws.com/my-app".to_string(),
            registry_id: "123456789012".to_string(),
            created_at: Some("2025-01-10T09:30:00Z".to_string()),
            image_tag_mutability: "MUTABLE".to_string(),
        }
    }

    fn create_test_image(digest: &str, tags: Vec<&str>, size: Option<i64>) -> Image {
        Image {
            image_digest: digest.to_string(),
            image_tags: tags.into_iter().map(String::from).collect(),
            pushed_at: Some("2025-01-15T12:00:00Z".to_string()),
            image_size_bytes: size,
        }
    }

    #[test]
    fn render_returns_repository_name_when_rendered() {
        let repo = create_test_repository();
        let images: Vec<Image> = vec![];
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &repo,
                    &images,
                    0,
                    false,
                    0,
                    Some("dev-account"),
                    Some("ap-northeast-1"),
                );
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("my-app"));
    }

    #[test]
    fn render_returns_repository_info_when_rendered() {
        let repo = create_test_repository();
        let images: Vec<Image> = vec![];
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &repo, &images, 0, false, 0, None, None))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Repository Info"));
        assert!(content.contains("123456789012"));
    }

    #[test]
    fn render_returns_image_table_headers_when_images_exist() {
        let repo = create_test_repository();
        let images = vec![create_test_image(
            "sha256:abc123def456789",
            vec!["latest"],
            Some(52428800),
        )];
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &repo, &images, 0, false, 0, None, None))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Digest"));
        assert!(content.contains("Tags"));
        assert!(content.contains("Size"));
        assert!(content.contains("Pushed At"));
    }

    #[test]
    fn render_returns_image_data_when_images_provided() {
        let repo = create_test_repository();
        let images = vec![
            create_test_image(
                "sha256:abc123def456789",
                vec!["latest", "v1.0"],
                Some(52428800),
            ),
            create_test_image("sha256:def789abc123456", vec!["v0.9"], Some(10485760)),
        ];
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &repo, &images, 0, false, 0, None, None))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("sha256:abc123def456.."));
        assert!(content.contains("latest"));
        assert!(content.contains("50.0 MB"));
    }

    #[test]
    fn render_returns_loading_when_loading_state() {
        let repo = create_test_repository();
        let images: Vec<Image> = vec![];
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &repo, &images, 0, true, 0, None, None))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("Loading images..."));
    }

    #[test]
    fn render_returns_keybinds_when_rendered() {
        let repo = create_test_repository();
        let images: Vec<Image> = vec![];
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &repo, &images, 0, false, 0, None, None))
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("j/k:move"));
        assert!(content.contains("Esc:back"));
    }

    #[test]
    fn render_returns_right_title_when_profile_and_region_set() {
        let repo = create_test_repository();
        let images: Vec<Image> = vec![];
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &repo,
                    &images,
                    0,
                    false,
                    0,
                    Some("dev-account"),
                    Some("ap-northeast-1"),
                );
            })
            .unwrap();

        let content = buffer_to_string(&terminal);
        assert!(content.contains("dev-account"));
        assert!(content.contains("ap-northeast-1"));
    }

    #[test]
    fn truncate_digest_returns_truncated_when_long_hash() {
        let result = truncate_digest("sha256:abc123def456789012345678");
        assert_eq!(result, "sha256:abc123def456...");
    }

    #[test]
    fn truncate_digest_returns_full_when_short_hash() {
        let result = truncate_digest("sha256:abc123");
        assert_eq!(result, "sha256:abc123");
    }

    #[test]
    fn truncate_digest_returns_original_when_no_prefix() {
        let result = truncate_digest("noprefix");
        assert_eq!(result, "noprefix");
    }

    #[test]
    fn format_size_returns_bytes_when_small() {
        assert_eq!(format_size(500), "500 B");
    }

    #[test]
    fn format_size_returns_kb_when_kilobytes() {
        assert_eq!(format_size(2048), "2.0 KB");
    }

    #[test]
    fn format_size_returns_mb_when_megabytes() {
        assert_eq!(format_size(52428800), "50.0 MB");
    }

    #[test]
    fn format_size_returns_gb_when_gigabytes() {
        assert_eq!(format_size(1073741824), "1.0 GB");
    }

    #[test]
    fn render_returns_snapshot_when_rendered() {
        let repo = create_test_repository();
        let images = vec![
            create_test_image(
                "sha256:abc123def456789",
                vec!["latest", "v1.0"],
                Some(52428800),
            ),
            create_test_image("sha256:def789abc123456", vec!["v0.9"], Some(10485760)),
            create_test_image("sha256:ghi012jkl345678", vec![], Some(1048576)),
        ];
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &repo,
                    &images,
                    0,
                    false,
                    0,
                    Some("dev-account"),
                    Some("ap-northeast-1"),
                );
            })
            .unwrap();

        insta::assert_snapshot!(buffer_to_string(&terminal));
    }
}
