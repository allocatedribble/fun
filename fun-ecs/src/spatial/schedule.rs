use fun_scheduler_types::{
    EcsSystemClass, EcsSystemExecutionContract, EcsWorkKind, MainThreadRequirement, ScheduleBudget,
    ScheduleDeadline, ScheduleDomain, ScheduleLane, WorkBlockingClass, WorkCostHint,
    WorkLocalityHint, WorkRequiredness, WorkSplitHint,
};

use crate::EcsArtifactConsumer;

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
