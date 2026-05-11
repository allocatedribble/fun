//! FS-11 page scheduler and virtual resource scheduling.
//!
//! Turns the renderer's typed page / virtual-resource policy into
//! scheduler-executable work. The renderer's existing page-priority
//! and virtual-resource code produces typed [`PageRequest`]s; this
//! module admits, coalesces, defers, or cancels them per the FS-11
//! scheduling rules, then emits a typed
//! [`PageSchedulerReport`] with `p50` / `p95` / `p99` latency
//! summaries.
//!
//! **Scope guard (FS-11)**: this module is contract + policy helper
//! only. The renderer remains owner of page priority — the
//! [`PagePriority`] value on a [`PageRequest`] is set by the
//! renderer's policy layer and is read-only from this scheduler's
//! perspective. The only priority transformation this module
//! performs is *aging* a pending request's effective priority based
//! on how many ticks it has waited; the base priority the renderer
//! supplied is preserved on the typed request record.
//!
//! No backend handles (wgpu / DX12 / Vulkan / Metal) leak into
//! scheduler decisions — every typed surface on this module is
//! either a primitive integer, a fixed enum, or a stable
//! `&'static str` label.

use fun_scheduler_types::schedule::ScheduleLane;

// ============================================================================
// PageWorkClass — 9 typed page-work taxonomy
// ============================================================================

/// FS-11 typed taxonomy of page / virtual-resource work classes.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Ord, PartialOrd)]
#[repr(u8)]
#[non_exhaustive]
pub enum PageWorkClass {
    /// Virtual-geometry page resolve (feedback → page ID set).
    VirtualGeometryPageResolve = 0,
    /// Virtual-geometry page upload (page bytes → GPU).
    VirtualGeometryPageUpload = 1,
    /// Virtual-shadow page update.
    VirtualShadowPageUpdate = 2,
    /// Shadow-atlas update (atlas allocation + content update).
    ShadowAtlasUpdate = 3,
    /// GI / probe cache update.
    GiProbeCacheUpdate = 4,
    /// Radiance / surface / probe cache refresh.
    RadianceSurfaceProbeCacheRefresh = 5,
    /// Texture-streaming page upload.
    TextureStreamingPageUpload = 6,
    /// Meshlet / cluster preparation.
    MeshletClusterPreparation = 7,
    /// Readback / diagnostic sampling.
    ReadbackDiagnosticSampling = 8,
    /// FS-M2.2: typed cloud-shadow refresh routed through the
    /// C7.2 / C7.3 / C7.4.5 cloud-shadow surface
    /// (`LuxCloudShadowProject` / `Filter` / `RegisterLayer` /
    /// `CloudShadowSamplePack`). Distinct from `ShadowAtlasUpdate`
    /// because cloud shadows have their own typed transmittance
    /// pipeline. Lands on `ScheduleLane::RenderPrepare` — the
    /// project/filter passes run before direct lighting consumes
    /// the transmittance, same lane as the other typed shadow
    /// setup work.
    CloudShadowUpdate = 9,
}

impl PageWorkClass {
    /// Return the canonical stable label.
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::VirtualGeometryPageResolve => "virtual_geometry_page_resolve",
            Self::VirtualGeometryPageUpload => "virtual_geometry_page_upload",
            Self::VirtualShadowPageUpdate => "virtual_shadow_page_update",
            Self::ShadowAtlasUpdate => "shadow_atlas_update",
            Self::GiProbeCacheUpdate => "gi_probe_cache_update",
            Self::RadianceSurfaceProbeCacheRefresh => "radiance_surface_probe_cache_refresh",
            Self::TextureStreamingPageUpload => "texture_streaming_page_upload",
            Self::MeshletClusterPreparation => "meshlet_cluster_preparation",
            Self::ReadbackDiagnosticSampling => "readback_diagnostic_sampling",
            Self::CloudShadowUpdate => "cloud_shadow_update",
        }
    }

    /// Every variant in declaration order.
    #[inline]
    pub const fn all() -> &'static [Self] {
        &[
            Self::VirtualGeometryPageResolve,
            Self::VirtualGeometryPageUpload,
            Self::VirtualShadowPageUpdate,
            Self::ShadowAtlasUpdate,
            Self::GiProbeCacheUpdate,
            Self::RadianceSurfaceProbeCacheRefresh,
            Self::TextureStreamingPageUpload,
            Self::MeshletClusterPreparation,
            Self::ReadbackDiagnosticSampling,
            Self::CloudShadowUpdate,
        ]
    }

    /// Map this work class to an FS-7 [`ScheduleLane`]. Background
    /// warmup classes land on `ResourceBackground` /
    /// `IdlePrefetch`; readback diagnostics on
    /// `DiagnosticsLowPriority`; everything else on
    /// `RenderPrepare` (CPU-side prep that gates render).
    ///
    /// The mapping is intentional: **background warmup classes
    /// never land on `RenderRecord` / `RenderSubmit` /
    /// `RenderPresent`** — the present-critical lanes are reserved
    /// for the render-graph itself, not page work.
    #[inline]
    pub const fn schedule_lane(self) -> ScheduleLane {
        match self {
            Self::VirtualGeometryPageResolve
            | Self::VirtualShadowPageUpdate
            | Self::ShadowAtlasUpdate
            | Self::GiProbeCacheUpdate
            | Self::MeshletClusterPreparation
            | Self::CloudShadowUpdate => ScheduleLane::RenderPrepare,
            Self::VirtualGeometryPageUpload
            | Self::RadianceSurfaceProbeCacheRefresh
            | Self::TextureStreamingPageUpload => ScheduleLane::ResourceBackground,
            Self::ReadbackDiagnosticSampling => ScheduleLane::DiagnosticsLowPriority,
        }
    }
}

// ============================================================================
// PagePriority — renderer-owned
// ============================================================================

/// FS-11 page-request priority. **Renderer policy owns priority**;
/// the scheduler never overrides the base priority set by the
/// renderer's policy layer.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Ord, PartialOrd)]
#[repr(u8)]
pub enum PagePriority {
    /// Background prewarm. Always lowest.
    Background = 0,
    /// Speculative — likely visible soon but not this frame.
    Low = 1,
    /// Visible this frame at low priority.
    Normal = 2,
    /// Visible this frame at high priority.
    High = 3,
    /// Present-path critical. Cannot be deferred or cancelled.
    Critical = 4,
}

impl PagePriority {
    /// Return the canonical stable label.
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

// ============================================================================
// PageRequest — one page-work request
// ============================================================================

/// FS-11 typed page request.
///
/// Constructed by the renderer's policy layer (page-priority
/// predictor, virtual-resource feedback consumer, texture streaming
/// budget owner) and handed to the [`PageScheduler`]'s `admit`
/// method.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct PageRequest {
    /// Stable page identifier (opaque to the scheduler).
    pub page_id: u64,
    /// Typed work class.
    pub class: PageWorkClass,
    /// View / camera generation. Stale generations cancel at
    /// admission with `PageCancelReason::ViewGenerationStale`.
    pub view_generation: u64,
    /// Renderer-supplied base priority. Never modified by the
    /// scheduler.
    pub priority: PagePriority,
    /// Estimated upload bytes.
    pub estimated_upload_bytes: u64,
    /// Estimated CPU-prep microseconds.
    pub estimated_cpu_us: u32,
    /// Tick the request was submitted at. Used for queue-wait
    /// metrics and priority aging.
    pub submitted_tick: u64,
}

// ============================================================================
// PageSchedulerBudget — per-frame budget envelope
// ============================================================================

/// FS-11 per-frame page-scheduler budget.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct PageSchedulerBudget {
    /// Maximum upload bytes per frame.
    pub upload_bytes_per_frame: u64,
    /// Maximum admitted pages per frame.
    pub max_pages_per_frame: u32,
    /// Maximum CPU-prep microseconds per frame.
    pub cpu_prep_us_per_frame: u32,
    /// Maximum readback bytes per frame.
    pub readback_bytes_per_frame: u64,
}

impl Default for PageSchedulerBudget {
    /// Sensible defaults sized for a 1080p / 60fps workload. Production
    /// callers should override based on measured frame budget.
    fn default() -> Self {
        Self {
            upload_bytes_per_frame: 16 * 1024 * 1024, // 16 MiB
            max_pages_per_frame: 256,
            cpu_prep_us_per_frame: 4_000,              // 4ms
            readback_bytes_per_frame: 4 * 1024 * 1024, // 4 MiB
        }
    }
}

// ============================================================================
// FramePressureState — caller-supplied tick state
// ============================================================================

/// FS-11 frame-pressure state supplied by the renderer (or wired
/// from the runtime's overload-state machine in a later pass).
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Ord, PartialOrd)]
#[repr(u8)]
pub enum FramePressureState {
    /// Normal frame — every priority admits up to budget.
    Normal = 0,
    /// Tight frame — `Background` / `Low` classes defer; `Normal`
    /// admits only within tight budget.
    Tight = 1,
    /// Critical frame — only `Critical` and `High` admit; everything
    /// else defers.
    Critical = 2,
}

impl FramePressureState {
    /// Return the canonical stable label.
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Tight => "tight",
            Self::Critical => "critical",
        }
    }
}

// ============================================================================
// PageCancelReason and PageRejectReason
// ============================================================================

/// FS-11 typed cancellation reason for a page request that the
/// scheduler decided to drop.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[non_exhaustive]
pub enum PageCancelReason {
    /// Request's `view_generation` is older than the scheduler's
    /// current view generation.
    ViewGenerationStale,
    /// A newer request for the same `page_id` arrived; this older
    /// request was superseded.
    SupersededByLater,
    /// The page's owner was unmounted (the renderer's policy layer
    /// signalled cancellation).
    OwnerUnmounted,
}

impl PageCancelReason {
    /// Return the canonical stable label.
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ViewGenerationStale => "view_generation_stale",
            Self::SupersededByLater => "superseded_by_later",
            Self::OwnerUnmounted => "owner_unmounted",
        }
    }
}

/// FS-11 typed reject reason for a page request that admission
/// declined.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[non_exhaustive]
pub enum PageRejectReason {
    /// Frame pressure prevented admission.
    FramePressure,
    /// Upload-byte budget for this frame is exhausted.
    UploadBudgetExhausted,
    /// Per-frame page cap is exhausted.
    PageCapExhausted,
    /// CPU-prep microsecond budget is exhausted.
    CpuPrepBudgetExhausted,
    /// Page-scheduler pending queue is at capacity.
    QueueFull,
}

impl PageRejectReason {
    /// Return the canonical stable label.
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::FramePressure => "frame_pressure",
            Self::UploadBudgetExhausted => "upload_budget_exhausted",
            Self::PageCapExhausted => "page_cap_exhausted",
            Self::CpuPrepBudgetExhausted => "cpu_prep_budget_exhausted",
            Self::QueueFull => "queue_full",
        }
    }
}

// ============================================================================
// PageAdmissionDecision
// ============================================================================

/// FS-11 typed admission decision for one [`PageRequest`].
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[non_exhaustive]
pub enum PageAdmissionDecision {
    /// Request admitted and queued for the current frame.
    Admit,
    /// Request deferred to the next frame at the same priority.
    Defer { tick_delay: u32 },
    /// Request coalesced into an existing pending request for the
    /// same `page_id`. The carried `superseded_page_id` is the
    /// `page_id` of the older request the scheduler kept; the new
    /// request was either dropped (older generation) or replaced
    /// the older one (newer generation, same page_id).
    Coalesce { superseded_page_id: u64 },
    /// Request cancelled with a typed reason.
    Cancel(PageCancelReason),
    /// Request rejected with a typed reason.
    Reject(PageRejectReason),
}

impl PageAdmissionDecision {
    /// Return the canonical stable label.
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Admit => "admit",
            Self::Defer { .. } => "defer",
            Self::Coalesce { .. } => "coalesce",
            Self::Cancel(_) => "cancel",
            Self::Reject(_) => "reject",
        }
    }
}

// ============================================================================
// PageSchedulerReport
// ============================================================================

/// FS-11 page-scheduler telemetry report.
///
/// Aggregates counts + latency percentiles for one tick or one
/// frame, depending on caller cadence. Latency percentiles are
/// computed from the last 256 samples by the
/// `queue_wait_*_ns` / `upload_prep_*_ns` accessors.
#[derive(Clone, Debug, Default)]
pub struct PageSchedulerReport {
    /// Number of requests admitted this period.
    pub admitted_count: u32,
    /// Number of requests deferred to a later tick.
    pub deferred_count: u32,
    /// Number of requests cancelled due to stale view generation.
    pub cancelled_by_generation: u32,
    /// Number of requests cancelled by being superseded.
    pub cancelled_by_supersede: u32,
    /// Number of requests cancelled due to owner unmount.
    pub cancelled_by_owner_unmount: u32,
    /// Number of requests coalesced into an existing pending entry.
    pub coalesced_count: u32,
    /// Number of requests rejected by admission.
    pub rejected_count: u32,
    /// Total upload bytes admitted this period.
    pub upload_bytes_admitted: u64,
    /// Total CPU-prep microseconds admitted this period.
    pub cpu_prep_us_admitted: u32,
    /// Queue-wait latency samples (last 256 admissions).
    queue_wait_samples: SampleRing,
    /// Upload-prep latency samples (last 256 admissions).
    upload_prep_samples: SampleRing,
}

impl PageSchedulerReport {
    /// p50 queue-wait latency in nanoseconds.
    #[must_use]
    pub fn queue_wait_p50_ns(&self) -> u64 {
        self.queue_wait_samples.percentile_ns(50)
    }

    /// p95 queue-wait latency in nanoseconds.
    #[must_use]
    pub fn queue_wait_p95_ns(&self) -> u64 {
        self.queue_wait_samples.percentile_ns(95)
    }

    /// p99 queue-wait latency in nanoseconds.
    #[must_use]
    pub fn queue_wait_p99_ns(&self) -> u64 {
        self.queue_wait_samples.percentile_ns(99)
    }

    /// p50 upload-prep latency in nanoseconds.
    #[must_use]
    pub fn upload_prep_p50_ns(&self) -> u64 {
        self.upload_prep_samples.percentile_ns(50)
    }

    /// p95 upload-prep latency in nanoseconds.
    #[must_use]
    pub fn upload_prep_p95_ns(&self) -> u64 {
        self.upload_prep_samples.percentile_ns(95)
    }

    /// p99 upload-prep latency in nanoseconds.
    #[must_use]
    pub fn upload_prep_p99_ns(&self) -> u64 {
        self.upload_prep_samples.percentile_ns(99)
    }

    /// Record one queue-wait sample.
    pub fn record_queue_wait_ns(&mut self, ns: u64) {
        self.queue_wait_samples.push(ns);
    }

    /// Record one upload-prep sample.
    pub fn record_upload_prep_ns(&mut self, ns: u64) {
        self.upload_prep_samples.push(ns);
    }
}

// ============================================================================
// SampleRing — fixed-size latency sample buffer
// ============================================================================

/// Fixed-size ring buffer of up to 256 latency samples in
/// nanoseconds.
#[derive(Clone, Debug)]
struct SampleRing {
    samples: [u64; 256],
    next: usize,
    count: u32,
}

impl Default for SampleRing {
    fn default() -> Self {
        Self {
            samples: [0u64; 256],
            next: 0,
            count: 0,
        }
    }
}

impl SampleRing {
    fn push(&mut self, sample: u64) {
        self.samples[self.next] = sample;
        self.next = (self.next + 1) % 256;
        if self.count < 256 {
            self.count += 1;
        }
    }

    /// Return the `percent`th percentile sample in nanoseconds.
    /// `percent` is clamped to `0..=100`. Returns `0` for an empty
    /// buffer.
    fn percentile_ns(&self, percent: u32) -> u64 {
        if self.count == 0 {
            return 0;
        }
        let n = (self.count as usize).min(256);
        let mut sorted = [0u64; 256];
        sorted[..n].copy_from_slice(&self.samples[..n]);
        let slice = &mut sorted[..n];
        slice.sort_unstable();
        let idx = ((n - 1).saturating_mul(percent.min(100) as usize)) / 100;
        slice[idx]
    }
}

// ============================================================================
// PageScheduler
// ============================================================================

/// FS-11 page scheduler.
///
/// Owns the pending-page queue, the per-frame budget envelope, the
/// current view generation, and the typed
/// [`PageSchedulerReport`]. The renderer's policy layer calls
/// `admit` to submit each [`PageRequest`]; once per frame it calls
/// `drain_for_frame` to receive the admitted requests in priority
/// order; once per camera/view change it calls
/// `advance_view_generation` to invalidate stale pending work.
pub struct PageScheduler {
    pending: Vec<PendingPage>,
    budget: PageSchedulerBudget,
    current_view_generation: u64,
    bytes_used_this_frame: u64,
    pages_admitted_this_frame: u32,
    cpu_us_used_this_frame: u32,
    /// Maximum pending queue depth before `QueueFull` rejection.
    queue_cap: usize,
    report: PageSchedulerReport,
    /// Monotonic tick counter incremented on each admission. Used
    /// for priority aging.
    current_tick: u64,
}

/// One pending [`PageRequest`] with scheduler-owned aging state.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
struct PendingPage {
    request: PageRequest,
    /// Age in ticks since submission. Drives priority aging without
    /// modifying the request's base `priority`.
    age_ticks: u32,
}

impl PendingPage {
    /// Effective priority = base priority + age aging.
    ///
    /// Aging promotes a request by one priority band every 4 ticks
    /// it spends pending (capped at `Critical`). The renderer's
    /// base priority is preserved on `request.priority`; this
    /// accessor returns the *effective* priority for scheduling
    /// decisions only.
    #[inline]
    const fn effective_priority(&self) -> PagePriority {
        let base = self.request.priority as u8;
        let raw_bonus = self.age_ticks / 4;
        // `u32::min`/`u8::min` are not const-stable on Rust 1.97;
        // use plain comparisons.
        let bonus = if raw_bonus < 4 { raw_bonus as u8 } else { 4 };
        let promoted_pre = base.saturating_add(bonus);
        let promoted = if promoted_pre < 4 { promoted_pre } else { 4 };
        match promoted {
            0 => PagePriority::Background,
            1 => PagePriority::Low,
            2 => PagePriority::Normal,
            3 => PagePriority::High,
            _ => PagePriority::Critical,
        }
    }
}

impl PageScheduler {
    /// Construct a new scheduler with the supplied budget envelope
    /// and initial view generation. The pending queue cap defaults
    /// to 4096 entries.
    #[must_use]
    pub fn new(budget: PageSchedulerBudget, initial_view_generation: u64) -> Self {
        Self {
            pending: Vec::with_capacity(256),
            budget,
            current_view_generation: initial_view_generation,
            bytes_used_this_frame: 0,
            pages_admitted_this_frame: 0,
            cpu_us_used_this_frame: 0,
            queue_cap: 4096,
            report: PageSchedulerReport::default(),
            current_tick: 0,
        }
    }

    /// Override the pending queue capacity.
    #[must_use]
    pub fn with_queue_cap(mut self, cap: usize) -> Self {
        self.queue_cap = cap.max(1);
        self
    }

    /// Return the current view generation.
    #[inline]
    pub fn view_generation(&self) -> u64 {
        self.current_view_generation
    }

    /// Return a reference to the current report.
    #[inline]
    pub fn report(&self) -> &PageSchedulerReport {
        &self.report
    }

    /// Return a mutable reference to the report (for `record_*`
    /// calls).
    #[inline]
    pub fn report_mut(&mut self) -> &mut PageSchedulerReport {
        &mut self.report
    }

    /// Current pending-queue depth.
    #[inline]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Advance the view generation. Pending requests with older
    /// generations are cancelled with
    /// [`PageCancelReason::ViewGenerationStale`] and counted in the
    /// report.
    pub fn advance_view_generation(&mut self, new_generation: u64) {
        if new_generation <= self.current_view_generation {
            return;
        }
        self.current_view_generation = new_generation;
        let mut cancelled: u32 = 0;
        self.pending.retain(|p| {
            if p.request.view_generation < new_generation {
                cancelled = cancelled.saturating_add(1);
                false
            } else {
                true
            }
        });
        self.report.cancelled_by_generation = self
            .report
            .cancelled_by_generation
            .saturating_add(cancelled);
    }

    /// Cancel every pending request matching `page_id` (typically
    /// called when the owning component unmounts).
    pub fn cancel_owner(&mut self, page_id: u64) {
        let mut cancelled: u32 = 0;
        self.pending.retain(|p| {
            if p.request.page_id == page_id {
                cancelled = cancelled.saturating_add(1);
                false
            } else {
                true
            }
        });
        self.report.cancelled_by_owner_unmount = self
            .report
            .cancelled_by_owner_unmount
            .saturating_add(cancelled);
    }

    /// Reset per-frame budget counters. Call at the start of each
    /// frame; the pending queue persists across frames.
    pub fn begin_frame(&mut self) {
        self.bytes_used_this_frame = 0;
        self.pages_admitted_this_frame = 0;
        self.cpu_us_used_this_frame = 0;
    }

    /// Admit (or refuse) a page request.
    ///
    /// Evaluation order:
    ///
    /// 1. Stale view generation → `Cancel(ViewGenerationStale)`.
    /// 2. Critical pressure + non-Critical/non-High priority →
    ///    `Defer { tick_delay: 1 }`.
    /// 3. Tight pressure + Background/Low priority →
    ///    `Defer { tick_delay: 1 }`.
    /// 4. Existing pending entry with same `page_id`:
    ///    - Older generation → new request supersedes; existing
    ///      cancelled (`SupersededByLater`); the new entry replaces
    ///      it; return `Coalesce`.
    ///    - Newer generation → the new request is stale; cancel it
    ///      (`SupersededByLater`); return `Coalesce`.
    ///    - Same generation → keep the existing; return `Coalesce`.
    /// 5. Upload-byte / page-cap / CPU-prep budget exhaustion →
    ///    `Reject(...)`.
    /// 6. Queue cap reached → `Reject(QueueFull)`.
    /// 7. Otherwise → `Admit` and enqueue.
    pub fn admit(
        &mut self,
        request: PageRequest,
        pressure: FramePressureState,
    ) -> PageAdmissionDecision {
        self.current_tick = self.current_tick.saturating_add(1);

        // 1. Stale view generation.
        if request.view_generation < self.current_view_generation {
            self.report.cancelled_by_generation =
                self.report.cancelled_by_generation.saturating_add(1);
            return PageAdmissionDecision::Cancel(PageCancelReason::ViewGenerationStale);
        }

        // 2-3. Frame pressure shedding.
        match pressure {
            FramePressureState::Critical => {
                if !matches!(
                    request.priority,
                    PagePriority::Critical | PagePriority::High
                ) {
                    self.report.deferred_count = self.report.deferred_count.saturating_add(1);
                    return PageAdmissionDecision::Defer { tick_delay: 1 };
                }
            }
            FramePressureState::Tight => {
                if matches!(
                    request.priority,
                    PagePriority::Background | PagePriority::Low
                ) {
                    self.report.deferred_count = self.report.deferred_count.saturating_add(1);
                    return PageAdmissionDecision::Defer { tick_delay: 1 };
                }
            }
            FramePressureState::Normal => {}
        }

        // 4. Coalesce by page_id.
        if let Some(existing_idx) = self
            .pending
            .iter()
            .position(|p| p.request.page_id == request.page_id)
        {
            let existing_gen = self.pending[existing_idx].request.view_generation;
            if request.view_generation > existing_gen {
                // New request supersedes the existing entry.
                self.pending[existing_idx] = PendingPage {
                    request,
                    age_ticks: 0,
                };
                self.report.cancelled_by_supersede =
                    self.report.cancelled_by_supersede.saturating_add(1);
                self.report.coalesced_count = self.report.coalesced_count.saturating_add(1);
                return PageAdmissionDecision::Coalesce {
                    superseded_page_id: request.page_id,
                };
            }
            // Same or older generation: drop the new request as
            // superseded.
            self.report.coalesced_count = self.report.coalesced_count.saturating_add(1);
            return PageAdmissionDecision::Coalesce {
                superseded_page_id: request.page_id,
            };
        }

        // 5. Budget exhaustion.
        let next_bytes = self
            .bytes_used_this_frame
            .saturating_add(request.estimated_upload_bytes);
        if next_bytes > self.budget.upload_bytes_per_frame {
            self.report.rejected_count = self.report.rejected_count.saturating_add(1);
            return PageAdmissionDecision::Reject(PageRejectReason::UploadBudgetExhausted);
        }
        let next_pages = self.pages_admitted_this_frame.saturating_add(1);
        if next_pages > self.budget.max_pages_per_frame {
            self.report.rejected_count = self.report.rejected_count.saturating_add(1);
            return PageAdmissionDecision::Reject(PageRejectReason::PageCapExhausted);
        }
        let next_cpu = self
            .cpu_us_used_this_frame
            .saturating_add(request.estimated_cpu_us);
        if next_cpu > self.budget.cpu_prep_us_per_frame {
            self.report.rejected_count = self.report.rejected_count.saturating_add(1);
            return PageAdmissionDecision::Reject(PageRejectReason::CpuPrepBudgetExhausted);
        }

        // 6. Queue cap.
        if self.pending.len() >= self.queue_cap {
            self.report.rejected_count = self.report.rejected_count.saturating_add(1);
            return PageAdmissionDecision::Reject(PageRejectReason::QueueFull);
        }

        // 7. Admit.
        self.bytes_used_this_frame = next_bytes;
        self.pages_admitted_this_frame = next_pages;
        self.cpu_us_used_this_frame = next_cpu;
        self.report.admitted_count = self.report.admitted_count.saturating_add(1);
        self.report.upload_bytes_admitted = self
            .report
            .upload_bytes_admitted
            .saturating_add(request.estimated_upload_bytes);
        self.report.cpu_prep_us_admitted = self
            .report
            .cpu_prep_us_admitted
            .saturating_add(request.estimated_cpu_us);
        self.pending.push(PendingPage {
            request,
            age_ticks: 0,
        });
        PageAdmissionDecision::Admit
    }

    /// Drain pending requests for the current frame in
    /// effective-priority order (highest first; aging promotes
    /// long-waiting requests). Each call also ages every remaining
    /// entry by one tick.
    pub fn drain_for_frame(&mut self) -> Vec<PageRequest> {
        // Sort descending by effective priority. Stable sort
        // preserves submission order within a priority band.
        self.pending.sort_by(|a, b| {
            b.effective_priority()
                .cmp(&a.effective_priority())
                .then_with(|| a.request.submitted_tick.cmp(&b.request.submitted_tick))
        });
        let drained: Vec<PageRequest> = self.pending.iter().map(|p| p.request).collect();
        self.pending.clear();
        drained
    }

    /// Age every pending request by one tick. Called by
    /// schedulers that retain pending requests across frames.
    pub fn age_pending(&mut self) {
        for p in &mut self.pending {
            p.age_ticks = p.age_ticks.saturating_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn basic_request(page_id: u64, view_gen: u64, priority: PagePriority) -> PageRequest {
        PageRequest {
            page_id,
            class: PageWorkClass::VirtualGeometryPageUpload,
            view_generation: view_gen,
            priority,
            estimated_upload_bytes: 1024,
            estimated_cpu_us: 100,
            submitted_tick: 0,
        }
    }

    #[test]
    fn page_work_class_labels_are_unique_and_non_empty() {
        let labels: Vec<&'static str> = PageWorkClass::all().iter().map(|c| c.label()).collect();
        // FS-M2.2 added the `CloudShadowUpdate` variant for the
        // typed cloud-shadow refresh pipeline; the count is now 10.
        assert_eq!(labels.len(), 10);
        for label in &labels {
            assert!(!label.is_empty());
        }
        for i in 0..labels.len() {
            for j in (i + 1)..labels.len() {
                assert_ne!(labels[i], labels[j]);
            }
        }
    }

    #[test]
    fn cloud_shadow_update_routes_to_render_prepare() {
        // FS-M2.2: the new `CloudShadowUpdate` variant must land on
        // `RenderPrepare` (the cloud-shadow project/filter passes
        // run before direct lighting consumes the transmittance,
        // same lane as other typed shadow setup work). Background
        // warmup never preempts present.
        assert_eq!(
            PageWorkClass::CloudShadowUpdate.schedule_lane(),
            ScheduleLane::RenderPrepare,
        );
        assert_eq!(PageWorkClass::CloudShadowUpdate.label(), "cloud_shadow_update");
    }

    #[test]
    fn page_work_classes_route_to_correct_lanes() {
        // FS-11 scheduling rule: background warmup classes never
        // land on render-record / submit / present lanes.
        for class in PageWorkClass::all() {
            let lane = class.schedule_lane();
            assert_ne!(lane, ScheduleLane::RenderRecord);
            assert_ne!(lane, ScheduleLane::RenderSubmit);
            assert_ne!(lane, ScheduleLane::RenderPresent);
            assert_ne!(lane, ScheduleLane::FrameCritical);
        }
        // Spot-check a few mappings.
        assert_eq!(
            PageWorkClass::VirtualGeometryPageUpload.schedule_lane(),
            ScheduleLane::ResourceBackground,
        );
        assert_eq!(
            PageWorkClass::VirtualShadowPageUpdate.schedule_lane(),
            ScheduleLane::RenderPrepare,
        );
        assert_eq!(
            PageWorkClass::ReadbackDiagnosticSampling.schedule_lane(),
            ScheduleLane::DiagnosticsLowPriority,
        );
    }

    #[test]
    fn generation_invalidation_cancels_stale_pending_work() {
        // FS-11 exit gate: stale camera/view page work cancels
        // cleanly.
        let mut sched = PageScheduler::new(PageSchedulerBudget::default(), 1);
        sched.admit(
            basic_request(1, 1, PagePriority::Normal),
            FramePressureState::Normal,
        );
        sched.admit(
            basic_request(2, 1, PagePriority::Normal),
            FramePressureState::Normal,
        );
        assert_eq!(sched.pending_count(), 2);
        sched.advance_view_generation(2);
        // Both entries had generation=1 < 2; both cancelled.
        assert_eq!(sched.pending_count(), 0);
        assert_eq!(sched.report().cancelled_by_generation, 2);
    }

    #[test]
    fn stale_admission_returns_cancel_view_generation() {
        let mut sched = PageScheduler::new(PageSchedulerBudget::default(), 5);
        let decision = sched.admit(
            basic_request(7, 4, PagePriority::Normal),
            FramePressureState::Normal,
        );
        match decision {
            PageAdmissionDecision::Cancel(PageCancelReason::ViewGenerationStale) => {}
            other => panic!("expected Cancel(ViewGenerationStale), got {other:?}"),
        }
        assert_eq!(sched.report().cancelled_by_generation, 1);
    }

    #[test]
    fn newer_generation_supersedes_older_pending_entry() {
        let mut sched = PageScheduler::new(PageSchedulerBudget::default(), 1);
        sched.admit(
            basic_request(42, 1, PagePriority::Normal),
            FramePressureState::Normal,
        );
        // Newer generation arrives for the SAME page_id.
        let decision = sched.admit(
            basic_request(42, 2, PagePriority::High),
            FramePressureState::Normal,
        );
        match decision {
            PageAdmissionDecision::Coalesce { superseded_page_id } => {
                assert_eq!(superseded_page_id, 42);
            }
            other => panic!("expected Coalesce, got {other:?}"),
        }
        assert_eq!(sched.pending_count(), 1);
        assert_eq!(sched.report().cancelled_by_supersede, 1);
        assert_eq!(sched.report().coalesced_count, 1);
    }

    #[test]
    fn critical_pressure_defers_non_critical_high_work() {
        // FS-11 scheduling rule: background warmup cannot preempt
        // present/submit/record under pressure.
        let mut sched = PageScheduler::new(PageSchedulerBudget::default(), 1);
        // Background under critical pressure: deferred.
        let decision = sched.admit(
            basic_request(1, 1, PagePriority::Background),
            FramePressureState::Critical,
        );
        assert!(matches!(decision, PageAdmissionDecision::Defer { .. }));
        // Normal under critical pressure: deferred.
        let decision = sched.admit(
            basic_request(2, 1, PagePriority::Normal),
            FramePressureState::Critical,
        );
        assert!(matches!(decision, PageAdmissionDecision::Defer { .. }));
        // High under critical pressure: admitted.
        let decision = sched.admit(
            basic_request(3, 1, PagePriority::High),
            FramePressureState::Critical,
        );
        assert_eq!(decision, PageAdmissionDecision::Admit);
        // Critical under critical pressure: admitted.
        let decision = sched.admit(
            basic_request(4, 1, PagePriority::Critical),
            FramePressureState::Critical,
        );
        assert_eq!(decision, PageAdmissionDecision::Admit);
    }

    #[test]
    fn tight_pressure_defers_background_low_only() {
        let mut sched = PageScheduler::new(PageSchedulerBudget::default(), 1);
        // Background under tight: deferred.
        let decision = sched.admit(
            basic_request(1, 1, PagePriority::Background),
            FramePressureState::Tight,
        );
        assert!(matches!(decision, PageAdmissionDecision::Defer { .. }));
        // Low under tight: deferred.
        let decision = sched.admit(
            basic_request(2, 1, PagePriority::Low),
            FramePressureState::Tight,
        );
        assert!(matches!(decision, PageAdmissionDecision::Defer { .. }));
        // Normal under tight: admitted.
        let decision = sched.admit(
            basic_request(3, 1, PagePriority::Normal),
            FramePressureState::Tight,
        );
        assert_eq!(decision, PageAdmissionDecision::Admit);
    }

    #[test]
    fn upload_budget_exhaustion_rejects_with_typed_reason() {
        let budget = PageSchedulerBudget {
            upload_bytes_per_frame: 2048,
            max_pages_per_frame: 100,
            cpu_prep_us_per_frame: 1_000_000,
            readback_bytes_per_frame: u64::MAX,
        };
        let mut sched = PageScheduler::new(budget, 1);
        // First 2 requests of 1024 bytes fit.
        assert_eq!(
            sched.admit(
                basic_request(1, 1, PagePriority::Normal),
                FramePressureState::Normal,
            ),
            PageAdmissionDecision::Admit,
        );
        assert_eq!(
            sched.admit(
                basic_request(2, 1, PagePriority::Normal),
                FramePressureState::Normal,
            ),
            PageAdmissionDecision::Admit,
        );
        // Third overflows the 2048-byte budget.
        let decision = sched.admit(
            basic_request(3, 1, PagePriority::Normal),
            FramePressureState::Normal,
        );
        assert_eq!(
            decision,
            PageAdmissionDecision::Reject(PageRejectReason::UploadBudgetExhausted),
        );
        assert_eq!(sched.report().rejected_count, 1);
    }

    #[test]
    fn page_cap_exhaustion_rejects() {
        let budget = PageSchedulerBudget {
            upload_bytes_per_frame: u64::MAX,
            max_pages_per_frame: 2,
            cpu_prep_us_per_frame: 1_000_000,
            readback_bytes_per_frame: u64::MAX,
        };
        let mut sched = PageScheduler::new(budget, 1);
        sched.admit(
            basic_request(1, 1, PagePriority::Normal),
            FramePressureState::Normal,
        );
        sched.admit(
            basic_request(2, 1, PagePriority::Normal),
            FramePressureState::Normal,
        );
        let decision = sched.admit(
            basic_request(3, 1, PagePriority::Normal),
            FramePressureState::Normal,
        );
        assert_eq!(
            decision,
            PageAdmissionDecision::Reject(PageRejectReason::PageCapExhausted),
        );
    }

    #[test]
    fn drain_returns_requests_in_effective_priority_order() {
        let mut sched = PageScheduler::new(PageSchedulerBudget::default(), 1);
        // Submit in order: Background, Critical, Normal, High.
        sched.admit(
            basic_request(1, 1, PagePriority::Background),
            FramePressureState::Normal,
        );
        sched.admit(
            basic_request(2, 1, PagePriority::Critical),
            FramePressureState::Normal,
        );
        sched.admit(
            basic_request(3, 1, PagePriority::Normal),
            FramePressureState::Normal,
        );
        sched.admit(
            basic_request(4, 1, PagePriority::High),
            FramePressureState::Normal,
        );
        let drained = sched.drain_for_frame();
        let ids: Vec<u64> = drained.iter().map(|r| r.page_id).collect();
        // Critical → High → Normal → Background.
        assert_eq!(ids, vec![2, 4, 3, 1]);
    }

    #[test]
    fn priority_aging_promotes_long_waiting_requests() {
        // FS-11: priority aging. Base priority Background, after
        // 16 ticks of aging the effective priority promotes by
        // 4 bands → Critical.
        let mut sched = PageScheduler::new(PageSchedulerBudget::default(), 1);
        sched.admit(
            basic_request(1, 1, PagePriority::Background),
            FramePressureState::Normal,
        );
        for _ in 0..16 {
            sched.age_pending();
        }
        // Now add a fresh Background; the aged one should drain
        // first.
        sched.admit(
            basic_request(2, 1, PagePriority::Background),
            FramePressureState::Normal,
        );
        let drained = sched.drain_for_frame();
        let ids: Vec<u64> = drained.iter().map(|r| r.page_id).collect();
        assert_eq!(ids[0], 1, "aged background should outrank fresh background");
        // Base priority on the request itself is preserved (renderer
        // owns priority).
        assert_eq!(drained[0].priority, PagePriority::Background);
    }

    #[test]
    fn renderer_priority_is_never_overwritten() {
        // FS-11 exit gate: renderer policy remains owner of
        // priority. The scheduler ages effective priority but
        // never mutates `request.priority` on the typed request.
        let mut sched = PageScheduler::new(PageSchedulerBudget::default(), 1);
        let original = basic_request(99, 1, PagePriority::Low);
        sched.admit(original, FramePressureState::Normal);
        for _ in 0..100 {
            sched.age_pending();
        }
        let drained = sched.drain_for_frame();
        assert_eq!(drained.len(), 1);
        assert_eq!(
            drained[0].priority,
            PagePriority::Low,
            "renderer-supplied priority must be preserved verbatim",
        );
    }

    #[test]
    fn cancel_owner_drops_matching_page_requests() {
        let mut sched = PageScheduler::new(PageSchedulerBudget::default(), 1);
        sched.admit(
            basic_request(7, 1, PagePriority::Normal),
            FramePressureState::Normal,
        );
        sched.admit(
            basic_request(8, 1, PagePriority::Normal),
            FramePressureState::Normal,
        );
        sched.cancel_owner(7);
        assert_eq!(sched.pending_count(), 1);
        assert_eq!(sched.report().cancelled_by_owner_unmount, 1);
        let drained = sched.drain_for_frame();
        assert_eq!(drained[0].page_id, 8);
    }

    #[test]
    fn percentile_metrics_compute_from_sample_ring() {
        let mut sched = PageScheduler::new(PageSchedulerBudget::default(), 1);
        // Push samples 1..=100. p50 should be ~50ns, p95 ~95ns,
        // p99 ~99ns.
        for i in 1..=100u64 {
            sched.report_mut().record_queue_wait_ns(i);
        }
        let p50 = sched.report().queue_wait_p50_ns();
        let p95 = sched.report().queue_wait_p95_ns();
        let p99 = sched.report().queue_wait_p99_ns();
        // Integer percentile of sorted 1..=100 (n=100):
        //   idx_p = (100 - 1) * p / 100 = 99 * p / 100
        //   p50: idx=49 → sorted[49] = 50
        //   p95: idx=94 → sorted[94] = 95
        //   p99: idx=98 → sorted[98] = 99
        assert_eq!(p50, 50);
        assert_eq!(p95, 95);
        assert_eq!(p99, 99);
    }

    #[test]
    fn percentile_metrics_handle_empty_buffer() {
        let sched = PageScheduler::new(PageSchedulerBudget::default(), 1);
        assert_eq!(sched.report().queue_wait_p50_ns(), 0);
        assert_eq!(sched.report().queue_wait_p95_ns(), 0);
        assert_eq!(sched.report().queue_wait_p99_ns(), 0);
        assert_eq!(sched.report().upload_prep_p50_ns(), 0);
        assert_eq!(sched.report().upload_prep_p95_ns(), 0);
        assert_eq!(sched.report().upload_prep_p99_ns(), 0);
    }

    #[test]
    fn sample_ring_wraps_at_256() {
        let mut sched = PageScheduler::new(PageSchedulerBudget::default(), 1);
        for i in 0..300u64 {
            sched.report_mut().record_queue_wait_ns(i);
        }
        // After 300 pushes the ring holds the last 256 samples
        // (indices 44..299). Lowest sample is 44.
        let p50 = sched.report().queue_wait_p50_ns();
        // n=256, idx = 255 * 50 / 100 = 127, sorted[127] should be
        // 44 + 127 = 171.
        assert_eq!(p50, 171);
    }

    #[test]
    fn page_admission_decision_labels_are_unique() {
        let decisions = [
            PageAdmissionDecision::Admit,
            PageAdmissionDecision::Defer { tick_delay: 1 },
            PageAdmissionDecision::Coalesce {
                superseded_page_id: 0,
            },
            PageAdmissionDecision::Cancel(PageCancelReason::ViewGenerationStale),
            PageAdmissionDecision::Reject(PageRejectReason::QueueFull),
        ];
        let labels: Vec<&'static str> = decisions.iter().map(|d| d.label()).collect();
        for i in 0..labels.len() {
            for j in (i + 1)..labels.len() {
                assert_ne!(labels[i], labels[j]);
            }
        }
    }

    #[test]
    fn cancel_reject_reason_labels_are_unique_and_static() {
        for reason in [
            PageCancelReason::ViewGenerationStale,
            PageCancelReason::SupersededByLater,
            PageCancelReason::OwnerUnmounted,
        ] {
            let _: &'static str = reason.label();
            assert!(!reason.label().is_empty());
        }
        for reason in [
            PageRejectReason::FramePressure,
            PageRejectReason::UploadBudgetExhausted,
            PageRejectReason::PageCapExhausted,
            PageRejectReason::CpuPrepBudgetExhausted,
            PageRejectReason::QueueFull,
        ] {
            let _: &'static str = reason.label();
            assert!(!reason.label().is_empty());
        }
    }

    #[test]
    fn page_priority_ordering_is_critical_highest() {
        assert!(PagePriority::Critical > PagePriority::High);
        assert!(PagePriority::High > PagePriority::Normal);
        assert!(PagePriority::Normal > PagePriority::Low);
        assert!(PagePriority::Low > PagePriority::Background);
    }

    #[test]
    fn begin_frame_resets_per_frame_budget() {
        let budget = PageSchedulerBudget {
            upload_bytes_per_frame: 1024,
            max_pages_per_frame: 1,
            cpu_prep_us_per_frame: 1_000_000,
            readback_bytes_per_frame: u64::MAX,
        };
        let mut sched = PageScheduler::new(budget, 1);
        sched.admit(
            basic_request(1, 1, PagePriority::Normal),
            FramePressureState::Normal,
        );
        // Second admission this frame: rejected (page cap).
        let r = sched.admit(
            basic_request(2, 1, PagePriority::Normal),
            FramePressureState::Normal,
        );
        assert!(matches!(r, PageAdmissionDecision::Reject(_)));
        // Next frame resets the per-frame budget.
        sched.begin_frame();
        let r = sched.admit(
            basic_request(3, 1, PagePriority::Normal),
            FramePressureState::Normal,
        );
        assert_eq!(r, PageAdmissionDecision::Admit);
    }

    #[test]
    fn report_contains_no_backend_handles() {
        // FS-11 / FS-10 invariant: report fields are all
        // primitives or sample-ring internals; no backend handles
        // (wgpu / DX12 / Vulkan / Metal types).
        let report = PageSchedulerReport::default();
        let _: u32 = report.admitted_count;
        let _: u32 = report.deferred_count;
        let _: u32 = report.cancelled_by_generation;
        let _: u32 = report.cancelled_by_supersede;
        let _: u32 = report.cancelled_by_owner_unmount;
        let _: u32 = report.coalesced_count;
        let _: u32 = report.rejected_count;
        let _: u64 = report.upload_bytes_admitted;
        let _: u32 = report.cpu_prep_us_admitted;
    }
}
