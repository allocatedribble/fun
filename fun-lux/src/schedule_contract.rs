//! Tier-6 Lux schedule and liveness contract.
//!
//! `fun-lux` owns lighting policy and resource availability. This
//! module lets Lux emit scheduler-visible work graphs without taking
//! a renderer dependency or naming backend handles.

use core::fmt;
use core::marker::PhantomData;

use fun_scheduler_types::class::TaskPriority;
use fun_scheduler_types::schedule::{
    ScheduleBudget, ScheduleDeadline, ScheduleDomain, ScheduleLane,
};
use fun_scheduler_types::work_graph::{
    CommitPolicy, DeterministicDescriptor, GraphExecutionMode, GraphInvariantError, LiveWorkGraph,
    RuntimeLiveProof, WaitForEdge, WaitForEdgeKind, WorkEdge, WorkGraph, WorkGraphId, WorkNode,
    WorkNodeId, WorkNodeLiveness, WorkPhase, WorkWaitToken,
};

use crate::frame_plan::LuxResourceIntentKind;

/// Scheduler-visible Lux work taxonomy.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Ord, PartialOrd)]
#[repr(u8)]
#[non_exhaustive]
pub enum LuxWorkKind {
    /// Build the direct-light list.
    DirectLightListBuild = 0,
    /// Prepare many-light sampling inputs.
    ManyLightSamplePrepare = 1,
    /// Assign lights to clusters.
    ClusteredLightAssign = 2,
    /// Cull shadow casters.
    ShadowCasterCull = 3,
    /// Classify virtual shadow pages.
    VirtualShadowPageClassify = 4,
    /// Render virtual shadow pages.
    VirtualShadowPageRender = 5,
    /// Update GI probes.
    GiProbeUpdate = 6,
    /// Update reflection probes.
    ReflectionProbeUpdate = 7,
    /// Prepare denoiser inputs/history.
    DenoisePrepare = 8,
    /// Prepare temporal reconstruction inputs/history.
    TemporalReconstructionPrepare = 9,
    /// Prepare volumetric lighting inputs.
    VolumetricPrepare = 10,
    /// Maintain Lux cache residency.
    LuxCacheResidency = 11,
    /// Emit Lux diagnostics.
    LuxDiagnostics = 12,
}

impl LuxWorkKind {
    /// Stable label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::DirectLightListBuild => "direct_light_list_build",
            Self::ManyLightSamplePrepare => "many_light_sample_prepare",
            Self::ClusteredLightAssign => "clustered_light_assign",
            Self::ShadowCasterCull => "shadow_caster_cull",
            Self::VirtualShadowPageClassify => "virtual_shadow_page_classify",
            Self::VirtualShadowPageRender => "virtual_shadow_page_render",
            Self::GiProbeUpdate => "gi_probe_update",
            Self::ReflectionProbeUpdate => "reflection_probe_update",
            Self::DenoisePrepare => "denoise_prepare",
            Self::TemporalReconstructionPrepare => "temporal_reconstruction_prepare",
            Self::VolumetricPrepare => "volumetric_prepare",
            Self::LuxCacheResidency => "lux_cache_residency",
            Self::LuxDiagnostics => "lux_diagnostics",
        }
    }

    /// Every variant in canonical declaration order.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::DirectLightListBuild,
            Self::ManyLightSamplePrepare,
            Self::ClusteredLightAssign,
            Self::ShadowCasterCull,
            Self::VirtualShadowPageClassify,
            Self::VirtualShadowPageRender,
            Self::GiProbeUpdate,
            Self::ReflectionProbeUpdate,
            Self::DenoisePrepare,
            Self::TemporalReconstructionPrepare,
            Self::VolumetricPrepare,
            Self::LuxCacheResidency,
            Self::LuxDiagnostics,
        ]
    }

    /// Scheduler lane projection.
    #[must_use]
    pub const fn lane(self) -> ScheduleLane {
        match self {
            Self::DirectLightListBuild
            | Self::ManyLightSamplePrepare
            | Self::ClusteredLightAssign
            | Self::ShadowCasterCull
            | Self::VirtualShadowPageClassify
            | Self::VirtualShadowPageRender
            | Self::GiProbeUpdate
            | Self::ReflectionProbeUpdate
            | Self::DenoisePrepare
            | Self::TemporalReconstructionPrepare
            | Self::VolumetricPrepare => ScheduleLane::RenderPrepare,
            Self::LuxCacheResidency => ScheduleLane::ResourceBackground,
            Self::LuxDiagnostics => ScheduleLane::DiagnosticsLowPriority,
        }
    }

    /// Coarse work phase.
    #[must_use]
    pub const fn phase(self) -> WorkPhase {
        match self {
            Self::LuxDiagnostics => WorkPhase::Telemetry,
            Self::LuxCacheResidency => WorkPhase::Setup,
            _ => WorkPhase::Compute,
        }
    }

    /// Whether the pass is optional and shed-safe.
    #[must_use]
    pub const fn optional(self) -> bool {
        matches!(
            self,
            Self::LuxDiagnostics
                | Self::LuxCacheResidency
                | Self::GiProbeUpdate
                | Self::ReflectionProbeUpdate
                | Self::DenoisePrepare
                | Self::TemporalReconstructionPrepare
                | Self::VolumetricPrepare
        )
    }
}

/// Runtime Lux plan branch.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
#[non_exhaustive]
pub enum LuxPlanBranch {
    /// Lights changed this frame.
    LightsChanged,
    /// Lights did not change this frame.
    LightsUnchanged,
    /// Static-lighting branch.
    StaticLighting,
    /// Dynamic-lighting branch.
    #[default]
    DynamicLighting,
}

impl LuxPlanBranch {
    /// Stable label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::LightsChanged => "lights_changed",
            Self::LightsUnchanged => "lights_unchanged",
            Self::StaticLighting => "static_lighting",
            Self::DynamicLighting => "dynamic_lighting",
        }
    }
}

/// Compile-time Lux plan typestate.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct LuxPlan<State> {
    /// Frame index.
    pub frame_index: u64,
    _state: PhantomData<State>,
}

impl<State> LuxPlan<State> {
    #[must_use]
    pub const fn new(frame_index: u64) -> Self {
        Self {
            frame_index,
            _state: PhantomData,
        }
    }
}

/// Lights-changed plan marker.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct LightsChanged;
/// Lights-unchanged plan marker.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct LightsUnchanged;
/// Static-lighting plan marker.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct StaticLighting;
/// Dynamic-lighting plan marker.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct DynamicLighting;

/// Compile-time branch mapping.
pub trait LuxPlanState {
    /// Runtime branch represented by this typestate.
    const BRANCH: LuxPlanBranch;
}

impl LuxPlanState for LightsChanged {
    const BRANCH: LuxPlanBranch = LuxPlanBranch::LightsChanged;
}

impl LuxPlanState for LightsUnchanged {
    const BRANCH: LuxPlanBranch = LuxPlanBranch::LightsUnchanged;
}

impl LuxPlanState for StaticLighting {
    const BRANCH: LuxPlanBranch = LuxPlanBranch::StaticLighting;
}

impl LuxPlanState for DynamicLighting {
    const BRANCH: LuxPlanBranch = LuxPlanBranch::DynamicLighting;
}

impl<State: LuxPlanState> LuxPlan<State> {
    /// Runtime branch for this static plan.
    #[must_use]
    pub const fn branch(&self) -> LuxPlanBranch {
        State::BRANCH
    }
}

/// Resource condition used by conditional resource manifests.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[non_exhaustive]
pub enum LuxResourceCondition {
    /// Lights changed.
    LightsChanged,
    /// Lights did not change.
    LightsUnchanged,
    /// Dynamic lighting is active.
    DynamicLighting,
    /// Static lighting is active.
    StaticLighting,
    /// Virtual shadows are active.
    VirtualShadows,
    /// GI is active.
    GlobalIllumination,
    /// Reflection probes are active.
    Reflections,
    /// Volumetrics are active.
    Volumetrics,
}

impl LuxResourceCondition {
    /// Stable label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::LightsChanged => "lights_changed",
            Self::LightsUnchanged => "lights_unchanged",
            Self::DynamicLighting => "dynamic_lighting",
            Self::StaticLighting => "static_lighting",
            Self::VirtualShadows => "virtual_shadows",
            Self::GlobalIllumination => "global_illumination",
            Self::Reflections => "reflections",
            Self::Volumetrics => "volumetrics",
        }
    }

    /// Whether this condition is active under a runtime branch.
    #[must_use]
    pub const fn active_in(self, branch: LuxPlanBranch) -> bool {
        matches!(
            (self, branch),
            (Self::LightsChanged, LuxPlanBranch::LightsChanged)
                | (Self::LightsUnchanged, LuxPlanBranch::LightsUnchanged)
                | (Self::DynamicLighting, LuxPlanBranch::DynamicLighting)
                | (Self::StaticLighting, LuxPlanBranch::StaticLighting)
        ) || !matches!(
            self,
            Self::LightsChanged
                | Self::LightsUnchanged
                | Self::DynamicLighting
                | Self::StaticLighting
        )
    }
}

/// Compile-time mapping for conditional Lux resources.
pub trait LuxResourceConditionState {
    /// Runtime condition represented by this marker.
    const CONDITION: LuxResourceCondition;
}

impl LuxResourceConditionState for LightsChanged {
    const CONDITION: LuxResourceCondition = LuxResourceCondition::LightsChanged;
}

impl LuxResourceConditionState for LightsUnchanged {
    const CONDITION: LuxResourceCondition = LuxResourceCondition::LightsUnchanged;
}

impl LuxResourceConditionState for StaticLighting {
    const CONDITION: LuxResourceCondition = LuxResourceCondition::StaticLighting;
}

impl LuxResourceConditionState for DynamicLighting {
    const CONDITION: LuxResourceCondition = LuxResourceCondition::DynamicLighting;
}

/// Virtual-shadow condition marker.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct VirtualShadowsActive;
/// Global-illumination condition marker.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct GlobalIlluminationActive;
/// Reflections condition marker.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct ReflectionsActive;
/// Volumetrics condition marker.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct VolumetricsActive;

impl LuxResourceConditionState for VirtualShadowsActive {
    const CONDITION: LuxResourceCondition = LuxResourceCondition::VirtualShadows;
}

impl LuxResourceConditionState for GlobalIlluminationActive {
    const CONDITION: LuxResourceCondition = LuxResourceCondition::GlobalIllumination;
}

impl LuxResourceConditionState for ReflectionsActive {
    const CONDITION: LuxResourceCondition = LuxResourceCondition::Reflections;
}

impl LuxResourceConditionState for VolumetricsActive {
    const CONDITION: LuxResourceCondition = LuxResourceCondition::Volumetrics;
}

/// Always-available resource manifest wrapper.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct AlwaysResource<R>(pub R);
/// Imported renderer resource manifest wrapper.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct ImportedResource<R>(pub R);
/// Conditional resource manifest wrapper.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct ConditionalResource<R, Condition> {
    /// Resource.
    pub resource: R,
    _condition: PhantomData<Condition>,
}
/// Backfilled resource manifest wrapper.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct BackfilledResource<R>(pub R);
/// Optional resource manifest wrapper.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct OptionalResource<R>(pub R);

impl<R, Condition> ConditionalResource<R, Condition> {
    /// Construct a conditional resource.
    #[must_use]
    pub const fn new(resource: R) -> Self {
        Self {
            resource,
            _condition: PhantomData,
        }
    }
}

impl AlwaysResource<LuxResourceIntentKind> {
    /// Convert this typed wrapper into a runtime manifest row.
    #[must_use]
    pub const fn manifest(self) -> LuxResourceManifest {
        LuxResourceManifest::always(self.0)
    }
}

impl ImportedResource<LuxResourceIntentKind> {
    /// Convert this typed wrapper into a runtime manifest row.
    #[must_use]
    pub const fn manifest(self) -> LuxResourceManifest {
        LuxResourceManifest::imported(self.0)
    }
}

impl<Condition: LuxResourceConditionState> ConditionalResource<LuxResourceIntentKind, Condition> {
    /// Convert this typed wrapper into a runtime manifest row.
    #[must_use]
    pub const fn manifest(self) -> LuxResourceManifest {
        LuxResourceManifest::conditional(self.resource, Condition::CONDITION)
    }
}

impl BackfilledResource<LuxResourceIntentKind> {
    /// Convert this typed wrapper into a runtime manifest row.
    #[must_use]
    pub const fn manifest(self) -> LuxResourceManifest {
        LuxResourceManifest::backfilled(self.0)
    }
}

impl OptionalResource<LuxResourceIntentKind> {
    /// Convert this typed wrapper into a runtime manifest row.
    #[must_use]
    pub const fn manifest(self) -> LuxResourceManifest {
        LuxResourceManifest::optional(self.0)
    }
}

/// Runtime resource availability declared by the Lux plan.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[non_exhaustive]
pub enum LuxResourceAvailability {
    /// Always produced by Lux.
    Always,
    /// Imported from renderer or a cross-domain handoff.
    Imported,
    /// Produced only when the named condition is active.
    Conditional(LuxResourceCondition),
    /// Conditional/optional path has a deterministic backfill.
    Backfilled,
    /// Optional resource.
    Optional,
}

/// One typed resource manifest row.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct LuxResourceManifest {
    /// Resource kind.
    pub resource: LuxResourceIntentKind,
    /// Availability class.
    pub availability: LuxResourceAvailability,
    /// Explicit fallback/backfill resource, if any.
    pub fallback: Option<LuxResourceIntentKind>,
}

impl LuxResourceManifest {
    /// Always-available resource.
    #[must_use]
    pub const fn always(resource: LuxResourceIntentKind) -> Self {
        Self {
            resource,
            availability: LuxResourceAvailability::Always,
            fallback: None,
        }
    }

    /// Imported resource.
    #[must_use]
    pub const fn imported(resource: LuxResourceIntentKind) -> Self {
        Self {
            resource,
            availability: LuxResourceAvailability::Imported,
            fallback: None,
        }
    }

    /// Conditional resource.
    #[must_use]
    pub const fn conditional(
        resource: LuxResourceIntentKind,
        condition: LuxResourceCondition,
    ) -> Self {
        Self {
            resource,
            availability: LuxResourceAvailability::Conditional(condition),
            fallback: None,
        }
    }

    /// Backfilled resource.
    #[must_use]
    pub const fn backfilled(resource: LuxResourceIntentKind) -> Self {
        Self {
            resource,
            availability: LuxResourceAvailability::Backfilled,
            fallback: Some(resource),
        }
    }

    /// Optional resource.
    #[must_use]
    pub const fn optional(resource: LuxResourceIntentKind) -> Self {
        Self {
            resource,
            availability: LuxResourceAvailability::Optional,
            fallback: None,
        }
    }

    /// Attach an explicit fallback resource.
    #[must_use]
    pub const fn with_fallback(mut self, fallback: LuxResourceIntentKind) -> Self {
        self.fallback = Some(fallback);
        self
    }
}

/// One resource read by a Lux pass.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct LuxResourceRead {
    /// Resource kind read.
    pub resource: LuxResourceIntentKind,
    /// Condition guarding this read.
    pub guard: Option<LuxResourceCondition>,
    /// Explicit fallback/backfill resource.
    pub fallback: Option<LuxResourceIntentKind>,
}

impl LuxResourceRead {
    /// Unguarded read.
    #[must_use]
    pub const fn new(resource: LuxResourceIntentKind) -> Self {
        Self {
            resource,
            guard: None,
            fallback: None,
        }
    }

    /// Guard this read by a condition.
    #[must_use]
    pub const fn guarded(mut self, condition: LuxResourceCondition) -> Self {
        self.guard = Some(condition);
        self
    }

    /// Add an explicit fallback/backfill resource.
    #[must_use]
    pub const fn with_fallback(mut self, fallback: LuxResourceIntentKind) -> Self {
        self.fallback = Some(fallback);
        self
    }
}

/// One resource write by a Lux pass.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct LuxResourceWrite {
    /// Resource kind written.
    pub resource: LuxResourceIntentKind,
}

impl LuxResourceWrite {
    /// Construct a write descriptor.
    #[must_use]
    pub const fn new(resource: LuxResourceIntentKind) -> Self {
        Self { resource }
    }
}

/// Renderer phase a Lux node may try to wait on.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[non_exhaustive]
pub enum LuxRendererWait {
    /// Renderer graph build.
    GraphBuild,
    /// Renderer graph compile.
    GraphCompile,
    /// Renderer command recording.
    Record,
    /// Renderer queue submit.
    Submit,
    /// Renderer present.
    Present,
}

impl LuxRendererWait {
    /// Stable label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::GraphBuild => "graph_build",
            Self::GraphCompile => "graph_compile",
            Self::Record => "record",
            Self::Submit => "submit",
            Self::Present => "present",
        }
    }

    /// Renderer pipeline order.
    #[must_use]
    pub const fn order(self) -> u8 {
        match self {
            Self::GraphBuild => 0,
            Self::GraphCompile => 1,
            Self::Record => 2,
            Self::Submit => 3,
            Self::Present => 4,
        }
    }

    /// Whether Lux may wait on this renderer phase.
    #[must_use]
    pub const fn is_forbidden_lux_wait(self) -> bool {
        matches!(self, Self::Record | Self::Submit | Self::Present)
    }

    /// Whether Lux outputs may be consumed by this renderer phase.
    #[must_use]
    pub const fn is_valid_lux_output_consumer(self) -> bool {
        matches!(self, Self::GraphCompile | Self::Record)
    }
}

/// Lux cache access discipline.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
#[non_exhaustive]
pub enum LuxCacheAccess {
    /// No cache mutation.
    #[default]
    None,
    /// Uses a versioned snapshot while waiting.
    VersionedSnapshot { version: u64 },
    /// Holds a shared mutation lock. This cannot wait on scheduler work.
    SharedMutationLock,
    /// Mutually-exclusive mutation expressed as a scheduler conflict set.
    ConflictSet,
}

/// One scheduler-visible Lux work item.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct LuxWork {
    /// Work kind.
    pub kind: LuxWorkKind,
    /// Resource reads.
    pub reads: Vec<LuxResourceRead>,
    /// Resource writes.
    pub writes: Vec<LuxResourceWrite>,
    /// Runtime condition guarding this node.
    pub condition: Option<LuxResourceCondition>,
    /// Whether the node is optional and shed-safe.
    pub optional: bool,
    /// Renderer phase wait, if declared.
    pub renderer_wait: Option<LuxRendererWait>,
    /// Renderer phase that consumes this Lux output, if declared.
    pub renderer_consumer: Option<LuxRendererWait>,
    /// Cache access discipline.
    pub cache_access: LuxCacheAccess,
    /// Scheduler wait tokens this node awaits.
    pub awaited_tokens: Vec<WorkWaitToken>,
    /// Scheduler wait tokens this node produces.
    pub produced_tokens: Vec<WorkWaitToken>,
}

impl LuxWork {
    /// Construct from a work kind.
    #[must_use]
    pub fn new(kind: LuxWorkKind) -> Self {
        Self {
            kind,
            reads: Vec::new(),
            writes: Vec::new(),
            condition: None,
            optional: kind.optional(),
            renderer_wait: None,
            renderer_consumer: None,
            cache_access: LuxCacheAccess::None,
            awaited_tokens: Vec::new(),
            produced_tokens: Vec::new(),
        }
    }

    /// Add a resource read.
    #[must_use]
    pub fn with_read(mut self, read: LuxResourceRead) -> Self {
        self.reads.push(read);
        self
    }

    /// Add a resource write.
    #[must_use]
    pub fn with_write(mut self, write: LuxResourceWrite) -> Self {
        self.writes.push(write);
        self
    }

    /// Guard this work by a condition.
    #[must_use]
    pub fn with_condition(mut self, condition: LuxResourceCondition) -> Self {
        self.condition = Some(condition);
        self
    }

    /// Override optionality.
    #[must_use]
    pub const fn with_optional(mut self, optional: bool) -> Self {
        self.optional = optional;
        self
    }

    /// Declare a renderer wait.
    #[must_use]
    pub const fn with_renderer_wait(mut self, wait: LuxRendererWait) -> Self {
        self.renderer_wait = Some(wait);
        self
    }

    /// Declare the renderer phase that consumes this Lux output.
    #[must_use]
    pub const fn with_renderer_consumer(mut self, phase: LuxRendererWait) -> Self {
        self.renderer_consumer = Some(phase);
        self
    }

    /// Set cache access discipline.
    #[must_use]
    pub const fn with_cache_access(mut self, access: LuxCacheAccess) -> Self {
        self.cache_access = access;
        self
    }

    /// Add an awaited scheduler token.
    #[must_use]
    pub fn with_awaited_token(mut self, token: WorkWaitToken) -> Self {
        self.awaited_tokens.push(token);
        self
    }

    /// Add a produced scheduler token.
    #[must_use]
    pub fn with_produced_token(mut self, token: WorkWaitToken) -> Self {
        self.produced_tokens.push(token);
        self
    }

    fn liveness(&self) -> WorkNodeLiveness {
        let mut liveness = WorkNodeLiveness::new()
            .with_may_wait(!self.awaited_tokens.is_empty() || self.renderer_wait.is_some());
        for token in &self.awaited_tokens {
            liveness = liveness.with_awaited_token(*token);
        }
        for token in &self.produced_tokens {
            liveness = liveness.with_produced_token(*token);
        }
        if matches!(self.cache_access, LuxCacheAccess::SharedMutationLock) {
            liveness = liveness.with_external_state(true);
        }
        liveness
    }
}

/// Lux work graph alias.
pub type LuxWorkGraph = WorkGraph<LuxWork>;
/// Runtime-proven Lux schedule.
pub type RuntimeLuxSchedule = LiveWorkGraph<LuxWork, RuntimeLiveProof>;

/// Lux schedule validation failure.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
#[non_exhaustive]
pub enum LuxScheduleError {
    /// Referenced resource is unavailable.
    ResourceUnavailable {
        /// Node.
        node: WorkNodeId,
        /// Resource.
        resource: LuxResourceIntentKind,
    },
    /// Conditional resource read is neither guarded nor backfilled.
    ConditionalResourceUnguarded {
        /// Node.
        node: WorkNodeId,
        /// Resource.
        resource: LuxResourceIntentKind,
        /// Required condition.
        condition: LuxResourceCondition,
    },
    /// Conditional resource is inactive for the selected plan branch
    /// and has no fallback/backfill.
    ConditionalResourceInactive {
        /// Node.
        node: WorkNodeId,
        /// Resource.
        resource: LuxResourceIntentKind,
        /// Required condition.
        condition: LuxResourceCondition,
        /// Runtime branch.
        branch: LuxPlanBranch,
    },
    /// Plan branch mismatch.
    PlanBranchMismatch {
        /// Node.
        node: WorkNodeId,
        /// Condition.
        condition: LuxResourceCondition,
        /// Branch.
        branch: LuxPlanBranch,
    },
    /// Lux waited on a forbidden renderer phase.
    LuxWaitsOnRendererPhase {
        /// Node.
        node: WorkNodeId,
        /// Phase.
        phase: LuxRendererWait,
    },
    /// Lux output targets a renderer phase that cannot consume Lux.
    InvalidRendererConsumerPhase {
        /// Node.
        node: WorkNodeId,
        /// Phase.
        phase: LuxRendererWait,
    },
    /// Lux output feeds a renderer phase that is also waited on by
    /// later Lux work.
    LuxRendererCycle {
        /// Lux producer feeding renderer.
        producer: WorkNodeId,
        /// Renderer phase.
        phase: LuxRendererWait,
        /// Lux node waiting on that renderer phase or a later phase.
        waiter: WorkNodeId,
    },
    /// Optional Lux work gates direct lighting.
    OptionalBlocksDirectLighting {
        /// Optional node.
        optional: WorkNodeId,
        /// Direct-lighting node.
        direct: WorkNodeId,
    },
    /// Optional Lux work gates a renderer phase on the present path.
    OptionalBlocksPresent {
        /// Optional node.
        optional: WorkNodeId,
        /// Renderer phase.
        phase: LuxRendererWait,
    },
    /// Cache mutation lock waits on scheduler work.
    CacheLockWaits {
        /// Node.
        node: WorkNodeId,
    },
    /// Lux cache residency work did not use a versioned snapshot or
    /// scheduler-visible conflict set.
    CacheRequiresSnapshotOrConflictSet {
        /// Node.
        node: WorkNodeId,
    },
    /// Generic graph invariant failed.
    GraphInvariant(GraphInvariantError),
}

impl LuxScheduleError {
    /// Stable label.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::ResourceUnavailable { .. } => "resource_unavailable",
            Self::ConditionalResourceUnguarded { .. } => "conditional_resource_unguarded",
            Self::ConditionalResourceInactive { .. } => "conditional_resource_inactive",
            Self::PlanBranchMismatch { .. } => "plan_branch_mismatch",
            Self::LuxWaitsOnRendererPhase { .. } => "lux_waits_on_renderer_phase",
            Self::InvalidRendererConsumerPhase { .. } => "invalid_renderer_consumer_phase",
            Self::LuxRendererCycle { .. } => "lux_renderer_cycle",
            Self::OptionalBlocksDirectLighting { .. } => "optional_blocks_direct_lighting",
            Self::OptionalBlocksPresent { .. } => "optional_blocks_present",
            Self::CacheLockWaits { .. } => "cache_lock_waits",
            Self::CacheRequiresSnapshotOrConflictSet { .. } => {
                "cache_requires_snapshot_or_conflict_set"
            }
            Self::GraphInvariant(_) => "graph_invariant",
        }
    }
}

impl fmt::Display for LuxScheduleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::ResourceUnavailable { node, resource } => write!(
                f,
                "Lux node {} reads unavailable resource {}",
                node.get(),
                resource.as_str()
            ),
            Self::ConditionalResourceUnguarded {
                node,
                resource,
                condition,
            } => write!(
                f,
                "Lux node {} reads conditional resource {} without guard {} or fallback",
                node.get(),
                resource.as_str(),
                condition.label()
            ),
            Self::ConditionalResourceInactive {
                node,
                resource,
                condition,
                branch,
            } => write!(
                f,
                "Lux node {} reads inactive conditional resource {} guarded by {} under branch {} without fallback",
                node.get(),
                resource.as_str(),
                condition.label(),
                branch.label()
            ),
            Self::PlanBranchMismatch {
                node,
                condition,
                branch,
            } => write!(
                f,
                "Lux node {} condition {} is inactive for branch {}",
                node.get(),
                condition.label(),
                branch.label()
            ),
            Self::LuxWaitsOnRendererPhase { node, phase } => write!(
                f,
                "Lux node {} waits on forbidden renderer phase {}",
                node.get(),
                phase.label()
            ),
            Self::InvalidRendererConsumerPhase { node, phase } => write!(
                f,
                "Lux node {} targets invalid renderer consumer phase {}",
                node.get(),
                phase.label()
            ),
            Self::LuxRendererCycle {
                producer,
                phase,
                waiter,
            } => write!(
                f,
                "Lux producer {} feeds renderer phase {} while Lux node {} waits on that phase or later",
                producer.get(),
                phase.label(),
                waiter.get()
            ),
            Self::OptionalBlocksDirectLighting { optional, direct } => write!(
                f,
                "optional Lux node {} blocks direct lighting node {}",
                optional.get(),
                direct.get()
            ),
            Self::OptionalBlocksPresent { optional, phase } => write!(
                f,
                "optional Lux node {} blocks renderer {} on the present path",
                optional.get(),
                phase.label()
            ),
            Self::CacheLockWaits { node } => {
                write!(
                    f,
                    "Lux cache node {} waits while holding mutation lock",
                    node.get()
                )
            }
            Self::CacheRequiresSnapshotOrConflictSet { node } => write!(
                f,
                "Lux cache node {} must use a versioned snapshot or scheduler-visible conflict set",
                node.get()
            ),
            Self::GraphInvariant(ref err) => write!(f, "Lux graph invariant failed: {err}"),
        }
    }
}

impl From<GraphInvariantError> for LuxScheduleError {
    fn from(value: GraphInvariantError) -> Self {
        Self::GraphInvariant(value)
    }
}

/// Builder for runtime-validated Lux graphs.
#[derive(Clone, Debug)]
pub struct LuxScheduleBuilder {
    graph_id: WorkGraphId,
    branch: LuxPlanBranch,
    resources: Vec<LuxResourceManifest>,
    works: Vec<LuxWork>,
    dependencies: Vec<(usize, usize)>,
    resource_edges: Vec<(usize, usize, LuxResourceIntentKind)>,
    conflicts: Vec<(usize, usize)>,
}

impl Default for LuxScheduleBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl LuxScheduleBuilder {
    /// Empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            graph_id: WorkGraphId::new(0),
            branch: LuxPlanBranch::DynamicLighting,
            resources: Vec::new(),
            works: Vec::new(),
            dependencies: Vec::new(),
            resource_edges: Vec::new(),
            conflicts: Vec::new(),
        }
    }

    /// Set graph id.
    #[must_use]
    pub const fn with_graph_id(mut self, graph_id: WorkGraphId) -> Self {
        self.graph_id = graph_id;
        self
    }

    /// Set runtime plan branch.
    #[must_use]
    pub const fn with_branch(mut self, branch: LuxPlanBranch) -> Self {
        self.branch = branch;
        self
    }

    /// Add a resource manifest.
    #[must_use]
    pub fn with_resource(mut self, resource: LuxResourceManifest) -> Self {
        self.resources.push(resource);
        self
    }

    /// Add a work node.
    #[must_use]
    pub fn with_work(mut self, work: LuxWork) -> Self {
        self.works.push(work);
        self
    }

    /// Add a dependency by insertion index.
    #[must_use]
    pub fn with_dependency(mut self, from: usize, to: usize) -> Self {
        self.dependencies.push((from, to));
        self
    }

    /// Add a resource read-after-write edge by insertion index.
    #[must_use]
    pub fn with_resource_edge(
        mut self,
        writer: usize,
        reader: usize,
        resource: LuxResourceIntentKind,
    ) -> Self {
        self.resource_edges.push((writer, reader, resource));
        self
    }

    /// Add a scheduler-visible conflict relation by insertion index.
    #[must_use]
    pub fn with_conflict(mut self, a: usize, b: usize) -> Self {
        self.conflicts.push((a, b));
        self
    }

    /// Build and runtime-prove the Lux graph.
    pub fn build_runtime_checked(self) -> Result<RuntimeLuxSchedule, LuxScheduleError> {
        let mut graph = WorkGraph::new(
            self.graph_id,
            ScheduleDomain::RendererLux,
            GraphExecutionMode::DeterministicParallel,
            CommitPolicy::DescriptorOrder,
        );

        for work in self.works {
            let id = graph.next_node_id();
            graph.add_node(lux_work_node(id, work));
        }
        for (from, to) in self.dependencies {
            graph.add_edge(WorkEdge::Dependency {
                from: WorkNodeId::new(from as u64),
                to: WorkNodeId::new(to as u64),
            });
        }
        for (writer, reader, resource) in self.resource_edges {
            graph.add_edge(WorkEdge::ResourceReadAfterWrite {
                writer: WorkNodeId::new(writer as u64),
                reader: WorkNodeId::new(reader as u64),
                resource_id: resource as u64,
            });
        }
        for (a, b) in self.conflicts {
            let a = WorkNodeId::new(a as u64);
            let b = WorkNodeId::new(b as u64);
            if let Some(node) = graph.nodes.get_mut(a.get() as usize) {
                node.conflict_set.push(b);
            }
            if let Some(node) = graph.nodes.get_mut(b.get() as usize) {
                node.conflict_set.push(a);
            }
            graph.add_edge(WorkEdge::Conflict { a, b });
        }

        validate_lux_graph(&graph, self.branch, &self.resources)?;
        Ok(graph.prove_runtime_liveness()?)
    }
}

/// Construct a scheduler work node for Lux work.
#[must_use]
pub fn lux_work_node(id: WorkNodeId, work: LuxWork) -> WorkNode<LuxWork> {
    let label = work.kind.label();
    let liveness = work.liveness();
    let priority = match work.kind {
        LuxWorkKind::DirectLightListBuild => TaskPriority::High,
        LuxWorkKind::LuxDiagnostics => TaskPriority::Idle,
        _ => TaskPriority::Normal,
    };
    WorkNode::new(
        id,
        ScheduleDomain::RendererLux,
        work.kind.lane(),
        work.kind.phase(),
        priority,
        ScheduleBudget::UNBOUNDED,
        ScheduleDeadline::Frame,
        work,
    )
    .with_deterministic_descriptor(DeterministicDescriptor::new(label, id.get()))
    .with_liveness_contract(liveness)
}

/// Validate a Lux graph against the runtime branch and resource manifest.
pub fn validate_lux_graph(
    graph: &LuxWorkGraph,
    branch: LuxPlanBranch,
    resources: &[LuxResourceManifest],
) -> Result<(), LuxScheduleError> {
    for node in &graph.nodes {
        let work = &node.work_descriptor;
        if let Some(condition) = work.condition
            && !condition.active_in(branch)
        {
            return Err(LuxScheduleError::PlanBranchMismatch {
                node: node.id,
                condition,
                branch,
            });
        }

        if let Some(phase) = work.renderer_wait
            && phase.is_forbidden_lux_wait()
        {
            return Err(LuxScheduleError::LuxWaitsOnRendererPhase {
                node: node.id,
                phase,
            });
        }

        if let Some(phase) = work.renderer_consumer {
            if !phase.is_valid_lux_output_consumer() {
                return Err(LuxScheduleError::InvalidRendererConsumerPhase {
                    node: node.id,
                    phase,
                });
            }
            if work.optional {
                return Err(LuxScheduleError::OptionalBlocksPresent {
                    optional: node.id,
                    phase,
                });
            }
        }

        if matches!(work.cache_access, LuxCacheAccess::SharedMutationLock)
            && (!work.awaited_tokens.is_empty() || work.renderer_wait.is_some())
        {
            return Err(LuxScheduleError::CacheLockWaits { node: node.id });
        }

        if work.kind == LuxWorkKind::LuxCacheResidency {
            match work.cache_access {
                LuxCacheAccess::VersionedSnapshot { .. } => {}
                LuxCacheAccess::ConflictSet
                    if !node.conflict_set.is_empty() || node.conflict_set_id.is_some() => {}
                LuxCacheAccess::SharedMutationLock
                    if !work.awaited_tokens.is_empty() || work.renderer_wait.is_some() =>
                {
                    return Err(LuxScheduleError::CacheLockWaits { node: node.id });
                }
                LuxCacheAccess::None
                | LuxCacheAccess::SharedMutationLock
                | LuxCacheAccess::ConflictSet => {
                    return Err(LuxScheduleError::CacheRequiresSnapshotOrConflictSet {
                        node: node.id,
                    });
                }
            }
        }

        for read in &work.reads {
            validate_resource_read(node.id, *read, branch, resources)?;
        }
    }

    validate_renderer_handoffs(graph)?;

    for optional in graph
        .nodes
        .iter()
        .filter(|node| node.work_descriptor.optional)
    {
        for direct in graph
            .nodes
            .iter()
            .filter(|node| node.work_descriptor.kind == LuxWorkKind::DirectLightListBuild)
        {
            if optional.id != direct.id && reaches(graph, optional.id, direct.id) {
                return Err(LuxScheduleError::OptionalBlocksDirectLighting {
                    optional: optional.id,
                    direct: direct.id,
                });
            }
        }
    }

    graph.liveness_proof()?;
    Ok(())
}

fn validate_resource_read(
    node: WorkNodeId,
    read: LuxResourceRead,
    branch: LuxPlanBranch,
    resources: &[LuxResourceManifest],
) -> Result<(), LuxScheduleError> {
    let Some(manifest) = resources
        .iter()
        .find(|manifest| manifest.resource == read.resource)
    else {
        return Err(LuxScheduleError::ResourceUnavailable {
            node,
            resource: read.resource,
        });
    };

    if let LuxResourceAvailability::Conditional(condition) = manifest.availability {
        let guarded = read.guard == Some(condition);
        let fallback = read.fallback.is_some() || manifest.fallback.is_some();
        if !condition.active_in(branch) && !fallback {
            return Err(LuxScheduleError::ConditionalResourceInactive {
                node,
                resource: read.resource,
                condition,
                branch,
            });
        }
        if !guarded && !fallback {
            return Err(LuxScheduleError::ConditionalResourceUnguarded {
                node,
                resource: read.resource,
                condition,
            });
        }
    }
    Ok(())
}

fn validate_renderer_handoffs(graph: &LuxWorkGraph) -> Result<(), LuxScheduleError> {
    for producer in graph
        .nodes
        .iter()
        .filter(|node| node.work_descriptor.renderer_consumer.is_some())
    {
        if let Some(phase) = producer.work_descriptor.renderer_consumer
            && let Some(waiter) = graph.nodes.iter().find(|node| {
                node.work_descriptor
                    .renderer_wait
                    .is_some_and(|wait| wait.order() >= phase.order())
            })
        {
            return Err(LuxScheduleError::LuxRendererCycle {
                producer: producer.id,
                phase,
                waiter: waiter.id,
            });
        }
    }
    Ok(())
}

fn successors(graph: &LuxWorkGraph, node: WorkNodeId) -> Vec<WorkNodeId> {
    let mut out = Vec::new();
    for candidate in &graph.nodes {
        if candidate.dependencies.contains(&node) {
            push_unique(&mut out, candidate.id);
        }
    }
    for edge in &graph.edges {
        match *edge {
            WorkEdge::Dependency { from, to } if from == node => push_unique(&mut out, to),
            WorkEdge::ResourceReadAfterWrite { writer, reader, .. } if writer == node => {
                push_unique(&mut out, reader);
            }
            _ => {}
        }
    }
    out
}

fn reaches(graph: &LuxWorkGraph, from: WorkNodeId, to: WorkNodeId) -> bool {
    let mut stack = vec![from];
    let mut seen = vec![false; graph.node_count()];
    while let Some(node) = stack.pop() {
        if node == to {
            return true;
        }
        let idx = node.get() as usize;
        if idx >= seen.len() || seen[idx] {
            continue;
        }
        seen[idx] = true;
        for successor in successors(graph, node) {
            stack.push(successor);
        }
    }
    false
}

fn push_unique(nodes: &mut Vec<WorkNodeId>, node: WorkNodeId) {
    if !nodes.contains(&node) {
        nodes.push(node);
    }
}

/// Add a renderer cross-domain handoff edge from Lux to renderer.
///
/// This is allowed for Lux outputs consumed by renderer graph compile
/// or record. Lux waits on renderer record/submit/present are rejected
/// by [`validate_lux_graph`].
pub fn add_lux_to_renderer_handoff(
    graph: &mut LuxWorkGraph,
    producer: WorkNodeId,
    consumer: WorkNodeId,
) {
    graph.add_wait_for_edge(WaitForEdge::new(
        WaitForEdgeKind::CrossDomainHandoff,
        producer,
        consumer,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn light_buffer() -> LuxResourceIntentKind {
        LuxResourceIntentKind::LightBuffer
    }

    fn cluster_grid() -> LuxResourceIntentKind {
        LuxResourceIntentKind::ClusterGrid
    }

    fn minimal_resources() -> Vec<LuxResourceManifest> {
        vec![
            LuxResourceManifest::always(light_buffer()),
            LuxResourceManifest::always(cluster_grid()),
        ]
    }

    #[test]
    fn lux_work_kind_labels_are_unique_and_complete() {
        let labels: Vec<&'static str> =
            LuxWorkKind::all().iter().map(|kind| kind.label()).collect();
        assert_eq!(labels.len(), 13);
        for i in 0..labels.len() {
            for j in (i + 1)..labels.len() {
                assert_ne!(labels[i], labels[j]);
            }
        }
        assert!(LuxWorkKind::LuxDiagnostics.optional());
        assert_eq!(
            LuxWorkKind::LuxDiagnostics.lane(),
            ScheduleLane::DiagnosticsLowPriority
        );
    }

    #[test]
    fn lux_plan_typestates_report_branch() {
        let changed = LuxPlan::<LightsChanged>::new(7);
        let dynamic = LuxPlan::<DynamicLighting>::new(8);
        assert_eq!(changed.branch(), LuxPlanBranch::LightsChanged);
        assert_eq!(dynamic.branch(), LuxPlanBranch::DynamicLighting);
    }

    #[test]
    fn conditional_resource_requires_matching_guard_or_fallback() {
        let resources = [LuxResourceManifest::conditional(
            LuxResourceIntentKind::ReflectionTraceBuffer,
            LuxResourceCondition::Reflections,
        )];
        let graph = LuxScheduleBuilder::new()
            .with_resource(resources[0])
            .with_work(LuxWork::new(LuxWorkKind::ReflectionProbeUpdate).with_read(
                LuxResourceRead::new(LuxResourceIntentKind::ReflectionTraceBuffer),
            ))
            .build_runtime_checked()
            .expect_err("unguarded conditional read rejects");
        assert!(matches!(
            graph,
            LuxScheduleError::ConditionalResourceUnguarded { .. }
        ));

        LuxScheduleBuilder::new()
            .with_resource(resources[0])
            .with_work(
                LuxWork::new(LuxWorkKind::ReflectionProbeUpdate).with_read(
                    LuxResourceRead::new(LuxResourceIntentKind::ReflectionTraceBuffer)
                        .guarded(LuxResourceCondition::Reflections),
                ),
            )
            .build_runtime_checked()
            .expect("guarded conditional read validates");

        LuxScheduleBuilder::new()
            .with_resource(resources[0].with_fallback(LuxResourceIntentKind::DenoiseHistory))
            .with_work(LuxWork::new(LuxWorkKind::ReflectionProbeUpdate).with_read(
                LuxResourceRead::new(LuxResourceIntentKind::ReflectionTraceBuffer),
            ))
            .build_runtime_checked()
            .expect("backfilled conditional read validates");
    }

    #[test]
    fn inactive_conditional_resource_requires_fallback() {
        let err = LuxScheduleBuilder::new()
            .with_branch(LuxPlanBranch::LightsUnchanged)
            .with_resource(LuxResourceManifest::conditional(
                LuxResourceIntentKind::LightBuffer,
                LuxResourceCondition::LightsChanged,
            ))
            .with_work(
                LuxWork::new(LuxWorkKind::DirectLightListBuild).with_read(
                    LuxResourceRead::new(LuxResourceIntentKind::LightBuffer)
                        .guarded(LuxResourceCondition::LightsChanged),
                ),
            )
            .build_runtime_checked()
            .expect_err("inactive conditional resource rejects without fallback");
        assert!(matches!(
            err,
            LuxScheduleError::ConditionalResourceInactive { .. }
        ));

        LuxScheduleBuilder::new()
            .with_branch(LuxPlanBranch::LightsUnchanged)
            .with_resource(
                LuxResourceManifest::conditional(
                    LuxResourceIntentKind::LightBuffer,
                    LuxResourceCondition::LightsChanged,
                )
                .with_fallback(LuxResourceIntentKind::ClusterGrid),
            )
            .with_work(
                LuxWork::new(LuxWorkKind::DirectLightListBuild).with_read(
                    LuxResourceRead::new(LuxResourceIntentKind::LightBuffer)
                        .guarded(LuxResourceCondition::LightsChanged),
                ),
            )
            .build_runtime_checked()
            .expect("fallback covers inactive conditional resource");
    }

    #[test]
    fn runtime_branch_mismatch_rejects() {
        let err = LuxScheduleBuilder::new()
            .with_branch(LuxPlanBranch::LightsUnchanged)
            .with_resource(LuxResourceManifest::always(light_buffer()))
            .with_work(
                LuxWork::new(LuxWorkKind::DirectLightListBuild)
                    .with_condition(LuxResourceCondition::LightsChanged)
                    .with_read(LuxResourceRead::new(light_buffer())),
            )
            .build_runtime_checked()
            .expect_err("inactive branch rejects");
        assert!(matches!(err, LuxScheduleError::PlanBranchMismatch { .. }));
    }

    #[test]
    fn lux_cannot_wait_on_renderer_record_submit_or_present() {
        for wait in [
            LuxRendererWait::Record,
            LuxRendererWait::Submit,
            LuxRendererWait::Present,
        ] {
            let err = LuxScheduleBuilder::new()
                .with_resource(LuxResourceManifest::always(light_buffer()))
                .with_work(
                    LuxWork::new(LuxWorkKind::DirectLightListBuild)
                        .with_read(LuxResourceRead::new(light_buffer()))
                        .with_renderer_wait(wait),
                )
                .build_runtime_checked()
                .expect_err("forbidden renderer wait rejects");
            assert!(matches!(
                err,
                LuxScheduleError::LuxWaitsOnRendererPhase { .. }
            ));
        }
    }

    #[test]
    fn renderer_can_consume_lux_outputs_at_compile_or_record() {
        LuxScheduleBuilder::new()
            .with_resource(LuxResourceManifest::always(light_buffer()))
            .with_work(
                LuxWork::new(LuxWorkKind::DirectLightListBuild)
                    .with_write(LuxResourceWrite::new(light_buffer()))
                    .with_renderer_consumer(LuxRendererWait::GraphCompile),
            )
            .build_runtime_checked()
            .expect("renderer graph compile may consume Lux output");

        LuxScheduleBuilder::new()
            .with_resource(LuxResourceManifest::always(light_buffer()))
            .with_work(
                LuxWork::new(LuxWorkKind::DirectLightListBuild)
                    .with_write(LuxResourceWrite::new(light_buffer()))
                    .with_renderer_consumer(LuxRendererWait::Record),
            )
            .build_runtime_checked()
            .expect("renderer record may consume Lux output");
    }

    #[test]
    fn invalid_renderer_consumer_phase_rejects() {
        let err = LuxScheduleBuilder::new()
            .with_resource(LuxResourceManifest::always(light_buffer()))
            .with_work(
                LuxWork::new(LuxWorkKind::DirectLightListBuild)
                    .with_renderer_consumer(LuxRendererWait::Present),
            )
            .build_runtime_checked()
            .expect_err("present cannot directly consume Lux output");
        assert!(matches!(
            err,
            LuxScheduleError::InvalidRendererConsumerPhase { .. }
        ));
    }

    #[test]
    fn lux_renderer_lux_cycle_rejects() {
        let err = LuxScheduleBuilder::new()
            .with_resource(LuxResourceManifest::always(light_buffer()))
            .with_work(
                LuxWork::new(LuxWorkKind::DirectLightListBuild)
                    .with_write(LuxResourceWrite::new(light_buffer()))
                    .with_renderer_consumer(LuxRendererWait::GraphCompile),
            )
            .with_work(
                LuxWork::new(LuxWorkKind::ClusteredLightAssign)
                    .with_renderer_wait(LuxRendererWait::GraphCompile),
            )
            .build_runtime_checked()
            .expect_err("Lux to renderer to Lux cycle rejects");
        assert!(matches!(err, LuxScheduleError::LuxRendererCycle { .. }));
    }

    #[test]
    fn optional_lux_output_cannot_block_present_path() {
        let err = LuxScheduleBuilder::new()
            .with_resource(LuxResourceManifest::always(light_buffer()))
            .with_work(
                LuxWork::new(LuxWorkKind::GiProbeUpdate)
                    .with_renderer_consumer(LuxRendererWait::Record),
            )
            .build_runtime_checked()
            .expect_err("optional Lux output cannot gate renderer record");
        assert!(matches!(
            err,
            LuxScheduleError::OptionalBlocksPresent { .. }
        ));
    }

    #[test]
    fn optional_lux_work_cannot_block_direct_lighting() {
        let err = LuxScheduleBuilder::new()
            .with_resource(LuxResourceManifest::always(light_buffer()))
            .with_work(LuxWork::new(LuxWorkKind::LuxDiagnostics))
            .with_work(
                LuxWork::new(LuxWorkKind::DirectLightListBuild)
                    .with_read(LuxResourceRead::new(light_buffer())),
            )
            .with_dependency(0, 1)
            .build_runtime_checked()
            .expect_err("optional diagnostic blocks direct light");
        assert!(matches!(
            err,
            LuxScheduleError::OptionalBlocksDirectLighting { .. }
        ));
    }

    #[test]
    fn cache_lock_cannot_wait_on_scheduler_work() {
        let err = LuxScheduleBuilder::new()
            .with_resource(LuxResourceManifest::always(light_buffer()))
            .with_work(
                LuxWork::new(LuxWorkKind::LuxCacheResidency)
                    .with_cache_access(LuxCacheAccess::SharedMutationLock)
                    .with_awaited_token(WorkWaitToken::new(9)),
            )
            .build_runtime_checked()
            .expect_err("cache lock wait rejects");
        assert!(matches!(err, LuxScheduleError::CacheLockWaits { .. }));
    }

    #[test]
    fn cache_residency_requires_snapshot_or_conflict_set() {
        let err = LuxScheduleBuilder::new()
            .with_resource(LuxResourceManifest::always(light_buffer()))
            .with_work(
                LuxWork::new(LuxWorkKind::LuxCacheResidency)
                    .with_cache_access(LuxCacheAccess::ConflictSet),
            )
            .build_runtime_checked()
            .expect_err("declared conflict set requires scheduler conflict relation");
        assert!(matches!(
            err,
            LuxScheduleError::CacheRequiresSnapshotOrConflictSet { .. }
        ));

        LuxScheduleBuilder::new()
            .with_resource(LuxResourceManifest::always(light_buffer()))
            .with_work(
                LuxWork::new(LuxWorkKind::LuxCacheResidency)
                    .with_cache_access(LuxCacheAccess::ConflictSet),
            )
            .with_work(LuxWork::new(LuxWorkKind::DirectLightListBuild))
            .with_conflict(0, 1)
            .build_runtime_checked()
            .expect("scheduler-visible conflict set is accepted");
    }

    #[test]
    fn versioned_cache_snapshot_can_wait() {
        let token = WorkWaitToken::new(9);
        LuxScheduleBuilder::new()
            .with_resource(LuxResourceManifest::always(light_buffer()))
            .with_work(LuxWork::new(LuxWorkKind::DirectLightListBuild).with_produced_token(token))
            .with_work(
                LuxWork::new(LuxWorkKind::LuxCacheResidency)
                    .with_cache_access(LuxCacheAccess::VersionedSnapshot { version: 4 })
                    .with_awaited_token(token),
            )
            .with_dependency(0, 1)
            .build_runtime_checked()
            .expect("versioned snapshot can wait");
    }

    #[test]
    fn resource_cycle_rejects_through_work_graph_liveness() {
        let err = LuxScheduleBuilder::new()
            .with_resource(LuxResourceManifest::always(light_buffer()))
            .with_resource(LuxResourceManifest::always(cluster_grid()))
            .with_work(
                LuxWork::new(LuxWorkKind::ClusteredLightAssign)
                    .with_read(LuxResourceRead::new(light_buffer()))
                    .with_write(LuxResourceWrite::new(cluster_grid())),
            )
            .with_work(
                LuxWork::new(LuxWorkKind::DirectLightListBuild)
                    .with_read(LuxResourceRead::new(cluster_grid()))
                    .with_write(LuxResourceWrite::new(light_buffer())),
            )
            .with_resource_edge(0, 1, cluster_grid())
            .with_resource_edge(1, 0, light_buffer())
            .build_runtime_checked()
            .expect_err("resource cycle rejects");
        assert!(matches!(
            err,
            LuxScheduleError::GraphInvariant(GraphInvariantError::Cycle { .. })
                | LuxScheduleError::GraphInvariant(GraphInvariantError::WaitForCycle { .. })
        ));
    }

    #[test]
    fn valid_dynamic_lux_graph_proves_runtime_liveness() {
        let schedule = LuxScheduleBuilder::new()
            .with_branch(LuxPlanBranch::DynamicLighting)
            .with_resource(LuxResourceManifest::always(light_buffer()))
            .with_resource(LuxResourceManifest::always(cluster_grid()))
            .with_work(
                LuxWork::new(LuxWorkKind::ClusteredLightAssign)
                    .with_read(LuxResourceRead::new(light_buffer()))
                    .with_write(LuxResourceWrite::new(cluster_grid())),
            )
            .with_work(
                LuxWork::new(LuxWorkKind::DirectLightListBuild)
                    .with_read(LuxResourceRead::new(cluster_grid())),
            )
            .with_resource_edge(0, 1, cluster_grid())
            .build_runtime_checked()
            .expect("valid Lux schedule");
        assert_eq!(schedule.as_graph().node_count(), 2);
    }

    #[test]
    fn handoff_edge_helper_records_cross_domain_wait() {
        let mut graph = WorkGraph::new(
            WorkGraphId::new(5),
            ScheduleDomain::RendererLux,
            GraphExecutionMode::DeterministicParallel,
            CommitPolicy::DescriptorOrder,
        );
        let a = graph.next_node_id();
        graph.add_node(lux_work_node(
            a,
            LuxWork::new(LuxWorkKind::ClusteredLightAssign),
        ));
        let b = graph.next_node_id();
        graph.add_node(lux_work_node(
            b,
            LuxWork::new(LuxWorkKind::DirectLightListBuild),
        ));
        add_lux_to_renderer_handoff(&mut graph, a, b);
        assert_eq!(graph.wait_for_edges.len(), 1);
        assert_eq!(
            graph.wait_for_edges[0].kind,
            WaitForEdgeKind::CrossDomainHandoff
        );
    }

    #[test]
    fn resource_manifest_wrapper_types_compile() {
        let always = AlwaysResource(light_buffer());
        let imported = ImportedResource(cluster_grid());
        let conditional =
            ConditionalResource::<_, LightsChanged>::new(LuxResourceIntentKind::ShadowAtlas);
        let backfilled = BackfilledResource(LuxResourceIntentKind::DenoiseHistory);
        let optional = OptionalResource(LuxResourceIntentKind::LuxDebugBuffer);
        let _: LuxResourceIntentKind = always.0;
        let _: LuxResourceIntentKind = imported.0;
        let _: LuxResourceIntentKind = conditional.resource;
        let _: LuxResourceIntentKind = backfilled.0;
        let _: LuxResourceIntentKind = optional.0;
        assert_eq!(
            always.manifest().availability,
            LuxResourceAvailability::Always
        );
        assert_eq!(
            imported.manifest().availability,
            LuxResourceAvailability::Imported
        );
        assert_eq!(
            conditional.manifest().availability,
            LuxResourceAvailability::Conditional(LuxResourceCondition::LightsChanged)
        );
        assert_eq!(
            backfilled.manifest().availability,
            LuxResourceAvailability::Backfilled
        );
        assert_eq!(
            optional.manifest().availability,
            LuxResourceAvailability::Optional
        );
    }

    #[test]
    fn minimal_resources_helper_is_valid() {
        assert_eq!(minimal_resources().len(), 2);
    }
}
