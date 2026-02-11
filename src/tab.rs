use tui_input::Input;

use crate::app::{DetailTab, Ec2DetailField, Mode, NavigationEntry};
use crate::aws::ecr_model::{Image, Repository};
use crate::aws::ecs_model::{Cluster, Service, Task};
use crate::aws::logs_model::LogEvent;
use crate::aws::model::Instance;
use crate::aws::s3_model::{Bucket, S3Object};
use crate::aws::secrets_model::{Secret, SecretDetail};
use crate::aws::vpc_model::{Subnet, Vpc};
use crate::fuzzy::fuzzy_filter_items;
use crate::service::ServiceKind;
use crate::tui::views::secrets_detail::SecretsDetailTab;

/// アイテムリストとフィルタ済みリストを統一管理する
#[derive(Debug, Clone)]
pub struct FilterableList<T: Clone> {
    items: Vec<T>,
    pub filtered: Vec<T>,
}

impl<T: Clone> FilterableList<T> {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            filtered: Vec::new(),
        }
    }

    /// 全アイテムを設定し、フィルタ済みリストも同じにリセット
    pub fn set_items(&mut self, items: Vec<T>) {
        self.filtered = items.clone();
        self.items = items;
    }

    /// 全アイテムへの参照
    pub fn all(&self) -> &[T] {
        &self.items
    }

    /// フィルタを適用（フィルタ関数で全アイテムから絞り込み）
    pub fn apply_filter(&mut self, filter_fn: impl Fn(&T) -> bool) {
        self.filtered = self
            .items
            .iter()
            .filter(|item| filter_fn(item))
            .cloned()
            .collect();
    }

    /// フィルタをリセット（全アイテムを表示）
    pub fn reset_filter(&mut self) {
        self.filtered = self.items.clone();
    }

    /// フィルタ済みリストの長さ
    pub fn len(&self) -> usize {
        self.filtered.len()
    }

    pub fn is_empty(&self) -> bool {
        self.filtered.is_empty()
    }
}

impl<T: Clone> Default for FilterableList<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// ログビューの状態
#[derive(Debug, Clone)]
pub struct LogViewState {
    pub container_name: String,
    pub log_group: String,
    pub log_stream: String,
    pub events: Vec<LogEvent>,
    pub next_forward_token: Option<String>,
    pub auto_scroll: bool,
    pub scroll_offset: usize,
    pub search_query: String,
    pub search_matches: Vec<usize>,
    pub current_match_index: Option<usize>,
}

impl LogViewState {
    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
        self.auto_scroll = false;
    }

    pub fn scroll_down(&mut self) {
        let max = self.events.len().saturating_sub(1);
        if self.scroll_offset < max {
            self.scroll_offset += 1;
        }
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
        self.auto_scroll = false;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.events.len().saturating_sub(1);
        self.auto_scroll = true;
    }

    pub fn toggle_auto_scroll(&mut self) {
        self.auto_scroll = !self.auto_scroll;
        if self.auto_scroll {
            self.scroll_offset = self.events.len().saturating_sub(1);
        }
    }

    /// 検索クエリを適用し、最初のマッチにジャンプ
    pub fn apply_search(&mut self, query: &str) {
        self.search_query = query.to_string();
        self.recompute_search_matches();
        if let Some(&first) = self.search_matches.first() {
            self.current_match_index = Some(0);
            self.scroll_offset = first;
            self.auto_scroll = false;
        }
    }

    /// 次の検索マッチに移動
    pub fn search_next(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        let next = match self.current_match_index {
            Some(idx) => (idx + 1) % self.search_matches.len(),
            None => 0,
        };
        self.current_match_index = Some(next);
        self.scroll_offset = self.search_matches[next];
        self.auto_scroll = false;
    }

    /// 前の検索マッチに移動
    pub fn search_prev(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        let len = self.search_matches.len();
        let prev = match self.current_match_index {
            Some(idx) => {
                if idx == 0 {
                    len - 1
                } else {
                    idx - 1
                }
            }
            None => len - 1,
        };
        self.current_match_index = Some(prev);
        self.scroll_offset = self.search_matches[prev];
        self.auto_scroll = false;
    }

    /// 検索マッチを再計算する
    pub fn recompute_search_matches(&mut self) {
        if self.search_query.is_empty() {
            self.search_matches.clear();
            self.current_match_index = None;
            return;
        }
        let query = self.search_query.clone();
        self.search_matches = self
            .events
            .iter()
            .enumerate()
            .filter(|(_, e)| e.message.to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect();
        if self.search_matches.is_empty() {
            self.current_match_index = None;
        }
    }
}

/// タブの一意識別子
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(pub u32);

/// タブ内のビュー
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabView {
    List,
    Detail,
}

/// ECS詳細画面のナビゲーションレベル
#[derive(Debug, Clone)]
pub enum EcsNavLevel {
    ClusterDetail,
    ServiceDetail {
        service_index: usize,
    },
    TaskDetail {
        service_index: usize,
        task_index: usize,
    },
    LogView {
        service_index: usize,
        task_index: usize,
        log_state: Box<LogViewState>,
    },
}

impl EcsNavLevel {
    pub fn service_index(&self) -> Option<usize> {
        match self {
            EcsNavLevel::ServiceDetail { service_index, .. }
            | EcsNavLevel::TaskDetail { service_index, .. }
            | EcsNavLevel::LogView { service_index, .. } => Some(*service_index),
            _ => None,
        }
    }

    pub fn task_index(&self) -> Option<usize> {
        match self {
            EcsNavLevel::TaskDetail { task_index, .. }
            | EcsNavLevel::LogView { task_index, .. } => Some(*task_index),
            _ => None,
        }
    }

    pub fn log_state(&self) -> Option<&LogViewState> {
        match self {
            EcsNavLevel::LogView { log_state, .. } => Some(log_state),
            _ => None,
        }
    }

    pub fn log_state_mut(&mut self) -> Option<&mut LogViewState> {
        match self {
            EcsNavLevel::LogView { log_state, .. } => Some(log_state),
            _ => None,
        }
    }
}

/// サービス固有のデータ
#[derive(Debug, Clone)]
pub enum ServiceData {
    Ec2 {
        instances: FilterableList<Instance>,
    },
    Ecr {
        repositories: FilterableList<Repository>,
        images: Vec<Image>,
    },
    Ecs {
        clusters: FilterableList<Cluster>,
        services: Vec<Service>,
        tasks: Vec<Task>,
        nav_level: Option<EcsNavLevel>,
    },
    S3 {
        buckets: FilterableList<Bucket>,
        objects: Vec<S3Object>,
        selected_bucket: Option<String>,
        current_prefix: String,
    },
    Vpc {
        vpcs: FilterableList<Vpc>,
        subnets: Vec<Subnet>,
    },
    Secrets {
        secrets: FilterableList<Secret>,
        detail: Option<Box<SecretDetail>>,
        detail_tab: SecretsDetailTab,
        value_visible: bool,
    },
}

impl ServiceData {
    pub fn new(service: ServiceKind) -> Self {
        match service {
            ServiceKind::Ec2 => ServiceData::Ec2 {
                instances: FilterableList::new(),
            },
            ServiceKind::Ecr => ServiceData::Ecr {
                repositories: FilterableList::new(),
                images: Vec::new(),
            },
            ServiceKind::Ecs => ServiceData::Ecs {
                clusters: FilterableList::new(),
                services: Vec::new(),
                tasks: Vec::new(),
                nav_level: None,
            },
            ServiceKind::S3 => ServiceData::S3 {
                buckets: FilterableList::new(),
                objects: Vec::new(),
                selected_bucket: None,
                current_prefix: String::new(),
            },
            ServiceKind::Vpc => ServiceData::Vpc {
                vpcs: FilterableList::new(),
                subnets: Vec::new(),
            },
            ServiceKind::SecretsManager => ServiceData::Secrets {
                secrets: FilterableList::new(),
                detail: None,
                detail_tab: SecretsDetailTab::Overview,
                value_visible: false,
            },
        }
    }
}

/// 1つのタブ
pub struct Tab {
    pub id: TabId,
    pub service: ServiceKind,
    pub tab_view: TabView,
    pub mode: Mode,
    pub loading: bool,
    pub selected_index: usize,
    pub filter_input: Input,
    pub detail_tab: DetailTab,
    pub detail_tag_index: usize,
    pub data: ServiceData,
    pub navigation_stack: Vec<NavigationEntry>,
    pub navigate_target_id: Option<String>,
}

impl Tab {
    pub fn new(id: TabId, service: ServiceKind) -> Self {
        Self {
            id,
            service,
            tab_view: TabView::List,
            mode: Mode::Normal,
            loading: true,
            selected_index: 0,
            filter_input: Input::default(),
            detail_tab: DetailTab::Overview,
            detail_tag_index: 0,
            data: ServiceData::new(service),
            navigation_stack: Vec::new(),
            navigate_target_id: None,
        }
    }

    /// タブのタイトル（タブバー表示用）
    pub fn title(&self) -> &'static str {
        self.service.short_name()
    }

    /// リスト状態をリセットする
    pub fn reset_list_state(&mut self) {
        self.selected_index = 0;
        self.filter_input.reset();
        self.mode = Mode::Normal;
    }

    /// 詳細状態をリセットする
    pub fn reset_detail_state(&mut self) {
        self.detail_tag_index = 0;
        self.detail_tab = DetailTab::Overview;
        if let ServiceData::Secrets {
            detail_tab,
            value_visible,
            ..
        } = &mut self.data
        {
            *detail_tab = SecretsDetailTab::Overview;
            *value_visible = false;
        }
        if let ServiceData::Ecs { nav_level, .. } = &mut self.data {
            *nav_level = Some(EcsNavLevel::ClusterDetail);
        }
    }

    /// 現在のリストビューのフィルタ済みリスト長を返す
    pub fn filtered_list_len(&self) -> usize {
        match &self.data {
            ServiceData::Ec2 { instances, .. } => instances.len(),
            ServiceData::Ecr { repositories, .. } => repositories.len(),
            ServiceData::Ecs { clusters, .. } => clusters.len(),
            ServiceData::S3 { buckets, .. } => buckets.len(),
            ServiceData::Vpc { vpcs, .. } => vpcs.len(),
            ServiceData::Secrets { secrets, .. } => secrets.len(),
        }
    }

    /// 現在のディテールビューのリスト長を返す
    pub fn detail_list_len(&self) -> usize {
        match &self.data {
            ServiceData::Ec2 { instances, .. } => {
                if self.detail_tab == DetailTab::Overview {
                    Ec2DetailField::ALL.len()
                } else {
                    instances
                        .filtered
                        .get(self.selected_index)
                        .map(|i| i.tags.len())
                        .unwrap_or(0)
                }
            }
            ServiceData::Ecr { images, .. } => images.len(),
            ServiceData::Ecs {
                services,
                tasks,
                nav_level,
                ..
            } => match nav_level {
                Some(EcsNavLevel::TaskDetail { .. }) | Some(EcsNavLevel::LogView { .. }) => 0, // タスク詳細・ログはスクロール不要
                Some(EcsNavLevel::ServiceDetail { .. }) => tasks.len(), // サービス詳細 → タスク一覧
                _ => services.len(), // クラスター詳細 → サービス一覧
            },
            ServiceData::S3 { objects, .. } => objects.len(),
            ServiceData::Vpc { subnets, .. } => subnets.len(),
            ServiceData::Secrets {
                detail, detail_tab, ..
            } => match detail_tab {
                SecretsDetailTab::Tags => detail.as_ref().map(|d| d.tags.len()).unwrap_or(0),
                SecretsDetailTab::Versions => {
                    detail.as_ref().map(|d| d.version_stages.len()).unwrap_or(0)
                }
                _ => 0,
            },
        }
    }

    /// フィルタを適用
    pub fn apply_filter(&mut self) {
        let filter_text = self.filter_input.value().to_string();
        match &mut self.data {
            ServiceData::Ec2 { instances } => {
                instances.filtered = fuzzy_filter_items(instances.all(), &filter_text, 1, |i| {
                    vec![
                        i.instance_id.as_str(),
                        i.name.as_str(),
                        i.instance_type.as_str(),
                        i.state.as_str(),
                    ]
                });
            }
            ServiceData::Ecr { repositories, .. } => {
                repositories.filtered =
                    fuzzy_filter_items(repositories.all(), &filter_text, 0, |r| {
                        vec![r.repository_name.as_str(), r.repository_uri.as_str()]
                    });
            }
            ServiceData::Ecs { clusters, .. } => {
                clusters.filtered = fuzzy_filter_items(clusters.all(), &filter_text, 0, |c| {
                    vec![c.cluster_name.as_str(), c.status.as_str()]
                });
            }
            ServiceData::S3 { buckets, .. } => {
                buckets.filtered =
                    fuzzy_filter_items(buckets.all(), &filter_text, 0, |b| vec![b.name.as_str()]);
            }
            ServiceData::Vpc { vpcs, .. } => {
                vpcs.filtered = fuzzy_filter_items(vpcs.all(), &filter_text, 1, |v| {
                    vec![v.vpc_id.as_str(), v.name.as_str(), v.cidr_block.as_str()]
                });
            }
            ServiceData::Secrets { secrets, .. } => {
                secrets.filtered = fuzzy_filter_items(secrets.all(), &filter_text, 0, |s| {
                    vec![s.name.as_str(), s.arn.as_str()]
                });
            }
        }
        let len = self.filtered_list_len();
        if len > 0 && self.selected_index >= len {
            self.selected_index = len - 1;
        }
    }

    /// サービスデータをクリアする
    pub fn clear_data(&mut self) {
        self.data = ServiceData::new(self.service);
    }

    /// このタブがログビューかどうかを判定
    pub fn is_in_log_view(&self) -> bool {
        matches!(
            &self.data,
            ServiceData::Ecs {
                nav_level: Some(EcsNavLevel::LogView { .. }),
                ..
            }
        )
    }

    /// ログビューの状態への可変参照を取得
    pub fn log_state_mut(&mut self) -> Option<&mut LogViewState> {
        if let ServiceData::Ecs {
            nav_level: Some(nav),
            ..
        } = &mut self.data
        {
            nav.log_state_mut()
        } else {
            None
        }
    }

    /// 選択を1つ上に移動
    pub fn move_up(&mut self) {
        match self.tab_view {
            TabView::List => {
                self.selected_index = self.selected_index.saturating_sub(1);
            }
            TabView::Detail => {
                self.detail_tag_index = self.detail_tag_index.saturating_sub(1);
            }
        }
    }

    /// 選択を1つ下に移動
    pub fn move_down(&mut self) {
        match self.tab_view {
            TabView::List => {
                let max = self.filtered_list_len().saturating_sub(1);
                if self.selected_index < max {
                    self.selected_index += 1;
                }
            }
            TabView::Detail => {
                let max = self.detail_list_len().saturating_sub(1);
                if self.detail_tag_index < max {
                    self.detail_tag_index += 1;
                }
            }
        }
    }

    /// 選択を先頭に移動
    pub fn move_to_top(&mut self) {
        match self.tab_view {
            TabView::List => self.selected_index = 0,
            TabView::Detail => self.detail_tag_index = 0,
        }
    }

    /// 選択を末尾に移動
    pub fn move_to_bottom(&mut self) {
        match self.tab_view {
            TabView::List => {
                self.selected_index = self.filtered_list_len().saturating_sub(1);
            }
            TabView::Detail => {
                self.detail_tag_index = self.detail_list_len().saturating_sub(1);
            }
        }
    }

    /// 半ページ上に移動
    pub fn half_page_up(&mut self) {
        match self.tab_view {
            TabView::List => {
                self.selected_index = self.selected_index.saturating_sub(10);
            }
            TabView::Detail => {
                self.detail_tag_index = self.detail_tag_index.saturating_sub(10);
            }
        }
    }

    /// 半ページ下に移動
    pub fn half_page_down(&mut self) {
        match self.tab_view {
            TabView::List => {
                let max = self.filtered_list_len().saturating_sub(1);
                self.selected_index = (self.selected_index + 10).min(max);
            }
            TabView::Detail => {
                let max = self.detail_list_len().saturating_sub(1);
                self.detail_tag_index = (self.detail_tag_index + 10).min(max);
            }
        }
    }

    /// Enterキーの処理
    pub fn handle_enter(&mut self) {
        match self.tab_view {
            TabView::List => {
                if self.filtered_list_len() == 0 {
                    return;
                }
                // S3: バケット選択時にselected_bucketを設定
                if self.service == ServiceKind::S3
                    && let ServiceData::S3 {
                        buckets,
                        selected_bucket,
                        current_prefix,
                        ..
                    } = &mut self.data
                    && let Some(bucket) = buckets.filtered.get(self.selected_index)
                {
                    *selected_bucket = Some(bucket.name.clone());
                    current_prefix.clear();
                }
                self.tab_view = TabView::Detail;
                self.reset_detail_state();
                // EC2は詳細画面でloadingしない（リストデータから表示）
                if self.service != ServiceKind::Ec2 {
                    self.loading = true;
                }
            }
            TabView::Detail => {
                // ECS Detail: 3段階ナビゲーション
                if self.service == ServiceKind::Ecs
                    && let ServiceData::Ecs {
                        services,
                        tasks,
                        nav_level,
                        ..
                    } = &mut self.data
                {
                    match nav_level {
                        Some(EcsNavLevel::ServiceDetail { service_index }) => {
                            // サービス詳細 → タスク詳細
                            let svc_idx = *service_index;
                            if !tasks.is_empty() && self.detail_tag_index < tasks.len() {
                                *nav_level = Some(EcsNavLevel::TaskDetail {
                                    service_index: svc_idx,
                                    task_index: self.detail_tag_index,
                                });
                            }
                        }
                        Some(EcsNavLevel::ClusterDetail) => {
                            // サービス一覧 → サービス詳細（タスク読み込みトリガー）
                            if !services.is_empty() && self.detail_tag_index < services.len() {
                                *nav_level = Some(EcsNavLevel::ServiceDetail {
                                    service_index: self.detail_tag_index,
                                });
                                self.detail_tag_index = 0;
                                self.loading = true;
                            }
                        }
                        _ => {}
                    }
                }

                // S3 Detail: プレフィックス(ディレクトリ)の場合は中に入る
                if self.service == ServiceKind::S3
                    && let ServiceData::S3 {
                        objects,
                        current_prefix,
                        ..
                    } = &mut self.data
                    && let Some(obj) = objects.get(self.detail_tag_index)
                    && obj.is_prefix
                {
                    *current_prefix = obj.key.clone();
                    self.detail_tag_index = 0;
                    self.loading = true;
                }
            }
        }
    }

    /// Backキーの処理
    pub fn handle_back(&mut self) {
        match self.tab_view {
            TabView::List => {
                // リストビューではEscは何もしない
            }
            TabView::Detail => {
                // S3: プレフィックス内にいる場合は一つ上に移動
                if self.service == ServiceKind::S3
                    && let ServiceData::S3 {
                        current_prefix,
                        objects,
                        selected_bucket,
                        ..
                    } = &mut self.data
                {
                    if !current_prefix.is_empty() {
                        let trimmed = current_prefix.trim_end_matches('/');
                        if let Some(pos) = trimmed.rfind('/') {
                            *current_prefix = trimmed[..=pos].to_string();
                        } else {
                            current_prefix.clear();
                        }
                        self.detail_tag_index = 0;
                        self.loading = true;
                        return;
                    }
                    // ルートにいる場合はリストに戻る
                    objects.clear();
                    *selected_bucket = None;
                }

                // VPC: ナビゲーションスタックがある場合は戻る
                if self.service == ServiceKind::Vpc {
                    if let Some(entry) = self.navigation_stack.pop() {
                        self.selected_index = entry.selected_index;
                        self.detail_tag_index = entry.detail_tag_index;
                        self.detail_tab = entry.detail_tab;
                        if let ServiceData::Vpc { subnets, .. } = &mut self.data {
                            subnets.clear();
                        }
                        return;
                    }
                    if let ServiceData::Vpc { subnets, .. } = &mut self.data {
                        subnets.clear();
                    }
                }

                // ECR: イメージをクリア
                if self.service == ServiceKind::Ecr
                    && let ServiceData::Ecr { images, .. } = &mut self.data
                {
                    images.clear();
                }

                // ECS: ログ→タスク詳細→サービス詳細→サービス一覧→クラスター一覧
                if self.service == ServiceKind::Ecs
                    && let ServiceData::Ecs {
                        services,
                        tasks,
                        nav_level,
                        ..
                    } = &mut self.data
                {
                    match nav_level {
                        Some(EcsNavLevel::LogView {
                            service_index,
                            task_index,
                            ..
                        }) => {
                            let si = *service_index;
                            let ti = *task_index;
                            *nav_level = Some(EcsNavLevel::TaskDetail {
                                service_index: si,
                                task_index: ti,
                            });
                            return;
                        }
                        Some(EcsNavLevel::TaskDetail { service_index, .. }) => {
                            let si = *service_index;
                            *nav_level = Some(EcsNavLevel::ServiceDetail { service_index: si });
                            return;
                        }
                        Some(EcsNavLevel::ServiceDetail { .. }) => {
                            *nav_level = Some(EcsNavLevel::ClusterDetail);
                            tasks.clear();
                            self.detail_tag_index = 0;
                            return;
                        }
                        Some(EcsNavLevel::ClusterDetail) => {
                            *nav_level = None;
                            services.clear();
                        }
                        None => {}
                    }
                }

                // Secrets: 詳細をクリア
                if self.service == ServiceKind::SecretsManager
                    && let ServiceData::Secrets { detail, .. } = &mut self.data
                {
                    *detail = None;
                }

                // EC2: ナビゲーションスタックをクリア
                if self.service == ServiceKind::Ec2 {
                    self.navigation_stack.clear();
                }

                self.tab_view = TabView::List;
            }
        }
    }

    /// 次の詳細タブに切り替え
    pub fn switch_detail_tab(&mut self) {
        self.detail_tag_index = 0;
        match self.service {
            ServiceKind::Ec2 => {
                self.detail_tab = match self.detail_tab {
                    DetailTab::Overview => DetailTab::Tags,
                    DetailTab::Tags => DetailTab::Overview,
                };
            }
            ServiceKind::SecretsManager => {
                if let ServiceData::Secrets { detail_tab, .. } = &mut self.data {
                    *detail_tab = match detail_tab {
                        SecretsDetailTab::Overview => SecretsDetailTab::Rotation,
                        SecretsDetailTab::Rotation => SecretsDetailTab::Versions,
                        SecretsDetailTab::Versions => SecretsDetailTab::Tags,
                        SecretsDetailTab::Tags => SecretsDetailTab::Overview,
                    };
                }
            }
            _ => {}
        }
    }

    /// 前の詳細タブに切り替え
    pub fn prev_detail_tab(&mut self) {
        self.detail_tag_index = 0;
        match self.service {
            ServiceKind::Ec2 => {
                self.detail_tab = match self.detail_tab {
                    DetailTab::Overview => DetailTab::Tags,
                    DetailTab::Tags => DetailTab::Overview,
                };
            }
            ServiceKind::SecretsManager => {
                if let ServiceData::Secrets { detail_tab, .. } = &mut self.data {
                    *detail_tab = match detail_tab {
                        SecretsDetailTab::Overview => SecretsDetailTab::Tags,
                        SecretsDetailTab::Tags => SecretsDetailTab::Versions,
                        SecretsDetailTab::Versions => SecretsDetailTab::Rotation,
                        SecretsDetailTab::Rotation => SecretsDetailTab::Overview,
                    };
                }
            }
            _ => {}
        }
    }

    /// 選択中のアイテムのIDをクリップボードにコピー
    pub fn copy_id(&self) {
        match (self.service, self.tab_view) {
            (ServiceKind::Ec2, _) => {
                if let ServiceData::Ec2 { instances, .. } = &self.data
                    && let Some(instance) = instances.filtered.get(self.selected_index)
                {
                    let _ = cli_clipboard::set_contents(instance.instance_id.clone());
                }
            }
            (ServiceKind::Ecr, TabView::List) => {
                if let ServiceData::Ecr { repositories, .. } = &self.data
                    && let Some(repo) = repositories.filtered.get(self.selected_index)
                {
                    let _ = cli_clipboard::set_contents(repo.repository_uri.clone());
                }
            }
            (ServiceKind::Ecr, TabView::Detail) => {
                if let ServiceData::Ecr { images, .. } = &self.data
                    && let Some(image) = images.get(self.detail_tag_index)
                {
                    let _ = cli_clipboard::set_contents(image.image_digest.clone());
                }
            }
            (ServiceKind::Vpc, TabView::List) => {
                if let ServiceData::Vpc { vpcs, .. } = &self.data
                    && let Some(vpc) = vpcs.filtered.get(self.selected_index)
                {
                    let _ = cli_clipboard::set_contents(vpc.vpc_id.clone());
                }
            }
            (ServiceKind::SecretsManager, TabView::List) => {
                if let ServiceData::Secrets { secrets, .. } = &self.data
                    && let Some(secret) = secrets.filtered.get(self.selected_index)
                {
                    let _ = cli_clipboard::set_contents(secret.arn.clone());
                }
            }
            (ServiceKind::SecretsManager, TabView::Detail) => {
                if let ServiceData::Secrets { detail, .. } = &self.data
                    && let Some(d) = detail
                {
                    let _ = cli_clipboard::set_contents(d.arn.clone());
                }
            }
            (ServiceKind::S3, TabView::List) => {
                if let ServiceData::S3 { buckets, .. } = &self.data
                    && let Some(bucket) = buckets.filtered.get(self.selected_index)
                {
                    let _ = cli_clipboard::set_contents(bucket.name.clone());
                }
            }
            _ => {}
        }
    }
}
