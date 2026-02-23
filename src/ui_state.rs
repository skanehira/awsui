use tui_input::Input;

use crate::config::SsoProfile;
use crate::fuzzy::fuzzy_filter_items;
use crate::service::ServiceKind;
use crate::tab::TabView;

/// ナビゲーションスタックのエントリ
/// リンクフォロー時に遷移元の状態を保存する
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationEntry {
    /// 遷移元のビュー (ServiceKind, TabView)
    pub view: (ServiceKind, TabView),
    /// 遷移元のリスト選択インデックス
    pub selected_index: usize,
    /// 遷移元の詳細タグインデックス
    pub detail_tag_index: usize,
    /// 遷移元の詳細タブ
    pub detail_tab: DetailTab,
    /// パンくずリスト用のラベル（例: "i-0abc123", "vpc-0def456"）
    pub label: String,
}

/// EC2 Detail Overviewタブのリンク可能フィールド
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ec2DetailField {
    VpcId,
    SubnetId,
}

impl Ec2DetailField {
    /// 全フィールドのリスト（表示順）
    pub const ALL: &'static [Ec2DetailField] = &[Ec2DetailField::VpcId, Ec2DetailField::SubnetId];

    /// フィールド名を返す
    pub fn label(&self) -> &str {
        match self {
            Ec2DetailField::VpcId => "VPC",
            Ec2DetailField::SubnetId => "Subnet",
        }
    }
}

/// アプリケーションのモード
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Filter,
    Confirm(ConfirmAction),
    Message,
    Help,
    /// フォーム入力モード（Create/Edit操作）
    Form(FormContext),
    /// 危険操作確認モード（リソース名入力での確認）
    DangerConfirm(DangerConfirmContext),
    /// コンテナ選択モード（ログ表示・ECS Exec用）
    ContainerSelect(ContainerSelectState),
}

/// コンテナ選択の目的
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerSelectPurpose {
    ShowLogs,
    EcsExec,
}

/// コンテナ選択ダイアログの状態
#[derive(Debug, Clone)]
pub struct ContainerSelectState {
    pub all_names: Vec<String>,
    pub filtered_names: Vec<String>,
    pub selected_index: usize,
    pub filter_input: Input,
    pub purpose: ContainerSelectPurpose,
}

impl PartialEq for ContainerSelectState {
    fn eq(&self, other: &Self) -> bool {
        self.all_names == other.all_names
            && self.filtered_names == other.filtered_names
            && self.selected_index == other.selected_index
            && self.filter_input.value() == other.filter_input.value()
            && self.purpose == other.purpose
    }
}

impl Eq for ContainerSelectState {}

impl ContainerSelectState {
    pub fn new(names: Vec<String>, purpose: ContainerSelectPurpose) -> Self {
        let filtered_names = names.clone();
        Self {
            all_names: names,
            filtered_names,
            selected_index: 0,
            filter_input: Input::default(),
            purpose,
        }
    }

    pub fn apply_filter(&mut self) {
        self.filtered_names =
            fuzzy_filter_items(&self.all_names, self.filter_input.value(), 0, |s| {
                vec![s.as_str()]
            });
        self.selected_index = 0;
    }

    pub fn move_up(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        if !self.filtered_names.is_empty() {
            self.selected_index = (self.selected_index + 1).min(self.filtered_names.len() - 1);
        }
    }

    pub fn selected_name(&self) -> Option<&str> {
        self.filtered_names
            .get(self.selected_index)
            .map(|s| s.as_str())
    }
}

/// 確認ダイアログで実行するアクション
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmAction {
    Start(String),
    Stop(String),
    Reboot(String),
}

/// フォームの種類
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormKind {
    CreateS3Bucket,
    CreateSecret,
    UpdateSecretValue,
}

/// フォーム入力のコンテキスト
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormContext {
    pub kind: FormKind,
    pub fields: Vec<FormField>,
    pub focused_field: usize,
}

/// フォームの1フィールド
#[derive(Debug, Clone)]
pub struct FormField {
    pub label: String,
    pub input: Input,
    pub required: bool,
}

impl PartialEq for FormField {
    fn eq(&self, other: &Self) -> bool {
        self.label == other.label
            && self.input.value() == other.input.value()
            && self.required == other.required
    }
}

impl Eq for FormField {}

impl FormContext {
    pub fn field_values(&self) -> Vec<(&str, &str)> {
        self.fields
            .iter()
            .map(|f| (f.label.as_str(), f.input.value()))
            .collect()
    }
}

/// 危険操作の種類
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DangerAction {
    TerminateEc2(String),
    DeleteS3Bucket(String),
    DeleteS3Object { bucket: String, key: String },
    DeleteSecret(String),
}

impl DangerAction {
    /// 確認に必要な入力テキストを返す
    pub fn confirm_text(&self) -> &str {
        match self {
            DangerAction::TerminateEc2(id) => id,
            DangerAction::DeleteS3Bucket(name) => name,
            DangerAction::DeleteS3Object { key, .. } => key,
            DangerAction::DeleteSecret(name) => name,
        }
    }

    /// ダイアログに表示するメッセージ
    pub fn message(&self) -> String {
        match self {
            DangerAction::TerminateEc2(id) => {
                format!("Type '{}' to terminate this instance:", id)
            }
            DangerAction::DeleteS3Bucket(name) => {
                format!("Type '{}' to delete this bucket:", name)
            }
            DangerAction::DeleteS3Object { key, .. } => {
                format!("Type '{}' to delete this object:", key)
            }
            DangerAction::DeleteSecret(name) => {
                format!("Type '{}' to delete this secret:", name)
            }
        }
    }
}

/// 危険操作確認のコンテキスト
#[derive(Debug, Clone)]
pub struct DangerConfirmContext {
    pub action: DangerAction,
    pub input: Input,
}

impl PartialEq for DangerConfirmContext {
    fn eq(&self, other: &Self) -> bool {
        self.action == other.action && self.input.value() == other.input.value()
    }
}

impl Eq for DangerConfirmContext {}

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

/// ダッシュボードの状態
pub struct DashboardState {
    pub selected_index: usize,
    pub filter_input: Input,
    pub filtered_services: Vec<ServiceKind>,
    pub mode: Mode,
    /// フィルタ適用後の最近使ったサービス
    pub recent_services: Vec<ServiceKind>,
    /// 元の最近使ったサービス（フィルタリセット時に復元用）
    pub(crate) all_recent_services: Vec<ServiceKind>,
}

impl Default for DashboardState {
    fn default() -> Self {
        Self::new()
    }
}

impl DashboardState {
    pub fn new() -> Self {
        #[cfg(not(test))]
        let recent = crate::recent::load_recent()
            .into_iter()
            .map(|e| e.service)
            .collect::<Vec<_>>();
        #[cfg(test)]
        let recent = Vec::new();
        Self {
            selected_index: 0,
            filter_input: Input::default(),
            filtered_services: ServiceKind::ALL.to_vec(),
            mode: Mode::Normal,
            recent_services: recent.clone(),
            all_recent_services: recent,
        }
    }

    /// ダッシュボードの合計アイテム数（Recent + All Services）
    pub fn item_count(&self) -> usize {
        self.recent_services.len() + self.filtered_services.len()
    }

    /// 選択されたアイテムのServiceKindを返す
    pub fn selected_service(&self) -> Option<ServiceKind> {
        let recent_len = self.recent_services.len();
        if self.selected_index < recent_len {
            self.recent_services.get(self.selected_index).copied()
        } else {
            self.filtered_services
                .get(self.selected_index - recent_len)
                .copied()
        }
    }

    /// 最近使ったサービスを更新（メモリ内のみ）
    pub fn update_recent(&mut self, service: ServiceKind) {
        crate::recent::apply_recent_update(&mut self.all_recent_services, service);
        self.recent_services = self.all_recent_services.clone();
    }

    pub fn move_up(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        let max = self.item_count().saturating_sub(1);
        if self.selected_index < max {
            self.selected_index += 1;
        }
    }

    pub fn move_to_top(&mut self) {
        self.selected_index = 0;
    }

    pub fn move_to_bottom(&mut self) {
        self.selected_index = self.item_count().saturating_sub(1);
    }

    pub fn half_page_up(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(10);
    }

    pub fn half_page_down(&mut self) {
        let max = self.item_count().saturating_sub(1);
        self.selected_index = (self.selected_index + 10).min(max);
    }
}

/// dispatch() が返す副作用
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SideEffect {
    None,
    Confirm(ConfirmAction),
    FormSubmit(FormContext),
    DangerAction(DangerAction),
    StartSsoLogin {
        profile_name: String,
        region: Option<String>,
    },
    SsmConnect {
        instance_id: String,
    },
    EcsExec {
        cluster_arn: String,
        task_arn: String,
        container_name: String,
    },
}

/// プロファイル選択画面の状態
pub struct ProfileSelectorState {
    pub profiles: Vec<SsoProfile>,
    pub filtered_profiles: Vec<SsoProfile>,
    pub selected_index: usize,
    pub filter_input: Input,
    pub mode: Mode,
    pub logging_in: bool,
    pub login_output: Vec<String>,
}

impl ProfileSelectorState {
    pub fn new(profiles: Vec<SsoProfile>) -> Self {
        let filtered_profiles = profiles.clone();
        Self {
            profiles,
            filtered_profiles,
            selected_index: 0,
            filter_input: Input::default(),
            mode: Mode::Normal,
            logging_in: false,
            login_output: Vec::new(),
        }
    }

    pub fn apply_filter(&mut self) {
        self.filtered_profiles =
            fuzzy_filter_items(&self.profiles, self.filter_input.value(), 0, |p| {
                vec![&p.name, p.region.as_deref().unwrap_or(""), &p.sso_start_url]
            });
        self.selected_index = 0;
    }

    pub fn clear_filter(&mut self) {
        self.filter_input = Input::default();
        self.filtered_profiles = self.profiles.clone();
        self.selected_index = 0;
    }

    pub fn selected_profile(&self) -> Option<&SsoProfile> {
        self.filtered_profiles.get(self.selected_index)
    }

    pub fn move_up(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        if !self.filtered_profiles.is_empty() {
            self.selected_index = (self.selected_index + 1).min(self.filtered_profiles.len() - 1);
        }
    }

    pub fn move_to_top(&mut self) {
        self.selected_index = 0;
    }

    pub fn move_to_bottom(&mut self) {
        if !self.filtered_profiles.is_empty() {
            self.selected_index = self.filtered_profiles.len() - 1;
        }
    }
}

/// サービスピッカーの状態
pub struct ServicePickerState {
    pub selected_index: usize,
    pub filter_input: Input,
    pub filtered_services: Vec<ServiceKind>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SsoProfile;

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
    fn profile_selector_new_returns_all_profiles_when_initialized() {
        let profiles = test_profiles();
        let state = ProfileSelectorState::new(profiles.clone());

        assert_eq!(state.profiles, profiles);
        assert_eq!(state.filtered_profiles, profiles);
        assert_eq!(state.selected_index, 0);
        assert_eq!(state.mode, Mode::Normal);
        assert!(!state.logging_in);
        assert!(state.login_output.is_empty());
    }

    #[test]
    fn profile_selector_apply_filter_returns_matching_profiles_when_filter_text_set() {
        let profiles = test_profiles();
        let mut state = ProfileSelectorState::new(profiles);
        state.filter_input = "dev".into();

        state.apply_filter();

        assert_eq!(state.filtered_profiles.len(), 1);
        assert_eq!(state.filtered_profiles[0].name, "dev-account");
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn profile_selector_apply_filter_returns_all_profiles_when_filter_empty() {
        let profiles = test_profiles();
        let mut state = ProfileSelectorState::new(profiles.clone());
        state.filter_input = "".into();

        state.apply_filter();

        assert_eq!(state.filtered_profiles.len(), 3);
        assert_eq!(state.filtered_profiles, profiles);
    }

    #[test]
    fn profile_selector_apply_filter_resets_selected_index_when_filter_applied() {
        let profiles = test_profiles();
        let mut state = ProfileSelectorState::new(profiles);
        state.selected_index = 2;
        state.filter_input = "staging".into();

        state.apply_filter();

        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn profile_selector_move_down_increments_index_when_not_at_bottom() {
        let profiles = test_profiles();
        let mut state = ProfileSelectorState::new(profiles);

        state.move_down();

        assert_eq!(state.selected_index, 1);
    }

    #[test]
    fn profile_selector_move_down_stays_at_bottom_when_at_last_item() {
        let profiles = test_profiles();
        let mut state = ProfileSelectorState::new(profiles);
        state.selected_index = 2;

        state.move_down();

        assert_eq!(state.selected_index, 2);
    }

    #[test]
    fn profile_selector_move_up_decrements_index_when_not_at_top() {
        let profiles = test_profiles();
        let mut state = ProfileSelectorState::new(profiles);
        state.selected_index = 2;

        state.move_up();

        assert_eq!(state.selected_index, 1);
    }

    #[test]
    fn profile_selector_move_up_stays_at_top_when_at_first_item() {
        let profiles = test_profiles();
        let mut state = ProfileSelectorState::new(profiles);

        state.move_up();

        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn profile_selector_move_to_top_sets_index_to_zero() {
        let profiles = test_profiles();
        let mut state = ProfileSelectorState::new(profiles);
        state.selected_index = 2;

        state.move_to_top();

        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn profile_selector_move_to_bottom_sets_index_to_last_item() {
        let profiles = test_profiles();
        let mut state = ProfileSelectorState::new(profiles);

        state.move_to_bottom();

        assert_eq!(state.selected_index, 2);
    }

    #[test]
    fn profile_selector_clear_filter_restores_all_profiles() {
        let profiles = test_profiles();
        let mut state = ProfileSelectorState::new(profiles.clone());
        state.filter_input = "dev".into();
        state.apply_filter();
        assert_eq!(state.filtered_profiles.len(), 1);

        state.clear_filter();

        assert_eq!(state.filtered_profiles, profiles);
        assert_eq!(state.selected_index, 0);
        assert_eq!(state.filter_input.value(), "");
    }

    #[test]
    fn profile_selector_selected_profile_returns_profile_at_index() {
        let profiles = test_profiles();
        let mut state = ProfileSelectorState::new(profiles.clone());
        state.selected_index = 1;

        assert_eq!(state.selected_profile(), Some(&profiles[1]));
    }

    #[test]
    fn profile_selector_selected_profile_returns_none_when_empty() {
        let state = ProfileSelectorState::new(vec![]);

        assert_eq!(state.selected_profile(), None);
    }
}
