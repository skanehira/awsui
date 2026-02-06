use tokio::sync::mpsc;
use tui_input::Input;

use crate::action::Action;
use crate::aws::ecr_model::{Image, Repository};
use crate::aws::ecs_model::{Cluster, Service};
use crate::aws::model::{Instance, InstanceState};
use crate::aws::s3_model::{Bucket, S3Object};
use crate::aws::secrets_model::{Secret, SecretDetail};
use crate::aws::vpc_model::{Subnet, Vpc};
use crate::event::AppEvent;
use crate::tui::views::secrets_detail::SecretsDetailTab;

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

    // Profile selection
    pub profile_names: Vec<String>,
    pub profile_selected: usize,

    // Service selection
    pub service_selected: usize,

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
            event_tx,
            event_rx,
        }
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
            View::Ec2Detail => self.selected_instance().map(|i| i.tags.len()).unwrap_or(0),
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
            View::Ec2List => {
                self.filtered_instances = filter_items(&self.instances, filter_text, |i| {
                    [
                        i.instance_id.as_str(),
                        i.name.as_str(),
                        i.instance_type.as_str(),
                        i.state.as_str(),
                    ]
                    .into_iter()
                    .collect()
                });
            }
            View::EcrList => {
                self.ecr_filtered_repositories =
                    filter_items(&self.ecr_repositories, filter_text, |r| {
                        vec![r.repository_name.as_str(), r.repository_uri.as_str()]
                    });
            }
            View::EcsList => {
                self.ecs_filtered_clusters = filter_items(&self.ecs_clusters, filter_text, |c| {
                    vec![c.cluster_name.as_str(), c.status.as_str()]
                });
            }
            View::S3List => {
                self.s3_filtered_buckets =
                    filter_items(&self.s3_buckets, filter_text, |b| vec![b.name.as_str()]);
            }
            View::VpcList => {
                self.filtered_vpcs = filter_items(&self.vpcs, filter_text, |v| {
                    vec![v.vpc_id.as_str(), v.name.as_str(), v.cidr_block.as_str()]
                });
            }
            View::SecretsList => {
                self.filtered_secrets = filter_items(&self.secrets, filter_text, |s| {
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
        }
    }

    fn move_up(&mut self) {
        match self.view {
            View::ProfileSelect => {
                self.profile_selected = self.profile_selected.saturating_sub(1);
            }
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
            View::ProfileSelect => {
                let max = self.profile_names.len().saturating_sub(1);
                if self.profile_selected < max {
                    self.profile_selected += 1;
                }
            }
            View::ServiceSelect => {
                let max = crate::tui::views::service_select::SERVICE_NAMES
                    .len()
                    .saturating_sub(1);
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
            View::ProfileSelect => self.profile_selected = 0,
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
            View::ProfileSelect => {
                self.profile_selected = self.profile_names.len().saturating_sub(1);
            }
            View::ServiceSelect => {
                self.service_selected = crate::tui::views::service_select::SERVICE_NAMES
                    .len()
                    .saturating_sub(1);
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
            View::ProfileSelect => {
                self.profile_selected = self.profile_selected.saturating_sub(10);
            }
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
            View::ProfileSelect => {
                let max = self.profile_names.len().saturating_sub(1);
                self.profile_selected = (self.profile_selected + 10).min(max);
            }
            View::ServiceSelect => {
                let max = crate::tui::views::service_select::SERVICE_NAMES
                    .len()
                    .saturating_sub(1);
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
            View::ProfileSelect => {
                if let Some(name) = self.profile_names.get(self.profile_selected) {
                    self.profile = Some(name.clone());
                    self.view = View::ServiceSelect;
                    self.service_selected = 0;
                }
            }
            View::ServiceSelect => {
                // SERVICE_NAMES: ["EC2", "ECR", "ECS", "S3", "VPC", "Secrets Manager"]
                let view = match self.service_selected {
                    0 => View::Ec2List,
                    1 => View::EcrList,
                    2 => View::EcsList,
                    3 => View::S3List,
                    4 => View::VpcList,
                    5 => View::SecretsList,
                    _ => return,
                };
                self.view = view;
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
            View::ProfileSelect => {}
            View::ServiceSelect => {
                self.view = View::ProfileSelect;
                self.profile = None;
                self.region = None;
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
            View::Ec2Detail => self.view = View::Ec2List,
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
                self.view = View::VpcList;
                self.subnets.clear();
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
}

/// フィルタリングのヘルパー関数
fn filter_items<T: Clone>(
    items: &[T],
    filter_text: &str,
    fields: impl Fn(&T) -> Vec<&str>,
) -> Vec<T> {
    if filter_text.is_empty() {
        return items.to_vec();
    }
    let query = filter_text.to_lowercase();
    items
        .iter()
        .filter(|item| {
            fields(item)
                .into_iter()
                .any(|f| f.to_lowercase().contains(&query))
        })
        .cloned()
        .collect()
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
        let mut app = App::new(vec![]);
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
        let mut app = App::new(vec![]);
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
        assert_eq!(app.view, View::ServiceSelect);
    }

    #[test]
    fn dispatch_returns_none_and_switches_to_ec2_list_when_enter_in_service_select() {
        let mut app = App::new(vec!["dev".to_string()]);
        app.view = View::ServiceSelect;
        app.service_selected = 0; // EC2
        app.dispatch(Action::Enter);
        assert_eq!(app.view, View::Ec2List);
        assert!(app.loading);
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
    fn dispatch_returns_none_and_goes_to_service_select_when_back_in_ec2_list() {
        let mut app = App::new(vec!["dev".to_string()]);
        app.view = View::Ec2List;
        app.instances = vec![create_test_instance("i-001", "web", InstanceState::Running)];
        app.filtered_instances = vec![create_test_instance("i-001", "web", InstanceState::Running)];
        app.dispatch(Action::Back);
        assert_eq!(app.view, View::ServiceSelect);
        assert!(app.instances.is_empty());
        assert!(app.filtered_instances.is_empty());
    }

    #[test]
    fn dispatch_returns_none_and_goes_to_profile_select_when_back_in_service_select() {
        let mut app = App::new(vec!["dev".to_string()]);
        app.view = View::ServiceSelect;
        app.profile = Some("dev".to_string());
        app.dispatch(Action::Back);
        assert_eq!(app.view, View::ProfileSelect);
        assert!(app.profile.is_none());
    }

    #[test]
    fn dispatch_returns_none_and_sets_normal_mode_when_back_in_help() {
        let mut app = App::new(vec![]);
        app.mode = Mode::Help;
        app.view = View::Ec2List;
        app.dispatch(Action::Back);
        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.view, View::Ec2List);
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
        let mut app = App::new(vec![]);
        app.view = View::Ec2List;
        app.instances = vec![create_test_instance("i-001", "web", InstanceState::Running)];
        app.dispatch(Action::FilterHandleInput(InputRequest::InsertChar('w')));
        assert_eq!(app.filter_input.value(), "w");
        assert_eq!(app.filtered_instances.len(), 1);
    }

    #[test]
    fn dispatch_returns_none_and_deletes_char_when_filter_handle_input_delete() {
        use tui_input::InputRequest;
        let mut app = App::new(vec![]);
        app.view = View::Ec2List;
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
        app.view = View::Ec2Detail;
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
        let mut app = App::new(vec![]);
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
        let mut app = App::new(vec!["dev".to_string()]);
        app.view = View::ServiceSelect;
        app.service_selected = 0;
        app.dispatch(Action::MoveDown);
        assert_eq!(app.service_selected, 1);
    }

    #[test]
    fn dispatch_returns_none_and_decrements_service_selected_when_move_up_in_service_select() {
        let mut app = App::new(vec!["dev".to_string()]);
        app.view = View::ServiceSelect;
        app.service_selected = 2;
        app.dispatch(Action::MoveUp);
        assert_eq!(app.service_selected, 1);
    }

    #[test]
    fn dispatch_returns_none_and_enters_ecr_list_when_enter_in_service_select_ecr() {
        let mut app = App::new(vec!["dev".to_string()]);
        app.view = View::ServiceSelect;
        app.service_selected = 1; // ECR
        app.dispatch(Action::Enter);
        assert_eq!(app.view, View::EcrList);
        assert!(app.loading);
    }

    #[test]
    fn dispatch_returns_none_and_enters_s3_list_when_enter_in_service_select_s3() {
        let mut app = App::new(vec!["dev".to_string()]);
        app.view = View::ServiceSelect;
        app.service_selected = 3; // S3
        app.dispatch(Action::Enter);
        assert_eq!(app.view, View::S3List);
        assert!(app.loading);
    }
}
