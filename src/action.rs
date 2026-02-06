use tui_input::InputRequest;

/// UIアクション。キー入力から変換され、App状態を更新する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Quit,
    MoveUp,
    MoveDown,
    MoveToTop,
    MoveToBottom,
    HalfPageUp,
    HalfPageDown,
    Enter,
    Back,
    Refresh,
    CopyId,
    StartFilter,
    ConfirmFilter,
    CancelFilter,
    FilterHandleInput(InputRequest),
    StartStop,
    Reboot,
    ConfirmYes,
    ConfirmNo,
    DismissMessage,
    ShowHelp,
    SwitchDetailTab,
    Noop,
}
