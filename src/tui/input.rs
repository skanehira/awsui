use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use tui_input::backend::crossterm::to_input_request;

use crate::action::Action;
use crate::app::{App, Mode, View};

/// キーイベントをActionに変換する。
/// Appの現在のmode/viewに応じて適切なActionを返す。
pub fn handle_key(app: &App, key: KeyEvent) -> Action {
    // グローバルオーバーレイが優先
    if app.message.is_some() {
        return handle_message_key(key);
    }
    if app.show_help {
        return handle_help_key(key);
    }

    // サービスピッカー表示中
    if app.service_picker.is_some() {
        return handle_picker_key(key);
    }

    // ダッシュボード表示中
    if app.show_dashboard {
        return match app.dashboard.mode {
            Mode::Filter => handle_filter_key(key),
            _ => handle_service_select_key(key),
        };
    }

    // アクティブタブのモード/ビュー
    let Some(tab) = app.active_tab() else {
        return Action::Noop;
    };

    // タブ固有モーダルが優先
    match &tab.mode {
        Mode::Confirm(_) => return handle_confirm_key(key),
        Mode::Form(_) => return handle_form_key(key),
        Mode::DangerConfirm(_) => return handle_danger_confirm_key(key),
        _ => {}
    }

    // タブ操作キー（Normalモード時のみ）
    if tab.mode == Mode::Normal {
        match key.code {
            KeyCode::Tab if key.modifiers.is_empty() => return Action::NextTab,
            KeyCode::BackTab => return Action::PrevTab,
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Action::CloseTab;
            }
            KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Action::NewTab;
            }
            _ => {}
        }
    }

    // View別のハンドリング
    let Some(view) = app.current_view() else {
        return Action::Noop;
    };
    let mode = &tab.mode;
    match view {
        View::ServiceSelect => match mode {
            Mode::Filter => handle_filter_key(key),
            _ => handle_service_select_key(key),
        },
        View::Ec2List => match mode {
            Mode::Filter => handle_filter_key(key),
            _ => handle_ec2_list_key(key),
        },
        View::EcrList | View::EcsList | View::VpcList => match mode {
            Mode::Filter => handle_filter_key(key),
            _ => handle_generic_list_key(key),
        },
        View::S3List => match mode {
            Mode::Filter => handle_filter_key(key),
            _ => handle_s3_list_key(key),
        },
        View::SecretsList => match mode {
            Mode::Filter => handle_filter_key(key),
            _ => handle_secrets_list_key(key),
        },
        View::Ec2Detail => handle_ec2_detail_key(key),
        View::EcrDetail | View::EcsDetail | View::VpcDetail => handle_generic_detail_key(key),
        View::S3Detail => handle_s3_detail_key(key),
        View::SecretsDetail => handle_secrets_detail_key(key),
    }
}

/// 確認ダイアログのキー処理
fn handle_confirm_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('y') => Action::ConfirmYes,
        KeyCode::Char('n') | KeyCode::Esc => Action::ConfirmNo,
        _ => Action::Noop,
    }
}

/// メッセージダイアログのキー処理
fn handle_message_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Enter | KeyCode::Esc => Action::DismissMessage,
        _ => Action::Noop,
    }
}

/// ヘルプポップアップのキー処理
fn handle_help_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc | KeyCode::Char('?') => Action::Back,
        _ => Action::Noop,
    }
}

/// サービスピッカーのキー処理
fn handle_picker_key(key: KeyEvent) -> Action {
    // C-n / C-p で選択移動
    if key.code == KeyCode::Char('n') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Action::PickerMoveDown;
    }
    if key.code == KeyCode::Char('p') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Action::PickerMoveUp;
    }
    match key.code {
        KeyCode::Enter => Action::PickerConfirm,
        KeyCode::Esc => Action::PickerCancel,
        KeyCode::Down => Action::PickerMoveDown,
        KeyCode::Up => Action::PickerMoveUp,
        _ => {
            if let Some(req) = to_input_request(&Event::Key(key)) {
                Action::PickerHandleInput(req)
            } else {
                Action::Noop
            }
        }
    }
}

/// サービス選択画面のキー処理
fn handle_service_select_key(key: KeyEvent) -> Action {
    if is_quit_key(&key) {
        return Action::Quit;
    }
    // ダッシュボードでも Ctrl+t でピッカーを開ける
    if key.code == KeyCode::Char('t') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Action::NewTab;
    }
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Action::MoveDown,
        KeyCode::Char('k') | KeyCode::Up => Action::MoveUp,
        KeyCode::Enter => Action::Enter,
        KeyCode::Char('/') => Action::StartFilter,
        KeyCode::Char('?') => Action::ShowHelp,
        KeyCode::Esc => Action::Back,
        _ => Action::Noop,
    }
}

/// EC2一覧画面(Normalモード)のキー処理
fn handle_ec2_list_key(key: KeyEvent) -> Action {
    if is_quit_key(&key) {
        return Action::Quit;
    }
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Action::MoveDown,
        KeyCode::Char('k') | KeyCode::Up => Action::MoveUp,
        KeyCode::Char('g') => Action::MoveToTop,
        KeyCode::Char('G') => Action::MoveToBottom,
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::HalfPageDown,
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::HalfPageUp,
        KeyCode::Enter => Action::Enter,
        KeyCode::Char('/') => Action::StartFilter,
        KeyCode::Char('S') => Action::StartStop,
        KeyCode::Char('R') => Action::Reboot,
        KeyCode::Char('r') => Action::Refresh,
        KeyCode::Char('y') => Action::CopyId,
        KeyCode::Char('D') => Action::Delete,
        KeyCode::Char('?') => Action::ShowHelp,
        KeyCode::Esc => Action::Back,
        _ => Action::Noop,
    }
}

/// 汎用リストビュー(Normalモード)のキー処理 (ECR, ECS, S3, VPC, Secrets)
fn handle_generic_list_key(key: KeyEvent) -> Action {
    if is_quit_key(&key) {
        return Action::Quit;
    }
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Action::MoveDown,
        KeyCode::Char('k') | KeyCode::Up => Action::MoveUp,
        KeyCode::Char('g') => Action::MoveToTop,
        KeyCode::Char('G') => Action::MoveToBottom,
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::HalfPageDown,
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::HalfPageUp,
        KeyCode::Enter => Action::Enter,
        KeyCode::Char('/') => Action::StartFilter,
        KeyCode::Char('r') => Action::Refresh,
        KeyCode::Char('y') => Action::CopyId,
        KeyCode::Char('?') => Action::ShowHelp,
        KeyCode::Esc => Action::Back,
        _ => Action::Noop,
    }
}

/// S3一覧画面(Normalモード)のキー処理
fn handle_s3_list_key(key: KeyEvent) -> Action {
    if is_quit_key(&key) {
        return Action::Quit;
    }
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Action::MoveDown,
        KeyCode::Char('k') | KeyCode::Up => Action::MoveUp,
        KeyCode::Char('g') => Action::MoveToTop,
        KeyCode::Char('G') => Action::MoveToBottom,
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::HalfPageDown,
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::HalfPageUp,
        KeyCode::Enter => Action::Enter,
        KeyCode::Char('/') => Action::StartFilter,
        KeyCode::Char('r') => Action::Refresh,
        KeyCode::Char('y') => Action::CopyId,
        KeyCode::Char('c') => Action::Create,
        KeyCode::Char('D') => Action::Delete,
        KeyCode::Char('?') => Action::ShowHelp,
        KeyCode::Esc => Action::Back,
        _ => Action::Noop,
    }
}

/// Secrets一覧画面(Normalモード)のキー処理
fn handle_secrets_list_key(key: KeyEvent) -> Action {
    if is_quit_key(&key) {
        return Action::Quit;
    }
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Action::MoveDown,
        KeyCode::Char('k') | KeyCode::Up => Action::MoveUp,
        KeyCode::Char('g') => Action::MoveToTop,
        KeyCode::Char('G') => Action::MoveToBottom,
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::HalfPageDown,
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::HalfPageUp,
        KeyCode::Enter => Action::Enter,
        KeyCode::Char('/') => Action::StartFilter,
        KeyCode::Char('r') => Action::Refresh,
        KeyCode::Char('y') => Action::CopyId,
        KeyCode::Char('c') => Action::Create,
        KeyCode::Char('D') => Action::Delete,
        KeyCode::Char('?') => Action::ShowHelp,
        KeyCode::Esc => Action::Back,
        _ => Action::Noop,
    }
}

/// フィルタモードのキー処理
fn handle_filter_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Enter => Action::ConfirmFilter,
        KeyCode::Esc => Action::CancelFilter,
        _ => {
            if let Some(req) = to_input_request(&Event::Key(key)) {
                Action::FilterHandleInput(req)
            } else {
                Action::Noop
            }
        }
    }
}

/// EC2詳細画面のキー処理
fn handle_ec2_detail_key(key: KeyEvent) -> Action {
    if is_quit_key(&key) {
        return Action::Quit;
    }
    match key.code {
        KeyCode::Char(']') => Action::SwitchDetailTab,
        KeyCode::Char('j') | KeyCode::Down => Action::MoveDown,
        KeyCode::Char('k') | KeyCode::Up => Action::MoveUp,
        KeyCode::Enter => Action::FollowLink,
        KeyCode::Char('S') => Action::StartStop,
        KeyCode::Char('R') => Action::Reboot,
        KeyCode::Char('y') => Action::CopyId,
        KeyCode::Char('?') => Action::ShowHelp,
        KeyCode::Esc => Action::Back,
        _ => Action::Noop,
    }
}

/// 汎用詳細画面のキー処理 (ECR Detail, ECS Detail, VPC Detail)
fn handle_generic_detail_key(key: KeyEvent) -> Action {
    if is_quit_key(&key) {
        return Action::Quit;
    }
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Action::MoveDown,
        KeyCode::Char('k') | KeyCode::Up => Action::MoveUp,
        KeyCode::Char('g') => Action::MoveToTop,
        KeyCode::Char('G') => Action::MoveToBottom,
        KeyCode::Char('y') => Action::CopyId,
        KeyCode::Char('?') => Action::ShowHelp,
        KeyCode::Esc => Action::Back,
        _ => Action::Noop,
    }
}

/// S3詳細画面のキー処理
fn handle_s3_detail_key(key: KeyEvent) -> Action {
    if is_quit_key(&key) {
        return Action::Quit;
    }
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Action::MoveDown,
        KeyCode::Char('k') | KeyCode::Up => Action::MoveUp,
        KeyCode::Char('g') => Action::MoveToTop,
        KeyCode::Char('G') => Action::MoveToBottom,
        KeyCode::Enter => Action::Enter,
        KeyCode::Char('D') => Action::Delete,
        KeyCode::Char('?') => Action::ShowHelp,
        KeyCode::Esc => Action::Back,
        _ => Action::Noop,
    }
}

/// Secrets詳細画面のキー処理
fn handle_secrets_detail_key(key: KeyEvent) -> Action {
    if is_quit_key(&key) {
        return Action::Quit;
    }
    match key.code {
        KeyCode::Char(']') => Action::SwitchDetailTab,
        KeyCode::Char('j') | KeyCode::Down => Action::MoveDown,
        KeyCode::Char('k') | KeyCode::Up => Action::MoveUp,
        KeyCode::Char('y') => Action::CopyId,
        KeyCode::Char('e') => Action::Edit,
        KeyCode::Char('?') => Action::ShowHelp,
        KeyCode::Esc => Action::Back,
        _ => Action::Noop,
    }
}

/// フォームモードのキー処理
fn handle_form_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Enter => Action::FormSubmit,
        KeyCode::Esc => Action::FormCancel,
        KeyCode::Tab => Action::FormNextField,
        _ => {
            if let Some(req) = to_input_request(&Event::Key(key)) {
                Action::FormHandleInput(req)
            } else {
                Action::Noop
            }
        }
    }
}

/// 危険操作確認モードのキー処理
fn handle_danger_confirm_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Enter => Action::DangerConfirmSubmit,
        KeyCode::Esc => Action::DangerConfirmCancel,
        _ => {
            if let Some(req) = to_input_request(&Event::Key(key)) {
                Action::DangerConfirmHandleInput(req)
            } else {
                Action::Noop
            }
        }
    }
}

/// 終了キーかどうかを判定
fn is_quit_key(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('q'))
        || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{ConfirmAction, Message, MessageLevel};
    use crate::service::ServiceKind;
    use crate::tab::TabView;
    use rstest::rstest;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn key_with_ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn key_char(c: char) -> KeyEvent {
        key(KeyCode::Char(c))
    }

    /// ViewからServiceKindとTabViewに変換
    fn view_to_service(view: &View) -> (ServiceKind, TabView) {
        match view {
            View::Ec2List => (ServiceKind::Ec2, TabView::List),
            View::Ec2Detail => (ServiceKind::Ec2, TabView::Detail),
            View::EcrList => (ServiceKind::Ecr, TabView::List),
            View::EcrDetail => (ServiceKind::Ecr, TabView::Detail),
            View::EcsList => (ServiceKind::Ecs, TabView::List),
            View::EcsDetail => (ServiceKind::Ecs, TabView::Detail),
            View::S3List => (ServiceKind::S3, TabView::List),
            View::S3Detail => (ServiceKind::S3, TabView::Detail),
            View::VpcList => (ServiceKind::Vpc, TabView::List),
            View::VpcDetail => (ServiceKind::Vpc, TabView::Detail),
            View::SecretsList => (ServiceKind::SecretsManager, TabView::List),
            View::SecretsDetail => (ServiceKind::SecretsManager, TabView::Detail),
            View::ServiceSelect => unreachable!("ServiceSelect has no tab"),
        }
    }

    fn app_with_view(view: View) -> App {
        let mut app = App::new("dev".to_string(), None);
        if view == View::ServiceSelect {
            // show_dashboard is true by default
            return app;
        }
        let (service, tab_view) = view_to_service(&view);
        app.create_tab(service);
        if tab_view == TabView::Detail {
            if let Some(tab) = app.active_tab_mut() {
                tab.tab_view = TabView::Detail;
            }
        }
        app
    }

    fn app_with_mode(view: View, mode: Mode) -> App {
        let is_service_select = view == View::ServiceSelect;
        let mut app = app_with_view(view);
        match mode {
            Mode::Message => {
                app.message = Some(Message {
                    level: MessageLevel::Info,
                    title: "Test".to_string(),
                    body: "Test message".to_string(),
                });
            }
            Mode::Help => {
                app.show_help = true;
            }
            _ => {
                if is_service_select {
                    app.dashboard.mode = mode;
                } else if let Some(tab) = app.active_tab_mut() {
                    tab.mode = mode;
                }
            }
        }
        app
    }

    // ──────────────────────────────────────────────
    // サービス選択画面テスト
    // ──────────────────────────────────────────────

    #[rstest]
    #[case(key_char('j'), Action::MoveDown)]
    #[case(key(KeyCode::Down), Action::MoveDown)]
    #[case(key_char('k'), Action::MoveUp)]
    #[case(key(KeyCode::Up), Action::MoveUp)]
    #[case(key(KeyCode::Enter), Action::Enter)]
    #[case(key_char('/'), Action::StartFilter)]
    #[case(key(KeyCode::Esc), Action::Back)]
    #[case(key_char('q'), Action::Quit)]
    #[case(key_char('?'), Action::ShowHelp)]
    #[case(key_char('x'), Action::Noop)]
    fn handle_key_returns_expected_action_when_service_select(
        #[case] input: KeyEvent,
        #[case] expected: Action,
    ) {
        let app = app_with_view(View::ServiceSelect);
        assert_eq!(handle_key(&app, input), expected);
    }

    // ──────────────────────────────────────────────
    // EC2一覧画面(Normalモード)テスト
    // ──────────────────────────────────────────────

    #[rstest]
    #[case(key_char('j'), Action::MoveDown)]
    #[case(key(KeyCode::Down), Action::MoveDown)]
    #[case(key_char('k'), Action::MoveUp)]
    #[case(key(KeyCode::Up), Action::MoveUp)]
    #[case(key_char('g'), Action::MoveToTop)]
    #[case(key_char('G'), Action::MoveToBottom)]
    #[case(key(KeyCode::Enter), Action::Enter)]
    #[case(key_char('/'), Action::StartFilter)]
    #[case(key_char('S'), Action::StartStop)]
    #[case(key_char('R'), Action::Reboot)]
    #[case(key_char('r'), Action::Refresh)]
    #[case(key_char('y'), Action::CopyId)]
    #[case(key_char('?'), Action::ShowHelp)]
    #[case(key(KeyCode::Esc), Action::Back)]
    #[case(key_char('q'), Action::Quit)]
    #[case(key_char('x'), Action::Noop)]
    fn handle_key_returns_expected_action_when_ec2_list_normal(
        #[case] input: KeyEvent,
        #[case] expected: Action,
    ) {
        let app = app_with_view(View::Ec2List);
        assert_eq!(handle_key(&app, input), expected);
    }

    #[test]
    fn handle_key_returns_half_page_down_when_ctrl_d_in_ec2_list() {
        let app = app_with_view(View::Ec2List);
        assert_eq!(handle_key(&app, key_with_ctrl('d')), Action::HalfPageDown);
    }

    #[test]
    fn handle_key_returns_half_page_up_when_ctrl_u_in_ec2_list() {
        let app = app_with_view(View::Ec2List);
        assert_eq!(handle_key(&app, key_with_ctrl('u')), Action::HalfPageUp);
    }

    // ──────────────────────────────────────────────
    // 汎用リスト画面テスト (ECR)
    // ──────────────────────────────────────────────

    #[rstest]
    #[case(key_char('j'), Action::MoveDown)]
    #[case(key_char('k'), Action::MoveUp)]
    #[case(key(KeyCode::Enter), Action::Enter)]
    #[case(key_char('/'), Action::StartFilter)]
    #[case(key_char('r'), Action::Refresh)]
    #[case(key_char('y'), Action::CopyId)]
    #[case(key(KeyCode::Esc), Action::Back)]
    #[case(key_char('q'), Action::Quit)]
    fn handle_key_returns_expected_action_when_ecr_list_normal(
        #[case] input: KeyEvent,
        #[case] expected: Action,
    ) {
        let app = app_with_view(View::EcrList);
        assert_eq!(handle_key(&app, input), expected);
    }

    // ──────────────────────────────────────────────
    // EC2一覧画面(Filterモード)テスト
    // ──────────────────────────────────────────────

    #[rstest]
    #[case(key(KeyCode::Enter), Action::ConfirmFilter)]
    #[case(key(KeyCode::Esc), Action::CancelFilter)]
    fn handle_key_returns_expected_action_when_ec2_list_filter(
        #[case] input: KeyEvent,
        #[case] expected: Action,
    ) {
        let app = app_with_mode(View::Ec2List, Mode::Filter);
        assert_eq!(handle_key(&app, input), expected);
    }

    #[test]
    fn handle_key_returns_filter_handle_input_when_char_in_filter() {
        let app = app_with_mode(View::Ec2List, Mode::Filter);
        let action = handle_key(&app, key_char('a'));
        assert!(matches!(action, Action::FilterHandleInput(_)));
    }

    #[test]
    fn handle_key_returns_filter_handle_input_when_backspace_in_filter() {
        let app = app_with_mode(View::Ec2List, Mode::Filter);
        let action = handle_key(&app, key(KeyCode::Backspace));
        assert!(matches!(action, Action::FilterHandleInput(_)));
    }

    // ──────────────────────────────────────────────
    // EC2詳細画面テスト
    // ──────────────────────────────────────────────

    #[rstest]
    #[case(key_char(']'), Action::SwitchDetailTab)]
    #[case(key(KeyCode::Enter), Action::FollowLink)]
    #[case(key_char('j'), Action::MoveDown)]
    #[case(key(KeyCode::Down), Action::MoveDown)]
    #[case(key_char('k'), Action::MoveUp)]
    #[case(key(KeyCode::Up), Action::MoveUp)]
    #[case(key_char('S'), Action::StartStop)]
    #[case(key_char('R'), Action::Reboot)]
    #[case(key_char('y'), Action::CopyId)]
    #[case(key_char('?'), Action::ShowHelp)]
    #[case(key(KeyCode::Esc), Action::Back)]
    #[case(key_char('q'), Action::Quit)]
    #[case(key_char('x'), Action::Noop)]
    fn handle_key_returns_expected_action_when_ec2_detail(
        #[case] input: KeyEvent,
        #[case] expected: Action,
    ) {
        let app = app_with_view(View::Ec2Detail);
        assert_eq!(handle_key(&app, input), expected);
    }

    // ──────────────────────────────────────────────
    // S3詳細画面テスト
    // ──────────────────────────────────────────────

    #[rstest]
    #[case(key(KeyCode::Enter), Action::Enter)]
    #[case(key_char('j'), Action::MoveDown)]
    #[case(key_char('k'), Action::MoveUp)]
    #[case(key(KeyCode::Esc), Action::Back)]
    #[case(key_char('q'), Action::Quit)]
    fn handle_key_returns_expected_action_when_s3_detail(
        #[case] input: KeyEvent,
        #[case] expected: Action,
    ) {
        let app = app_with_view(View::S3Detail);
        assert_eq!(handle_key(&app, input), expected);
    }

    // ──────────────────────────────────────────────
    // Secrets詳細画面テスト
    // ──────────────────────────────────────────────

    #[rstest]
    #[case(key_char(']'), Action::SwitchDetailTab)]
    #[case(key_char('j'), Action::MoveDown)]
    #[case(key_char('k'), Action::MoveUp)]
    #[case(key_char('y'), Action::CopyId)]
    #[case(key(KeyCode::Esc), Action::Back)]
    #[case(key_char('q'), Action::Quit)]
    fn handle_key_returns_expected_action_when_secrets_detail(
        #[case] input: KeyEvent,
        #[case] expected: Action,
    ) {
        let app = app_with_view(View::SecretsDetail);
        assert_eq!(handle_key(&app, input), expected);
    }

    // ──────────────────────────────────────────────
    // 確認ダイアログテスト
    // ──────────────────────────────────────────────

    #[rstest]
    #[case(key_char('y'), Action::ConfirmYes)]
    #[case(key_char('n'), Action::ConfirmNo)]
    #[case(key(KeyCode::Esc), Action::ConfirmNo)]
    #[case(key_char('x'), Action::Noop)]
    fn handle_key_returns_expected_action_when_confirm_dialog(
        #[case] input: KeyEvent,
        #[case] expected: Action,
    ) {
        let app = app_with_mode(
            View::Ec2List,
            Mode::Confirm(ConfirmAction::Stop("i-123".to_string())),
        );
        assert_eq!(handle_key(&app, input), expected);
    }

    // ──────────────────────────────────────────────
    // メッセージダイアログテスト
    // ──────────────────────────────────────────────

    #[rstest]
    #[case(key(KeyCode::Enter), Action::DismissMessage)]
    #[case(key(KeyCode::Esc), Action::DismissMessage)]
    #[case(key_char('x'), Action::Noop)]
    fn handle_key_returns_expected_action_when_message_dialog(
        #[case] input: KeyEvent,
        #[case] expected: Action,
    ) {
        let app = app_with_mode(View::Ec2List, Mode::Message);
        assert_eq!(handle_key(&app, input), expected);
    }

    // ──────────────────────────────────────────────
    // ヘルプポップアップテスト
    // ──────────────────────────────────────────────

    #[rstest]
    #[case(key(KeyCode::Esc), Action::Back)]
    #[case(key_char('?'), Action::Back)]
    #[case(key_char('x'), Action::Noop)]
    fn handle_key_returns_expected_action_when_help_popup(
        #[case] input: KeyEvent,
        #[case] expected: Action,
    ) {
        let app = app_with_mode(View::Ec2List, Mode::Help);
        assert_eq!(handle_key(&app, input), expected);
    }

    // ──────────────────────────────────────────────
    // モーダル優先テスト
    // ──────────────────────────────────────────────

    #[test]
    fn handle_key_returns_confirm_yes_when_confirm_dialog_overrides_view_keys() {
        let app = app_with_mode(
            View::Ec2List,
            Mode::Confirm(ConfirmAction::Start("i-123".to_string())),
        );
        assert_eq!(handle_key(&app, key_char('y')), Action::ConfirmYes);
    }

    #[test]
    fn handle_key_returns_dismiss_message_when_message_overrides_view_keys() {
        let app = app_with_mode(View::Ec2List, Mode::Message);
        assert_eq!(
            handle_key(&app, key(KeyCode::Enter)),
            Action::DismissMessage
        );
    }

    // ──────────────────────────────────────────────
    // S3リスト画面テスト
    // ──────────────────────────────────────────────

    #[rstest]
    #[case(key_char('j'), Action::MoveDown)]
    #[case(key_char('k'), Action::MoveUp)]
    #[case(key(KeyCode::Enter), Action::Enter)]
    #[case(key_char('/'), Action::StartFilter)]
    #[case(key_char('r'), Action::Refresh)]
    #[case(key_char('y'), Action::CopyId)]
    #[case(key_char('c'), Action::Create)]
    #[case(key_char('D'), Action::Delete)]
    #[case(key(KeyCode::Esc), Action::Back)]
    #[case(key_char('q'), Action::Quit)]
    fn handle_key_returns_expected_action_when_s3_list_normal(
        #[case] input: KeyEvent,
        #[case] expected: Action,
    ) {
        let app = app_with_view(View::S3List);
        assert_eq!(handle_key(&app, input), expected);
    }

    // ──────────────────────────────────────────────
    // Secretsリスト画面テスト
    // ──────────────────────────────────────────────

    #[rstest]
    #[case(key_char('j'), Action::MoveDown)]
    #[case(key_char('k'), Action::MoveUp)]
    #[case(key(KeyCode::Enter), Action::Enter)]
    #[case(key_char('/'), Action::StartFilter)]
    #[case(key_char('r'), Action::Refresh)]
    #[case(key_char('y'), Action::CopyId)]
    #[case(key_char('c'), Action::Create)]
    #[case(key_char('D'), Action::Delete)]
    #[case(key(KeyCode::Esc), Action::Back)]
    #[case(key_char('q'), Action::Quit)]
    fn handle_key_returns_expected_action_when_secrets_list_normal(
        #[case] input: KeyEvent,
        #[case] expected: Action,
    ) {
        let app = app_with_view(View::SecretsList);
        assert_eq!(handle_key(&app, input), expected);
    }

    // ──────────────────────────────────────────────
    // EC2リスト Delete キーテスト
    // ──────────────────────────────────────────────

    #[test]
    fn handle_key_returns_delete_when_shift_d_in_ec2_list() {
        let app = app_with_view(View::Ec2List);
        assert_eq!(
            handle_key(&app, KeyEvent::new(KeyCode::Char('D'), KeyModifiers::SHIFT)),
            Action::Delete,
        );
    }

    // ──────────────────────────────────────────────
    // S3詳細 Delete キーテスト
    // ──────────────────────────────────────────────

    #[test]
    fn handle_key_returns_delete_when_shift_d_in_s3_detail() {
        let app = app_with_view(View::S3Detail);
        assert_eq!(
            handle_key(&app, KeyEvent::new(KeyCode::Char('D'), KeyModifiers::SHIFT)),
            Action::Delete,
        );
    }

    // ──────────────────────────────────────────────
    // Secrets詳細 Edit キーテスト
    // ──────────────────────────────────────────────

    #[test]
    fn handle_key_returns_edit_when_e_in_secrets_detail() {
        let app = app_with_view(View::SecretsDetail);
        assert_eq!(handle_key(&app, key_char('e')), Action::Edit);
    }

    // ──────────────────────────────────────────────
    // フォームモードテスト
    // ──────────────────────────────────────────────

    #[rstest]
    #[case(key(KeyCode::Enter), Action::FormSubmit)]
    #[case(key(KeyCode::Esc), Action::FormCancel)]
    #[case(key(KeyCode::Tab), Action::FormNextField)]
    fn handle_key_returns_expected_action_when_form_mode(
        #[case] input: KeyEvent,
        #[case] expected: Action,
    ) {
        use crate::app::{FormContext, FormField, FormKind};
        use tui_input::Input;
        let app = app_with_mode(
            View::S3List,
            Mode::Form(FormContext {
                kind: FormKind::CreateS3Bucket,
                fields: vec![FormField {
                    label: "Name".to_string(),
                    input: Input::default(),
                    required: true,
                }],
                focused_field: 0,
            }),
        );
        assert_eq!(handle_key(&app, input), expected);
    }

    #[test]
    fn handle_key_returns_form_handle_input_when_char_in_form() {
        use crate::app::{FormContext, FormField, FormKind};
        use tui_input::Input;
        let app = app_with_mode(
            View::S3List,
            Mode::Form(FormContext {
                kind: FormKind::CreateS3Bucket,
                fields: vec![FormField {
                    label: "Name".to_string(),
                    input: Input::default(),
                    required: true,
                }],
                focused_field: 0,
            }),
        );
        let action = handle_key(&app, key_char('a'));
        assert!(matches!(action, Action::FormHandleInput(_)));
    }

    // ──────────────────────────────────────────────
    // 危険操作確認モードテスト
    // ──────────────────────────────────────────────

    #[rstest]
    #[case(key(KeyCode::Enter), Action::DangerConfirmSubmit)]
    #[case(key(KeyCode::Esc), Action::DangerConfirmCancel)]
    fn handle_key_returns_expected_action_when_danger_confirm_mode(
        #[case] input: KeyEvent,
        #[case] expected: Action,
    ) {
        use crate::app::{DangerAction, DangerConfirmContext};
        use tui_input::Input;
        let app = app_with_mode(
            View::Ec2List,
            Mode::DangerConfirm(DangerConfirmContext {
                action: DangerAction::TerminateEc2("i-001".to_string()),
                input: Input::default(),
            }),
        );
        assert_eq!(handle_key(&app, input), expected);
    }

    #[test]
    fn handle_key_returns_danger_confirm_handle_input_when_char_in_danger_confirm() {
        use crate::app::{DangerAction, DangerConfirmContext};
        use tui_input::Input;
        let app = app_with_mode(
            View::Ec2List,
            Mode::DangerConfirm(DangerConfirmContext {
                action: DangerAction::TerminateEc2("i-001".to_string()),
                input: Input::default(),
            }),
        );
        let action = handle_key(&app, key_char('i'));
        assert!(matches!(action, Action::DangerConfirmHandleInput(_)));
    }

    // ──────────────────────────────────────────────
    // フォーム/DangerConfirmモーダル優先テスト
    // ──────────────────────────────────────────────

    #[test]
    fn handle_key_returns_form_submit_when_form_overrides_view_keys() {
        use crate::app::{FormContext, FormField, FormKind};
        use tui_input::Input;
        let app = app_with_mode(
            View::S3List,
            Mode::Form(FormContext {
                kind: FormKind::CreateS3Bucket,
                fields: vec![FormField {
                    label: "Name".to_string(),
                    input: Input::default(),
                    required: true,
                }],
                focused_field: 0,
            }),
        );
        assert_eq!(handle_key(&app, key(KeyCode::Enter)), Action::FormSubmit);
    }

    #[test]
    fn handle_key_returns_danger_confirm_submit_when_danger_overrides_view_keys() {
        use crate::app::{DangerAction, DangerConfirmContext};
        use tui_input::Input;
        let app = app_with_mode(
            View::Ec2List,
            Mode::DangerConfirm(DangerConfirmContext {
                action: DangerAction::TerminateEc2("i-001".to_string()),
                input: Input::default(),
            }),
        );
        assert_eq!(
            handle_key(&app, key(KeyCode::Enter)),
            Action::DangerConfirmSubmit
        );
    }

    // ──────────────────────────────────────────────
    // ServiceSelect Filterモードテスト
    // ──────────────────────────────────────────────

    #[rstest]
    #[case(key(KeyCode::Enter), Action::ConfirmFilter)]
    #[case(key(KeyCode::Esc), Action::CancelFilter)]
    fn handle_key_returns_expected_action_when_service_select_filter(
        #[case] input: KeyEvent,
        #[case] expected: Action,
    ) {
        let app = app_with_mode(View::ServiceSelect, Mode::Filter);
        assert_eq!(handle_key(&app, input), expected);
    }

    #[test]
    fn handle_key_returns_filter_handle_input_when_char_in_service_filter() {
        let app = app_with_mode(View::ServiceSelect, Mode::Filter);
        let action = handle_key(&app, key_char('s'));
        assert!(matches!(action, Action::FilterHandleInput(_)));
    }

    // ──────────────────────────────────────────────
    // タブ操作テスト
    // ──────────────────────────────────────────────

    #[test]
    fn handle_key_returns_next_tab_when_tab_key_in_normal_mode() {
        let app = app_with_view(View::Ec2List);
        assert_eq!(handle_key(&app, key(KeyCode::Tab)), Action::NextTab);
    }

    #[test]
    fn handle_key_returns_prev_tab_when_backtab_in_normal_mode() {
        let app = app_with_view(View::Ec2List);
        assert_eq!(handle_key(&app, key(KeyCode::BackTab)), Action::PrevTab);
    }

    #[test]
    fn handle_key_returns_close_tab_when_ctrl_w_in_normal_mode() {
        let app = app_with_view(View::Ec2List);
        assert_eq!(
            handle_key(
                &app,
                KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL)
            ),
            Action::CloseTab
        );
    }

    #[test]
    fn handle_key_returns_new_tab_when_ctrl_t_in_normal_mode() {
        let app = app_with_view(View::Ec2List);
        assert_eq!(
            handle_key(
                &app,
                KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL)
            ),
            Action::NewTab
        );
    }

    #[test]
    fn handle_key_returns_noop_when_tab_key_in_filter_mode() {
        let app = app_with_mode(View::Ec2List, Mode::Filter);
        // Filterモードでは Tab はタブ操作にならない
        assert_eq!(handle_key(&app, key(KeyCode::Tab)), Action::Noop);
    }

    // ──────────────────────────────────────────────
    // サービスピッカーテスト
    // ──────────────────────────────────────────────

    fn app_with_picker() -> App {
        let mut app = app_with_view(View::Ec2List);
        app.dispatch(Action::NewTab); // ピッカーを開く
        app
    }

    #[test]
    fn handle_key_returns_picker_confirm_when_enter_in_picker() {
        let app = app_with_picker();
        assert!(app.service_picker.is_some());
        assert_eq!(handle_key(&app, key(KeyCode::Enter)), Action::PickerConfirm);
    }

    #[test]
    fn handle_key_returns_picker_cancel_when_esc_in_picker() {
        let app = app_with_picker();
        assert_eq!(handle_key(&app, key(KeyCode::Esc)), Action::PickerCancel);
    }

    #[test]
    fn handle_key_returns_picker_handle_input_when_j_in_picker() {
        let app = app_with_picker();
        let action = handle_key(&app, key_char('j'));
        assert!(matches!(action, Action::PickerHandleInput(_)));
    }

    #[test]
    fn handle_key_returns_picker_move_down_when_ctrl_n_in_picker() {
        let app = app_with_picker();
        assert_eq!(
            handle_key(&app, key_with_ctrl('n')),
            Action::PickerMoveDown
        );
    }

    #[test]
    fn handle_key_returns_picker_move_up_when_ctrl_p_in_picker() {
        let app = app_with_picker();
        assert_eq!(
            handle_key(&app, key_with_ctrl('p')),
            Action::PickerMoveUp
        );
    }

    #[test]
    fn handle_key_returns_picker_handle_input_when_char_in_picker() {
        let app = app_with_picker();
        let action = handle_key(&app, key_char('s'));
        assert!(matches!(action, Action::PickerHandleInput(_)));
    }
}
