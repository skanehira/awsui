use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use tui_input::backend::crossterm::to_input_request;

use crate::action::Action;
use crate::app::{App, Mode, View};

/// キーイベントをActionに変換する。
/// Appの現在のmode/viewに応じて適切なActionを返す。
pub fn handle_key(app: &App, key: KeyEvent) -> Action {
    // モーダルが優先
    match &app.mode {
        Mode::Confirm(_) => return handle_confirm_key(key),
        Mode::Message => return handle_message_key(key),
        Mode::Help => return handle_help_key(key),
        _ => {}
    }

    // View別のハンドリング
    match app.view {
        View::ProfileSelect => handle_profile_select_key(key),
        View::Ec2List => match app.mode {
            Mode::Filter => handle_filter_key(key),
            _ => handle_ec2_list_key(key),
        },
        View::Ec2Detail => handle_ec2_detail_key(key),
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

/// Profile選択画面のキー処理
fn handle_profile_select_key(key: KeyEvent) -> Action {
    if is_quit_key(&key) {
        return Action::Quit;
    }
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Action::MoveDown,
        KeyCode::Char('k') | KeyCode::Up => Action::MoveUp,
        KeyCode::Enter => Action::Enter,
        KeyCode::Char('?') => Action::ShowHelp,
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
        KeyCode::Char('?') => Action::ShowHelp,
        KeyCode::Esc => Action::Back,
        _ => Action::Noop,
    }
}

/// EC2一覧画面(Filterモード)のキー処理
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
        KeyCode::Tab => Action::SwitchDetailTab,
        KeyCode::Char('j') | KeyCode::Down => Action::MoveDown,
        KeyCode::Char('k') | KeyCode::Up => Action::MoveUp,
        KeyCode::Char('S') => Action::StartStop,
        KeyCode::Char('R') => Action::Reboot,
        KeyCode::Char('y') => Action::CopyId,
        KeyCode::Char('?') => Action::ShowHelp,
        KeyCode::Esc => Action::Back,
        _ => Action::Noop,
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
    use crate::app::ConfirmAction;
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

    fn app_with_view(view: View) -> App {
        let mut app = App::new(vec!["dev".to_string()]);
        app.view = view;
        app
    }

    fn app_with_mode(view: View, mode: Mode) -> App {
        let mut app = app_with_view(view);
        app.mode = mode;
        app
    }

    // ──────────────────────────────────────────────
    // Profile選択画面テスト
    // ──────────────────────────────────────────────

    #[rstest]
    #[case(key_char('j'), Action::MoveDown)]
    #[case(key(KeyCode::Down), Action::MoveDown)]
    #[case(key_char('k'), Action::MoveUp)]
    #[case(key(KeyCode::Up), Action::MoveUp)]
    #[case(key(KeyCode::Enter), Action::Enter)]
    #[case(key_char('q'), Action::Quit)]
    #[case(key_char('?'), Action::ShowHelp)]
    #[case(key_char('x'), Action::Noop)]
    fn handle_key_returns_expected_action_when_profile_select(
        #[case] input: KeyEvent,
        #[case] expected: Action,
    ) {
        let app = app_with_view(View::ProfileSelect);
        assert_eq!(handle_key(&app, input), expected);
    }

    #[test]
    fn handle_key_returns_quit_when_ctrl_c_in_profile_select() {
        let app = app_with_view(View::ProfileSelect);
        assert_eq!(handle_key(&app, key_with_ctrl('c')), Action::Quit);
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
    #[case(key(KeyCode::Tab), Action::SwitchDetailTab)]
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
        // 確認ダイアログ中は 'y' が ConfirmYes になる（CopyIdではなく）
        let app = app_with_mode(
            View::Ec2List,
            Mode::Confirm(ConfirmAction::Start("i-123".to_string())),
        );
        assert_eq!(handle_key(&app, key_char('y')), Action::ConfirmYes);
    }

    #[test]
    fn handle_key_returns_dismiss_message_when_message_overrides_view_keys() {
        // メッセージダイアログ中は Enter が DismissMessage になる（Enter/詳細ではなく）
        let app = app_with_mode(View::Ec2List, Mode::Message);
        assert_eq!(
            handle_key(&app, key(KeyCode::Enter)),
            Action::DismissMessage
        );
    }
}
