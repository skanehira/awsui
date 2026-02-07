use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::app::View;
use crate::tui::components::dialog::centered_rect;
use crate::tui::theme;

/// ヘルプポップアップWidget
pub struct HelpPopup<'a> {
    view: &'a View,
}

impl<'a> HelpPopup<'a> {
    pub fn new(view: &'a View) -> Self {
        Self { view }
    }

    fn action_lines(&self) -> Vec<Line<'a>> {
        match self.view {
            View::Ec2List => vec![
                Line::from("    S          Start/Stop instance"),
                Line::from("    R          Reboot instance"),
                Line::from("    D          Terminate instance"),
                Line::from("    r          Refresh list"),
                Line::from("    y          Copy ID"),
                Line::from("    /          Filter"),
            ],
            View::Ec2Detail => vec![
                Line::from("    S          Start/Stop instance"),
                Line::from("    R          Reboot instance"),
                Line::from("    y          Copy ID"),
                Line::from("    Tab        Switch tab"),
                Line::from("    Enter      Follow link"),
            ],
            View::S3List => vec![
                Line::from("    c          Create bucket"),
                Line::from("    D          Delete bucket"),
                Line::from("    r          Refresh list"),
                Line::from("    y          Copy ID"),
                Line::from("    /          Filter"),
            ],
            View::S3Detail => vec![
                Line::from("    Enter      Open directory"),
                Line::from("    D          Delete object"),
            ],
            View::SecretsList => vec![
                Line::from("    c          Create secret"),
                Line::from("    D          Delete secret"),
                Line::from("    r          Refresh list"),
                Line::from("    y          Copy ID"),
                Line::from("    /          Filter"),
            ],
            View::SecretsDetail => vec![
                Line::from("    e          Edit secret value"),
                Line::from("    y          Copy ID"),
                Line::from("    Tab        Switch tab"),
            ],
            View::EcrList | View::EcsList | View::VpcList => vec![
                Line::from("    r          Refresh list"),
                Line::from("    y          Copy ID"),
                Line::from("    /          Filter"),
            ],
            View::EcrDetail | View::EcsDetail | View::VpcDetail => {
                vec![Line::from("    y          Copy ID")]
            }
            View::ProfileSelect | View::ServiceSelect => vec![],
        }
    }
}

impl<'a> Widget for HelpPopup<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let action_lines = self.action_lines();
        // Navigation(7) + Actions(2+N) + General(6) + borders(2)
        let height = 17 + action_lines.len() as u16;
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
    use crate::app::View;

    fn render_help(view: &View) -> String {
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        HelpPopup::new(view).render(area, &mut buf);

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
        let content = render_help(&View::Ec2List);
        assert!(content.contains("Navigation"));
        assert!(content.contains("j/k"));
        assert!(content.contains("Move down/up"));
    }

    #[test]
    fn help_popup_render_returns_general_section_when_rendered() {
        let content = render_help(&View::Ec2List);
        assert!(content.contains("General"));
        assert!(content.contains("Quit"));
        assert!(content.contains("Press Esc to close"));
    }

    // EC2 List: S(Start/Stop), R(Reboot), D(Terminate), r, y, /
    #[test]
    fn help_popup_render_returns_ec2_actions_when_ec2_list() {
        let content = render_help(&View::Ec2List);
        assert!(content.contains("Start/Stop"));
        assert!(content.contains("Reboot"));
        assert!(content.contains("Terminate"));
        assert!(content.contains("Refresh"));
        assert!(content.contains("Copy"));
        assert!(content.contains("Filter"));
    }

    // EC2 Detail: S(Start/Stop), R(Reboot), y, Tab, Enter(follow-link)
    #[test]
    fn help_popup_render_returns_ec2_detail_actions_when_ec2_detail() {
        let content = render_help(&View::Ec2Detail);
        assert!(content.contains("Start/Stop"));
        assert!(content.contains("Reboot"));
        assert!(content.contains("Copy"));
        assert!(content.contains("Tab"));
        assert!(content.contains("Follow link"));
        // リストのみのアクションは表示しない
        assert!(!content.contains("Filter"));
        assert!(!content.contains("Refresh"));
    }

    // S3 List: c(Create), D(Delete), r, y, /
    #[test]
    fn help_popup_render_returns_s3_actions_when_s3_list() {
        let content = render_help(&View::S3List);
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
        let content = render_help(&View::SecretsList);
        assert!(content.contains("Create"));
        assert!(content.contains("Delete"));
        assert!(content.contains("Refresh"));
        assert!(content.contains("Filter"));
        assert!(!content.contains("Start/Stop"));
        assert!(!content.contains("Reboot"));
    }

    // Secrets Detail: e(Edit), y, Tab
    #[test]
    fn help_popup_render_returns_secrets_detail_actions_when_secrets_detail() {
        let content = render_help(&View::SecretsDetail);
        assert!(content.contains("Edit"));
        assert!(content.contains("Copy"));
        assert!(content.contains("Tab"));
        assert!(!content.contains("Start/Stop"));
        assert!(!content.contains("Filter"));
    }

    // Generic List (ECR, ECS, VPC): r, y, /のみ
    #[test]
    fn help_popup_render_returns_generic_actions_when_ecr_list() {
        let content = render_help(&View::EcrList);
        assert!(content.contains("Refresh"));
        assert!(content.contains("Copy"));
        assert!(content.contains("Filter"));
        assert!(!content.contains("Start/Stop"));
        assert!(!content.contains("Reboot"));
        assert!(!content.contains("Create"));
        assert!(!content.contains("Delete"));
    }

    // S3 Detail: Enter(open dir), D(Delete)
    #[test]
    fn help_popup_render_returns_s3_detail_actions_when_s3_detail() {
        let content = render_help(&View::S3Detail);
        assert!(content.contains("Open"));
        assert!(content.contains("Delete"));
        assert!(!content.contains("Start/Stop"));
        assert!(!content.contains("Filter"));
    }

    // Generic Detail (ECR, ECS, VPC): y のみ
    #[test]
    fn help_popup_render_returns_generic_detail_actions_when_ecr_detail() {
        let content = render_help(&View::EcrDetail);
        assert!(content.contains("Copy"));
        assert!(!content.contains("Start/Stop"));
        assert!(!content.contains("Filter"));
        assert!(!content.contains("Create"));
        assert!(!content.contains("Delete"));
    }
}
