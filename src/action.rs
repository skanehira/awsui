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
    PrevDetailTab,
    RevealSecretValue,
    FollowLink,
    /// CRUD操作
    Create,
    Delete,
    Edit,
    /// フォーム入力操作
    FormSubmit,
    FormCancel,
    FormNextField,
    FormHandleInput(InputRequest),
    /// 危険操作確認（リソース名入力）
    DangerConfirmSubmit,
    DangerConfirmCancel,
    DangerConfirmHandleInput(InputRequest),
    /// タブ操作
    NextTab,
    PrevTab,
    CloseTab,
    NewTab,
    /// サービスピッカー操作
    PickerConfirm,
    PickerCancel,
    PickerMoveUp,
    PickerMoveDown,
    PickerHandleInput(InputRequest),
    Noop,
}
