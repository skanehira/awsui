use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::service::ServiceKind;
use crate::tab::TabView;
use crate::tui::components::dialog::centered_rect;
use crate::tui::theme;

/// ヘルプポップアップWidget
pub struct HelpPopup {
    view: (ServiceKind, TabView),
    can_delete: bool,
}

impl HelpPopup {
    pub fn new(view: (ServiceKind, TabView), can_delete: bool) -> Self {
        Self { view, can_delete }
    }

    fn action_lines(&self) -> Vec<Line<'static>> {
        let mut lines = match self.view {
            (ServiceKind::Ec2, TabView::List) => vec![
                Line::from("    S          Start/Stop instance"),
                Line::from("    R          Reboot instance"),
                Line::from("    s          SSM Connect"),
                Line::from("    r          Refresh list"),
                Line::from("    y          Copy ID"),
                Line::from("    /          Filter"),
            ],
            (ServiceKind::Ec2, TabView::Detail) => vec![
                Line::from("    S          Start/Stop instance"),
                Line::from("    R          Reboot instance"),
                Line::from("    s          SSM Connect"),
                Line::from("    y          Copy ID"),
                Line::from("    [/]        Switch tab"),
                Line::from("    Enter      Follow link"),
            ],
            (ServiceKind::S3, TabView::List) => vec![
                Line::from("    c          Create bucket"),
                Line::from("    r          Refresh list"),
                Line::from("    y          Copy ID"),
                Line::from("    /          Filter"),
            ],
            (ServiceKind::S3, TabView::Detail) => vec![
                Line::from("    Enter      Open directory"),
                Line::from("    d          Download object"),
                Line::from("    u          Upload object"),
                Line::from("    [/]        Switch tab"),
            ],
            (ServiceKind::SecretsManager, TabView::List) => vec![
                Line::from("    c          Create secret"),
                Line::from("    r          Refresh list"),
                Line::from("    y          Copy ID"),
                Line::from("    /          Filter"),
            ],
            (ServiceKind::SecretsManager, TabView::Detail) => vec![
                Line::from("    e          Edit secret value"),
                Line::from("    v          Show/hide value"),
                Line::from("    y          Copy ID"),
                Line::from("    [/]        Switch tab"),
            ],
            (ServiceKind::Ecr, TabView::List) => vec![
                Line::from("    c          Create repository"),
                Line::from("    r          Refresh list"),
                Line::from("    y          Copy ID"),
                Line::from("    /          Filter"),
            ],
            (ServiceKind::Ecs, TabView::List) | (ServiceKind::Vpc, TabView::List) => vec![
                Line::from("    r          Refresh list"),
                Line::from("    y          Copy ID"),
                Line::from("    /          Filter"),
            ],
            (ServiceKind::Ecr, TabView::Detail) => vec![
                Line::from("    y          Copy digest"),
                Line::from("    [/]        Switch tab"),
            ],
            (ServiceKind::Ecs, TabView::Detail) => vec![
                Line::from("    d          Force deploy"),
                Line::from("    s          Scale service"),
                Line::from("    x          Stop task"),
                Line::from("    l          Show logs"),
                Line::from("    a          Exec into container"),
                Line::from("    y          Copy ID"),
                Line::from("    [/]        Switch tab"),
                Line::from("    Enter      Open detail"),
            ],
            (ServiceKind::Vpc, TabView::Detail) => {
                vec![Line::from("    y          Copy ID")]
            }
        };

        // 削除許可がある場合のみ削除キーを表示
        if self.can_delete {
            let delete_line = match self.view {
                (ServiceKind::Ec2, TabView::List) => Some("    D          Terminate instance"),
                (ServiceKind::S3, TabView::List) => Some("    D          Delete bucket"),
                (ServiceKind::S3, TabView::Detail) => Some("    D          Delete object"),
                (ServiceKind::SecretsManager, TabView::List) => {
                    Some("    D          Delete secret")
                }
                (ServiceKind::Ecr, TabView::List) => Some("    D          Delete repository"),
                (ServiceKind::Ecr, TabView::Detail) => Some("    D          Delete image"),
                _ => None,
            };
            if let Some(line) = delete_line {
                lines.push(Line::from(line));
            }
        }

        lines
    }
}

impl Widget for HelpPopup {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let action_lines = self.action_lines();
        // Navigation(7) + Actions(2+N) + Tabs(6) + General(6) + borders(2)
        let height = 23 + action_lines.len() as u16;
        let popup = centered_rect(70, height, area);
        Clear.render(popup, buf);

        let block = Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .style(theme::active());

        let inner = block.inner(popup);
        block.render(popup, buf);

        let mut sections: Vec<Line<'_>> = vec![
            Line::from(""),
            Line::from(Span::styled("  Navigation", theme::header())),
            Line::from("    j/k        Move down/up"),
            Line::from("    g/G        Go to first/last"),
            Line::from("    Ctrl+d/u   Half page down/up"),
            Line::from("    Enter      Open detail"),
            Line::from("    Esc        Go back"),
        ];

        if !action_lines.is_empty() {
            sections.push(Line::from(""));
            sections.push(Line::from(Span::styled("  Actions", theme::header())));
            sections.extend(action_lines);
        }

        sections.push(Line::from(""));
        sections.push(Line::from(Span::styled("  Tabs", theme::header())));
        sections.push(Line::from("    Tab        Next tab"));
        sections.push(Line::from("    Shift+Tab  Previous tab"));
        sections.push(Line::from("    Ctrl+t     New tab"));
        sections.push(Line::from("    Ctrl+w     Close tab"));

        sections.push(Line::from(""));
        sections.push(Line::from(Span::styled("  General", theme::header())));
        sections.push(Line::from("    ?          Show this help"));
        sections.push(Line::from("    q          Quit"));
        sections.push(Line::from(""));
        sections.push(Line::from("  Press Esc to close"));

        let help_text = Paragraph::new(sections);
        help_text.render(inner, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::ServiceKind;
    use crate::tab::TabView;

    fn render_help(view: (ServiceKind, TabView)) -> String {
        let area = Rect::new(0, 0, 80, 40);
        let mut buf = Buffer::empty(area);
        HelpPopup::new(view, true).render(area, &mut buf);

        let mut result = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                result.push_str(buf[(x, y)].symbol());
            }
            result.push('\n');
        }
        result
    }

    #[test]
    fn help_popup_render_returns_navigation_section_when_rendered() {
        let content = render_help((ServiceKind::Ec2, TabView::List));
        assert!(content.contains("Navigation"));
        assert!(content.contains("j/k"));
        assert!(content.contains("Move down/up"));
    }

    #[test]
    fn help_popup_render_returns_general_section_when_rendered() {
        let content = render_help((ServiceKind::Ec2, TabView::List));
        assert!(content.contains("General"));
        assert!(content.contains("Quit"));
        assert!(content.contains("Press Esc to close"));
    }

    // EC2 List: S(Start/Stop), R(Reboot), s(SSM Connect), D(Terminate), r, y, /
    #[test]
    fn help_popup_render_returns_ec2_actions_when_ec2_list() {
        let content = render_help((ServiceKind::Ec2, TabView::List));
        assert!(content.contains("Start/Stop"));
        assert!(content.contains("Reboot"));
        assert!(content.contains("SSM Connect"));
        assert!(content.contains("Terminate"));
        assert!(content.contains("Refresh"));
        assert!(content.contains("Copy"));
        assert!(content.contains("Filter"));
    }

    // EC2 Detail: S(Start/Stop), R(Reboot), s(SSM Connect), y, ](Switch tab), Enter(follow-link)
    #[test]
    fn help_popup_render_returns_ec2_detail_actions_when_ec2_detail() {
        let content = render_help((ServiceKind::Ec2, TabView::Detail));
        assert!(content.contains("Start/Stop"));
        assert!(content.contains("Reboot"));
        assert!(content.contains("SSM Connect"));
        assert!(content.contains("Copy"));
        assert!(content.contains("]"));
        assert!(content.contains("Follow link"));
        // リストのみのアクションは表示しない
        assert!(!content.contains("Filter"));
        assert!(!content.contains("Refresh"));
    }

    // S3 List: c(Create), D(Delete), r, y, /
    #[test]
    fn help_popup_render_returns_s3_actions_when_s3_list() {
        let content = render_help((ServiceKind::S3, TabView::List));
        assert!(content.contains("Create"));
        assert!(content.contains("Delete"));
        assert!(content.contains("Refresh"));
        assert!(content.contains("Filter"));
        // EC2固有は表示しない
        assert!(!content.contains("Start/Stop"));
        assert!(!content.contains("Reboot"));
    }

    // Secrets List: c(Create), D(Delete), r, y, /
    #[test]
    fn help_popup_render_returns_secrets_actions_when_secrets_list() {
        let content = render_help((ServiceKind::SecretsManager, TabView::List));
        assert!(content.contains("Create"));
        assert!(content.contains("Delete"));
        assert!(content.contains("Refresh"));
        assert!(content.contains("Filter"));
        assert!(!content.contains("Start/Stop"));
        assert!(!content.contains("Reboot"));
    }

    // Secrets Detail: e(Edit), v(Show/hide), y, [/](Switch tab)
    #[test]
    fn help_popup_render_returns_secrets_detail_actions_when_secrets_detail() {
        let content = render_help((ServiceKind::SecretsManager, TabView::Detail));
        assert!(content.contains("Edit"));
        assert!(content.contains("Show/hide"));
        assert!(content.contains("Copy"));
        assert!(content.contains("[/]"));
        assert!(!content.contains("Start/Stop"));
        assert!(!content.contains("Filter"));
    }

    // ECR List: c(Create), D(Delete), r, y, /
    #[test]
    fn help_popup_render_returns_ecr_actions_when_ecr_list() {
        let content = render_help((ServiceKind::Ecr, TabView::List));
        assert!(content.contains("Create"));
        assert!(content.contains("Delete"));
        assert!(content.contains("Refresh"));
        assert!(content.contains("Copy"));
        assert!(content.contains("Filter"));
        assert!(!content.contains("Start/Stop"));
        assert!(!content.contains("Reboot"));
    }

    // Generic List (ECS, VPC): r, y, /のみ
    #[test]
    fn help_popup_render_returns_generic_actions_when_ecs_list() {
        let content = render_help((ServiceKind::Ecs, TabView::List));
        assert!(content.contains("Refresh"));
        assert!(content.contains("Copy"));
        assert!(content.contains("Filter"));
        assert!(!content.contains("Start/Stop"));
        assert!(!content.contains("Reboot"));
        assert!(!content.contains("Create"));
        assert!(!content.contains("Delete"));
    }

    // S3 Detail: Enter(open dir), d(Download), u(Upload), D(Delete), [/](Switch tab)
    #[test]
    fn help_popup_render_returns_s3_detail_actions_when_s3_detail() {
        let content = render_help((ServiceKind::S3, TabView::Detail));
        assert!(content.contains("Open"));
        assert!(content.contains("Download"));
        assert!(content.contains("Upload"));
        assert!(content.contains("Delete"));
        assert!(content.contains("[/]"));
        assert!(!content.contains("Start/Stop"));
        assert!(!content.contains("Filter"));
    }

    // ECR Detail: D(Delete), y(Copy digest), [/](Switch tab)
    #[test]
    fn help_popup_render_returns_ecr_detail_actions_when_ecr_detail() {
        let content = render_help((ServiceKind::Ecr, TabView::Detail));
        assert!(content.contains("Delete"));
        assert!(content.contains("Copy digest"));
        assert!(content.contains("[/]"));
        assert!(!content.contains("Start/Stop"));
        assert!(!content.contains("Filter"));
        assert!(!content.contains("Create"));
    }

    // ECS Detail: d(Force deploy), s(Scale), x(Stop task), l(Logs), a(Exec), y, [/], Enter
    #[test]
    fn help_popup_render_returns_ecs_detail_actions_when_ecs_detail() {
        let content = render_help((ServiceKind::Ecs, TabView::Detail));
        assert!(content.contains("Force deploy"));
        assert!(content.contains("Scale service"));
        assert!(content.contains("Stop task"));
        assert!(content.contains("Show logs"));
        assert!(content.contains("Exec into container"));
        assert!(content.contains("Copy ID"));
        assert!(content.contains("[/]"));
        assert!(content.contains("Open detail"));
        assert!(!content.contains("Start/Stop"));
        assert!(!content.contains("Filter"));
    }

    // VPC Detail: y のみ
    #[test]
    fn help_popup_render_returns_vpc_detail_actions_when_vpc_detail() {
        let content = render_help((ServiceKind::Vpc, TabView::Detail));
        assert!(content.contains("Copy ID"));
        assert!(!content.contains("Start/Stop"));
        assert!(!content.contains("Delete"));
    }
}
