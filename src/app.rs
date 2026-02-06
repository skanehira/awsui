use tokio::sync::mpsc;
use tui_input::Input;

use crate::action::Action;
use crate::aws::model::{Instance, InstanceState};
use crate::event::AppEvent;

/// アプリケーションのモード
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Filter,
    Confirm(ConfirmAction),
    Message,
    Help,
}

/// 確認ダイアログで実行するアクション
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmAction {
    Start(String),
    Stop(String),
    Reboot(String),
}

/// 現在の画面
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    ProfileSelect,
    Ec2List,
    Ec2Detail,
}

/// 詳細画面のタブ
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetailTab {
    Overview,
    Tags,
}

/// メッセージダイアログの種別
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageLevel {
    Info,
    Success,
    Error,
}

/// メッセージダイアログの内容
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub level: MessageLevel,
    pub title: String,
    pub body: String,
}

/// アプリケーション全体の状態
pub struct App {
    // UI state
    pub mode: Mode,
    pub view: View,
    pub should_quit: bool,
    pub loading: bool,
    pub message: Option<Message>,

    // AWS context
    pub profile: Option<String>,
    pub region: Option<String>,

    // Profile selection
    pub profile_names: Vec<String>,
    pub profile_selected: usize,

    // EC2 state
    pub instances: Vec<Instance>,
    pub filtered_instances: Vec<Instance>,
    pub selected_index: usize,
    pub filter_input: Input,
    pub detail_tab: DetailTab,
    pub detail_tag_index: usize,

    // Async communication
    pub event_tx: mpsc::Sender<AppEvent>,
    pub event_rx: mpsc::Receiver<AppEvent>,
}

impl App {
    pub fn new(profile_names: Vec<String>) -> Self {
        let (event_tx, event_rx) = mpsc::channel(32);
        Self {
            mode: Mode::Normal,
            view: View::ProfileSelect,
            should_quit: false,
            loading: false,
            message: None,
            profile: None,
            region: None,
            profile_names,
            profile_selected: 0,
            instances: Vec::new(),
            filtered_instances: Vec::new(),
            selected_index: 0,
            filter_input: Input::default(),
            detail_tab: DetailTab::Overview,
            detail_tag_index: 0,
            event_tx,
            event_rx,
        }
    }

    /// 選択中のインスタンスを返す
    pub fn selected_instance(&self) -> Option<&Instance> {
        self.filtered_instances.get(self.selected_index)
    }

    /// フィルタを適用してfiltered_instancesを更新
    pub fn apply_filter(&mut self) {
        let filter_text = self.filter_input.value();
        if filter_text.is_empty() {
            self.filtered_instances = self.instances.clone();
        } else {
            let query = filter_text.to_lowercase();
            self.filtered_instances = self
                .instances
                .iter()
                .filter(|i| {
                    i.instance_id.to_lowercase().contains(&query)
                        || i.name.to_lowercase().contains(&query)
                        || i.instance_type.to_lowercase().contains(&query)
                        || i.state.as_str().contains(&query)
                })
                .cloned()
                .collect();
        }
        // フィルタ後にインデックスが範囲外にならないよう調整
        if !self.filtered_instances.is_empty()
            && self.selected_index >= self.filtered_instances.len()
        {
            self.selected_index = self.filtered_instances.len() - 1;
        }
    }

    /// メッセージダイアログを表示
    pub fn show_message(
        &mut self,
        level: MessageLevel,
        title: impl Into<String>,
        body: impl Into<String>,
    ) {
        self.message = Some(Message {
            level,
            title: title.into(),
            body: body.into(),
        });
        self.mode = Mode::Message;
    }

    /// メッセージダイアログを閉じる
    pub fn dismiss_message(&mut self) {
        self.message = None;
        self.mode = Mode::Normal;
    }

    /// Actionに基づいてApp状態を更新する。
    /// ConfirmYes時にconfirm_actionを返す（main側でAPI呼び出しに使う）。
    pub fn dispatch(&mut self, action: Action) -> Option<ConfirmAction> {
        match action {
            Action::Quit => self.should_quit = true,
            Action::MoveUp => self.move_up(),
            Action::MoveDown => self.move_down(),
            Action::MoveToTop => self.move_to_top(),
            Action::MoveToBottom => self.move_to_bottom(),
            Action::HalfPageUp => self.half_page_up(),
            Action::HalfPageDown => self.half_page_down(),
            Action::Enter => self.handle_enter(),
            Action::Back => self.handle_back(),
            Action::Refresh => self.loading = true,
            Action::CopyId => self.copy_instance_id(),
            Action::StartFilter => self.mode = Mode::Filter,
            Action::ConfirmFilter => self.mode = Mode::Normal,
            Action::CancelFilter => {
                self.mode = Mode::Normal;
                self.filter_input.reset();
                self.apply_filter();
            }
            Action::FilterHandleInput(req) => {
                self.filter_input.handle(req);
                self.apply_filter();
            }
            Action::StartStop => self.handle_start_stop(),
            Action::Reboot => self.handle_reboot(),
            Action::ConfirmYes => return self.handle_confirm_yes(),
            Action::ConfirmNo => self.mode = Mode::Normal,
            Action::DismissMessage => self.dismiss_message(),
            Action::ShowHelp => self.mode = Mode::Help,
            Action::SwitchDetailTab => {
                self.detail_tag_index = 0;
                self.detail_tab = match self.detail_tab {
                    DetailTab::Overview => DetailTab::Tags,
                    DetailTab::Tags => DetailTab::Overview,
                };
            }
            Action::Noop => {}
        }
        None
    }

    /// AppEventをApp状態に反映する。
    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::InstancesLoaded(Ok(instances)) => {
                let is_empty = instances.is_empty();
                self.instances = instances;
                self.loading = false;
                self.apply_filter();
                if is_empty {
                    self.show_message(MessageLevel::Info, "Info", "No instances found");
                }
            }
            AppEvent::InstancesLoaded(Err(e)) => {
                self.loading = false;
                self.show_message(MessageLevel::Error, "Error", e.to_string());
            }
            AppEvent::ActionCompleted(Ok(msg)) => {
                self.show_message(MessageLevel::Success, "Success", msg);
                self.loading = true;
            }
            AppEvent::ActionCompleted(Err(e)) => {
                self.show_message(MessageLevel::Error, "Error", e.to_string());
            }
        }
    }

    fn move_up(&mut self) {
        match self.view {
            View::ProfileSelect => {
                self.profile_selected = self.profile_selected.saturating_sub(1);
            }
            View::Ec2List => {
                self.selected_index = self.selected_index.saturating_sub(1);
            }
            View::Ec2Detail => {
                self.detail_tag_index = self.detail_tag_index.saturating_sub(1);
            }
        }
    }

    fn move_down(&mut self) {
        match self.view {
            View::ProfileSelect => {
                let max = self.profile_names.len().saturating_sub(1);
                if self.profile_selected < max {
                    self.profile_selected += 1;
                }
            }
            View::Ec2List => {
                let max = self.filtered_instances.len().saturating_sub(1);
                if self.selected_index < max {
                    self.selected_index += 1;
                }
            }
            View::Ec2Detail => {
                let max = self
                    .selected_instance()
                    .map(|i| i.tags.len().saturating_sub(1))
                    .unwrap_or(0);
                if self.detail_tag_index < max {
                    self.detail_tag_index += 1;
                }
            }
        }
    }

    fn move_to_top(&mut self) {
        match self.view {
            View::ProfileSelect => self.profile_selected = 0,
            View::Ec2List => self.selected_index = 0,
            View::Ec2Detail => self.detail_tag_index = 0,
        }
    }

    fn move_to_bottom(&mut self) {
        match self.view {
            View::ProfileSelect => {
                self.profile_selected = self.profile_names.len().saturating_sub(1);
            }
            View::Ec2List => {
                self.selected_index = self.filtered_instances.len().saturating_sub(1);
            }
            View::Ec2Detail => {
                self.detail_tag_index = self
                    .selected_instance()
                    .map(|i| i.tags.len().saturating_sub(1))
                    .unwrap_or(0);
            }
        }
    }

    fn half_page_up(&mut self) {
        match self.view {
            View::ProfileSelect => {
                self.profile_selected = self.profile_selected.saturating_sub(10);
            }
            View::Ec2List => {
                self.selected_index = self.selected_index.saturating_sub(10);
            }
            View::Ec2Detail => {
                self.detail_tag_index = self.detail_tag_index.saturating_sub(10);
            }
        }
    }

    fn half_page_down(&mut self) {
        match self.view {
            View::ProfileSelect => {
                let max = self.profile_names.len().saturating_sub(1);
                self.profile_selected = (self.profile_selected + 10).min(max);
            }
            View::Ec2List => {
                let max = self.filtered_instances.len().saturating_sub(1);
                self.selected_index = (self.selected_index + 10).min(max);
            }
            View::Ec2Detail => {
                let max = self
                    .selected_instance()
                    .map(|i| i.tags.len().saturating_sub(1))
                    .unwrap_or(0);
                self.detail_tag_index = (self.detail_tag_index + 10).min(max);
            }
        }
    }

    fn handle_enter(&mut self) {
        match self.view {
            View::ProfileSelect => {
                if let Some(name) = self.profile_names.get(self.profile_selected) {
                    self.profile = Some(name.clone());
                    self.view = View::Ec2List;
                }
            }
            View::Ec2List => {
                if !self.filtered_instances.is_empty() {
                    self.view = View::Ec2Detail;
                    self.detail_tab = DetailTab::Overview;
                    self.detail_tag_index = 0;
                }
            }
            View::Ec2Detail => {}
        }
    }

    fn handle_back(&mut self) {
        match self.view {
            View::Ec2Detail => self.view = View::Ec2List,
            View::Ec2List => {
                self.view = View::ProfileSelect;
                self.instances.clear();
                self.filtered_instances.clear();
            }
            View::ProfileSelect => {}
        }
        // Help/Message mode のバック
        match self.mode {
            Mode::Help => self.mode = Mode::Normal,
            Mode::Message => self.dismiss_message(),
            _ => {}
        }
    }

    fn copy_instance_id(&self) {
        if let Some(instance) = self.selected_instance() {
            let _ = cli_clipboard::set_contents(instance.instance_id.clone());
        }
    }

    fn handle_start_stop(&mut self) {
        if let Some(instance) = self.selected_instance() {
            let id = instance.instance_id.clone();
            match instance.state {
                InstanceState::Running => {
                    self.mode = Mode::Confirm(ConfirmAction::Stop(id));
                }
                InstanceState::Stopped => {
                    self.mode = Mode::Confirm(ConfirmAction::Start(id));
                }
                _ => {}
            }
        }
    }

    fn handle_reboot(&mut self) {
        if let Some(instance) = self.selected_instance() {
            let id = instance.instance_id.clone();
            self.mode = Mode::Confirm(ConfirmAction::Reboot(id));
        }
    }

    fn handle_confirm_yes(&mut self) -> Option<ConfirmAction> {
        let confirmed = if let Mode::Confirm(action) = &self.mode {
            Some(action.clone())
        } else {
            None
        };
        self.mode = Mode::Normal;
        confirmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Action;
    use crate::aws::model::InstanceState;
    use crate::error::AppError;
    use std::collections::HashMap;

    fn create_test_instance(id: &str, name: &str, state: InstanceState) -> Instance {
        Instance {
            instance_id: id.to_string(),
            name: name.to_string(),
            state,
            instance_type: "t3.micro".to_string(),
            availability_zone: "ap-northeast-1a".to_string(),
            private_ip: None,
            public_ip: None,
            vpc_id: None,
            subnet_id: None,
            ami_id: "ami-test".to_string(),
            key_name: None,
            platform: None,
            launch_time: None,
            security_groups: Vec::new(),
            volumes: Vec::new(),
            tags: HashMap::new(),
        }
    }

    #[test]
    fn app_new_returns_initial_state_when_created() {
        let app = App::new(vec!["dev".to_string(), "staging".to_string()]);
        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.view, View::ProfileSelect);
        assert!(!app.should_quit);
        assert!(!app.loading);
        assert!(app.message.is_none());
        assert_eq!(app.profile_names.len(), 2);
    }

    #[test]
    fn selected_instance_returns_none_when_no_instances() {
        let app = App::new(vec![]);
        assert!(app.selected_instance().is_none());
    }

    #[test]
    fn selected_instance_returns_instance_when_index_valid() {
        let mut app = App::new(vec![]);
        app.filtered_instances = vec![
            create_test_instance("i-001", "web", InstanceState::Running),
            create_test_instance("i-002", "api", InstanceState::Stopped),
        ];
        app.selected_index = 1;
        let instance = app.selected_instance().unwrap();
        assert_eq!(instance.instance_id, "i-002");
    }

    #[test]
    fn apply_filter_returns_all_instances_when_empty_filter() {
        let mut app = App::new(vec![]);
        app.instances = vec![
            create_test_instance("i-001", "web", InstanceState::Running),
            create_test_instance("i-002", "api", InstanceState::Stopped),
        ];
        app.filter_input = Input::default();
        app.apply_filter();
        assert_eq!(app.filtered_instances.len(), 2);
    }

    #[test]
    fn apply_filter_returns_matching_instances_when_filter_set() {
        let mut app = App::new(vec![]);
        app.instances = vec![
            create_test_instance("i-001", "web", InstanceState::Running),
            create_test_instance("i-002", "api", InstanceState::Stopped),
        ];
        app.filter_input = Input::from("web");
        app.apply_filter();
        assert_eq!(app.filtered_instances.len(), 1);
        assert_eq!(app.filtered_instances[0].name, "web");
    }

    #[test]
    fn apply_filter_adjusts_index_when_filter_reduces_list() {
        let mut app = App::new(vec![]);
        app.instances = vec![
            create_test_instance("i-001", "web", InstanceState::Running),
            create_test_instance("i-002", "api", InstanceState::Stopped),
        ];
        app.selected_index = 1;
        app.filter_input = Input::from("web");
        app.apply_filter();
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn show_message_sets_mode_to_message_when_called() {
        let mut app = App::new(vec![]);
        app.show_message(MessageLevel::Error, "Error", "Something failed");
        assert_eq!(app.mode, Mode::Message);
        assert!(app.message.is_some());
        let msg = app.message.as_ref().unwrap();
        assert_eq!(msg.level, MessageLevel::Error);
        assert_eq!(msg.title, "Error");
    }

    #[test]
    fn dismiss_message_clears_message_when_called() {
        let mut app = App::new(vec![]);
        app.show_message(MessageLevel::Info, "Info", "test");
        app.dismiss_message();
        assert!(app.message.is_none());
        assert_eq!(app.mode, Mode::Normal);
    }

    // ──────────────────────────────────────────────
    // dispatch テスト
    // ──────────────────────────────────────────────

    #[test]
    fn dispatch_returns_none_and_sets_quit_when_quit_action() {
        let mut app = App::new(vec!["dev".to_string()]);
        let result = app.dispatch(Action::Quit);
        assert!(app.should_quit);
        assert!(result.is_none());
    }

    #[test]
    fn dispatch_returns_none_and_decrements_profile_selected_when_move_up_in_profile_select() {
        let mut app = App::new(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
        app.profile_selected = 2;
        app.dispatch(Action::MoveUp);
        assert_eq!(app.profile_selected, 1);
    }

    #[test]
    fn dispatch_returns_none_and_clamps_at_zero_when_move_up_at_top() {
        let mut app = App::new(vec!["a".to_string()]);
        app.profile_selected = 0;
        app.dispatch(Action::MoveUp);
        assert_eq!(app.profile_selected, 0);
    }

    #[test]
    fn dispatch_returns_none_and_increments_profile_selected_when_move_down_in_profile_select() {
        let mut app = App::new(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
        app.profile_selected = 0;
        app.dispatch(Action::MoveDown);
        assert_eq!(app.profile_selected, 1);
    }

    #[test]
    fn dispatch_returns_none_and_clamps_at_max_when_move_down_at_bottom() {
        let mut app = App::new(vec!["a".to_string(), "b".to_string()]);
        app.profile_selected = 1;
        app.dispatch(Action::MoveDown);
        assert_eq!(app.profile_selected, 1);
    }

    #[test]
    fn dispatch_returns_none_and_decrements_selected_index_when_move_up_in_ec2_list() {
        let mut app = App::new(vec![]);
        app.view = View::Ec2List;
        app.filtered_instances = vec![
            create_test_instance("i-001", "a", InstanceState::Running),
            create_test_instance("i-002", "b", InstanceState::Stopped),
        ];
        app.selected_index = 1;
        app.dispatch(Action::MoveUp);
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn dispatch_returns_none_and_increments_selected_index_when_move_down_in_ec2_list() {
        let mut app = App::new(vec![]);
        app.view = View::Ec2List;
        app.filtered_instances = vec![
            create_test_instance("i-001", "a", InstanceState::Running),
            create_test_instance("i-002", "b", InstanceState::Stopped),
        ];
        app.selected_index = 0;
        app.dispatch(Action::MoveDown);
        assert_eq!(app.selected_index, 1);
    }

    #[test]
    fn dispatch_returns_none_and_sets_index_zero_when_move_to_top() {
        let mut app = App::new(vec![]);
        app.view = View::Ec2List;
        app.filtered_instances = vec![
            create_test_instance("i-001", "a", InstanceState::Running),
            create_test_instance("i-002", "b", InstanceState::Stopped),
        ];
        app.selected_index = 1;
        app.dispatch(Action::MoveToTop);
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn dispatch_returns_none_and_sets_index_to_last_when_move_to_bottom() {
        let mut app = App::new(vec![]);
        app.view = View::Ec2List;
        app.filtered_instances = vec![
            create_test_instance("i-001", "a", InstanceState::Running),
            create_test_instance("i-002", "b", InstanceState::Stopped),
        ];
        app.selected_index = 0;
        app.dispatch(Action::MoveToBottom);
        assert_eq!(app.selected_index, 1);
    }

    #[test]
    fn dispatch_returns_none_and_moves_up_10_when_half_page_up() {
        let mut app = App::new(vec![]);
        app.view = View::Ec2List;
        app.filtered_instances = (0..20)
            .map(|i| create_test_instance(&format!("i-{i:03}"), "inst", InstanceState::Running))
            .collect();
        app.selected_index = 15;
        app.dispatch(Action::HalfPageUp);
        assert_eq!(app.selected_index, 5);
    }

    #[test]
    fn dispatch_returns_none_and_clamps_at_zero_when_half_page_up_near_top() {
        let mut app = App::new(vec![]);
        app.view = View::Ec2List;
        app.filtered_instances = vec![
            create_test_instance("i-001", "a", InstanceState::Running),
            create_test_instance("i-002", "b", InstanceState::Stopped),
        ];
        app.selected_index = 1;
        app.dispatch(Action::HalfPageUp);
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn dispatch_returns_none_and_moves_down_10_when_half_page_down() {
        let mut app = App::new(vec![]);
        app.view = View::Ec2List;
        app.filtered_instances = (0..20)
            .map(|i| create_test_instance(&format!("i-{i:03}"), "inst", InstanceState::Running))
            .collect();
        app.selected_index = 5;
        app.dispatch(Action::HalfPageDown);
        assert_eq!(app.selected_index, 15);
    }

    #[test]
    fn dispatch_returns_none_and_clamps_at_max_when_half_page_down_near_bottom() {
        let mut app = App::new(vec![]);
        app.view = View::Ec2List;
        app.filtered_instances = vec![
            create_test_instance("i-001", "a", InstanceState::Running),
            create_test_instance("i-002", "b", InstanceState::Stopped),
        ];
        app.selected_index = 0;
        app.dispatch(Action::HalfPageDown);
        assert_eq!(app.selected_index, 1);
    }

    #[test]
    fn dispatch_returns_none_and_sets_profile_when_enter_in_profile_select() {
        let mut app = App::new(vec!["dev".to_string(), "staging".to_string()]);
        app.profile_selected = 1;
        app.dispatch(Action::Enter);
        assert_eq!(app.profile, Some("staging".to_string()));
        assert_eq!(app.view, View::Ec2List);
    }

    #[test]
    fn dispatch_returns_none_and_switches_to_detail_when_enter_in_ec2_list() {
        let mut app = App::new(vec![]);
        app.view = View::Ec2List;
        app.filtered_instances = vec![create_test_instance("i-001", "web", InstanceState::Running)];
        app.dispatch(Action::Enter);
        assert_eq!(app.view, View::Ec2Detail);
    }

    #[test]
    fn dispatch_returns_none_and_stays_on_ec2_list_when_enter_with_empty_instances() {
        let mut app = App::new(vec![]);
        app.view = View::Ec2List;
        app.dispatch(Action::Enter);
        assert_eq!(app.view, View::Ec2List);
    }

    #[test]
    fn dispatch_returns_none_and_goes_back_to_ec2_list_when_back_in_ec2_detail() {
        let mut app = App::new(vec![]);
        app.view = View::Ec2Detail;
        app.dispatch(Action::Back);
        assert_eq!(app.view, View::Ec2List);
    }

    #[test]
    fn dispatch_returns_none_and_clears_instances_when_back_in_ec2_list() {
        let mut app = App::new(vec!["dev".to_string()]);
        app.view = View::Ec2List;
        app.instances = vec![create_test_instance("i-001", "web", InstanceState::Running)];
        app.filtered_instances = vec![create_test_instance("i-001", "web", InstanceState::Running)];
        app.dispatch(Action::Back);
        assert_eq!(app.view, View::ProfileSelect);
        assert!(app.instances.is_empty());
        assert!(app.filtered_instances.is_empty());
    }

    #[test]
    fn dispatch_returns_none_and_sets_normal_mode_when_back_in_help() {
        let mut app = App::new(vec![]);
        app.mode = Mode::Help;
        app.view = View::Ec2List;
        app.dispatch(Action::Back);
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn dispatch_returns_none_and_dismisses_message_when_back_in_message() {
        let mut app = App::new(vec![]);
        app.view = View::Ec2List;
        app.show_message(MessageLevel::Info, "Info", "test");
        app.dispatch(Action::Back);
        assert!(app.message.is_none());
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn dispatch_returns_none_and_sets_loading_when_refresh() {
        let mut app = App::new(vec![]);
        app.dispatch(Action::Refresh);
        assert!(app.loading);
    }

    #[test]
    fn dispatch_returns_none_and_sets_filter_mode_when_start_filter() {
        let mut app = App::new(vec![]);
        app.dispatch(Action::StartFilter);
        assert_eq!(app.mode, Mode::Filter);
    }

    #[test]
    fn dispatch_returns_none_and_sets_normal_mode_when_confirm_filter() {
        let mut app = App::new(vec![]);
        app.mode = Mode::Filter;
        app.dispatch(Action::ConfirmFilter);
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn dispatch_returns_none_and_clears_filter_when_cancel_filter() {
        let mut app = App::new(vec![]);
        app.mode = Mode::Filter;
        app.filter_input = Input::from("web");
        app.instances = vec![create_test_instance("i-001", "web", InstanceState::Running)];
        app.dispatch(Action::CancelFilter);
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.filter_input.value().is_empty());
        assert_eq!(app.filtered_instances.len(), 1);
    }

    #[test]
    fn dispatch_returns_none_and_updates_input_when_filter_handle_input() {
        use tui_input::InputRequest;
        let mut app = App::new(vec![]);
        app.instances = vec![create_test_instance("i-001", "web", InstanceState::Running)];
        app.dispatch(Action::FilterHandleInput(InputRequest::InsertChar('w')));
        assert_eq!(app.filter_input.value(), "w");
        assert_eq!(app.filtered_instances.len(), 1);
    }

    #[test]
    fn dispatch_returns_none_and_deletes_char_when_filter_handle_input_delete() {
        use tui_input::InputRequest;
        let mut app = App::new(vec![]);
        app.filter_input = Input::from("web");
        app.instances = vec![create_test_instance("i-001", "web", InstanceState::Running)];
        app.dispatch(Action::FilterHandleInput(InputRequest::DeletePrevChar));
        assert_eq!(app.filter_input.value(), "we");
    }

    #[test]
    fn dispatch_returns_none_and_sets_confirm_stop_when_start_stop_on_running() {
        let mut app = App::new(vec![]);
        app.view = View::Ec2List;
        app.filtered_instances = vec![create_test_instance("i-001", "web", InstanceState::Running)];
        app.dispatch(Action::StartStop);
        assert_eq!(
            app.mode,
            Mode::Confirm(ConfirmAction::Stop("i-001".to_string()))
        );
    }

    #[test]
    fn dispatch_returns_none_and_sets_confirm_start_when_start_stop_on_stopped() {
        let mut app = App::new(vec![]);
        app.view = View::Ec2List;
        app.filtered_instances = vec![create_test_instance("i-001", "web", InstanceState::Stopped)];
        app.dispatch(Action::StartStop);
        assert_eq!(
            app.mode,
            Mode::Confirm(ConfirmAction::Start("i-001".to_string()))
        );
    }

    #[test]
    fn dispatch_returns_none_and_sets_confirm_reboot_when_reboot() {
        let mut app = App::new(vec![]);
        app.view = View::Ec2List;
        app.filtered_instances = vec![create_test_instance("i-001", "web", InstanceState::Running)];
        app.dispatch(Action::Reboot);
        assert_eq!(
            app.mode,
            Mode::Confirm(ConfirmAction::Reboot("i-001".to_string()))
        );
    }

    #[test]
    fn dispatch_returns_confirm_action_when_confirm_yes() {
        let mut app = App::new(vec![]);
        app.mode = Mode::Confirm(ConfirmAction::Stop("i-001".to_string()));
        let result = app.dispatch(Action::ConfirmYes);
        assert_eq!(result, Some(ConfirmAction::Stop("i-001".to_string())));
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn dispatch_returns_none_and_sets_normal_when_confirm_no() {
        let mut app = App::new(vec![]);
        app.mode = Mode::Confirm(ConfirmAction::Stop("i-001".to_string()));
        let result = app.dispatch(Action::ConfirmNo);
        assert!(result.is_none());
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn dispatch_returns_none_and_dismisses_when_dismiss_message() {
        let mut app = App::new(vec![]);
        app.show_message(MessageLevel::Info, "Info", "test");
        app.dispatch(Action::DismissMessage);
        assert!(app.message.is_none());
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn dispatch_returns_none_and_sets_help_mode_when_show_help() {
        let mut app = App::new(vec![]);
        app.dispatch(Action::ShowHelp);
        assert_eq!(app.mode, Mode::Help);
    }

    #[test]
    fn dispatch_returns_none_and_toggles_tab_when_switch_detail_tab() {
        let mut app = App::new(vec![]);
        assert_eq!(app.detail_tab, DetailTab::Overview);
        app.dispatch(Action::SwitchDetailTab);
        assert_eq!(app.detail_tab, DetailTab::Tags);
        app.dispatch(Action::SwitchDetailTab);
        assert_eq!(app.detail_tab, DetailTab::Overview);
    }

    #[test]
    fn dispatch_returns_none_when_noop() {
        let mut app = App::new(vec!["dev".to_string()]);
        let result = app.dispatch(Action::Noop);
        assert!(result.is_none());
    }

    // ──────────────────────────────────────────────
    // handle_event テスト
    // ──────────────────────────────────────────────

    #[test]
    fn handle_event_sets_instances_when_instances_loaded_ok() {
        let mut app = App::new(vec![]);
        app.loading = true;
        let instances = vec![
            create_test_instance("i-001", "web", InstanceState::Running),
            create_test_instance("i-002", "api", InstanceState::Stopped),
        ];
        app.handle_event(AppEvent::InstancesLoaded(Ok(instances)));
        assert!(!app.loading);
        assert_eq!(app.instances.len(), 2);
        assert_eq!(app.filtered_instances.len(), 2);
    }

    #[test]
    fn handle_event_shows_info_message_when_instances_loaded_ok_empty() {
        let mut app = App::new(vec![]);
        app.loading = true;
        app.handle_event(AppEvent::InstancesLoaded(Ok(vec![])));
        assert!(!app.loading);
        assert_eq!(app.mode, Mode::Message);
        let msg = app.message.as_ref().unwrap();
        assert_eq!(msg.level, MessageLevel::Info);
        assert_eq!(msg.body, "No instances found");
    }

    #[test]
    fn handle_event_shows_error_when_instances_loaded_err() {
        let mut app = App::new(vec![]);
        app.loading = true;
        app.handle_event(AppEvent::InstancesLoaded(Err(AppError::AwsApi(
            "access denied".to_string(),
        ))));
        assert!(!app.loading);
        assert_eq!(app.mode, Mode::Message);
        let msg = app.message.as_ref().unwrap();
        assert_eq!(msg.level, MessageLevel::Error);
        assert!(msg.body.contains("access denied"));
    }

    #[test]
    fn handle_event_shows_success_and_sets_loading_when_action_completed_ok() {
        let mut app = App::new(vec![]);
        app.handle_event(AppEvent::ActionCompleted(
            Ok("Instance started".to_string()),
        ));
        assert_eq!(app.mode, Mode::Message);
        let msg = app.message.as_ref().unwrap();
        assert_eq!(msg.level, MessageLevel::Success);
        assert_eq!(msg.body, "Instance started");
        assert!(app.loading);
    }

    #[test]
    fn handle_event_shows_error_when_action_completed_err() {
        let mut app = App::new(vec![]);
        app.handle_event(AppEvent::ActionCompleted(Err(AppError::AwsApi(
            "start failed".to_string(),
        ))));
        assert_eq!(app.mode, Mode::Message);
        let msg = app.message.as_ref().unwrap();
        assert_eq!(msg.level, MessageLevel::Error);
        assert!(msg.body.contains("start failed"));
    }

    // ──────────────────────────────────────────────
    // filter_delete_word テスト
    // ──────────────────────────────────────────────

    // ──────────────────────────────────────────────
    // detail_tag_index テスト
    // ──────────────────────────────────────────────

    #[test]
    fn dispatch_returns_none_and_moves_detail_tag_index_when_move_down_in_ec2_detail() {
        let mut app = App::new(vec![]);
        app.view = View::Ec2Detail;
        let mut instance = create_test_instance("i-001", "web", InstanceState::Running);
        instance.tags.insert("env".to_string(), "prod".to_string());
        instance
            .tags
            .insert("team".to_string(), "backend".to_string());
        app.instances = vec![instance.clone()];
        app.filtered_instances = vec![instance];
        app.selected_index = 0;
        app.detail_tag_index = 0;
        app.dispatch(Action::MoveDown);
        assert_eq!(app.detail_tag_index, 1);
        // selected_index は変わらない
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn dispatch_returns_none_and_moves_detail_tag_index_when_move_up_in_ec2_detail() {
        let mut app = App::new(vec![]);
        app.view = View::Ec2Detail;
        let instance = create_test_instance("i-001", "web", InstanceState::Running);
        app.instances = vec![instance.clone()];
        app.filtered_instances = vec![instance];
        app.selected_index = 0;
        app.detail_tag_index = 1;
        app.dispatch(Action::MoveUp);
        assert_eq!(app.detail_tag_index, 0);
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn dispatch_returns_none_and_resets_detail_tag_index_when_enter_ec2_detail() {
        let mut app = App::new(vec![]);
        app.view = View::Ec2List;
        let instance = create_test_instance("i-001", "web", InstanceState::Running);
        app.instances = vec![instance.clone()];
        app.filtered_instances = vec![instance];
        app.detail_tag_index = 5;
        app.dispatch(Action::Enter);
        assert_eq!(app.view, View::Ec2Detail);
        assert_eq!(app.detail_tag_index, 0);
    }

    #[test]
    fn dispatch_returns_none_and_resets_detail_tag_index_when_switch_detail_tab() {
        let mut app = App::new(vec![]);
        app.detail_tag_index = 3;
        app.dispatch(Action::SwitchDetailTab);
        assert_eq!(app.detail_tag_index, 0);
    }
}
