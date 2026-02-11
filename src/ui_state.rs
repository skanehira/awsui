use tui_input::Input;

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
    /// コンテナ選択モード（ログ表示用）
    ContainerSelect {
        names: Vec<String>,
        selected: usize,
    },
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
}

/// サービスピッカーの状態
pub struct ServicePickerState {
    pub selected_index: usize,
    pub filter_input: Input,
    pub filtered_services: Vec<ServiceKind>,
}
