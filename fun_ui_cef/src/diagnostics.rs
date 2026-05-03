pub const FUN_UI_DIAGNOSTICS_TARGET: &str = "fun::ui";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum CefUiDiagnosticSeverity {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum CefUiDiagnosticKind {
    RuntimeInitialize,
    SubprocessHandled,
    BrowserCreated,
    PageLoaded,
    BrowserClosed,
    PaintReceived,
    DirtyRectUpload,
    SchemeRequestServed,
    SchemeRequestRejected,
    NavigationBlocked,
    JsBridgeMessageReceived,
    BridgePacketRejected,
    JsBridgeMessageRejected,
    OutgoingPatchCoalesced,
    OutgoingPatchDropped,
    OverlayStateChanged,
    ShutdownStarted,
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct CefUiDiagnosticEvent {
    pub kind: CefUiDiagnosticKind,
    pub severity: CefUiDiagnosticSeverity,
    pub browser_id: Option<i32>,
    pub detail: CefUiDiagnosticDetail,
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub enum CefUiDiagnosticDetail {
    None,
    Count { value: u64 },
    Code { value: i32 },
    Label { value: CefUiDiagnosticLabel },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum CefUiDiagnosticLabel {
    RemoteDebuggingEnabled,
    TransparentPaintingEnabled,
    WindowlessPaintingEnabled,
}

impl CefUiDiagnosticEvent {
    #[must_use]
    pub const fn lifecycle(kind: CefUiDiagnosticKind, severity: CefUiDiagnosticSeverity) -> Self {
        Self {
            kind,
            severity,
            browser_id: None,
            detail: CefUiDiagnosticDetail::None,
        }
    }
}
