use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct Dx12CefInteropDiagnostics {
    init_success_count: AtomicU64,
    init_failure_count: AtomicU64,
    shared_texture_open_count: AtomicU64,
    shared_texture_open_failure_count: AtomicU64,
    gpu_copy_bytes: AtomicU64,
    gpu_copy_ns: AtomicU64,
    gpu_copy_failure_count: AtomicU64,
    fallback_count: AtomicU64,
    last_published_generation: AtomicU64,
    last_fence_value: AtomicU64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Dx12CefInteropDiagnosticSnapshot {
    pub init_success_count: u64,
    pub init_failure_count: u64,
    pub shared_texture_open_count: u64,
    pub shared_texture_open_failure_count: u64,
    pub gpu_copy_bytes: u64,
    pub gpu_copy_ns: u64,
    pub gpu_copy_failure_count: u64,
    pub fallback_count: u64,
    pub last_published_generation: u64,
    pub last_fence_value: u64,
}

impl Dx12CefInteropDiagnostics {
    pub fn record_init_success(&self) {
        self.init_success_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_init_failure(&self) {
        self.init_failure_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_shared_texture_open(&self) {
        self.shared_texture_open_count
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_shared_texture_open_failure(&self) {
        self.shared_texture_open_failure_count
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_gpu_copy(&self, copied_bytes: u64, copy_ns: u64) {
        self.gpu_copy_bytes
            .fetch_add(copied_bytes, Ordering::Relaxed);
        self.gpu_copy_ns.fetch_add(copy_ns, Ordering::Relaxed);
    }

    pub fn record_gpu_copy_failure(&self) {
        self.gpu_copy_failure_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_fallback(&self) {
        self.fallback_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_published_generation(&self, generation: u64, fence_value: u64) {
        self.last_published_generation
            .store(generation, Ordering::Relaxed);
        self.last_fence_value.store(fence_value, Ordering::Relaxed);
    }

    #[must_use]
    pub fn snapshot(&self) -> Dx12CefInteropDiagnosticSnapshot {
        Dx12CefInteropDiagnosticSnapshot {
            init_success_count: self.init_success_count.load(Ordering::Relaxed),
            init_failure_count: self.init_failure_count.load(Ordering::Relaxed),
            shared_texture_open_count: self.shared_texture_open_count.load(Ordering::Relaxed),
            shared_texture_open_failure_count: self
                .shared_texture_open_failure_count
                .load(Ordering::Relaxed),
            gpu_copy_bytes: self.gpu_copy_bytes.load(Ordering::Relaxed),
            gpu_copy_ns: self.gpu_copy_ns.load(Ordering::Relaxed),
            gpu_copy_failure_count: self.gpu_copy_failure_count.load(Ordering::Relaxed),
            fallback_count: self.fallback_count.load(Ordering::Relaxed),
            last_published_generation: self.last_published_generation.load(Ordering::Relaxed),
            last_fence_value: self.last_fence_value.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dx12_cef_interop_diagnostics_accumulate_counters() {
        let diagnostics = Dx12CefInteropDiagnostics::default();

        diagnostics.record_init_success();
        diagnostics.record_init_failure();
        diagnostics.record_shared_texture_open();
        diagnostics.record_shared_texture_open_failure();
        diagnostics.record_gpu_copy(4096, 120);
        diagnostics.record_gpu_copy_failure();
        diagnostics.record_fallback();
        diagnostics.record_published_generation(9, 12);

        assert_eq!(
            diagnostics.snapshot(),
            Dx12CefInteropDiagnosticSnapshot {
                init_success_count: 1,
                init_failure_count: 1,
                shared_texture_open_count: 1,
                shared_texture_open_failure_count: 1,
                gpu_copy_bytes: 4096,
                gpu_copy_ns: 120,
                gpu_copy_failure_count: 1,
                fallback_count: 1,
                last_published_generation: 9,
                last_fence_value: 12,
            }
        );
    }
}
