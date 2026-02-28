use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Row};
use tui_input::Input;

use crate::app::Mode;
use crate::aws::ecr_model::Repository;
use crate::tui::components::loading::Loading;
use crate::tui::components::status_bar::render_footer;
use crate::tui::components::table::{SelectableTable, SelectableTableWidget};
use crate::tui::theme;

/// ECRリポジトリ一覧画面の描画に必要なプロパティ
pub struct EcrListProps<'a> {
    pub repositories: &'a [Repository],
    pub selected_index: usize,
    pub filter_input: &'a Input,
    pub mode: &'a Mode,
    pub loading: bool,
    pub spinner_tick: usize,
    pub profile: Option<&'a str>,
    pub region: Option<&'a str>,
    pub can_delete: bool,
}

/// ECRリポジトリ一覧画面を描画する
pub fn render(frame: &mut Frame, props: &EcrListProps, area: Rect) {
    let EcrListProps {
        repositories,
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
    let mut outer_block = Block::default()
        .title(" ECR Repositories ")
        .borders(Borders::ALL);
    if let Some(title) = right_title {
        outer_block = outer_block.title_top(Line::from(title).alignment(Alignment::Right));
    }
    let inner = outer_block.inner(outer_chunks[0]);
    frame.render_widget(outer_block, outer_chunks[0]);

    // メインコンテンツ
    if loading {
        let loading_widget = Loading::new("Loading repositories...", spinner_tick);
        frame.render_widget(loading_widget, inner);
    } else {
        render_table(frame, repositories, selected_index, inner);
    }

    // フッター
    render_footer(
        frame,
        outer_chunks[1],
        mode,
        filter_input.value(),
        if *can_delete {
            "j/k:move Enter:images c:create D:delete /:filter ?:help Esc:back"
        } else {
            "j/k:move Enter:images c:create /:filter ?:help Esc:back"
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
    repositories: &[Repository],
    selected_index: usize,
    area: ratatui::layout::Rect,
) {
    let headers = Row::new(vec!["Repository Name", "URI", "Tag Mutability", "Created"])
        .style(theme::header());

    let rows: Vec<Row> = repositories
        .iter()
        .map(|repo| repository_to_row(repo))
        .collect();

    let widths = vec![
        Constraint::Length(25),
        Constraint::Min(30),
        Constraint::Length(16),
        Constraint::Length(22),
    ];

    let table = SelectableTable::new(headers, rows, widths);
    let widget = SelectableTableWidget::new(table, selected_index);
    frame.render_widget(widget, area);
}

/// リポジトリをテーブル行に変換
fn repository_to_row(repo: &Repository) -> Row<'_> {
    Row::new(vec![
        Line::from(repo.repository_name.as_str()),
        Line::from(repo.repository_uri.as_str()),
        Line::from(repo.image_tag_mutability.as_str()),
        Line::from(repo.created_at.as_deref().unwrap_or("-")),
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

    fn create_test_repository(name: &str, uri: &str) -> Repository {
        Repository {
            repository_name: name.to_string(),
            repository_uri: uri.to_string(),
            registry_id: "123456789012".to_string(),
            created_at: Some("2025-01-10T09:30:00Z".to_string()),
            image_tag_mutability: "MUTABLE".to_string(),
        }
    }

    #[test]
    fn render_returns_header_when_ecr_list() {
        let repos: Vec<Repository> = vec![];
        let input = Input::default();
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &EcrListProps {
                        repositories: &repos,
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
        assert!(content.contains("ECR Repositories"));
    }

    #[test]
    fn render_returns_table_headers_when_repositories_exist() {
        let repos = vec![create_test_repository(
            "my-app",
            "123456789012.dkr.ecr.ap-northeast-1.amazonaws.com/my-app",
        )];
        let input = Input::default();
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &EcrListProps {
                        repositories: &repos,
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
        assert!(content.contains("Repository Name"));
        assert!(content.contains("URI"));
        assert!(content.contains("Tag Mutability"));
        assert!(content.contains("Created"));
    }

    #[test]
    fn render_returns_repository_data_when_repositories_provided() {
        let repos = vec![
            create_test_repository(
                "my-app",
                "123456789012.dkr.ecr.ap-northeast-1.amazonaws.com/my-app",
            ),
            create_test_repository(
                "api-server",
                "123456789012.dkr.ecr.ap-northeast-1.amazonaws.com/api-server",
            ),
        ];
        let input = Input::default();
        let backend = TestBackend::new(120, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &EcrListProps {
                        repositories: &repos,
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
        assert!(content.contains("my-app"));
        assert!(content.contains("api-server"));
        assert!(content.contains("MUTABLE"));
    }

    #[test]
    fn render_returns_right_title_when_profile_and_region_set() {
        let repos = vec![create_test_repository("my-app", "uri")];
        let input = Input::default();
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &EcrListProps {
                        repositories: &repos,
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
    }

    #[test]
    fn render_returns_loading_when_loading_state() {
        let repos: Vec<Repository> = vec![];
        let input = Input::default();
        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &EcrListProps {
                        repositories: &repos,
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
        assert!(content.contains("Loading repositories..."));
    }

    #[test]
    fn render_returns_filter_input_when_filter_mode() {
        let repos = vec![create_test_repository("my-app", "uri")];
        let input = Input::from("my-app");
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &EcrListProps {
                        repositories: &repos,
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
        assert!(content.contains("/my-app"));
    }

    #[test]
    fn render_returns_keybinds_when_normal_mode() {
        let repos: Vec<Repository> = vec![];
        let input = Input::default();
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &EcrListProps {
                        repositories: &repos,
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
        assert!(content.contains("j/k:move"));
    }

    #[test]
    fn render_returns_snapshot_when_rendered() {
        let repos = vec![
            create_test_repository(
                "my-app",
                "123456789012.dkr.ecr.ap-northeast-1.amazonaws.com/my-app",
            ),
            create_test_repository(
                "api-server",
                "123456789012.dkr.ecr.ap-northeast-1.amazonaws.com/api-server",
            ),
            create_test_repository(
                "batch-job",
                "123456789012.dkr.ecr.ap-northeast-1.amazonaws.com/batch-job",
            ),
        ];
        let input = Input::default();
        let backend = TestBackend::new(110, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &EcrListProps {
                        repositories: &repos,
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
