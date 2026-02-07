use tokio::sync::mpsc;
use tui_input::Input;

use crate::action::Action;
use crate::aws::ecr_model::{Image, Repository};
use crate::aws::ecs_model::{Cluster, Service};
use crate::aws::model::{Instance, InstanceState};
use crate::aws::s3_model::{Bucket, S3Object};
use crate::aws::secrets_model::{Secret, SecretDetail};
use crate::aws::vpc_model::{Subnet, Vpc};
use crate::cli::DeletePermissions;
use crate::event::AppEvent;
use crate::fuzzy::fuzzy_filter_items;
use crate::tui::views::secrets_detail::SecretsDetailTab;

/// ナビゲーションスタックのエントリ
/// リンクフォロー時に遷移元の状態を保存する
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationEntry {
    /// 遷移元のビュー
    pub view: View,
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

/// 現在の画面
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    ServiceSelect,
    Ec2List,
    Ec2Detail,
    EcrList,
    EcrDetail,
    EcsList,
    EcsDetail,
    S3List,
    S3Detail,
    VpcList,
    VpcDetail,
    SecretsList,
    SecretsDetail,
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

    // Service selection
    pub service_selected: usize,
    pub filtered_service_names: Vec<String>,

    // Shared list/detail state
    pub selected_index: usize,
    pub filter_input: Input,
    pub detail_tab: DetailTab,
    pub detail_tag_index: usize,

    // EC2 state
    pub instances: Vec<Instance>,
    pub filtered_instances: Vec<Instance>,

    // ECR state
    pub ecr_repositories: Vec<Repository>,
    pub ecr_filtered_repositories: Vec<Repository>,
    pub ecr_images: Vec<Image>,

    // ECS state
    pub ecs_clusters: Vec<Cluster>,
    pub ecs_filtered_clusters: Vec<Cluster>,
    pub ecs_services: Vec<Service>,

    // S3 state
    pub s3_buckets: Vec<Bucket>,
    pub s3_filtered_buckets: Vec<Bucket>,
    pub s3_objects: Vec<S3Object>,
    pub s3_selected_bucket: Option<String>,
    pub s3_current_prefix: String,

    // VPC state
    pub vpcs: Vec<Vpc>,
    pub filtered_vpcs: Vec<Vpc>,
    pub subnets: Vec<Subnet>,

    // Secrets state
    pub secrets: Vec<Secret>,
    pub filtered_secrets: Vec<Secret>,
    pub secret_detail: Option<SecretDetail>,
    pub secrets_detail_tab: SecretsDetailTab,

    // Navigation
    pub navigation_stack: Vec<NavigationEntry>,
    pub navigate_target_id: Option<String>,

    // Delete permissions
    pub delete_permissions: DeletePermissions,

    // CRUD pending state (set by dispatch, consumed by main.rs)
    pub pending_form: Option<FormContext>,
    pub pending_danger_action: Option<DangerAction>,

    // Async communication
    pub event_tx: mpsc::Sender<AppEvent>,
    pub event_rx: mpsc::Receiver<AppEvent>,
}

impl App {
    pub fn new(profile: String, region: Option<String>) -> Self {
        Self::with_delete_permissions(profile, region, DeletePermissions::None)
    }

    pub fn with_delete_permissions(
        profile: String,
        region: Option<String>,
        delete_permissions: DeletePermissions,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::channel(32);
        Self {
            mode: Mode::Normal,
            view: View::ServiceSelect,
            should_quit: false,
            loading: false,
            message: None,
            profile: Some(profile),
            region,
            filtered_service_names: crate::tui::views::service_select::SERVICE_NAMES
                .iter()
                .map(|s| s.to_string())
                .collect(),
            service_selected: 0,
            selected_index: 0,
            filter_input: Input::default(),
            detail_tab: DetailTab::Overview,
            detail_tag_index: 0,
            instances: Vec::new(),
            filtered_instances: Vec::new(),
            ecr_repositories: Vec::new(),
            ecr_filtered_repositories: Vec::new(),
            ecr_images: Vec::new(),
            ecs_clusters: Vec::new(),
            ecs_filtered_clusters: Vec::new(),
            ecs_services: Vec::new(),
            s3_buckets: Vec::new(),
            s3_filtered_buckets: Vec::new(),
            s3_objects: Vec::new(),
            s3_selected_bucket: None,
            s3_current_prefix: String::new(),
            vpcs: Vec::new(),
            filtered_vpcs: Vec::new(),
            subnets: Vec::new(),
            secrets: Vec::new(),
            filtered_secrets: Vec::new(),
            secret_detail: None,
            secrets_detail_tab: SecretsDetailTab::Overview,
            navigation_stack: Vec::new(),
            navigate_target_id: None,
            delete_permissions,
            pending_form: None,
            pending_danger_action: None,
            event_tx,
            event_rx,
        }
    }

    /// 指定サービスの削除操作が許可されているか
    pub fn can_delete(&self, service: &str) -> bool {
        self.delete_permissions.can_delete(service)
    }

    /// 選択中のインスタンスを返す
    pub fn selected_instance(&self) -> Option<&Instance> {
        self.filtered_instances.get(self.selected_index)
    }

    /// 現在のリストビューのフィルタ済みリスト長を返す
    fn filtered_list_len(&self) -> usize {
        match self.view {
            View::Ec2List => self.filtered_instances.len(),
            View::EcrList => self.ecr_filtered_repositories.len(),
            View::EcsList => self.ecs_filtered_clusters.len(),
            View::S3List => self.s3_filtered_buckets.len(),
            View::VpcList => self.filtered_vpcs.len(),
            View::SecretsList => self.filtered_secrets.len(),
            _ => 0,
        }
    }

    /// 現在のディテールビューのリスト長を返す
    fn detail_list_len(&self) -> usize {
        match self.view {
            View::Ec2Detail => {
                if self.detail_tab == DetailTab::Overview {
                    Ec2DetailField::ALL.len()
                } else {
                    self.selected_instance().map(|i| i.tags.len()).unwrap_or(0)
                }
            }
            View::EcrDetail => self.ecr_images.len(),
            View::EcsDetail => self.ecs_services.len(),
            View::S3Detail => self.s3_objects.len(),
            View::VpcDetail => self.subnets.len(),
            View::SecretsDetail => {
                if self.secrets_detail_tab == SecretsDetailTab::Tags {
                    self.secret_detail
                        .as_ref()
                        .map(|d| d.tags.len())
                        .unwrap_or(0)
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    /// フィルタを適用
    pub fn apply_filter(&mut self) {
        let filter_text = self.filter_input.value();
        match self.view {
            View::ServiceSelect => {
                let all_services: Vec<String> = crate::tui::views::service_select::SERVICE_NAMES
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
                self.filtered_service_names =
                    fuzzy_filter_items(&all_services, filter_text, 0, |s| vec![s.as_str()]);
                let len = self.filtered_service_names.len();
                if len > 0 && self.service_selected >= len {
                    self.service_selected = len - 1;
                }
                return;
            }
            View::Ec2List => {
                // name_index=1: [instance_id, name, instance_type, state]
                self.filtered_instances =
                    fuzzy_filter_items(&self.instances, filter_text, 1, |i| {
                        vec![
                            i.instance_id.as_str(),
                            i.name.as_str(),
                            i.instance_type.as_str(),
                            i.state.as_str(),
                        ]
                    });
            }
            View::EcrList => {
                // name_index=0: [repository_name, repository_uri]
                self.ecr_filtered_repositories =
                    fuzzy_filter_items(&self.ecr_repositories, filter_text, 0, |r| {
                        vec![r.repository_name.as_str(), r.repository_uri.as_str()]
                    });
            }
            View::EcsList => {
                // name_index=0: [cluster_name, status]
                self.ecs_filtered_clusters =
                    fuzzy_filter_items(&self.ecs_clusters, filter_text, 0, |c| {
                        vec![c.cluster_name.as_str(), c.status.as_str()]
                    });
            }
            View::S3List => {
                // name_index=0: [name]
                self.s3_filtered_buckets =
                    fuzzy_filter_items(&self.s3_buckets, filter_text, 0, |b| vec![b.name.as_str()]);
            }
            View::VpcList => {
                // name_index=1: [vpc_id, name, cidr_block]
                self.filtered_vpcs = fuzzy_filter_items(&self.vpcs, filter_text, 1, |v| {
                    vec![v.vpc_id.as_str(), v.name.as_str(), v.cidr_block.as_str()]
                });
            }
            View::SecretsList => {
                // name_index=0: [name, arn]
                self.filtered_secrets = fuzzy_filter_items(&self.secrets, filter_text, 0, |s| {
                    vec![s.name.as_str(), s.arn.as_str()]
                });
            }
            _ => {}
        }
        // フィルタ後にインデックスが範囲外にならないよう調整
        let len = self.filtered_list_len();
        if len > 0 && self.selected_index >= len {
            self.selected_index = len - 1;
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

    /// サービス固有のデータをクリアする
    fn clear_service_data(&mut self) {
        self.instances.clear();
        self.filtered_instances.clear();
        self.ecr_repositories.clear();
        self.ecr_filtered_repositories.clear();
        self.ecr_images.clear();
        self.ecs_clusters.clear();
        self.ecs_filtered_clusters.clear();
        self.ecs_services.clear();
        self.s3_buckets.clear();
        self.s3_filtered_buckets.clear();
        self.s3_objects.clear();
        self.s3_selected_bucket = None;
        self.s3_current_prefix.clear();
        self.vpcs.clear();
        self.filtered_vpcs.clear();
        self.subnets.clear();
        self.secrets.clear();
        self.filtered_secrets.clear();
        self.secret_detail = None;
    }

    /// リスト状態をリセットする（リストビューに遷移する際に呼ぶ）
    fn reset_list_state(&mut self) {
        self.selected_index = 0;
        self.filter_input.reset();
        self.mode = Mode::Normal;
    }

    /// 詳細状態をリセットする
    fn reset_detail_state(&mut self) {
        self.detail_tag_index = 0;
        self.detail_tab = DetailTab::Overview;
        self.secrets_detail_tab = SecretsDetailTab::Overview;
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
            Action::CopyId => self.copy_id(),
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
            Action::SwitchDetailTab => self.switch_detail_tab(),
            Action::FollowLink => self.handle_follow_link(),
            Action::Create => self.handle_create(),
            Action::Delete => self.handle_delete(),
            Action::Edit => self.handle_edit(),
            Action::FormSubmit => return self.handle_form_submit(),
            Action::FormCancel => self.mode = Mode::Normal,
            Action::FormNextField => self.handle_form_next_field(),
            Action::FormHandleInput(req) => self.handle_form_input(req),
            Action::DangerConfirmSubmit => return self.handle_danger_confirm_submit(),
            Action::DangerConfirmCancel => self.mode = Mode::Normal,
            Action::DangerConfirmHandleInput(req) => self.handle_danger_confirm_input(req),
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
            AppEvent::RepositoriesLoaded(Ok(repos)) => {
                let is_empty = repos.is_empty();
                self.ecr_repositories = repos;
                self.loading = false;
                self.apply_filter();
                if is_empty {
                    self.show_message(MessageLevel::Info, "Info", "No repositories found");
                }
            }
            AppEvent::RepositoriesLoaded(Err(e)) => {
                self.loading = false;
                self.show_message(MessageLevel::Error, "Error", e.to_string());
            }
            AppEvent::ImagesLoaded(Ok(images)) => {
                let is_empty = images.is_empty();
                self.ecr_images = images;
                self.loading = false;
                if is_empty {
                    self.show_message(MessageLevel::Info, "Info", "No images found");
                }
            }
            AppEvent::ImagesLoaded(Err(e)) => {
                self.loading = false;
                self.show_message(MessageLevel::Error, "Error", e.to_string());
            }
            AppEvent::ClustersLoaded(Ok(clusters)) => {
                let is_empty = clusters.is_empty();
                self.ecs_clusters = clusters;
                self.loading = false;
                self.apply_filter();
                if is_empty {
                    self.show_message(MessageLevel::Info, "Info", "No clusters found");
                }
            }
            AppEvent::ClustersLoaded(Err(e)) => {
                self.loading = false;
                self.show_message(MessageLevel::Error, "Error", e.to_string());
            }
            AppEvent::EcsServicesLoaded(Ok(services)) => {
                let is_empty = services.is_empty();
                self.ecs_services = services;
                self.loading = false;
                if is_empty {
                    self.show_message(MessageLevel::Info, "Info", "No services found");
                }
            }
            AppEvent::EcsServicesLoaded(Err(e)) => {
                self.loading = false;
                self.show_message(MessageLevel::Error, "Error", e.to_string());
            }
            AppEvent::BucketsLoaded(Ok(buckets)) => {
                let is_empty = buckets.is_empty();
                self.s3_buckets = buckets;
                self.loading = false;
                self.apply_filter();
                if is_empty {
                    self.show_message(MessageLevel::Info, "Info", "No buckets found");
                }
            }
            AppEvent::BucketsLoaded(Err(e)) => {
                self.loading = false;
                self.show_message(MessageLevel::Error, "Error", e.to_string());
            }
            AppEvent::ObjectsLoaded(Ok(objects)) => {
                self.s3_objects = objects;
                self.loading = false;
            }
            AppEvent::ObjectsLoaded(Err(e)) => {
                self.loading = false;
                self.show_message(MessageLevel::Error, "Error", e.to_string());
            }
            AppEvent::VpcsLoaded(Ok(vpcs)) => {
                let is_empty = vpcs.is_empty();
                self.vpcs = vpcs;
                self.loading = false;
                self.apply_filter();
                if is_empty {
                    self.show_message(MessageLevel::Info, "Info", "No VPCs found");
                }
            }
            AppEvent::VpcsLoaded(Err(e)) => {
                self.loading = false;
                self.show_message(MessageLevel::Error, "Error", e.to_string());
            }
            AppEvent::SubnetsLoaded(Ok(subnets)) => {
                let is_empty = subnets.is_empty();
                self.subnets = subnets;
                self.loading = false;
                if is_empty {
                    self.show_message(MessageLevel::Info, "Info", "No subnets found");
                }
            }
            AppEvent::SubnetsLoaded(Err(e)) => {
                self.loading = false;
                self.show_message(MessageLevel::Error, "Error", e.to_string());
            }
            AppEvent::SecretsLoaded(Ok(secrets)) => {
                let is_empty = secrets.is_empty();
                self.secrets = secrets;
                self.loading = false;
                self.apply_filter();
                if is_empty {
                    self.show_message(MessageLevel::Info, "Info", "No secrets found");
                }
            }
            AppEvent::SecretsLoaded(Err(e)) => {
                self.loading = false;
                self.show_message(MessageLevel::Error, "Error", e.to_string());
            }
            AppEvent::SecretDetailLoaded(Ok(detail)) => {
                self.secret_detail = Some(*detail);
                self.loading = false;
            }
            AppEvent::SecretDetailLoaded(Err(e)) => {
                self.loading = false;
                self.show_message(MessageLevel::Error, "Error", e.to_string());
            }
            AppEvent::NavigateVpcLoaded(Ok((vpcs, subnets))) => {
                self.vpcs = vpcs;
                self.filtered_vpcs = self.vpcs.clone();
                self.subnets = subnets;
                self.loading = false;
            }
            AppEvent::NavigateVpcLoaded(Err(e)) => {
                // ナビゲーション失敗時はスタックを巻き戻す
                if let Some(entry) = self.navigation_stack.pop() {
                    self.view = entry.view;
                    self.selected_index = entry.selected_index;
                    self.detail_tag_index = entry.detail_tag_index;
                    self.detail_tab = entry.detail_tab;
                }
                self.loading = false;
                self.show_message(MessageLevel::Error, "Error", e.to_string());
            }
            AppEvent::CrudCompleted(Ok(msg)) => {
                self.show_message(MessageLevel::Success, "Success", msg);
                self.loading = true;
            }
            AppEvent::CrudCompleted(Err(e)) => {
                self.show_message(MessageLevel::Error, "Error", e.to_string());
            }
        }
    }

    fn move_up(&mut self) {
        match self.view {
            View::ServiceSelect => {
                self.service_selected = self.service_selected.saturating_sub(1);
            }
            View::Ec2List
            | View::EcrList
            | View::EcsList
            | View::S3List
            | View::VpcList
            | View::SecretsList => {
                self.selected_index = self.selected_index.saturating_sub(1);
            }
            View::Ec2Detail
            | View::EcrDetail
            | View::EcsDetail
            | View::S3Detail
            | View::VpcDetail
            | View::SecretsDetail => {
                self.detail_tag_index = self.detail_tag_index.saturating_sub(1);
            }
        }
    }

    fn move_down(&mut self) {
        match self.view {
            View::ServiceSelect => {
                let max = self.filtered_service_names.len().saturating_sub(1);
                if self.service_selected < max {
                    self.service_selected += 1;
                }
            }
            View::Ec2List
            | View::EcrList
            | View::EcsList
            | View::S3List
            | View::VpcList
            | View::SecretsList => {
                let max = self.filtered_list_len().saturating_sub(1);
                if self.selected_index < max {
                    self.selected_index += 1;
                }
            }
            View::Ec2Detail
            | View::EcrDetail
            | View::EcsDetail
            | View::S3Detail
            | View::VpcDetail
            | View::SecretsDetail => {
                let max = self.detail_list_len().saturating_sub(1);
                if self.detail_tag_index < max {
                    self.detail_tag_index += 1;
                }
            }
        }
    }

    fn move_to_top(&mut self) {
        match self.view {
            View::ServiceSelect => self.service_selected = 0,
            View::Ec2List
            | View::EcrList
            | View::EcsList
            | View::S3List
            | View::VpcList
            | View::SecretsList => self.selected_index = 0,
            View::Ec2Detail
            | View::EcrDetail
            | View::EcsDetail
            | View::S3Detail
            | View::VpcDetail
            | View::SecretsDetail => self.detail_tag_index = 0,
        }
    }

    fn move_to_bottom(&mut self) {
        match self.view {
            View::ServiceSelect => {
                self.service_selected = self.filtered_service_names.len().saturating_sub(1);
            }
            View::Ec2List
            | View::EcrList
            | View::EcsList
            | View::S3List
            | View::VpcList
            | View::SecretsList => {
                self.selected_index = self.filtered_list_len().saturating_sub(1);
            }
            View::Ec2Detail
            | View::EcrDetail
            | View::EcsDetail
            | View::S3Detail
            | View::VpcDetail
            | View::SecretsDetail => {
                self.detail_tag_index = self.detail_list_len().saturating_sub(1);
            }
        }
    }

    fn half_page_up(&mut self) {
        match self.view {
            View::ServiceSelect => {
                self.service_selected = self.service_selected.saturating_sub(10);
            }
            View::Ec2List
            | View::EcrList
            | View::EcsList
            | View::S3List
            | View::VpcList
            | View::SecretsList => {
                self.selected_index = self.selected_index.saturating_sub(10);
            }
            View::Ec2Detail
            | View::EcrDetail
            | View::EcsDetail
            | View::S3Detail
            | View::VpcDetail
            | View::SecretsDetail => {
                self.detail_tag_index = self.detail_tag_index.saturating_sub(10);
            }
        }
    }

    fn half_page_down(&mut self) {
        match self.view {
            View::ServiceSelect => {
                let max = self.filtered_service_names.len().saturating_sub(1);
                self.service_selected = (self.service_selected + 10).min(max);
            }
            View::Ec2List
            | View::EcrList
            | View::EcsList
            | View::S3List
            | View::VpcList
            | View::SecretsList => {
                let max = self.filtered_list_len().saturating_sub(1);
                self.selected_index = (self.selected_index + 10).min(max);
            }
            View::Ec2Detail
            | View::EcrDetail
            | View::EcsDetail
            | View::S3Detail
            | View::VpcDetail
            | View::SecretsDetail => {
                let max = self.detail_list_len().saturating_sub(1);
                self.detail_tag_index = (self.detail_tag_index + 10).min(max);
            }
        }
    }

    fn handle_enter(&mut self) {
        match self.view {
            View::ServiceSelect => {
                let Some(selected_name) = self.filtered_service_names.get(self.service_selected)
                else {
                    return;
                };
                let view = match selected_name.as_str() {
                    "EC2" => View::Ec2List,
                    "ECR" => View::EcrList,
                    "ECS" => View::EcsList,
                    "S3" => View::S3List,
                    "VPC" => View::VpcList,
                    "Secrets Manager" => View::SecretsList,
                    _ => return,
                };
                self.view = view;
                self.filter_input.reset();
                self.reset_list_state();
                self.loading = true;
            }
            View::Ec2List => {
                if !self.filtered_instances.is_empty() {
                    self.view = View::Ec2Detail;
                    self.reset_detail_state();
                }
            }
            View::EcrList => {
                if !self.ecr_filtered_repositories.is_empty() {
                    self.view = View::EcrDetail;
                    self.reset_detail_state();
                    self.loading = true;
                }
            }
            View::EcsList => {
                if !self.ecs_filtered_clusters.is_empty() {
                    self.view = View::EcsDetail;
                    self.reset_detail_state();
                    self.loading = true;
                }
            }
            View::S3List => {
                if let Some(bucket) = self.s3_filtered_buckets.get(self.selected_index) {
                    self.s3_selected_bucket = Some(bucket.name.clone());
                    self.s3_current_prefix.clear();
                    self.view = View::S3Detail;
                    self.reset_detail_state();
                    self.loading = true;
                }
            }
            View::VpcList => {
                if !self.filtered_vpcs.is_empty() {
                    self.view = View::VpcDetail;
                    self.reset_detail_state();
                    self.loading = true;
                }
            }
            View::SecretsList => {
                if !self.filtered_secrets.is_empty() {
                    self.view = View::SecretsDetail;
                    self.reset_detail_state();
                    self.loading = true;
                }
            }
            View::S3Detail => {
                // プレフィックス(ディレクトリ)の場合は中に入る
                if let Some(obj) = self.s3_objects.get(self.detail_tag_index)
                    && obj.is_prefix
                {
                    self.s3_current_prefix = obj.key.clone();
                    self.detail_tag_index = 0;
                    self.loading = true;
                }
            }
            _ => {}
        }
    }

    fn handle_back(&mut self) {
        // Help/Message mode はモード変更のみ（ビュー遷移しない）
        match self.mode {
            Mode::Help => {
                self.mode = Mode::Normal;
                return;
            }
            Mode::Message => {
                self.dismiss_message();
                return;
            }
            _ => {}
        }
        match self.view {
            View::ServiceSelect => {
                self.should_quit = true;
            }
            View::Ec2List
            | View::EcrList
            | View::EcsList
            | View::S3List
            | View::VpcList
            | View::SecretsList => {
                self.view = View::ServiceSelect;
                self.clear_service_data();
                self.reset_list_state();
            }
            View::Ec2Detail => {
                self.navigation_stack.clear();
                self.view = View::Ec2List;
            }
            View::EcrDetail => {
                self.view = View::EcrList;
                self.ecr_images.clear();
            }
            View::EcsDetail => {
                self.view = View::EcsList;
                self.ecs_services.clear();
            }
            View::S3Detail => {
                if self.s3_current_prefix.is_empty() {
                    self.view = View::S3List;
                    self.s3_objects.clear();
                    self.s3_selected_bucket = None;
                } else {
                    // 一つ上のプレフィックスに移動
                    let trimmed = self.s3_current_prefix.trim_end_matches('/');
                    if let Some(pos) = trimmed.rfind('/') {
                        self.s3_current_prefix = trimmed[..=pos].to_string();
                    } else {
                        self.s3_current_prefix.clear();
                    }
                    self.detail_tag_index = 0;
                    self.loading = true;
                }
            }
            View::VpcDetail => {
                if let Some(entry) = self.navigation_stack.pop() {
                    // ナビゲーションスタックから戻る
                    self.view = entry.view;
                    self.selected_index = entry.selected_index;
                    self.detail_tag_index = entry.detail_tag_index;
                    self.detail_tab = entry.detail_tab;
                    self.subnets.clear();
                } else {
                    self.view = View::VpcList;
                    self.subnets.clear();
                }
            }
            View::SecretsDetail => {
                self.view = View::SecretsList;
                self.secret_detail = None;
            }
        }
    }

    fn copy_id(&self) {
        match self.view {
            View::Ec2List | View::Ec2Detail => {
                if let Some(instance) = self.selected_instance() {
                    let _ = cli_clipboard::set_contents(instance.instance_id.clone());
                }
            }
            View::EcrList => {
                if let Some(repo) = self.ecr_filtered_repositories.get(self.selected_index) {
                    let _ = cli_clipboard::set_contents(repo.repository_uri.clone());
                }
            }
            View::EcrDetail => {
                if let Some(image) = self.ecr_images.get(self.detail_tag_index) {
                    let _ = cli_clipboard::set_contents(image.image_digest.clone());
                }
            }
            View::VpcList => {
                if let Some(vpc) = self.filtered_vpcs.get(self.selected_index) {
                    let _ = cli_clipboard::set_contents(vpc.vpc_id.clone());
                }
            }
            View::SecretsList => {
                if let Some(secret) = self.filtered_secrets.get(self.selected_index) {
                    let _ = cli_clipboard::set_contents(secret.arn.clone());
                }
            }
            View::SecretsDetail => {
                if let Some(detail) = &self.secret_detail {
                    let _ = cli_clipboard::set_contents(detail.arn.clone());
                }
            }
            View::S3List => {
                if let Some(bucket) = self.s3_filtered_buckets.get(self.selected_index) {
                    let _ = cli_clipboard::set_contents(bucket.name.clone());
                }
            }
            _ => {}
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

    fn switch_detail_tab(&mut self) {
        self.detail_tag_index = 0;
        match self.view {
            View::Ec2Detail => {
                self.detail_tab = match self.detail_tab {
                    DetailTab::Overview => DetailTab::Tags,
                    DetailTab::Tags => DetailTab::Overview,
                };
            }
            View::SecretsDetail => {
                self.secrets_detail_tab = match self.secrets_detail_tab {
                    SecretsDetailTab::Overview => SecretsDetailTab::Tags,
                    SecretsDetailTab::Tags => SecretsDetailTab::Overview,
                };
            }
            _ => {}
        }
    }

    /// EC2 Detail Overviewタブでリンクをフォローする
    fn handle_follow_link(&mut self) {
        if self.view != View::Ec2Detail || self.detail_tab != DetailTab::Overview {
            return;
        }

        let Some(instance) = self.selected_instance().cloned() else {
            return;
        };

        let Some(field) = Ec2DetailField::ALL.get(self.detail_tag_index) else {
            return;
        };

        let target_id = match field {
            Ec2DetailField::VpcId => instance.vpc_id.clone(),
            Ec2DetailField::SubnetId => instance.subnet_id.clone(),
        };

        let Some(target_id) = target_id else {
            return; // フィールドが "-" の場合は何もしない
        };

        if target_id.is_empty() {
            return;
        }

        // ナビゲーションスタックに現在の状態を保存
        self.navigation_stack.push(NavigationEntry {
            view: self.view.clone(),
            selected_index: self.selected_index,
            detail_tag_index: self.detail_tag_index,
            detail_tab: self.detail_tab.clone(),
            label: instance.instance_id.clone(),
        });

        // VPC詳細画面に遷移
        self.view = View::VpcDetail;
        self.detail_tag_index = 0;
        self.loading = true;
        // navigate_target_id を保持して main.rs 側で使う
        self.navigate_target_id = Some(target_id);
    }

    /// Create操作のハンドリング
    fn handle_create(&mut self) {
        match self.view {
            View::S3List => {
                self.mode = Mode::Form(FormContext {
                    kind: FormKind::CreateS3Bucket,
                    fields: vec![FormField {
                        label: "Bucket Name".to_string(),
                        input: Input::default(),
                        required: true,
                    }],
                    focused_field: 0,
                });
            }
            View::SecretsList => {
                self.mode = Mode::Form(FormContext {
                    kind: FormKind::CreateSecret,
                    fields: vec![
                        FormField {
                            label: "Name".to_string(),
                            input: Input::default(),
                            required: true,
                        },
                        FormField {
                            label: "Value".to_string(),
                            input: Input::default(),
                            required: true,
                        },
                        FormField {
                            label: "Description".to_string(),
                            input: Input::default(),
                            required: false,
                        },
                    ],
                    focused_field: 0,
                });
            }
            _ => {}
        }
    }

    /// Delete操作のハンドリング
    fn handle_delete(&mut self) {
        match self.view {
            View::Ec2List => {
                if !self.can_delete("ec2") {
                    self.show_message(
                        MessageLevel::Error,
                        "Permission Denied",
                        "Delete not allowed. Use --allow-delete=ec2 or --allow-delete",
                    );
                    return;
                }
                if let Some(instance) = self.selected_instance() {
                    let id = instance.instance_id.clone();
                    self.mode = Mode::DangerConfirm(DangerConfirmContext {
                        action: DangerAction::TerminateEc2(id),
                        input: Input::default(),
                    });
                }
            }
            View::S3List => {
                if !self.can_delete("s3") {
                    self.show_message(
                        MessageLevel::Error,
                        "Permission Denied",
                        "Delete not allowed. Use --allow-delete=s3 or --allow-delete",
                    );
                    return;
                }
                if let Some(bucket) = self.s3_filtered_buckets.get(self.selected_index) {
                    let name = bucket.name.clone();
                    self.mode = Mode::DangerConfirm(DangerConfirmContext {
                        action: DangerAction::DeleteS3Bucket(name),
                        input: Input::default(),
                    });
                }
            }
            View::S3Detail => {
                if !self.can_delete("s3") {
                    self.show_message(
                        MessageLevel::Error,
                        "Permission Denied",
                        "Delete not allowed. Use --allow-delete=s3 or --allow-delete",
                    );
                    return;
                }
                if let Some(obj) = self.s3_objects.get(self.detail_tag_index)
                    && !obj.is_prefix
                {
                    let bucket = self.s3_selected_bucket.clone().unwrap_or_default();
                    let key = obj.key.clone();
                    self.mode = Mode::DangerConfirm(DangerConfirmContext {
                        action: DangerAction::DeleteS3Object { bucket, key },
                        input: Input::default(),
                    });
                }
            }
            View::SecretsList => {
                if !self.can_delete("secretsmanager") {
                    self.show_message(
                        MessageLevel::Error,
                        "Permission Denied",
                        "Delete not allowed. Use --allow-delete=secretsmanager or --allow-delete",
                    );
                    return;
                }
                if let Some(secret) = self.filtered_secrets.get(self.selected_index) {
                    let name = secret.name.clone();
                    self.mode = Mode::DangerConfirm(DangerConfirmContext {
                        action: DangerAction::DeleteSecret(name),
                        input: Input::default(),
                    });
                }
            }
            _ => {}
        }
    }

    /// Edit操作のハンドリング
    fn handle_edit(&mut self) {
        if self.view == View::SecretsDetail
            && let Some(detail) = &self.secret_detail
        {
            self.mode = Mode::Form(FormContext {
                kind: FormKind::UpdateSecretValue,
                fields: vec![FormField {
                    label: format!("New value for '{}'", detail.name),
                    input: Input::default(),
                    required: true,
                }],
                focused_field: 0,
            });
        }
    }

    /// FormSubmitのハンドリング。dispatchの戻り値としてConfirmActionの代わりにNoneを返す。
    /// main.rs側でCRUD APIを呼ぶためにFormContextを保持する。
    fn handle_form_submit(&mut self) -> Option<ConfirmAction> {
        let Mode::Form(ctx) = &self.mode else {
            return None;
        };

        // 必須フィールドのバリデーション
        for field in &ctx.fields {
            if field.required && field.input.value().is_empty() {
                self.show_message(
                    MessageLevel::Error,
                    "Validation Error",
                    format!("'{}' is required", field.label),
                );
                return None;
            }
        }

        // FormContextを取り出してNormalに戻す
        // main.rs側でpending_formをチェックしてAPI呼び出しを行う
        let Mode::Form(ctx) = std::mem::replace(&mut self.mode, Mode::Normal) else {
            return None;
        };
        self.pending_form = Some(ctx);
        self.loading = true;
        None
    }

    /// フォームの次のフィールドにフォーカスを移動
    fn handle_form_next_field(&mut self) {
        if let Mode::Form(ctx) = &mut self.mode {
            ctx.focused_field = (ctx.focused_field + 1) % ctx.fields.len();
        }
    }

    /// フォーム入力のハンドリング
    fn handle_form_input(&mut self, req: tui_input::InputRequest) {
        if let Mode::Form(ctx) = &mut self.mode
            && let Some(field) = ctx.fields.get_mut(ctx.focused_field)
        {
            field.input.handle(req);
        }
    }

    /// DangerConfirmSubmitのハンドリング
    fn handle_danger_confirm_submit(&mut self) -> Option<ConfirmAction> {
        let Mode::DangerConfirm(ctx) = &self.mode else {
            return None;
        };

        if ctx.input.value() != ctx.action.confirm_text() {
            return None;
        }

        let Mode::DangerConfirm(ctx) = std::mem::replace(&mut self.mode, Mode::Normal) else {
            return None;
        };
        self.pending_danger_action = Some(ctx.action);
        self.loading = true;
        None
    }

    /// DangerConfirm入力のハンドリング
    fn handle_danger_confirm_input(&mut self, req: tui_input::InputRequest) {
        if let Mode::DangerConfirm(ctx) = &mut self.mode {
            ctx.input.handle(req);
        }
    }

    /// パンくずリスト文字列を生成する
    pub fn breadcrumb(&self) -> Option<String> {
        if self.navigation_stack.is_empty() {
            return None;
        }

        let mut parts: Vec<&str> = self
            .navigation_stack
            .iter()
            .map(|e| e.label.as_str())
            .collect();

        // 現在のビューのラベルを追加
        let current_label = match self.view {
            View::VpcDetail => self
                .filtered_vpcs
                .first()
                .map(|v| v.vpc_id.as_str())
                .unwrap_or("VPC"),
            _ => "",
        };
        if !current_label.is_empty() {
            parts.push(current_label);
        }

        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" > "))
        }
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
    fn apply_filter_returns_all_instances_when_empty_filter() {
        let mut app = App::new("dev".to_string(), None);
        app.view = View::Ec2List;
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
        let mut app = App::new("dev".to_string(), None);
        app.view = View::Ec2List;
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
        let mut app = App::new("dev".to_string(), None);
        app.view = View::Ec2List;
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
        let mut app = App::new("dev".to_string(), None);
        app.show_message(MessageLevel::Error, "Error", "Something failed");
        assert_eq!(app.mode, Mode::Message);
        assert!(app.message.is_some());
        let msg = app.message.as_ref().unwrap();
        assert_eq!(msg.level, MessageLevel::Error);
        assert_eq!(msg.title, "Error");
    }

    #[test]
    fn dismiss_message_clears_message_when_called() {
        let mut app = App::new("dev".to_string(), None);
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
        let mut app = App::new("dev".to_string(), None);
        let result = app.dispatch(Action::Quit);
        assert!(app.should_quit);
        assert!(result.is_none());
    }

    #[test]
    fn dispatch_returns_none_and_decrements_selected_index_when_move_up_in_ec2_list() {
        let mut app = App::new("dev".to_string(), None);
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
        let mut app = App::new("dev".to_string(), None);
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
        let mut app = App::new("dev".to_string(), None);
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
        let mut app = App::new("dev".to_string(), None);
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
        let mut app = App::new("dev".to_string(), None);
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
        let mut app = App::new("dev".to_string(), None);
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
        let mut app = App::new("dev".to_string(), None);
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
        let mut app = App::new("dev".to_string(), None);
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
    fn dispatch_returns_none_and_switches_to_ec2_list_when_enter_in_service_select() {
        let mut app = App::new("dev".to_string(), None);
        app.view = View::ServiceSelect;
        app.service_selected = 0; // EC2
        app.dispatch(Action::Enter);
        assert_eq!(app.view, View::Ec2List);
        assert!(app.loading);
    }

    #[test]
    fn dispatch_returns_none_and_switches_to_detail_when_enter_in_ec2_list() {
        let mut app = App::new("dev".to_string(), None);
        app.view = View::Ec2List;
        app.filtered_instances = vec![create_test_instance("i-001", "web", InstanceState::Running)];
        app.dispatch(Action::Enter);
        assert_eq!(app.view, View::Ec2Detail);
    }

    #[test]
    fn dispatch_returns_none_and_stays_on_ec2_list_when_enter_with_empty_instances() {
        let mut app = App::new("dev".to_string(), None);
        app.view = View::Ec2List;
        app.dispatch(Action::Enter);
        assert_eq!(app.view, View::Ec2List);
    }

    #[test]
    fn dispatch_returns_none_and_goes_back_to_ec2_list_when_back_in_ec2_detail() {
        let mut app = App::new("dev".to_string(), None);
        app.view = View::Ec2Detail;
        app.dispatch(Action::Back);
        assert_eq!(app.view, View::Ec2List);
    }

    #[test]
    fn dispatch_returns_none_and_goes_to_service_select_when_back_in_ec2_list() {
        let mut app = App::new("dev".to_string(), None);
        app.view = View::Ec2List;
        app.instances = vec![create_test_instance("i-001", "web", InstanceState::Running)];
        app.filtered_instances = vec![create_test_instance("i-001", "web", InstanceState::Running)];
        app.dispatch(Action::Back);
        assert_eq!(app.view, View::ServiceSelect);
        assert!(app.instances.is_empty());
        assert!(app.filtered_instances.is_empty());
    }

    #[test]
    fn dispatch_returns_none_and_sets_should_quit_when_back_in_service_select() {
        let mut app = App::new("dev".to_string(), None);
        app.view = View::ServiceSelect;
        app.dispatch(Action::Back);
        assert!(app.should_quit);
    }

    #[test]
    fn dispatch_returns_none_and_sets_normal_mode_when_back_in_help() {
        let mut app = App::new("dev".to_string(), None);
        app.mode = Mode::Help;
        app.view = View::Ec2List;
        app.dispatch(Action::Back);
        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.view, View::Ec2List);
    }

    #[test]
    fn dispatch_returns_none_and_dismisses_message_when_back_in_message() {
        let mut app = App::new("dev".to_string(), None);
        app.view = View::Ec2List;
        app.show_message(MessageLevel::Info, "Info", "test");
        app.dispatch(Action::Back);
        assert!(app.message.is_none());
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn dispatch_returns_none_and_sets_loading_when_refresh() {
        let mut app = App::new("dev".to_string(), None);
        app.dispatch(Action::Refresh);
        assert!(app.loading);
    }

    #[test]
    fn dispatch_returns_none_and_sets_filter_mode_when_start_filter() {
        let mut app = App::new("dev".to_string(), None);
        app.dispatch(Action::StartFilter);
        assert_eq!(app.mode, Mode::Filter);
    }

    #[test]
    fn dispatch_returns_none_and_sets_normal_mode_when_confirm_filter() {
        let mut app = App::new("dev".to_string(), None);
        app.mode = Mode::Filter;
        app.dispatch(Action::ConfirmFilter);
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn dispatch_returns_none_and_clears_filter_when_cancel_filter() {
        let mut app = App::new("dev".to_string(), None);
        app.view = View::Ec2List;
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
        let mut app = App::new("dev".to_string(), None);
        app.view = View::Ec2List;
        app.instances = vec![create_test_instance("i-001", "web", InstanceState::Running)];
        app.dispatch(Action::FilterHandleInput(InputRequest::InsertChar('w')));
        assert_eq!(app.filter_input.value(), "w");
        assert_eq!(app.filtered_instances.len(), 1);
    }

    #[test]
    fn dispatch_returns_none_and_deletes_char_when_filter_handle_input_delete() {
        use tui_input::InputRequest;
        let mut app = App::new("dev".to_string(), None);
        app.view = View::Ec2List;
        app.filter_input = Input::from("web");
        app.instances = vec![create_test_instance("i-001", "web", InstanceState::Running)];
        app.dispatch(Action::FilterHandleInput(InputRequest::DeletePrevChar));
        assert_eq!(app.filter_input.value(), "we");
    }

    #[test]
    fn dispatch_returns_none_and_sets_confirm_stop_when_start_stop_on_running() {
        let mut app = App::new("dev".to_string(), None);
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
        let mut app = App::new("dev".to_string(), None);
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
        let mut app = App::new("dev".to_string(), None);
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
        let mut app = App::new("dev".to_string(), None);
        app.mode = Mode::Confirm(ConfirmAction::Stop("i-001".to_string()));
        let result = app.dispatch(Action::ConfirmYes);
        assert_eq!(result, Some(ConfirmAction::Stop("i-001".to_string())));
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn dispatch_returns_none_and_sets_normal_when_confirm_no() {
        let mut app = App::new("dev".to_string(), None);
        app.mode = Mode::Confirm(ConfirmAction::Stop("i-001".to_string()));
        let result = app.dispatch(Action::ConfirmNo);
        assert!(result.is_none());
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn dispatch_returns_none_and_dismisses_when_dismiss_message() {
        let mut app = App::new("dev".to_string(), None);
        app.show_message(MessageLevel::Info, "Info", "test");
        app.dispatch(Action::DismissMessage);
        assert!(app.message.is_none());
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn dispatch_returns_none_and_sets_help_mode_when_show_help() {
        let mut app = App::new("dev".to_string(), None);
        app.dispatch(Action::ShowHelp);
        assert_eq!(app.mode, Mode::Help);
    }

    #[test]
    fn dispatch_returns_none_and_toggles_tab_when_switch_detail_tab() {
        let mut app = App::new("dev".to_string(), None);
        app.view = View::Ec2Detail;
        assert_eq!(app.detail_tab, DetailTab::Overview);
        app.dispatch(Action::SwitchDetailTab);
        assert_eq!(app.detail_tab, DetailTab::Tags);
        app.dispatch(Action::SwitchDetailTab);
        assert_eq!(app.detail_tab, DetailTab::Overview);
    }

    #[test]
    fn dispatch_returns_none_when_noop() {
        let mut app = App::new("dev".to_string(), None);
        let result = app.dispatch(Action::Noop);
        assert!(result.is_none());
    }

    // ──────────────────────────────────────────────
    // handle_event テスト
    // ──────────────────────────────────────────────

    #[test]
    fn handle_event_sets_instances_when_instances_loaded_ok() {
        let mut app = App::new("dev".to_string(), None);
        app.view = View::Ec2List;
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
        let mut app = App::new("dev".to_string(), None);
        app.view = View::Ec2List;
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
        let mut app = App::new("dev".to_string(), None);
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
        let mut app = App::new("dev".to_string(), None);
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
        let mut app = App::new("dev".to_string(), None);
        app.handle_event(AppEvent::ActionCompleted(Err(AppError::AwsApi(
            "start failed".to_string(),
        ))));
        assert_eq!(app.mode, Mode::Message);
        let msg = app.message.as_ref().unwrap();
        assert_eq!(msg.level, MessageLevel::Error);
        assert!(msg.body.contains("start failed"));
    }

    // ──────────────────────────────────────────────
    // detail_tag_index テスト
    // ──────────────────────────────────────────────

    #[test]
    fn dispatch_returns_none_and_moves_detail_tag_index_when_move_down_in_ec2_detail() {
        let mut app = App::new("dev".to_string(), None);
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
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn dispatch_returns_none_and_moves_detail_tag_index_when_move_up_in_ec2_detail() {
        let mut app = App::new("dev".to_string(), None);
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
        let mut app = App::new("dev".to_string(), None);
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
        let mut app = App::new("dev".to_string(), None);
        app.view = View::Ec2Detail;
        app.detail_tag_index = 3;
        app.dispatch(Action::SwitchDetailTab);
        assert_eq!(app.detail_tag_index, 0);
    }

    // ──────────────────────────────────────────────
    // ServiceSelect テスト
    // ──────────────────────────────────────────────

    #[test]
    fn dispatch_returns_none_and_increments_service_selected_when_move_down_in_service_select() {
        let mut app = App::new("dev".to_string(), None);
        app.view = View::ServiceSelect;
        app.service_selected = 0;
        app.dispatch(Action::MoveDown);
        assert_eq!(app.service_selected, 1);
    }

    #[test]
    fn dispatch_returns_none_and_decrements_service_selected_when_move_up_in_service_select() {
        let mut app = App::new("dev".to_string(), None);
        app.view = View::ServiceSelect;
        app.service_selected = 2;
        app.dispatch(Action::MoveUp);
        assert_eq!(app.service_selected, 1);
    }

    #[test]
    fn dispatch_returns_none_and_enters_ecr_list_when_enter_in_service_select_ecr() {
        let mut app = App::new("dev".to_string(), None);
        app.view = View::ServiceSelect;
        app.service_selected = 1; // ECR
        app.dispatch(Action::Enter);
        assert_eq!(app.view, View::EcrList);
        assert!(app.loading);
    }

    #[test]
    fn dispatch_returns_none_and_enters_s3_list_when_enter_in_service_select_s3() {
        let mut app = App::new("dev".to_string(), None);
        app.view = View::ServiceSelect;
        app.service_selected = 3; // S3
        app.dispatch(Action::Enter);
        assert_eq!(app.view, View::S3List);
        assert!(app.loading);
    }

    // ──────────────────────────────────────────────
    // can_delete テスト
    // ──────────────────────────────────────────────

    #[test]
    fn can_delete_returns_false_when_default_permissions() {
        let app = App::new("dev".to_string(), None);
        assert!(!app.can_delete("ec2"));
        assert!(!app.can_delete("s3"));
    }

    #[test]
    fn can_delete_returns_true_when_all_permissions() {
        let app = App::with_delete_permissions("dev".to_string(), None, DeletePermissions::All);
        assert!(app.can_delete("ec2"));
        assert!(app.can_delete("s3"));
        assert!(app.can_delete("secrets"));
    }

    #[test]
    fn can_delete_returns_true_when_service_permitted() {
        let app = App::with_delete_permissions(
            "dev".to_string(),
            None,
            DeletePermissions::Services(vec!["ec2".to_string(), "s3".to_string()]),
        );
        assert!(app.can_delete("ec2"));
        assert!(app.can_delete("s3"));
        assert!(!app.can_delete("ecs"));
    }

    // ──────────────────────────────────────────────
    // fuzzy filter 統合テスト
    // ──────────────────────────────────────────────

    #[test]
    fn apply_filter_returns_fuzzy_matches_when_ec2_filter_set() {
        let mut app = App::new("dev".to_string(), None);
        app.view = View::Ec2List;
        app.instances = vec![
            create_test_instance("i-001", "web-server", InstanceState::Running),
            create_test_instance("i-002", "database", InstanceState::Stopped),
            create_test_instance("i-003", "worker", InstanceState::Running),
        ];
        app.filter_input = Input::from("wbsr");
        app.apply_filter();
        assert_eq!(app.filtered_instances.len(), 1);
        assert_eq!(app.filtered_instances[0].name, "web-server");
    }

    #[test]
    fn apply_filter_returns_sorted_by_score_when_multiple_matches() {
        use crate::aws::s3_model::Bucket;
        let mut app = App::new("dev".to_string(), None);
        app.view = View::S3List;
        app.s3_buckets = vec![
            Bucket {
                name: "my-logs-backup".to_string(),
                creation_date: None,
            },
            Bucket {
                name: "logs".to_string(),
                creation_date: None,
            },
            Bucket {
                name: "access-logs-archive".to_string(),
                creation_date: None,
            },
        ];
        app.filter_input = Input::from("logs");
        app.apply_filter();
        assert!(app.s3_filtered_buckets.len() >= 2);
        // Exact match "logs" should score highest
        assert_eq!(app.s3_filtered_buckets[0].name, "logs");
    }

    // ──────────────────────────────────────────────
    // ナビゲーションテスト
    // ──────────────────────────────────────────────

    fn app_with_ec2_detail() -> App {
        let mut app = App::new("dev".to_string(), None);
        app.view = View::Ec2Detail;
        app.detail_tab = DetailTab::Overview;
        let instance = create_test_instance("i-001", "web", InstanceState::Running);
        let mut instance = instance;
        instance.vpc_id = Some("vpc-abc123".to_string());
        instance.subnet_id = Some("subnet-def456".to_string());
        app.instances = vec![instance.clone()];
        app.filtered_instances = vec![instance];
        app.selected_index = 0;
        app
    }

    #[test]
    fn dispatch_returns_vpc_detail_when_follow_link_on_vpc_field() {
        let mut app = app_with_ec2_detail();
        app.detail_tag_index = 0; // VpcId field
        app.dispatch(Action::FollowLink);
        assert_eq!(app.view, View::VpcDetail);
        assert!(app.loading);
        assert_eq!(app.navigate_target_id, Some("vpc-abc123".to_string()));
        assert_eq!(app.navigation_stack.len(), 1);
        assert_eq!(app.navigation_stack[0].view, View::Ec2Detail);
        assert_eq!(app.navigation_stack[0].label, "i-001");
    }

    #[test]
    fn dispatch_returns_vpc_detail_when_follow_link_on_subnet_field() {
        let mut app = app_with_ec2_detail();
        app.detail_tag_index = 1; // SubnetId field
        app.dispatch(Action::FollowLink);
        assert_eq!(app.view, View::VpcDetail);
        assert!(app.loading);
        assert_eq!(app.navigate_target_id, Some("subnet-def456".to_string()));
        assert_eq!(app.navigation_stack.len(), 1);
    }

    #[test]
    fn dispatch_returns_no_change_when_follow_link_without_vpc_id() {
        let mut app = app_with_ec2_detail();
        // Clear vpc_id
        app.instances[0].vpc_id = None;
        app.filtered_instances[0].vpc_id = None;
        app.detail_tag_index = 0; // VpcId field
        app.dispatch(Action::FollowLink);
        assert_eq!(app.view, View::Ec2Detail); // no navigation
        assert!(app.navigation_stack.is_empty());
    }

    #[test]
    fn dispatch_returns_no_change_when_follow_link_on_tags_tab() {
        let mut app = app_with_ec2_detail();
        app.detail_tab = DetailTab::Tags;
        app.dispatch(Action::FollowLink);
        assert_eq!(app.view, View::Ec2Detail); // no navigation
        assert!(app.navigation_stack.is_empty());
    }

    #[test]
    fn handle_back_returns_ec2_detail_when_vpc_detail_with_nav_stack() {
        let mut app = app_with_ec2_detail();
        app.detail_tag_index = 0;
        app.dispatch(Action::FollowLink);
        assert_eq!(app.view, View::VpcDetail);

        // Go back
        app.dispatch(Action::Back);
        assert_eq!(app.view, View::Ec2Detail);
        assert!(app.navigation_stack.is_empty());
    }

    #[test]
    fn handle_back_returns_vpc_list_when_vpc_detail_without_nav_stack() {
        let mut app = App::new("dev".to_string(), None);
        app.view = View::VpcDetail;
        app.dispatch(Action::Back);
        assert_eq!(app.view, View::VpcList);
    }

    #[test]
    fn handle_back_clears_nav_stack_when_ec2_detail_back() {
        let mut app = app_with_ec2_detail();
        app.navigation_stack.push(NavigationEntry {
            view: View::Ec2Detail,
            selected_index: 0,
            detail_tag_index: 0,
            detail_tab: DetailTab::Overview,
            label: "i-001".to_string(),
        });
        app.dispatch(Action::Back);
        assert_eq!(app.view, View::Ec2List);
        assert!(app.navigation_stack.is_empty());
    }

    #[test]
    fn breadcrumb_returns_none_when_nav_stack_empty() {
        let app = app_with_ec2_detail();
        assert!(app.breadcrumb().is_none());
    }

    #[test]
    fn breadcrumb_returns_path_when_nav_stack_has_entries() {
        let mut app = app_with_ec2_detail();
        app.navigation_stack.push(NavigationEntry {
            view: View::Ec2Detail,
            selected_index: 0,
            detail_tag_index: 0,
            detail_tab: DetailTab::Overview,
            label: "i-001".to_string(),
        });
        app.view = View::VpcDetail;
        app.filtered_vpcs = vec![crate::aws::vpc_model::Vpc {
            vpc_id: "vpc-abc123".to_string(),
            name: "main-vpc".to_string(),
            cidr_block: "10.0.0.0/16".to_string(),
            state: "available".to_string(),
            is_default: false,
            owner_id: "123456789012".to_string(),
            tags: HashMap::new(),
        }];
        let breadcrumb = app.breadcrumb().unwrap();
        assert_eq!(breadcrumb, "i-001 > vpc-abc123");
    }

    #[test]
    fn navigate_vpc_loaded_returns_vpc_data_when_success() {
        let mut app = app_with_ec2_detail();
        app.detail_tag_index = 0;
        app.dispatch(Action::FollowLink);
        assert!(app.loading);

        let vpcs = vec![crate::aws::vpc_model::Vpc {
            vpc_id: "vpc-abc123".to_string(),
            name: "main-vpc".to_string(),
            cidr_block: "10.0.0.0/16".to_string(),
            state: "available".to_string(),
            is_default: false,
            owner_id: "123456789012".to_string(),
            tags: HashMap::new(),
        }];
        let subnets = vec![crate::aws::vpc_model::Subnet {
            subnet_id: "subnet-001".to_string(),
            vpc_id: "vpc-abc123".to_string(),
            name: "public-1a".to_string(),
            cidr_block: "10.0.1.0/24".to_string(),
            availability_zone: "ap-northeast-1a".to_string(),
            available_ip_count: 251,
            state: "available".to_string(),
            is_default: false,
            map_public_ip_on_launch: false,
        }];

        app.handle_event(AppEvent::NavigateVpcLoaded(Ok((vpcs, subnets))));
        assert!(!app.loading);
        assert_eq!(app.filtered_vpcs.len(), 1);
        assert_eq!(app.subnets.len(), 1);
    }

    #[test]
    fn navigate_vpc_loaded_returns_error_and_pops_stack_when_failure() {
        let mut app = app_with_ec2_detail();
        app.detail_tag_index = 0;
        app.dispatch(Action::FollowLink);
        assert_eq!(app.navigation_stack.len(), 1);

        app.handle_event(AppEvent::NavigateVpcLoaded(Err(AppError::AwsApi(
            "test error".to_string(),
        ))));
        assert!(!app.loading);
        assert!(app.navigation_stack.is_empty()); // popped back
        assert_eq!(app.view, View::Ec2Detail); // restored
    }

    #[test]
    fn detail_list_len_returns_field_count_when_ec2_detail_overview() {
        let mut app = app_with_ec2_detail();
        app.detail_tab = DetailTab::Overview;
        assert_eq!(app.detail_list_len(), Ec2DetailField::ALL.len());
    }

    // ──────────────────────────────────────────────
    // CRUD: Create操作テスト
    // ──────────────────────────────────────────────

    #[test]
    fn handle_create_returns_form_mode_when_s3_list() {
        let mut app = App::new("dev".to_string(), None);
        app.view = View::S3List;
        app.dispatch(Action::Create);
        assert!(matches!(
            app.mode,
            Mode::Form(FormContext {
                kind: FormKind::CreateS3Bucket,
                ..
            })
        ));
    }

    #[test]
    fn handle_create_returns_form_mode_when_secrets_list() {
        let mut app = App::new("dev".to_string(), None);
        app.view = View::SecretsList;
        app.dispatch(Action::Create);
        if let Mode::Form(ctx) = &app.mode {
            assert_eq!(ctx.kind, FormKind::CreateSecret);
            assert_eq!(ctx.fields.len(), 3);
        } else {
            panic!("Expected Form mode");
        }
    }

    #[test]
    fn handle_create_returns_no_change_when_ec2_list() {
        let mut app = App::new("dev".to_string(), None);
        app.view = View::Ec2List;
        app.dispatch(Action::Create);
        assert_eq!(app.mode, Mode::Normal);
    }

    // ──────────────────────────────────────────────
    // CRUD: Delete操作テスト
    // ──────────────────────────────────────────────

    #[test]
    fn handle_delete_returns_permission_denied_when_no_permission() {
        let mut app = App::new("dev".to_string(), None);
        app.view = View::Ec2List;
        app.filtered_instances = vec![create_test_instance("i-001", "web", InstanceState::Running)];
        app.dispatch(Action::Delete);
        assert_eq!(app.mode, Mode::Message);
    }

    #[test]
    fn handle_delete_returns_danger_confirm_when_ec2_with_permission() {
        let mut app = App::with_delete_permissions("dev".to_string(), None, DeletePermissions::All);
        app.view = View::Ec2List;
        app.filtered_instances = vec![create_test_instance("i-001", "web", InstanceState::Running)];
        app.dispatch(Action::Delete);
        if let Mode::DangerConfirm(ctx) = &app.mode {
            assert_eq!(ctx.action, DangerAction::TerminateEc2("i-001".to_string()));
        } else {
            panic!("Expected DangerConfirm mode");
        }
    }

    #[test]
    fn handle_delete_returns_danger_confirm_when_s3_list_with_permission() {
        let mut app = App::with_delete_permissions("dev".to_string(), None, DeletePermissions::All);
        app.view = View::S3List;
        app.s3_filtered_buckets = vec![crate::aws::s3_model::Bucket {
            name: "my-bucket".to_string(),
            creation_date: None,
        }];
        app.dispatch(Action::Delete);
        if let Mode::DangerConfirm(ctx) = &app.mode {
            assert_eq!(
                ctx.action,
                DangerAction::DeleteS3Bucket("my-bucket".to_string())
            );
        } else {
            panic!("Expected DangerConfirm mode");
        }
    }

    #[test]
    fn handle_delete_returns_danger_confirm_when_secrets_list_with_permission() {
        let mut app = App::with_delete_permissions("dev".to_string(), None, DeletePermissions::All);
        app.view = View::SecretsList;
        app.filtered_secrets = vec![crate::aws::secrets_model::Secret {
            name: "my-secret".to_string(),
            arn: "arn:aws:secretsmanager:us-east-1:123:secret:my-secret".to_string(),
            description: None,
            last_changed_date: None,
            last_accessed_date: None,
            tags: HashMap::new(),
        }];
        app.dispatch(Action::Delete);
        if let Mode::DangerConfirm(ctx) = &app.mode {
            assert_eq!(
                ctx.action,
                DangerAction::DeleteSecret("my-secret".to_string())
            );
        } else {
            panic!("Expected DangerConfirm mode");
        }
    }

    #[test]
    fn handle_delete_returns_permission_denied_when_service_not_allowed() {
        let mut app = App::with_delete_permissions(
            "dev".to_string(),
            None,
            DeletePermissions::Services(vec!["ec2".to_string()]),
        );
        app.view = View::S3List;
        app.s3_filtered_buckets = vec![crate::aws::s3_model::Bucket {
            name: "my-bucket".to_string(),
            creation_date: None,
        }];
        app.dispatch(Action::Delete);
        assert_eq!(app.mode, Mode::Message);
    }

    // ──────────────────────────────────────────────
    // CRUD: Edit操作テスト
    // ──────────────────────────────────────────────

    #[test]
    fn handle_edit_returns_form_mode_when_secrets_detail_with_detail() {
        let mut app = App::new("dev".to_string(), None);
        app.view = View::SecretsDetail;
        app.secret_detail = Some(crate::aws::secrets_model::SecretDetail {
            name: "my-secret".to_string(),
            arn: "arn:test".to_string(),
            description: None,
            kms_key_id: None,
            rotation_enabled: false,
            rotation_lambda_arn: None,
            last_rotated_date: None,
            last_changed_date: None,
            last_accessed_date: None,
            created_date: None,
            tags: HashMap::new(),
            version_ids: Vec::new(),
        });
        app.dispatch(Action::Edit);
        if let Mode::Form(ctx) = &app.mode {
            assert_eq!(ctx.kind, FormKind::UpdateSecretValue);
            assert_eq!(ctx.fields.len(), 1);
        } else {
            panic!("Expected Form mode");
        }
    }

    #[test]
    fn handle_edit_returns_no_change_when_no_detail() {
        let mut app = App::new("dev".to_string(), None);
        app.view = View::SecretsDetail;
        app.dispatch(Action::Edit);
        assert_eq!(app.mode, Mode::Normal);
    }

    // ──────────────────────────────────────────────
    // CRUD: Form操作テスト
    // ──────────────────────────────────────────────

    #[test]
    fn handle_form_cancel_returns_normal_mode_when_in_form() {
        let mut app = App::new("dev".to_string(), None);
        app.mode = Mode::Form(FormContext {
            kind: FormKind::CreateS3Bucket,
            fields: vec![FormField {
                label: "Bucket Name".to_string(),
                input: Input::default(),
                required: true,
            }],
            focused_field: 0,
        });
        app.dispatch(Action::FormCancel);
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn handle_form_submit_returns_error_when_required_field_empty() {
        let mut app = App::new("dev".to_string(), None);
        app.mode = Mode::Form(FormContext {
            kind: FormKind::CreateS3Bucket,
            fields: vec![FormField {
                label: "Bucket Name".to_string(),
                input: Input::default(),
                required: true,
            }],
            focused_field: 0,
        });
        app.dispatch(Action::FormSubmit);
        assert_eq!(app.mode, Mode::Message);
    }

    #[test]
    fn handle_form_submit_returns_pending_form_when_valid() {
        let mut app = App::new("dev".to_string(), None);
        let mut input = Input::default();
        input.handle(tui_input::InputRequest::InsertChar('t'));
        input.handle(tui_input::InputRequest::InsertChar('e'));
        input.handle(tui_input::InputRequest::InsertChar('s'));
        input.handle(tui_input::InputRequest::InsertChar('t'));
        app.mode = Mode::Form(FormContext {
            kind: FormKind::CreateS3Bucket,
            fields: vec![FormField {
                label: "Bucket Name".to_string(),
                input,
                required: true,
            }],
            focused_field: 0,
        });
        app.dispatch(Action::FormSubmit);
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.pending_form.is_some());
        assert!(app.loading);
    }

    #[test]
    fn handle_form_next_field_returns_next_when_multiple_fields() {
        let mut app = App::new("dev".to_string(), None);
        app.mode = Mode::Form(FormContext {
            kind: FormKind::CreateSecret,
            fields: vec![
                FormField {
                    label: "Name".to_string(),
                    input: Input::default(),
                    required: true,
                },
                FormField {
                    label: "Value".to_string(),
                    input: Input::default(),
                    required: true,
                },
            ],
            focused_field: 0,
        });
        app.dispatch(Action::FormNextField);
        if let Mode::Form(ctx) = &app.mode {
            assert_eq!(ctx.focused_field, 1);
        } else {
            panic!("Expected Form mode");
        }
    }

    #[test]
    fn handle_form_next_field_wraps_around_when_at_last_field() {
        let mut app = App::new("dev".to_string(), None);
        app.mode = Mode::Form(FormContext {
            kind: FormKind::CreateSecret,
            fields: vec![
                FormField {
                    label: "Name".to_string(),
                    input: Input::default(),
                    required: true,
                },
                FormField {
                    label: "Value".to_string(),
                    input: Input::default(),
                    required: true,
                },
            ],
            focused_field: 1,
        });
        app.dispatch(Action::FormNextField);
        if let Mode::Form(ctx) = &app.mode {
            assert_eq!(ctx.focused_field, 0);
        } else {
            panic!("Expected Form mode");
        }
    }

    // ──────────────────────────────────────────────
    // CRUD: DangerConfirm操作テスト
    // ──────────────────────────────────────────────

    #[test]
    fn handle_danger_confirm_cancel_returns_normal_mode() {
        let mut app = App::new("dev".to_string(), None);
        app.mode = Mode::DangerConfirm(DangerConfirmContext {
            action: DangerAction::TerminateEc2("i-001".to_string()),
            input: Input::default(),
        });
        app.dispatch(Action::DangerConfirmCancel);
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn handle_danger_confirm_submit_returns_no_action_when_text_mismatch() {
        let mut app = App::new("dev".to_string(), None);
        app.mode = Mode::DangerConfirm(DangerConfirmContext {
            action: DangerAction::TerminateEc2("i-001".to_string()),
            input: Input::default(),
        });
        app.dispatch(Action::DangerConfirmSubmit);
        assert!(matches!(app.mode, Mode::DangerConfirm(_)));
    }

    #[test]
    fn handle_danger_confirm_submit_returns_pending_action_when_text_matches() {
        let mut app = App::new("dev".to_string(), None);
        let mut input = Input::default();
        for c in "i-001".chars() {
            input.handle(tui_input::InputRequest::InsertChar(c));
        }
        app.mode = Mode::DangerConfirm(DangerConfirmContext {
            action: DangerAction::TerminateEc2("i-001".to_string()),
            input,
        });
        app.dispatch(Action::DangerConfirmSubmit);
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.pending_danger_action.is_some());
        assert_eq!(
            app.pending_danger_action.unwrap(),
            DangerAction::TerminateEc2("i-001".to_string())
        );
    }

    // ──────────────────────────────────────────────
    // CRUD: DangerAction テスト
    // ──────────────────────────────────────────────

    #[test]
    fn danger_action_confirm_text_returns_id_when_terminate_ec2() {
        let action = DangerAction::TerminateEc2("i-001".to_string());
        assert_eq!(action.confirm_text(), "i-001");
    }

    #[test]
    fn danger_action_confirm_text_returns_name_when_delete_s3_bucket() {
        let action = DangerAction::DeleteS3Bucket("my-bucket".to_string());
        assert_eq!(action.confirm_text(), "my-bucket");
    }

    #[test]
    fn danger_action_confirm_text_returns_key_when_delete_s3_object() {
        let action = DangerAction::DeleteS3Object {
            bucket: "my-bucket".to_string(),
            key: "path/to/file.txt".to_string(),
        };
        assert_eq!(action.confirm_text(), "path/to/file.txt");
    }

    #[test]
    fn danger_action_message_returns_terminate_msg_when_ec2() {
        let action = DangerAction::TerminateEc2("i-001".to_string());
        assert!(action.message().contains("i-001"));
        assert!(action.message().contains("terminate"));
    }

    #[test]
    fn danger_action_message_returns_delete_msg_when_s3_bucket() {
        let action = DangerAction::DeleteS3Bucket("my-bucket".to_string());
        assert!(action.message().contains("my-bucket"));
        assert!(action.message().contains("delete"));
    }

    // ──────────────────────────────────────────────
    // CRUD: CrudCompleted イベントテスト
    // ──────────────────────────────────────────────

    #[test]
    fn handle_event_returns_success_message_when_crud_completed_ok() {
        let mut app = App::new("dev".to_string(), None);
        app.handle_event(AppEvent::CrudCompleted(Ok("Bucket created".to_string())));
        assert_eq!(app.mode, Mode::Message);
        assert!(app.loading);
        let msg = app.message.as_ref().unwrap();
        assert_eq!(msg.level, MessageLevel::Success);
        assert!(msg.body.contains("Bucket created"));
    }

    #[test]
    fn handle_event_returns_error_message_when_crud_completed_err() {
        let mut app = App::new("dev".to_string(), None);
        app.handle_event(AppEvent::CrudCompleted(Err(AppError::AwsApi(
            "access denied".to_string(),
        ))));
        assert_eq!(app.mode, Mode::Message);
        let msg = app.message.as_ref().unwrap();
        assert_eq!(msg.level, MessageLevel::Error);
        assert!(msg.body.contains("access denied"));
    }

    // ──────────────────────────────────────────────
    // CRUD: FormContext テスト
    // ──────────────────────────────────────────────

    #[test]
    fn form_context_field_values_returns_label_value_pairs() {
        let mut input = Input::default();
        input.handle(tui_input::InputRequest::InsertChar('a'));
        let ctx = FormContext {
            kind: FormKind::CreateS3Bucket,
            fields: vec![FormField {
                label: "Name".to_string(),
                input,
                required: true,
            }],
            focused_field: 0,
        };
        let values = ctx.field_values();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0], ("Name", "a"));
    }

    // ──────────────────────────────────────────────
    // ServiceSelect fuzzy filter テスト
    // ──────────────────────────────────────────────

    #[test]
    fn apply_filter_returns_all_services_when_empty_query_in_service_select() {
        let mut app = App::new("dev".to_string(), None);
        app.view = View::ServiceSelect;
        app.filter_input = Input::default();
        app.apply_filter();
        assert_eq!(
            app.filtered_service_names,
            vec!["EC2", "ECR", "ECS", "S3", "VPC", "Secrets Manager"]
        );
    }

    #[test]
    fn apply_filter_returns_matching_services_when_fuzzy_query_in_service_select() {
        let mut app = App::new("dev".to_string(), None);
        app.view = View::ServiceSelect;
        app.filter_input = Input::from("ec");
        app.apply_filter();
        // EC2, ECR, ECS, Secrets Manager はすべてecにマッチしうるが、
        // fuzzyスコアで上位のものだけ返る可能性がある
        assert!(app.filtered_service_names.contains(&"EC2".to_string()));
        assert!(app.filtered_service_names.contains(&"ECR".to_string()));
        assert!(app.filtered_service_names.contains(&"ECS".to_string()));
    }

    #[test]
    fn handle_enter_returns_correct_service_when_filtered_in_service_select() {
        let mut app = App::new("dev".to_string(), None);
        app.profile = Some("dev".to_string());
        app.view = View::ServiceSelect;
        // フィルタでS3だけ残った状態をシミュレート
        app.filtered_service_names = vec!["S3".to_string()];
        app.service_selected = 0;
        app.dispatch(Action::Enter);
        assert_eq!(app.view, View::S3List);
    }

}
