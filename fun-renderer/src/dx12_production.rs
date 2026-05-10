//! Pass 26 — DX12 Production Hardening Through wgpu Bridge.
//!
//! Pass 16 installed the typed bridge health surface
//! (`Dx12NativeBackendHealth`, `Dx12ProductionGateError`,
//! `enforce_dx12_production_gate`). Pass 26 adds the measured
//! runtime gates layer that asserts no per-frame growth after
//! warmup, classifies gate violations, and ties everything into a
//! single typed `Dx12ProductionHardeningReport` artifact bundle.
//!
//! `Dx12WarmupState` is the typed state machine
//! (Cold / Warming / Warm / Production) so the runtime knows when
//! "after warmup" applies. `Dx12RuntimeGateCounters` is the Bevy
//! Resource that tracks cumulative shader translations, pipeline
//! creations, bind layout creations, resource creations, and
//! normal-frame blocking waits per warmup state. The renderer
//! never holds wgpu objects; the counters are typed numbers the
//! bridge increments before each event.
//!
//! `Dx12GateViolationKind` is the typed enum naming each
//! violation. The Pass 26 contract enforces
//! `ShaderTranslationAfterWarmup`, `PipelineCreationAfterWarmup`,
//! `BindLayoutCreationAfterWarmup`,
//! `PerFrameResourceCreationGrowth`, `NormalFrameBlockingWait`,
//! `BackendMismatch`, `DeviceLostUnhandled`, and
//! `NativeCommandListClaimedButUnavailable`.
//! `Dx12RuntimeGateReport` is the Bevy Resource that aggregates
//! the violation list per frame and per warmup state.
//! `Dx12NativeSdkClaimPolicy` is the typed scaffold that asserts
//! DLSS / native SDK claims remain blocked until either wgpu
//! exposes sanctioned command-list access or a direct DX12 backend
//! owns command recording. `Dx12ProductionHardeningReport` is the
//! top-level Bevy Resource that ties the bridge health surface,
//! runtime gate report, native SDK claim policy, and warmup state
//! into one typed artifact bundle.
//!
//! Design rules: the DX12 production path is fail-closed by
//! default — any typed gate violation is recorded and the operator
//! decides whether to crash or downgrade the renderer. Native
//! command list access stays unavailable until either (a) wgpu
//! exposes sanctioned command-list access, or (b) a direct DX12
//! backend owns command recording; DLSS / native SDK claims that
//! bypass the gate are recorded as
//! `NativeCommandListClaimedButUnavailable`. Backend mismatch is
//! a fatal startup violation — the `Dx12StrictStartupOutcome` enum
//! encodes it as `RejectedDueToBackendMismatch`.

use bevy_ecs::prelude::Resource;

use crate::backend::NativeBackend;

pub const DX12_PRODUCTION_SCHEMA_VERSION: u16 = 1;

pub const DX12_GATE_VIOLATION_KIND_COUNT: usize = 8;
pub const DX12_WARMUP_STATE_COUNT: usize = 4;
pub const DX12_RUNTIME_RESOURCE_KIND_COUNT: usize = 5;

// ============================================================================
// Section 1 — Warmup state machine
// ============================================================================

/// Typed warmup state. The renderer drives this through the per-frame
/// schedule by calling `advance_with_event`; the bridge does not
/// maintain its own warmup state — the renderer-side machine is
/// the single source of truth.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dx12WarmupState {
    #[default]
    Cold,
    Warming,
    Warm,
    Production,
}

impl Dx12WarmupState {
    pub const ALL: [Self; DX12_WARMUP_STATE_COUNT] =
        [Self::Cold, Self::Warming, Self::Warm, Self::Production];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Cold => 0,
            Self::Warming => 1,
            Self::Warm => 2,
            Self::Production => 3,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cold => "cold",
            Self::Warming => "warming",
            Self::Warm => "warm",
            Self::Production => "production",
        }
    }

    /// Returns true once the runtime is warm enough that the
    /// "no shader translation / no pipeline creation / no bind
    /// layout creation / no resource growth / no blocking wait"
    /// rules apply. Pre-Warm states are allowed to allocate
    /// freely — the gate is a *post-warmup* assertion.
    #[must_use]
    pub const fn enforces_no_growth(self) -> bool {
        matches!(self, Self::Warm | Self::Production)
    }

    #[must_use]
    pub const fn allows_warmup_creation(self) -> bool {
        matches!(self, Self::Cold | Self::Warming)
    }
}

/// Event the renderer signals to drive the warmup state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dx12WarmupEvent {
    InstanceInitialized,
    AdapterSelected,
    DeviceCreated,
    InitialShaderTranslationCompleted,
    InitialPipelineWarmupCompleted,
    InitialBindLayoutWarmupCompleted,
    InitialResourcesUploaded,
    FirstFramePresented,
    EnteredProductionLoop,
}

impl Dx12WarmupEvent {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InstanceInitialized => "instance_initialized",
            Self::AdapterSelected => "adapter_selected",
            Self::DeviceCreated => "device_created",
            Self::InitialShaderTranslationCompleted => "initial_shader_translation_completed",
            Self::InitialPipelineWarmupCompleted => "initial_pipeline_warmup_completed",
            Self::InitialBindLayoutWarmupCompleted => "initial_bind_layout_warmup_completed",
            Self::InitialResourcesUploaded => "initial_resources_uploaded",
            Self::FirstFramePresented => "first_frame_presented",
            Self::EnteredProductionLoop => "entered_production_loop",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Resource)]
pub struct Dx12WarmupTracker {
    pub schema_version: u16,
    pub state: Dx12WarmupState,
    pub instance_initialized: bool,
    pub adapter_selected: bool,
    pub device_created: bool,
    pub initial_shaders_translated: bool,
    pub initial_pipelines_warmed: bool,
    pub initial_bind_layouts_warmed: bool,
    pub initial_resources_uploaded: bool,
    pub first_frame_presented: bool,
    pub event_count: u32,
}

impl Dx12WarmupTracker {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            schema_version: DX12_PRODUCTION_SCHEMA_VERSION,
            state: Dx12WarmupState::Cold,
            instance_initialized: false,
            adapter_selected: false,
            device_created: false,
            initial_shaders_translated: false,
            initial_pipelines_warmed: false,
            initial_bind_layouts_warmed: false,
            initial_resources_uploaded: false,
            first_frame_presented: false,
            event_count: 0,
        }
    }

    pub fn advance_with_event(&mut self, event: Dx12WarmupEvent) -> Dx12WarmupState {
        self.event_count = self.event_count.saturating_add(1);
        match event {
            Dx12WarmupEvent::InstanceInitialized => self.instance_initialized = true,
            Dx12WarmupEvent::AdapterSelected => self.adapter_selected = true,
            Dx12WarmupEvent::DeviceCreated => self.device_created = true,
            Dx12WarmupEvent::InitialShaderTranslationCompleted => {
                self.initial_shaders_translated = true;
            }
            Dx12WarmupEvent::InitialPipelineWarmupCompleted => {
                self.initial_pipelines_warmed = true;
            }
            Dx12WarmupEvent::InitialBindLayoutWarmupCompleted => {
                self.initial_bind_layouts_warmed = true;
            }
            Dx12WarmupEvent::InitialResourcesUploaded => {
                self.initial_resources_uploaded = true;
            }
            Dx12WarmupEvent::FirstFramePresented => self.first_frame_presented = true,
            Dx12WarmupEvent::EnteredProductionLoop => {
                self.state = Dx12WarmupState::Production;
                return self.state;
            }
        }

        if self.first_frame_presented
            && self.initial_shaders_translated
            && self.initial_pipelines_warmed
            && self.initial_bind_layouts_warmed
            && self.initial_resources_uploaded
        {
            self.state = Dx12WarmupState::Warm;
        } else if self.device_created {
            self.state = Dx12WarmupState::Warming;
        } else {
            self.state = Dx12WarmupState::Cold;
        }
        self.state
    }

    #[must_use]
    pub const fn enforces_no_growth(&self) -> bool {
        self.state.enforces_no_growth()
    }

    #[must_use]
    pub fn fully_warmed(&self) -> bool {
        matches!(
            self.state,
            Dx12WarmupState::Warm | Dx12WarmupState::Production
        )
    }
}

// ============================================================================
// Section 2 — Measured runtime gate counters
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dx12RuntimeResourceKind {
    ShaderTranslation,
    PipelineCreation,
    BindLayoutCreation,
    ResourceCreation,
    NormalFrameBlockingWait,
}

impl Dx12RuntimeResourceKind {
    pub const ALL: [Self; DX12_RUNTIME_RESOURCE_KIND_COUNT] = [
        Self::ShaderTranslation,
        Self::PipelineCreation,
        Self::BindLayoutCreation,
        Self::ResourceCreation,
        Self::NormalFrameBlockingWait,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::ShaderTranslation => 0,
            Self::PipelineCreation => 1,
            Self::BindLayoutCreation => 2,
            Self::ResourceCreation => 3,
            Self::NormalFrameBlockingWait => 4,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ShaderTranslation => "shader_translation",
            Self::PipelineCreation => "pipeline_creation",
            Self::BindLayoutCreation => "bind_layout_creation",
            Self::ResourceCreation => "resource_creation",
            Self::NormalFrameBlockingWait => "normal_frame_blocking_wait",
        }
    }

    #[must_use]
    pub const fn associated_violation(self) -> Dx12GateViolationKind {
        match self {
            Self::ShaderTranslation => Dx12GateViolationKind::ShaderTranslationAfterWarmup,
            Self::PipelineCreation => Dx12GateViolationKind::PipelineCreationAfterWarmup,
            Self::BindLayoutCreation => Dx12GateViolationKind::BindLayoutCreationAfterWarmup,
            Self::ResourceCreation => Dx12GateViolationKind::PerFrameResourceCreationGrowth,
            Self::NormalFrameBlockingWait => Dx12GateViolationKind::NormalFrameBlockingWait,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Resource)]
pub struct Dx12RuntimeGateCounters {
    pub schema_version: u16,
    pub frame_index: u64,
    pub state: Dx12WarmupState,
    pub warmup_totals: [u64; DX12_RUNTIME_RESOURCE_KIND_COUNT],
    pub post_warmup_totals: [u64; DX12_RUNTIME_RESOURCE_KIND_COUNT],
    pub current_frame_totals: [u64; DX12_RUNTIME_RESOURCE_KIND_COUNT],
    pub last_frame_totals: [u64; DX12_RUNTIME_RESOURCE_KIND_COUNT],
}

impl Dx12RuntimeGateCounters {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            schema_version: DX12_PRODUCTION_SCHEMA_VERSION,
            frame_index: 0,
            state: Dx12WarmupState::Cold,
            warmup_totals: [0; DX12_RUNTIME_RESOURCE_KIND_COUNT],
            post_warmup_totals: [0; DX12_RUNTIME_RESOURCE_KIND_COUNT],
            current_frame_totals: [0; DX12_RUNTIME_RESOURCE_KIND_COUNT],
            last_frame_totals: [0; DX12_RUNTIME_RESOURCE_KIND_COUNT],
        }
    }

    pub fn note_state(&mut self, state: Dx12WarmupState) {
        self.state = state;
    }

    pub fn record(&mut self, kind: Dx12RuntimeResourceKind, count: u64) {
        let slot = kind.index();
        self.current_frame_totals[slot] = self.current_frame_totals[slot].saturating_add(count);
        if self.state.enforces_no_growth() {
            self.post_warmup_totals[slot] = self.post_warmup_totals[slot].saturating_add(count);
        } else {
            self.warmup_totals[slot] = self.warmup_totals[slot].saturating_add(count);
        }
    }

    /// End-of-frame rotation: copy `current_frame_totals` into
    /// `last_frame_totals` and zero the current frame.
    pub fn rotate_frame(&mut self) {
        self.last_frame_totals = self.current_frame_totals;
        self.current_frame_totals = [0; DX12_RUNTIME_RESOURCE_KIND_COUNT];
        self.frame_index = self.frame_index.saturating_add(1);
    }

    #[must_use]
    pub fn current_frame_count(&self, kind: Dx12RuntimeResourceKind) -> u64 {
        self.current_frame_totals[kind.index()]
    }

    #[must_use]
    pub fn last_frame_count(&self, kind: Dx12RuntimeResourceKind) -> u64 {
        self.last_frame_totals[kind.index()]
    }

    #[must_use]
    pub fn warmup_count(&self, kind: Dx12RuntimeResourceKind) -> u64 {
        self.warmup_totals[kind.index()]
    }

    #[must_use]
    pub fn post_warmup_count(&self, kind: Dx12RuntimeResourceKind) -> u64 {
        self.post_warmup_totals[kind.index()]
    }

    #[must_use]
    pub fn post_warmup_total(&self) -> u64 {
        self.post_warmup_totals
            .iter()
            .copied()
            .fold(0u64, |acc, v| acc.saturating_add(v))
    }
}

// ============================================================================
// Section 3 — Gate violation kinds + post-warmup enforcement
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dx12GateViolationKind {
    ShaderTranslationAfterWarmup,
    PipelineCreationAfterWarmup,
    BindLayoutCreationAfterWarmup,
    PerFrameResourceCreationGrowth,
    NormalFrameBlockingWait,
    BackendMismatch,
    DeviceLostUnhandled,
    NativeCommandListClaimedButUnavailable,
}

impl Dx12GateViolationKind {
    pub const ALL: [Self; DX12_GATE_VIOLATION_KIND_COUNT] = [
        Self::ShaderTranslationAfterWarmup,
        Self::PipelineCreationAfterWarmup,
        Self::BindLayoutCreationAfterWarmup,
        Self::PerFrameResourceCreationGrowth,
        Self::NormalFrameBlockingWait,
        Self::BackendMismatch,
        Self::DeviceLostUnhandled,
        Self::NativeCommandListClaimedButUnavailable,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::ShaderTranslationAfterWarmup => 0,
            Self::PipelineCreationAfterWarmup => 1,
            Self::BindLayoutCreationAfterWarmup => 2,
            Self::PerFrameResourceCreationGrowth => 3,
            Self::NormalFrameBlockingWait => 4,
            Self::BackendMismatch => 5,
            Self::DeviceLostUnhandled => 6,
            Self::NativeCommandListClaimedButUnavailable => 7,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ShaderTranslationAfterWarmup => "shader_translation_after_warmup",
            Self::PipelineCreationAfterWarmup => "pipeline_creation_after_warmup",
            Self::BindLayoutCreationAfterWarmup => "bind_layout_creation_after_warmup",
            Self::PerFrameResourceCreationGrowth => "per_frame_resource_creation_growth",
            Self::NormalFrameBlockingWait => "normal_frame_blocking_wait",
            Self::BackendMismatch => "backend_mismatch",
            Self::DeviceLostUnhandled => "device_lost_unhandled",
            Self::NativeCommandListClaimedButUnavailable => {
                "native_command_list_claimed_but_unavailable"
            }
        }
    }

    #[must_use]
    pub const fn is_fatal(self) -> bool {
        matches!(
            self,
            Self::BackendMismatch
                | Self::DeviceLostUnhandled
                | Self::NativeCommandListClaimedButUnavailable,
        )
    }

    #[must_use]
    pub const fn classifies_post_warmup_growth(self) -> bool {
        matches!(
            self,
            Self::ShaderTranslationAfterWarmup
                | Self::PipelineCreationAfterWarmup
                | Self::BindLayoutCreationAfterWarmup
                | Self::PerFrameResourceCreationGrowth,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dx12GateViolation {
    pub schema_version: u16,
    pub kind: Dx12GateViolationKind,
    pub frame_index: u64,
    pub state: Dx12WarmupState,
    pub count: u64,
    pub debug_label: &'static str,
}

impl Dx12GateViolation {
    #[must_use]
    pub const fn new(
        kind: Dx12GateViolationKind,
        frame_index: u64,
        state: Dx12WarmupState,
        count: u64,
    ) -> Self {
        Self {
            schema_version: DX12_PRODUCTION_SCHEMA_VERSION,
            kind,
            frame_index,
            state,
            count,
            debug_label: "dx12_gate.violation",
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Resource)]
pub struct Dx12RuntimeGateReport {
    pub schema_version: u16,
    pub frame_index: u64,
    pub state: Dx12WarmupState,
    pub violations: Vec<Dx12GateViolation>,
    pub violation_counts: [u32; DX12_GATE_VIOLATION_KIND_COUNT],
    pub fatal_violation_count: u32,
}

impl Dx12RuntimeGateReport {
    pub fn evaluate(&mut self, counters: &Dx12RuntimeGateCounters) {
        self.schema_version = DX12_PRODUCTION_SCHEMA_VERSION;
        self.frame_index = counters.frame_index;
        self.state = counters.state;
        if !counters.state.enforces_no_growth() {
            return;
        }
        for kind in Dx12RuntimeResourceKind::ALL {
            let post = counters.last_frame_count(kind);
            if post == 0 {
                continue;
            }
            let violation_kind = kind.associated_violation();
            self.record_violation(violation_kind, counters.frame_index, counters.state, post);
        }
    }

    pub fn record_violation(
        &mut self,
        kind: Dx12GateViolationKind,
        frame_index: u64,
        state: Dx12WarmupState,
        count: u64,
    ) {
        self.violations
            .push(Dx12GateViolation::new(kind, frame_index, state, count));
        self.violation_counts[kind.index()] = self.violation_counts[kind.index()].saturating_add(1);
        if kind.is_fatal() {
            self.fatal_violation_count = self.fatal_violation_count.saturating_add(1);
        }
    }

    pub fn clear(&mut self) {
        self.violations.clear();
        self.violation_counts = [0; DX12_GATE_VIOLATION_KIND_COUNT];
        self.fatal_violation_count = 0;
    }

    #[must_use]
    pub fn passes(&self) -> bool {
        self.violations.is_empty()
    }

    #[must_use]
    pub fn count(&self, kind: Dx12GateViolationKind) -> u32 {
        self.violation_counts[kind.index()]
    }
}

// ============================================================================
// Section 4 — DX12 strict startup gate
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dx12StrictStartupOutcome {
    Accepted {
        actual_backend: NativeBackend,
    },
    RejectedDueToBackendMismatch {
        requested: NativeBackend,
        actual: NativeBackend,
    },
    RejectedDueToBridgeNotDx12 {
        requested: NativeBackend,
    },
    RejectedDueToActualBackendUnknown {
        requested: NativeBackend,
    },
}

impl Dx12StrictStartupOutcome {
    #[must_use]
    pub const fn is_accepted(self) -> bool {
        matches!(self, Self::Accepted { .. })
    }

    #[must_use]
    pub const fn associated_violation(self) -> Option<Dx12GateViolationKind> {
        match self {
            Self::Accepted { .. } => None,
            Self::RejectedDueToBackendMismatch { .. }
            | Self::RejectedDueToBridgeNotDx12 { .. }
            | Self::RejectedDueToActualBackendUnknown { .. } => {
                Some(Dx12GateViolationKind::BackendMismatch)
            }
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Accepted { .. } => "accepted",
            Self::RejectedDueToBackendMismatch { .. } => "rejected_backend_mismatch",
            Self::RejectedDueToBridgeNotDx12 { .. } => "rejected_bridge_not_dx12",
            Self::RejectedDueToActualBackendUnknown { .. } => "rejected_actual_backend_unknown",
        }
    }

    #[must_use]
    pub const fn requested(self) -> NativeBackend {
        match self {
            Self::Accepted { actual_backend } => actual_backend,
            Self::RejectedDueToBackendMismatch { requested, .. }
            | Self::RejectedDueToBridgeNotDx12 { requested }
            | Self::RejectedDueToActualBackendUnknown { requested } => requested,
        }
    }

    #[must_use]
    pub const fn actual(self) -> NativeBackend {
        match self {
            Self::Accepted { actual_backend } => actual_backend,
            Self::RejectedDueToBackendMismatch { actual, .. } => actual,
            Self::RejectedDueToBridgeNotDx12 { .. }
            | Self::RejectedDueToActualBackendUnknown { .. } => NativeBackend::Unknown,
        }
    }
}

#[must_use]
pub const fn evaluate_dx12_strict_startup(
    requested: NativeBackend,
    actual: NativeBackend,
) -> Dx12StrictStartupOutcome {
    if !matches!(requested, NativeBackend::Dx12) {
        return Dx12StrictStartupOutcome::RejectedDueToBridgeNotDx12 { requested };
    }
    match actual {
        NativeBackend::Dx12 => Dx12StrictStartupOutcome::Accepted {
            actual_backend: actual,
        },
        NativeBackend::Unknown => {
            Dx12StrictStartupOutcome::RejectedDueToActualBackendUnknown { requested }
        }
        NativeBackend::Vulkan | NativeBackend::Metal => {
            Dx12StrictStartupOutcome::RejectedDueToBackendMismatch { requested, actual }
        }
    }
}

// ============================================================================
// Section 5 — Native SDK / DLSS claim policy
// ============================================================================

/// Typed scaffold asserting that DLSS / native SDK claims remain
/// blocked while neither sanctioned wgpu access nor a direct DX12
/// backend owns command recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dx12NativeSdkClaimPolicy {
    pub schema_version: u16,
    pub wgpu_exposes_sanctioned_command_list_access: bool,
    pub direct_dx12_backend_owns_command_recording: bool,
    pub dlss_claim_blocked: bool,
    pub native_sdk_claim_blocked: bool,
}

impl Dx12NativeSdkClaimPolicy {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: DX12_PRODUCTION_SCHEMA_VERSION,
        wgpu_exposes_sanctioned_command_list_access: false,
        direct_dx12_backend_owns_command_recording: false,
        dlss_claim_blocked: true,
        native_sdk_claim_blocked: true,
    };

    #[must_use]
    pub const fn from_capabilities(wgpu_exposes_sanctioned: bool, direct_dx12_owns: bool) -> Self {
        let blocked = !(wgpu_exposes_sanctioned || direct_dx12_owns);
        Self {
            schema_version: DX12_PRODUCTION_SCHEMA_VERSION,
            wgpu_exposes_sanctioned_command_list_access: wgpu_exposes_sanctioned,
            direct_dx12_backend_owns_command_recording: direct_dx12_owns,
            dlss_claim_blocked: blocked,
            native_sdk_claim_blocked: blocked,
        }
    }

    #[must_use]
    pub const fn allows_native_command_list_use(self) -> bool {
        !self.dlss_claim_blocked && !self.native_sdk_claim_blocked
    }
}

impl Default for Dx12NativeSdkClaimPolicy {
    fn default() -> Self {
        Self::PRODUCT_DEFAULT
    }
}

// ============================================================================
// Section 6 — DX12 production hardening report (artifact bundle)
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dx12AdapterIdentitySummary {
    pub vendor_id: u32,
    pub device_id: u32,
    pub adapter_name_hash: u64,
    pub adapter_name_len: u32,
    pub driver_signature: u64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dx12FeatureLimitSummary {
    pub feature_bits_low: u64,
    pub feature_bits_high: u64,
    pub max_texture_dimension_2d: u32,
    pub max_bind_groups: u32,
    pub max_buffer_size: u64,
    pub max_color_attachments: u32,
    pub max_compute_workgroups_per_dimension: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dx12SurfaceStatus {
    pub surface_configured: bool,
    pub surface_format_signature: u64,
    pub present_mode_signature: u64,
    pub width_px: u32,
    pub height_px: u32,
    pub image_count: u8,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dx12NativeInteropCapabilities {
    pub native_command_list_available: bool,
    pub d3d12_resource_interop_available: bool,
    pub shared_handle_interop_available: bool,
    pub fence_interop_available: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dx12DeviceLostState {
    #[default]
    Healthy,
    Recovering,
    Lost,
    Unrecoverable,
}

impl Dx12DeviceLostState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Recovering => "recovering",
            Self::Lost => "lost",
            Self::Unrecoverable => "unrecoverable",
        }
    }

    #[must_use]
    pub const fn is_runtime_blocker(self) -> bool {
        matches!(self, Self::Lost | Self::Unrecoverable)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dx12ValidationHookState {
    #[default]
    DefaultDisabled,
    DebugLayerActive,
    GpuBasedValidationActive,
    DebugLayerWithGpuValidation,
    BridgeRejectedActivation,
}

impl Dx12ValidationHookState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DefaultDisabled => "default_disabled",
            Self::DebugLayerActive => "debug_layer_active",
            Self::GpuBasedValidationActive => "gpu_based_validation_active",
            Self::DebugLayerWithGpuValidation => "debug_layer_with_gpu_validation",
            Self::BridgeRejectedActivation => "bridge_rejected_activation",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dx12CommandListBlockerPolicy {
    #[default]
    NativeCommandListUnavailableFailClosed,
    NativeCommandListUnavailableSilentFallback,
    NativeCommandListAvailable,
}

impl Dx12CommandListBlockerPolicy {
    pub const PRODUCT_DEFAULT: Self = Self::NativeCommandListUnavailableFailClosed;

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NativeCommandListUnavailableFailClosed => {
                "native_command_list_unavailable_fail_closed"
            }
            Self::NativeCommandListUnavailableSilentFallback => {
                "native_command_list_unavailable_silent_fallback"
            }
            Self::NativeCommandListAvailable => "native_command_list_available",
        }
    }

    #[must_use]
    pub const fn fails_closed(self) -> bool {
        matches!(self, Self::NativeCommandListUnavailableFailClosed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Resource)]
pub struct Dx12ProductionHardeningReport {
    pub schema_version: u16,
    pub canonical_path: &'static str,
    pub startup_outcome: Dx12StrictStartupOutcome,
    pub warmup: Dx12WarmupTracker,
    pub gate_counters: Dx12RuntimeGateCounters,
    pub gate_report: Dx12RuntimeGateReport,
    pub adapter_identity: Dx12AdapterIdentitySummary,
    pub feature_limit: Dx12FeatureLimitSummary,
    pub surface_status: Dx12SurfaceStatus,
    pub interop: Dx12NativeInteropCapabilities,
    pub device_lost_state: Dx12DeviceLostState,
    pub validation_hook_state: Dx12ValidationHookState,
    pub command_list_blocker_policy: Dx12CommandListBlockerPolicy,
    pub native_sdk_claim_policy: Dx12NativeSdkClaimPolicy,
}

impl Dx12ProductionHardeningReport {
    pub const CANONICAL_ARTIFACT_PATH: &'static str =
        "fun_renderer.dx12.production_hardening.funpb.zst";

    #[must_use]
    pub fn cold_default() -> Self {
        Self {
            schema_version: DX12_PRODUCTION_SCHEMA_VERSION,
            canonical_path: Self::CANONICAL_ARTIFACT_PATH,
            startup_outcome: Dx12StrictStartupOutcome::RejectedDueToActualBackendUnknown {
                requested: NativeBackend::Dx12,
            },
            warmup: Dx12WarmupTracker::new(),
            gate_counters: Dx12RuntimeGateCounters::new(),
            gate_report: Dx12RuntimeGateReport::default(),
            adapter_identity: Dx12AdapterIdentitySummary::default(),
            feature_limit: Dx12FeatureLimitSummary::default(),
            surface_status: Dx12SurfaceStatus::default(),
            interop: Dx12NativeInteropCapabilities::default(),
            device_lost_state: Dx12DeviceLostState::Healthy,
            validation_hook_state: Dx12ValidationHookState::default(),
            command_list_blocker_policy: Dx12CommandListBlockerPolicy::PRODUCT_DEFAULT,
            native_sdk_claim_policy: Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT,
        }
    }

    pub fn note_startup_outcome(&mut self, outcome: Dx12StrictStartupOutcome) {
        self.startup_outcome = outcome;
        if let Some(violation) = outcome.associated_violation() {
            self.gate_report.record_violation(
                violation,
                self.gate_counters.frame_index,
                self.warmup.state,
                1,
            );
        }
    }

    pub fn note_device_lost(&mut self, state: Dx12DeviceLostState) {
        self.device_lost_state = state;
        if state.is_runtime_blocker() {
            self.gate_report.record_violation(
                Dx12GateViolationKind::DeviceLostUnhandled,
                self.gate_counters.frame_index,
                self.warmup.state,
                1,
            );
        }
    }

    pub fn note_native_sdk_claim_attempted(&mut self) {
        if !self
            .native_sdk_claim_policy
            .allows_native_command_list_use()
        {
            self.gate_report.record_violation(
                Dx12GateViolationKind::NativeCommandListClaimedButUnavailable,
                self.gate_counters.frame_index,
                self.warmup.state,
                1,
            );
        }
    }

    pub fn evaluate_runtime_gates(&mut self) {
        self.gate_counters.note_state(self.warmup.state);
        self.gate_report.evaluate(&self.gate_counters);
    }

    #[must_use]
    pub fn product_dx12_truth_holds(&self) -> bool {
        self.startup_outcome.is_accepted()
            && matches!(self.device_lost_state, Dx12DeviceLostState::Healthy)
            && self.gate_report.fatal_violation_count == 0
    }

    #[must_use]
    pub fn fail_closed_command_list(&self) -> bool {
        self.command_list_blocker_policy.fails_closed()
            && !self.interop.native_command_list_available
    }
}

impl Default for Dx12ProductionHardeningReport {
    fn default() -> Self {
        Self::cold_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fully_warmed_tracker() -> Dx12WarmupTracker {
        let mut tracker = Dx12WarmupTracker::new();
        tracker.advance_with_event(Dx12WarmupEvent::InstanceInitialized);
        tracker.advance_with_event(Dx12WarmupEvent::AdapterSelected);
        tracker.advance_with_event(Dx12WarmupEvent::DeviceCreated);
        tracker.advance_with_event(Dx12WarmupEvent::InitialShaderTranslationCompleted);
        tracker.advance_with_event(Dx12WarmupEvent::InitialPipelineWarmupCompleted);
        tracker.advance_with_event(Dx12WarmupEvent::InitialBindLayoutWarmupCompleted);
        tracker.advance_with_event(Dx12WarmupEvent::InitialResourcesUploaded);
        tracker.advance_with_event(Dx12WarmupEvent::FirstFramePresented);
        tracker
    }

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(DX12_PRODUCTION_SCHEMA_VERSION, 1);
        assert_eq!(DX12_GATE_VIOLATION_KIND_COUNT, 8);
        assert_eq!(DX12_WARMUP_STATE_COUNT, 4);
        assert_eq!(DX12_RUNTIME_RESOURCE_KIND_COUNT, 5);
    }

    #[test]
    fn warmup_default_is_cold_with_no_growth_enforcement() {
        let tracker = Dx12WarmupTracker::new();
        assert_eq!(tracker.state, Dx12WarmupState::Cold);
        assert!(!tracker.enforces_no_growth());
        assert!(tracker.state.allows_warmup_creation());
    }

    #[test]
    fn warmup_advance_to_warming_after_device_creation() {
        let mut tracker = Dx12WarmupTracker::new();
        tracker.advance_with_event(Dx12WarmupEvent::InstanceInitialized);
        tracker.advance_with_event(Dx12WarmupEvent::AdapterSelected);
        tracker.advance_with_event(Dx12WarmupEvent::DeviceCreated);
        assert_eq!(tracker.state, Dx12WarmupState::Warming);
    }

    #[test]
    fn warmup_advances_to_warm_when_all_warmup_events_complete() {
        let tracker = fully_warmed_tracker();
        assert_eq!(tracker.state, Dx12WarmupState::Warm);
        assert!(tracker.enforces_no_growth());
        assert!(tracker.fully_warmed());
    }

    #[test]
    fn warmup_explicitly_advances_to_production_on_event() {
        let mut tracker = fully_warmed_tracker();
        tracker.advance_with_event(Dx12WarmupEvent::EnteredProductionLoop);
        assert_eq!(tracker.state, Dx12WarmupState::Production);
        assert!(tracker.enforces_no_growth());
    }

    #[test]
    fn warmup_state_strings_match_pass_26_contract() {
        assert_eq!(Dx12WarmupState::Cold.as_str(), "cold");
        assert_eq!(Dx12WarmupState::Warming.as_str(), "warming");
        assert_eq!(Dx12WarmupState::Warm.as_str(), "warm");
        assert_eq!(Dx12WarmupState::Production.as_str(), "production");
    }

    #[test]
    fn runtime_resource_kind_associated_violation_routes_correctly() {
        assert!(matches!(
            Dx12RuntimeResourceKind::ShaderTranslation.associated_violation(),
            Dx12GateViolationKind::ShaderTranslationAfterWarmup,
        ));
        assert!(matches!(
            Dx12RuntimeResourceKind::PipelineCreation.associated_violation(),
            Dx12GateViolationKind::PipelineCreationAfterWarmup,
        ));
        assert!(matches!(
            Dx12RuntimeResourceKind::ResourceCreation.associated_violation(),
            Dx12GateViolationKind::PerFrameResourceCreationGrowth,
        ));
        assert!(matches!(
            Dx12RuntimeResourceKind::NormalFrameBlockingWait.associated_violation(),
            Dx12GateViolationKind::NormalFrameBlockingWait,
        ));
    }

    #[test]
    fn counters_attribute_pre_warmup_creations_to_warmup_total() {
        let mut counters = Dx12RuntimeGateCounters::new();
        counters.note_state(Dx12WarmupState::Warming);
        counters.record(Dx12RuntimeResourceKind::ShaderTranslation, 5);
        assert_eq!(
            counters.warmup_count(Dx12RuntimeResourceKind::ShaderTranslation),
            5,
        );
        assert_eq!(
            counters.post_warmup_count(Dx12RuntimeResourceKind::ShaderTranslation),
            0,
        );
    }

    #[test]
    fn counters_attribute_post_warmup_creations_to_post_warmup_total() {
        let mut counters = Dx12RuntimeGateCounters::new();
        counters.note_state(Dx12WarmupState::Production);
        counters.record(Dx12RuntimeResourceKind::PipelineCreation, 1);
        assert_eq!(
            counters.warmup_count(Dx12RuntimeResourceKind::PipelineCreation),
            0,
        );
        assert_eq!(
            counters.post_warmup_count(Dx12RuntimeResourceKind::PipelineCreation),
            1,
        );
    }

    #[test]
    fn counters_rotate_frame_moves_current_to_last_and_advances_index() {
        let mut counters = Dx12RuntimeGateCounters::new();
        counters.note_state(Dx12WarmupState::Production);
        counters.record(Dx12RuntimeResourceKind::ResourceCreation, 3);
        counters.rotate_frame();
        assert_eq!(
            counters.last_frame_count(Dx12RuntimeResourceKind::ResourceCreation),
            3,
        );
        assert_eq!(
            counters.current_frame_count(Dx12RuntimeResourceKind::ResourceCreation),
            0,
        );
        assert_eq!(counters.frame_index, 1);
    }

    #[test]
    fn gate_report_records_no_violations_when_no_post_warmup_events() {
        let counters = Dx12RuntimeGateCounters::new();
        let mut report = Dx12RuntimeGateReport::default();
        report.evaluate(&counters);
        assert!(report.passes());
        assert_eq!(report.fatal_violation_count, 0);
    }

    #[test]
    fn gate_report_records_violation_per_resource_kind_post_warmup() {
        let mut counters = Dx12RuntimeGateCounters::new();
        counters.note_state(Dx12WarmupState::Production);
        counters.record(Dx12RuntimeResourceKind::ShaderTranslation, 2);
        counters.record(Dx12RuntimeResourceKind::PipelineCreation, 1);
        counters.rotate_frame();
        counters.note_state(Dx12WarmupState::Production);
        let mut report = Dx12RuntimeGateReport::default();
        report.evaluate(&counters);
        assert_eq!(
            report.count(Dx12GateViolationKind::ShaderTranslationAfterWarmup),
            1,
        );
        assert_eq!(
            report.count(Dx12GateViolationKind::PipelineCreationAfterWarmup),
            1,
        );
        assert_eq!(report.fatal_violation_count, 0);
    }

    #[test]
    fn gate_violation_kind_classifies_post_warmup_growth_subset() {
        for kind in Dx12GateViolationKind::ALL {
            match kind {
                Dx12GateViolationKind::ShaderTranslationAfterWarmup
                | Dx12GateViolationKind::PipelineCreationAfterWarmup
                | Dx12GateViolationKind::BindLayoutCreationAfterWarmup
                | Dx12GateViolationKind::PerFrameResourceCreationGrowth => {
                    assert!(kind.classifies_post_warmup_growth());
                    assert!(!kind.is_fatal());
                }
                Dx12GateViolationKind::NormalFrameBlockingWait => {
                    assert!(!kind.classifies_post_warmup_growth());
                    assert!(!kind.is_fatal());
                }
                Dx12GateViolationKind::BackendMismatch
                | Dx12GateViolationKind::DeviceLostUnhandled
                | Dx12GateViolationKind::NativeCommandListClaimedButUnavailable => {
                    assert!(!kind.classifies_post_warmup_growth());
                    assert!(kind.is_fatal());
                }
            }
        }
    }

    #[test]
    fn dx12_strict_startup_accepts_actual_dx12() {
        let outcome = evaluate_dx12_strict_startup(NativeBackend::Dx12, NativeBackend::Dx12);
        assert!(outcome.is_accepted());
        assert!(outcome.associated_violation().is_none());
    }

    #[test]
    fn dx12_strict_startup_rejects_actual_vulkan() {
        let outcome = evaluate_dx12_strict_startup(NativeBackend::Dx12, NativeBackend::Vulkan);
        assert!(!outcome.is_accepted());
        assert_eq!(
            outcome.associated_violation(),
            Some(Dx12GateViolationKind::BackendMismatch),
        );
        assert_eq!(outcome.actual(), NativeBackend::Vulkan);
    }

    #[test]
    fn dx12_strict_startup_rejects_actual_unknown() {
        let outcome = evaluate_dx12_strict_startup(NativeBackend::Dx12, NativeBackend::Unknown);
        assert!(matches!(
            outcome,
            Dx12StrictStartupOutcome::RejectedDueToActualBackendUnknown { .. },
        ));
    }

    #[test]
    fn dx12_strict_startup_rejects_non_dx12_bridge() {
        let outcome = evaluate_dx12_strict_startup(NativeBackend::Vulkan, NativeBackend::Vulkan);
        assert!(matches!(
            outcome,
            Dx12StrictStartupOutcome::RejectedDueToBridgeNotDx12 { .. },
        ));
    }

    #[test]
    fn native_sdk_claim_policy_default_blocks_dlss_and_native_sdk() {
        let policy = Dx12NativeSdkClaimPolicy::default();
        assert!(policy.dlss_claim_blocked);
        assert!(policy.native_sdk_claim_blocked);
        assert!(!policy.allows_native_command_list_use());
    }

    #[test]
    fn native_sdk_claim_policy_unblocks_when_sanctioned_access_available() {
        let policy = Dx12NativeSdkClaimPolicy::from_capabilities(true, false);
        assert!(!policy.dlss_claim_blocked);
        assert!(!policy.native_sdk_claim_blocked);
        assert!(policy.allows_native_command_list_use());
    }

    #[test]
    fn native_sdk_claim_policy_unblocks_when_direct_dx12_owns_recording() {
        let policy = Dx12NativeSdkClaimPolicy::from_capabilities(false, true);
        assert!(policy.allows_native_command_list_use());
    }

    #[test]
    fn command_list_blocker_default_fails_closed() {
        assert!(Dx12CommandListBlockerPolicy::PRODUCT_DEFAULT.fails_closed());
        assert!(!Dx12CommandListBlockerPolicy::NativeCommandListAvailable.fails_closed());
    }

    #[test]
    fn device_lost_state_is_runtime_blocker_only_when_lost() {
        assert!(!Dx12DeviceLostState::Healthy.is_runtime_blocker());
        assert!(!Dx12DeviceLostState::Recovering.is_runtime_blocker());
        assert!(Dx12DeviceLostState::Lost.is_runtime_blocker());
        assert!(Dx12DeviceLostState::Unrecoverable.is_runtime_blocker());
    }

    #[test]
    fn report_cold_default_is_dx12_canonical_path() {
        let report = Dx12ProductionHardeningReport::cold_default();
        assert_eq!(
            report.canonical_path,
            Dx12ProductionHardeningReport::CANONICAL_ARTIFACT_PATH,
        );
        assert!(matches!(
            report.startup_outcome,
            Dx12StrictStartupOutcome::RejectedDueToActualBackendUnknown { .. },
        ));
        assert!(report.fail_closed_command_list());
    }

    #[test]
    fn report_note_startup_outcome_records_backend_mismatch_violation() {
        let mut report = Dx12ProductionHardeningReport::cold_default();
        report.note_startup_outcome(Dx12StrictStartupOutcome::RejectedDueToBackendMismatch {
            requested: NativeBackend::Dx12,
            actual: NativeBackend::Vulkan,
        });
        assert_eq!(
            report
                .gate_report
                .count(Dx12GateViolationKind::BackendMismatch),
            1,
        );
        assert_eq!(report.gate_report.fatal_violation_count, 1);
    }

    #[test]
    fn report_note_device_lost_records_unhandled_violation() {
        let mut report = Dx12ProductionHardeningReport::cold_default();
        report.note_device_lost(Dx12DeviceLostState::Lost);
        assert_eq!(
            report
                .gate_report
                .count(Dx12GateViolationKind::DeviceLostUnhandled),
            1,
        );
    }

    #[test]
    fn report_native_sdk_claim_records_violation_when_blocked() {
        let mut report = Dx12ProductionHardeningReport::cold_default();
        report.note_native_sdk_claim_attempted();
        assert_eq!(
            report
                .gate_report
                .count(Dx12GateViolationKind::NativeCommandListClaimedButUnavailable),
            1,
        );
    }

    #[test]
    fn report_native_sdk_claim_skips_violation_when_unblocked() {
        let mut report = Dx12ProductionHardeningReport::cold_default();
        report.native_sdk_claim_policy = Dx12NativeSdkClaimPolicy::from_capabilities(true, false);
        report.note_native_sdk_claim_attempted();
        assert_eq!(
            report
                .gate_report
                .count(Dx12GateViolationKind::NativeCommandListClaimedButUnavailable),
            0,
        );
    }

    #[test]
    fn report_evaluate_runtime_gates_records_post_warmup_growth() {
        let mut report = Dx12ProductionHardeningReport::cold_default();
        report.warmup = fully_warmed_tracker();
        report.gate_counters.note_state(report.warmup.state);
        report
            .gate_counters
            .record(Dx12RuntimeResourceKind::PipelineCreation, 1);
        report.gate_counters.rotate_frame();
        report.evaluate_runtime_gates();
        assert_eq!(
            report
                .gate_report
                .count(Dx12GateViolationKind::PipelineCreationAfterWarmup),
            1,
        );
    }

    #[test]
    fn report_truth_holds_only_when_all_gates_pass() {
        let mut report = Dx12ProductionHardeningReport::cold_default();
        assert!(!report.product_dx12_truth_holds());
        report.note_startup_outcome(Dx12StrictStartupOutcome::Accepted {
            actual_backend: NativeBackend::Dx12,
        });
        // Accepted startup outcome should not record a violation
        assert!(report.product_dx12_truth_holds());
        report.note_device_lost(Dx12DeviceLostState::Lost);
        assert!(!report.product_dx12_truth_holds());
    }

    #[test]
    fn fail_closed_command_list_reports_true_when_unavailable() {
        let report = Dx12ProductionHardeningReport::cold_default();
        assert!(report.fail_closed_command_list());
        let mut report_with_interop = report;
        report_with_interop.interop.native_command_list_available = true;
        assert!(!report_with_interop.fail_closed_command_list());
    }

    #[test]
    fn warmup_event_strings_are_stable() {
        assert_eq!(
            Dx12WarmupEvent::InstanceInitialized.as_str(),
            "instance_initialized",
        );
        assert_eq!(
            Dx12WarmupEvent::EnteredProductionLoop.as_str(),
            "entered_production_loop",
        );
    }

    #[test]
    fn warmup_resets_to_cold_only_before_device_created() {
        let mut tracker = Dx12WarmupTracker::new();
        tracker.advance_with_event(Dx12WarmupEvent::InstanceInitialized);
        assert_eq!(tracker.state, Dx12WarmupState::Cold);
        tracker.advance_with_event(Dx12WarmupEvent::AdapterSelected);
        assert_eq!(tracker.state, Dx12WarmupState::Cold);
    }

    #[test]
    fn validation_hook_state_strings_match_pass_26_contract() {
        assert_eq!(
            Dx12ValidationHookState::default().as_str(),
            "default_disabled",
        );
        assert_eq!(
            Dx12ValidationHookState::DebugLayerWithGpuValidation.as_str(),
            "debug_layer_with_gpu_validation",
        );
        assert_eq!(
            Dx12ValidationHookState::BridgeRejectedActivation.as_str(),
            "bridge_rejected_activation",
        );
    }
}
