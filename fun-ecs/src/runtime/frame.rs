use fun_scheduler_types::{
    CommitPolicy, DeterministicDescriptor, EcsAccess, EcsAccessTarget, EcsChunkKey,
    EcsLivenessClass, EcsResourceId, EcsSystemClass, EcsSystemExecutionContract, EcsSystemId,
    EcsWork, EcsWorkKind, EcsWorldRevision, GraphExecutionMode, GraphInvariantError, LivenessProof,
    MainThreadRequirement, ProductRegistry, ScheduleBudget, ScheduleDeadline, ScheduleDomain,
    ScheduleLane, SystemAccess, TaskPriority, WaitForEdge, WaitForEdgeKind, WorkBlockingClass,
    WorkBlockingDisposition, WorkCostHint, WorkEdge, WorkGraph, WorkGraphId, WorkLocalityHint,
    WorkNode, WorkNodeId, WorkNodeLiveness, WorkRequiredness, WorkSplitHint, WorkWaitToken,
};

use crate::{
    FunEcsResourceKind, FunEcsSubsystem, FunFrameId, FunResourceId, FunRevision,
    scheduler_resource_id,
};

pub type FunFrameGraph = WorkGraph<EcsWork<ProductRegistry>>;

const FUN_FRAME_WAIT_TOKEN_TAG: u64 = 0x0f13_0000_0000_0000;
const DEFAULT_FRAME_GRAPH_ID: WorkGraphId = WorkGraphId::new(2_600);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FunFrameExecutionMode {
    #[default]
    DeterministicParallel = 0,
    DeterministicSingleThread = 1,
}

impl FunFrameExecutionMode {
    #[must_use]
    pub const fn graph_mode(self) -> GraphExecutionMode {
        match self {
            Self::DeterministicParallel => GraphExecutionMode::DeterministicParallel,
            Self::DeterministicSingleThread => GraphExecutionMode::SingleThreadDeterministic,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunFrameIntent {
    pub graph_id: WorkGraphId,
    pub frame: FunFrameId,
    pub mode: FunFrameExecutionMode,
    pub include_optional_work: bool,
}

impl Default for FunFrameIntent {
    fn default() -> Self {
        Self {
            graph_id: DEFAULT_FRAME_GRAPH_ID,
            frame: FunFrameId::new(1),
            mode: FunFrameExecutionMode::DeterministicParallel,
            include_optional_work: true,
        }
    }
}

impl FunFrameIntent {
    #[must_use]
    pub const fn with_graph_id(mut self, graph_id: WorkGraphId) -> Self {
        self.graph_id = graph_id;
        self
    }

    #[must_use]
    pub const fn with_frame(mut self, frame: FunFrameId) -> Self {
        self.frame = frame;
        self
    }

    #[must_use]
    pub const fn include_optional_work(mut self, include_optional_work: bool) -> Self {
        self.include_optional_work = include_optional_work;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunFrameContext {
    pub observed_revision: FunRevision,
    pub active_systems: u32,
    pub active_cameras: u32,
    pub stream_requests: u32,
    pub dirty_regions: u32,
    pub pending_commands: u32,
    pub frame_deadline: ScheduleDeadline,
    pub subsystem_readiness: FunFrameSubsystemReadiness,
    pub budget_pressure: FunFrameBudgetPressure,
    pub fallback_availability: FunFrameFallbackAvailability,
    pub private_queue_bypass: Vec<FunEcsSubsystem>,
    pub extra_wait_plans: Vec<FunFrameWaitPlan>,
}

impl Default for FunFrameContext {
    fn default() -> Self {
        Self {
            observed_revision: FunRevision::INITIAL,
            active_systems: 0,
            active_cameras: 0,
            stream_requests: 0,
            dirty_regions: 0,
            pending_commands: 0,
            frame_deadline: ScheduleDeadline::Present,
            subsystem_readiness: FunFrameSubsystemReadiness::all_ready(),
            budget_pressure: FunFrameBudgetPressure::default(),
            fallback_availability: FunFrameFallbackAvailability::default(),
            private_queue_bypass: Vec::new(),
            extra_wait_plans: Vec::new(),
        }
    }
}

impl FunFrameContext {
    #[must_use]
    pub const fn with_observed_revision(mut self, revision: FunRevision) -> Self {
        self.observed_revision = revision;
        self
    }

    #[must_use]
    pub const fn with_active_cameras(mut self, active_cameras: u32) -> Self {
        self.active_cameras = active_cameras;
        self
    }

    #[must_use]
    pub const fn with_stream_requests(mut self, stream_requests: u32) -> Self {
        self.stream_requests = stream_requests;
        self
    }

    #[must_use]
    pub const fn with_dirty_regions(mut self, dirty_regions: u32) -> Self {
        self.dirty_regions = dirty_regions;
        self
    }

    #[must_use]
    pub const fn with_pending_commands(mut self, pending_commands: u32) -> Self {
        self.pending_commands = pending_commands;
        self
    }

    #[must_use]
    pub fn with_private_queue_bypass(mut self, subsystem: FunEcsSubsystem) -> Self {
        if !self.private_queue_bypass.contains(&subsystem) {
            self.private_queue_bypass.push(subsystem);
            self.private_queue_bypass.sort_unstable();
        }
        self
    }

    #[must_use]
    pub fn with_extra_wait_plan(mut self, plan: FunFrameWaitPlan) -> Self {
        self.extra_wait_plans.push(plan);
        self
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunFrameSubsystemReadiness {
    pub renderer: bool,
    pub lux: bool,
    pub avis: bool,
    pub thunder: bool,
    pub rvelte: bool,
}

impl FunFrameSubsystemReadiness {
    #[must_use]
    pub const fn all_ready() -> Self {
        Self {
            renderer: true,
            lux: true,
            avis: true,
            thunder: true,
            rvelte: true,
        }
    }

    #[must_use]
    pub const fn is_ready(self, subsystem: FunEcsSubsystem) -> bool {
        match subsystem {
            FunEcsSubsystem::Renderer => self.renderer,
            FunEcsSubsystem::Lux => self.lux,
            FunEcsSubsystem::Avis => self.avis,
            FunEcsSubsystem::Thunder => self.thunder,
            FunEcsSubsystem::Rvelte => self.rvelte,
            FunEcsSubsystem::FunEcs
            | FunEcsSubsystem::Animation
            | FunEcsSubsystem::Ai
            | FunEcsSubsystem::Warden
            | FunEcsSubsystem::Telemetry => true,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunFrameBudgetPressure {
    pub renderer: u8,
    pub lux: u8,
    pub avis: u8,
    pub thunder: u8,
    pub rvelte: u8,
}

impl FunFrameBudgetPressure {
    #[must_use]
    pub const fn max_pressure(self) -> u8 {
        let renderer_lux = if self.renderer > self.lux {
            self.renderer
        } else {
            self.lux
        };
        let avis_thunder = if self.avis > self.thunder {
            self.avis
        } else {
            self.thunder
        };
        let a = if renderer_lux > avis_thunder {
            renderer_lux
        } else {
            avis_thunder
        };
        if a > self.rvelte { a } else { self.rvelte }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunFrameFallbackAvailability {
    pub render_artifact: bool,
    pub collision_proxy: bool,
    pub lighting: bool,
    pub hud_packet: bool,
    pub network_delta: bool,
}

impl Default for FunFrameFallbackAvailability {
    fn default() -> Self {
        Self {
            render_artifact: true,
            collision_proxy: true,
            lighting: true,
            hud_packet: true,
            network_delta: true,
        }
    }
}

impl FunFrameFallbackAvailability {
    #[must_use]
    pub const fn supports(self, kind: FunFrameFallbackKind) -> bool {
        match kind {
            FunFrameFallbackKind::RenderArtifact => self.render_artifact,
            FunFrameFallbackKind::ConservativePhysicsProxy => self.collision_proxy,
            FunFrameFallbackKind::LightingCache => self.lighting,
            FunFrameFallbackKind::HudPacket => self.hud_packet,
            FunFrameFallbackKind::NetworkDelta => self.network_delta,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum FunFrameStage {
    #[default]
    InputAcquire = 0,
    EcsPreUpdate = 1,
    GameplayUpdate = 2,
    EcsCommandApply = 3,
    SpatialSense = 4,
    SpatialBuildInterest = 5,
    SpatialDiffRequests = 6,
    SourceAcquire = 7,
    SourceDecode = 8,
    ArtifactBuild = 9,
    DirtyPropagation = 10,
    HandoffPublication = 11,
    AvisFixedStep = 12,
    AvisCook = 13,
    AvisWriteback = 14,
    RvelteInput = 15,
    RvelteState = 16,
    RvelteLayout = 17,
    RveltePaint = 18,
    LuxConsume = 19,
    LuxPlan = 20,
    LuxPolicy = 21,
    RendererConsume = 22,
    RendererUpload = 23,
    RendererExtract = 24,
    RendererCompile = 25,
    RendererRecord = 26,
    RendererSubmit = 27,
    RendererPresent = 28,
    ThunderRelevance = 29,
    ThunderSnapshot = 30,
    ThunderDelta = 31,
    TelemetryFlush = 32,
    EvictionRetirement = 33,
}

pub const FUN_FRAME_STAGE_COUNT: usize = 34;
pub const FUN_FRAME_STAGES: [FunFrameStage; FUN_FRAME_STAGE_COUNT] = [
    FunFrameStage::InputAcquire,
    FunFrameStage::EcsPreUpdate,
    FunFrameStage::GameplayUpdate,
    FunFrameStage::EcsCommandApply,
    FunFrameStage::SpatialSense,
    FunFrameStage::SpatialBuildInterest,
    FunFrameStage::SpatialDiffRequests,
    FunFrameStage::SourceAcquire,
    FunFrameStage::SourceDecode,
    FunFrameStage::ArtifactBuild,
    FunFrameStage::DirtyPropagation,
    FunFrameStage::HandoffPublication,
    FunFrameStage::AvisFixedStep,
    FunFrameStage::AvisCook,
    FunFrameStage::AvisWriteback,
    FunFrameStage::RvelteInput,
    FunFrameStage::RvelteState,
    FunFrameStage::RvelteLayout,
    FunFrameStage::RveltePaint,
    FunFrameStage::LuxConsume,
    FunFrameStage::LuxPlan,
    FunFrameStage::LuxPolicy,
    FunFrameStage::RendererConsume,
    FunFrameStage::RendererUpload,
    FunFrameStage::RendererExtract,
    FunFrameStage::RendererCompile,
    FunFrameStage::RendererRecord,
    FunFrameStage::RendererSubmit,
    FunFrameStage::RendererPresent,
    FunFrameStage::ThunderRelevance,
    FunFrameStage::ThunderSnapshot,
    FunFrameStage::ThunderDelta,
    FunFrameStage::TelemetryFlush,
    FunFrameStage::EvictionRetirement,
];

impl FunFrameStage {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::InputAcquire => "input_acquire",
            Self::EcsPreUpdate => "ecs_pre_update",
            Self::GameplayUpdate => "gameplay_update",
            Self::EcsCommandApply => "ecs_command_apply",
            Self::SpatialSense => "spatial_sense",
            Self::SpatialBuildInterest => "spatial_build_interest",
            Self::SpatialDiffRequests => "spatial_diff_requests",
            Self::SourceAcquire => "source_acquire",
            Self::SourceDecode => "source_decode",
            Self::ArtifactBuild => "artifact_build",
            Self::DirtyPropagation => "dirty_propagation",
            Self::HandoffPublication => "handoff_publication",
            Self::AvisFixedStep => "avis_fixed_step",
            Self::AvisCook => "avis_cook",
            Self::AvisWriteback => "avis_writeback",
            Self::RvelteInput => "rvelte_input",
            Self::RvelteState => "rvelte_state",
            Self::RvelteLayout => "rvelte_layout",
            Self::RveltePaint => "rvelte_paint",
            Self::LuxConsume => "lux_consume",
            Self::LuxPlan => "lux_plan",
            Self::LuxPolicy => "lux_policy",
            Self::RendererConsume => "renderer_consume",
            Self::RendererUpload => "renderer_upload",
            Self::RendererExtract => "renderer_extract",
            Self::RendererCompile => "renderer_compile",
            Self::RendererRecord => "renderer_record",
            Self::RendererSubmit => "renderer_submit",
            Self::RendererPresent => "renderer_present",
            Self::ThunderRelevance => "thunder_relevance",
            Self::ThunderSnapshot => "thunder_snapshot",
            Self::ThunderDelta => "thunder_delta",
            Self::TelemetryFlush => "telemetry_flush",
            Self::EvictionRetirement => "eviction_retirement",
        }
    }

    #[must_use]
    pub const fn all() -> &'static [Self] {
        &FUN_FRAME_STAGES
    }

    #[must_use]
    pub const fn ordinal(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn subsystem(self) -> FunEcsSubsystem {
        match self {
            Self::InputAcquire
            | Self::EcsPreUpdate
            | Self::GameplayUpdate
            | Self::EcsCommandApply
            | Self::SpatialSense
            | Self::SpatialBuildInterest
            | Self::SpatialDiffRequests
            | Self::SourceAcquire
            | Self::SourceDecode
            | Self::ArtifactBuild
            | Self::DirtyPropagation
            | Self::HandoffPublication
            | Self::EvictionRetirement => FunEcsSubsystem::FunEcs,
            Self::AvisFixedStep | Self::AvisCook | Self::AvisWriteback => FunEcsSubsystem::Avis,
            Self::RvelteInput | Self::RvelteState | Self::RvelteLayout | Self::RveltePaint => {
                FunEcsSubsystem::Rvelte
            }
            Self::LuxConsume | Self::LuxPlan | Self::LuxPolicy => FunEcsSubsystem::Lux,
            Self::RendererConsume
            | Self::RendererUpload
            | Self::RendererExtract
            | Self::RendererCompile
            | Self::RendererRecord
            | Self::RendererSubmit
            | Self::RendererPresent => FunEcsSubsystem::Renderer,
            Self::ThunderRelevance | Self::ThunderSnapshot | Self::ThunderDelta => {
                FunEcsSubsystem::Thunder
            }
            Self::TelemetryFlush => FunEcsSubsystem::Telemetry,
        }
    }

    #[must_use]
    pub const fn requiredness(self) -> WorkRequiredness {
        match self {
            Self::TelemetryFlush | Self::EvictionRetirement => WorkRequiredness::Optional,
            _ => WorkRequiredness::Required,
        }
    }

    #[must_use]
    pub const fn is_required_path(self) -> bool {
        matches!(
            self,
            Self::RendererPresent
                | Self::AvisFixedStep
                | Self::AvisWriteback
                | Self::ThunderSnapshot
                | Self::ThunderDelta
        )
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum FunFrameWaitTokenKind {
    #[default]
    InputReady = 0,
    EcsPreUpdateReady = 1,
    GameplayUpdated = 2,
    CommandsApplied = 3,
    SpatialDiffReady = 4,
    SourceAcquired = 5,
    PagesDecoded = 6,
    ArtifactsBuilt = 7,
    DirtyPropagated = 8,
    HandoffsPublished = 9,
    PhysicsFixedStepReady = 10,
    PhysicsCookReady = 11,
    PhysicsWritebackReady = 12,
    RvelteStateReady = 13,
    RvelteLayoutReady = 14,
    RveltePaintReady = 15,
    LuxPolicyReady = 16,
    RendererConsumeReady = 17,
    RendererSubmitReady = 18,
    RendererPresentReady = 19,
    NetworkRelevanceReady = 20,
    NetworkDeltaReady = 21,
}

impl FunFrameWaitTokenKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::InputReady => "input_ready",
            Self::EcsPreUpdateReady => "ecs_pre_update_ready",
            Self::GameplayUpdated => "gameplay_updated",
            Self::CommandsApplied => "commands_applied",
            Self::SpatialDiffReady => "spatial_diff_ready",
            Self::SourceAcquired => "source_acquired",
            Self::PagesDecoded => "pages_decoded",
            Self::ArtifactsBuilt => "artifacts_built",
            Self::DirtyPropagated => "dirty_propagated",
            Self::HandoffsPublished => "handoffs_published",
            Self::PhysicsFixedStepReady => "physics_fixed_step_ready",
            Self::PhysicsCookReady => "physics_cook_ready",
            Self::PhysicsWritebackReady => "physics_writeback_ready",
            Self::RvelteStateReady => "rvelte_state_ready",
            Self::RvelteLayoutReady => "rvelte_layout_ready",
            Self::RveltePaintReady => "rvelte_paint_ready",
            Self::LuxPolicyReady => "lux_policy_ready",
            Self::RendererConsumeReady => "renderer_consume_ready",
            Self::RendererSubmitReady => "renderer_submit_ready",
            Self::RendererPresentReady => "renderer_present_ready",
            Self::NetworkRelevanceReady => "network_relevance_ready",
            Self::NetworkDeltaReady => "network_delta_ready",
        }
    }

    #[must_use]
    pub const fn scheduler_token(self, frame: FunFrameId) -> WorkWaitToken {
        WorkWaitToken::new(
            FUN_FRAME_WAIT_TOKEN_TAG
                | ((self as u64) << 40)
                | (frame.get() & 0x0000_00ff_ffff_ffff),
        )
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FunFrameFallbackKind {
    #[default]
    RenderArtifact = 0,
    ConservativePhysicsProxy = 1,
    LightingCache = 2,
    HudPacket = 3,
    NetworkDelta = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunFrameFallbackPlan {
    pub kind: FunFrameFallbackKind,
    pub has_fallback: bool,
    pub timeout: Option<ScheduleDeadline>,
}

impl FunFrameFallbackPlan {
    #[must_use]
    pub const fn fallback(kind: FunFrameFallbackKind) -> Self {
        Self {
            kind,
            has_fallback: true,
            timeout: None,
        }
    }

    #[must_use]
    pub const fn bounded(kind: FunFrameFallbackKind, timeout: ScheduleDeadline) -> Self {
        Self {
            kind,
            has_fallback: false,
            timeout: Some(timeout),
        }
    }

    #[must_use]
    pub const fn unavailable(kind: FunFrameFallbackKind) -> Self {
        Self {
            kind,
            has_fallback: false,
            timeout: None,
        }
    }

    #[must_use]
    pub const fn has_timeout_or_fallback(self) -> bool {
        self.has_fallback || self.timeout.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunFrameWaitPlan {
    pub token: FunFrameWaitTokenKind,
    pub producer: FunFrameStage,
    pub consumer: FunFrameStage,
    pub wait_kind: WaitForEdgeKind,
    pub requiredness: WorkRequiredness,
    pub cancellation_stage: FunFrameStage,
    pub fallback: FunFrameFallbackPlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunFrameResourceSource {
    ProducedBy(FunFrameStage),
    ImportedFrom(FunEcsSubsystem),
    Fallback(FunFrameFallbackKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunFrameResourceReadPlan {
    pub stage: FunFrameStage,
    pub resource: FunEcsResourceKind,
    pub requiredness: WorkRequiredness,
    pub source: FunFrameResourceSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunFrameStageNode {
    pub node_id: WorkNodeId,
    pub stage: FunFrameStage,
    pub subsystem: FunEcsSubsystem,
    pub domain: ScheduleDomain,
    pub lane: ScheduleLane,
    pub requiredness: WorkRequiredness,
    pub awaited_token_count: u16,
    pub produced_token_count: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunFrameImportedGraph {
    pub subsystem: FunEcsSubsystem,
    pub first_stage: FunFrameStage,
    pub last_stage: FunFrameStage,
    pub node_count: u32,
    pub digest: FunFrameDigest,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunFrameDigest {
    pub value: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunFrameReport {
    pub stages: u32,
    pub graph_nodes: u32,
    pub dependency_edges: u32,
    pub wait_edges: u32,
    pub cancellation_edges: u32,
    pub imported_graphs: u32,
    pub required_resource_reads: u32,
    pub optional_nodes: u32,
    pub liveness_proven: bool,
    pub blocking_nodes: u32,
    pub max_budget_pressure: u8,
    pub digest: FunFrameDigest,
}

#[derive(Debug, Clone)]
pub struct FunFrameSchedule {
    pub intent: FunFrameIntent,
    pub context: FunFrameContext,
    pub graph: FunFrameGraph,
    pub liveness_proof: LivenessProof,
    pub digest: FunFrameDigest,
    pub report: FunFrameReport,
    pub stages: Vec<FunFrameStageNode>,
    pub wait_plans: Vec<FunFrameWaitPlan>,
    pub resource_reads: Vec<FunFrameResourceReadPlan>,
    pub imported_graphs: Vec<FunFrameImportedGraph>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunFrameCompileError {
    Graph(GraphInvariantError),
    PrivateSubsystemQueueBypass {
        subsystem: FunEcsSubsystem,
    },
    RequiredResourceReadWithoutProducerImportOrFallback {
        stage: FunFrameStage,
        resource: FunEcsResourceKind,
    },
    OptionalWorkGatesRequiredPath {
        optional: FunFrameStage,
        required: FunFrameStage,
    },
    BlockingWorkOutsideBlockingLane {
        stage: FunFrameStage,
        lane: ScheduleLane,
    },
    WaitTokenMissingProducer {
        token: FunFrameWaitTokenKind,
    },
    WaitTokenMissingConsumer {
        token: FunFrameWaitTokenKind,
    },
    WaitTokenMissingCancellationPath {
        token: FunFrameWaitTokenKind,
    },
    WaitTokenMissingTimeoutOrFallback {
        token: FunFrameWaitTokenKind,
    },
    RequiredSubsystemUnavailable {
        subsystem: FunEcsSubsystem,
    },
}

impl From<GraphInvariantError> for FunFrameCompileError {
    fn from(value: GraphInvariantError) -> Self {
        Self::Graph(value)
    }
}

pub struct FunFrameCompiler;

impl FunFrameCompiler {
    pub fn compile(context: FunFrameContext) -> Result<FunFrameSchedule, FunFrameCompileError> {
        Self::compile_with_intent(FunFrameIntent::default(), context)
    }

    pub fn compile_with_intent(
        intent: FunFrameIntent,
        context: FunFrameContext,
    ) -> Result<FunFrameSchedule, FunFrameCompileError> {
        Self::validate_private_queue_policy(&context)?;
        Self::validate_required_subsystems(&context)?;

        let stage_declarations = stage_declarations(intent.include_optional_work);
        let wait_plans = frame_wait_plans(&intent, &context, &stage_declarations);
        let resource_reads = resource_read_plans();

        Self::validate_wait_plans(&wait_plans, &stage_declarations)?;
        Self::validate_resource_reads(&resource_reads, &context, &stage_declarations)?;

        let mut graph = WorkGraph::new(
            intent.graph_id,
            ScheduleDomain::FunEcs,
            intent.mode.graph_mode(),
            CommitPolicy::DescriptorOrder,
        )
        .with_deadline(context.frame_deadline);

        let mut stages = Vec::with_capacity(stage_declarations.len());
        for declaration in &stage_declarations {
            let id = graph.next_node_id();
            let awaited = awaited_tokens_for_stage(*declaration, &wait_plans, intent.frame);
            let produced = produced_tokens_for_stage(*declaration, &wait_plans, intent.frame);
            let work = frame_work(*declaration, context.observed_revision, &awaited, &produced);
            let node = frame_work_node(id, *declaration, work, &awaited, &produced);
            graph.add_node(node);
            stages.push(FunFrameStageNode {
                node_id: id,
                stage: declaration.stage,
                subsystem: declaration.stage.subsystem(),
                domain: declaration.domain,
                lane: declaration.lane,
                requiredness: declaration.requiredness,
                awaited_token_count: awaited.len() as u16,
                produced_token_count: produced.len() as u16,
            });
        }

        for pair in stages.windows(2) {
            graph.add_edge(WorkEdge::Dependency {
                from: pair[0].node_id,
                to: pair[1].node_id,
            });
        }

        for stage in &stages {
            if matches!(
                stage.stage,
                FunFrameStage::EcsCommandApply | FunFrameStage::HandoffPublication
            ) {
                graph.add_edge(WorkEdge::Barrier { at: stage.node_id });
            }
        }

        for plan in &wait_plans {
            let producer = node_for_stage(&stages, plan.producer)
                .ok_or(FunFrameCompileError::WaitTokenMissingProducer { token: plan.token })?;
            let consumer = node_for_stage(&stages, plan.consumer)
                .ok_or(FunFrameCompileError::WaitTokenMissingConsumer { token: plan.token })?;
            graph.add_wait_for_edge(WaitForEdge::new(plan.wait_kind, producer, consumer));
            graph.add_edge(WorkEdge::CancellationPropagation {
                parent: producer,
                child: consumer,
            });
        }

        Self::validate_optional_work_does_not_gate_required_paths(&graph, &stages)?;
        Self::validate_blocking_lane_policy(&stages)?;
        graph.validate(true)?;
        let liveness_proof = graph.liveness_proof()?;
        let imported_graphs = imported_graphs(&stages);
        let digest = FunFrameDigest::from_parts(
            &intent,
            &context,
            &graph,
            &wait_plans,
            &resource_reads,
            &imported_graphs,
        );
        let report = FunFrameReport::from_graph(
            &graph,
            &stages,
            &resource_reads,
            &imported_graphs,
            context.budget_pressure.max_pressure(),
            digest,
            true,
        );

        Ok(FunFrameSchedule {
            intent,
            context,
            graph,
            liveness_proof,
            digest,
            report,
            stages,
            wait_plans,
            resource_reads,
            imported_graphs,
        })
    }

    pub fn validate_wait_plans(
        wait_plans: &[FunFrameWaitPlan],
        stage_declarations: &[FunFrameStageDeclaration],
    ) -> Result<(), FunFrameCompileError> {
        for plan in wait_plans {
            if !stage_declarations
                .iter()
                .any(|stage| stage.stage == plan.producer)
            {
                return Err(FunFrameCompileError::WaitTokenMissingProducer { token: plan.token });
            }
            if !stage_declarations
                .iter()
                .any(|stage| stage.stage == plan.consumer)
            {
                return Err(FunFrameCompileError::WaitTokenMissingConsumer { token: plan.token });
            }
            if !stage_declarations
                .iter()
                .any(|stage| stage.stage == plan.cancellation_stage)
            {
                return Err(FunFrameCompileError::WaitTokenMissingCancellationPath {
                    token: plan.token,
                });
            }
            if !plan.fallback.has_timeout_or_fallback() {
                return Err(FunFrameCompileError::WaitTokenMissingTimeoutOrFallback {
                    token: plan.token,
                });
            }
        }
        Ok(())
    }

    pub fn validate_resource_reads(
        reads: &[FunFrameResourceReadPlan],
        context: &FunFrameContext,
        stage_declarations: &[FunFrameStageDeclaration],
    ) -> Result<(), FunFrameCompileError> {
        for read in reads {
            if read.requiredness.is_optional() {
                continue;
            }
            let valid = match read.source {
                FunFrameResourceSource::ProducedBy(producer) => {
                    stage_declarations
                        .iter()
                        .any(|stage| stage.stage == producer)
                        && stage_declarations
                            .iter()
                            .any(|stage| stage.stage == read.stage)
                        && producer.ordinal() < read.stage.ordinal()
                }
                FunFrameResourceSource::ImportedFrom(subsystem) => {
                    context.subsystem_readiness.is_ready(subsystem)
                }
                FunFrameResourceSource::Fallback(kind) => {
                    context.fallback_availability.supports(kind)
                }
            };
            if !valid {
                return Err(
                    FunFrameCompileError::RequiredResourceReadWithoutProducerImportOrFallback {
                        stage: read.stage,
                        resource: read.resource,
                    },
                );
            }
        }
        Ok(())
    }

    fn validate_private_queue_policy(
        context: &FunFrameContext,
    ) -> Result<(), FunFrameCompileError> {
        if let Some(subsystem) = context.private_queue_bypass.first().copied() {
            return Err(FunFrameCompileError::PrivateSubsystemQueueBypass { subsystem });
        }
        Ok(())
    }

    fn validate_required_subsystems(context: &FunFrameContext) -> Result<(), FunFrameCompileError> {
        if !context.subsystem_readiness.renderer && !context.fallback_availability.render_artifact {
            return Err(FunFrameCompileError::RequiredSubsystemUnavailable {
                subsystem: FunEcsSubsystem::Renderer,
            });
        }
        if !context.subsystem_readiness.avis && !context.fallback_availability.collision_proxy {
            return Err(FunFrameCompileError::RequiredSubsystemUnavailable {
                subsystem: FunEcsSubsystem::Avis,
            });
        }
        if !context.subsystem_readiness.rvelte && !context.fallback_availability.hud_packet {
            return Err(FunFrameCompileError::RequiredSubsystemUnavailable {
                subsystem: FunEcsSubsystem::Rvelte,
            });
        }
        if !context.subsystem_readiness.lux && !context.fallback_availability.lighting {
            return Err(FunFrameCompileError::RequiredSubsystemUnavailable {
                subsystem: FunEcsSubsystem::Lux,
            });
        }
        if !context.subsystem_readiness.thunder && !context.fallback_availability.network_delta {
            return Err(FunFrameCompileError::RequiredSubsystemUnavailable {
                subsystem: FunEcsSubsystem::Thunder,
            });
        }
        Ok(())
    }

    fn validate_optional_work_does_not_gate_required_paths(
        graph: &FunFrameGraph,
        stages: &[FunFrameStageNode],
    ) -> Result<(), FunFrameCompileError> {
        for optional in stages
            .iter()
            .filter(|stage| stage.requiredness.is_optional())
            .copied()
        {
            let mut stack = successors(graph, optional.node_id);
            let mut visited = Vec::new();
            while let Some(node) = stack.pop() {
                if visited.contains(&node) {
                    continue;
                }
                visited.push(node);
                let Some(stage) = stages.iter().find(|stage| stage.node_id == node) else {
                    continue;
                };
                if stage.requiredness == WorkRequiredness::Required
                    && stage.stage.is_required_path()
                {
                    return Err(FunFrameCompileError::OptionalWorkGatesRequiredPath {
                        optional: optional.stage,
                        required: stage.stage,
                    });
                }
                stack.extend(successors(graph, node));
            }
        }
        Ok(())
    }

    fn validate_blocking_lane_policy(
        stages: &[FunFrameStageNode],
    ) -> Result<(), FunFrameCompileError> {
        for stage in stages {
            if matches!(stage.stage, FunFrameStage::SourceAcquire)
                && stage.lane != ScheduleLane::Blocking
            {
                return Err(FunFrameCompileError::BlockingWorkOutsideBlockingLane {
                    stage: stage.stage,
                    lane: stage.lane,
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunFrameStageDeclaration {
    pub stage: FunFrameStage,
    pub kind: EcsWorkKind,
    pub class: EcsSystemClass,
    pub domain: ScheduleDomain,
    pub lane: ScheduleLane,
    pub deadline: ScheduleDeadline,
    pub requiredness: WorkRequiredness,
    pub blocking: WorkBlockingClass,
    pub resource_read: Option<FunEcsResourceKind>,
    pub resource_write: Option<FunEcsResourceKind>,
}

impl FunFrameStageDeclaration {
    #[must_use]
    pub const fn new(stage: FunFrameStage) -> Self {
        Self {
            stage,
            kind: EcsWorkKind::RunSystem,
            class: EcsSystemClass::WorldQuery,
            domain: ScheduleDomain::FunEcs,
            lane: ScheduleLane::EcsSystem,
            deadline: ScheduleDeadline::Frame,
            requiredness: stage.requiredness(),
            blocking: WorkBlockingClass::NonBlocking,
            resource_read: None,
            resource_write: None,
        }
    }

    #[must_use]
    pub const fn with_execution(
        mut self,
        domain: ScheduleDomain,
        lane: ScheduleLane,
        deadline: ScheduleDeadline,
    ) -> Self {
        self.domain = domain;
        self.lane = lane;
        self.deadline = deadline;
        self
    }

    #[must_use]
    pub const fn with_class(mut self, class: EcsSystemClass) -> Self {
        self.class = class;
        self
    }

    #[must_use]
    pub const fn with_kind(mut self, kind: EcsWorkKind) -> Self {
        self.kind = kind;
        self
    }

    #[must_use]
    pub const fn with_requiredness(mut self, requiredness: WorkRequiredness) -> Self {
        self.requiredness = requiredness;
        self
    }

    #[must_use]
    pub const fn with_blocking(mut self) -> Self {
        self.domain = ScheduleDomain::Blocking;
        self.lane = ScheduleLane::Blocking;
        self.deadline = ScheduleDeadline::Stream;
        self.blocking = WorkBlockingClass::BlockingPool;
        self
    }

    #[must_use]
    pub const fn reads(mut self, resource: FunEcsResourceKind) -> Self {
        self.resource_read = Some(resource);
        self
    }

    #[must_use]
    pub const fn writes(mut self, resource: FunEcsResourceKind) -> Self {
        self.resource_write = Some(resource);
        self
    }

    #[must_use]
    pub const fn execution(self) -> EcsSystemExecutionContract {
        EcsSystemExecutionContract {
            domain: self.domain,
            lane: self.lane,
            deadline: self.deadline,
            budget: ScheduleBudget::UNBOUNDED,
            requiredness: self.requiredness,
            blocking_class: self.blocking,
            main_thread: MainThreadRequirement::AnyWorker,
            locality: WorkLocalityHint::NONE,
            cost: WorkCostHint::UNKNOWN,
            split: WorkSplitHint::Unsplittable,
        }
    }
}

fn stage_declarations(include_optional_work: bool) -> Vec<FunFrameStageDeclaration> {
    let mut stages = vec![
        FunFrameStageDeclaration::new(FunFrameStage::InputAcquire)
            .with_execution(
                ScheduleDomain::FunEcs,
                ScheduleLane::FrameCritical,
                ScheduleDeadline::Frame,
            )
            .writes(FunEcsResourceKind::TelemetryEventQueue),
        FunFrameStageDeclaration::new(FunFrameStage::EcsPreUpdate)
            .reads(FunEcsResourceKind::TelemetryEventQueue),
        FunFrameStageDeclaration::new(FunFrameStage::GameplayUpdate)
            .writes(FunEcsResourceKind::AiIntentQueue),
        FunFrameStageDeclaration::new(FunFrameStage::EcsCommandApply)
            .with_kind(EcsWorkKind::ApplyCommands)
            .with_execution(
                ScheduleDomain::FunEcs,
                ScheduleLane::EcsCommandBarrier,
                ScheduleDeadline::Frame,
            ),
        FunFrameStageDeclaration::new(FunFrameStage::SpatialSense)
            .with_class(EcsSystemClass::StreamInterestBuild)
            .reads(FunEcsResourceKind::SpatialGridRegistry),
        FunFrameStageDeclaration::new(FunFrameStage::SpatialBuildInterest)
            .with_class(EcsSystemClass::StreamInterestBuild)
            .reads(FunEcsResourceKind::PageResidencyTable)
            .writes(FunEcsResourceKind::StreamInterestTable),
        FunFrameStageDeclaration::new(FunFrameStage::SpatialDiffRequests)
            .with_class(EcsSystemClass::StreamRequestDiff)
            .reads(FunEcsResourceKind::StreamInterestTable)
            .writes(FunEcsResourceKind::StreamRequestQueue),
        FunFrameStageDeclaration::new(FunFrameStage::SourceAcquire)
            .with_class(EcsSystemClass::SourceAcquire)
            .with_blocking()
            .reads(FunEcsResourceKind::StreamRequestQueue)
            .writes(FunEcsResourceKind::SourceAcquireQueue),
        FunFrameStageDeclaration::new(FunFrameStage::SourceDecode)
            .with_class(EcsSystemClass::Decode)
            .with_execution(
                ScheduleDomain::EcsSpatialStreaming,
                ScheduleLane::ResourceBackground,
                ScheduleDeadline::Stream,
            )
            .reads(FunEcsResourceKind::SourceAcquireQueue)
            .writes(FunEcsResourceKind::DecodedPageQueue),
        FunFrameStageDeclaration::new(FunFrameStage::ArtifactBuild)
            .with_class(EcsSystemClass::DerivedArtifactBuild)
            .with_execution(
                ScheduleDomain::EcsSpatialStreaming,
                ScheduleLane::ResourceBackground,
                ScheduleDeadline::Stream,
            )
            .reads(FunEcsResourceKind::DecodedPageQueue)
            .writes(FunEcsResourceKind::DerivedArtifactRegistry),
        FunFrameStageDeclaration::new(FunFrameStage::DirtyPropagation)
            .with_class(EcsSystemClass::DirtyPropagation)
            .reads(FunEcsResourceKind::DerivedArtifactRegistry)
            .writes(FunEcsResourceKind::DirtyRegionLedger),
        FunFrameStageDeclaration::new(FunFrameStage::HandoffPublication)
            .with_class(EcsSystemClass::RendererHandoff)
            .reads(FunEcsResourceKind::DirtyRegionLedger)
            .writes(FunEcsResourceKind::RendererHandoffQueue),
        FunFrameStageDeclaration::new(FunFrameStage::AvisFixedStep)
            .with_execution(
                ScheduleDomain::AvisPhysics,
                ScheduleLane::PhysicsFixedStep,
                ScheduleDeadline::FixedStep,
            )
            .with_class(EcsSystemClass::PhysicsHandoff)
            .reads(FunEcsResourceKind::PhysicsCookQueue),
        FunFrameStageDeclaration::new(FunFrameStage::AvisCook)
            .with_execution(
                ScheduleDomain::AvisPhysics,
                ScheduleLane::PhysicsSolve,
                ScheduleDeadline::FixedStep,
            )
            .with_class(EcsSystemClass::PhysicsHandoff)
            .reads(FunEcsResourceKind::PhysicsCookQueue),
        FunFrameStageDeclaration::new(FunFrameStage::AvisWriteback)
            .with_execution(
                ScheduleDomain::AvisPhysics,
                ScheduleLane::PhysicsFixedStep,
                ScheduleDeadline::FixedStep,
            )
            .with_class(EcsSystemClass::PhysicsHandoff)
            .writes(FunEcsResourceKind::DirtyRegionLedger),
        FunFrameStageDeclaration::new(FunFrameStage::RvelteInput)
            .with_execution(
                ScheduleDomain::RvelteUi,
                ScheduleLane::UiSyncInput,
                ScheduleDeadline::Frame,
            )
            .reads(FunEcsResourceKind::RvelteUiPacketQueue),
        FunFrameStageDeclaration::new(FunFrameStage::RvelteState)
            .with_execution(
                ScheduleDomain::RvelteUi,
                ScheduleLane::UiAnimationFrame,
                ScheduleDeadline::Frame,
            )
            .writes(FunEcsResourceKind::RvelteUiPacketQueue),
        FunFrameStageDeclaration::new(FunFrameStage::RvelteLayout)
            .with_execution(
                ScheduleDomain::RvelteUi,
                ScheduleLane::UiDeferredLayout,
                ScheduleDeadline::Frame,
            )
            .reads(FunEcsResourceKind::RvelteUiPacketQueue),
        FunFrameStageDeclaration::new(FunFrameStage::RveltePaint)
            .with_execution(
                ScheduleDomain::RvelteUi,
                ScheduleLane::UiAnimationFrame,
                ScheduleDeadline::Frame,
            )
            .writes(FunEcsResourceKind::RvelteUiPacketQueue),
        FunFrameStageDeclaration::new(FunFrameStage::LuxConsume)
            .with_execution(
                ScheduleDomain::RendererLux,
                ScheduleLane::RenderGraphCompile,
                ScheduleDeadline::Frame,
            )
            .with_class(EcsSystemClass::LuxHandoff)
            .reads(FunEcsResourceKind::LuxHandoffQueue),
        FunFrameStageDeclaration::new(FunFrameStage::LuxPlan)
            .with_execution(
                ScheduleDomain::RendererLux,
                ScheduleLane::RenderGraphCompile,
                ScheduleDeadline::Frame,
            )
            .with_class(EcsSystemClass::LuxHandoff),
        FunFrameStageDeclaration::new(FunFrameStage::LuxPolicy)
            .with_execution(
                ScheduleDomain::RendererLux,
                ScheduleLane::RenderGraphCompile,
                ScheduleDeadline::Frame,
            )
            .with_class(EcsSystemClass::LuxHandoff),
        FunFrameStageDeclaration::new(FunFrameStage::RendererConsume)
            .with_execution(
                ScheduleDomain::Renderer,
                ScheduleLane::RenderExtract,
                ScheduleDeadline::Present,
            )
            .with_class(EcsSystemClass::RendererHandoff)
            .reads(FunEcsResourceKind::RendererHandoffQueue),
        FunFrameStageDeclaration::new(FunFrameStage::RendererUpload)
            .with_execution(
                ScheduleDomain::Renderer,
                ScheduleLane::RenderPrepare,
                ScheduleDeadline::Present,
            )
            .with_class(EcsSystemClass::GpuArtifactPrepare),
        FunFrameStageDeclaration::new(FunFrameStage::RendererExtract)
            .with_execution(
                ScheduleDomain::Renderer,
                ScheduleLane::RenderExtract,
                ScheduleDeadline::Present,
            )
            .with_class(EcsSystemClass::GpuArtifactPrepare),
        FunFrameStageDeclaration::new(FunFrameStage::RendererCompile)
            .with_execution(
                ScheduleDomain::Renderer,
                ScheduleLane::RenderGraphCompile,
                ScheduleDeadline::Present,
            )
            .with_class(EcsSystemClass::GpuArtifactPrepare),
        FunFrameStageDeclaration::new(FunFrameStage::RendererRecord)
            .with_execution(
                ScheduleDomain::Renderer,
                ScheduleLane::RenderRecord,
                ScheduleDeadline::Present,
            )
            .with_class(EcsSystemClass::GpuArtifactPrepare),
        FunFrameStageDeclaration::new(FunFrameStage::RendererSubmit)
            .with_execution(
                ScheduleDomain::Renderer,
                ScheduleLane::RenderSubmit,
                ScheduleDeadline::Present,
            )
            .with_class(EcsSystemClass::GpuArtifactPrepare),
        FunFrameStageDeclaration::new(FunFrameStage::RendererPresent)
            .with_execution(
                ScheduleDomain::Renderer,
                ScheduleLane::RenderPresent,
                ScheduleDeadline::Present,
            )
            .with_class(EcsSystemClass::GpuArtifactPrepare),
        FunFrameStageDeclaration::new(FunFrameStage::ThunderRelevance)
            .with_execution(
                ScheduleDomain::ThunderNetwork,
                ScheduleLane::NetworkRealtime,
                ScheduleDeadline::Frame,
            )
            .with_class(EcsSystemClass::NetworkHandoff)
            .reads(FunEcsResourceKind::NetworkHandoffQueue),
        FunFrameStageDeclaration::new(FunFrameStage::ThunderSnapshot)
            .with_execution(
                ScheduleDomain::ThunderNetwork,
                ScheduleLane::NetworkRealtime,
                ScheduleDeadline::Frame,
            )
            .with_class(EcsSystemClass::NetworkHandoff),
        FunFrameStageDeclaration::new(FunFrameStage::ThunderDelta)
            .with_execution(
                ScheduleDomain::ThunderNetwork,
                ScheduleLane::NetworkRealtime,
                ScheduleDeadline::Frame,
            )
            .with_class(EcsSystemClass::NetworkHandoff)
            .writes(FunEcsResourceKind::NetworkHandoffQueue),
    ];

    if include_optional_work {
        stages.push(
            FunFrameStageDeclaration::new(FunFrameStage::TelemetryFlush)
                .with_kind(EcsWorkKind::ScheduleDiagnostics)
                .with_execution(
                    ScheduleDomain::Telemetry,
                    ScheduleLane::TelemetryIngest,
                    ScheduleDeadline::IdleWindow,
                )
                .with_requiredness(WorkRequiredness::Optional)
                .reads(FunEcsResourceKind::TelemetryEventQueue),
        );
        stages.push(
            FunFrameStageDeclaration::new(FunFrameStage::EvictionRetirement)
                .with_class(EcsSystemClass::Eviction)
                .with_execution(
                    ScheduleDomain::FunEcs,
                    ScheduleLane::IdlePrefetch,
                    ScheduleDeadline::IdleWindow,
                )
                .with_requiredness(WorkRequiredness::Optional)
                .reads(FunEcsResourceKind::PageResidencyTable),
        );
    }
    stages
}

fn frame_wait_plans(
    intent: &FunFrameIntent,
    context: &FunFrameContext,
    stage_declarations: &[FunFrameStageDeclaration],
) -> Vec<FunFrameWaitPlan> {
    let mut waits = vec![
        wait(
            FunFrameWaitTokenKind::InputReady,
            FunFrameStage::InputAcquire,
            FunFrameStage::EcsPreUpdate,
            WaitForEdgeKind::AsyncJoin,
            FunFrameFallbackPlan::bounded(
                FunFrameFallbackKind::RenderArtifact,
                ScheduleDeadline::Frame,
            ),
        ),
        wait(
            FunFrameWaitTokenKind::CommandsApplied,
            FunFrameStage::EcsCommandApply,
            FunFrameStage::SpatialSense,
            WaitForEdgeKind::CommandBufferApply,
            FunFrameFallbackPlan::bounded(
                FunFrameFallbackKind::RenderArtifact,
                ScheduleDeadline::Frame,
            ),
        ),
        wait(
            FunFrameWaitTokenKind::SpatialDiffReady,
            FunFrameStage::SpatialDiffRequests,
            FunFrameStage::SourceAcquire,
            WaitForEdgeKind::CrossDomainHandoff,
            FunFrameFallbackPlan::bounded(
                FunFrameFallbackKind::RenderArtifact,
                ScheduleDeadline::Stream,
            ),
        ),
        wait(
            FunFrameWaitTokenKind::SourceAcquired,
            FunFrameStage::SourceAcquire,
            FunFrameStage::SourceDecode,
            WaitForEdgeKind::BlockingPermit,
            FunFrameFallbackPlan::bounded(
                FunFrameFallbackKind::RenderArtifact,
                ScheduleDeadline::Stream,
            ),
        ),
        wait(
            FunFrameWaitTokenKind::PagesDecoded,
            FunFrameStage::SourceDecode,
            FunFrameStage::ArtifactBuild,
            WaitForEdgeKind::CrossDomainHandoff,
            FunFrameFallbackPlan::bounded(
                FunFrameFallbackKind::RenderArtifact,
                ScheduleDeadline::Stream,
            ),
        ),
        wait(
            FunFrameWaitTokenKind::ArtifactsBuilt,
            FunFrameStage::ArtifactBuild,
            FunFrameStage::DirtyPropagation,
            WaitForEdgeKind::CrossDomainHandoff,
            FunFrameFallbackPlan::fallback(FunFrameFallbackKind::RenderArtifact),
        ),
        wait(
            FunFrameWaitTokenKind::DirtyPropagated,
            FunFrameStage::DirtyPropagation,
            FunFrameStage::HandoffPublication,
            WaitForEdgeKind::CrossDomainHandoff,
            FunFrameFallbackPlan::bounded(
                FunFrameFallbackKind::RenderArtifact,
                ScheduleDeadline::Frame,
            ),
        ),
        wait(
            FunFrameWaitTokenKind::HandoffsPublished,
            FunFrameStage::HandoffPublication,
            FunFrameStage::AvisFixedStep,
            WaitForEdgeKind::FixedStepWriteback,
            fallback_or_timeout(
                FunFrameFallbackKind::ConservativePhysicsProxy,
                context.fallback_availability.collision_proxy,
                ScheduleDeadline::FixedStep,
            ),
        ),
        wait(
            FunFrameWaitTokenKind::HandoffsPublished,
            FunFrameStage::HandoffPublication,
            FunFrameStage::RvelteInput,
            WaitForEdgeKind::UiSnapshot,
            fallback_or_timeout(
                FunFrameFallbackKind::HudPacket,
                context.fallback_availability.hud_packet,
                ScheduleDeadline::Frame,
            ),
        ),
        wait(
            FunFrameWaitTokenKind::HandoffsPublished,
            FunFrameStage::HandoffPublication,
            FunFrameStage::LuxConsume,
            WaitForEdgeKind::CrossDomainHandoff,
            fallback_or_timeout(
                FunFrameFallbackKind::LightingCache,
                context.fallback_availability.lighting,
                ScheduleDeadline::Frame,
            ),
        ),
        wait(
            FunFrameWaitTokenKind::PhysicsWritebackReady,
            FunFrameStage::AvisWriteback,
            FunFrameStage::RendererConsume,
            WaitForEdgeKind::FixedStepWriteback,
            fallback_or_timeout(
                FunFrameFallbackKind::ConservativePhysicsProxy,
                context.fallback_availability.collision_proxy,
                ScheduleDeadline::Frame,
            ),
        ),
        wait(
            FunFrameWaitTokenKind::RveltePaintReady,
            FunFrameStage::RveltePaint,
            FunFrameStage::RendererConsume,
            WaitForEdgeKind::UiSnapshot,
            fallback_or_timeout(
                FunFrameFallbackKind::HudPacket,
                context.fallback_availability.hud_packet,
                ScheduleDeadline::Present,
            ),
        ),
        wait(
            FunFrameWaitTokenKind::LuxPolicyReady,
            FunFrameStage::LuxPolicy,
            FunFrameStage::RendererConsume,
            WaitForEdgeKind::CrossDomainHandoff,
            fallback_or_timeout(
                FunFrameFallbackKind::LightingCache,
                context.fallback_availability.lighting,
                ScheduleDeadline::Present,
            ),
        ),
        wait(
            FunFrameWaitTokenKind::RendererSubmitReady,
            FunFrameStage::RendererSubmit,
            FunFrameStage::RendererPresent,
            WaitForEdgeKind::RendererSubmitPresent,
            fallback_or_timeout(
                FunFrameFallbackKind::RenderArtifact,
                context.fallback_availability.render_artifact,
                ScheduleDeadline::Present,
            ),
        ),
        wait(
            FunFrameWaitTokenKind::NetworkRelevanceReady,
            FunFrameStage::ThunderRelevance,
            FunFrameStage::ThunderSnapshot,
            WaitForEdgeKind::CrossDomainHandoff,
            fallback_or_timeout(
                FunFrameFallbackKind::NetworkDelta,
                context.fallback_availability.network_delta,
                ScheduleDeadline::Frame,
            ),
        ),
        wait(
            FunFrameWaitTokenKind::NetworkDeltaReady,
            FunFrameStage::ThunderDelta,
            FunFrameStage::TelemetryFlush,
            WaitForEdgeKind::CrossDomainHandoff,
            fallback_or_timeout(
                FunFrameFallbackKind::NetworkDelta,
                context.fallback_availability.network_delta,
                ScheduleDeadline::IdleWindow,
            ),
        ),
    ];

    if !intent.include_optional_work
        || !stage_declarations
            .iter()
            .any(|stage| stage.stage == FunFrameStage::TelemetryFlush)
    {
        waits.retain(|plan| plan.consumer != FunFrameStage::TelemetryFlush);
    }
    waits.extend(context.extra_wait_plans.iter().copied());
    waits
}

fn wait(
    token: FunFrameWaitTokenKind,
    producer: FunFrameStage,
    consumer: FunFrameStage,
    wait_kind: WaitForEdgeKind,
    fallback: FunFrameFallbackPlan,
) -> FunFrameWaitPlan {
    FunFrameWaitPlan {
        token,
        producer,
        consumer,
        wait_kind,
        requiredness: consumer.requiredness(),
        cancellation_stage: FunFrameStage::TelemetryFlush,
        fallback,
    }
}

const fn fallback_or_timeout(
    kind: FunFrameFallbackKind,
    available: bool,
    deadline: ScheduleDeadline,
) -> FunFrameFallbackPlan {
    if available {
        FunFrameFallbackPlan::fallback(kind)
    } else {
        FunFrameFallbackPlan::bounded(kind, deadline)
    }
}

fn resource_read_plans() -> Vec<FunFrameResourceReadPlan> {
    vec![
        resource_read(
            FunFrameStage::SourceAcquire,
            FunEcsResourceKind::StreamRequestQueue,
            FunFrameResourceSource::ProducedBy(FunFrameStage::SpatialDiffRequests),
        ),
        resource_read(
            FunFrameStage::SourceDecode,
            FunEcsResourceKind::SourceAcquireQueue,
            FunFrameResourceSource::ProducedBy(FunFrameStage::SourceAcquire),
        ),
        resource_read(
            FunFrameStage::ArtifactBuild,
            FunEcsResourceKind::DecodedPageQueue,
            FunFrameResourceSource::ProducedBy(FunFrameStage::SourceDecode),
        ),
        resource_read(
            FunFrameStage::HandoffPublication,
            FunEcsResourceKind::DerivedArtifactRegistry,
            FunFrameResourceSource::ProducedBy(FunFrameStage::ArtifactBuild),
        ),
        resource_read(
            FunFrameStage::AvisFixedStep,
            FunEcsResourceKind::PhysicsCookQueue,
            FunFrameResourceSource::ProducedBy(FunFrameStage::HandoffPublication),
        ),
        resource_read(
            FunFrameStage::RvelteInput,
            FunEcsResourceKind::RvelteUiPacketQueue,
            FunFrameResourceSource::ImportedFrom(FunEcsSubsystem::Rvelte),
        ),
        resource_read(
            FunFrameStage::LuxConsume,
            FunEcsResourceKind::LuxHandoffQueue,
            FunFrameResourceSource::ProducedBy(FunFrameStage::HandoffPublication),
        ),
        resource_read(
            FunFrameStage::RendererConsume,
            FunEcsResourceKind::RendererHandoffQueue,
            FunFrameResourceSource::ProducedBy(FunFrameStage::HandoffPublication),
        ),
        resource_read(
            FunFrameStage::ThunderRelevance,
            FunEcsResourceKind::NetworkHandoffQueue,
            FunFrameResourceSource::ProducedBy(FunFrameStage::HandoffPublication),
        ),
    ]
}

const fn resource_read(
    stage: FunFrameStage,
    resource: FunEcsResourceKind,
    source: FunFrameResourceSource,
) -> FunFrameResourceReadPlan {
    FunFrameResourceReadPlan {
        stage,
        resource,
        requiredness: stage.requiredness(),
        source,
    }
}

fn awaited_tokens_for_stage(
    declaration: FunFrameStageDeclaration,
    waits: &[FunFrameWaitPlan],
    frame: FunFrameId,
) -> Vec<WorkWaitToken> {
    waits
        .iter()
        .filter(|plan| plan.consumer == declaration.stage)
        .map(|plan| plan.token.scheduler_token(frame))
        .collect()
}

fn produced_tokens_for_stage(
    declaration: FunFrameStageDeclaration,
    waits: &[FunFrameWaitPlan],
    frame: FunFrameId,
) -> Vec<WorkWaitToken> {
    let mut tokens = Vec::new();
    for token in waits
        .iter()
        .filter(|plan| plan.producer == declaration.stage)
        .map(|plan| plan.token.scheduler_token(frame))
    {
        if !tokens.contains(&token) {
            tokens.push(token);
        }
    }
    tokens
}

fn frame_work(
    declaration: FunFrameStageDeclaration,
    observed_revision: FunRevision,
    awaited: &[WorkWaitToken],
    produced: &[WorkWaitToken],
) -> EcsWork<ProductRegistry> {
    let access = access_for_stage(declaration);
    EcsWork {
        kind: declaration.kind,
        class: declaration.class,
        system: Some(frame_system_id(declaration.stage)),
        access,
        chunk_key: EcsChunkKey::WHOLE_WORLD,
        command_buffer: None,
        deterministic_key: DeterministicDescriptor::new(
            declaration.stage.label(),
            u64::from(declaration.stage.ordinal()),
        ),
        world_revision: EcsWorldRevision::new(observed_revision.get()),
        awaited_tokens: awaited.to_vec(),
        produced_tokens: produced.to_vec(),
        liveness_class: if awaited.is_empty() {
            EcsLivenessClass::Normal
        } else {
            EcsLivenessClass::SchedulerWait
        },
        execution: declaration.execution(),
        non_send: false,
        main_thread_only: false,
    }
}

fn frame_work_node(
    id: WorkNodeId,
    declaration: FunFrameStageDeclaration,
    work: EcsWork<ProductRegistry>,
    awaited: &[WorkWaitToken],
    produced: &[WorkWaitToken],
) -> WorkNode<EcsWork<ProductRegistry>> {
    let mut liveness = WorkNodeLiveness::new()
        .with_may_wait(!awaited.is_empty())
        .with_blocking(match declaration.blocking {
            WorkBlockingClass::NonBlocking => WorkBlockingDisposition::NonBlocking,
            WorkBlockingClass::BlockingPool => WorkBlockingDisposition::BlockingPool,
            WorkBlockingClass::ExternalBlocking => WorkBlockingDisposition::ExternalBlocking,
            _ => WorkBlockingDisposition::ExternalBlocking,
        })
        .with_barrier_participation(true);
    for token in awaited {
        liveness = liveness.with_awaited_token(*token);
    }
    for token in produced {
        liveness = liveness.with_produced_token(*token);
    }
    WorkNode::new(
        id,
        declaration.domain,
        declaration.lane,
        declaration.kind.phase(),
        priority_for_stage(declaration),
        ScheduleBudget::UNBOUNDED,
        declaration.deadline,
        work,
    )
    .with_deterministic_descriptor(DeterministicDescriptor::new(
        declaration.stage.label(),
        u64::from(declaration.stage.ordinal()),
    ))
    .with_liveness_contract(liveness)
}

fn access_for_stage(declaration: FunFrameStageDeclaration) -> SystemAccess<ProductRegistry> {
    let mut access = SystemAccess::new();
    if let Some(resource) = declaration.resource_read {
        access = access.with_access(EcsAccess::read(EcsAccessTarget::Resource(resource_id(
            resource,
        ))));
    }
    if let Some(resource) = declaration.resource_write {
        access = access.with_access(EcsAccess::write(EcsAccessTarget::Resource(resource_id(
            resource,
        ))));
    }
    access
}

const fn resource_id(resource: FunEcsResourceKind) -> EcsResourceId {
    scheduler_resource_id(FunResourceId::from_resource_kind(resource))
}

const fn frame_system_id(stage: FunFrameStage) -> EcsSystemId {
    EcsSystemId::new(stage.ordinal() as u32 + 10_000)
}

const fn priority_for_stage(declaration: FunFrameStageDeclaration) -> TaskPriority {
    match declaration.deadline {
        ScheduleDeadline::Present | ScheduleDeadline::FixedStep => TaskPriority::Critical,
        ScheduleDeadline::Frame | ScheduleDeadline::Stream => TaskPriority::High,
        ScheduleDeadline::IdleWindow | ScheduleDeadline::None => TaskPriority::Idle,
        _ => TaskPriority::Normal,
    }
}

fn node_for_stage(stages: &[FunFrameStageNode], stage: FunFrameStage) -> Option<WorkNodeId> {
    stages
        .iter()
        .find_map(|node| (node.stage == stage).then_some(node.node_id))
}

fn successors(graph: &FunFrameGraph, node: WorkNodeId) -> Vec<WorkNodeId> {
    graph
        .edges
        .iter()
        .filter_map(|edge| match *edge {
            WorkEdge::Dependency { from, to }
            | WorkEdge::ResourceReadAfterWrite {
                writer: from,
                reader: to,
                ..
            } if from == node => Some(to),
            WorkEdge::Barrier { .. }
            | WorkEdge::Conflict { .. }
            | WorkEdge::StreamFairness { .. }
            | WorkEdge::CancellationPropagation { .. }
            | WorkEdge::Dependency { .. }
            | WorkEdge::ResourceReadAfterWrite { .. } => None,
            _ => None,
        })
        .collect()
}

fn imported_graphs(stages: &[FunFrameStageNode]) -> Vec<FunFrameImportedGraph> {
    [
        FunEcsSubsystem::Renderer,
        FunEcsSubsystem::Lux,
        FunEcsSubsystem::Avis,
        FunEcsSubsystem::Thunder,
        FunEcsSubsystem::Rvelte,
    ]
    .iter()
    .filter_map(|subsystem| imported_graph_for(*subsystem, stages))
    .collect()
}

fn imported_graph_for(
    subsystem: FunEcsSubsystem,
    stages: &[FunFrameStageNode],
) -> Option<FunFrameImportedGraph> {
    let owned = stages
        .iter()
        .filter(|stage| stage.subsystem == subsystem)
        .copied()
        .collect::<Vec<_>>();
    let first_stage = owned.first()?.stage;
    let last_stage = owned.last()?.stage;
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    hash = hash_u8(hash, subsystem as u8);
    for stage in &owned {
        hash = hash_u8(hash, stage.stage as u8);
        hash = hash_u64(hash, stage.node_id.get());
    }
    Some(FunFrameImportedGraph {
        subsystem,
        first_stage,
        last_stage,
        node_count: owned.len() as u32,
        digest: FunFrameDigest { value: hash },
    })
}

impl FunFrameDigest {
    #[must_use]
    pub fn from_parts(
        intent: &FunFrameIntent,
        context: &FunFrameContext,
        graph: &FunFrameGraph,
        waits: &[FunFrameWaitPlan],
        reads: &[FunFrameResourceReadPlan],
        imports: &[FunFrameImportedGraph],
    ) -> Self {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        hash = hash_u64(hash, intent.graph_id.get());
        hash = hash_u64(hash, intent.frame.get());
        hash = hash_u64(hash, context.observed_revision.get());
        hash = hash_u64(hash, u64::from(context.active_systems));
        hash = hash_u64(hash, u64::from(context.active_cameras));
        hash = hash_u64(hash, u64::from(context.stream_requests));
        hash = hash_u64(hash, u64::from(context.dirty_regions));
        hash = hash_u64(hash, u64::from(context.pending_commands));
        for node in &graph.nodes {
            hash = hash_u64(hash, node.id.get());
            hash = hash_u8(hash, node.work_descriptor.kind as u8);
            hash = hash_u8(hash, node.work_descriptor.class as u8);
            hash = hash_u8(hash, node.domain as u8);
            hash = hash_u8(hash, node.lane as u8);
            hash = hash_u64(hash, node.work_descriptor.deterministic_key.sort_key);
        }
        for edge in &graph.edges {
            hash = hash_u8(hash, edge_label_id(edge));
        }
        for wait in waits {
            hash = hash_u8(hash, wait.token as u8);
            hash = hash_u8(hash, wait.producer as u8);
            hash = hash_u8(hash, wait.consumer as u8);
        }
        for read in reads {
            hash = hash_u8(hash, read.stage as u8);
            hash = hash_u8(hash, read.resource as u8);
        }
        for import in imports {
            hash = hash_u8(hash, import.subsystem as u8);
            hash = hash_u64(hash, u64::from(import.node_count));
            hash = hash_u64(hash, import.digest.value);
        }
        Self { value: hash }
    }
}

impl FunFrameReport {
    #[must_use]
    pub fn from_graph(
        graph: &FunFrameGraph,
        stages: &[FunFrameStageNode],
        reads: &[FunFrameResourceReadPlan],
        imports: &[FunFrameImportedGraph],
        max_budget_pressure: u8,
        digest: FunFrameDigest,
        liveness_proven: bool,
    ) -> Self {
        Self {
            stages: stages.len() as u32,
            graph_nodes: graph.nodes.len() as u32,
            dependency_edges: graph
                .edges
                .iter()
                .filter(|edge| matches!(edge, WorkEdge::Dependency { .. }))
                .count() as u32,
            wait_edges: graph.wait_for_edges.len() as u32,
            cancellation_edges: graph
                .edges
                .iter()
                .filter(|edge| matches!(edge, WorkEdge::CancellationPropagation { .. }))
                .count() as u32,
            imported_graphs: imports.len() as u32,
            required_resource_reads: reads
                .iter()
                .filter(|read| !read.requiredness.is_optional())
                .count() as u32,
            optional_nodes: stages
                .iter()
                .filter(|stage| stage.requiredness.is_optional())
                .count() as u32,
            liveness_proven,
            blocking_nodes: graph
                .nodes
                .iter()
                .filter(|node| node.liveness.blocking.can_block())
                .count() as u32,
            max_budget_pressure,
            digest,
        }
    }
}

fn edge_label_id(edge: &WorkEdge) -> u8 {
    match edge {
        WorkEdge::Dependency { .. } => 0,
        WorkEdge::Barrier { .. } => 1,
        WorkEdge::ResourceReadAfterWrite { .. } => 2,
        WorkEdge::Conflict { .. } => 3,
        WorkEdge::StreamFairness { .. } => 4,
        WorkEdge::CancellationPropagation { .. } => 5,
        _ => 255,
    }
}

const fn hash_u8(hash: u64, value: u8) -> u64 {
    (hash ^ value as u64).wrapping_mul(0x0000_0100_0000_01b3)
}

const fn hash_u64(mut hash: u64, value: u64) -> u64 {
    hash = hash_u8(hash, (value & 0xff) as u8);
    hash = hash_u8(hash, ((value >> 8) & 0xff) as u8);
    hash = hash_u8(hash, ((value >> 16) & 0xff) as u8);
    hash = hash_u8(hash, ((value >> 24) & 0xff) as u8);
    hash = hash_u8(hash, ((value >> 32) & 0xff) as u8);
    hash = hash_u8(hash, ((value >> 40) & 0xff) as u8);
    hash = hash_u8(hash, ((value >> 48) & 0xff) as u8);
    hash_u8(hash, ((value >> 56) & 0xff) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_compiler_emits_unified_project_fun_graph() {
        let schedule = FunFrameCompiler::compile_with_intent(
            FunFrameIntent::default().with_frame(FunFrameId::new(9)),
            FunFrameContext::default()
                .with_active_cameras(2)
                .with_stream_requests(4)
                .with_dirty_regions(3)
                .with_pending_commands(5),
        )
        .expect("compile unified frame graph");

        assert_eq!(schedule.report.stages as usize, FUN_FRAME_STAGE_COUNT);
        assert_eq!(schedule.graph.domain, ScheduleDomain::FunEcs);
        assert_eq!(schedule.imported_graphs.len(), 5);
        assert!(schedule.report.liveness_proven);
        assert_ne!(schedule.digest.value, 0);
        assert!(
            schedule
                .stages
                .iter()
                .any(|stage| stage.stage == FunFrameStage::RendererPresent
                    && stage.lane == ScheduleLane::RenderPresent)
        );
        assert!(
            schedule
                .stages
                .iter()
                .any(|stage| stage.stage == FunFrameStage::ThunderDelta
                    && stage.domain == ScheduleDomain::ThunderNetwork)
        );
    }

    #[test]
    fn frame_compiler_tracks_imported_subsystem_graphs() {
        let schedule = FunFrameCompiler::compile(FunFrameContext::default())
            .expect("compile unified frame graph");
        let renderer = schedule
            .imported_graphs
            .iter()
            .find(|graph| graph.subsystem == FunEcsSubsystem::Renderer)
            .expect("renderer import");
        let rvelte = schedule
            .imported_graphs
            .iter()
            .find(|graph| graph.subsystem == FunEcsSubsystem::Rvelte)
            .expect("rvelte import");

        assert_eq!(renderer.first_stage, FunFrameStage::RendererConsume);
        assert_eq!(renderer.last_stage, FunFrameStage::RendererPresent);
        assert_eq!(renderer.node_count, 7);
        assert_eq!(rvelte.node_count, 4);
    }

    #[test]
    fn required_resource_reads_need_producer_import_or_fallback() {
        let declarations = stage_declarations(true);
        let invalid = [FunFrameResourceReadPlan {
            stage: FunFrameStage::RendererConsume,
            resource: FunEcsResourceKind::RendererHandoffQueue,
            requiredness: WorkRequiredness::Required,
            source: FunFrameResourceSource::ProducedBy(FunFrameStage::TelemetryFlush),
        }];

        let err = FunFrameCompiler::validate_resource_reads(
            &invalid,
            &FunFrameContext::default(),
            &declarations,
        )
        .expect_err("late optional producer cannot satisfy renderer consume");

        assert_eq!(
            err,
            FunFrameCompileError::RequiredResourceReadWithoutProducerImportOrFallback {
                stage: FunFrameStage::RendererConsume,
                resource: FunEcsResourceKind::RendererHandoffQueue
            }
        );
    }

    #[test]
    fn optional_work_cannot_gate_required_present_fixed_step_or_network_paths() {
        let mut graph = WorkGraph::new(
            WorkGraphId::new(2_601),
            ScheduleDomain::FunEcs,
            GraphExecutionMode::DeterministicParallel,
            CommitPolicy::DescriptorOrder,
        );
        let telemetry = FunFrameStageNode {
            node_id: WorkNodeId::new(0),
            stage: FunFrameStage::TelemetryFlush,
            subsystem: FunEcsSubsystem::Telemetry,
            domain: ScheduleDomain::Telemetry,
            lane: ScheduleLane::TelemetryIngest,
            requiredness: WorkRequiredness::Optional,
            awaited_token_count: 0,
            produced_token_count: 0,
        };
        let present = FunFrameStageNode {
            node_id: WorkNodeId::new(1),
            stage: FunFrameStage::RendererPresent,
            subsystem: FunEcsSubsystem::Renderer,
            domain: ScheduleDomain::Renderer,
            lane: ScheduleLane::RenderPresent,
            requiredness: WorkRequiredness::Required,
            awaited_token_count: 0,
            produced_token_count: 0,
        };
        graph.add_edge(WorkEdge::Dependency {
            from: telemetry.node_id,
            to: present.node_id,
        });

        let err = FunFrameCompiler::validate_optional_work_does_not_gate_required_paths(
            &graph,
            &[telemetry, present],
        )
        .expect_err("optional telemetry cannot gate present");

        assert_eq!(
            err,
            FunFrameCompileError::OptionalWorkGatesRequiredPath {
                optional: FunFrameStage::TelemetryFlush,
                required: FunFrameStage::RendererPresent
            }
        );
    }

    #[test]
    fn blocking_source_acquire_uses_only_blocking_lane() {
        let schedule = FunFrameCompiler::compile(FunFrameContext::default())
            .expect("compile unified frame graph");
        let source_acquire = schedule
            .stages
            .iter()
            .find(|stage| stage.stage == FunFrameStage::SourceAcquire)
            .expect("source acquire stage");

        assert_eq!(source_acquire.domain, ScheduleDomain::Blocking);
        assert_eq!(source_acquire.lane, ScheduleLane::Blocking);
        assert_eq!(schedule.report.blocking_nodes, 1);
    }

    #[test]
    fn private_subsystem_queue_bypass_is_rejected() {
        let err = FunFrameCompiler::compile(
            FunFrameContext::default().with_private_queue_bypass(FunEcsSubsystem::Renderer),
        )
        .expect_err("private renderer bypass must be rejected");

        assert_eq!(
            err,
            FunFrameCompileError::PrivateSubsystemQueueBypass {
                subsystem: FunEcsSubsystem::Renderer
            }
        );
    }

    #[test]
    fn every_wait_token_declares_producer_consumer_cancellation_and_fallback() {
        let schedule = FunFrameCompiler::compile(FunFrameContext::default())
            .expect("compile unified frame graph");

        assert!(!schedule.wait_plans.is_empty());
        assert!(schedule.wait_plans.iter().all(|plan| {
            schedule
                .stages
                .iter()
                .any(|stage| stage.stage == plan.producer)
                && schedule
                    .stages
                    .iter()
                    .any(|stage| stage.stage == plan.consumer)
                && schedule
                    .stages
                    .iter()
                    .any(|stage| stage.stage == plan.cancellation_stage)
                && plan.fallback.has_timeout_or_fallback()
        }));
        assert_eq!(schedule.report.wait_edges, schedule.wait_plans.len() as u32);
        assert_eq!(
            schedule.report.cancellation_edges,
            schedule.wait_plans.len() as u32
        );
    }

    #[test]
    fn deterministic_digest_is_stable_for_same_frame_inputs() {
        let context = FunFrameContext::default()
            .with_observed_revision(FunRevision::new(33))
            .with_active_cameras(1)
            .with_stream_requests(2);
        let a = FunFrameCompiler::compile_with_intent(
            FunFrameIntent::default().with_frame(FunFrameId::new(12)),
            context.clone(),
        )
        .expect("compile a");
        let b = FunFrameCompiler::compile_with_intent(
            FunFrameIntent::default().with_frame(FunFrameId::new(12)),
            context,
        )
        .expect("compile b");

        assert_eq!(a.digest, b.digest);
        assert_eq!(a.report.digest, b.report.digest);
    }

    #[test]
    fn unavailable_required_subsystem_needs_fallback() {
        let mut context = FunFrameContext::default();
        context.subsystem_readiness.renderer = false;
        context.fallback_availability.render_artifact = false;

        let err = FunFrameCompiler::compile(context)
            .expect_err("renderer unready without fallback rejects");

        assert_eq!(
            err,
            FunFrameCompileError::RequiredSubsystemUnavailable {
                subsystem: FunEcsSubsystem::Renderer
            }
        );
    }
}
