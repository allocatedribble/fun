use fun_scheduler_types::{
    CommitPolicy, DeterministicDescriptor, EcsAccessMode, EcsChunkKey, EcsCommandBufferId,
    EcsLivenessClass, EcsSpatialDomainKind, EcsSpatialWaitTokenKind, EcsSystemClass,
    EcsSystemDescriptor, EcsSystemExecutionContract, EcsWork, EcsWorkKind, EcsWorldRevision,
    GraphExecutionMode, GraphInvariantError, LivenessProof, MainThreadRequirement, ProductRegistry,
    ScheduleBudget, ScheduleDeadline, ScheduleDomain, ScheduleLane, SystemAccess, TaskPriority,
    WaitForEdge, WaitForEdgeKind, WorkBlockingClass, WorkBlockingDisposition, WorkCostHint,
    WorkEdge, WorkGraph, WorkGraphId, WorkLocalityHint, WorkNode, WorkNodeId, WorkNodeLiveness,
    WorkRequiredness, WorkSplitHint, WorkWaitToken,
};

use crate::{
    EcsArtifactConsumer, FUN_COMMAND_BUFFER_ARTIFACTS, FUN_COMMAND_BUFFER_DIRTY_PROPAGATION,
    FUN_COMMAND_BUFFER_HANDOFFS, FUN_COMMAND_BUFFER_SPATIAL_REQUESTS, FunEcsComponentKind,
    FunEcsResourceKind, FunResourceTableId, FunRevision, FunSchedulerEcsRegistry, FunSystemAccess,
    FunSystemChunkPolicy, FunSystemDescriptor, FunSystemId, FunSystemValidationError,
};

pub const ECS_SPATIAL_SCHEDULE_SET_COUNT: usize = 15;
pub const ECS_SPATIAL_COMPILED_SCHEDULE_NODE_COUNT: usize = 18;

pub const ECS_SPATIAL_SCHEDULE_FRAME_ORDER: [EcsSpatialScheduleSet;
    ECS_SPATIAL_SCHEDULE_SET_COUNT] = [
    EcsSpatialScheduleSet::SenseSources,
    EcsSpatialScheduleSet::BuildInterest,
    EcsSpatialScheduleSet::PlanStreamWave,
    EcsSpatialScheduleSet::DiffRequests,
    EcsSpatialScheduleSet::ApplyStreamCommands,
    EcsSpatialScheduleSet::AcquireSources,
    EcsSpatialScheduleSet::DecodePages,
    EcsSpatialScheduleSet::BuildDerivedArtifacts,
    EcsSpatialScheduleSet::PropagateDirtyRegions,
    EcsSpatialScheduleSet::PublishRendererHandoffs,
    EcsSpatialScheduleSet::PublishLuxHandoffs,
    EcsSpatialScheduleSet::PublishPhysicsHandoffs,
    EcsSpatialScheduleSet::PublishNetworkHandoffs,
    EcsSpatialScheduleSet::EvictColdPages,
    EcsSpatialScheduleSet::FlushDiagnostics,
];

pub const ECS_SPATIAL_COMPILED_SCHEDULE_FRAME_ORDER: [EcsSpatialCompiledScheduleNode;
    ECS_SPATIAL_COMPILED_SCHEDULE_NODE_COUNT] = [
    EcsSpatialCompiledScheduleNode::set(0, EcsSpatialScheduleSet::SenseSources),
    EcsSpatialCompiledScheduleNode::set(1, EcsSpatialScheduleSet::BuildInterest),
    EcsSpatialCompiledScheduleNode::set(2, EcsSpatialScheduleSet::PlanStreamWave),
    EcsSpatialCompiledScheduleNode::set(3, EcsSpatialScheduleSet::DiffRequests),
    EcsSpatialCompiledScheduleNode::barrier(
        4,
        EcsSpatialCommandBarrierKind::ApplyRequestCommands,
        EcsSpatialScheduleSet::DiffRequests,
    ),
    EcsSpatialCompiledScheduleNode::set(5, EcsSpatialScheduleSet::AcquireSources),
    EcsSpatialCompiledScheduleNode::set(6, EcsSpatialScheduleSet::DecodePages),
    EcsSpatialCompiledScheduleNode::set(7, EcsSpatialScheduleSet::BuildDerivedArtifacts),
    EcsSpatialCompiledScheduleNode::barrier(
        8,
        EcsSpatialCommandBarrierKind::ApplyArtifactCommands,
        EcsSpatialScheduleSet::BuildDerivedArtifacts,
    ),
    EcsSpatialCompiledScheduleNode::set(9, EcsSpatialScheduleSet::PropagateDirtyRegions),
    EcsSpatialCompiledScheduleNode::barrier(
        10,
        EcsSpatialCommandBarrierKind::ApplyDirtyPropagationCommands,
        EcsSpatialScheduleSet::PropagateDirtyRegions,
    ),
    EcsSpatialCompiledScheduleNode::set(11, EcsSpatialScheduleSet::PublishRendererHandoffs),
    EcsSpatialCompiledScheduleNode::set(12, EcsSpatialScheduleSet::PublishLuxHandoffs),
    EcsSpatialCompiledScheduleNode::set(13, EcsSpatialScheduleSet::PublishPhysicsHandoffs),
    EcsSpatialCompiledScheduleNode::set(14, EcsSpatialScheduleSet::PublishNetworkHandoffs),
    EcsSpatialCompiledScheduleNode::barrier(
        15,
        EcsSpatialCommandBarrierKind::ApplyHandoffCommands,
        EcsSpatialScheduleSet::PublishNetworkHandoffs,
    ),
    EcsSpatialCompiledScheduleNode::set(16, EcsSpatialScheduleSet::EvictColdPages),
    EcsSpatialCompiledScheduleNode::set(17, EcsSpatialScheduleSet::FlushDiagnostics),
];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum EcsSpatialScheduleSet {
    #[default]
    SenseSources = 0,
    BuildInterest = 1,
    PlanStreamWave = 2,
    DiffRequests = 3,
    ApplyStreamCommands = 4,
    AcquireSources = 5,
    DecodePages = 6,
    BuildDerivedArtifacts = 7,
    PropagateDirtyRegions = 8,
    PublishRendererHandoffs = 9,
    PublishLuxHandoffs = 10,
    PublishPhysicsHandoffs = 11,
    PublishNetworkHandoffs = 12,
    EvictColdPages = 13,
    FlushDiagnostics = 14,
}

impl EcsSpatialScheduleSet {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::SenseSources => "sense_sources",
            Self::BuildInterest => "build_interest",
            Self::PlanStreamWave => "plan_stream_wave",
            Self::DiffRequests => "diff_requests",
            Self::ApplyStreamCommands => "apply_stream_commands",
            Self::AcquireSources => "acquire_sources",
            Self::DecodePages => "decode_pages",
            Self::BuildDerivedArtifacts => "build_derived_artifacts",
            Self::PropagateDirtyRegions => "propagate_dirty_regions",
            Self::PublishRendererHandoffs => "publish_renderer_handoffs",
            Self::PublishLuxHandoffs => "publish_lux_handoffs",
            Self::PublishPhysicsHandoffs => "publish_physics_handoffs",
            Self::PublishNetworkHandoffs => "publish_network_handoffs",
            Self::EvictColdPages => "evict_cold_pages",
            Self::FlushDiagnostics => "flush_diagnostics",
        }
    }

    #[must_use]
    pub const fn all() -> &'static [Self] {
        &ECS_SPATIAL_SCHEDULE_FRAME_ORDER
    }

    #[must_use]
    pub const fn ordinal(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn next(self) -> Option<Self> {
        match self {
            Self::SenseSources => Some(Self::BuildInterest),
            Self::BuildInterest => Some(Self::PlanStreamWave),
            Self::PlanStreamWave => Some(Self::DiffRequests),
            Self::DiffRequests => Some(Self::ApplyStreamCommands),
            Self::ApplyStreamCommands => Some(Self::AcquireSources),
            Self::AcquireSources => Some(Self::DecodePages),
            Self::DecodePages => Some(Self::BuildDerivedArtifacts),
            Self::BuildDerivedArtifacts => Some(Self::PropagateDirtyRegions),
            Self::PropagateDirtyRegions => Some(Self::PublishRendererHandoffs),
            Self::PublishRendererHandoffs => Some(Self::PublishLuxHandoffs),
            Self::PublishLuxHandoffs => Some(Self::PublishPhysicsHandoffs),
            Self::PublishPhysicsHandoffs => Some(Self::PublishNetworkHandoffs),
            Self::PublishNetworkHandoffs => Some(Self::EvictColdPages),
            Self::EvictColdPages => Some(Self::FlushDiagnostics),
            Self::FlushDiagnostics => None,
        }
    }

    #[must_use]
    pub const fn class(self) -> EcsSystemClass {
        match self {
            Self::SenseSources => EcsSystemClass::WorldQuery,
            Self::BuildInterest => EcsSystemClass::StreamInterestBuild,
            Self::PlanStreamWave => EcsSystemClass::StreamPlan,
            Self::DiffRequests => EcsSystemClass::StreamRequestDiff,
            Self::ApplyStreamCommands => EcsSystemClass::DirtyPropagation,
            Self::AcquireSources => EcsSystemClass::SourceAcquire,
            Self::DecodePages => EcsSystemClass::Decode,
            Self::BuildDerivedArtifacts => EcsSystemClass::DerivedArtifactBuild,
            Self::PropagateDirtyRegions => EcsSystemClass::DirtyPropagation,
            Self::PublishRendererHandoffs => EcsSystemClass::RendererHandoff,
            Self::PublishLuxHandoffs => EcsSystemClass::LuxHandoff,
            Self::PublishPhysicsHandoffs => EcsSystemClass::PhysicsHandoff,
            Self::PublishNetworkHandoffs => EcsSystemClass::NetworkHandoff,
            Self::EvictColdPages => EcsSystemClass::Eviction,
            Self::FlushDiagnostics => EcsSystemClass::Diagnostics,
        }
    }

    #[must_use]
    pub const fn work_kind(self) -> EcsWorkKind {
        match self {
            Self::ApplyStreamCommands => EcsWorkKind::ApplyCommands,
            Self::DecodePages | Self::BuildDerivedArtifacts => EcsWorkKind::RunSystemChunk,
            Self::FlushDiagnostics => EcsWorkKind::ScheduleDiagnostics,
            Self::SenseSources
            | Self::BuildInterest
            | Self::PlanStreamWave
            | Self::DiffRequests
            | Self::AcquireSources
            | Self::PropagateDirtyRegions
            | Self::PublishRendererHandoffs
            | Self::PublishLuxHandoffs
            | Self::PublishPhysicsHandoffs
            | Self::PublishNetworkHandoffs
            | Self::EvictColdPages => EcsWorkKind::RunSystem,
        }
    }

    #[must_use]
    pub const fn is_chunk_parallel(self) -> bool {
        matches!(self, Self::DecodePages | Self::BuildDerivedArtifacts)
    }

    #[must_use]
    pub const fn execution_contract(self) -> EcsSystemExecutionContract {
        match self {
            Self::SenseSources => frame_ecs_contract(),
            Self::BuildInterest => frame_ecs_contract(),
            Self::PlanStreamWave => renderer_page_scheduler_contract(ScheduleDeadline::Frame),
            Self::DiffRequests => renderer_page_scheduler_contract(ScheduleDeadline::Frame),
            Self::ApplyStreamCommands => EcsSystemExecutionContract::COMMAND_BARRIER,
            Self::AcquireSources => blocking_stream_contract(),
            Self::DecodePages => chunked_page_scheduler_contract(),
            Self::BuildDerivedArtifacts => {
                Self::build_derived_artifact_execution(EcsArtifactConsumer::Renderer)
            }
            Self::PropagateDirtyRegions => frame_ecs_contract(),
            Self::PublishRendererHandoffs => Self::renderer_handoff_execution(true),
            Self::PublishLuxHandoffs => Self::lux_handoff_execution(true),
            Self::PublishPhysicsHandoffs => Self::physics_handoff_execution(true),
            Self::PublishNetworkHandoffs => network_handoff_contract(),
            Self::EvictColdPages => idle_page_scheduler_contract(),
            Self::FlushDiagnostics => telemetry_idle_contract(),
        }
    }

    #[must_use]
    pub const fn execution_with_deadline(
        self,
        deadline: ScheduleDeadline,
    ) -> EcsSystemExecutionContract {
        EcsSystemExecutionContract {
            deadline,
            ..self.execution_contract()
        }
    }

    #[must_use]
    pub const fn build_derived_artifact_execution(
        consumer: EcsArtifactConsumer,
    ) -> EcsSystemExecutionContract {
        match consumer {
            EcsArtifactConsumer::Renderer => EcsSystemExecutionContract {
                domain: ScheduleDomain::Renderer,
                lane: ScheduleLane::RenderPrepare,
                deadline: ScheduleDeadline::Stream,
                split: WorkSplitHint::Splittable,
                ..non_blocking_contract()
            },
            EcsArtifactConsumer::Lux => EcsSystemExecutionContract {
                domain: ScheduleDomain::RendererLux,
                lane: ScheduleLane::RenderGraphCompile,
                deadline: ScheduleDeadline::Stream,
                split: WorkSplitHint::Splittable,
                ..non_blocking_contract()
            },
            EcsArtifactConsumer::AvisPhysics => EcsSystemExecutionContract {
                domain: ScheduleDomain::AvisPhysics,
                lane: ScheduleLane::PhysicsSolve,
                deadline: ScheduleDeadline::Stream,
                split: WorkSplitHint::Splittable,
                ..non_blocking_contract()
            },
            EcsArtifactConsumer::ThunderNetwork => EcsSystemExecutionContract {
                domain: ScheduleDomain::ThunderNetwork,
                lane: ScheduleLane::NetworkRealtime,
                deadline: ScheduleDeadline::Stream,
                split: WorkSplitHint::Splittable,
                ..non_blocking_contract()
            },
            EcsArtifactConsumer::Navigation
            | EcsArtifactConsumer::Audio
            | EcsArtifactConsumer::Telemetry
            | EcsArtifactConsumer::Editor => chunked_page_scheduler_contract(),
        }
    }

    #[must_use]
    pub const fn renderer_handoff_execution(visible_or_near: bool) -> EcsSystemExecutionContract {
        EcsSystemExecutionContract {
            domain: ScheduleDomain::Renderer,
            lane: ScheduleLane::RenderPrepare,
            deadline: if visible_or_near {
                ScheduleDeadline::Frame
            } else {
                ScheduleDeadline::Stream
            },
            ..non_blocking_contract()
        }
    }

    #[must_use]
    pub const fn lux_handoff_execution(shadow_critical: bool) -> EcsSystemExecutionContract {
        EcsSystemExecutionContract {
            domain: ScheduleDomain::RendererLux,
            lane: ScheduleLane::RenderGraphCompile,
            deadline: if shadow_critical {
                ScheduleDeadline::Frame
            } else {
                ScheduleDeadline::Stream
            },
            ..non_blocking_contract()
        }
    }

    #[must_use]
    pub const fn physics_handoff_execution(fixed_step: bool) -> EcsSystemExecutionContract {
        EcsSystemExecutionContract {
            domain: ScheduleDomain::AvisPhysics,
            lane: if fixed_step {
                ScheduleLane::PhysicsFixedStep
            } else {
                ScheduleLane::PhysicsSolve
            },
            deadline: if fixed_step {
                ScheduleDeadline::FixedStep
            } else {
                ScheduleDeadline::Stream
            },
            ..non_blocking_contract()
        }
    }

    #[must_use]
    pub fn system_descriptor(self) -> FunSystemDescriptor {
        spatial_system_descriptor(self)
    }

    pub fn scheduler_descriptor(
        self,
    ) -> Result<EcsSystemDescriptor<FunSchedulerEcsRegistry>, FunSystemValidationError> {
        self.system_descriptor().to_scheduler_descriptor()
    }
}

#[must_use]
pub fn spatial_system_descriptor(set: EcsSpatialScheduleSet) -> FunSystemDescriptor {
    let mut descriptor = FunSystemDescriptor::new(spatial_system_id(set), set.label())
        .with_class(set.class())
        .with_execution(set.execution_contract())
        .with_chunk_policy(if set.is_chunk_parallel() {
            FunSystemChunkPolicy::ChunkParallel
        } else {
            FunSystemChunkPolicy::WholeWorld
        })
        .with_access(spatial_set_access(set));

    descriptor = match set {
        EcsSpatialScheduleSet::DiffRequests => {
            descriptor.with_command_output(FUN_COMMAND_BUFFER_SPATIAL_REQUESTS)
        }
        EcsSpatialScheduleSet::ApplyStreamCommands => descriptor
            .with_chunk_policy(FunSystemChunkPolicy::CommandBarrier)
            .with_direct_world_structure_write(),
        EcsSpatialScheduleSet::DecodePages => descriptor
            .with_liveness_class(EcsLivenessClass::SchedulerWait)
            .with_awaited_token(spatial_token(EcsSpatialWaitTokenKind::PageSourceReady))
            .with_produced_token(spatial_token(EcsSpatialWaitTokenKind::PageDecoded)),
        EcsSpatialScheduleSet::AcquireSources => {
            descriptor.with_produced_token(spatial_token(EcsSpatialWaitTokenKind::PageSourceReady))
        }
        EcsSpatialScheduleSet::BuildDerivedArtifacts => descriptor
            .with_liveness_class(EcsLivenessClass::SchedulerWait)
            .with_awaited_token(spatial_token(EcsSpatialWaitTokenKind::PageDecoded))
            .with_produced_token(spatial_token(EcsSpatialWaitTokenKind::DerivedArtifactReady))
            .with_command_output(FUN_COMMAND_BUFFER_ARTIFACTS),
        EcsSpatialScheduleSet::PropagateDirtyRegions => descriptor
            .with_command_output(FUN_COMMAND_BUFFER_DIRTY_PROPAGATION)
            .reads_post_command_state(),
        EcsSpatialScheduleSet::PublishRendererHandoffs => descriptor
            .with_liveness_class(EcsLivenessClass::SchedulerWait)
            .with_awaited_token(spatial_token(EcsSpatialWaitTokenKind::DerivedArtifactReady))
            .with_produced_token(spatial_token(
                EcsSpatialWaitTokenKind::RendererArtifactPublished,
            ))
            .with_command_output(FUN_COMMAND_BUFFER_HANDOFFS)
            .reads_post_command_state(),
        EcsSpatialScheduleSet::PublishLuxHandoffs => descriptor
            .with_liveness_class(EcsLivenessClass::SchedulerWait)
            .with_awaited_token(spatial_token(EcsSpatialWaitTokenKind::DerivedArtifactReady))
            .with_produced_token(spatial_token(EcsSpatialWaitTokenKind::ShadowArtifactReady))
            .with_command_output(FUN_COMMAND_BUFFER_HANDOFFS)
            .reads_post_command_state(),
        EcsSpatialScheduleSet::PublishPhysicsHandoffs => descriptor
            .with_liveness_class(EcsLivenessClass::SchedulerWait)
            .with_awaited_token(spatial_token(EcsSpatialWaitTokenKind::DerivedArtifactReady))
            .with_produced_token(spatial_token(EcsSpatialWaitTokenKind::PhysicsProxyReady))
            .with_command_output(FUN_COMMAND_BUFFER_HANDOFFS)
            .reads_post_command_state(),
        EcsSpatialScheduleSet::PublishNetworkHandoffs => descriptor
            .with_liveness_class(EcsLivenessClass::SchedulerWait)
            .with_awaited_token(spatial_token(EcsSpatialWaitTokenKind::DerivedArtifactReady))
            .with_produced_token(spatial_token(
                EcsSpatialWaitTokenKind::NetworkDeltaPublished,
            ))
            .with_command_output(FUN_COMMAND_BUFFER_HANDOFFS)
            .reads_post_command_state(),
        EcsSpatialScheduleSet::FlushDiagnostics => {
            descriptor.with_chunk_policy(FunSystemChunkPolicy::Diagnostics)
        }
        EcsSpatialScheduleSet::SenseSources
        | EcsSpatialScheduleSet::BuildInterest
        | EcsSpatialScheduleSet::PlanStreamWave
        | EcsSpatialScheduleSet::EvictColdPages => descriptor,
    };

    descriptor
}

#[must_use]
pub fn spatial_set_access(set: EcsSpatialScheduleSet) -> FunSystemAccess {
    match set {
        EcsSpatialScheduleSet::SenseSources => {
            FunSystemAccess::read_component_kind(FunEcsComponentKind::StreamCamera)
                .merge(FunSystemAccess::read_component_kind(
                    FunEcsComponentKind::StreamSource,
                ))
                .merge(FunSystemAccess::read_component_kind(
                    FunEcsComponentKind::SpatialVolume,
                ))
                .merge(FunSystemAccess::read_component_kind(
                    FunEcsComponentKind::DebugPin,
                ))
        }
        EcsSpatialScheduleSet::BuildInterest => {
            FunSystemAccess::read_resource_kind(FunEcsResourceKind::SpatialGridRegistry)
                .merge(FunSystemAccess::read_table_kind(
                    FunEcsResourceKind::PageResidencyTable,
                ))
                .merge(FunSystemAccess::write_table_kind(
                    FunEcsResourceKind::StreamInterestTable,
                ))
        }
        EcsSpatialScheduleSet::PlanStreamWave => {
            FunSystemAccess::read_table_kind(FunEcsResourceKind::StreamInterestTable).merge(
                FunSystemAccess::write_table_kind(FunEcsResourceKind::StreamWaveLedger),
            )
        }
        EcsSpatialScheduleSet::DiffRequests => {
            FunSystemAccess::read_table_kind(FunEcsResourceKind::StreamInterestTable)
                .merge(FunSystemAccess::read_table_kind(
                    FunEcsResourceKind::PageResidencyTable,
                ))
                .merge(FunSystemAccess::command_output(
                    FUN_COMMAND_BUFFER_SPATIAL_REQUESTS,
                ))
        }
        EcsSpatialScheduleSet::ApplyStreamCommands => {
            FunSystemAccess::write_table_kind(FunEcsResourceKind::PageResidencyTable).merge(
                FunSystemAccess::write_table_kind(FunEcsResourceKind::StreamRequestQueue),
            )
        }
        EcsSpatialScheduleSet::AcquireSources => {
            FunSystemAccess::read_table_kind(FunEcsResourceKind::StreamRequestQueue).merge(
                FunSystemAccess::write_table_kind(FunEcsResourceKind::SourceAcquireQueue),
            )
        }
        EcsSpatialScheduleSet::DecodePages => {
            FunSystemAccess::read_table_kind(FunEcsResourceKind::SourceAcquireQueue)
                .merge(FunSystemAccess::read_table_kind(
                    FunEcsResourceKind::DirtyRegionLedger,
                ))
                .merge(FunSystemAccess::write_table_kind(
                    FunEcsResourceKind::DecodedPageQueue,
                ))
        }
        EcsSpatialScheduleSet::BuildDerivedArtifacts => {
            FunSystemAccess::read_table_kind(FunEcsResourceKind::DecodedPageQueue).merge(
                FunSystemAccess::command_output(FUN_COMMAND_BUFFER_ARTIFACTS),
            )
        }
        EcsSpatialScheduleSet::PropagateDirtyRegions => {
            FunSystemAccess::write_table_kind(FunEcsResourceKind::PageResidencyTable)
                .merge(FunSystemAccess::write_table_kind(
                    FunEcsResourceKind::DirtyRegionLedger,
                ))
                .merge(FunSystemAccess::command_output(
                    FUN_COMMAND_BUFFER_DIRTY_PROPAGATION,
                ))
        }
        EcsSpatialScheduleSet::PublishRendererHandoffs => {
            FunSystemAccess::read_table_kind(FunEcsResourceKind::DerivedArtifactRegistry)
                .merge(FunSystemAccess::command_output(FUN_COMMAND_BUFFER_HANDOFFS))
        }
        EcsSpatialScheduleSet::PublishLuxHandoffs => {
            FunSystemAccess::read_table_kind(FunEcsResourceKind::DerivedArtifactRegistry)
                .merge(FunSystemAccess::command_output(FUN_COMMAND_BUFFER_HANDOFFS))
        }
        EcsSpatialScheduleSet::PublishPhysicsHandoffs => {
            FunSystemAccess::read_table_kind(FunEcsResourceKind::DerivedArtifactRegistry)
                .merge(FunSystemAccess::command_output(FUN_COMMAND_BUFFER_HANDOFFS))
        }
        EcsSpatialScheduleSet::PublishNetworkHandoffs => {
            FunSystemAccess::read_table_kind(FunEcsResourceKind::DerivedArtifactRegistry)
                .merge(FunSystemAccess::command_output(FUN_COMMAND_BUFFER_HANDOFFS))
        }
        EcsSpatialScheduleSet::EvictColdPages => {
            FunSystemAccess::read_table_kind(FunEcsResourceKind::PageResidencyTable).merge(
                FunSystemAccess::write_table_kind(FunEcsResourceKind::PageResidencyTable),
            )
        }
        EcsSpatialScheduleSet::FlushDiagnostics => {
            FunSystemAccess::read_resource_kind(FunEcsResourceKind::TelemetryEventQueue)
        }
    }
}

#[must_use]
pub const fn spatial_system_id(set: EcsSpatialScheduleSet) -> FunSystemId {
    FunSystemId::new(set.ordinal() as u32 + 1)
}

#[must_use]
pub const fn spatial_token(kind: EcsSpatialWaitTokenKind) -> WorkWaitToken {
    kind.for_chunk(EcsSpatialDomainKind::Terrain, EcsChunkKey::WHOLE_WORLD, 0)
}

pub const ECS_SPATIAL_DEFAULT_DECODE_CHUNKS: u16 = 2;
pub const ECS_SPATIAL_MAX_COMPILE_CHUNKS: u16 = 64;
pub const ECS_SPATIAL_MAX_COMPILED_CHUNKS: u16 = ECS_SPATIAL_MAX_COMPILE_CHUNKS;
pub const ECS_SPATIAL_ARTIFACT_BUILD_CONSUMER_COUNT: usize = 8;
pub const ECS_SPATIAL_ARTIFACT_BUILD_CONSUMERS: [EcsArtifactConsumer;
    ECS_SPATIAL_ARTIFACT_BUILD_CONSUMER_COUNT] = [
    EcsArtifactConsumer::Renderer,
    EcsArtifactConsumer::Lux,
    EcsArtifactConsumer::AvisPhysics,
    EcsArtifactConsumer::ThunderNetwork,
    EcsArtifactConsumer::Navigation,
    EcsArtifactConsumer::Audio,
    EcsArtifactConsumer::Telemetry,
    EcsArtifactConsumer::Editor,
];

pub type EcsSpatialProductWorkGraph = WorkGraph<EcsWork<ProductRegistry>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsSpatialScheduleBuildInput {
    pub graph_id: WorkGraphId,
    pub observed_revision: FunRevision,
    pub decode_chunk_count: u16,
}

impl Default for EcsSpatialScheduleBuildInput {
    fn default() -> Self {
        Self {
            graph_id: WorkGraphId::new(1_600),
            observed_revision: FunRevision::INITIAL,
            decode_chunk_count: ECS_SPATIAL_DEFAULT_DECODE_CHUNKS,
        }
    }
}

impl EcsSpatialScheduleBuildInput {
    #[must_use]
    pub const fn with_graph_id(mut self, graph_id: WorkGraphId) -> Self {
        self.graph_id = graph_id;
        self
    }

    #[must_use]
    pub const fn with_observed_revision(mut self, revision: FunRevision) -> Self {
        self.observed_revision = revision;
        self
    }

    #[must_use]
    pub const fn with_decode_chunk_count(mut self, chunks: u16) -> Self {
        self.decode_chunk_count = chunks;
        self
    }

    #[must_use]
    pub const fn normalized_decode_chunk_count(self) -> u16 {
        if self.decode_chunk_count == 0 {
            1
        } else if self.decode_chunk_count > ECS_SPATIAL_MAX_COMPILE_CHUNKS {
            ECS_SPATIAL_MAX_COMPILE_CHUNKS
        } else {
            self.decode_chunk_count
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsSpatialWorkNodeKind {
    ScalarSet = 0,
    ChunkSet = 1,
    Barrier = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsSpatialCompilerBarrierKind {
    ApplyRequestCommands = 0,
    ApplyArtifactCommands = 1,
    ApplyDirtyPropagationCommands = 2,
    ApplyHandoffCommands = 3,
    BeforeDiagnostics = 4,
}

impl EcsSpatialCompilerBarrierKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ApplyRequestCommands => "apply_request_commands",
            Self::ApplyArtifactCommands => "apply_artifact_commands",
            Self::ApplyDirtyPropagationCommands => "apply_dirty_propagation_commands",
            Self::ApplyHandoffCommands => "apply_handoff_commands",
            Self::BeforeDiagnostics => "before_diagnostics",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsSpatialWorkNodeSpec {
    pub node_id: WorkNodeId,
    pub set: EcsSpatialScheduleSet,
    pub kind: EcsSpatialWorkNodeKind,
    pub label: &'static str,
    pub chunk_key: EcsChunkKey,
    pub chunk_index: u16,
    pub artifact_consumer: Option<EcsArtifactConsumer>,
    pub barrier: Option<EcsSpatialCompilerBarrierKind>,
    pub work_kind: EcsWorkKind,
    pub execution: EcsSystemExecutionContract,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsSpatialAccessPlan {
    pub nodes: u32,
    pub read_rows: u32,
    pub write_rows: u32,
    pub command_buffers: u32,
    pub awaited_tokens: u32,
    pub produced_tokens: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsSpatialChunkPlan {
    pub decode_chunks: u16,
    pub artifact_consumer_chunks: u16,
    pub chunk_nodes: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsSpatialBarrierPlan {
    pub request_apply: bool,
    pub artifact_apply: bool,
    pub dirty_apply: bool,
    pub handoff_apply: bool,
    pub diagnostics_finalize: bool,
    pub barrier_nodes: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsSpatialGraphDigest {
    pub value: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsSpatialScheduleBuildReport {
    pub high_level_sets: u32,
    pub work_nodes: u32,
    pub scalar_nodes: u32,
    pub chunk_nodes: u32,
    pub barrier_nodes: u32,
    pub dependency_edges: u32,
    pub wait_edges: u32,
    pub conflict_sets: u32,
    pub liveness_proven: bool,
    pub graph_digest: EcsSpatialGraphDigest,
}

#[derive(Debug, Clone)]
pub struct EcsSpatialScheduleCompileOutput {
    pub graph: EcsSpatialProductWorkGraph,
    pub liveness_proof: LivenessProof,
    pub graph_digest: EcsSpatialGraphDigest,
    pub build_report: EcsSpatialScheduleBuildReport,
    pub node_specs: Vec<EcsSpatialWorkNodeSpec>,
    pub access_plan: EcsSpatialAccessPlan,
    pub chunk_plan: EcsSpatialChunkPlan,
    pub barrier_plan: EcsSpatialBarrierPlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EcsSpatialScheduleCompileError {
    Descriptor(FunSystemValidationError),
    Graph(GraphInvariantError),
}

impl From<FunSystemValidationError> for EcsSpatialScheduleCompileError {
    fn from(value: FunSystemValidationError) -> Self {
        Self::Descriptor(value)
    }
}

impl From<GraphInvariantError> for EcsSpatialScheduleCompileError {
    fn from(value: GraphInvariantError) -> Self {
        Self::Graph(value)
    }
}

pub struct EcsSpatialScheduleCompiler;

impl EcsSpatialScheduleCompiler {
    pub fn compile(
        input: EcsSpatialScheduleBuildInput,
    ) -> Result<EcsSpatialScheduleCompileOutput, EcsSpatialScheduleCompileError> {
        let mut graph = WorkGraph::new(
            input.graph_id,
            ScheduleDomain::FunEcs,
            GraphExecutionMode::DeterministicParallel,
            CommitPolicy::DescriptorOrder,
        )
        .with_deadline(ScheduleDeadline::Frame);

        let mut node_specs = Vec::new();
        let mut token_producers: Vec<(WorkWaitToken, WorkNodeId)> = Vec::new();

        let sense = add_set_node(
            &mut graph,
            &mut node_specs,
            &mut token_producers,
            input,
            EcsSpatialScheduleSet::SenseSources,
            EcsSpatialWorkNodeKind::ScalarSet,
            EcsChunkKey::WHOLE_WORLD,
            0,
            None,
        )?;
        let build_interest = add_set_node(
            &mut graph,
            &mut node_specs,
            &mut token_producers,
            input,
            EcsSpatialScheduleSet::BuildInterest,
            EcsSpatialWorkNodeKind::ScalarSet,
            EcsChunkKey::WHOLE_WORLD,
            0,
            None,
        )?;
        add_dependency(&mut graph, sense, build_interest);

        let plan = add_set_node(
            &mut graph,
            &mut node_specs,
            &mut token_producers,
            input,
            EcsSpatialScheduleSet::PlanStreamWave,
            EcsSpatialWorkNodeKind::ScalarSet,
            EcsChunkKey::WHOLE_WORLD,
            0,
            None,
        )?;
        add_dependency(&mut graph, build_interest, plan);

        let diff = add_set_node(
            &mut graph,
            &mut node_specs,
            &mut token_producers,
            input,
            EcsSpatialScheduleSet::DiffRequests,
            EcsSpatialWorkNodeKind::ScalarSet,
            EcsChunkKey::WHOLE_WORLD,
            0,
            None,
        )?;
        add_dependency(&mut graph, plan, diff);

        let apply_requests = add_barrier_node(
            &mut graph,
            &mut node_specs,
            input,
            EcsSpatialScheduleSet::ApplyStreamCommands,
            EcsSpatialCompilerBarrierKind::ApplyRequestCommands,
            Some(FUN_COMMAND_BUFFER_SPATIAL_REQUESTS.scheduler_id()),
            apply_request_access().to_scheduler_access_for::<ProductRegistry>(),
        );
        add_dependency(&mut graph, diff, apply_requests);

        let acquire = add_set_node(
            &mut graph,
            &mut node_specs,
            &mut token_producers,
            input,
            EcsSpatialScheduleSet::AcquireSources,
            EcsSpatialWorkNodeKind::ScalarSet,
            EcsChunkKey::WHOLE_WORLD,
            0,
            None,
        )?;
        add_dependency(&mut graph, apply_requests, acquire);

        let mut decode_nodes = Vec::new();
        for idx in 0..input.normalized_decode_chunk_count() {
            let chunk_key = decode_chunk_key(idx);
            let decode = add_set_node_with_access(
                &mut graph,
                &mut node_specs,
                &mut token_producers,
                input,
                EcsSpatialScheduleSet::DecodePages,
                EcsSpatialWorkNodeKind::ChunkSet,
                chunk_key,
                idx,
                None,
                decode_chunk_access(chunk_key),
                EcsSpatialScheduleSet::DecodePages.execution_contract(),
            )?;
            add_dependency(&mut graph, acquire, decode);
            decode_nodes.push(decode);
        }

        let mut artifact_nodes = Vec::new();
        for consumer in ECS_SPATIAL_ARTIFACT_BUILD_CONSUMERS {
            let chunk_key = artifact_consumer_chunk_key(consumer);
            let artifact = add_set_node_with_access(
                &mut graph,
                &mut node_specs,
                &mut token_producers,
                input,
                EcsSpatialScheduleSet::BuildDerivedArtifacts,
                EcsSpatialWorkNodeKind::ChunkSet,
                chunk_key,
                consumer as u16,
                Some(consumer),
                artifact_consumer_access(chunk_key),
                EcsSpatialScheduleSet::build_derived_artifact_execution(consumer),
            )?;
            for decode in &decode_nodes {
                add_dependency(&mut graph, *decode, artifact);
            }
            artifact_nodes.push(artifact);
        }

        let apply_artifacts = add_barrier_node(
            &mut graph,
            &mut node_specs,
            input,
            EcsSpatialScheduleSet::BuildDerivedArtifacts,
            EcsSpatialCompilerBarrierKind::ApplyArtifactCommands,
            Some(FUN_COMMAND_BUFFER_ARTIFACTS.scheduler_id()),
            apply_artifact_access().to_scheduler_access_for::<ProductRegistry>(),
        );
        for artifact in &artifact_nodes {
            add_dependency(&mut graph, *artifact, apply_artifacts);
        }

        let dirty = add_set_node(
            &mut graph,
            &mut node_specs,
            &mut token_producers,
            input,
            EcsSpatialScheduleSet::PropagateDirtyRegions,
            EcsSpatialWorkNodeKind::ScalarSet,
            EcsChunkKey::WHOLE_WORLD,
            0,
            None,
        )?;
        add_dependency(&mut graph, apply_artifacts, dirty);

        let apply_dirty = add_barrier_node(
            &mut graph,
            &mut node_specs,
            input,
            EcsSpatialScheduleSet::PropagateDirtyRegions,
            EcsSpatialCompilerBarrierKind::ApplyDirtyPropagationCommands,
            Some(FUN_COMMAND_BUFFER_DIRTY_PROPAGATION.scheduler_id()),
            apply_dirty_access().to_scheduler_access_for::<ProductRegistry>(),
        );
        add_dependency(&mut graph, dirty, apply_dirty);

        let renderer = add_set_node(
            &mut graph,
            &mut node_specs,
            &mut token_producers,
            input,
            EcsSpatialScheduleSet::PublishRendererHandoffs,
            EcsSpatialWorkNodeKind::ScalarSet,
            EcsChunkKey::WHOLE_WORLD,
            0,
            None,
        )?;
        add_dependency(&mut graph, apply_dirty, renderer);

        let lux = add_set_node(
            &mut graph,
            &mut node_specs,
            &mut token_producers,
            input,
            EcsSpatialScheduleSet::PublishLuxHandoffs,
            EcsSpatialWorkNodeKind::ScalarSet,
            EcsChunkKey::WHOLE_WORLD,
            0,
            None,
        )?;
        add_dependency(&mut graph, renderer, lux);

        let physics = add_set_node(
            &mut graph,
            &mut node_specs,
            &mut token_producers,
            input,
            EcsSpatialScheduleSet::PublishPhysicsHandoffs,
            EcsSpatialWorkNodeKind::ScalarSet,
            EcsChunkKey::WHOLE_WORLD,
            0,
            None,
        )?;
        add_dependency(&mut graph, lux, physics);

        let network = add_set_node(
            &mut graph,
            &mut node_specs,
            &mut token_producers,
            input,
            EcsSpatialScheduleSet::PublishNetworkHandoffs,
            EcsSpatialWorkNodeKind::ScalarSet,
            EcsChunkKey::WHOLE_WORLD,
            0,
            None,
        )?;
        add_dependency(&mut graph, physics, network);

        let apply_handoffs = add_barrier_node(
            &mut graph,
            &mut node_specs,
            input,
            EcsSpatialScheduleSet::PublishNetworkHandoffs,
            EcsSpatialCompilerBarrierKind::ApplyHandoffCommands,
            Some(FUN_COMMAND_BUFFER_HANDOFFS.scheduler_id()),
            apply_handoff_access().to_scheduler_access_for::<ProductRegistry>(),
        );
        add_dependency(&mut graph, network, apply_handoffs);

        let evict = add_set_node(
            &mut graph,
            &mut node_specs,
            &mut token_producers,
            input,
            EcsSpatialScheduleSet::EvictColdPages,
            EcsSpatialWorkNodeKind::ScalarSet,
            EcsChunkKey::WHOLE_WORLD,
            0,
            None,
        )?;
        add_dependency(&mut graph, apply_handoffs, evict);

        let before_diagnostics = add_barrier_node(
            &mut graph,
            &mut node_specs,
            input,
            EcsSpatialScheduleSet::FlushDiagnostics,
            EcsSpatialCompilerBarrierKind::BeforeDiagnostics,
            None,
            diagnostics_finalize_access().to_scheduler_access_for::<ProductRegistry>(),
        );
        add_dependency(&mut graph, evict, before_diagnostics);

        let diagnostics = add_set_node(
            &mut graph,
            &mut node_specs,
            &mut token_producers,
            input,
            EcsSpatialScheduleSet::FlushDiagnostics,
            EcsSpatialWorkNodeKind::ScalarSet,
            EcsChunkKey::WHOLE_WORLD,
            0,
            None,
        )?;
        add_dependency(&mut graph, before_diagnostics, diagnostics);

        add_token_wait_edges(&mut graph, &token_producers);
        install_write_conflict_set(&mut graph);

        let liveness_proof = graph.liveness_proof()?;
        graph.validate(false)?;
        let graph_digest = EcsSpatialGraphDigest::from_graph(&graph);
        let access_plan = EcsSpatialAccessPlan::from_graph(&graph);
        let chunk_plan = EcsSpatialChunkPlan {
            decode_chunks: input.normalized_decode_chunk_count(),
            artifact_consumer_chunks: ECS_SPATIAL_ARTIFACT_BUILD_CONSUMER_COUNT as u16,
            chunk_nodes: node_specs
                .iter()
                .filter(|spec| spec.kind == EcsSpatialWorkNodeKind::ChunkSet)
                .count() as u32,
        };
        let barrier_plan = EcsSpatialBarrierPlan::from_specs(&node_specs);
        let build_report = EcsSpatialScheduleBuildReport {
            high_level_sets: ECS_SPATIAL_SCHEDULE_SET_COUNT as u32,
            work_nodes: graph.nodes.len() as u32,
            scalar_nodes: node_specs
                .iter()
                .filter(|spec| spec.kind == EcsSpatialWorkNodeKind::ScalarSet)
                .count() as u32,
            chunk_nodes: chunk_plan.chunk_nodes,
            barrier_nodes: barrier_plan.barrier_nodes,
            dependency_edges: graph
                .edges
                .iter()
                .filter(|edge| matches!(edge, WorkEdge::Dependency { .. }))
                .count() as u32,
            wait_edges: graph.wait_for_edges.len() as u32,
            conflict_sets: graph.conflict_sets.len() as u32,
            liveness_proven: true,
            graph_digest,
        };

        Ok(EcsSpatialScheduleCompileOutput {
            graph,
            liveness_proof,
            graph_digest,
            build_report,
            node_specs,
            access_plan,
            chunk_plan,
            barrier_plan,
        })
    }
}

impl EcsSpatialGraphDigest {
    #[must_use]
    pub fn from_graph(graph: &EcsSpatialProductWorkGraph) -> Self {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for node in &graph.nodes {
            hash = spatial_hash_u64(hash, node.id.get());
            hash = spatial_hash_u8(hash, node.work_descriptor.kind as u8);
            hash = spatial_hash_u8(hash, node.work_descriptor.class as u8);
            hash = spatial_hash_u64(hash, node.work_descriptor.chunk_key.get());
            hash = spatial_hash_u8(hash, node.work_descriptor.execution.domain as u8);
            hash = spatial_hash_u8(hash, node.work_descriptor.execution.lane as u8);
            for row in &node.work_descriptor.access.accesses {
                hash = spatial_hash_u64(hash, row.target.key());
                hash = spatial_hash_u8(hash, row.mode as u8);
            }
        }
        for edge in &graph.edges {
            match *edge {
                WorkEdge::Dependency { from, to } => {
                    hash = spatial_hash_u8(hash, 1);
                    hash = spatial_hash_u64(hash, from.get());
                    hash = spatial_hash_u64(hash, to.get());
                }
                WorkEdge::Barrier { at } => {
                    hash = spatial_hash_u8(hash, 2);
                    hash = spatial_hash_u64(hash, at.get());
                }
                WorkEdge::ResourceReadAfterWrite {
                    writer,
                    reader,
                    resource_id,
                } => {
                    hash = spatial_hash_u8(hash, 3);
                    hash = spatial_hash_u64(hash, writer.get());
                    hash = spatial_hash_u64(hash, reader.get());
                    hash = spatial_hash_u64(hash, resource_id);
                }
                WorkEdge::Conflict { a, b } => {
                    hash = spatial_hash_u8(hash, 4);
                    hash = spatial_hash_u64(hash, a.get());
                    hash = spatial_hash_u64(hash, b.get());
                }
                WorkEdge::StreamFairness {
                    sibling_a,
                    sibling_b,
                } => {
                    hash = spatial_hash_u8(hash, 5);
                    hash = spatial_hash_u64(hash, sibling_a.get());
                    hash = spatial_hash_u64(hash, sibling_b.get());
                }
                WorkEdge::CancellationPropagation { parent, child } => {
                    hash = spatial_hash_u8(hash, 6);
                    hash = spatial_hash_u64(hash, parent.get());
                    hash = spatial_hash_u64(hash, child.get());
                }
                _ => {
                    hash = spatial_hash_u8(hash, 255);
                }
            }
        }
        for (set_idx, conflict_set) in graph.conflict_sets.iter().enumerate() {
            hash = spatial_hash_u8(hash, 7);
            hash = spatial_hash_u64(hash, set_idx as u64);
            for node in conflict_set {
                hash = spatial_hash_u64(hash, node.get());
            }
        }
        Self { value: hash }
    }
}

impl EcsSpatialAccessPlan {
    #[must_use]
    pub fn from_graph(graph: &EcsSpatialProductWorkGraph) -> Self {
        let mut plan = Self {
            nodes: graph.nodes.len() as u32,
            ..Self::default()
        };
        for node in &graph.nodes {
            if node.work_descriptor.command_buffer.is_some() {
                plan.command_buffers += 1;
            }
            plan.awaited_tokens += node.work_descriptor.awaited_tokens.len() as u32;
            plan.produced_tokens += node.work_descriptor.produced_tokens.len() as u32;
            for row in &node.work_descriptor.access.accesses {
                match row.mode {
                    EcsAccessMode::Read => plan.read_rows += 1,
                    EcsAccessMode::Write => plan.write_rows += 1,
                    _ => {}
                }
            }
        }
        plan
    }
}

impl EcsSpatialBarrierPlan {
    #[must_use]
    pub fn from_specs(specs: &[EcsSpatialWorkNodeSpec]) -> Self {
        let mut plan = Self::default();
        for spec in specs {
            match spec.barrier {
                Some(EcsSpatialCompilerBarrierKind::ApplyRequestCommands) => {
                    plan.request_apply = true;
                    plan.barrier_nodes += 1;
                }
                Some(EcsSpatialCompilerBarrierKind::ApplyArtifactCommands) => {
                    plan.artifact_apply = true;
                    plan.barrier_nodes += 1;
                }
                Some(EcsSpatialCompilerBarrierKind::ApplyDirtyPropagationCommands) => {
                    plan.dirty_apply = true;
                    plan.barrier_nodes += 1;
                }
                Some(EcsSpatialCompilerBarrierKind::ApplyHandoffCommands) => {
                    plan.handoff_apply = true;
                    plan.barrier_nodes += 1;
                }
                Some(EcsSpatialCompilerBarrierKind::BeforeDiagnostics) => {
                    plan.diagnostics_finalize = true;
                    plan.barrier_nodes += 1;
                }
                None => {}
            }
        }
        plan
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "compiler node construction keeps every scheduler-visible dimension explicit"
)]
fn add_set_node(
    graph: &mut EcsSpatialProductWorkGraph,
    node_specs: &mut Vec<EcsSpatialWorkNodeSpec>,
    token_producers: &mut Vec<(WorkWaitToken, WorkNodeId)>,
    input: EcsSpatialScheduleBuildInput,
    set: EcsSpatialScheduleSet,
    kind: EcsSpatialWorkNodeKind,
    chunk_key: EcsChunkKey,
    chunk_index: u16,
    artifact_consumer: Option<EcsArtifactConsumer>,
) -> Result<WorkNodeId, EcsSpatialScheduleCompileError> {
    add_set_node_with_access(
        graph,
        node_specs,
        token_producers,
        input,
        set,
        kind,
        chunk_key,
        chunk_index,
        artifact_consumer,
        set.system_descriptor().access,
        set.execution_contract(),
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "compiler node construction keeps every scheduler-visible dimension explicit"
)]
fn add_set_node_with_access(
    graph: &mut EcsSpatialProductWorkGraph,
    node_specs: &mut Vec<EcsSpatialWorkNodeSpec>,
    token_producers: &mut Vec<(WorkWaitToken, WorkNodeId)>,
    input: EcsSpatialScheduleBuildInput,
    set: EcsSpatialScheduleSet,
    kind: EcsSpatialWorkNodeKind,
    chunk_key: EcsChunkKey,
    chunk_index: u16,
    artifact_consumer: Option<EcsArtifactConsumer>,
    access: FunSystemAccess,
    execution: EcsSystemExecutionContract,
) -> Result<WorkNodeId, EcsSpatialScheduleCompileError> {
    let mut descriptor = set.system_descriptor();
    descriptor.access = access;
    descriptor.execution = execution;
    descriptor.world_revision = input.observed_revision;
    descriptor.chunk_key = chunk_key;
    if kind == EcsSpatialWorkNodeKind::ChunkSet {
        descriptor.chunk_policy = FunSystemChunkPolicy::ChunkParallel;
    }

    let scheduler = descriptor.to_scheduler_descriptor_for::<ProductRegistry>()?;
    let id = graph.next_node_id();
    let work_kind = match kind {
        EcsSpatialWorkNodeKind::ScalarSet => set.work_kind(),
        EcsSpatialWorkNodeKind::ChunkSet => EcsWorkKind::RunSystemChunk,
        EcsSpatialWorkNodeKind::Barrier => EcsWorkKind::ApplyCommands,
    };
    let deterministic_key = DeterministicDescriptor::new(
        set.label(),
        spatial_node_sort_key(set, kind, chunk_index, artifact_consumer, None),
    );
    let work = EcsWork {
        kind: work_kind,
        class: scheduler.class,
        system: Some(scheduler.id),
        access: scheduler.access,
        chunk_key,
        command_buffer: scheduler.produces_commands,
        deterministic_key,
        world_revision: EcsWorldRevision::new(input.observed_revision.get()),
        awaited_tokens: scheduler.awaited_tokens,
        produced_tokens: scheduler.produced_tokens,
        liveness_class: scheduler.liveness_class,
        execution,
        non_send: scheduler.non_send,
        main_thread_only: scheduler.main_thread_only,
    };
    let produced_tokens = work.produced_tokens.clone();
    let node = ecs_work_node(id, work);
    let work_kind = node.work_descriptor.kind;
    graph.add_node(node);
    for token in produced_tokens {
        let producer = (token, id);
        if !token_producers.contains(&producer) {
            token_producers.push(producer);
        }
    }
    node_specs.push(EcsSpatialWorkNodeSpec {
        node_id: id,
        set,
        kind,
        label: set.label(),
        chunk_key,
        chunk_index,
        artifact_consumer,
        barrier: None,
        work_kind,
        execution,
    });
    Ok(id)
}

fn add_barrier_node(
    graph: &mut EcsSpatialProductWorkGraph,
    node_specs: &mut Vec<EcsSpatialWorkNodeSpec>,
    input: EcsSpatialScheduleBuildInput,
    set: EcsSpatialScheduleSet,
    barrier: EcsSpatialCompilerBarrierKind,
    command_buffer: Option<EcsCommandBufferId>,
    access: SystemAccess<ProductRegistry>,
) -> WorkNodeId {
    let id = graph.next_node_id();
    let work_kind = barrier_work_kind(barrier);
    let execution = EcsSystemExecutionContract::COMMAND_BARRIER;
    let deterministic_key = DeterministicDescriptor::new(
        barrier.label(),
        spatial_node_sort_key(set, EcsSpatialWorkNodeKind::Barrier, 0, None, Some(barrier)),
    );
    let work = EcsWork {
        kind: work_kind,
        class: set.class(),
        system: None,
        access,
        chunk_key: EcsChunkKey::WHOLE_WORLD,
        command_buffer,
        deterministic_key,
        world_revision: EcsWorldRevision::new(input.observed_revision.get()),
        awaited_tokens: Vec::new(),
        produced_tokens: Vec::new(),
        liveness_class: EcsLivenessClass::Normal,
        execution,
        non_send: false,
        main_thread_only: false,
    };
    let node = ecs_work_node(id, work);
    graph.add_node(node);
    graph.add_edge(WorkEdge::Barrier { at: id });
    node_specs.push(EcsSpatialWorkNodeSpec {
        node_id: id,
        set,
        kind: EcsSpatialWorkNodeKind::Barrier,
        label: barrier.label(),
        chunk_key: EcsChunkKey::WHOLE_WORLD,
        chunk_index: 0,
        artifact_consumer: None,
        barrier: Some(barrier),
        work_kind,
        execution,
    });
    id
}

fn ecs_work_node(
    id: WorkNodeId,
    work: EcsWork<ProductRegistry>,
) -> WorkNode<EcsWork<ProductRegistry>> {
    let execution = work.execution;
    let deterministic_key = work.deterministic_key;
    let liveness = ecs_work_liveness(&work);
    WorkNode::new(
        id,
        execution.domain,
        execution.lane,
        work.kind.phase(),
        priority_for_deadline(execution.deadline),
        execution.budget,
        execution.deadline,
        work,
    )
    .with_deterministic_descriptor(deterministic_key)
    .with_liveness_contract(liveness)
}

fn ecs_work_liveness(work: &EcsWork<ProductRegistry>) -> WorkNodeLiveness {
    let mut liveness = WorkNodeLiveness::new()
        .with_may_wait(work.liveness_class == EcsLivenessClass::SchedulerWait)
        .with_barrier_participation(true)
        .with_blocking(match work.execution.blocking_class {
            WorkBlockingClass::NonBlocking => WorkBlockingDisposition::NonBlocking,
            WorkBlockingClass::BlockingPool => WorkBlockingDisposition::BlockingPool,
            WorkBlockingClass::ExternalBlocking => WorkBlockingDisposition::ExternalBlocking,
            _ => WorkBlockingDisposition::ExternalBlocking,
        });
    for token in &work.awaited_tokens {
        liveness = liveness.with_awaited_token(*token);
    }
    for token in &work.produced_tokens {
        liveness = liveness.with_produced_token(*token);
    }
    liveness
}

fn add_dependency(graph: &mut EcsSpatialProductWorkGraph, from: WorkNodeId, to: WorkNodeId) {
    let edge = WorkEdge::Dependency { from, to };
    if !graph.edges.contains(&edge) {
        graph.add_edge(edge);
    }
}

fn add_token_wait_edges(
    graph: &mut EcsSpatialProductWorkGraph,
    token_producers: &[(WorkWaitToken, WorkNodeId)],
) {
    let mut edges = Vec::new();
    for node in &graph.nodes {
        for awaited in &node.work_descriptor.awaited_tokens {
            for (produced, producer) in token_producers {
                if produced == awaited && *producer != node.id {
                    let edge =
                        WaitForEdge::new(WaitForEdgeKind::CrossDomainHandoff, *producer, node.id);
                    if !edges.contains(&edge) && !graph.wait_for_edges.contains(&edge) {
                        edges.push(edge);
                    }
                }
            }
        }
    }
    for edge in edges {
        graph.add_wait_for_edge(edge);
    }
}

fn install_write_conflict_set(graph: &mut EcsSpatialProductWorkGraph) {
    let writers: Vec<WorkNodeId> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.work_descriptor
                .access
                .accesses
                .iter()
                .any(|row| row.mode == EcsAccessMode::Write)
        })
        .map(|node| node.id)
        .collect();
    if writers.len() < 2 {
        return;
    }

    let conflict_set = graph.add_conflict_set(writers.clone());
    for node in &mut graph.nodes {
        if writers.contains(&node.id) {
            node.conflict_set_id = Some(conflict_set);
        }
    }
}

fn apply_request_access() -> FunSystemAccess {
    FunSystemAccess::write_table_kind(FunEcsResourceKind::PageResidencyTable)
        .merge(FunSystemAccess::write_table_kind(
            FunEcsResourceKind::StreamRequestQueue,
        ))
        .merge(FunSystemAccess::write_world_structure())
}

fn apply_artifact_access() -> FunSystemAccess {
    FunSystemAccess::write_table_kind(FunEcsResourceKind::DerivedArtifactRegistry)
}

fn apply_dirty_access() -> FunSystemAccess {
    FunSystemAccess::write_table_kind(FunEcsResourceKind::DirtyRegionLedger)
        .merge(FunSystemAccess::write_table_kind(
            FunEcsResourceKind::DerivedArtifactRegistry,
        ))
        .merge(FunSystemAccess::write_table_kind(
            FunEcsResourceKind::PageResidencyTable,
        ))
        .merge(FunSystemAccess::write_table_kind(
            FunEcsResourceKind::PhysicsCookQueue,
        ))
}

fn apply_handoff_access() -> FunSystemAccess {
    FunSystemAccess::write_table_kind(FunEcsResourceKind::RendererHandoffQueue)
        .merge(FunSystemAccess::write_table_kind(
            FunEcsResourceKind::LuxHandoffQueue,
        ))
        .merge(FunSystemAccess::write_table_kind(
            FunEcsResourceKind::PhysicsCookQueue,
        ))
        .merge(FunSystemAccess::write_table_kind(
            FunEcsResourceKind::NetworkHandoffQueue,
        ))
}

fn diagnostics_finalize_access() -> FunSystemAccess {
    FunSystemAccess::read_resource_kind(FunEcsResourceKind::TelemetryEventQueue)
        .merge(FunSystemAccess::read_table_kind(
            FunEcsResourceKind::PageResidencyTable,
        ))
        .merge(FunSystemAccess::read_table_kind(
            FunEcsResourceKind::DerivedArtifactRegistry,
        ))
        .merge(FunSystemAccess::read_table_kind(
            FunEcsResourceKind::RendererHandoffQueue,
        ))
        .merge(FunSystemAccess::read_table_kind(
            FunEcsResourceKind::LuxHandoffQueue,
        ))
        .merge(FunSystemAccess::read_table_kind(
            FunEcsResourceKind::PhysicsCookQueue,
        ))
        .merge(FunSystemAccess::read_table_kind(
            FunEcsResourceKind::NetworkHandoffQueue,
        ))
}

fn decode_chunk_access(chunk: EcsChunkKey) -> FunSystemAccess {
    read_table_chunk_kind(FunEcsResourceKind::SourceAcquireQueue, chunk)
        .merge(read_table_chunk_kind(
            FunEcsResourceKind::DirtyRegionLedger,
            chunk,
        ))
        .merge(write_table_chunk_kind(
            FunEcsResourceKind::DecodedPageQueue,
            chunk,
        ))
}

fn artifact_consumer_access(chunk: EcsChunkKey) -> FunSystemAccess {
    read_table_chunk_kind(FunEcsResourceKind::DecodedPageQueue, chunk).merge(
        FunSystemAccess::command_output(FUN_COMMAND_BUFFER_ARTIFACTS),
    )
}

fn read_table_chunk_kind(kind: FunEcsResourceKind, chunk: EcsChunkKey) -> FunSystemAccess {
    FunSystemAccess::read_table_chunk(FunResourceTableId::from_resource_kind(kind), chunk)
}

fn write_table_chunk_kind(kind: FunEcsResourceKind, chunk: EcsChunkKey) -> FunSystemAccess {
    FunSystemAccess::write_table_chunk(FunResourceTableId::from_resource_kind(kind), chunk)
}

const fn decode_chunk_key(index: u16) -> EcsChunkKey {
    EcsChunkKey::new(0x0dec_0000_u64 + index as u64 + 1)
}

const fn artifact_consumer_chunk_key(consumer: EcsArtifactConsumer) -> EcsChunkKey {
    EcsChunkKey::new(0x0a7f_0000_u64 + consumer as u64 + 1)
}

const fn barrier_work_kind(barrier: EcsSpatialCompilerBarrierKind) -> EcsWorkKind {
    match barrier {
        EcsSpatialCompilerBarrierKind::BeforeDiagnostics => EcsWorkKind::UpdateChangeTicks,
        EcsSpatialCompilerBarrierKind::ApplyRequestCommands
        | EcsSpatialCompilerBarrierKind::ApplyArtifactCommands
        | EcsSpatialCompilerBarrierKind::ApplyDirtyPropagationCommands
        | EcsSpatialCompilerBarrierKind::ApplyHandoffCommands => EcsWorkKind::ApplyCommands,
    }
}

const fn priority_for_deadline(deadline: ScheduleDeadline) -> TaskPriority {
    match deadline {
        ScheduleDeadline::Frame | ScheduleDeadline::FixedStep | ScheduleDeadline::Present => {
            TaskPriority::Critical
        }
        ScheduleDeadline::Stream | ScheduleDeadline::Request => TaskPriority::High,
        ScheduleDeadline::IdleWindow | ScheduleDeadline::None => TaskPriority::Idle,
        _ => TaskPriority::Normal,
    }
}

const fn spatial_node_sort_key(
    set: EcsSpatialScheduleSet,
    kind: EcsSpatialWorkNodeKind,
    chunk_index: u16,
    artifact_consumer: Option<EcsArtifactConsumer>,
    barrier: Option<EcsSpatialCompilerBarrierKind>,
) -> u64 {
    let consumer = match artifact_consumer {
        Some(consumer) => consumer as u64 + 1,
        None => 0,
    };
    let barrier = match barrier {
        Some(barrier) => barrier as u64 + 1,
        None => 0,
    };
    (set.ordinal() as u64)
        .saturating_mul(10_000)
        .saturating_add((kind as u64).saturating_mul(1_000))
        .saturating_add(barrier.saturating_mul(100))
        .saturating_add(consumer.saturating_mul(10))
        .saturating_add(chunk_index as u64)
}

const fn spatial_hash_u8(hash: u64, value: u8) -> u64 {
    (hash ^ value as u64).wrapping_mul(0x0000_0100_0000_01b3)
}

const fn spatial_hash_u64(hash: u64, value: u64) -> u64 {
    let mut hash = hash;
    let mut shift = 0;
    while shift < 64 {
        hash = spatial_hash_u8(hash, ((value >> shift) & 0xff) as u8);
        shift += 8;
    }
    hash
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum EcsSpatialCommandBarrierKind {
    #[default]
    ApplyRequestCommands = 0,
    ApplyArtifactCommands = 1,
    ApplyDirtyPropagationCommands = 2,
    ApplyHandoffCommands = 3,
}

impl EcsSpatialCommandBarrierKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ApplyRequestCommands => "apply_request_commands",
            Self::ApplyArtifactCommands => "apply_artifact_commands",
            Self::ApplyDirtyPropagationCommands => "apply_dirty_propagation_commands",
            Self::ApplyHandoffCommands => "apply_handoff_commands",
        }
    }

    #[must_use]
    pub const fn contract(self) -> EcsSystemExecutionContract {
        let _ = self;
        EcsSystemExecutionContract::COMMAND_BARRIER
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EcsSpatialCompiledScheduleNodeKind {
    Set(EcsSpatialScheduleSet),
    Barrier(EcsSpatialCommandBarrierKind),
}

impl EcsSpatialCompiledScheduleNodeKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Set(set) => set.label(),
            Self::Barrier(barrier) => barrier.label(),
        }
    }

    #[must_use]
    pub const fn work_kind(self) -> EcsWorkKind {
        match self {
            Self::Set(set) => set.work_kind(),
            Self::Barrier(_) => EcsWorkKind::ApplyCommands,
        }
    }

    #[must_use]
    pub const fn contract(self) -> EcsSystemExecutionContract {
        match self {
            Self::Set(set) => set.execution_contract(),
            Self::Barrier(barrier) => barrier.contract(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsSpatialCompiledScheduleNode {
    pub ordinal: u8,
    pub kind: EcsSpatialCompiledScheduleNodeKind,
    pub after_set: Option<EcsSpatialScheduleSet>,
}

impl EcsSpatialCompiledScheduleNode {
    #[must_use]
    pub const fn set(ordinal: u8, set: EcsSpatialScheduleSet) -> Self {
        Self {
            ordinal,
            kind: EcsSpatialCompiledScheduleNodeKind::Set(set),
            after_set: None,
        }
    }

    #[must_use]
    pub const fn barrier(
        ordinal: u8,
        barrier: EcsSpatialCommandBarrierKind,
        after_set: EcsSpatialScheduleSet,
    ) -> Self {
        Self {
            ordinal,
            kind: EcsSpatialCompiledScheduleNodeKind::Barrier(barrier),
            after_set: Some(after_set),
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        self.kind.label()
    }

    #[must_use]
    pub const fn work_kind(self) -> EcsWorkKind {
        self.kind.work_kind()
    }

    #[must_use]
    pub const fn execution_contract(self) -> EcsSystemExecutionContract {
        self.kind.contract()
    }
}

#[must_use]
pub const fn compile_spatial_schedule_graph() -> &'static [EcsSpatialCompiledScheduleNode] {
    &ECS_SPATIAL_COMPILED_SCHEDULE_FRAME_ORDER
}

const fn non_blocking_contract() -> EcsSystemExecutionContract {
    EcsSystemExecutionContract {
        domain: ScheduleDomain::FunEcs,
        lane: ScheduleLane::EcsSystem,
        deadline: ScheduleDeadline::Frame,
        budget: ScheduleBudget::UNBOUNDED,
        requiredness: WorkRequiredness::Required,
        blocking_class: WorkBlockingClass::NonBlocking,
        main_thread: MainThreadRequirement::AnyWorker,
        locality: WorkLocalityHint::NONE,
        cost: WorkCostHint::UNKNOWN,
        split: WorkSplitHint::Unsplittable,
    }
}

const fn frame_ecs_contract() -> EcsSystemExecutionContract {
    EcsSystemExecutionContract {
        domain: ScheduleDomain::FunEcs,
        lane: ScheduleLane::EcsSystem,
        deadline: ScheduleDeadline::Frame,
        ..non_blocking_contract()
    }
}

const fn renderer_page_scheduler_contract(
    deadline: ScheduleDeadline,
) -> EcsSystemExecutionContract {
    EcsSystemExecutionContract {
        domain: ScheduleDomain::RendererPageScheduler,
        lane: ScheduleLane::ResourceBackground,
        deadline,
        main_thread: MainThreadRequirement::NotMainThread,
        ..non_blocking_contract()
    }
}

const fn chunked_page_scheduler_contract() -> EcsSystemExecutionContract {
    EcsSystemExecutionContract {
        domain: ScheduleDomain::RendererPageScheduler,
        lane: ScheduleLane::ResourceBackground,
        deadline: ScheduleDeadline::Stream,
        main_thread: MainThreadRequirement::NotMainThread,
        split: WorkSplitHint::Splittable,
        ..non_blocking_contract()
    }
}

const fn blocking_stream_contract() -> EcsSystemExecutionContract {
    EcsSystemExecutionContract {
        domain: ScheduleDomain::Blocking,
        lane: ScheduleLane::Blocking,
        deadline: ScheduleDeadline::Stream,
        blocking_class: WorkBlockingClass::BlockingPool,
        main_thread: MainThreadRequirement::NotMainThread,
        ..non_blocking_contract()
    }
}

const fn network_handoff_contract() -> EcsSystemExecutionContract {
    EcsSystemExecutionContract {
        domain: ScheduleDomain::ThunderNetwork,
        lane: ScheduleLane::NetworkRealtime,
        deadline: ScheduleDeadline::Stream,
        ..non_blocking_contract()
    }
}

const fn idle_page_scheduler_contract() -> EcsSystemExecutionContract {
    EcsSystemExecutionContract {
        domain: ScheduleDomain::RendererPageScheduler,
        lane: ScheduleLane::IdlePrefetch,
        deadline: ScheduleDeadline::IdleWindow,
        requiredness: WorkRequiredness::Optional,
        main_thread: MainThreadRequirement::NotMainThread,
        ..non_blocking_contract()
    }
}

const fn telemetry_idle_contract() -> EcsSystemExecutionContract {
    EcsSystemExecutionContract {
        domain: ScheduleDomain::Telemetry,
        lane: ScheduleLane::TelemetryIngest,
        deadline: ScheduleDeadline::IdleWindow,
        requiredness: WorkRequiredness::Optional,
        ..non_blocking_contract()
    }
}

#[cfg(test)]
mod tests {
    use fun_scheduler_types::EcsSystemId;

    use super::*;

    #[test]
    fn spatial_system_descriptors_bridge_each_current_set_to_scheduler_contracts() {
        for set in EcsSpatialScheduleSet::all() {
            let descriptor = set.system_descriptor();
            let scheduler = descriptor
                .to_scheduler_descriptor()
                .expect("spatial descriptor bridges to scheduler");

            assert_eq!(descriptor.class, set.class());
            assert_eq!(descriptor.execution, set.execution_contract());
            assert_eq!(descriptor.work_kind(), set.work_kind());
            assert_eq!(scheduler.class, set.class());
            assert_eq!(scheduler.execution, set.execution_contract());
            assert_eq!(
                scheduler.id,
                EcsSystemId::new(spatial_system_id(*set).get())
            );
        }
    }

    #[test]
    fn spatial_access_descriptors_match_declared_inputs_and_outputs() {
        let build_interest = EcsSpatialScheduleSet::BuildInterest.system_descriptor();
        assert!(
            build_interest
                .access
                .reads_resource_kind(FunEcsResourceKind::SpatialGridRegistry)
        );
        assert!(
            build_interest
                .access
                .reads_resource_kind(FunEcsResourceKind::PageResidencyTable)
        );
        assert!(
            build_interest
                .access
                .writes_resource_kind(FunEcsResourceKind::StreamInterestTable)
        );

        let plan = EcsSpatialScheduleSet::PlanStreamWave.system_descriptor();
        assert!(
            plan.access
                .reads_resource_kind(FunEcsResourceKind::StreamInterestTable)
        );
        assert!(
            plan.access
                .writes_resource_kind(FunEcsResourceKind::StreamWaveLedger)
        );

        let diff = EcsSpatialScheduleSet::DiffRequests.system_descriptor();
        assert!(
            diff.access
                .reads_resource_kind(FunEcsResourceKind::StreamInterestTable)
        );
        assert!(
            diff.access
                .reads_resource_kind(FunEcsResourceKind::PageResidencyTable)
        );
        assert!(
            diff.access
                .has_command_output_for(FUN_COMMAND_BUFFER_SPATIAL_REQUESTS)
        );

        let acquire = EcsSpatialScheduleSet::AcquireSources.system_descriptor();
        assert!(
            acquire
                .access
                .reads_resource_kind(FunEcsResourceKind::StreamRequestQueue)
        );
        assert!(
            acquire
                .access
                .writes_resource_kind(FunEcsResourceKind::SourceAcquireQueue)
        );

        let decode = EcsSpatialScheduleSet::DecodePages.system_descriptor();
        assert!(
            decode
                .access
                .reads_resource_kind(FunEcsResourceKind::SourceAcquireQueue)
        );
        assert!(
            decode
                .access
                .writes_resource_kind(FunEcsResourceKind::DecodedPageQueue)
        );

        let artifact = EcsSpatialScheduleSet::BuildDerivedArtifacts.system_descriptor();
        assert!(
            artifact
                .access
                .reads_resource_kind(FunEcsResourceKind::DecodedPageQueue)
        );
        assert!(
            artifact
                .access
                .has_command_output_for(FUN_COMMAND_BUFFER_ARTIFACTS)
        );

        for publisher in [
            EcsSpatialScheduleSet::PublishRendererHandoffs,
            EcsSpatialScheduleSet::PublishLuxHandoffs,
            EcsSpatialScheduleSet::PublishPhysicsHandoffs,
            EcsSpatialScheduleSet::PublishNetworkHandoffs,
        ] {
            let descriptor = publisher.system_descriptor();
            assert!(
                descriptor
                    .access
                    .reads_resource_kind(FunEcsResourceKind::DerivedArtifactRegistry)
            );
            assert!(
                descriptor
                    .access
                    .has_command_output_for(FUN_COMMAND_BUFFER_HANDOFFS)
            );
        }
    }

    #[test]
    fn spatial_command_emitters_declare_command_buffers_and_liveness() {
        for set in [
            EcsSpatialScheduleSet::DiffRequests,
            EcsSpatialScheduleSet::BuildDerivedArtifacts,
            EcsSpatialScheduleSet::PropagateDirtyRegions,
            EcsSpatialScheduleSet::PublishRendererHandoffs,
            EcsSpatialScheduleSet::PublishLuxHandoffs,
            EcsSpatialScheduleSet::PublishPhysicsHandoffs,
            EcsSpatialScheduleSet::PublishNetworkHandoffs,
        ] {
            let descriptor = set.system_descriptor();
            assert!(!descriptor.command_outputs.is_empty());
            for output in &descriptor.command_outputs {
                assert!(descriptor.access.has_command_output_for(*output));
            }
        }

        for set in [
            EcsSpatialScheduleSet::DecodePages,
            EcsSpatialScheduleSet::BuildDerivedArtifacts,
            EcsSpatialScheduleSet::PublishRendererHandoffs,
            EcsSpatialScheduleSet::PublishLuxHandoffs,
            EcsSpatialScheduleSet::PublishPhysicsHandoffs,
            EcsSpatialScheduleSet::PublishNetworkHandoffs,
        ] {
            let descriptor = set.system_descriptor();
            assert_eq!(descriptor.liveness_class, EcsLivenessClass::SchedulerWait);
            assert!(!descriptor.awaited_tokens.is_empty());
        }
    }

    #[test]
    fn spatial_blocking_chunkable_and_idle_work_policies_are_declared() {
        let acquire = EcsSpatialScheduleSet::AcquireSources.system_descriptor();
        assert_eq!(acquire.execution.domain, ScheduleDomain::Blocking);
        assert_eq!(acquire.execution.lane, ScheduleLane::Blocking);
        assert_eq!(
            acquire.execution.blocking_class,
            WorkBlockingClass::BlockingPool
        );

        for set in [
            EcsSpatialScheduleSet::DecodePages,
            EcsSpatialScheduleSet::BuildDerivedArtifacts,
        ] {
            let descriptor = set.system_descriptor();
            assert_eq!(descriptor.chunk_policy, FunSystemChunkPolicy::ChunkParallel);
            assert_eq!(descriptor.work_kind(), EcsWorkKind::RunSystemChunk);
            assert_eq!(descriptor.execution.split, WorkSplitHint::Splittable);
        }

        for set in [
            EcsSpatialScheduleSet::EvictColdPages,
            EcsSpatialScheduleSet::FlushDiagnostics,
        ] {
            let descriptor = set.system_descriptor();
            assert_eq!(
                descriptor.execution.requiredness,
                WorkRequiredness::Optional
            );
            assert_eq!(descriptor.execution.deadline, ScheduleDeadline::IdleWindow);
        }

        assert_eq!(
            EcsSpatialScheduleSet::FlushDiagnostics
                .system_descriptor()
                .work_kind(),
            EcsWorkKind::ScheduleDiagnostics
        );
    }

    #[test]
    fn spatial_schedule_compiler_emits_product_ecs_work_graph() {
        let output = EcsSpatialScheduleCompiler::compile(EcsSpatialScheduleBuildInput::default())
            .expect("compile spatial product work graph");

        assert_eq!(output.graph.domain, ScheduleDomain::FunEcs);
        assert_eq!(
            output.build_report.high_level_sets,
            ECS_SPATIAL_SCHEDULE_SET_COUNT as u32
        );
        assert_eq!(
            output.chunk_plan.decode_chunks,
            ECS_SPATIAL_DEFAULT_DECODE_CHUNKS
        );
        assert_eq!(
            output.chunk_plan.artifact_consumer_chunks,
            ECS_SPATIAL_ARTIFACT_BUILD_CONSUMER_COUNT as u16
        );
        assert_eq!(output.barrier_plan.barrier_nodes, 5);
        assert!(output.barrier_plan.request_apply);
        assert!(output.barrier_plan.artifact_apply);
        assert!(output.barrier_plan.dirty_apply);
        assert!(output.barrier_plan.handoff_apply);
        assert!(output.barrier_plan.diagnostics_finalize);
        assert_eq!(
            output.build_report.work_nodes,
            output.build_report.scalar_nodes
                + output.build_report.chunk_nodes
                + output.build_report.barrier_nodes
        );
        assert!(output.build_report.liveness_proven);
        assert_ne!(output.graph_digest.value, 0);
        assert_eq!(
            output.liveness_proof.topology.order.len(),
            output.graph.nodes.len()
        );

        let mut previous = 0;
        for set in EcsSpatialScheduleSet::all() {
            let position = output
                .node_specs
                .iter()
                .position(|spec| spec.set == *set)
                .expect("compiled graph contains each public set");
            assert!(position >= previous, "set {:?} is out of order", set);
            previous = position;
        }

        assert_eq!(
            output
                .node_specs
                .iter()
                .filter(|spec| spec.set == EcsSpatialScheduleSet::DecodePages)
                .count(),
            ECS_SPATIAL_DEFAULT_DECODE_CHUNKS as usize
        );
        assert_eq!(
            output
                .node_specs
                .iter()
                .filter(|spec| spec.set == EcsSpatialScheduleSet::BuildDerivedArtifacts)
                .filter(|spec| spec.kind == EcsSpatialWorkNodeKind::ChunkSet)
                .count(),
            ECS_SPATIAL_ARTIFACT_BUILD_CONSUMER_COUNT
        );
    }

    #[test]
    fn spatial_compiler_has_no_producerless_required_reads_and_no_apply_cycles() {
        let output = EcsSpatialScheduleCompiler::compile(EcsSpatialScheduleBuildInput::default())
            .expect("compile spatial product work graph");

        output.graph.liveness_proof().expect("liveness proof holds");
        let produced: Vec<WorkWaitToken> = output
            .graph
            .nodes
            .iter()
            .flat_map(|node| node.work_descriptor.produced_tokens.iter().copied())
            .collect();
        for node in &output.graph.nodes {
            for token in &node.work_descriptor.awaited_tokens {
                assert!(
                    produced.contains(token),
                    "node {:?} awaits producerless token {:?}",
                    node.id,
                    token
                );
            }
        }

        for edge in &output.graph.edges {
            if let WorkEdge::Dependency { from, to } = *edge {
                let from_index = output
                    .liveness_proof
                    .topology
                    .order
                    .iter()
                    .position(|id| *id == from)
                    .expect("dependency source appears in topology");
                let to_index = output
                    .liveness_proof
                    .topology
                    .order
                    .iter()
                    .position(|id| *id == to)
                    .expect("dependency target appears in topology");
                assert!(
                    from_index < to_index,
                    "dependency edge creates an apply/topology cycle"
                );
            }
        }
    }

    #[test]
    fn spatial_compiler_does_not_gate_required_present_path_on_optional_work() {
        let output = EcsSpatialScheduleCompiler::compile(EcsSpatialScheduleBuildInput::default())
            .expect("compile spatial product work graph");

        assert!(
            output
                .graph
                .nodes
                .iter()
                .all(|node| node.lane != ScheduleLane::RenderPresent)
        );
        assert!(
            output
                .graph
                .wait_for_edges
                .iter()
                .all(|edge| edge.kind != WaitForEdgeKind::RendererSubmitPresent)
        );

        let renderer = output
            .graph
            .nodes
            .iter()
            .find(|node| node.work_descriptor.class == EcsSystemClass::RendererHandoff)
            .expect("renderer handoff node");
        assert!(
            renderer
                .work_descriptor
                .awaited_tokens
                .contains(&spatial_token(
                    EcsSpatialWaitTokenKind::DerivedArtifactReady
                ))
        );
        assert!(
            !renderer
                .work_descriptor
                .awaited_tokens
                .contains(&spatial_token(EcsSpatialWaitTokenKind::ShadowArtifactReady))
        );
        assert!(
            !renderer
                .work_descriptor
                .awaited_tokens
                .contains(&spatial_token(EcsSpatialWaitTokenKind::PhysicsProxyReady))
        );
        for edge in &output.graph.edges {
            if let WorkEdge::Dependency { from, to } = *edge
                && to == renderer.id
            {
                let producer = &output.graph.nodes[from.get() as usize];
                assert_ne!(producer.work_descriptor.class, EcsSystemClass::LuxHandoff);
                assert_ne!(
                    producer.work_descriptor.class,
                    EcsSystemClass::PhysicsHandoff
                );
                assert_ne!(
                    producer.work_descriptor.class,
                    EcsSystemClass::NetworkHandoff
                );
            }
        }
    }

    #[test]
    fn spatial_compiler_places_blocking_source_acquire_only_on_blocking_lane() {
        let output = EcsSpatialScheduleCompiler::compile(EcsSpatialScheduleBuildInput::default())
            .expect("compile spatial product work graph");
        let blocking_nodes: Vec<_> = output
            .graph
            .nodes
            .iter()
            .filter(|node| node.liveness.blocking.can_block())
            .collect();

        assert_eq!(blocking_nodes.len(), 1);
        let acquire = blocking_nodes[0];
        assert_eq!(acquire.work_descriptor.class, EcsSystemClass::SourceAcquire);
        assert_eq!(acquire.domain, ScheduleDomain::Blocking);
        assert_eq!(acquire.lane, ScheduleLane::Blocking);
        assert_eq!(
            acquire.work_descriptor.execution.blocking_class,
            WorkBlockingClass::BlockingPool
        );
    }

    #[test]
    fn spatial_compiler_installs_conflict_sets_for_resource_table_writes() {
        let output = EcsSpatialScheduleCompiler::compile(EcsSpatialScheduleBuildInput::default())
            .expect("compile spatial product work graph");

        assert_eq!(output.build_report.conflict_sets, 1);
        let write_conflicts = output
            .graph
            .conflict_sets
            .first()
            .expect("write conflict set is installed");
        assert!(write_conflicts.len() > 1);
        for node_id in write_conflicts {
            let node = &output.graph.nodes[node_id.get() as usize];
            assert!(node.conflict_set_id.is_some());
            assert!(
                node.work_descriptor
                    .access
                    .accesses
                    .iter()
                    .any(|row| row.mode == EcsAccessMode::Write)
            );
        }
    }
}
