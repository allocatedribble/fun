//! Tier 8 — Direct Backend Experiments.
//!
//! The current `WgpuBridge<Dx12Native>` is a good zero-cost
//! high-level abstraction path; some bleeding-edge features
//! may hit wgpu limits. The user's rule: **do not fork the
//! renderer API**. Add direct backend experiments below the
//! same IR.
//!
//! Tier 8 installs the typed contracts that govern direct
//! backend experimentation:
//!
//! 1. **Trigger conditions** — direct DX12 may only start when
//!    at least one of the seven named blockers is active:
//!    NativeCommandListAccess / DlssReflexStreamline /
//!    EnhancedBarrierControl / DescriptorHeapBindless /
//!    AsyncQueueOverlap / SamplerFeedback / MeshShaders.
//!    `Tier8DirectBackendStartGate::evaluate(blockers)` returns
//!    `MayStart` only when at least one trigger is recorded;
//!    otherwise `MustNotStartNoTriggerActive`.
//!
//! 2. **Experimental scope rules** — the same ECS API, same
//!    renderer IR, same graph, same resource / binding /
//!    pipeline concepts. Only the bridge realization changes.
//!    `Tier8DirectDx12ExperimentalScope` records the typed
//!    bool flags; `is_honest_scope()` requires every flag to
//!    be true.
//!
//! 3. **Proof-of-life stages** — five typed stages every
//!    direct-DX12 experiment must complete in order before
//!    any production-shaped claim:
//!    DeviceQueueSwapchain / ImportProofSceneIr /
//!    ExecuteClearAndOpaque / Present /
//!    CompareAgainstWgpuDx12Baseline. The comparison stage
//!    must produce a typed `Tier8DirectDx12ComparisonResult`
//!    showing the direct path matches the wgpu-DX12 baseline.
//!
//! 4. **Vulkan / Metal conformance** — NOT separate
//!    renderers. Typed `Tier8ConformanceRunner` runs the same
//!    proof scene + same UI packet + same graph + same cache
//!    policy through each backend and reports
//!    `Tier8ConformanceCapabilityReport`. The acceptance rule
//!    is **honest capability status, not fake parity**:
//!    `passes_honest_capability_status` is true only when each
//!    backend's report names the actual feature set, and
//!    `passes_no_fake_parity_claims` rejects any claim that
//!    two backends are at parity when the typed capability
//!    flags differ.
//!
//! Honest scope: this module is the *contract* layer. Actually
//! building a direct DX12 backend requires `windows-rs` raw
//! D3D12 bindings, real swapchain integration, and a multi-pass
//! engineering arc. Tier 8's deliverable is the typed gate that
//! governs *when* such an arc is legitimate to start, *what*
//! its scope is allowed to be, and *how* it must prove itself
//! against the wgpu-DX12 baseline. Tier 0 already establishes
//! the typed proof-frame gate; Tier 8 extends that contract to
//! the direct DX12 / Vulkan / Metal paths.

use bevy_ecs::prelude::Resource;

use crate::backend::NativeBackend;

pub const TIER8_DIRECT_BACKEND_EXPERIMENTS_SCHEMA_VERSION: u16 = 1;

pub const TIER8_DIRECT_BACKEND_BLOCKER_COUNT: usize = 7;
pub const TIER8_DIRECT_DX12_PROOF_STAGE_COUNT: usize = 5;
pub const TIER8_CONFORMANCE_RUNNER_KIND_COUNT: usize = 2;
pub const TIER8_DIRECT_DX12_COMPARISON_RESULT_COUNT: usize = 4;
pub const TIER8_DIRECT_BACKEND_START_GATE_VERDICT_COUNT: usize = 3;

// ============================================================================
// Section 1 — Direct backend trigger conditions
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier8DirectBackendBlocker {
    NativeCommandListAccess,
    DlssReflexStreamline,
    EnhancedBarrierControl,
    DescriptorHeapBindless,
    AsyncQueueOverlap,
    SamplerFeedback,
    MeshShaders,
}

impl Tier8DirectBackendBlocker {
    pub const ALL: [Self; TIER8_DIRECT_BACKEND_BLOCKER_COUNT] = [
        Self::NativeCommandListAccess,
        Self::DlssReflexStreamline,
        Self::EnhancedBarrierControl,
        Self::DescriptorHeapBindless,
        Self::AsyncQueueOverlap,
        Self::SamplerFeedback,
        Self::MeshShaders,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::NativeCommandListAccess => 0,
            Self::DlssReflexStreamline => 1,
            Self::EnhancedBarrierControl => 2,
            Self::DescriptorHeapBindless => 3,
            Self::AsyncQueueOverlap => 4,
            Self::SamplerFeedback => 5,
            Self::MeshShaders => 6,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NativeCommandListAccess => "native_command_list_access",
            Self::DlssReflexStreamline => "dlss_reflex_streamline",
            Self::EnhancedBarrierControl => "enhanced_barrier_control",
            Self::DescriptorHeapBindless => "descriptor_heap_bindless",
            Self::AsyncQueueOverlap => "async_queue_overlap",
            Self::SamplerFeedback => "sampler_feedback",
            Self::MeshShaders => "mesh_shaders",
        }
    }

    #[must_use]
    pub const fn requires_dx12_specific(self) -> bool {
        // Sampler feedback and DX12 enhanced barriers are
        // DX12-specific. Mesh shaders are available on Vulkan
        // too, so they are NOT exclusively DX12.
        matches!(self, Self::EnhancedBarrierControl | Self::SamplerFeedback)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier8DirectBackendBlockerSet {
    pub schema_version: u16,
    pub flags: [bool; TIER8_DIRECT_BACKEND_BLOCKER_COUNT],
}

impl Tier8DirectBackendBlockerSet {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            schema_version: TIER8_DIRECT_BACKEND_EXPERIMENTS_SCHEMA_VERSION,
            flags: [false; TIER8_DIRECT_BACKEND_BLOCKER_COUNT],
        }
    }

    pub fn record(&mut self, blocker: Tier8DirectBackendBlocker) {
        self.flags[blocker.index()] = true;
    }

    #[must_use]
    pub const fn contains(&self, blocker: Tier8DirectBackendBlocker) -> bool {
        self.flags[blocker.index()]
    }

    #[must_use]
    pub fn active_count(&self) -> u32 {
        self.flags.iter().filter(|f| **f).count() as u32
    }

    #[must_use]
    pub fn any_active(&self) -> bool {
        self.flags.iter().any(|f| *f)
    }

    pub fn active(&self) -> impl Iterator<Item = Tier8DirectBackendBlocker> + '_ {
        Tier8DirectBackendBlocker::ALL
            .into_iter()
            .filter(|b| self.flags[b.index()])
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier8DirectBackendStartVerdict {
    #[default]
    MayStartUnderActiveBlocker,
    MustNotStartNoTriggerActive,
    MustNotStartBridgeRequirementsAlreadyAdequate,
}

impl Tier8DirectBackendStartVerdict {
    pub const ALL: [Self; TIER8_DIRECT_BACKEND_START_GATE_VERDICT_COUNT] = [
        Self::MayStartUnderActiveBlocker,
        Self::MustNotStartNoTriggerActive,
        Self::MustNotStartBridgeRequirementsAlreadyAdequate,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MayStartUnderActiveBlocker => "may_start_under_active_blocker",
            Self::MustNotStartNoTriggerActive => "must_not_start_no_trigger_active",
            Self::MustNotStartBridgeRequirementsAlreadyAdequate => {
                "must_not_start_bridge_requirements_already_adequate"
            }
        }
    }

    #[must_use]
    pub const fn may_start(self) -> bool {
        matches!(self, Self::MayStartUnderActiveBlocker)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier8DirectBackendStartGate {
    pub schema_version: u16,
    pub blockers: Tier8DirectBackendBlockerSet,
    pub bridge_requirements_already_adequate: bool,
    pub verdict: Tier8DirectBackendStartVerdict,
}

impl Tier8DirectBackendStartGate {
    #[must_use]
    pub fn evaluate(
        blockers: Tier8DirectBackendBlockerSet,
        bridge_requirements_already_adequate: bool,
    ) -> Self {
        let verdict = if bridge_requirements_already_adequate {
            Tier8DirectBackendStartVerdict::MustNotStartBridgeRequirementsAlreadyAdequate
        } else if !blockers.any_active() {
            Tier8DirectBackendStartVerdict::MustNotStartNoTriggerActive
        } else {
            Tier8DirectBackendStartVerdict::MayStartUnderActiveBlocker
        };
        Self {
            schema_version: TIER8_DIRECT_BACKEND_EXPERIMENTS_SCHEMA_VERSION,
            blockers,
            bridge_requirements_already_adequate,
            verdict,
        }
    }

    #[must_use]
    pub const fn may_start(&self) -> bool {
        self.verdict.may_start()
    }
}

// ============================================================================
// Section 2 — Direct DX12 experimental scope rules
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier8DirectDx12ExperimentalScope {
    pub schema_version: u16,
    pub same_ecs_api: bool,
    pub same_renderer_ir: bool,
    pub same_graph: bool,
    pub same_resources_bindings_pipelines_conceptually: bool,
    pub replaces_only_bridge_realization: bool,
}

impl Tier8DirectDx12ExperimentalScope {
    pub const HONEST: Self = Self {
        schema_version: TIER8_DIRECT_BACKEND_EXPERIMENTS_SCHEMA_VERSION,
        same_ecs_api: true,
        same_renderer_ir: true,
        same_graph: true,
        same_resources_bindings_pipelines_conceptually: true,
        replaces_only_bridge_realization: true,
    };

    pub const FORKED_RENDERER_API_REJECTED: Self = Self {
        schema_version: TIER8_DIRECT_BACKEND_EXPERIMENTS_SCHEMA_VERSION,
        same_ecs_api: false,
        same_renderer_ir: false,
        same_graph: false,
        same_resources_bindings_pipelines_conceptually: false,
        replaces_only_bridge_realization: false,
    };

    /// Acceptance: every flag must be true. The user's rule
    /// "do not fork the renderer API" is encoded directly — any
    /// false flag fails the typed scope check.
    #[must_use]
    pub const fn is_honest_scope(&self) -> bool {
        self.same_ecs_api
            && self.same_renderer_ir
            && self.same_graph
            && self.same_resources_bindings_pipelines_conceptually
            && self.replaces_only_bridge_realization
    }

    #[must_use]
    pub fn first_violated_rule(&self) -> Option<&'static str> {
        if !self.same_ecs_api {
            return Some("same_ecs_api");
        }
        if !self.same_renderer_ir {
            return Some("same_renderer_ir");
        }
        if !self.same_graph {
            return Some("same_graph");
        }
        if !self.same_resources_bindings_pipelines_conceptually {
            return Some("same_resources_bindings_pipelines_conceptually");
        }
        if !self.replaces_only_bridge_realization {
            return Some("replaces_only_bridge_realization");
        }
        None
    }
}

// ============================================================================
// Section 3 — Direct DX12 proof-of-life stages
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier8DirectDx12ProofStage {
    #[default]
    DeviceQueueSwapchain,
    ImportProofSceneIr,
    ExecuteClearAndOpaque,
    Present,
    CompareAgainstWgpuDx12Baseline,
}

impl Tier8DirectDx12ProofStage {
    pub const ALL: [Self; TIER8_DIRECT_DX12_PROOF_STAGE_COUNT] = [
        Self::DeviceQueueSwapchain,
        Self::ImportProofSceneIr,
        Self::ExecuteClearAndOpaque,
        Self::Present,
        Self::CompareAgainstWgpuDx12Baseline,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::DeviceQueueSwapchain => 0,
            Self::ImportProofSceneIr => 1,
            Self::ExecuteClearAndOpaque => 2,
            Self::Present => 3,
            Self::CompareAgainstWgpuDx12Baseline => 4,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DeviceQueueSwapchain => "device_queue_swapchain",
            Self::ImportProofSceneIr => "import_proof_scene_ir",
            Self::ExecuteClearAndOpaque => "execute_clear_and_opaque",
            Self::Present => "present",
            Self::CompareAgainstWgpuDx12Baseline => "compare_against_wgpu_dx12_baseline",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier8DirectDx12StageStatus {
    #[default]
    NotYetAttempted,
    InProgress,
    Completed,
    Failed,
}

impl Tier8DirectDx12StageStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotYetAttempted => "not_yet_attempted",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    #[must_use]
    pub const fn is_completed(self) -> bool {
        matches!(self, Self::Completed)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier8DirectDx12StageRecord {
    pub stage: Tier8DirectDx12ProofStage,
    pub status: Tier8DirectDx12StageStatus,
    pub elapsed_ns: u64,
    pub failure_reason_hash: u64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Resource)]
pub struct Tier8DirectDx12ProofRecord {
    pub schema_version: u16,
    pub stages: [Tier8DirectDx12StageRecord; TIER8_DIRECT_DX12_PROOF_STAGE_COUNT],
}

impl Tier8DirectDx12ProofRecord {
    #[must_use]
    pub fn new() -> Self {
        let mut stages =
            [Tier8DirectDx12StageRecord::default(); TIER8_DIRECT_DX12_PROOF_STAGE_COUNT];
        for stage in Tier8DirectDx12ProofStage::ALL {
            stages[stage.index()] = Tier8DirectDx12StageRecord {
                stage,
                status: Tier8DirectDx12StageStatus::NotYetAttempted,
                elapsed_ns: 0,
                failure_reason_hash: 0,
            };
        }
        Self {
            schema_version: TIER8_DIRECT_BACKEND_EXPERIMENTS_SCHEMA_VERSION,
            stages,
        }
    }

    pub fn record_stage(
        &mut self,
        stage: Tier8DirectDx12ProofStage,
        status: Tier8DirectDx12StageStatus,
        elapsed_ns: u64,
        failure_reason_hash: u64,
    ) {
        self.stages[stage.index()] = Tier8DirectDx12StageRecord {
            stage,
            status,
            elapsed_ns,
            failure_reason_hash,
        };
    }

    #[must_use]
    pub const fn stage(&self, stage: Tier8DirectDx12ProofStage) -> Tier8DirectDx12StageRecord {
        self.stages[stage.index()]
    }

    /// Acceptance: every stage from `DeviceQueueSwapchain`
    /// through `CompareAgainstWgpuDx12Baseline` must be in
    /// `Completed` status. The user's "first direct-DX12 proof"
    /// requirement.
    #[must_use]
    pub fn all_stages_completed(&self) -> bool {
        self.stages.iter().all(|s| s.status.is_completed())
    }

    #[must_use]
    pub fn first_incomplete_stage(&self) -> Option<Tier8DirectDx12ProofStage> {
        Tier8DirectDx12ProofStage::ALL
            .into_iter()
            .find(|s| !self.stage(*s).status.is_completed())
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier8DirectDx12ComparisonResult {
    #[default]
    Pending,
    EquivalentToWgpuDx12Baseline,
    DivergedFromBaseline,
    FailedToProduce,
}

impl Tier8DirectDx12ComparisonResult {
    pub const ALL: [Self; TIER8_DIRECT_DX12_COMPARISON_RESULT_COUNT] = [
        Self::Pending,
        Self::EquivalentToWgpuDx12Baseline,
        Self::DivergedFromBaseline,
        Self::FailedToProduce,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::EquivalentToWgpuDx12Baseline => "equivalent_to_wgpu_dx12_baseline",
            Self::DivergedFromBaseline => "diverged_from_baseline",
            Self::FailedToProduce => "failed_to_produce",
        }
    }

    #[must_use]
    pub const fn passes(self) -> bool {
        matches!(self, Self::EquivalentToWgpuDx12Baseline)
    }
}

// ============================================================================
// Section 4 — Vulkan / Metal conformance
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier8ConformanceRunnerKind {
    #[default]
    Vulkan,
    Metal,
}

impl Tier8ConformanceRunnerKind {
    pub const ALL: [Self; TIER8_CONFORMANCE_RUNNER_KIND_COUNT] = [Self::Vulkan, Self::Metal];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Vulkan => "vulkan",
            Self::Metal => "metal",
        }
    }

    #[must_use]
    pub const fn corresponds_to_native_backend(self) -> NativeBackend {
        match self {
            Self::Vulkan => NativeBackend::Vulkan,
            Self::Metal => NativeBackend::Metal,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier8ConformanceCapabilityReport {
    pub schema_version: u16,
    pub runner_kind: Tier8ConformanceRunnerKind,
    pub mesh_shader_supported: bool,
    pub async_queue_overlap_supported: bool,
    pub bindless_descriptor_strategy_supported: bool,
    pub indirect_draw_count_supported: bool,
    pub timestamp_query_supported: bool,
    pub hdr_swapchain_format_supported: bool,
    pub backend_specific_extension_count: u16,
    pub frames_observed: u32,
}

impl Tier8ConformanceCapabilityReport {
    #[must_use]
    pub const fn for_runner(runner_kind: Tier8ConformanceRunnerKind) -> Self {
        Self {
            schema_version: TIER8_DIRECT_BACKEND_EXPERIMENTS_SCHEMA_VERSION,
            runner_kind,
            mesh_shader_supported: false,
            async_queue_overlap_supported: false,
            bindless_descriptor_strategy_supported: false,
            indirect_draw_count_supported: false,
            timestamp_query_supported: false,
            hdr_swapchain_format_supported: false,
            backend_specific_extension_count: 0,
            frames_observed: 0,
        }
    }

    /// Two reports are at parity only when *every* capability
    /// flag matches. The Tier 8 acceptance rule "honest
    /// capability status, not fake parity" is enforced by this
    /// strict equality predicate — any divergence in a single
    /// flag means the backends are NOT at parity, and a parity
    /// claim is faked.
    #[must_use]
    pub const fn at_strict_parity_with(&self, other: &Self) -> bool {
        self.mesh_shader_supported == other.mesh_shader_supported
            && self.async_queue_overlap_supported == other.async_queue_overlap_supported
            && self.bindless_descriptor_strategy_supported
                == other.bindless_descriptor_strategy_supported
            && self.indirect_draw_count_supported == other.indirect_draw_count_supported
            && self.timestamp_query_supported == other.timestamp_query_supported
            && self.hdr_swapchain_format_supported == other.hdr_swapchain_format_supported
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier8ConformanceVerdict {
    #[default]
    HonestCapabilityStatus,
    FakeParityClaimRejected,
    InsufficientFramesObserved,
    BackendInitFailed,
}

impl Tier8ConformanceVerdict {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HonestCapabilityStatus => "honest_capability_status",
            Self::FakeParityClaimRejected => "fake_parity_claim_rejected",
            Self::InsufficientFramesObserved => "insufficient_frames_observed",
            Self::BackendInitFailed => "backend_init_failed",
        }
    }

    #[must_use]
    pub const fn passes(self) -> bool {
        matches!(self, Self::HonestCapabilityStatus)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Resource)]
pub struct Tier8ConformanceRunner {
    pub schema_version: u16,
    pub kind: Tier8ConformanceRunnerKind,
    pub same_proof_scene: bool,
    pub same_ui_packet: bool,
    pub same_graph: bool,
    pub same_cache_policy: bool,
    pub capability_report: Tier8ConformanceCapabilityReport,
    pub verdict: Tier8ConformanceVerdict,
}

impl Tier8ConformanceRunner {
    /// The Tier 8 conformance contract. The runner must run the
    /// *same* proof scene, the *same* UI packet, the *same*
    /// graph, and the *same* cache policy as the wgpu-DX12
    /// baseline — otherwise the comparison is meaningless.
    #[must_use]
    pub fn evaluate(
        kind: Tier8ConformanceRunnerKind,
        capability_report: Tier8ConformanceCapabilityReport,
        same_proof_scene: bool,
        same_ui_packet: bool,
        same_graph: bool,
        same_cache_policy: bool,
        claimed_strict_parity_with_baseline: Option<&Tier8ConformanceCapabilityReport>,
    ) -> Self {
        let verdict = if !same_proof_scene || !same_ui_packet || !same_graph || !same_cache_policy {
            // The runner is not even comparable. Any status
            // claim is suspect.
            Tier8ConformanceVerdict::FakeParityClaimRejected
        } else if let Some(baseline) = claimed_strict_parity_with_baseline
            && !capability_report.at_strict_parity_with(baseline)
        {
            // The caller claimed parity, but the typed
            // capability flags say otherwise. Reject the claim.
            Tier8ConformanceVerdict::FakeParityClaimRejected
        } else if capability_report.frames_observed == 0 {
            Tier8ConformanceVerdict::InsufficientFramesObserved
        } else {
            Tier8ConformanceVerdict::HonestCapabilityStatus
        };
        Self {
            schema_version: TIER8_DIRECT_BACKEND_EXPERIMENTS_SCHEMA_VERSION,
            kind,
            same_proof_scene,
            same_ui_packet,
            same_graph,
            same_cache_policy,
            capability_report,
            verdict,
        }
    }

    #[must_use]
    pub const fn passes(&self) -> bool {
        self.verdict.passes()
    }
}

// ============================================================================
// Section 5 — Tier 8 acceptance verdict
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier8AcceptanceVerdict {
    pub schema_version: u16,
    pub passes_no_renderer_api_fork: bool,
    pub passes_direct_dx12_only_when_blocker_active: bool,
    pub passes_direct_dx12_proof_stages_completed: bool,
    pub passes_direct_dx12_comparison_against_baseline: bool,
    pub passes_conformance_honest_capability_status: bool,
    pub passes_no_fake_parity_claims: bool,
}

impl Tier8AcceptanceVerdict {
    /// The Tier 8 acceptance test. Inputs:
    ///
    /// - `scope` — the typed experimental scope record. The
    ///   "do not fork the renderer API" rule is encoded as
    ///   `is_honest_scope()`.
    /// - `start_gate` — the typed start gate record. Direct
    ///   DX12 may only start when at least one trigger is
    ///   active.
    /// - `proof` — the typed five-stage proof record. All
    ///   stages must be `Completed` for the experiment to
    ///   claim production-shaped status. The current state
    ///   may be partial; the verdict records progress.
    /// - `comparison` — the typed comparison result. Must be
    ///   `EquivalentToWgpuDx12Baseline` for the
    ///   compare-against-baseline rule.
    /// - `vulkan_runner`, `metal_runner` — typed conformance
    ///   runners. Both must produce
    ///   `HonestCapabilityStatus`; any `FakeParityClaimRejected`
    ///   fails the typed gate.
    #[must_use]
    pub fn evaluate(
        scope: &Tier8DirectDx12ExperimentalScope,
        start_gate: &Tier8DirectBackendStartGate,
        proof: &Tier8DirectDx12ProofRecord,
        comparison: Tier8DirectDx12ComparisonResult,
        vulkan_runner: &Tier8ConformanceRunner,
        metal_runner: &Tier8ConformanceRunner,
    ) -> Self {
        // Rule 1: do not fork the renderer API. The scope must
        // be honest.
        let passes_no_renderer_api_fork = scope.is_honest_scope();

        // Rule 2: direct DX12 may only start when at least one
        // trigger is active. The start gate enforces it.
        let passes_direct_dx12_only_when_blocker_active = start_gate.may_start();

        // Rule 3: proof stages completed. May be partial.
        let passes_direct_dx12_proof_stages_completed = proof.all_stages_completed();

        // Rule 4: comparison against baseline.
        let passes_direct_dx12_comparison_against_baseline = comparison.passes();

        // Rule 5: conformance runners produce honest capability
        // status, not fake parity.
        let conformance_honest = vulkan_runner.passes() && metal_runner.passes();
        let no_fake_parity = !matches!(
            vulkan_runner.verdict,
            Tier8ConformanceVerdict::FakeParityClaimRejected,
        ) && !matches!(
            metal_runner.verdict,
            Tier8ConformanceVerdict::FakeParityClaimRejected,
        );

        Self {
            schema_version: TIER8_DIRECT_BACKEND_EXPERIMENTS_SCHEMA_VERSION,
            passes_no_renderer_api_fork,
            passes_direct_dx12_only_when_blocker_active,
            passes_direct_dx12_proof_stages_completed,
            passes_direct_dx12_comparison_against_baseline,
            passes_conformance_honest_capability_status: conformance_honest,
            passes_no_fake_parity_claims: no_fake_parity,
        }
    }

    /// The full Tier 8 smoke gate. Returns true only when every
    /// rule holds.
    #[must_use]
    pub const fn passes(&self) -> bool {
        self.passes_no_renderer_api_fork
            && self.passes_direct_dx12_only_when_blocker_active
            && self.passes_direct_dx12_proof_stages_completed
            && self.passes_direct_dx12_comparison_against_baseline
            && self.passes_conformance_honest_capability_status
            && self.passes_no_fake_parity_claims
    }

    /// Honesty-only verdict — does not require the proof or
    /// comparison stages to be completed (they may be in
    /// progress). Useful for checking that nothing is being
    /// faked even when the experiment is partial.
    #[must_use]
    pub const fn passes_honesty_only(&self) -> bool {
        self.passes_no_renderer_api_fork
            && self.passes_direct_dx12_only_when_blocker_active
            && self.passes_no_fake_parity_claims
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_blockers() -> Tier8DirectBackendBlockerSet {
        Tier8DirectBackendBlockerSet::empty()
    }

    fn all_blockers() -> Tier8DirectBackendBlockerSet {
        let mut blockers = Tier8DirectBackendBlockerSet::empty();
        for b in Tier8DirectBackendBlocker::ALL {
            blockers.record(b);
        }
        blockers
    }

    fn baseline_capability() -> Tier8ConformanceCapabilityReport {
        Tier8ConformanceCapabilityReport {
            schema_version: TIER8_DIRECT_BACKEND_EXPERIMENTS_SCHEMA_VERSION,
            runner_kind: Tier8ConformanceRunnerKind::Vulkan,
            mesh_shader_supported: false,
            async_queue_overlap_supported: false,
            bindless_descriptor_strategy_supported: false,
            indirect_draw_count_supported: true,
            timestamp_query_supported: true,
            hdr_swapchain_format_supported: false,
            backend_specific_extension_count: 0,
            frames_observed: 30,
        }
    }

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(TIER8_DIRECT_BACKEND_EXPERIMENTS_SCHEMA_VERSION, 1);
        assert_eq!(TIER8_DIRECT_BACKEND_BLOCKER_COUNT, 7);
        assert_eq!(TIER8_DIRECT_DX12_PROOF_STAGE_COUNT, 5);
        assert_eq!(TIER8_CONFORMANCE_RUNNER_KIND_COUNT, 2);
        assert_eq!(TIER8_DIRECT_DX12_COMPARISON_RESULT_COUNT, 4);
        assert_eq!(TIER8_DIRECT_BACKEND_START_GATE_VERDICT_COUNT, 3);
    }

    #[test]
    fn direct_backend_blocker_dx12_specificity_routes_correctly() {
        for blocker in Tier8DirectBackendBlocker::ALL {
            match blocker {
                Tier8DirectBackendBlocker::EnhancedBarrierControl
                | Tier8DirectBackendBlocker::SamplerFeedback => {
                    assert!(blocker.requires_dx12_specific());
                }
                _ => assert!(!blocker.requires_dx12_specific()),
            }
        }
    }

    #[test]
    fn blocker_set_records_and_iterates_active_blockers() {
        let mut set = Tier8DirectBackendBlockerSet::empty();
        set.record(Tier8DirectBackendBlocker::DlssReflexStreamline);
        set.record(Tier8DirectBackendBlocker::MeshShaders);
        assert_eq!(set.active_count(), 2);
        assert!(set.contains(Tier8DirectBackendBlocker::DlssReflexStreamline));
        assert!(set.contains(Tier8DirectBackendBlocker::MeshShaders));
        assert!(!set.contains(Tier8DirectBackendBlocker::SamplerFeedback));
        let active: Vec<_> = set.active().collect();
        assert_eq!(active.len(), 2);
    }

    #[test]
    fn start_gate_must_not_start_when_no_blocker_active() {
        let gate = Tier8DirectBackendStartGate::evaluate(empty_blockers(), false);
        assert_eq!(
            gate.verdict,
            Tier8DirectBackendStartVerdict::MustNotStartNoTriggerActive,
        );
        assert!(!gate.may_start());
    }

    #[test]
    fn start_gate_must_not_start_when_bridge_already_adequate() {
        let gate = Tier8DirectBackendStartGate::evaluate(all_blockers(), true);
        assert_eq!(
            gate.verdict,
            Tier8DirectBackendStartVerdict::MustNotStartBridgeRequirementsAlreadyAdequate,
        );
        assert!(!gate.may_start());
    }

    #[test]
    fn start_gate_may_start_when_blocker_active_and_bridge_inadequate() {
        let mut blockers = Tier8DirectBackendBlockerSet::empty();
        blockers.record(Tier8DirectBackendBlocker::NativeCommandListAccess);
        let gate = Tier8DirectBackendStartGate::evaluate(blockers, false);
        assert_eq!(
            gate.verdict,
            Tier8DirectBackendStartVerdict::MayStartUnderActiveBlocker,
        );
        assert!(gate.may_start());
    }

    #[test]
    fn experimental_scope_honest_requires_every_flag_true() {
        assert!(Tier8DirectDx12ExperimentalScope::HONEST.is_honest_scope());
        assert!(
            Tier8DirectDx12ExperimentalScope::HONEST
                .first_violated_rule()
                .is_none()
        );
    }

    #[test]
    fn experimental_scope_forked_renderer_api_is_rejected() {
        let scope = Tier8DirectDx12ExperimentalScope::FORKED_RENDERER_API_REJECTED;
        assert!(!scope.is_honest_scope());
        // First violated rule walks in the order the scope rules
        // were declared.
        assert_eq!(scope.first_violated_rule(), Some("same_ecs_api"));
    }

    #[test]
    fn experimental_scope_partial_violation_returns_first_failed_rule() {
        let scope = Tier8DirectDx12ExperimentalScope {
            same_ecs_api: true,
            same_renderer_ir: true,
            same_graph: false,
            ..Tier8DirectDx12ExperimentalScope::HONEST
        };
        assert!(!scope.is_honest_scope());
        assert_eq!(scope.first_violated_rule(), Some("same_graph"));
    }

    #[test]
    fn proof_record_default_marks_every_stage_not_yet_attempted() {
        let proof = Tier8DirectDx12ProofRecord::new();
        for stage in Tier8DirectDx12ProofStage::ALL {
            assert_eq!(
                proof.stage(stage).status,
                Tier8DirectDx12StageStatus::NotYetAttempted,
            );
        }
        assert!(!proof.all_stages_completed());
        assert_eq!(
            proof.first_incomplete_stage(),
            Some(Tier8DirectDx12ProofStage::DeviceQueueSwapchain),
        );
    }

    #[test]
    fn proof_record_progresses_first_incomplete_through_pipeline() {
        let mut proof = Tier8DirectDx12ProofRecord::new();
        proof.record_stage(
            Tier8DirectDx12ProofStage::DeviceQueueSwapchain,
            Tier8DirectDx12StageStatus::Completed,
            1_000,
            0,
        );
        assert_eq!(
            proof.first_incomplete_stage(),
            Some(Tier8DirectDx12ProofStage::ImportProofSceneIr),
        );
    }

    #[test]
    fn proof_record_all_stages_completed_when_every_stage_completes() {
        let mut proof = Tier8DirectDx12ProofRecord::new();
        for stage in Tier8DirectDx12ProofStage::ALL {
            proof.record_stage(stage, Tier8DirectDx12StageStatus::Completed, 1_000, 0);
        }
        assert!(proof.all_stages_completed());
        assert!(proof.first_incomplete_stage().is_none());
    }

    #[test]
    fn proof_record_failed_stage_blocks_completion() {
        let mut proof = Tier8DirectDx12ProofRecord::new();
        proof.record_stage(
            Tier8DirectDx12ProofStage::DeviceQueueSwapchain,
            Tier8DirectDx12StageStatus::Failed,
            500,
            0xdead,
        );
        assert!(!proof.all_stages_completed());
        assert_eq!(
            proof.first_incomplete_stage(),
            Some(Tier8DirectDx12ProofStage::DeviceQueueSwapchain),
        );
    }

    #[test]
    fn comparison_result_passes_only_for_equivalent() {
        for result in Tier8DirectDx12ComparisonResult::ALL {
            match result {
                Tier8DirectDx12ComparisonResult::EquivalentToWgpuDx12Baseline => {
                    assert!(result.passes());
                }
                _ => assert!(!result.passes()),
            }
        }
    }

    #[test]
    fn conformance_runner_kind_routes_to_native_backend() {
        assert_eq!(
            Tier8ConformanceRunnerKind::Vulkan.corresponds_to_native_backend(),
            NativeBackend::Vulkan,
        );
        assert_eq!(
            Tier8ConformanceRunnerKind::Metal.corresponds_to_native_backend(),
            NativeBackend::Metal,
        );
    }

    #[test]
    fn capability_report_strict_parity_requires_every_flag_match() {
        let baseline = baseline_capability();
        let same = baseline;
        assert!(baseline.at_strict_parity_with(&same));
        let diverged = Tier8ConformanceCapabilityReport {
            mesh_shader_supported: true,
            ..baseline
        };
        assert!(!baseline.at_strict_parity_with(&diverged));
    }

    #[test]
    fn conformance_runner_rejects_fake_parity_when_capability_flags_diverge() {
        let baseline = baseline_capability();
        let diverged_report = Tier8ConformanceCapabilityReport {
            runner_kind: Tier8ConformanceRunnerKind::Metal,
            mesh_shader_supported: true,
            ..baseline
        };
        let runner = Tier8ConformanceRunner::evaluate(
            Tier8ConformanceRunnerKind::Metal,
            diverged_report,
            true,
            true,
            true,
            true,
            // Caller falsely claims parity with baseline despite
            // the typed capability divergence.
            Some(&baseline),
        );
        assert_eq!(
            runner.verdict,
            Tier8ConformanceVerdict::FakeParityClaimRejected,
        );
        assert!(!runner.passes());
    }

    #[test]
    fn conformance_runner_rejects_when_proof_scene_differs() {
        let runner = Tier8ConformanceRunner::evaluate(
            Tier8ConformanceRunnerKind::Vulkan,
            baseline_capability(),
            false, // different proof scene
            true,
            true,
            true,
            None,
        );
        assert_eq!(
            runner.verdict,
            Tier8ConformanceVerdict::FakeParityClaimRejected,
        );
    }

    #[test]
    fn conformance_runner_passes_with_honest_capability_status_and_zero_parity_claim() {
        let runner = Tier8ConformanceRunner::evaluate(
            Tier8ConformanceRunnerKind::Vulkan,
            baseline_capability(),
            true,
            true,
            true,
            true,
            None,
        );
        assert_eq!(
            runner.verdict,
            Tier8ConformanceVerdict::HonestCapabilityStatus
        );
        assert!(runner.passes());
    }

    #[test]
    fn conformance_runner_records_insufficient_frames_when_zero_observed() {
        let mut report = baseline_capability();
        report.frames_observed = 0;
        let runner = Tier8ConformanceRunner::evaluate(
            Tier8ConformanceRunnerKind::Vulkan,
            report,
            true,
            true,
            true,
            true,
            None,
        );
        assert_eq!(
            runner.verdict,
            Tier8ConformanceVerdict::InsufficientFramesObserved,
        );
    }

    #[test]
    fn tier8_acceptance_passes_under_full_passing_state() {
        let scope = Tier8DirectDx12ExperimentalScope::HONEST;
        let mut blockers = Tier8DirectBackendBlockerSet::empty();
        blockers.record(Tier8DirectBackendBlocker::EnhancedBarrierControl);
        let gate = Tier8DirectBackendStartGate::evaluate(blockers, false);
        let mut proof = Tier8DirectDx12ProofRecord::new();
        for stage in Tier8DirectDx12ProofStage::ALL {
            proof.record_stage(stage, Tier8DirectDx12StageStatus::Completed, 1_000, 0);
        }
        let comparison = Tier8DirectDx12ComparisonResult::EquivalentToWgpuDx12Baseline;
        let vulkan_runner = Tier8ConformanceRunner::evaluate(
            Tier8ConformanceRunnerKind::Vulkan,
            baseline_capability(),
            true,
            true,
            true,
            true,
            None,
        );
        let metal_runner = Tier8ConformanceRunner::evaluate(
            Tier8ConformanceRunnerKind::Metal,
            Tier8ConformanceCapabilityReport {
                runner_kind: Tier8ConformanceRunnerKind::Metal,
                ..baseline_capability()
            },
            true,
            true,
            true,
            true,
            None,
        );
        let verdict = Tier8AcceptanceVerdict::evaluate(
            &scope,
            &gate,
            &proof,
            comparison,
            &vulkan_runner,
            &metal_runner,
        );
        assert!(verdict.passes_no_renderer_api_fork);
        assert!(verdict.passes_direct_dx12_only_when_blocker_active);
        assert!(verdict.passes_direct_dx12_proof_stages_completed);
        assert!(verdict.passes_direct_dx12_comparison_against_baseline);
        assert!(verdict.passes_conformance_honest_capability_status);
        assert!(verdict.passes_no_fake_parity_claims);
        assert!(verdict.passes());
    }

    #[test]
    fn tier8_acceptance_fails_when_renderer_api_is_forked() {
        let scope = Tier8DirectDx12ExperimentalScope::FORKED_RENDERER_API_REJECTED;
        let mut blockers = Tier8DirectBackendBlockerSet::empty();
        blockers.record(Tier8DirectBackendBlocker::EnhancedBarrierControl);
        let gate = Tier8DirectBackendStartGate::evaluate(blockers, false);
        let proof = Tier8DirectDx12ProofRecord::new();
        let vulkan_runner = Tier8ConformanceRunner::evaluate(
            Tier8ConformanceRunnerKind::Vulkan,
            baseline_capability(),
            true,
            true,
            true,
            true,
            None,
        );
        let metal_runner = vulkan_runner;
        let verdict = Tier8AcceptanceVerdict::evaluate(
            &scope,
            &gate,
            &proof,
            Tier8DirectDx12ComparisonResult::Pending,
            &vulkan_runner,
            &metal_runner,
        );
        assert!(!verdict.passes_no_renderer_api_fork);
        assert!(!verdict.passes());
    }

    #[test]
    fn tier8_acceptance_fails_when_no_blocker_triggers_direct_dx12() {
        let scope = Tier8DirectDx12ExperimentalScope::HONEST;
        let gate = Tier8DirectBackendStartGate::evaluate(empty_blockers(), false);
        let proof = Tier8DirectDx12ProofRecord::new();
        let vulkan_runner = Tier8ConformanceRunner::evaluate(
            Tier8ConformanceRunnerKind::Vulkan,
            baseline_capability(),
            true,
            true,
            true,
            true,
            None,
        );
        let metal_runner = vulkan_runner;
        let verdict = Tier8AcceptanceVerdict::evaluate(
            &scope,
            &gate,
            &proof,
            Tier8DirectDx12ComparisonResult::Pending,
            &vulkan_runner,
            &metal_runner,
        );
        assert!(!verdict.passes_direct_dx12_only_when_blocker_active);
    }

    #[test]
    fn tier8_acceptance_fails_when_fake_parity_claim_detected() {
        let scope = Tier8DirectDx12ExperimentalScope::HONEST;
        let mut blockers = Tier8DirectBackendBlockerSet::empty();
        blockers.record(Tier8DirectBackendBlocker::DlssReflexStreamline);
        let gate = Tier8DirectBackendStartGate::evaluate(blockers, false);
        let proof = Tier8DirectDx12ProofRecord::new();
        // Vulkan runner falsely claims parity with a divergent
        // Metal capability report.
        let baseline = baseline_capability();
        let metal_diverged = Tier8ConformanceCapabilityReport {
            runner_kind: Tier8ConformanceRunnerKind::Metal,
            mesh_shader_supported: true,
            ..baseline
        };
        let vulkan_runner = Tier8ConformanceRunner::evaluate(
            Tier8ConformanceRunnerKind::Vulkan,
            baseline,
            true,
            true,
            true,
            true,
            Some(&metal_diverged),
        );
        let metal_runner = Tier8ConformanceRunner::evaluate(
            Tier8ConformanceRunnerKind::Metal,
            metal_diverged,
            true,
            true,
            true,
            true,
            None,
        );
        let verdict = Tier8AcceptanceVerdict::evaluate(
            &scope,
            &gate,
            &proof,
            Tier8DirectDx12ComparisonResult::Pending,
            &vulkan_runner,
            &metal_runner,
        );
        assert!(!verdict.passes_no_fake_parity_claims);
        assert!(!verdict.passes());
    }

    #[test]
    fn tier8_honesty_only_passes_under_partial_proof_state() {
        // Honesty-only verdict does not require the proof or
        // comparison stages to be complete — useful for in-progress
        // experiments.
        let scope = Tier8DirectDx12ExperimentalScope::HONEST;
        let mut blockers = Tier8DirectBackendBlockerSet::empty();
        blockers.record(Tier8DirectBackendBlocker::SamplerFeedback);
        let gate = Tier8DirectBackendStartGate::evaluate(blockers, false);
        let proof = Tier8DirectDx12ProofRecord::new();
        let vulkan_runner = Tier8ConformanceRunner::evaluate(
            Tier8ConformanceRunnerKind::Vulkan,
            baseline_capability(),
            true,
            true,
            true,
            true,
            None,
        );
        let metal_runner = vulkan_runner;
        let verdict = Tier8AcceptanceVerdict::evaluate(
            &scope,
            &gate,
            &proof,
            Tier8DirectDx12ComparisonResult::Pending,
            &vulkan_runner,
            &metal_runner,
        );
        assert!(verdict.passes_honesty_only());
        // Full smoke gate still fails because proof + comparison
        // are pending.
        assert!(!verdict.passes());
    }
}
