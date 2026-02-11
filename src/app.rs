use tokio::sync::mpsc;
use tui_input::Input;

use crate::action::Action;
use crate::aws::model::{Instance, InstanceState};
use crate::cli::DeletePermissions;
use crate::event::AppEvent;
use crate::fuzzy::fuzzy_filter_items;
use crate::service::ServiceKind;
pub use crate::ui_state::*;

/// アプリケーション全体の状態
pub struct App {
    // グローバル UI state
    pub should_quit: bool,
    pub message: Option<Message>,
    pub show_help: bool,

    // AWS context
    pub profile: Option<String>,
    pub region: Option<String>,

    // タブ管理
    pub tabs: Vec<crate::tab::Tab>,
    pub active_tab_index: usize,
    next_tab_id: u32,

    // ダッシュボード
    pub show_dashboard: bool,
    pub dashboard: DashboardState,

    // サービスピッカー（Ctrl+tポップアップ）
    pub service_picker: Option<ServicePickerState>,

    // Delete permissions
    pub delete_permissions: DeletePermissions,

    // ログ関連の一時状態
    pub(crate) pending_log_configs: Option<(
        crate::tab::TabId,
        Vec<crate::aws::ecs_model::ContainerLogConfig>,
    )>,

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
            should_quit: false,
            message: None,
            show_help: false,
            profile: Some(profile),
            region,
            tabs: Vec::new(),
            active_tab_index: 0,
            next_tab_id: 0,
            show_dashboard: true,
            dashboard: DashboardState::new(),
            service_picker: None,
            delete_permissions,
            pending_log_configs: None,
            event_tx,
            event_rx,
        }
    }

    /// 新しいタブを作成して追加し、そのTabIdを返す
    pub fn create_tab(&mut self, service: ServiceKind) -> crate::tab::TabId {
        let id = crate::tab::TabId(self.next_tab_id);
        self.next_tab_id += 1;
        let tab = crate::tab::Tab::new(id, service);
        self.tabs.push(tab);
        self.active_tab_index = self.tabs.len() - 1;
        self.show_dashboard = false;

        // 最近使ったサービスの履歴を更新
        self.dashboard.update_recent(service);
        #[cfg(not(test))]
        crate::recent::update_recent(service);

        id
    }

    /// アクティブなタブへの参照を返す
    pub fn active_tab(&self) -> Option<&crate::tab::Tab> {
        self.tabs.get(self.active_tab_index)
    }

    /// アクティブなタブへの可変参照を返す
    pub fn active_tab_mut(&mut self) -> Option<&mut crate::tab::Tab> {
        self.tabs.get_mut(self.active_tab_index)
    }

    /// アクティブタブのログビュー状態への可変参照を返す
    fn active_log_state_mut(&mut self) -> Option<&mut crate::tab::LogViewState> {
        self.active_tab_mut()?.log_state_mut()
    }

    /// TabIdからタブを検索
    pub fn find_tab(&self, tab_id: crate::tab::TabId) -> Option<&crate::tab::Tab> {
        self.tabs.iter().find(|t| t.id == tab_id)
    }

    /// TabIdからタブを可変検索
    pub fn find_tab_mut(&mut self, tab_id: crate::tab::TabId) -> Option<&mut crate::tab::Tab> {
        self.tabs.iter_mut().find(|t| t.id == tab_id)
    }

    /// 次のタブに切り替え
    pub fn switch_tab_next(&mut self) {
        if !self.tabs.is_empty() {
            self.active_tab_index = (self.active_tab_index + 1) % self.tabs.len();
        }
    }

    /// 前のタブに切り替え
    pub fn switch_tab_prev(&mut self) {
        if !self.tabs.is_empty() {
            self.active_tab_index = if self.active_tab_index == 0 {
                self.tabs.len() - 1
            } else {
                self.active_tab_index - 1
            };
        }
    }

    /// アクティブなタブを閉じる
    pub fn close_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        self.tabs.remove(self.active_tab_index);
        if self.tabs.is_empty() {
            self.show_dashboard = true;
            self.active_tab_index = 0;
        } else if self.active_tab_index >= self.tabs.len() {
            self.active_tab_index = self.tabs.len() - 1;
        }
    }

    /// サービスピッカーを開く
    fn open_service_picker(&mut self) {
        self.service_picker = Some(ServicePickerState {
            selected_index: 0,
            filter_input: Input::default(),
            filtered_services: ServiceKind::ALL.to_vec(),
        });
    }

    /// ピッカーで選択されたサービスのタブを作成
    fn picker_confirm(&mut self) {
        let Some(picker) = &self.service_picker else {
            return;
        };
        let Some(service) = picker.filtered_services.get(picker.selected_index).copied() else {
            return;
        };
        self.service_picker = None;
        self.create_tab(service);
    }

    /// ピッカーの選択を上に移動
    fn picker_move_up(&mut self) {
        if let Some(picker) = &mut self.service_picker {
            picker.selected_index = picker.selected_index.saturating_sub(1);
        }
    }

    /// ピッカーの選択を下に移動
    fn picker_move_down(&mut self) {
        if let Some(picker) = &mut self.service_picker {
            let max = picker.filtered_services.len().saturating_sub(1);
            if picker.selected_index < max {
                picker.selected_index += 1;
            }
        }
    }

    /// ピッカーのフィルタ入力を処理
    fn picker_handle_input(&mut self, req: tui_input::InputRequest) {
        if let Some(picker) = &mut self.service_picker {
            picker.filter_input.handle(req);
            let filter_text = picker.filter_input.value().to_string();
            picker.filtered_services = crate::fuzzy::fuzzy_filter_items(
                ServiceKind::ALL,
                &filter_text,
                0,
                |s: &ServiceKind| vec![s.full_name()],
            );
            if picker.selected_index >= picker.filtered_services.len() {
                picker.selected_index = picker.filtered_services.len().saturating_sub(1);
            }
        }
    }

    /// 指定サービスの削除操作が許可されているか
    pub fn can_delete(&self, service: &str) -> bool {
        self.delete_permissions.can_delete(service)
    }

    /// アクティブタブで選択中のインスタンスを返す
    pub fn selected_instance(&self) -> Option<&Instance> {
        let tab = self.active_tab()?;
        if let crate::tab::ServiceData::Ec2 {
            filtered_instances, ..
        } = &tab.data
        {
            filtered_instances.get(tab.selected_index)
        } else {
            None
        }
    }

    /// アクティブタブの(service, tab_view)を返す
    pub fn current_view(&self) -> Option<(ServiceKind, crate::tab::TabView)> {
        let tab = self.active_tab()?;
        Some((tab.service, tab.tab_view))
    }

    /// フィルタを適用
    pub fn apply_filter(&mut self) {
        if self.show_dashboard {
            let filter_text = self.dashboard.filter_input.value().to_string();
            self.dashboard.filtered_services =
                fuzzy_filter_items(ServiceKind::ALL, &filter_text, 0, |s: &ServiceKind| {
                    vec![s.full_name()]
                });
            // Recently Used もフィルタ適用
            self.dashboard.recent_services = if filter_text.is_empty() {
                self.dashboard.all_recent_services.clone()
            } else {
                fuzzy_filter_items(
                    &self.dashboard.all_recent_services,
                    &filter_text,
                    0,
                    |s: &ServiceKind| vec![s.full_name()],
                )
            };
            let total = self.dashboard.item_count();
            if total > 0 && self.dashboard.selected_index >= total {
                self.dashboard.selected_index = total - 1;
            }
            return;
        }
        let Some(tab) = self.active_tab_mut() else {
            return;
        };
        tab.apply_filter();
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
    }

    /// メッセージダイアログを閉じる
    pub fn dismiss_message(&mut self) {
        self.message = None;
    }

    /// Actionに基づいてApp状態を更新する。
    /// 副作用が必要な場合はSideEffectを返す（main側でAPI呼び出しに使う）。
    pub fn dispatch(&mut self, action: Action) -> SideEffect {
        // グローバルオーバーレイの処理
        if self.message.is_some() {
            match action {
                Action::DismissMessage | Action::Back => {
                    self.dismiss_message();
                    return SideEffect::None;
                }
                _ => return SideEffect::None,
            }
        }
        if self.show_help {
            match action {
                Action::ShowHelp | Action::Back => {
                    self.show_help = false;
                    return SideEffect::None;
                }
                _ => return SideEffect::None,
            }
        }

        // タブ固有モードの処理
        if !self.show_dashboard
            && let Some(tab) = self.active_tab()
        {
            match &tab.mode {
                Mode::Confirm(_) => match action {
                    Action::ConfirmYes => return self.handle_confirm_yes(),
                    Action::ConfirmNo => {
                        if let Some(tab) = self.active_tab_mut() {
                            tab.mode = Mode::Normal;
                        }
                        return SideEffect::None;
                    }
                    _ => return SideEffect::None,
                },
                Mode::Filter => match action {
                    Action::ConfirmFilter => {
                        if self.is_in_log_view() {
                            self.log_search_confirm();
                        } else if let Some(tab) = self.active_tab_mut() {
                            tab.mode = Mode::Normal;
                        }
                        return SideEffect::None;
                    }
                    Action::CancelFilter => {
                        if self.is_in_log_view() {
                            if let Some(tab) = self.active_tab_mut() {
                                tab.filter_input.reset();
                                tab.mode = Mode::Normal;
                            }
                        } else {
                            if let Some(tab) = self.active_tab_mut() {
                                tab.mode = Mode::Normal;
                                tab.filter_input.reset();
                            }
                            self.apply_filter();
                        }
                        return SideEffect::None;
                    }
                    Action::FilterHandleInput(req) => {
                        if self.is_in_log_view() {
                            if let Some(tab) = self.active_tab_mut() {
                                tab.filter_input.handle(req);
                            }
                        } else {
                            if let Some(tab) = self.active_tab_mut() {
                                tab.filter_input.handle(req);
                            }
                            self.apply_filter();
                        }
                        return SideEffect::None;
                    }
                    _ => return SideEffect::None,
                },
                Mode::Form(_) => match action {
                    Action::FormSubmit => return self.handle_form_submit(),
                    Action::FormCancel => {
                        if let Some(tab) = self.active_tab_mut() {
                            tab.mode = Mode::Normal;
                        }
                        return SideEffect::None;
                    }
                    Action::FormNextField => {
                        self.handle_form_next_field();
                        return SideEffect::None;
                    }
                    Action::FormHandleInput(req) => {
                        self.handle_form_input(req);
                        return SideEffect::None;
                    }
                    _ => return SideEffect::None,
                },
                Mode::DangerConfirm(_) => match action {
                    Action::DangerConfirmSubmit => return self.handle_danger_confirm_submit(),
                    Action::DangerConfirmCancel => {
                        if let Some(tab) = self.active_tab_mut() {
                            tab.mode = Mode::Normal;
                        }
                        return SideEffect::None;
                    }
                    Action::DangerConfirmHandleInput(req) => {
                        self.handle_danger_confirm_input(req);
                        return SideEffect::None;
                    }
                    _ => return SideEffect::None,
                },
                Mode::ContainerSelect { names, selected } => match action {
                    Action::ContainerSelectConfirm => {
                        let name = names.get(*selected).cloned();
                        if let Some(tab) = self.active_tab_mut() {
                            tab.mode = Mode::Normal;
                        }
                        if let Some(container_name) = name {
                            // pending_log_configsから選択されたコンテナのconfigを見つけてlog_stateを作成
                            if let Some((_config_tab_id, configs)) = self.pending_log_configs.take()
                                && let Some(config) =
                                    configs.iter().find(|c| c.container_name == container_name)
                            {
                                let log_group = config.log_group.clone().unwrap_or_default();
                                let tab_id = self.active_tab().map(|t| t.id);
                                let task_id = self.active_tab().and_then(|tab| {
                                    if let crate::tab::ServiceData::Ecs {
                                        tasks,
                                        selected_task_index,
                                        ..
                                    } = &tab.data
                                    {
                                        selected_task_index.and_then(|idx| tasks.get(idx)).map(
                                            |t| {
                                                t.task_arn
                                                    .rsplit('/')
                                                    .next()
                                                    .unwrap_or(&t.task_arn)
                                                    .to_string()
                                            },
                                        )
                                    } else {
                                        None
                                    }
                                });
                                if let (Some(_tab_id), Some(task_id)) = (tab_id, task_id) {
                                    let stream_prefix =
                                        config.stream_prefix.as_deref().unwrap_or_default();
                                    let log_stream =
                                        format!("{}/{}/{}", stream_prefix, container_name, task_id);
                                    if let Some(tab) = self.active_tab_mut()
                                        && let crate::tab::ServiceData::Ecs { log_state, .. } =
                                            &mut tab.data
                                    {
                                        *log_state = Some(Box::new(crate::tab::LogViewState {
                                            container_name,
                                            log_group,
                                            log_stream,
                                            events: Vec::new(),
                                            next_forward_token: None,
                                            auto_scroll: true,
                                            scroll_offset: 0,
                                            search_query: String::new(),
                                            search_matches: Vec::new(),
                                            current_match_index: None,
                                        }));
                                        tab.loading = true;
                                    }
                                }
                            }
                        }
                        return SideEffect::None;
                    }
                    Action::ContainerSelectCancel => {
                        if let Some(tab) = self.active_tab_mut() {
                            tab.mode = Mode::Normal;
                        }
                        return SideEffect::None;
                    }
                    Action::ContainerSelectUp => {
                        if let Some(tab) = self.active_tab_mut()
                            && let Mode::ContainerSelect { selected, .. } = &mut tab.mode
                        {
                            *selected = selected.saturating_sub(1);
                        }
                        return SideEffect::None;
                    }
                    Action::ContainerSelectDown => {
                        if let Some(tab) = self.active_tab_mut()
                            && let Mode::ContainerSelect { names, selected } = &mut tab.mode
                        {
                            let max = names.len().saturating_sub(1);
                            if *selected < max {
                                *selected += 1;
                            }
                        }
                        return SideEffect::None;
                    }
                    _ => return SideEffect::None,
                },
                _ => {}
            }
        }

        // ダッシュボードのフィルタモード処理
        if self.show_dashboard && self.dashboard.mode == Mode::Filter {
            match action {
                Action::ConfirmFilter => {
                    self.dashboard.mode = Mode::Normal;
                    return SideEffect::None;
                }
                Action::CancelFilter => {
                    self.dashboard.mode = Mode::Normal;
                    self.dashboard.filter_input.reset();
                    self.apply_filter();
                    return SideEffect::None;
                }
                Action::FilterHandleInput(req) => {
                    self.dashboard.filter_input.handle(req);
                    self.apply_filter();
                    return SideEffect::None;
                }
                _ => return SideEffect::None,
            }
        }

        // Normal モードのアクション
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
            Action::Refresh => {
                if let Some(tab) = self.active_tab_mut() {
                    tab.loading = true;
                }
            }
            Action::CopyId => self.copy_id(),
            Action::StartFilter => {
                if self.show_dashboard {
                    self.dashboard.mode = Mode::Filter;
                } else if let Some(tab) = self.active_tab_mut() {
                    tab.mode = Mode::Filter;
                }
            }
            Action::StartStop => self.handle_start_stop(),
            Action::Reboot => self.handle_reboot(),
            Action::DismissMessage => self.dismiss_message(),
            Action::ShowHelp => self.show_help = true,
            Action::SwitchDetailTab => self.switch_detail_tab(),
            Action::PrevDetailTab => self.prev_detail_tab(),
            Action::RevealSecretValue => self.reveal_secret_value(),
            Action::FollowLink => self.handle_follow_link(),
            Action::Create => self.handle_create(),
            Action::Delete => self.handle_delete(),
            Action::Edit => self.handle_edit(),
            Action::NextTab => self.switch_tab_next(),
            Action::PrevTab => self.switch_tab_prev(),
            Action::CloseTab => self.close_tab(),
            Action::NewTab => self.open_service_picker(),
            Action::PickerConfirm => self.picker_confirm(),
            Action::PickerCancel => self.service_picker = None,
            Action::PickerMoveUp => self.picker_move_up(),
            Action::PickerMoveDown => self.picker_move_down(),
            Action::PickerHandleInput(req) => self.picker_handle_input(req),
            Action::ShowLogs => {
                // タスク詳細画面でログ表示開始（main.rsでAPI呼び出し）
                if let Some(tab) = self.active_tab_mut() {
                    tab.loading = true;
                }
            }
            Action::LogScrollUp => self.log_scroll_up(),
            Action::LogScrollDown => self.log_scroll_down(),
            Action::LogScrollToTop => self.log_scroll_to_top(),
            Action::LogScrollToBottom => self.log_scroll_to_bottom(),
            Action::LogToggleAutoScroll => self.log_toggle_auto_scroll(),
            Action::LogSearchNext => self.log_search_next(),
            Action::LogSearchPrev => self.log_search_prev(),
            Action::Noop
            | Action::ConfirmYes
            | Action::ConfirmNo
            | Action::ConfirmFilter
            | Action::CancelFilter
            | Action::FilterHandleInput(_)
            | Action::FormSubmit
            | Action::FormCancel
            | Action::FormNextField
            | Action::FormHandleInput(_)
            | Action::DangerConfirmSubmit
            | Action::DangerConfirmCancel
            | Action::DangerConfirmHandleInput(_)
            | Action::ContainerSelectUp
            | Action::ContainerSelectDown
            | Action::ContainerSelectConfirm
            | Action::ContainerSelectCancel => {}
        }
        SideEffect::None
    }

    /// AppEventをApp状態に反映する。
    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::TabEvent(tab_id, tab_event) => {
                self.handle_tab_event(tab_id, tab_event);
            }
            AppEvent::CrudCompleted(tab_id, result) => match result {
                Ok(msg) => {
                    self.show_message(MessageLevel::Success, "Success", msg);
                    if let Some(tab) = self.find_tab_mut(tab_id) {
                        tab.loading = true;
                    }
                }
                Err(e) => {
                    self.show_message(MessageLevel::Error, "Error", e.to_string());
                }
            },
        }
    }

    /// Common handler for tab events that follow the Ok/Err pattern:
    /// Ok → update data + loading=false + optional apply_filter, Err → loading=false + error message.
    /// `update_fn` returns true if the loaded data is empty.
    fn handle_loaded_result<T, E: std::fmt::Display>(
        &mut self,
        tab_id: crate::tab::TabId,
        result: Result<T, E>,
        empty_message: Option<&str>,
        apply_filter: bool,
        update_fn: impl FnOnce(&mut crate::tab::Tab, T) -> bool,
    ) {
        match result {
            Ok(data) => {
                let is_empty = if let Some(tab) = self.find_tab_mut(tab_id) {
                    let empty = update_fn(tab, data);
                    tab.loading = false;
                    if apply_filter {
                        tab.apply_filter();
                    }
                    empty
                } else {
                    false
                };
                if is_empty && let Some(msg) = empty_message {
                    self.show_message(MessageLevel::Info, "Info", msg);
                }
            }
            Err(e) => {
                if let Some(tab) = self.find_tab_mut(tab_id) {
                    tab.loading = false;
                }
                self.show_message(MessageLevel::Error, "Error", e.to_string());
            }
        }
    }

    fn handle_tab_event(&mut self, tab_id: crate::tab::TabId, tab_event: crate::event::TabEvent) {
        use crate::event::TabEvent;
        use crate::tab::ServiceData;

        match tab_event {
            TabEvent::InstancesLoaded(result) => {
                self.handle_loaded_result(
                    tab_id,
                    result,
                    Some("No instances found"),
                    true,
                    |tab, instances| {
                        let is_empty = instances.is_empty();
                        if let ServiceData::Ec2 {
                            instances: inst,
                            filtered_instances,
                        } = &mut tab.data
                        {
                            *inst = instances;
                            *filtered_instances = inst.clone();
                        }
                        is_empty
                    },
                );
            }
            TabEvent::ActionCompleted(result) => match result {
                Ok(msg) => {
                    self.show_message(MessageLevel::Success, "Success", msg);
                    if let Some(tab) = self.find_tab_mut(tab_id) {
                        tab.loading = true;
                    }
                }
                Err(e) => {
                    self.show_message(MessageLevel::Error, "Error", e.to_string());
                }
            },
            TabEvent::RepositoriesLoaded(result) => {
                self.handle_loaded_result(
                    tab_id,
                    result,
                    Some("No repositories found"),
                    true,
                    |tab, repos| {
                        let is_empty = repos.is_empty();
                        if let ServiceData::Ecr {
                            repositories,
                            filtered_repositories,
                            ..
                        } = &mut tab.data
                        {
                            *repositories = repos;
                            *filtered_repositories = repositories.clone();
                        }
                        is_empty
                    },
                );
            }
            TabEvent::ImagesLoaded(result) => {
                self.handle_loaded_result(
                    tab_id,
                    result,
                    Some("No images found"),
                    false,
                    |tab, images| {
                        let is_empty = images.is_empty();
                        if let ServiceData::Ecr { images: imgs, .. } = &mut tab.data {
                            *imgs = images;
                        }
                        is_empty
                    },
                );
            }
            TabEvent::ClustersLoaded(result) => {
                self.handle_loaded_result(
                    tab_id,
                    result,
                    Some("No clusters found"),
                    true,
                    |tab, clusters| {
                        let is_empty = clusters.is_empty();
                        if let ServiceData::Ecs {
                            clusters: cls,
                            filtered_clusters,
                            ..
                        } = &mut tab.data
                        {
                            *cls = clusters;
                            *filtered_clusters = cls.clone();
                        }
                        is_empty
                    },
                );
            }
            TabEvent::EcsServicesLoaded(result) => {
                self.handle_loaded_result(
                    tab_id,
                    result,
                    Some("No services found"),
                    false,
                    |tab, services| {
                        let is_empty = services.is_empty();
                        if let ServiceData::Ecs { services: svcs, .. } = &mut tab.data {
                            *svcs = services;
                        }
                        is_empty
                    },
                );
            }
            TabEvent::EcsTasksLoaded(result) => {
                self.handle_loaded_result(
                    tab_id,
                    result,
                    Some("No tasks found"),
                    false,
                    |tab, tasks| {
                        let is_empty = tasks.is_empty();
                        if let ServiceData::Ecs { tasks: t, .. } = &mut tab.data {
                            *t = tasks;
                        }
                        is_empty
                    },
                );
            }
            TabEvent::BucketsLoaded(result) => {
                self.handle_loaded_result(
                    tab_id,
                    result,
                    Some("No buckets found"),
                    true,
                    |tab, buckets| {
                        let is_empty = buckets.is_empty();
                        if let ServiceData::S3 {
                            buckets: bkts,
                            filtered_buckets,
                            ..
                        } = &mut tab.data
                        {
                            *bkts = buckets;
                            *filtered_buckets = bkts.clone();
                        }
                        is_empty
                    },
                );
            }
            TabEvent::ObjectsLoaded(result) => {
                self.handle_loaded_result(tab_id, result, None, false, |tab, objects| {
                    if let ServiceData::S3 { objects: objs, .. } = &mut tab.data {
                        *objs = objects;
                    }
                    false
                });
            }
            TabEvent::VpcsLoaded(result) => {
                self.handle_loaded_result(
                    tab_id,
                    result,
                    Some("No VPCs found"),
                    true,
                    |tab, vpcs| {
                        let is_empty = vpcs.is_empty();
                        if let ServiceData::Vpc {
                            vpcs: vs,
                            filtered_vpcs,
                            ..
                        } = &mut tab.data
                        {
                            *vs = vpcs;
                            *filtered_vpcs = vs.clone();
                        }
                        is_empty
                    },
                );
            }
            TabEvent::SubnetsLoaded(result) => {
                self.handle_loaded_result(
                    tab_id,
                    result,
                    Some("No subnets found"),
                    false,
                    |tab, subnets| {
                        let is_empty = subnets.is_empty();
                        if let ServiceData::Vpc { subnets: subs, .. } = &mut tab.data {
                            *subs = subnets;
                        }
                        is_empty
                    },
                );
            }
            TabEvent::SecretsLoaded(result) => {
                self.handle_loaded_result(
                    tab_id,
                    result,
                    Some("No secrets found"),
                    true,
                    |tab, secrets| {
                        let is_empty = secrets.is_empty();
                        if let ServiceData::Secrets {
                            secrets: secs,
                            filtered_secrets,
                            ..
                        } = &mut tab.data
                        {
                            *secs = secrets;
                            *filtered_secrets = secs.clone();
                        }
                        is_empty
                    },
                );
            }
            TabEvent::SecretDetailLoaded(result) => {
                self.handle_loaded_result(tab_id, result, None, false, |tab, detail| {
                    if let ServiceData::Secrets { detail: det, .. } = &mut tab.data {
                        *det = Some(detail);
                    }
                    false
                });
            }
            TabEvent::SecretValueLoaded(result) => {
                self.handle_loaded_result(tab_id, result, None, false, |tab, value| {
                    if let ServiceData::Secrets {
                        detail,
                        value_visible,
                        ..
                    } = &mut tab.data
                    {
                        if let Some(d) = detail {
                            d.secret_value = Some(value);
                        }
                        *value_visible = true;
                    }
                    false
                });
            }
            TabEvent::NavigateVpcLoaded(result) => {
                match result {
                    Ok((vpcs, subnets)) => {
                        if let Some(tab) = self.find_tab_mut(tab_id) {
                            if let ServiceData::Vpc {
                                vpcs: vs,
                                filtered_vpcs,
                                subnets: subs,
                            } = &mut tab.data
                            {
                                *vs = vpcs;
                                *filtered_vpcs = vs.clone();
                                *subs = subnets;
                            }
                            tab.loading = false;
                        }
                    }
                    Err(e) => {
                        // ナビゲーション失敗時はスタックを巻き戻す
                        if let Some(tab) = self.find_tab_mut(tab_id) {
                            if let Some(entry) = tab.navigation_stack.pop() {
                                tab.selected_index = entry.selected_index;
                                tab.detail_tag_index = entry.detail_tag_index;
                                tab.detail_tab = entry.detail_tab;
                            }
                            tab.loading = false;
                        }
                        self.show_message(MessageLevel::Error, "Error", e.to_string());
                    }
                }
            }
            TabEvent::EcsLogConfigsLoaded(result) => match result {
                Ok(configs) => {
                    if let Some(tab) = self.find_tab_mut(tab_id) {
                        tab.loading = false;
                    }
                    if configs.is_empty() {
                        self.show_message(
                            MessageLevel::Error,
                            "Error",
                            "No awslogs configuration found",
                        );
                    } else if configs.len() == 1 {
                        // コンテナが1つ → 直接ログ取得開始
                        let config = &configs[0];
                        let Some(log_group) = config.log_group.clone() else {
                            self.show_message(
                                MessageLevel::Error,
                                "Error",
                                "No log group configured",
                            );
                            return;
                        };
                        // ログストリーム名を構築
                        let task_id = self.find_tab(tab_id).and_then(|tab| {
                            if let ServiceData::Ecs {
                                tasks,
                                selected_task_index,
                                ..
                            } = &tab.data
                            {
                                selected_task_index.and_then(|idx| tasks.get(idx)).map(|t| {
                                    t.task_arn
                                        .rsplit('/')
                                        .next()
                                        .unwrap_or(&t.task_arn)
                                        .to_string()
                                })
                            } else {
                                None
                            }
                        });
                        let Some(task_id) = task_id else { return };
                        let stream_prefix = config.stream_prefix.as_deref().unwrap_or_default();
                        let log_stream =
                            format!("{}/{}/{}", stream_prefix, config.container_name, task_id);
                        if let Some(tab) = self.find_tab_mut(tab_id) {
                            if let crate::tab::ServiceData::Ecs { log_state, .. } = &mut tab.data {
                                *log_state = Some(Box::new(crate::tab::LogViewState {
                                    container_name: config.container_name.clone(),
                                    log_group,
                                    log_stream,
                                    events: Vec::new(),
                                    next_forward_token: None,
                                    auto_scroll: true,
                                    scroll_offset: 0,
                                    search_query: String::new(),
                                    search_matches: Vec::new(),
                                    current_match_index: None,
                                }));
                            }
                            tab.loading = true;
                        }
                    } else {
                        // 複数コンテナ → 選択ダイアログ
                        let names: Vec<String> =
                            configs.iter().map(|c| c.container_name.clone()).collect();
                        if let Some(tab) = self.find_tab_mut(tab_id) {
                            tab.mode = Mode::ContainerSelect { names, selected: 0 };
                        }
                        // configs を一時保存（後でContainerSelectConfirmで使用）
                        // ContainerSelectConfirm時にmain.rsで再度describe_task_definition_log_configsを
                        // 呼ぶのは非効率なので、pending_log_configsに保存
                        self.pending_log_configs = Some((tab_id, configs));
                    }
                }
                Err(e) => {
                    if let Some(tab) = self.find_tab_mut(tab_id) {
                        tab.loading = false;
                    }
                    self.show_message(MessageLevel::Error, "Error", e.to_string());
                }
            },
            TabEvent::EcsLogEventsLoaded(result) => match result {
                Ok((events, next_token)) => {
                    if let Some(tab) = self.find_tab_mut(tab_id) {
                        if let crate::tab::ServiceData::Ecs { log_state, .. } = &mut tab.data
                            && let Some(state) = log_state
                        {
                            state.events.extend(events);
                            state.next_forward_token = next_token;
                            if state.auto_scroll {
                                state.scroll_offset = state.events.len().saturating_sub(1);
                            }
                            // 検索クエリがあればマッチを再計算
                            if !state.search_query.is_empty() {
                                state.recompute_search_matches();
                            }
                        }
                        tab.loading = false;
                    }
                }
                Err(e) => {
                    if let Some(tab) = self.find_tab_mut(tab_id) {
                        tab.loading = false;
                    }
                    self.show_message(MessageLevel::Error, "Error", e.to_string());
                }
            },
        }
    }

    fn log_scroll_up(&mut self) {
        if let Some(state) = self.active_log_state_mut() {
            state.scroll_up();
        }
    }

    fn log_scroll_down(&mut self) {
        if let Some(state) = self.active_log_state_mut() {
            state.scroll_down();
        }
    }

    fn log_scroll_to_top(&mut self) {
        if let Some(state) = self.active_log_state_mut() {
            state.scroll_to_top();
        }
    }

    fn log_scroll_to_bottom(&mut self) {
        if let Some(state) = self.active_log_state_mut() {
            state.scroll_to_bottom();
        }
    }

    fn log_toggle_auto_scroll(&mut self) {
        if let Some(state) = self.active_log_state_mut() {
            state.toggle_auto_scroll();
        }
    }

    /// アクティブタブがログビューかどうかを判定
    fn is_in_log_view(&self) -> bool {
        self.active_tab().is_some_and(|tab| tab.is_in_log_view())
    }

    /// 検索確定: filter_input の値を search_query にコピーし、マッチを計算
    fn log_search_confirm(&mut self) {
        let Some(tab) = self.active_tab_mut() else {
            return;
        };
        let query = tab.filter_input.value().to_lowercase();
        tab.mode = Mode::Normal;

        if let Some(state) = tab.log_state_mut() {
            state.apply_search(&query);
        }
    }

    fn log_search_next(&mut self) {
        if let Some(state) = self.active_log_state_mut() {
            state.search_next();
        }
    }

    fn log_search_prev(&mut self) {
        if let Some(state) = self.active_log_state_mut() {
            state.search_prev();
        }
    }

    fn move_up(&mut self) {
        if self.show_dashboard {
            self.dashboard.selected_index = self.dashboard.selected_index.saturating_sub(1);
            return;
        }
        if let Some(tab) = self.active_tab_mut() {
            tab.move_up();
        }
    }

    fn move_down(&mut self) {
        if self.show_dashboard {
            let max = self.dashboard.item_count().saturating_sub(1);
            if self.dashboard.selected_index < max {
                self.dashboard.selected_index += 1;
            }
            return;
        }
        if let Some(tab) = self.active_tab_mut() {
            tab.move_down();
        }
    }

    fn move_to_top(&mut self) {
        if self.show_dashboard {
            self.dashboard.selected_index = 0;
            return;
        }
        if let Some(tab) = self.active_tab_mut() {
            tab.move_to_top();
        }
    }

    fn move_to_bottom(&mut self) {
        if self.show_dashboard {
            self.dashboard.selected_index = self.dashboard.item_count().saturating_sub(1);
            return;
        }
        if let Some(tab) = self.active_tab_mut() {
            tab.move_to_bottom();
        }
    }

    fn half_page_up(&mut self) {
        if self.show_dashboard {
            self.dashboard.selected_index = self.dashboard.selected_index.saturating_sub(10);
            return;
        }
        if let Some(tab) = self.active_tab_mut() {
            tab.half_page_up();
        }
    }

    fn half_page_down(&mut self) {
        if self.show_dashboard {
            let max = self.dashboard.item_count().saturating_sub(1);
            self.dashboard.selected_index = (self.dashboard.selected_index + 10).min(max);
            return;
        }
        if let Some(tab) = self.active_tab_mut() {
            tab.half_page_down();
        }
    }

    fn handle_enter(&mut self) {
        if self.show_dashboard {
            let Some(service) = self.dashboard.selected_service() else {
                return;
            };
            self.create_tab(service);
            return;
        }
        if let Some(tab) = self.active_tab_mut() {
            tab.handle_enter();
        }
    }

    fn handle_back(&mut self) {
        if self.show_dashboard {
            return;
        }
        if let Some(tab) = self.active_tab_mut() {
            tab.handle_back();
        }
    }

    fn copy_id(&self) {
        if let Some(tab) = self.active_tab() {
            tab.copy_id();
        }
    }

    fn handle_start_stop(&mut self) {
        let instance_data = self
            .selected_instance()
            .map(|i| (i.instance_id.clone(), i.state.clone()));
        let Some((id, state)) = instance_data else {
            return;
        };
        let confirm = match state {
            InstanceState::Running => Some(ConfirmAction::Stop(id)),
            InstanceState::Stopped => Some(ConfirmAction::Start(id)),
            _ => None,
        };
        if let Some(action) = confirm
            && let Some(tab) = self.active_tab_mut()
        {
            tab.mode = Mode::Confirm(action);
        }
    }

    fn handle_reboot(&mut self) {
        let id = self.selected_instance().map(|i| i.instance_id.clone());
        let Some(id) = id else {
            return;
        };
        if let Some(tab) = self.active_tab_mut() {
            tab.mode = Mode::Confirm(ConfirmAction::Reboot(id));
        }
    }

    fn handle_confirm_yes(&mut self) -> SideEffect {
        let Some(tab) = self.active_tab_mut() else {
            return SideEffect::None;
        };
        let confirmed = if let Mode::Confirm(action) = &tab.mode {
            Some(action.clone())
        } else {
            None
        };
        tab.mode = Mode::Normal;
        match confirmed {
            Some(action) => SideEffect::Confirm(action),
            None => SideEffect::None,
        }
    }

    fn switch_detail_tab(&mut self) {
        if let Some(tab) = self.active_tab_mut() {
            tab.switch_detail_tab();
        }
    }

    fn prev_detail_tab(&mut self) {
        if let Some(tab) = self.active_tab_mut() {
            tab.prev_detail_tab();
        }
    }

    fn reveal_secret_value(&mut self) {
        let Some(tab) = self.active_tab_mut() else {
            return;
        };
        if let crate::tab::ServiceData::Secrets {
            detail: Some(d),
            value_visible,
            ..
        } = &mut tab.data
        {
            if d.secret_value.is_some() {
                *value_visible = !*value_visible;
            } else {
                tab.loading = true;
            }
        }
    }

    /// EC2 Detail Overviewタブでリンクをフォローする（未実装、将来対応）
    fn handle_follow_link(&mut self) {
        // no-op for now
    }

    /// Create操作のハンドリング
    fn handle_create(&mut self) {
        let Some(view) = self.current_view() else {
            return;
        };
        let form_ctx = match view {
            (ServiceKind::S3, crate::tab::TabView::List) => Some(FormContext {
                kind: FormKind::CreateS3Bucket,
                fields: vec![FormField {
                    label: "Bucket Name".to_string(),
                    input: Input::default(),
                    required: true,
                }],
                focused_field: 0,
            }),
            (ServiceKind::SecretsManager, crate::tab::TabView::List) => Some(FormContext {
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
            }),
            _ => None,
        };
        if let Some(ctx) = form_ctx
            && let Some(tab) = self.active_tab_mut()
        {
            tab.mode = Mode::Form(ctx);
        }
    }

    /// Delete操作のハンドリング
    fn handle_delete(&mut self) {
        let Some(view) = self.current_view() else {
            return;
        };
        match view {
            (ServiceKind::Ec2, crate::tab::TabView::List) => {
                if !self.can_delete("ec2") {
                    self.show_message(
                        MessageLevel::Error,
                        "Permission Denied",
                        "Delete not allowed. Use --allow-delete=ec2 or --allow-delete",
                    );
                    return;
                }
                let id = self.selected_instance().map(|i| i.instance_id.clone());
                let Some(id) = id else {
                    return;
                };
                if let Some(tab) = self.active_tab_mut() {
                    tab.mode = Mode::DangerConfirm(DangerConfirmContext {
                        action: DangerAction::TerminateEc2(id),
                        input: Input::default(),
                    });
                }
            }
            (ServiceKind::S3, crate::tab::TabView::List) => {
                if !self.can_delete("s3") {
                    self.show_message(
                        MessageLevel::Error,
                        "Permission Denied",
                        "Delete not allowed. Use --allow-delete=s3 or --allow-delete",
                    );
                    return;
                }
                let bucket_name = self.active_tab().and_then(|tab| {
                    if let crate::tab::ServiceData::S3 {
                        filtered_buckets, ..
                    } = &tab.data
                    {
                        filtered_buckets
                            .get(tab.selected_index)
                            .map(|b| b.name.clone())
                    } else {
                        None
                    }
                });
                let Some(name) = bucket_name else {
                    return;
                };
                if let Some(tab) = self.active_tab_mut() {
                    tab.mode = Mode::DangerConfirm(DangerConfirmContext {
                        action: DangerAction::DeleteS3Bucket(name),
                        input: Input::default(),
                    });
                }
            }
            (ServiceKind::S3, crate::tab::TabView::Detail) => {
                if !self.can_delete("s3") {
                    self.show_message(
                        MessageLevel::Error,
                        "Permission Denied",
                        "Delete not allowed. Use --allow-delete=s3 or --allow-delete",
                    );
                    return;
                }
                let obj_info = self.active_tab().and_then(|tab| {
                    if let crate::tab::ServiceData::S3 {
                        objects,
                        selected_bucket,
                        ..
                    } = &tab.data
                    {
                        objects.get(tab.detail_tag_index).and_then(|obj| {
                            if !obj.is_prefix {
                                Some((selected_bucket.clone().unwrap_or_default(), obj.key.clone()))
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    }
                });
                let Some((bucket, key)) = obj_info else {
                    return;
                };
                if let Some(tab) = self.active_tab_mut() {
                    tab.mode = Mode::DangerConfirm(DangerConfirmContext {
                        action: DangerAction::DeleteS3Object { bucket, key },
                        input: Input::default(),
                    });
                }
            }
            (ServiceKind::SecretsManager, crate::tab::TabView::List) => {
                if !self.can_delete("secretsmanager") {
                    self.show_message(
                        MessageLevel::Error,
                        "Permission Denied",
                        "Delete not allowed. Use --allow-delete=secretsmanager or --allow-delete",
                    );
                    return;
                }
                let secret_name = self.active_tab().and_then(|tab| {
                    if let crate::tab::ServiceData::Secrets {
                        filtered_secrets, ..
                    } = &tab.data
                    {
                        filtered_secrets
                            .get(tab.selected_index)
                            .map(|s| s.name.clone())
                    } else {
                        None
                    }
                });
                let Some(name) = secret_name else {
                    return;
                };
                if let Some(tab) = self.active_tab_mut() {
                    tab.mode = Mode::DangerConfirm(DangerConfirmContext {
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
        let Some(view) = self.current_view() else {
            return;
        };
        if view != (ServiceKind::SecretsManager, crate::tab::TabView::Detail) {
            return;
        }
        let detail_name = self.active_tab().and_then(|tab| {
            if let crate::tab::ServiceData::Secrets { detail, .. } = &tab.data {
                detail.as_ref().map(|d| d.name.clone())
            } else {
                None
            }
        });
        let Some(name) = detail_name else {
            return;
        };
        if let Some(tab) = self.active_tab_mut() {
            tab.mode = Mode::Form(FormContext {
                kind: FormKind::UpdateSecretValue,
                fields: vec![FormField {
                    label: format!("New value for '{}'", name),
                    input: Input::default(),
                    required: true,
                }],
                focused_field: 0,
            });
        }
    }

    /// FormSubmitのハンドリング
    fn handle_form_submit(&mut self) -> SideEffect {
        let Some(tab) = self.active_tab() else {
            return SideEffect::None;
        };
        let Mode::Form(ctx) = &tab.mode else {
            return SideEffect::None;
        };

        // 必須フィールドのバリデーション
        for field in &ctx.fields {
            if field.required && field.input.value().is_empty() {
                let msg = format!("'{}' is required", field.label);
                self.show_message(MessageLevel::Error, "Validation Error", msg);
                return SideEffect::None;
            }
        }

        // FormContextを取り出してNormalに戻す
        let Some(tab) = self.active_tab_mut() else {
            return SideEffect::None;
        };
        let Mode::Form(ctx) = std::mem::replace(&mut tab.mode, Mode::Normal) else {
            return SideEffect::None;
        };
        if let Some(tab) = self.active_tab_mut() {
            tab.loading = true;
        }
        SideEffect::FormSubmit(ctx)
    }

    /// フォームの次のフィールドにフォーカスを移動
    fn handle_form_next_field(&mut self) {
        if let Some(tab) = self.active_tab_mut()
            && let Mode::Form(ctx) = &mut tab.mode
        {
            ctx.focused_field = (ctx.focused_field + 1) % ctx.fields.len();
        }
    }

    /// フォーム入力のハンドリング
    fn handle_form_input(&mut self, req: tui_input::InputRequest) {
        if let Some(tab) = self.active_tab_mut()
            && let Mode::Form(ctx) = &mut tab.mode
            && let Some(field) = ctx.fields.get_mut(ctx.focused_field)
        {
            field.input.handle(req);
        }
    }

    /// DangerConfirmSubmitのハンドリング
    fn handle_danger_confirm_submit(&mut self) -> SideEffect {
        let Some(tab) = self.active_tab() else {
            return SideEffect::None;
        };
        let Mode::DangerConfirm(ctx) = &tab.mode else {
            return SideEffect::None;
        };

        if ctx.input.value() != ctx.action.confirm_text() {
            return SideEffect::None;
        }

        let Some(tab) = self.active_tab_mut() else {
            return SideEffect::None;
        };
        let Mode::DangerConfirm(ctx) = std::mem::replace(&mut tab.mode, Mode::Normal) else {
            return SideEffect::None;
        };
        if let Some(tab) = self.active_tab_mut() {
            tab.loading = true;
        }
        SideEffect::DangerAction(ctx.action)
    }

    /// DangerConfirm入力のハンドリング
    fn handle_danger_confirm_input(&mut self, req: tui_input::InputRequest) {
        if let Some(tab) = self.active_tab_mut()
            && let Mode::DangerConfirm(ctx) = &mut tab.mode
        {
            ctx.input.handle(req);
        }
    }

    /// パンくずリスト文字列を生成する
    pub fn breadcrumb(&self) -> Option<String> {
        let tab = self.active_tab()?;
        if tab.navigation_stack.is_empty() {
            return None;
        }

        let mut parts: Vec<&str> = tab
            .navigation_stack
            .iter()
            .map(|e| e.label.as_str())
            .collect();

        // 現在のビューのラベルを追加
        let current_label = match self.current_view()? {
            (ServiceKind::Vpc, crate::tab::TabView::Detail) => {
                if let crate::tab::ServiceData::Vpc { filtered_vpcs, .. } = &tab.data {
                    filtered_vpcs
                        .first()
                        .map(|v| v.vpc_id.as_str())
                        .unwrap_or("VPC")
                } else {
                    ""
                }
            }
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
    use crate::event::TabEvent;
    use crate::tab::{ServiceData, TabView};
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

    fn app_with_ec2_tab() -> App {
        let mut app = App::new("dev".to_string(), None);
        app.create_tab(ServiceKind::Ec2);
        app
    }

    fn set_ec2_instances(app: &mut App, instances: Vec<Instance>) {
        let tab = app.active_tab_mut().unwrap();
        if let ServiceData::Ec2 {
            instances: inst,
            filtered_instances,
            ..
        } = &mut tab.data
        {
            *filtered_instances = instances.clone();
            *inst = instances;
        }
        tab.loading = false;
    }

    // ──────────────────────────────────────────────
    // create_tab テスト
    // ──────────────────────────────────────────────

    #[test]
    fn create_tab_creates_tab_and_switches_to_it() {
        let mut app = App::new("dev".to_string(), None);
        assert!(app.tabs.is_empty());
        assert!(app.show_dashboard);

        let id = app.create_tab(ServiceKind::Ec2);
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.active_tab_index, 0);
        assert!(!app.show_dashboard);
        assert_eq!(app.active_tab().unwrap().id, id);
        assert_eq!(app.active_tab().unwrap().service, ServiceKind::Ec2);
    }

    // ──────────────────────────────────────────────
    // dispatch Quit テスト
    // ──────────────────────────────────────────────

    #[test]
    fn dispatch_sets_should_quit_when_quit_action() {
        let mut app = App::new("dev".to_string(), None);
        let result = app.dispatch(Action::Quit);
        assert!(app.should_quit);
        assert_eq!(result, SideEffect::None);
    }

    // ──────────────────────────────────────────────
    // ダッシュボード移動テスト
    // ──────────────────────────────────────────────

    #[test]
    fn move_down_increments_dashboard_selected_index_when_on_dashboard() {
        let mut app = App::new("dev".to_string(), None);
        assert!(app.show_dashboard);
        assert_eq!(app.dashboard.selected_index, 0);
        app.dispatch(Action::MoveDown);
        assert_eq!(app.dashboard.selected_index, 1);
    }

    #[test]
    fn move_up_decrements_dashboard_selected_index_when_on_dashboard() {
        let mut app = App::new("dev".to_string(), None);
        app.dashboard.selected_index = 2;
        app.dispatch(Action::MoveUp);
        assert_eq!(app.dashboard.selected_index, 1);
    }

    #[test]
    fn move_to_top_sets_dashboard_index_to_zero() {
        let mut app = App::new("dev".to_string(), None);
        app.dashboard.selected_index = 3;
        app.dispatch(Action::MoveToTop);
        assert_eq!(app.dashboard.selected_index, 0);
    }

    #[test]
    fn move_to_bottom_sets_dashboard_index_to_last() {
        let mut app = App::new("dev".to_string(), None);
        app.dashboard.recent_services.clear();
        app.dispatch(Action::MoveToBottom);
        assert_eq!(app.dashboard.selected_index, ServiceKind::ALL.len() - 1);
    }

    // ──────────────────────────────────────────────
    // タブリスト移動テスト
    // ──────────────────────────────────────────────

    #[test]
    fn move_down_increments_tab_selected_index_when_on_tab_list() {
        let mut app = app_with_ec2_tab();
        set_ec2_instances(
            &mut app,
            vec![
                create_test_instance("i-001", "a", InstanceState::Running),
                create_test_instance("i-002", "b", InstanceState::Stopped),
            ],
        );
        app.dispatch(Action::MoveDown);
        assert_eq!(app.active_tab().unwrap().selected_index, 1);
    }

    #[test]
    fn move_up_decrements_tab_selected_index_when_on_tab_list() {
        let mut app = app_with_ec2_tab();
        set_ec2_instances(
            &mut app,
            vec![
                create_test_instance("i-001", "a", InstanceState::Running),
                create_test_instance("i-002", "b", InstanceState::Stopped),
            ],
        );
        app.active_tab_mut().unwrap().selected_index = 1;
        app.dispatch(Action::MoveUp);
        assert_eq!(app.active_tab().unwrap().selected_index, 0);
    }

    // ──────────────────────────────────────────────
    // タブ詳細移動テスト
    // ──────────────────────────────────────────────

    #[test]
    fn move_down_increments_detail_tag_index_when_on_tab_detail() {
        let mut app = app_with_ec2_tab();
        let mut instance = create_test_instance("i-001", "web", InstanceState::Running);
        instance.tags.insert("env".to_string(), "prod".to_string());
        instance
            .tags
            .insert("team".to_string(), "backend".to_string());
        set_ec2_instances(&mut app, vec![instance]);
        // Switch to detail
        app.active_tab_mut().unwrap().tab_view = TabView::Detail;
        app.active_tab_mut().unwrap().detail_tab = DetailTab::Overview;
        app.dispatch(Action::MoveDown);
        assert_eq!(app.active_tab().unwrap().detail_tag_index, 1);
    }

    #[test]
    fn move_up_decrements_detail_tag_index_when_on_tab_detail() {
        let mut app = app_with_ec2_tab();
        set_ec2_instances(
            &mut app,
            vec![create_test_instance("i-001", "web", InstanceState::Running)],
        );
        let tab = app.active_tab_mut().unwrap();
        tab.tab_view = TabView::Detail;
        tab.detail_tag_index = 1;
        app.dispatch(Action::MoveUp);
        assert_eq!(app.active_tab().unwrap().detail_tag_index, 0);
    }

    // ──────────────────────────────────────────────
    // handle_enter テスト
    // ──────────────────────────────────────────────

    #[test]
    fn handle_enter_creates_tab_when_on_dashboard() {
        let mut app = App::new("dev".to_string(), None);
        app.dashboard.recent_services.clear();
        assert!(app.show_dashboard);
        app.dashboard.selected_index = 0; // EC2 (All Servicesの先頭)
        app.dispatch(Action::Enter);
        assert_eq!(app.tabs.len(), 1);
        assert!(!app.show_dashboard);
        assert_eq!(app.active_tab().unwrap().service, ServiceKind::Ec2);
    }

    // ──────────────────────────────────────────────
    // サービスピッカーテスト
    // ──────────────────────────────────────────────

    #[test]
    fn new_tab_opens_service_picker() {
        let mut app = app_with_ec2_tab();
        assert!(app.service_picker.is_none());
        app.dispatch(Action::NewTab);
        assert!(app.service_picker.is_some());
        assert_eq!(
            app.service_picker.as_ref().unwrap().filtered_services.len(),
            ServiceKind::ALL.len()
        );
    }

    #[test]
    fn picker_confirm_creates_tab_and_closes_picker() {
        let mut app = app_with_ec2_tab();
        app.dispatch(Action::NewTab);
        assert!(app.service_picker.is_some());
        let old_tab_count = app.tabs.len();
        app.dispatch(Action::PickerConfirm);
        assert!(app.service_picker.is_none());
        assert_eq!(app.tabs.len(), old_tab_count + 1);
    }

    #[test]
    fn picker_cancel_closes_picker_without_creating_tab() {
        let mut app = app_with_ec2_tab();
        app.dispatch(Action::NewTab);
        let old_tab_count = app.tabs.len();
        app.dispatch(Action::PickerCancel);
        assert!(app.service_picker.is_none());
        assert_eq!(app.tabs.len(), old_tab_count);
    }

    #[test]
    fn picker_move_down_increments_index() {
        let mut app = app_with_ec2_tab();
        app.dispatch(Action::NewTab);
        assert_eq!(app.service_picker.as_ref().unwrap().selected_index, 0);
        app.dispatch(Action::PickerMoveDown);
        assert_eq!(app.service_picker.as_ref().unwrap().selected_index, 1);
    }

    #[test]
    fn picker_move_up_decrements_index() {
        let mut app = app_with_ec2_tab();
        app.dispatch(Action::NewTab);
        app.dispatch(Action::PickerMoveDown);
        app.dispatch(Action::PickerMoveDown);
        assert_eq!(app.service_picker.as_ref().unwrap().selected_index, 2);
        app.dispatch(Action::PickerMoveUp);
        assert_eq!(app.service_picker.as_ref().unwrap().selected_index, 1);
    }

    #[test]
    fn handle_enter_switches_to_detail_when_on_tab_list_with_data() {
        let mut app = app_with_ec2_tab();
        set_ec2_instances(
            &mut app,
            vec![create_test_instance("i-001", "web", InstanceState::Running)],
        );
        app.dispatch(Action::Enter);
        assert_eq!(app.active_tab().unwrap().tab_view, TabView::Detail);
    }

    #[test]
    fn handle_enter_stays_on_list_when_empty() {
        let mut app = app_with_ec2_tab();
        app.dispatch(Action::Enter);
        assert_eq!(app.active_tab().unwrap().tab_view, TabView::List);
    }

    // ──────────────────────────────────────────────
    // handle_back テスト
    // ──────────────────────────────────────────────

    #[test]
    fn handle_back_switches_to_list_when_on_tab_detail() {
        let mut app = app_with_ec2_tab();
        set_ec2_instances(
            &mut app,
            vec![create_test_instance("i-001", "web", InstanceState::Running)],
        );
        app.active_tab_mut().unwrap().tab_view = TabView::Detail;
        app.dispatch(Action::Back);
        assert_eq!(app.active_tab().unwrap().tab_view, TabView::List);
    }

    #[test]
    fn handle_back_does_nothing_when_on_dashboard() {
        let mut app = App::new("dev".to_string(), None);
        app.dispatch(Action::Back);
        assert!(app.show_dashboard);
        assert!(!app.should_quit);
    }

    #[test]
    fn handle_back_does_nothing_when_on_tab_list() {
        let mut app = app_with_ec2_tab();
        app.dispatch(Action::Back);
        assert_eq!(app.active_tab().unwrap().tab_view, TabView::List);
    }

    // ──────────────────────────────────────────────
    // show_message / dismiss_message テスト
    // ──────────────────────────────────────────────

    #[test]
    fn show_message_sets_message_when_called() {
        let mut app = App::new("dev".to_string(), None);
        app.show_message(MessageLevel::Error, "Error", "Something failed");
        assert!(app.message.is_some());
        let msg = app.message.as_ref().unwrap();
        assert_eq!(msg.level, MessageLevel::Error);
        assert_eq!(msg.title, "Error");
        assert_eq!(msg.body, "Something failed");
    }

    #[test]
    fn dismiss_message_clears_message_when_called() {
        let mut app = App::new("dev".to_string(), None);
        app.show_message(MessageLevel::Info, "Info", "test");
        app.dismiss_message();
        assert!(app.message.is_none());
    }

    // ──────────────────────────────────────────────
    // handle_event テスト
    // ──────────────────────────────────────────────

    #[test]
    fn handle_event_routes_instances_loaded_to_correct_tab() {
        let mut app = app_with_ec2_tab();
        let tab_id = app.active_tab().unwrap().id;
        let instances = vec![
            create_test_instance("i-001", "web", InstanceState::Running),
            create_test_instance("i-002", "api", InstanceState::Stopped),
        ];
        app.handle_event(AppEvent::TabEvent(
            tab_id,
            TabEvent::InstancesLoaded(Ok(instances)),
        ));
        let tab = app.active_tab().unwrap();
        assert!(!tab.loading);
        if let ServiceData::Ec2 {
            instances,
            filtered_instances,
        } = &tab.data
        {
            assert_eq!(instances.len(), 2);
            assert_eq!(filtered_instances.len(), 2);
        } else {
            panic!("Expected Ec2 ServiceData");
        }
    }

    #[test]
    fn handle_event_shows_error_when_instances_loaded_err() {
        let mut app = app_with_ec2_tab();
        let tab_id = app.active_tab().unwrap().id;
        app.handle_event(AppEvent::TabEvent(
            tab_id,
            TabEvent::InstancesLoaded(Err(AppError::AwsApi("access denied".to_string()))),
        ));
        let tab = app.active_tab().unwrap();
        assert!(!tab.loading);
        assert!(app.message.is_some());
        let msg = app.message.as_ref().unwrap();
        assert_eq!(msg.level, MessageLevel::Error);
    }

    #[test]
    fn handle_event_shows_info_when_instances_loaded_empty() {
        let mut app = app_with_ec2_tab();
        let tab_id = app.active_tab().unwrap().id;
        app.handle_event(AppEvent::TabEvent(
            tab_id,
            TabEvent::InstancesLoaded(Ok(vec![])),
        ));
        assert!(app.message.is_some());
        let msg = app.message.as_ref().unwrap();
        assert_eq!(msg.level, MessageLevel::Info);
        assert_eq!(msg.body, "No instances found");
    }

    #[test]
    fn handle_event_crud_completed_shows_success_and_sets_loading() {
        let mut app = app_with_ec2_tab();
        let tab_id = app.active_tab().unwrap().id;
        app.handle_event(AppEvent::CrudCompleted(
            tab_id,
            Ok("Bucket created".to_string()),
        ));
        assert!(app.message.is_some());
        let msg = app.message.as_ref().unwrap();
        assert_eq!(msg.level, MessageLevel::Success);
        assert_eq!(msg.body, "Bucket created");
        assert!(app.active_tab().unwrap().loading);
    }

    #[test]
    fn handle_event_crud_completed_shows_error_when_err() {
        let mut app = app_with_ec2_tab();
        let tab_id = app.active_tab().unwrap().id;
        app.handle_event(AppEvent::CrudCompleted(
            tab_id,
            Err(AppError::AwsApi("access denied".to_string())),
        ));
        assert!(app.message.is_some());
        let msg = app.message.as_ref().unwrap();
        assert_eq!(msg.level, MessageLevel::Error);
    }

    // ──────────────────────────────────────────────
    // StartStop / Reboot テスト
    // ──────────────────────────────────────────────

    #[test]
    fn start_stop_sets_confirm_stop_when_instance_running() {
        let mut app = app_with_ec2_tab();
        set_ec2_instances(
            &mut app,
            vec![create_test_instance("i-001", "web", InstanceState::Running)],
        );
        app.dispatch(Action::StartStop);
        assert_eq!(
            app.active_tab().unwrap().mode,
            Mode::Confirm(ConfirmAction::Stop("i-001".to_string()))
        );
    }

    #[test]
    fn start_stop_sets_confirm_start_when_instance_stopped() {
        let mut app = app_with_ec2_tab();
        set_ec2_instances(
            &mut app,
            vec![create_test_instance("i-001", "web", InstanceState::Stopped)],
        );
        app.dispatch(Action::StartStop);
        assert_eq!(
            app.active_tab().unwrap().mode,
            Mode::Confirm(ConfirmAction::Start("i-001".to_string()))
        );
    }

    #[test]
    fn reboot_sets_confirm_reboot_when_instance_exists() {
        let mut app = app_with_ec2_tab();
        set_ec2_instances(
            &mut app,
            vec![create_test_instance("i-001", "web", InstanceState::Running)],
        );
        app.dispatch(Action::Reboot);
        assert_eq!(
            app.active_tab().unwrap().mode,
            Mode::Confirm(ConfirmAction::Reboot("i-001".to_string()))
        );
    }

    // ──────────────────────────────────────────────
    // ConfirmYes テスト
    // ──────────────────────────────────────────────

    #[test]
    fn confirm_yes_returns_confirm_action_when_in_confirm_mode() {
        let mut app = app_with_ec2_tab();
        app.active_tab_mut().unwrap().mode =
            Mode::Confirm(ConfirmAction::Stop("i-001".to_string()));
        let result = app.dispatch(Action::ConfirmYes);
        assert_eq!(
            result,
            SideEffect::Confirm(ConfirmAction::Stop("i-001".to_string()))
        );
        assert_eq!(app.active_tab().unwrap().mode, Mode::Normal);
    }

    #[test]
    fn confirm_no_sets_normal_mode_when_in_confirm() {
        let mut app = app_with_ec2_tab();
        app.active_tab_mut().unwrap().mode =
            Mode::Confirm(ConfirmAction::Stop("i-001".to_string()));
        let result = app.dispatch(Action::ConfirmNo);
        assert_eq!(result, SideEffect::None);
        assert_eq!(app.active_tab().unwrap().mode, Mode::Normal);
    }

    // ──────────────────────────────────────────────
    // Create / Delete / Edit テスト
    // ──────────────────────────────────────────────

    #[test]
    fn handle_create_sets_form_mode_when_s3_list() {
        let mut app = App::new("dev".to_string(), None);
        app.create_tab(ServiceKind::S3);
        app.active_tab_mut().unwrap().loading = false;
        app.dispatch(Action::Create);
        assert!(matches!(
            app.active_tab().unwrap().mode,
            Mode::Form(FormContext {
                kind: FormKind::CreateS3Bucket,
                ..
            })
        ));
    }

    #[test]
    fn handle_create_sets_form_mode_when_secrets_list() {
        let mut app = App::new("dev".to_string(), None);
        app.create_tab(ServiceKind::SecretsManager);
        app.active_tab_mut().unwrap().loading = false;
        app.dispatch(Action::Create);
        if let Mode::Form(ctx) = &app.active_tab().unwrap().mode {
            assert_eq!(ctx.kind, FormKind::CreateSecret);
            assert_eq!(ctx.fields.len(), 3);
        } else {
            panic!("Expected Form mode");
        }
    }

    #[test]
    fn handle_create_does_nothing_when_ec2_list() {
        let mut app = app_with_ec2_tab();
        app.dispatch(Action::Create);
        assert_eq!(app.active_tab().unwrap().mode, Mode::Normal);
    }

    #[test]
    fn handle_delete_shows_permission_denied_when_no_permission() {
        let mut app = app_with_ec2_tab();
        set_ec2_instances(
            &mut app,
            vec![create_test_instance("i-001", "web", InstanceState::Running)],
        );
        app.dispatch(Action::Delete);
        assert!(app.message.is_some());
        let msg = app.message.as_ref().unwrap();
        assert_eq!(msg.level, MessageLevel::Error);
        assert_eq!(msg.title, "Permission Denied");
    }

    #[test]
    fn handle_delete_sets_danger_confirm_when_ec2_with_permission() {
        let mut app = App::with_delete_permissions("dev".to_string(), None, DeletePermissions::All);
        app.create_tab(ServiceKind::Ec2);
        set_ec2_instances(
            &mut app,
            vec![create_test_instance("i-001", "web", InstanceState::Running)],
        );
        app.dispatch(Action::Delete);
        if let Mode::DangerConfirm(ctx) = &app.active_tab().unwrap().mode {
            assert_eq!(ctx.action, DangerAction::TerminateEc2("i-001".to_string()));
        } else {
            panic!("Expected DangerConfirm mode");
        }
    }

    #[test]
    fn handle_edit_sets_form_mode_when_secrets_detail_with_detail() {
        let mut app = App::new("dev".to_string(), None);
        app.create_tab(ServiceKind::SecretsManager);
        let tab = app.active_tab_mut().unwrap();
        tab.tab_view = TabView::Detail;
        tab.loading = false;
        if let ServiceData::Secrets { detail, .. } = &mut tab.data {
            *detail = Some(Box::new(crate::aws::secrets_model::SecretDetail {
                name: "my-secret".to_string(),
                arn: "arn:test".to_string(),
                description: None,
                kms_key_id: None,
                rotation_enabled: false,
                rotation_lambda_arn: None,
                rotation_days: None,
                last_rotated_date: None,
                last_changed_date: None,
                last_accessed_date: None,
                created_date: None,
                tags: HashMap::new(),
                version_ids: Vec::new(),
                version_stages: Vec::new(),
                secret_value: None,
            }));
        }
        app.dispatch(Action::Edit);
        if let Mode::Form(ctx) = &app.active_tab().unwrap().mode {
            assert_eq!(ctx.kind, FormKind::UpdateSecretValue);
            assert_eq!(ctx.fields.len(), 1);
        } else {
            panic!("Expected Form mode");
        }
    }

    // ──────────────────────────────────────────────
    // FormSubmit テスト
    // ──────────────────────────────────────────────

    #[test]
    fn form_submit_shows_error_when_required_field_empty() {
        let mut app = app_with_ec2_tab();
        app.active_tab_mut().unwrap().mode = Mode::Form(FormContext {
            kind: FormKind::CreateS3Bucket,
            fields: vec![FormField {
                label: "Bucket Name".to_string(),
                input: Input::default(),
                required: true,
            }],
            focused_field: 0,
        });
        app.dispatch(Action::FormSubmit);
        assert!(app.message.is_some());
        let msg = app.message.as_ref().unwrap();
        assert_eq!(msg.level, MessageLevel::Error);
    }

    #[test]
    fn form_submit_returns_form_submit_side_effect_when_valid() {
        let mut app = app_with_ec2_tab();
        let mut input = Input::default();
        input.handle(tui_input::InputRequest::InsertChar('t'));
        input.handle(tui_input::InputRequest::InsertChar('e'));
        input.handle(tui_input::InputRequest::InsertChar('s'));
        input.handle(tui_input::InputRequest::InsertChar('t'));
        app.active_tab_mut().unwrap().mode = Mode::Form(FormContext {
            kind: FormKind::CreateS3Bucket,
            fields: vec![FormField {
                label: "Bucket Name".to_string(),
                input,
                required: true,
            }],
            focused_field: 0,
        });
        let result = app.dispatch(Action::FormSubmit);
        assert_eq!(app.active_tab().unwrap().mode, Mode::Normal);
        assert!(matches!(result, SideEffect::FormSubmit(_)));
        assert!(app.active_tab().unwrap().loading);
    }

    // ──────────────────────────────────────────────
    // DangerConfirm テスト
    // ──────────────────────────────────────────────

    #[test]
    fn danger_confirm_submit_does_nothing_when_text_mismatch() {
        let mut app = app_with_ec2_tab();
        app.active_tab_mut().unwrap().mode = Mode::DangerConfirm(DangerConfirmContext {
            action: DangerAction::TerminateEc2("i-001".to_string()),
            input: Input::default(),
        });
        app.dispatch(Action::DangerConfirmSubmit);
        assert!(matches!(
            app.active_tab().unwrap().mode,
            Mode::DangerConfirm(_)
        ));
    }

    #[test]
    fn danger_confirm_submit_returns_danger_action_when_text_matches() {
        let mut app = app_with_ec2_tab();
        let mut input = Input::default();
        for c in "i-001".chars() {
            input.handle(tui_input::InputRequest::InsertChar(c));
        }
        app.active_tab_mut().unwrap().mode = Mode::DangerConfirm(DangerConfirmContext {
            action: DangerAction::TerminateEc2("i-001".to_string()),
            input,
        });
        let result = app.dispatch(Action::DangerConfirmSubmit);
        assert_eq!(app.active_tab().unwrap().mode, Mode::Normal);
        assert_eq!(
            result,
            SideEffect::DangerAction(DangerAction::TerminateEc2("i-001".to_string()))
        );
    }

    // ──────────────────────────────────────────────
    // apply_filter テスト
    // ──────────────────────────────────────────────

    #[test]
    fn apply_filter_filters_tab_data_when_on_tab() {
        let mut app = app_with_ec2_tab();
        let tab = app.active_tab_mut().unwrap();
        if let ServiceData::Ec2 {
            instances,
            filtered_instances,
        } = &mut tab.data
        {
            *instances = vec![
                create_test_instance("i-001", "web", InstanceState::Running),
                create_test_instance("i-002", "api", InstanceState::Stopped),
            ];
            *filtered_instances = instances.clone();
        }
        tab.filter_input = Input::from("web");
        tab.loading = false;
        app.apply_filter();
        let tab = app.active_tab().unwrap();
        if let ServiceData::Ec2 {
            filtered_instances, ..
        } = &tab.data
        {
            assert_eq!(filtered_instances.len(), 1);
            assert_eq!(filtered_instances[0].name, "web");
        } else {
            panic!("Expected Ec2 data");
        }
    }

    #[test]
    fn apply_filter_filters_dashboard_services_when_on_dashboard() {
        let mut app = App::new("dev".to_string(), None);
        app.dashboard.filter_input = Input::from("EC2");
        app.apply_filter();
        assert!(!app.dashboard.filtered_services.is_empty());
        // EC2 should be in the results
        assert!(app.dashboard.filtered_services.contains(&ServiceKind::Ec2));
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
    // switch_tab_next / switch_tab_prev テスト
    // ──────────────────────────────────────────────

    #[test]
    fn switch_tab_next_cycles_through_tabs() {
        let mut app = App::new("dev".to_string(), None);
        app.create_tab(ServiceKind::Ec2);
        app.create_tab(ServiceKind::S3);
        assert_eq!(app.active_tab_index, 1); // last created
        app.switch_tab_next();
        assert_eq!(app.active_tab_index, 0); // wraps around
        app.switch_tab_next();
        assert_eq!(app.active_tab_index, 1);
    }

    #[test]
    fn switch_tab_prev_cycles_through_tabs() {
        let mut app = App::new("dev".to_string(), None);
        app.create_tab(ServiceKind::Ec2);
        app.create_tab(ServiceKind::S3);
        assert_eq!(app.active_tab_index, 1);
        app.switch_tab_prev();
        assert_eq!(app.active_tab_index, 0);
        app.switch_tab_prev();
        assert_eq!(app.active_tab_index, 1); // wraps around
    }

    // ──────────────────────────────────────────────
    // close_tab テスト
    // ──────────────────────────────────────────────

    #[test]
    fn close_tab_removes_tab_and_shows_dashboard_when_last() {
        let mut app = App::new("dev".to_string(), None);
        app.create_tab(ServiceKind::Ec2);
        assert!(!app.show_dashboard);
        app.close_tab();
        assert!(app.tabs.is_empty());
        assert!(app.show_dashboard);
    }

    #[test]
    fn close_tab_adjusts_index_when_not_last() {
        let mut app = App::new("dev".to_string(), None);
        app.create_tab(ServiceKind::Ec2);
        app.create_tab(ServiceKind::S3);
        assert_eq!(app.active_tab_index, 1);
        app.close_tab();
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.active_tab_index, 0);
        assert_eq!(app.active_tab().unwrap().service, ServiceKind::Ec2);
    }

    // ──────────────────────────────────────────────
    // current_view テスト
    // ──────────────────────────────────────────────

    #[test]
    fn current_view_returns_ec2_list_when_ec2_tab_in_list_view() {
        let app = app_with_ec2_tab();
        assert_eq!(app.current_view(), Some((ServiceKind::Ec2, TabView::List)));
    }

    #[test]
    fn current_view_returns_ec2_detail_when_ec2_tab_in_detail_view() {
        let mut app = app_with_ec2_tab();
        app.active_tab_mut().unwrap().tab_view = TabView::Detail;
        assert_eq!(
            app.current_view(),
            Some((ServiceKind::Ec2, TabView::Detail))
        );
    }

    #[test]
    fn current_view_returns_none_when_no_tabs() {
        let app = App::new("dev".to_string(), None);
        assert_eq!(app.current_view(), None);
    }

    // ──────────────────────────────────────────────
    // SwitchDetailTab テスト
    // ──────────────────────────────────────────────

    #[test]
    fn switch_detail_tab_toggles_ec2_detail_tab() {
        let mut app = app_with_ec2_tab();
        app.active_tab_mut().unwrap().tab_view = TabView::Detail;
        assert_eq!(app.active_tab().unwrap().detail_tab, DetailTab::Overview);
        app.dispatch(Action::SwitchDetailTab);
        assert_eq!(app.active_tab().unwrap().detail_tab, DetailTab::Tags);
        app.dispatch(Action::SwitchDetailTab);
        assert_eq!(app.active_tab().unwrap().detail_tab, DetailTab::Overview);
    }

    // ──────────────────────────────────────────────
    // ShowHelp テスト
    // ──────────────────────────────────────────────

    #[test]
    fn show_help_sets_flag_and_back_dismisses() {
        let mut app = App::new("dev".to_string(), None);
        app.dispatch(Action::ShowHelp);
        assert!(app.show_help);
        app.dispatch(Action::Back);
        assert!(!app.show_help);
    }

    // ──────────────────────────────────────────────
    // Message overlay テスト
    // ──────────────────────────────────────────────

    #[test]
    fn dispatch_dismiss_message_clears_message_overlay() {
        let mut app = App::new("dev".to_string(), None);
        app.show_message(MessageLevel::Info, "Info", "test");
        app.dispatch(Action::DismissMessage);
        assert!(app.message.is_none());
    }

    #[test]
    fn dispatch_back_clears_message_overlay() {
        let mut app = App::new("dev".to_string(), None);
        app.show_message(MessageLevel::Info, "Info", "test");
        app.dispatch(Action::Back);
        assert!(app.message.is_none());
    }

    // ──────────────────────────────────────────────
    // half_page テスト
    // ──────────────────────────────────────────────

    #[test]
    fn half_page_up_moves_10_when_on_tab_list() {
        let mut app = app_with_ec2_tab();
        let instances: Vec<Instance> = (0..20)
            .map(|i| create_test_instance(&format!("i-{i:03}"), "inst", InstanceState::Running))
            .collect();
        set_ec2_instances(&mut app, instances);
        app.active_tab_mut().unwrap().selected_index = 15;
        app.dispatch(Action::HalfPageUp);
        assert_eq!(app.active_tab().unwrap().selected_index, 5);
    }

    #[test]
    fn half_page_down_moves_10_when_on_tab_list() {
        let mut app = app_with_ec2_tab();
        let instances: Vec<Instance> = (0..20)
            .map(|i| create_test_instance(&format!("i-{i:03}"), "inst", InstanceState::Running))
            .collect();
        set_ec2_instances(&mut app, instances);
        app.active_tab_mut().unwrap().selected_index = 5;
        app.dispatch(Action::HalfPageDown);
        assert_eq!(app.active_tab().unwrap().selected_index, 15);
    }

    // ──────────────────────────────────────────────
    // Noop テスト
    // ──────────────────────────────────────────────

    #[test]
    fn dispatch_returns_side_effect_none_when_noop() {
        let mut app = App::new("dev".to_string(), None);
        let result = app.dispatch(Action::Noop);
        assert_eq!(result, SideEffect::None);
    }

    // ──────────────────────────────────────────────
    // Filter mode テスト
    // ──────────────────────────────────────────────

    #[test]
    fn start_filter_sets_filter_mode_on_tab() {
        let mut app = app_with_ec2_tab();
        app.dispatch(Action::StartFilter);
        assert_eq!(app.active_tab().unwrap().mode, Mode::Filter);
    }

    #[test]
    fn confirm_filter_sets_normal_mode_on_tab() {
        let mut app = app_with_ec2_tab();
        app.active_tab_mut().unwrap().mode = Mode::Filter;
        app.dispatch(Action::ConfirmFilter);
        assert_eq!(app.active_tab().unwrap().mode, Mode::Normal);
    }

    #[test]
    fn cancel_filter_resets_filter_and_sets_normal_mode() {
        let mut app = app_with_ec2_tab();
        let tab = app.active_tab_mut().unwrap();
        tab.mode = Mode::Filter;
        tab.filter_input = Input::from("web");
        if let ServiceData::Ec2 {
            instances,
            filtered_instances,
        } = &mut tab.data
        {
            *instances = vec![create_test_instance("i-001", "web", InstanceState::Running)];
            *filtered_instances = instances.clone();
        }
        app.dispatch(Action::CancelFilter);
        let tab = app.active_tab().unwrap();
        assert_eq!(tab.mode, Mode::Normal);
        assert!(tab.filter_input.value().is_empty());
    }

    // ──────────────────────────────────────────────
    // FormNextField テスト
    // ──────────────────────────────────────────────

    #[test]
    fn form_next_field_advances_when_multiple_fields() {
        let mut app = app_with_ec2_tab();
        app.active_tab_mut().unwrap().mode = Mode::Form(FormContext {
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
        if let Mode::Form(ctx) = &app.active_tab().unwrap().mode {
            assert_eq!(ctx.focused_field, 1);
        } else {
            panic!("Expected Form mode");
        }
    }

    #[test]
    fn form_next_field_wraps_around_when_at_last() {
        let mut app = app_with_ec2_tab();
        app.active_tab_mut().unwrap().mode = Mode::Form(FormContext {
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
        if let Mode::Form(ctx) = &app.active_tab().unwrap().mode {
            assert_eq!(ctx.focused_field, 0);
        } else {
            panic!("Expected Form mode");
        }
    }

    // ──────────────────────────────────────────────
    // Refresh テスト
    // ──────────────────────────────────────────────

    #[test]
    fn refresh_sets_loading_on_active_tab() {
        let mut app = app_with_ec2_tab();
        app.active_tab_mut().unwrap().loading = false;
        app.dispatch(Action::Refresh);
        assert!(app.active_tab().unwrap().loading);
    }

    // ──────────────────────────────────────────────
    // DangerAction テスト
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
        assert_eq!(action.message(), "Type 'i-001' to terminate this instance:");
    }

    #[test]
    fn danger_action_message_returns_delete_msg_when_s3_bucket() {
        let action = DangerAction::DeleteS3Bucket("my-bucket".to_string());
        assert_eq!(action.message(), "Type 'my-bucket' to delete this bucket:");
    }

    // ──────────────────────────────────────────────
    // FormContext テスト
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
    // Dashboard filter mode テスト
    // ──────────────────────────────────────────────

    #[test]
    fn start_filter_sets_dashboard_filter_mode_when_on_dashboard() {
        let mut app = App::new("dev".to_string(), None);
        app.dispatch(Action::StartFilter);
        assert_eq!(app.dashboard.mode, Mode::Filter);
    }

    #[test]
    fn cancel_filter_resets_dashboard_filter_when_on_dashboard() {
        let mut app = App::new("dev".to_string(), None);
        app.dashboard.mode = Mode::Filter;
        app.dashboard.filter_input = Input::from("ec");
        app.dispatch(Action::CancelFilter);
        assert_eq!(app.dashboard.mode, Mode::Normal);
        assert!(app.dashboard.filter_input.value().is_empty());
    }

    // ──────────────────────────────────────────────
    // FormCancel テスト
    // ──────────────────────────────────────────────

    #[test]
    fn form_cancel_sets_normal_mode_when_in_form() {
        let mut app = app_with_ec2_tab();
        app.active_tab_mut().unwrap().mode = Mode::Form(FormContext {
            kind: FormKind::CreateS3Bucket,
            fields: vec![FormField {
                label: "Bucket Name".to_string(),
                input: Input::default(),
                required: true,
            }],
            focused_field: 0,
        });
        app.dispatch(Action::FormCancel);
        assert_eq!(app.active_tab().unwrap().mode, Mode::Normal);
    }

    // ──────────────────────────────────────────────
    // DangerConfirmCancel テスト
    // ──────────────────────────────────────────────

    #[test]
    fn danger_confirm_cancel_sets_normal_mode() {
        let mut app = app_with_ec2_tab();
        app.active_tab_mut().unwrap().mode = Mode::DangerConfirm(DangerConfirmContext {
            action: DangerAction::TerminateEc2("i-001".to_string()),
            input: Input::default(),
        });
        app.dispatch(Action::DangerConfirmCancel);
        assert_eq!(app.active_tab().unwrap().mode, Mode::Normal);
    }
}
