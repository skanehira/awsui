mod crud;

#[cfg(test)]
mod tests;

use tokio::sync::mpsc;
use tui_input::Input;

use crate::action::Action;
use crate::aws::model::{Instance, InstanceState};
use crate::cli::DeletePermissions;
use crate::config::SsoProfile;
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

    // プロファイル選択画面
    pub profile_selector: Option<ProfileSelectorState>,

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
            profile_selector: None,
            service_picker: None,
            delete_permissions,
            pending_log_configs: None,
            event_tx,
            event_rx,
        }
    }

    /// プロファイル選択画面から開始する初期化
    pub fn new_with_profile_selector(
        profiles: Vec<SsoProfile>,
        delete_permissions: DeletePermissions,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::channel(32);
        Self {
            should_quit: false,
            message: None,
            show_help: false,
            profile: None,
            region: None,
            tabs: Vec::new(),
            active_tab_index: 0,
            next_tab_id: 0,
            show_dashboard: false,
            dashboard: DashboardState::new(),
            profile_selector: Some(ProfileSelectorState::new(profiles)),
            service_picker: None,
            delete_permissions,
            pending_log_configs: None,
            event_tx,
            event_rx,
        }
    }

    /// プロファイル選択完了後の状態遷移
    pub fn complete_profile_selection(&mut self, profile: String, region: Option<String>) {
        self.profile = Some(profile);
        self.region = region;
        self.profile_selector = None;
        self.show_dashboard = true;
    }

    /// プロファイル選択画面のアクション処理
    fn dispatch_profile_selector(&mut self, action: Action) -> SideEffect {
        let is_filter = self
            .profile_selector
            .as_ref()
            .is_some_and(|ps| ps.mode == Mode::Filter);

        if is_filter {
            if let Some(ps) = &mut self.profile_selector {
                match action {
                    Action::ConfirmFilter => ps.mode = Mode::Normal,
                    Action::CancelFilter => {
                        ps.clear_filter();
                        ps.mode = Mode::Normal;
                    }
                    Action::FilterHandleInput(req) => {
                        ps.filter_input.handle(req);
                        ps.apply_filter();
                    }
                    _ => {}
                }
            }
            return SideEffect::None;
        }

        match action {
            Action::Quit => self.should_quit = true,
            Action::MoveUp => {
                if let Some(ps) = &mut self.profile_selector {
                    ps.move_up();
                }
            }
            Action::MoveDown => {
                if let Some(ps) = &mut self.profile_selector {
                    ps.move_down();
                }
            }
            Action::MoveToTop => {
                if let Some(ps) = &mut self.profile_selector {
                    ps.move_to_top();
                }
            }
            Action::MoveToBottom => {
                if let Some(ps) = &mut self.profile_selector {
                    ps.move_to_bottom();
                }
            }
            Action::StartFilter => {
                if let Some(ps) = &mut self.profile_selector {
                    ps.mode = Mode::Filter;
                }
            }
            Action::Enter => {
                if let Some(ps) = &self.profile_selector
                    && !ps.logging_in
                    && let Some(profile) = ps.selected_profile().cloned()
                {
                    let profile_name = profile.name.clone();
                    let region = profile.region.clone();

                    match crate::sso::check_sso_token(&profile) {
                        crate::sso::SsoTokenStatus::Valid => {
                            self.complete_profile_selection(profile_name, region);
                        }
                        crate::sso::SsoTokenStatus::Expired
                        | crate::sso::SsoTokenStatus::NotFound => {
                            if let Some(ps) = &mut self.profile_selector {
                                ps.logging_in = true;
                                ps.login_output.clear();
                            }
                            return SideEffect::StartSsoLogin {
                                profile_name,
                                region,
                            };
                        }
                    }
                }
            }
            Action::CancelSsoLogin => {
                if let Some(ps) = &mut self.profile_selector {
                    ps.logging_in = false;
                    ps.login_output.clear();
                }
            }
            _ => {}
        }
        SideEffect::None
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
        if let crate::tab::ServiceData::Ec2 { instances, .. } = &tab.data {
            instances.filtered.get(tab.selected_index)
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

        // プロファイル選択画面の処理
        if self.profile_selector.is_some() {
            return self.dispatch_profile_selector(action);
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
                Mode::ContainerSelect(state) => match action {
                    Action::ContainerSelectConfirm => {
                        let name = state.selected_name().map(|s| s.to_string());
                        let purpose = state.purpose.clone();
                        if let Some(tab) = self.active_tab_mut() {
                            tab.mode = Mode::Normal;
                        }
                        if let Some(container_name) = name {
                            match purpose {
                                ContainerSelectPurpose::EcsExec => {
                                    return self.handle_container_select_ecs_exec(&container_name);
                                }
                                ContainerSelectPurpose::ShowLogs => {
                                    self.handle_container_select_show_logs(container_name);
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
                            && let Mode::ContainerSelect(state) = &mut tab.mode
                        {
                            state.move_up();
                        }
                        return SideEffect::None;
                    }
                    Action::ContainerSelectDown => {
                        if let Some(tab) = self.active_tab_mut()
                            && let Mode::ContainerSelect(state) = &mut tab.mode
                        {
                            state.move_down();
                        }
                        return SideEffect::None;
                    }
                    Action::ContainerSelectHandleInput(req) => {
                        if let Some(tab) = self.active_tab_mut()
                            && let Mode::ContainerSelect(state) = &mut tab.mode
                        {
                            state.filter_input.handle(req);
                            state.apply_filter();
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
            Action::LogScrollLeft => self.log_scroll_left(),
            Action::LogScrollRight => self.log_scroll_right(),
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
            | Action::ContainerSelectCancel
            | Action::ContainerSelectHandleInput(_)
            | Action::CancelSsoLogin => {}
            Action::SsmConnect => return self.handle_ssm_connect(),
            Action::EcsExec => return self.handle_ecs_exec(),
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
            AppEvent::SsoLoginOutput(line) => {
                if let Some(ps) = &mut self.profile_selector {
                    ps.login_output.push(line);
                }
            }
            AppEvent::SsoLoginCompleted(result) => match result {
                Ok((profile_name, region)) => {
                    self.complete_profile_selection(profile_name, region);
                }
                Err(e) => {
                    if let Some(ps) = &mut self.profile_selector {
                        ps.logging_in = false;
                        ps.login_output.clear();
                    }
                    self.show_message(MessageLevel::Error, "SSO Login Failed", e.to_string());
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
                    |tab, data| {
                        let is_empty = data.is_empty();
                        if let ServiceData::Ec2 { instances } = &mut tab.data {
                            instances.set_items(data);
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
                        if let ServiceData::Ecr { repositories, .. } = &mut tab.data {
                            repositories.set_items(repos);
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
                    |tab, data| {
                        let is_empty = data.is_empty();
                        if let ServiceData::Ecs { clusters, .. } = &mut tab.data {
                            clusters.set_items(data);
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
                    |tab, data| {
                        let is_empty = data.is_empty();
                        if let ServiceData::S3 { buckets, .. } = &mut tab.data {
                            buckets.set_items(data);
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
                    |tab, data| {
                        let is_empty = data.is_empty();
                        if let ServiceData::Vpc { vpcs, .. } = &mut tab.data {
                            vpcs.set_items(data);
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
                    |tab, data| {
                        let is_empty = data.is_empty();
                        if let ServiceData::Secrets { secrets, .. } = &mut tab.data {
                            secrets.set_items(data);
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
                    Ok((vpc_data, subnet_data)) => {
                        if let Some(tab) = self.find_tab_mut(tab_id) {
                            if let ServiceData::Vpc { vpcs, subnets } = &mut tab.data {
                                vpcs.set_items(vpc_data);
                                *subnets = subnet_data;
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
                        let stream_prefix = config.stream_prefix.as_deref().unwrap_or_default();
                        self.transition_to_log_view(
                            tab_id,
                            config.container_name.clone(),
                            log_group,
                            stream_prefix,
                        );
                    } else {
                        // 複数コンテナ → 選択ダイアログ
                        let names: Vec<String> =
                            configs.iter().map(|c| c.container_name.clone()).collect();
                        if let Some(tab) = self.find_tab_mut(tab_id) {
                            tab.mode = Mode::ContainerSelect(ContainerSelectState::new(
                                names,
                                ContainerSelectPurpose::ShowLogs,
                            ));
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
                        if let crate::tab::ServiceData::Ecs {
                            nav_level: Some(nav),
                            ..
                        } = &mut tab.data
                            && let Some(state) = nav.log_state_mut()
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

    fn log_scroll_left(&mut self) {
        if let Some(state) = self.active_log_state_mut() {
            state.scroll_left();
        }
    }

    fn log_scroll_right(&mut self) {
        if let Some(state) = self.active_log_state_mut() {
            state.scroll_right();
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
            self.dashboard.move_up();
            return;
        }
        if let Some(tab) = self.active_tab_mut() {
            tab.move_up();
        }
    }

    fn move_down(&mut self) {
        if self.show_dashboard {
            self.dashboard.move_down();
            return;
        }
        if let Some(tab) = self.active_tab_mut() {
            tab.move_down();
        }
    }

    fn move_to_top(&mut self) {
        if self.show_dashboard {
            self.dashboard.move_to_top();
            return;
        }
        if let Some(tab) = self.active_tab_mut() {
            tab.move_to_top();
        }
    }

    fn move_to_bottom(&mut self) {
        if self.show_dashboard {
            self.dashboard.move_to_bottom();
            return;
        }
        if let Some(tab) = self.active_tab_mut() {
            tab.move_to_bottom();
        }
    }

    fn half_page_up(&mut self) {
        if self.show_dashboard {
            self.dashboard.half_page_up();
            return;
        }
        if let Some(tab) = self.active_tab_mut() {
            tab.half_page_up();
        }
    }

    fn half_page_down(&mut self) {
        if self.show_dashboard {
            self.dashboard.half_page_down();
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

    fn handle_ssm_connect(&mut self) -> SideEffect {
        let instance_data = self
            .selected_instance()
            .map(|i| (i.instance_id.clone(), i.state.clone()));
        let Some((id, state)) = instance_data else {
            return SideEffect::None;
        };
        if state != InstanceState::Running {
            self.show_message(
                MessageLevel::Error,
                "SSM Connect",
                "Instance must be in Running state to connect via SSM",
            );
            return SideEffect::None;
        }
        SideEffect::SsmConnect { instance_id: id }
    }

    fn handle_container_select_ecs_exec(&mut self, container_name: &str) -> SideEffect {
        let Some(tab) = self.active_tab() else {
            return SideEffect::None;
        };
        let crate::tab::ServiceData::Ecs {
            tasks, nav_level, ..
        } = &tab.data
        else {
            return SideEffect::None;
        };
        let task_index = nav_level.as_ref().and_then(|nl| nl.task_index());
        let Some(task) = task_index.and_then(|idx| tasks.get(idx)) else {
            return SideEffect::None;
        };
        SideEffect::EcsExec {
            cluster_arn: task.cluster_arn.clone(),
            task_arn: task.task_arn.clone(),
            container_name: container_name.to_string(),
        }
    }

    /// ECSタスクから短縮タスクIDを抽出する
    fn extract_ecs_task_id(&self, tab_id: crate::tab::TabId) -> Option<String> {
        let tab = self.find_tab(tab_id)?;
        if let crate::tab::ServiceData::Ecs {
            tasks, nav_level, ..
        } = &tab.data
        {
            nav_level
                .as_ref()
                .and_then(|nl| nl.task_index())
                .and_then(|idx| tasks.get(idx))
                .map(|t| {
                    t.task_arn
                        .rsplit('/')
                        .next()
                        .unwrap_or(&t.task_arn)
                        .to_string()
                })
        } else {
            None
        }
    }

    /// ECSログビューへ遷移する
    fn transition_to_log_view(
        &mut self,
        tab_id: crate::tab::TabId,
        container_name: String,
        log_group: String,
        stream_prefix: &str,
    ) {
        let Some(task_id) = self.extract_ecs_task_id(tab_id) else {
            return;
        };
        let log_stream = format!("{}/{}/{}", stream_prefix, container_name, task_id);
        if let Some(tab) = self.find_tab_mut(tab_id)
            && let crate::tab::ServiceData::Ecs { nav_level, .. } = &mut tab.data
            && let Some(nl) = nav_level
        {
            let service_index = nl.service_index().unwrap_or(0);
            let task_index = nl.task_index().unwrap_or(0);
            *nav_level = Some(crate::tab::EcsNavLevel::LogView {
                service_index,
                task_index,
                log_state: Box::new(crate::tab::LogViewState {
                    container_name,
                    log_group,
                    log_stream,
                    events: Vec::new(),
                    next_forward_token: None,
                    auto_scroll: true,
                    scroll_offset: 0,
                    scroll_x: 0,
                    search_query: String::new(),
                    search_matches: Vec::new(),
                    current_match_index: None,
                }),
            });
            tab.loading = true;
        }
    }

    fn handle_container_select_show_logs(&mut self, container_name: String) {
        // pending_log_configsから選択されたコンテナのconfigを見つけてlog_stateを作成
        if let Some((_config_tab_id, configs)) = self.pending_log_configs.take()
            && let Some(config) = configs.iter().find(|c| c.container_name == container_name)
        {
            let log_group = config.log_group.clone().unwrap_or_default();
            let stream_prefix = config.stream_prefix.as_deref().unwrap_or_default();
            if let Some(tab_id) = self.active_tab().map(|t| t.id) {
                self.transition_to_log_view(tab_id, container_name, log_group, stream_prefix);
            }
        }
    }

    fn handle_ecs_exec(&mut self) -> SideEffect {
        let Some(tab) = self.active_tab() else {
            return SideEffect::None;
        };
        let crate::tab::ServiceData::Ecs {
            services,
            tasks,
            nav_level,
            ..
        } = &tab.data
        else {
            return SideEffect::None;
        };

        // ServiceDetail（タスク一覧）またはTaskDetail（タスク詳細）で動作
        let (service_index, task_index) = match nav_level {
            Some(crate::tab::EcsNavLevel::TaskDetail {
                service_index,
                task_index,
            }) => (*service_index, *task_index),
            Some(crate::tab::EcsNavLevel::ServiceDetail { service_index }) => {
                (*service_index, tab.detail_tag_index)
            }
            _ => return SideEffect::None,
        };

        // enable_execute_command チェック
        let Some(service) = services.get(service_index) else {
            return SideEffect::None;
        };
        if !service.enable_execute_command {
            self.show_message(
                MessageLevel::Error,
                "ECS Exec",
                "ExecuteCommand is not enabled on this service",
            );
            return SideEffect::None;
        }

        // タスク取得
        let Some(task) = tasks.get(task_index) else {
            return SideEffect::None;
        };

        // RUNNINGコンテナのフィルタリング
        let running_containers: Vec<&crate::aws::ecs_model::Container> = task
            .containers
            .iter()
            .filter(|c| c.last_status == "RUNNING")
            .collect();

        if running_containers.is_empty() {
            self.show_message(
                MessageLevel::Error,
                "ECS Exec",
                "No running containers found in this task",
            );
            return SideEffect::None;
        }

        let cluster_arn = task.cluster_arn.clone();
        let task_arn = task.task_arn.clone();

        if running_containers.len() == 1 {
            // コンテナが1つ → 直接実行
            return SideEffect::EcsExec {
                cluster_arn,
                task_arn,
                container_name: running_containers[0].name.clone(),
            };
        }

        // 複数コンテナ → 選択ダイアログ
        let names: Vec<String> = running_containers.iter().map(|c| c.name.clone()).collect();
        if let Some(tab) = self.active_tab_mut() {
            tab.mode = Mode::ContainerSelect(ContainerSelectState::new(
                names,
                ContainerSelectPurpose::EcsExec,
            ));
        }
        SideEffect::None
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

    /// EC2 Detail Overviewタブでリンクをフォローする（未実装、将来対応）
    fn handle_follow_link(&mut self) {
        // no-op for now
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
                if let crate::tab::ServiceData::Vpc { vpcs, .. } = &tab.data {
                    vpcs.filtered
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
