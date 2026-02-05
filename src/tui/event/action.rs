/// User action that can be triggered by input or timer
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    // Navigation
    Quit,
    Back,
    SelectNext,
    SelectPrev,
    Enter,
    ScrollUp,
    ScrollDown,
    PageUp,
    PageDown,

    // View switching
    SwitchToList,
    SwitchToDetail(String),
    SwitchToTmux(String),

    // Task operations
    StartTask(String),
    StopTask(String),
    ResetTask(String),
    NextTask(String),
    RetryTask(String),
    SkipTask(String),
    DoneTask(String),
    FailTask(String),
    BlockTask(String),

    // Modal
    ShowHelp,
    HideModal,

    // Confirm dialog
    ShowConfirm {
        title: String,
        message: String,
        on_confirm: Box<Action>,
    },
    ConfirmYes,
    ConfirmNo,

    // Refresh
    Refresh,
    Tick,
}
