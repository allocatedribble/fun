pub const FUN_UI_DIAGNOSTICS_TARGET: &str = "fun::ui";

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

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
    PaintTransportSelected,
    AcceleratedPaintReceived,
    AcceleratedPaintRejected,
    ShutdownStarted,
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct CefUiDiagnosticEvent {
    pub kind: CefUiDiagnosticKind,
    pub severity: CefUiDiagnosticSeverity,
    pub browser_id: Option<i32>,
    pub detail: CefUiDiagnosticDetail,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, compactly::v1::Encode)]
pub struct CefUiTransportCounterSnapshot {
    pub cef_on_paint_count: u64,
    pub cef_on_accelerated_paint_count: u64,
    pub cef_cpu_upload_bytes: u64,
    pub cef_gpu_copy_count: u64,
    pub cef_gpu_copy_bytes: u64,
    pub cef_gpu_copy_ns: u64,
    pub cef_gpu_copy_failures: u64,
    pub cef_transport_fallback_count: u64,
    pub cef_stale_gpu_frame_count: u64,
    pub cef_published_generation: u64,
}

#[derive(Debug, Default)]
struct CefUiTransportCounters {
    cef_on_paint_count: AtomicU64,
    cef_on_accelerated_paint_count: AtomicU64,
    cef_cpu_upload_bytes: AtomicU64,
    cef_gpu_copy_count: AtomicU64,
    cef_gpu_copy_bytes: AtomicU64,
    cef_gpu_copy_ns: AtomicU64,
    cef_gpu_copy_failures: AtomicU64,
    cef_transport_fallback_count: AtomicU64,
    cef_stale_gpu_frame_count: AtomicU64,
    cef_published_generation: AtomicU64,
}

#[derive(Debug, Clone, Default)]
pub struct SharedCefUiTransportCounters {
    counters: Arc<CefUiTransportCounters>,
}

impl SharedCefUiTransportCounters {
    pub fn record_on_paint(&self, copied_bytes: usize) {
        self.counters
            .cef_on_paint_count
            .fetch_add(1, Ordering::Relaxed);
        self.counters.cef_cpu_upload_bytes.fetch_add(
            copied_bytes.min(u64::MAX as usize) as u64,
            Ordering::Relaxed,
        );
    }

    pub fn record_on_accelerated_paint(&self) {
        self.counters
            .cef_on_accelerated_paint_count
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_gpu_copy(&self, copied_bytes: u64, copy_ns: u64) {
        self.counters
            .cef_gpu_copy_count
            .fetch_add(1, Ordering::Relaxed);
        self.counters
            .cef_gpu_copy_bytes
            .fetch_add(copied_bytes, Ordering::Relaxed);
        self.counters
            .cef_gpu_copy_ns
            .fetch_add(copy_ns, Ordering::Relaxed);
    }

    pub fn record_gpu_copy_failure(&self) {
        self.counters
            .cef_gpu_copy_failures
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_transport_fallback(&self) {
        self.counters
            .cef_transport_fallback_count
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_stale_gpu_frame(&self) {
        self.counters
            .cef_stale_gpu_frame_count
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_published_generation(&self, generation: u64) {
        self.counters
            .cef_published_generation
            .store(generation, Ordering::Relaxed);
    }

    #[must_use]
    pub fn snapshot(&self) -> CefUiTransportCounterSnapshot {
        CefUiTransportCounterSnapshot {
            cef_on_paint_count: self.counters.cef_on_paint_count.load(Ordering::Relaxed),
            cef_on_accelerated_paint_count: self
                .counters
                .cef_on_accelerated_paint_count
                .load(Ordering::Relaxed),
            cef_cpu_upload_bytes: self.counters.cef_cpu_upload_bytes.load(Ordering::Relaxed),
            cef_gpu_copy_count: self.counters.cef_gpu_copy_count.load(Ordering::Relaxed),
            cef_gpu_copy_bytes: self.counters.cef_gpu_copy_bytes.load(Ordering::Relaxed),
            cef_gpu_copy_ns: self.counters.cef_gpu_copy_ns.load(Ordering::Relaxed),
            cef_gpu_copy_failures: self.counters.cef_gpu_copy_failures.load(Ordering::Relaxed),
            cef_transport_fallback_count: self
                .counters
                .cef_transport_fallback_count
                .load(Ordering::Relaxed),
            cef_stale_gpu_frame_count: self
                .counters
                .cef_stale_gpu_frame_count
                .load(Ordering::Relaxed),
            cef_published_generation: self
                .counters
                .cef_published_generation
                .load(Ordering::Relaxed),
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_counters_snapshot_records_cpu_and_gpu_lanes() {
        let counters = SharedCefUiTransportCounters::default();

        counters.record_on_paint(16);
        counters.record_on_accelerated_paint();
        counters.record_gpu_copy(32, 40);
        counters.record_gpu_copy_failure();
        counters.record_transport_fallback();
        counters.record_stale_gpu_frame();
        counters.record_published_generation(11);

        assert_eq!(
            counters.snapshot(),
            CefUiTransportCounterSnapshot {
                cef_on_paint_count: 1,
                cef_on_accelerated_paint_count: 1,
                cef_cpu_upload_bytes: 16,
                cef_gpu_copy_count: 1,
                cef_gpu_copy_bytes: 32,
                cef_gpu_copy_ns: 40,
                cef_gpu_copy_failures: 1,
                cef_transport_fallback_count: 1,
                cef_stale_gpu_frame_count: 1,
                cef_published_generation: 11,
            }
        );
    }
}
