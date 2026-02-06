use tokio::sync::mpsc;

use crate::aws::model::Instance;
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
    pub filter_text: String,
    pub detail_tab: DetailTab,

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
            filter_text: String::new(),
            detail_tab: DetailTab::Overview,
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
        if self.filter_text.is_empty() {
            self.filtered_instances = self.instances.clone();
        } else {
            let query = self.filter_text.to_lowercase();
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aws::model::InstanceState;
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
        app.filter_text = String::new();
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
        app.filter_text = "web".to_string();
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
        app.filter_text = "web".to_string();
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
}
