use fun_scheduler_types::{
    CommitPolicy, DeterministicDescriptor, EcsSpatialWaitTokenKind, EcsVirtualResourceKey,
    GraphExecutionMode, GraphInvariantError, LivenessProof, ScheduleBudget, ScheduleDeadline,
    ScheduleDomain, ScheduleLane, TaskPriority, WaitForEdge, WaitForEdgeKind, WorkEdge, WorkGraph,
    WorkGraphId, WorkNode, WorkNodeId, WorkNodeLiveness, WorkPhase, WorkRequiredness,
    WorkWaitToken,
};

use crate::{
    EcsChunkKey, EcsPageChannel, EcsSpatialDomainKind, EcsSpatialPageKey, EcsSpatialRegionKey,
};

pub const ECS_CROSS_DOMAIN_FRAME_STAGE_COUNT: usize = 27;
pub const ECS_CROSS_DOMAIN_FRAME_EDGE_COUNT: usize = 28;

pub const ECS_CROSS_DOMAIN_FRAME_FLOW: [EcsCrossDomainFrameStage;
    ECS_CROSS_DOMAIN_FRAME_STAGE_COUNT] = [
    EcsCrossDomainFrameStage::EcsSenseSources,
    EcsCrossDomainFrameStage::EcsBuildStreamInterest,
    EcsCrossDomainFrameStage::EcsDiffRequests,
    EcsCrossDomainFrameStage::EcsDecodePages,
    EcsCrossDomainFrameStage::EcsBuildArtifacts,
    EcsCrossDomainFrameStage::EcsPropagateDirty,
    EcsCrossDomainFrameStage::EcsPublishHandoffQueues,
    EcsCrossDomainFrameStage::RvelteConsumeInputState,
    EcsCrossDomainFrameStage::RvelteApplyStateResources,
    EcsCrossDomainFrameStage::RveltePropagateDirty,
    EcsCrossDomainFrameStage::RvelteLayout,
    EcsCrossDomainFrameStage::RvelteTextShape,
    EcsCrossDomainFrameStage::RveltePaintPackets,
    EcsCrossDomainFrameStage::RvelteAccessibilityPackets,
    EcsCrossDomainFrameStage::RveltePublishRendererPackets,
    EcsCrossDomainFrameStage::RendererConsumeHandoffs,
    EcsCrossDomainFrameStage::RendererConsumeRveltePackets,
    EcsCrossDomainFrameStage::RendererUploadArtifacts,
    EcsCrossDomainFrameStage::RendererPublishVirtualGeometry,
    EcsCrossDomainFrameStage::RendererUpdateLoadAnimationBuffers,
    EcsCrossDomainFrameStage::LuxConsumeHandoffs,
    EcsCrossDomainFrameStage::LuxEmitFramePlan,
    EcsCrossDomainFrameStage::LuxChooseLightingPolicy,
    EcsCrossDomainFrameStage::RendererExecuteFrameGraph,
    EcsCrossDomainFrameStage::PhysicsConsumeCookRequests,
    EcsCrossDomainFrameStage::PhysicsPublishCollisionProxies,
    EcsCrossDomainFrameStage::ThunderConsumeNetworkRows,
];

pub const ECS_CROSS_DOMAIN_FRAME_EDGES: [EcsCrossDomainFrameEdge;
    ECS_CROSS_DOMAIN_FRAME_EDGE_COUNT] = [
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::EcsSenseSources,
        EcsCrossDomainFrameStage::EcsBuildStreamInterest,
        None,
        WorkRequiredness::Required,
    ),
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::EcsBuildStreamInterest,
        EcsCrossDomainFrameStage::EcsDiffRequests,
        None,
        WorkRequiredness::Required,
    ),
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::EcsDiffRequests,
        EcsCrossDomainFrameStage::EcsDecodePages,
        None,
        WorkRequiredness::Required,
    ),
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::EcsDecodePages,
        EcsCrossDomainFrameStage::EcsBuildArtifacts,
        Some(EcsCrossDomainWaitTokenKind::VoxelPageDecoded),
        WorkRequiredness::Required,
    ),
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::EcsBuildArtifacts,
        EcsCrossDomainFrameStage::EcsPropagateDirty,
        None,
        WorkRequiredness::Required,
    ),
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::EcsPropagateDirty,
        EcsCrossDomainFrameStage::EcsPublishHandoffQueues,
        None,
        WorkRequiredness::Required,
    ),
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::RvelteConsumeInputState,
        EcsCrossDomainFrameStage::RvelteApplyStateResources,
        None,
        WorkRequiredness::Required,
    ),
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::RvelteApplyStateResources,
        EcsCrossDomainFrameStage::RveltePropagateDirty,
        None,
        WorkRequiredness::Required,
    ),
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::RveltePropagateDirty,
        EcsCrossDomainFrameStage::RvelteLayout,
        None,
        WorkRequiredness::Required,
    ),
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::RvelteLayout,
        EcsCrossDomainFrameStage::RvelteTextShape,
        Some(EcsCrossDomainWaitTokenKind::RvelteLayoutSnapshotReady),
        WorkRequiredness::Required,
    ),
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::RvelteTextShape,
        EcsCrossDomainFrameStage::RveltePaintPackets,
        None,
        WorkRequiredness::Required,
    ),
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::RvelteLayout,
        EcsCrossDomainFrameStage::RvelteAccessibilityPackets,
        Some(EcsCrossDomainWaitTokenKind::RvelteLayoutSnapshotReady),
        WorkRequiredness::Optional,
    ),
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::RveltePaintPackets,
        EcsCrossDomainFrameStage::RveltePublishRendererPackets,
        Some(EcsCrossDomainWaitTokenKind::RveltePaintPacketReady),
        WorkRequiredness::Required,
    ),
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::RveltePublishRendererPackets,
        EcsCrossDomainFrameStage::RendererConsumeRveltePackets,
        Some(EcsCrossDomainWaitTokenKind::RvelteRendererUiPacketPublished),
        WorkRequiredness::Required,
    ),
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::EcsPublishHandoffQueues,
        EcsCrossDomainFrameStage::RendererConsumeHandoffs,
        Some(EcsCrossDomainWaitTokenKind::VoxelSurfaceArtifactReady),
        WorkRequiredness::Required,
    ),
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::RendererConsumeHandoffs,
        EcsCrossDomainFrameStage::RendererUploadArtifacts,
        None,
        WorkRequiredness::Required,
    ),
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::RendererUploadArtifacts,
        EcsCrossDomainFrameStage::RendererPublishVirtualGeometry,
        None,
        WorkRequiredness::Required,
    ),
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::RendererPublishVirtualGeometry,
        EcsCrossDomainFrameStage::RendererExecuteFrameGraph,
        Some(EcsCrossDomainWaitTokenKind::VoxelRendererArtifactPublished),
        WorkRequiredness::Required,
    ),
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::RendererConsumeRveltePackets,
        EcsCrossDomainFrameStage::RendererExecuteFrameGraph,
        Some(EcsCrossDomainWaitTokenKind::RvelteRendererUiPacketPublished),
        WorkRequiredness::Required,
    ),
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::RendererUploadArtifacts,
        EcsCrossDomainFrameStage::RendererUpdateLoadAnimationBuffers,
        None,
        WorkRequiredness::Optional,
    ),
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::RendererUpdateLoadAnimationBuffers,
        EcsCrossDomainFrameStage::RendererExecuteFrameGraph,
        Some(EcsCrossDomainWaitTokenKind::VoxelLoadAnimationPublished),
        WorkRequiredness::Optional,
    ),
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::EcsPublishHandoffQueues,
        EcsCrossDomainFrameStage::LuxConsumeHandoffs,
        Some(EcsCrossDomainWaitTokenKind::VoxelLuxInvalidationPublished),
        WorkRequiredness::Optional,
    ),
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::LuxConsumeHandoffs,
        EcsCrossDomainFrameStage::LuxEmitFramePlan,
        None,
        WorkRequiredness::Optional,
    ),
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::LuxEmitFramePlan,
        EcsCrossDomainFrameStage::LuxChooseLightingPolicy,
        None,
        WorkRequiredness::Optional,
    ),
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::LuxChooseLightingPolicy,
        EcsCrossDomainFrameStage::RendererExecuteFrameGraph,
        Some(EcsCrossDomainWaitTokenKind::VoxelShadowReady),
        WorkRequiredness::Optional,
    ),
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::EcsPublishHandoffQueues,
        EcsCrossDomainFrameStage::PhysicsConsumeCookRequests,
        None,
        WorkRequiredness::Required,
    ),
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::PhysicsConsumeCookRequests,
        EcsCrossDomainFrameStage::PhysicsPublishCollisionProxies,
        Some(EcsCrossDomainWaitTokenKind::VoxelPhysicsProxyReady),
        WorkRequiredness::Required,
    ),
    EcsCrossDomainFrameEdge::new(
        EcsCrossDomainFrameStage::EcsPublishHandoffQueues,
        EcsCrossDomainFrameStage::ThunderConsumeNetworkRows,
        Some(EcsCrossDomainWaitTokenKind::VoxelNetworkDeltaPublished),
        WorkRequiredness::Required,
    ),
];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsCrossDomainFrameOwner {
    #[default]
    FunEcs = 0,
    FunRenderer = 1,
    FunLux = 2,
    AvisPhysics = 3,
    Thunder = 4,
    Rvelte = 5,
}

impl EcsCrossDomainFrameOwner {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::FunEcs => "fun_ecs",
            Self::FunRenderer => "fun_renderer",
            Self::FunLux => "fun_lux",
            Self::AvisPhysics => "avis_physics",
            Self::Thunder => "thunder",
            Self::Rvelte => "rvelte",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsCrossDomainFrameStage {
    #[default]
    EcsSenseSources = 0,
    EcsBuildStreamInterest = 1,
    EcsDiffRequests = 2,
    EcsDecodePages = 3,
    EcsBuildArtifacts = 4,
    EcsPropagateDirty = 5,
    EcsPublishHandoffQueues = 6,
    RendererConsumeHandoffs = 7,
    RendererUploadArtifacts = 8,
    RendererPublishVirtualGeometry = 9,
    RendererUpdateLoadAnimationBuffers = 10,
    RendererExecuteFrameGraph = 11,
    LuxConsumeHandoffs = 12,
    LuxEmitFramePlan = 13,
    LuxChooseLightingPolicy = 14,
    PhysicsConsumeCookRequests = 15,
    PhysicsPublishCollisionProxies = 16,
    ThunderConsumeNetworkRows = 17,
    RvelteConsumeInputState = 18,
    RvelteApplyStateResources = 19,
    RveltePropagateDirty = 20,
    RvelteLayout = 21,
    RvelteTextShape = 22,
    RveltePaintPackets = 23,
    RvelteAccessibilityPackets = 24,
    RveltePublishRendererPackets = 25,
    RendererConsumeRveltePackets = 26,
}

impl EcsCrossDomainFrameStage {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::EcsSenseSources => "ecs_sense_sources",
            Self::EcsBuildStreamInterest => "ecs_build_stream_interest",
            Self::EcsDiffRequests => "ecs_diff_requests",
            Self::EcsDecodePages => "ecs_decode_pages",
            Self::EcsBuildArtifacts => "ecs_build_artifacts",
            Self::EcsPropagateDirty => "ecs_propagate_dirty",
            Self::EcsPublishHandoffQueues => "ecs_publish_handoff_queues",
            Self::RendererConsumeHandoffs => "renderer_consume_handoffs",
            Self::RendererUploadArtifacts => "renderer_upload_artifacts",
            Self::RendererPublishVirtualGeometry => "renderer_publish_virtual_geometry",
            Self::RendererUpdateLoadAnimationBuffers => "renderer_update_load_animation_buffers",
            Self::RendererExecuteFrameGraph => "renderer_execute_frame_graph",
            Self::LuxConsumeHandoffs => "lux_consume_handoffs",
            Self::LuxEmitFramePlan => "lux_emit_frame_plan",
            Self::LuxChooseLightingPolicy => "lux_choose_lighting_policy",
            Self::PhysicsConsumeCookRequests => "physics_consume_cook_requests",
            Self::PhysicsPublishCollisionProxies => "physics_publish_collision_proxies",
            Self::ThunderConsumeNetworkRows => "thunder_consume_network_rows",
            Self::RvelteConsumeInputState => "rvelte_consume_input_state",
            Self::RvelteApplyStateResources => "rvelte_apply_state_resources",
            Self::RveltePropagateDirty => "rvelte_propagate_dirty",
            Self::RvelteLayout => "rvelte_layout",
            Self::RvelteTextShape => "rvelte_text_shape",
            Self::RveltePaintPackets => "rvelte_paint_packets",
            Self::RvelteAccessibilityPackets => "rvelte_accessibility_packets",
            Self::RveltePublishRendererPackets => "rvelte_publish_renderer_packets",
            Self::RendererConsumeRveltePackets => "renderer_consume_rvelte_packets",
        }
    }

    #[must_use]
    pub const fn all() -> &'static [Self] {
        &ECS_CROSS_DOMAIN_FRAME_FLOW
    }

    #[must_use]
    pub const fn owner(self) -> EcsCrossDomainFrameOwner {
        match self {
            Self::EcsSenseSources
            | Self::EcsBuildStreamInterest
            | Self::EcsDiffRequests
            | Self::EcsDecodePages
            | Self::EcsBuildArtifacts
            | Self::EcsPropagateDirty
            | Self::EcsPublishHandoffQueues => EcsCrossDomainFrameOwner::FunEcs,
            Self::RendererConsumeHandoffs
            | Self::RendererUploadArtifacts
            | Self::RendererPublishVirtualGeometry
            | Self::RendererUpdateLoadAnimationBuffers
            | Self::RendererExecuteFrameGraph => EcsCrossDomainFrameOwner::FunRenderer,
            Self::LuxConsumeHandoffs | Self::LuxEmitFramePlan | Self::LuxChooseLightingPolicy => {
                EcsCrossDomainFrameOwner::FunLux
            }
            Self::PhysicsConsumeCookRequests | Self::PhysicsPublishCollisionProxies => {
                EcsCrossDomainFrameOwner::AvisPhysics
            }
            Self::ThunderConsumeNetworkRows => EcsCrossDomainFrameOwner::Thunder,
            Self::RvelteConsumeInputState
            | Self::RvelteApplyStateResources
            | Self::RveltePropagateDirty
            | Self::RvelteLayout
            | Self::RvelteTextShape
            | Self::RveltePaintPackets
            | Self::RvelteAccessibilityPackets
            | Self::RveltePublishRendererPackets => EcsCrossDomainFrameOwner::Rvelte,
            Self::RendererConsumeRveltePackets => EcsCrossDomainFrameOwner::FunRenderer,
        }
    }

    #[must_use]
    pub const fn domain(self) -> ScheduleDomain {
        match self {
            Self::EcsSenseSources
            | Self::EcsBuildStreamInterest
            | Self::EcsPropagateDirty
            | Self::EcsPublishHandoffQueues => ScheduleDomain::FunEcs,
            Self::EcsDiffRequests | Self::EcsDecodePages | Self::EcsBuildArtifacts => {
                ScheduleDomain::RendererPageScheduler
            }
            Self::RendererConsumeHandoffs
            | Self::RendererUploadArtifacts
            | Self::RendererPublishVirtualGeometry
            | Self::RendererUpdateLoadAnimationBuffers
            | Self::RendererExecuteFrameGraph => ScheduleDomain::Renderer,
            Self::LuxConsumeHandoffs | Self::LuxEmitFramePlan | Self::LuxChooseLightingPolicy => {
                ScheduleDomain::RendererLux
            }
            Self::PhysicsConsumeCookRequests | Self::PhysicsPublishCollisionProxies => {
                ScheduleDomain::AvisPhysics
            }
            Self::ThunderConsumeNetworkRows => ScheduleDomain::ThunderNetwork,
            Self::RvelteConsumeInputState
            | Self::RvelteApplyStateResources
            | Self::RveltePropagateDirty
            | Self::RvelteLayout
            | Self::RvelteTextShape
            | Self::RveltePaintPackets
            | Self::RvelteAccessibilityPackets
            | Self::RveltePublishRendererPackets => ScheduleDomain::RvelteUi,
            Self::RendererConsumeRveltePackets => ScheduleDomain::Renderer,
        }
    }

    #[must_use]
    pub const fn lane(self) -> ScheduleLane {
        match self {
            Self::EcsSenseSources
            | Self::EcsBuildStreamInterest
            | Self::EcsDiffRequests
            | Self::EcsDecodePages
            | Self::EcsBuildArtifacts
            | Self::EcsPropagateDirty
            | Self::EcsPublishHandoffQueues => ScheduleLane::EcsSystem,
            Self::RendererConsumeHandoffs
            | Self::RendererConsumeRveltePackets
            | Self::RendererUploadArtifacts
            | Self::RendererPublishVirtualGeometry
            | Self::RendererUpdateLoadAnimationBuffers => ScheduleLane::RenderPrepare,
            Self::RendererExecuteFrameGraph => ScheduleLane::RenderGraphCompile,
            Self::LuxConsumeHandoffs | Self::LuxEmitFramePlan | Self::LuxChooseLightingPolicy => {
                ScheduleLane::RenderGraphCompile
            }
            Self::PhysicsConsumeCookRequests => ScheduleLane::PhysicsSolve,
            Self::PhysicsPublishCollisionProxies => ScheduleLane::PhysicsFixedStep,
            Self::ThunderConsumeNetworkRows => ScheduleLane::NetworkRealtime,
            Self::RvelteConsumeInputState => ScheduleLane::UiSyncInput,
            Self::RvelteApplyStateResources
            | Self::RveltePropagateDirty
            | Self::RveltePaintPackets
            | Self::RvelteAccessibilityPackets
            | Self::RveltePublishRendererPackets => ScheduleLane::UiAnimationFrame,
            Self::RvelteLayout | Self::RvelteTextShape => ScheduleLane::UiDeferredLayout,
        }
    }

    #[must_use]
    pub const fn is_rvelte_retained_mutation(self) -> bool {
        matches!(
            self,
            Self::RvelteApplyStateResources | Self::RveltePropagateDirty
        )
    }

    #[must_use]
    pub const fn is_renderer_present_stage(self) -> bool {
        matches!(self, Self::RendererExecuteFrameGraph)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsCrossDomainFrameEdge {
    pub producer: EcsCrossDomainFrameStage,
    pub consumer: EcsCrossDomainFrameStage,
    pub token: Option<EcsCrossDomainWaitTokenKind>,
    pub requiredness: WorkRequiredness,
}

impl EcsCrossDomainFrameEdge {
    #[must_use]
    pub const fn new(
        producer: EcsCrossDomainFrameStage,
        consumer: EcsCrossDomainFrameStage,
        token: Option<EcsCrossDomainWaitTokenKind>,
        requiredness: WorkRequiredness,
    ) -> Self {
        Self {
            producer,
            consumer,
            token,
            requiredness,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsCrossDomainWaitTokenKind {
    #[default]
    VoxelPageDecoded = 0,
    VoxelSurfaceArtifactReady = 1,
    VoxelRendererArtifactPublished = 2,
    VoxelLuxInvalidationPublished = 3,
    VoxelShadowReady = 4,
    VoxelPhysicsProxyReady = 5,
    VoxelNetworkDeltaPublished = 6,
    VoxelLoadAnimationPublished = 7,
    RvelteLayoutSnapshotReady = 8,
    RveltePaintPacketReady = 9,
    RvelteAccessibilityPacketReady = 10,
    RvelteRendererUiPacketPublished = 11,
}

impl EcsCrossDomainWaitTokenKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::VoxelPageDecoded => "voxel_page_decoded",
            Self::VoxelSurfaceArtifactReady => "voxel_surface_artifact_ready",
            Self::VoxelRendererArtifactPublished => "voxel_renderer_artifact_published",
            Self::VoxelLuxInvalidationPublished => "voxel_lux_invalidation_published",
            Self::VoxelShadowReady => "voxel_shadow_ready",
            Self::VoxelPhysicsProxyReady => "voxel_physics_proxy_ready",
            Self::VoxelNetworkDeltaPublished => "voxel_network_delta_published",
            Self::VoxelLoadAnimationPublished => "voxel_load_animation_published",
            Self::RvelteLayoutSnapshotReady => "rvelte_layout_snapshot_ready",
            Self::RveltePaintPacketReady => "rvelte_paint_packet_ready",
            Self::RvelteAccessibilityPacketReady => "rvelte_accessibility_packet_ready",
            Self::RvelteRendererUiPacketPublished => "rvelte_renderer_ui_packet_published",
        }
    }

    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::VoxelPageDecoded,
            Self::VoxelSurfaceArtifactReady,
            Self::VoxelRendererArtifactPublished,
            Self::VoxelLuxInvalidationPublished,
            Self::VoxelShadowReady,
            Self::VoxelPhysicsProxyReady,
            Self::VoxelNetworkDeltaPublished,
            Self::VoxelLoadAnimationPublished,
            Self::RvelteLayoutSnapshotReady,
            Self::RveltePaintPacketReady,
            Self::RvelteAccessibilityPacketReady,
            Self::RvelteRendererUiPacketPublished,
        ]
    }

    #[must_use]
    pub const fn scheduler_kind(self) -> EcsSpatialWaitTokenKind {
        match self {
            Self::VoxelPageDecoded => EcsSpatialWaitTokenKind::PageDecoded,
            Self::VoxelSurfaceArtifactReady | Self::VoxelLuxInvalidationPublished => {
                EcsSpatialWaitTokenKind::DerivedArtifactReady
            }
            Self::VoxelRendererArtifactPublished => {
                EcsSpatialWaitTokenKind::RendererArtifactPublished
            }
            Self::VoxelShadowReady => EcsSpatialWaitTokenKind::ShadowArtifactReady,
            Self::VoxelPhysicsProxyReady => EcsSpatialWaitTokenKind::PhysicsProxyReady,
            Self::VoxelNetworkDeltaPublished => EcsSpatialWaitTokenKind::NetworkDeltaPublished,
            Self::VoxelLoadAnimationPublished => EcsSpatialWaitTokenKind::LoadAnimationPublished,
            Self::RvelteLayoutSnapshotReady
            | Self::RveltePaintPacketReady
            | Self::RvelteAccessibilityPacketReady
            | Self::RvelteRendererUiPacketPublished => {
                EcsSpatialWaitTokenKind::LoadAnimationPublished
            }
        }
    }

    #[must_use]
    pub const fn producer_stage(self) -> EcsCrossDomainFrameStage {
        match self {
            Self::VoxelPageDecoded => EcsCrossDomainFrameStage::EcsDecodePages,
            Self::VoxelSurfaceArtifactReady => EcsCrossDomainFrameStage::EcsBuildArtifacts,
            Self::VoxelRendererArtifactPublished => {
                EcsCrossDomainFrameStage::RendererPublishVirtualGeometry
            }
            Self::VoxelLuxInvalidationPublished => {
                EcsCrossDomainFrameStage::EcsPublishHandoffQueues
            }
            Self::VoxelShadowReady => EcsCrossDomainFrameStage::RendererExecuteFrameGraph,
            Self::VoxelPhysicsProxyReady => {
                EcsCrossDomainFrameStage::PhysicsPublishCollisionProxies
            }
            Self::VoxelNetworkDeltaPublished => EcsCrossDomainFrameStage::EcsPublishHandoffQueues,
            Self::VoxelLoadAnimationPublished => {
                EcsCrossDomainFrameStage::RendererUpdateLoadAnimationBuffers
            }
            Self::RvelteLayoutSnapshotReady => EcsCrossDomainFrameStage::RvelteLayout,
            Self::RveltePaintPacketReady => EcsCrossDomainFrameStage::RveltePaintPackets,
            Self::RvelteAccessibilityPacketReady => {
                EcsCrossDomainFrameStage::RvelteAccessibilityPackets
            }
            Self::RvelteRendererUiPacketPublished => {
                EcsCrossDomainFrameStage::RveltePublishRendererPackets
            }
        }
    }

    #[must_use]
    pub const fn is_rvelte(self) -> bool {
        matches!(
            self,
            Self::RvelteLayoutSnapshotReady
                | Self::RveltePaintPacketReady
                | Self::RvelteAccessibilityPacketReady
                | Self::RvelteRendererUiPacketPublished
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EcsCrossDomainWaitToken {
    VoxelPageDecoded(EcsSpatialPageKey),
    VoxelSurfaceArtifactReady(EcsSpatialPageKey),
    VoxelRendererArtifactPublished(EcsSpatialPageKey),
    VoxelLuxInvalidationPublished(EcsSpatialPageKey),
    VoxelShadowReady(EcsSpatialPageKey),
    VoxelPhysicsProxyReady(EcsSpatialPageKey),
    VoxelNetworkDeltaPublished(EcsSpatialRegionKey),
    VoxelLoadAnimationPublished(EcsSpatialPageKey),
    RvelteLayoutSnapshotReady(u32),
    RveltePaintPacketReady(u32),
    RvelteAccessibilityPacketReady(u32),
    RvelteRendererUiPacketPublished(u32),
}

impl EcsCrossDomainWaitToken {
    #[must_use]
    pub const fn kind(self) -> EcsCrossDomainWaitTokenKind {
        match self {
            Self::VoxelPageDecoded(_) => EcsCrossDomainWaitTokenKind::VoxelPageDecoded,
            Self::VoxelSurfaceArtifactReady(_) => {
                EcsCrossDomainWaitTokenKind::VoxelSurfaceArtifactReady
            }
            Self::VoxelRendererArtifactPublished(_) => {
                EcsCrossDomainWaitTokenKind::VoxelRendererArtifactPublished
            }
            Self::VoxelLuxInvalidationPublished(_) => {
                EcsCrossDomainWaitTokenKind::VoxelLuxInvalidationPublished
            }
            Self::VoxelShadowReady(_) => EcsCrossDomainWaitTokenKind::VoxelShadowReady,
            Self::VoxelPhysicsProxyReady(_) => EcsCrossDomainWaitTokenKind::VoxelPhysicsProxyReady,
            Self::VoxelNetworkDeltaPublished(_) => {
                EcsCrossDomainWaitTokenKind::VoxelNetworkDeltaPublished
            }
            Self::VoxelLoadAnimationPublished(_) => {
                EcsCrossDomainWaitTokenKind::VoxelLoadAnimationPublished
            }
            Self::RvelteLayoutSnapshotReady(_) => {
                EcsCrossDomainWaitTokenKind::RvelteLayoutSnapshotReady
            }
            Self::RveltePaintPacketReady(_) => EcsCrossDomainWaitTokenKind::RveltePaintPacketReady,
            Self::RvelteAccessibilityPacketReady(_) => {
                EcsCrossDomainWaitTokenKind::RvelteAccessibilityPacketReady
            }
            Self::RvelteRendererUiPacketPublished(_) => {
                EcsCrossDomainWaitTokenKind::RvelteRendererUiPacketPublished
            }
        }
    }

    #[must_use]
    pub fn virtual_resource_key(self, generation: u32) -> EcsVirtualResourceKey {
        match self {
            Self::VoxelPageDecoded(page)
            | Self::VoxelSurfaceArtifactReady(page)
            | Self::VoxelRendererArtifactPublished(page)
            | Self::VoxelLuxInvalidationPublished(page)
            | Self::VoxelShadowReady(page)
            | Self::VoxelPhysicsProxyReady(page)
            | Self::VoxelLoadAnimationPublished(page) => page.virtual_resource_key(generation),
            Self::VoxelNetworkDeltaPublished(region) => EcsVirtualResourceKey::new(
                region.domain,
                region.grid_id.get() as u16,
                region.level,
                EcsPageChannel::NetworkRelevance as u16,
                region.chunk_key(),
                generation,
            ),
            Self::RvelteLayoutSnapshotReady(packet_generation) => rvelte_virtual_resource_key(
                EcsCrossDomainWaitTokenKind::RvelteLayoutSnapshotReady,
                packet_generation,
                generation,
            ),
            Self::RveltePaintPacketReady(packet_generation) => rvelte_virtual_resource_key(
                EcsCrossDomainWaitTokenKind::RveltePaintPacketReady,
                packet_generation,
                generation,
            ),
            Self::RvelteAccessibilityPacketReady(packet_generation) => rvelte_virtual_resource_key(
                EcsCrossDomainWaitTokenKind::RvelteAccessibilityPacketReady,
                packet_generation,
                generation,
            ),
            Self::RvelteRendererUiPacketPublished(packet_generation) => {
                rvelte_virtual_resource_key(
                    EcsCrossDomainWaitTokenKind::RvelteRendererUiPacketPublished,
                    packet_generation,
                    generation,
                )
            }
        }
    }

    #[must_use]
    pub fn scheduler_token(self, generation: u32) -> WorkWaitToken {
        self.kind()
            .scheduler_kind()
            .to_wait_token(self.virtual_resource_key(generation))
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsCrossDomainWaitClass {
    #[default]
    RequiredRenderArtifact = 0,
    OptionalFoliage = 1,
    OptionalFineOverlay = 2,
    DiagnosticOverlay = 3,
    FarSdf = 4,
    FarGi = 5,
    NonCriticalPhysicsCook = 6,
    CollisionCriticalTerrainProxy = 7,
    LuxRefinement = 8,
    RendererPresent = 9,
    NetworkDelta = 10,
    RvelteRetainedMutation = 11,
    RequiredHudPaintPacket = 12,
    OptionalUiDiagnostics = 13,
    RvelteOptionalPrewarm = 14,
}

impl EcsCrossDomainWaitClass {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::RequiredRenderArtifact => "required_render_artifact",
            Self::OptionalFoliage => "optional_foliage",
            Self::OptionalFineOverlay => "optional_fine_overlay",
            Self::DiagnosticOverlay => "diagnostic_overlay",
            Self::FarSdf => "far_sdf",
            Self::FarGi => "far_gi",
            Self::NonCriticalPhysicsCook => "noncritical_physics_cook",
            Self::CollisionCriticalTerrainProxy => "collision_critical_terrain_proxy",
            Self::LuxRefinement => "lux_refinement",
            Self::RendererPresent => "renderer_present",
            Self::NetworkDelta => "network_delta",
            Self::RvelteRetainedMutation => "rvelte_retained_mutation",
            Self::RequiredHudPaintPacket => "required_hud_paint_packet",
            Self::OptionalUiDiagnostics => "optional_ui_diagnostics",
            Self::RvelteOptionalPrewarm => "rvelte_optional_prewarm",
        }
    }

    #[must_use]
    pub const fn can_gate_present(self) -> bool {
        matches!(
            self,
            Self::RequiredRenderArtifact | Self::RequiredHudPaintPacket
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsCrossDomainWaitPolicy {
    pub token: EcsCrossDomainWaitTokenKind,
    pub class: EcsCrossDomainWaitClass,
    pub requiredness: WorkRequiredness,
    pub has_valid_fallback: bool,
    pub bounded_deadline: bool,
}

impl EcsCrossDomainWaitPolicy {
    #[must_use]
    pub const fn new(
        token: EcsCrossDomainWaitTokenKind,
        class: EcsCrossDomainWaitClass,
        requiredness: WorkRequiredness,
        has_valid_fallback: bool,
    ) -> Self {
        Self {
            token,
            class,
            requiredness,
            has_valid_fallback,
            bounded_deadline: false,
        }
    }

    #[must_use]
    pub const fn with_bounded_deadline(mut self) -> Self {
        self.bounded_deadline = true;
        self
    }

    #[must_use]
    pub const fn renderer_present_decision(self) -> EcsCrossDomainWaitDecision {
        if !self.class.can_gate_present() {
            return EcsCrossDomainWaitDecision::Reject(
                EcsCrossDomainWaitRejectReason::NonRenderPresentDependency,
            );
        }
        if matches!(self.requiredness, WorkRequiredness::Optional) {
            return EcsCrossDomainWaitDecision::Reject(
                EcsCrossDomainWaitRejectReason::OptionalPresentDependency,
            );
        }
        if matches!(self.class, EcsCrossDomainWaitClass::RequiredHudPaintPacket)
            && !self.has_valid_fallback
            && !self.bounded_deadline
        {
            return EcsCrossDomainWaitDecision::Reject(
                EcsCrossDomainWaitRejectReason::RequiredHudPaintMissingFallbackOrDeadline,
            );
        }
        if self.has_valid_fallback {
            return EcsCrossDomainWaitDecision::UseFallback(
                EcsCrossDomainFallbackKind::RenderArtifactFallback,
            );
        }
        EcsCrossDomainWaitDecision::Wait
    }

    #[must_use]
    pub const fn physics_fixed_step_decision(self) -> EcsCrossDomainWaitDecision {
        if matches!(
            self.class,
            EcsCrossDomainWaitClass::CollisionCriticalTerrainProxy
        ) {
            if self.has_valid_fallback {
                return EcsCrossDomainWaitDecision::UseFallback(
                    EcsCrossDomainFallbackKind::ConservativePhysicsProxy,
                );
            }
            return EcsCrossDomainWaitDecision::Wait;
        }
        EcsCrossDomainWaitDecision::UseFallback(
            EcsCrossDomainFallbackKind::ConservativePhysicsProxy,
        )
    }

    #[must_use]
    pub const fn lux_wait_decision(self) -> EcsCrossDomainWaitDecision {
        if matches!(self.class, EcsCrossDomainWaitClass::RendererPresent) {
            return EcsCrossDomainWaitDecision::Reject(
                EcsCrossDomainWaitRejectReason::LuxWaitsOnRendererPresent,
            );
        }
        EcsCrossDomainWaitDecision::Wait
    }

    #[must_use]
    pub const fn renderer_lux_refinement_decision(self) -> EcsCrossDomainWaitDecision {
        if matches!(self.class, EcsCrossDomainWaitClass::LuxRefinement)
            && matches!(self.requiredness, WorkRequiredness::Optional)
        {
            return EcsCrossDomainWaitDecision::Reject(
                EcsCrossDomainWaitRejectReason::RendererWaitsOnOptionalLuxRefinement,
            );
        }
        EcsCrossDomainWaitDecision::Wait
    }

    #[must_use]
    pub const fn rvelte_wait_decision(self) -> EcsCrossDomainWaitDecision {
        if matches!(self.class, EcsCrossDomainWaitClass::RendererPresent) {
            return EcsCrossDomainWaitDecision::Reject(
                EcsCrossDomainWaitRejectReason::RvelteWaitsOnRendererPresent,
            );
        }
        if matches!(self.class, EcsCrossDomainWaitClass::RvelteOptionalPrewarm) {
            return EcsCrossDomainWaitDecision::Reject(
                EcsCrossDomainWaitRejectReason::InputStateWaitsOnOptionalPrewarm,
            );
        }
        EcsCrossDomainWaitDecision::Wait
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsCrossDomainFallbackKind {
    RenderArtifactFallback = 0,
    ConservativePhysicsProxy = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsCrossDomainWaitRejectReason {
    OptionalPresentDependency = 0,
    NonRenderPresentDependency = 1,
    LuxWaitsOnRendererPresent = 2,
    RendererWaitsOnOptionalLuxRefinement = 3,
    RvelteWaitsOnRendererPresent = 4,
    OptionalUiDiagnosticsGatePresent = 5,
    RequiredHudPaintMissingFallbackOrDeadline = 6,
    InputStateWaitsOnOptionalPrewarm = 7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EcsCrossDomainWaitDecision {
    Wait,
    UseFallback(EcsCrossDomainFallbackKind),
    Reject(EcsCrossDomainWaitRejectReason),
}

pub type EcsCrossDomainFrameWorkGraph = WorkGraph<EcsFrameStageNode>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EcsFrameBuildInput {
    pub graph_id: WorkGraphId,
    pub frame_revision: u64,
    pub queue_generation: u32,
    pub fixed_step_generation: u32,
    pub pages: Vec<EcsSpatialPageKey>,
    pub regions: Vec<EcsSpatialRegionKey>,
    pub extra_edges: Vec<EcsCrossDomainFrameEdge>,
    pub use_conservative_physics_fallback: bool,
    pub required_hud_has_fallback: bool,
    pub required_hud_bounded_deadline: bool,
}

impl Default for EcsFrameBuildInput {
    fn default() -> Self {
        Self {
            graph_id: WorkGraphId::new(2_100),
            frame_revision: 1,
            queue_generation: 1,
            fixed_step_generation: 1,
            pages: Vec::new(),
            regions: Vec::new(),
            extra_edges: Vec::new(),
            use_conservative_physics_fallback: false,
            required_hud_has_fallback: false,
            required_hud_bounded_deadline: true,
        }
    }
}

impl EcsFrameBuildInput {
    #[must_use]
    pub fn with_graph_id(mut self, graph_id: WorkGraphId) -> Self {
        self.graph_id = graph_id;
        self
    }

    #[must_use]
    pub fn with_frame_revision(mut self, frame_revision: u64) -> Self {
        self.frame_revision = frame_revision;
        self
    }

    #[must_use]
    pub fn with_queue_generation(mut self, queue_generation: u32) -> Self {
        self.queue_generation = queue_generation;
        self
    }

    #[must_use]
    pub fn with_fixed_step_generation(mut self, fixed_step_generation: u32) -> Self {
        self.fixed_step_generation = fixed_step_generation;
        self
    }

    #[must_use]
    pub fn with_page(mut self, page: EcsSpatialPageKey) -> Self {
        self.pages.push(page);
        self
    }

    #[must_use]
    pub fn with_region(mut self, region: EcsSpatialRegionKey) -> Self {
        self.regions.push(region);
        self
    }

    #[must_use]
    pub fn with_extra_edge(mut self, edge: EcsCrossDomainFrameEdge) -> Self {
        self.extra_edges.push(edge);
        self
    }

    #[must_use]
    pub fn with_conservative_physics_fallback(mut self, enabled: bool) -> Self {
        self.use_conservative_physics_fallback = enabled;
        self
    }

    #[must_use]
    pub fn with_required_hud_fallback(mut self, enabled: bool) -> Self {
        self.required_hud_has_fallback = enabled;
        self
    }

    #[must_use]
    pub fn with_required_hud_bounded_deadline(mut self, enabled: bool) -> Self {
        self.required_hud_bounded_deadline = enabled;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EcsFrameStageNode {
    pub stage: EcsCrossDomainFrameStage,
    pub owner: EcsCrossDomainFrameOwner,
    pub domain: ScheduleDomain,
    pub lane: ScheduleLane,
    pub frame_revision: u64,
    pub stage_generation: u32,
    pub awaited_tokens: Vec<WorkWaitToken>,
    pub produced_tokens: Vec<WorkWaitToken>,
    pub virtual_reads: Vec<EcsVirtualResourceKey>,
    pub virtual_writes: Vec<EcsVirtualResourceKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsFrameEdgePlan {
    pub producer: EcsCrossDomainFrameStage,
    pub consumer: EcsCrossDomainFrameStage,
    pub token: Option<EcsCrossDomainWaitTokenKind>,
    pub requiredness: WorkRequiredness,
    pub dependency_edge: bool,
    pub wait_for_edge: bool,
    pub virtual_resource_edge: bool,
    pub fallback_edge: bool,
    pub cancellation_edge: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EcsFrameWaitResourceScope {
    Page(EcsSpatialPageKey),
    Region(EcsSpatialRegionKey),
    QueueGeneration {
        owner: EcsCrossDomainFrameOwner,
        generation: u32,
    },
    FixedStep {
        generation: u32,
    },
    FrameRevision {
        revision: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsFrameWaitPlan {
    pub producer: EcsCrossDomainFrameStage,
    pub consumer: EcsCrossDomainFrameStage,
    pub token_kind: EcsCrossDomainWaitTokenKind,
    pub scheduler_token: WorkWaitToken,
    pub virtual_resource: EcsVirtualResourceKey,
    pub scope: EcsFrameWaitResourceScope,
    pub requiredness: WorkRequiredness,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsFrameFallbackPlan {
    pub producer: EcsCrossDomainFrameStage,
    pub consumer: EcsCrossDomainFrameStage,
    pub fallback: EcsCrossDomainFallbackKind,
    pub reason: EcsCrossDomainWaitRejectReason,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsFrameLivenessReport {
    pub stage_nodes: u32,
    pub dependency_edges: u32,
    pub wait_for_edges: u32,
    pub virtual_resource_edges: u32,
    pub fallback_edges: u32,
    pub cancellation_edges: u32,
    pub wait_tokens: u32,
    pub liveness_proven: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsFrameGraphDigest {
    pub value: u64,
}

#[derive(Debug, Clone)]
pub struct EcsCrossDomainFrameCompileOutput {
    pub graph: EcsCrossDomainFrameWorkGraph,
    pub liveness_proof: LivenessProof,
    pub stage_nodes: Vec<EcsFrameStageNode>,
    pub edge_plan: Vec<EcsFrameEdgePlan>,
    pub wait_plan: Vec<EcsFrameWaitPlan>,
    pub fallback_plan: Vec<EcsFrameFallbackPlan>,
    pub liveness_report: EcsFrameLivenessReport,
    pub graph_digest: EcsFrameGraphDigest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EcsFrameCompileError {
    Graph(GraphInvariantError),
    Policy {
        producer: EcsCrossDomainFrameStage,
        consumer: EcsCrossDomainFrameStage,
        reason: EcsCrossDomainWaitRejectReason,
    },
}

impl From<GraphInvariantError> for EcsFrameCompileError {
    fn from(value: GraphInvariantError) -> Self {
        Self::Graph(value)
    }
}

pub struct EcsCrossDomainFrameCompiler;

impl EcsCrossDomainFrameCompiler {
    pub fn compile(
        input: EcsFrameBuildInput,
    ) -> Result<EcsCrossDomainFrameCompileOutput, EcsFrameCompileError> {
        let mut graph = WorkGraph::new(
            input.graph_id,
            ScheduleDomain::FunEcs,
            GraphExecutionMode::DeterministicParallel,
            CommitPolicy::DescriptorOrder,
        )
        .with_deadline(ScheduleDeadline::Frame);

        for stage in EcsCrossDomainFrameStage::all() {
            let id = graph.next_node_id();
            graph.add_node(frame_work_node(id, frame_stage_node(*stage, &input)));
        }

        let mut edge_plan = Vec::new();
        let mut wait_plan = Vec::new();
        let mut fallback_plan = Vec::new();
        let mut virtual_resource_edges = 0_u32;
        let mut cancellation_edges = 0_u32;

        for edge in ECS_CROSS_DOMAIN_FRAME_EDGES
            .iter()
            .copied()
            .chain(input.extra_edges.iter().copied())
        {
            let producer = node_id_for_stage(&graph, edge.producer);
            let consumer = node_id_for_stage(&graph, edge.consumer);
            let policy = frame_edge_policy(edge, &input)?;
            let dependency_edge = policy.dependency_edge;
            let wait_tokens = frame_wait_tokens_for_edge(edge, &input);

            if dependency_edge {
                add_work_edge_once(
                    &mut graph,
                    WorkEdge::Dependency {
                        from: producer,
                        to: consumer,
                    },
                );
            }

            let mut wait_for_edge = false;
            let mut virtual_resource_edge = false;
            if dependency_edge && !wait_tokens.is_empty() {
                add_wait_for_edge_once(
                    &mut graph,
                    WaitForEdge::new(WaitForEdgeKind::CrossDomainHandoff, producer, consumer),
                );
                wait_for_edge = true;
                add_work_edge_once(
                    &mut graph,
                    WorkEdge::ResourceReadAfterWrite {
                        writer: producer,
                        reader: consumer,
                        resource_id: wait_tokens[0].virtual_resource.key(),
                    },
                );
                virtual_resource_edge = true;
                virtual_resource_edges += 1;
                for wait in &wait_tokens {
                    attach_produced_token(&mut graph, producer, *wait);
                    attach_awaited_token(&mut graph, consumer, *wait);
                    wait_plan.push(*wait);
                }
            }

            let mut cancellation_edge = false;
            if dependency_edge && edge.requiredness == WorkRequiredness::Optional {
                add_work_edge_once(
                    &mut graph,
                    WorkEdge::CancellationPropagation {
                        parent: producer,
                        child: consumer,
                    },
                );
                cancellation_edge = true;
                cancellation_edges += 1;
            }

            if let Some(fallback) = policy.fallback {
                fallback_plan.push(fallback);
            }

            edge_plan.push(EcsFrameEdgePlan {
                producer: edge.producer,
                consumer: edge.consumer,
                token: edge.token,
                requiredness: edge.requiredness,
                dependency_edge,
                wait_for_edge,
                virtual_resource_edge,
                fallback_edge: policy.fallback.is_some(),
                cancellation_edge,
            });
        }

        let liveness_proof = graph.liveness_proof()?;
        graph.validate(false)?;
        let graph_digest = EcsFrameGraphDigest::from_graph(&graph, &wait_plan, &fallback_plan);
        let liveness_report = EcsFrameLivenessReport {
            stage_nodes: graph.nodes.len() as u32,
            dependency_edges: graph
                .edges
                .iter()
                .filter(|edge| matches!(edge, WorkEdge::Dependency { .. }))
                .count() as u32,
            wait_for_edges: graph.wait_for_edges.len() as u32,
            virtual_resource_edges,
            fallback_edges: fallback_plan.len() as u32,
            cancellation_edges,
            wait_tokens: wait_plan.len() as u32,
            liveness_proven: true,
        };
        let stage_nodes = graph
            .nodes
            .iter()
            .map(|node| node.work_descriptor.clone())
            .collect();

        Ok(EcsCrossDomainFrameCompileOutput {
            graph,
            liveness_proof,
            stage_nodes,
            edge_plan,
            wait_plan,
            fallback_plan,
            liveness_report,
            graph_digest,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FrameEdgePolicy {
    dependency_edge: bool,
    fallback: Option<EcsFrameFallbackPlan>,
}

impl EcsFrameGraphDigest {
    #[must_use]
    pub fn from_graph(
        graph: &EcsCrossDomainFrameWorkGraph,
        wait_plan: &[EcsFrameWaitPlan],
        fallback_plan: &[EcsFrameFallbackPlan],
    ) -> Self {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for node in &graph.nodes {
            hash = frame_hash_u64(hash, node.id.get());
            hash = frame_hash_u8(hash, node.work_descriptor.stage as u8);
            hash = frame_hash_u8(hash, node.work_descriptor.domain as u8);
            hash = frame_hash_u8(hash, node.work_descriptor.lane as u8);
            hash = frame_hash_u64(hash, node.work_descriptor.frame_revision);
            hash = frame_hash_u64(hash, node.work_descriptor.stage_generation as u64);
        }
        for edge in &graph.edges {
            hash = frame_hash_u8(hash, edge.label().as_bytes()[0]);
            match *edge {
                WorkEdge::Dependency { from, to } => {
                    hash = frame_hash_u64(hash, from.get());
                    hash = frame_hash_u64(hash, to.get());
                }
                WorkEdge::ResourceReadAfterWrite {
                    writer,
                    reader,
                    resource_id,
                } => {
                    hash = frame_hash_u64(hash, writer.get());
                    hash = frame_hash_u64(hash, reader.get());
                    hash = frame_hash_u64(hash, resource_id);
                }
                WorkEdge::CancellationPropagation { parent, child } => {
                    hash = frame_hash_u64(hash, parent.get());
                    hash = frame_hash_u64(hash, child.get());
                }
                WorkEdge::Barrier { at } => {
                    hash = frame_hash_u64(hash, at.get());
                }
                WorkEdge::Conflict { a, b } => {
                    hash = frame_hash_u64(hash, a.get());
                    hash = frame_hash_u64(hash, b.get());
                }
                WorkEdge::StreamFairness {
                    sibling_a,
                    sibling_b,
                } => {
                    hash = frame_hash_u64(hash, sibling_a.get());
                    hash = frame_hash_u64(hash, sibling_b.get());
                }
                _ => {}
            }
        }
        for wait in wait_plan {
            hash = frame_hash_u8(hash, wait.token_kind as u8);
            hash = frame_hash_u64(hash, wait.scheduler_token.get());
            hash = frame_hash_u64(hash, wait.virtual_resource.key());
        }
        for fallback in fallback_plan {
            hash = frame_hash_u8(hash, fallback.fallback as u8);
            hash = frame_hash_u8(hash, fallback.reason as u8);
            hash = frame_hash_u8(hash, fallback.consumer as u8);
        }
        Self { value: hash }
    }
}

fn frame_stage_node(
    stage: EcsCrossDomainFrameStage,
    input: &EcsFrameBuildInput,
) -> EcsFrameStageNode {
    EcsFrameStageNode {
        stage,
        owner: stage.owner(),
        domain: stage.domain(),
        lane: stage.lane(),
        frame_revision: input.frame_revision,
        stage_generation: stage_generation(stage, input),
        awaited_tokens: Vec::new(),
        produced_tokens: Vec::new(),
        virtual_reads: Vec::new(),
        virtual_writes: Vec::new(),
    }
}

fn frame_work_node(id: WorkNodeId, work: EcsFrameStageNode) -> WorkNode<EcsFrameStageNode> {
    let stage = work.stage;
    WorkNode::new(
        id,
        work.domain,
        work.lane,
        frame_stage_phase(stage),
        priority_for_stage(stage),
        ScheduleBudget::UNBOUNDED,
        deadline_for_stage(stage),
        work,
    )
    .with_deterministic_descriptor(DeterministicDescriptor::new(stage.label(), id.get()))
    .with_liveness_contract(WorkNodeLiveness::new().with_barrier_participation(true))
}

fn frame_edge_policy(
    edge: EcsCrossDomainFrameEdge,
    input: &EcsFrameBuildInput,
) -> Result<FrameEdgePolicy, EcsFrameCompileError> {
    if edge.producer.is_renderer_present_stage()
        && edge.consumer.owner() == EcsCrossDomainFrameOwner::FunLux
    {
        return Err(policy_error(
            edge,
            EcsCrossDomainWaitRejectReason::LuxWaitsOnRendererPresent,
        ));
    }
    if edge.producer.is_renderer_present_stage() && edge.consumer.is_rvelte_retained_mutation() {
        return Err(policy_error(
            edge,
            EcsCrossDomainWaitRejectReason::RvelteWaitsOnRendererPresent,
        ));
    }

    let class = wait_class_for_edge(edge);
    if edge.consumer.is_renderer_present_stage() {
        if edge.requiredness == WorkRequiredness::Optional {
            return Ok(FrameEdgePolicy {
                dependency_edge: false,
                fallback: Some(EcsFrameFallbackPlan {
                    producer: edge.producer,
                    consumer: edge.consumer,
                    fallback: EcsCrossDomainFallbackKind::RenderArtifactFallback,
                    reason: if class == EcsCrossDomainWaitClass::OptionalUiDiagnostics {
                        EcsCrossDomainWaitRejectReason::OptionalUiDiagnosticsGatePresent
                    } else {
                        EcsCrossDomainWaitRejectReason::OptionalPresentDependency
                    },
                }),
            });
        }
        if class == EcsCrossDomainWaitClass::RequiredHudPaintPacket
            && !input.required_hud_has_fallback
            && !input.required_hud_bounded_deadline
        {
            return Err(policy_error(
                edge,
                EcsCrossDomainWaitRejectReason::RequiredHudPaintMissingFallbackOrDeadline,
            ));
        }
    }

    if edge.token == Some(EcsCrossDomainWaitTokenKind::VoxelPhysicsProxyReady)
        && input.use_conservative_physics_fallback
    {
        return Ok(FrameEdgePolicy {
            dependency_edge: false,
            fallback: Some(EcsFrameFallbackPlan {
                producer: edge.producer,
                consumer: edge.consumer,
                fallback: EcsCrossDomainFallbackKind::ConservativePhysicsProxy,
                reason: EcsCrossDomainWaitRejectReason::OptionalPresentDependency,
            }),
        });
    }

    Ok(FrameEdgePolicy {
        dependency_edge: true,
        fallback: None,
    })
}

fn wait_class_for_edge(edge: EcsCrossDomainFrameEdge) -> EcsCrossDomainWaitClass {
    match edge.token {
        Some(EcsCrossDomainWaitTokenKind::VoxelRendererArtifactPublished)
        | Some(EcsCrossDomainWaitTokenKind::VoxelSurfaceArtifactReady) => {
            EcsCrossDomainWaitClass::RequiredRenderArtifact
        }
        Some(EcsCrossDomainWaitTokenKind::VoxelShadowReady) => {
            EcsCrossDomainWaitClass::LuxRefinement
        }
        Some(EcsCrossDomainWaitTokenKind::VoxelPhysicsProxyReady) => {
            EcsCrossDomainWaitClass::CollisionCriticalTerrainProxy
        }
        Some(EcsCrossDomainWaitTokenKind::VoxelNetworkDeltaPublished) => {
            EcsCrossDomainWaitClass::NetworkDelta
        }
        Some(EcsCrossDomainWaitTokenKind::RvelteRendererUiPacketPublished)
        | Some(EcsCrossDomainWaitTokenKind::RveltePaintPacketReady) => {
            EcsCrossDomainWaitClass::RequiredHudPaintPacket
        }
        Some(EcsCrossDomainWaitTokenKind::RvelteAccessibilityPacketReady) => {
            EcsCrossDomainWaitClass::OptionalUiDiagnostics
        }
        Some(EcsCrossDomainWaitTokenKind::RvelteLayoutSnapshotReady) => {
            EcsCrossDomainWaitClass::RvelteRetainedMutation
        }
        Some(EcsCrossDomainWaitTokenKind::VoxelLoadAnimationPublished) => {
            EcsCrossDomainWaitClass::OptionalFineOverlay
        }
        Some(EcsCrossDomainWaitTokenKind::VoxelPageDecoded)
        | Some(EcsCrossDomainWaitTokenKind::VoxelLuxInvalidationPublished)
        | None => EcsCrossDomainWaitClass::RequiredRenderArtifact,
    }
}

fn frame_wait_tokens_for_edge(
    edge: EcsCrossDomainFrameEdge,
    input: &EcsFrameBuildInput,
) -> Vec<EcsFrameWaitPlan> {
    let Some(token_kind) = edge.token else {
        return Vec::new();
    };
    let mut scopes = Vec::new();
    match token_kind {
        EcsCrossDomainWaitTokenKind::VoxelNetworkDeltaPublished => {
            for region in &input.regions {
                scopes.push(EcsFrameWaitResourceScope::Region(*region));
            }
            scopes.push(EcsFrameWaitResourceScope::QueueGeneration {
                owner: EcsCrossDomainFrameOwner::Thunder,
                generation: input.queue_generation,
            });
        }
        EcsCrossDomainWaitTokenKind::RvelteLayoutSnapshotReady
        | EcsCrossDomainWaitTokenKind::RveltePaintPacketReady
        | EcsCrossDomainWaitTokenKind::RvelteAccessibilityPacketReady
        | EcsCrossDomainWaitTokenKind::RvelteRendererUiPacketPublished => {
            scopes.push(EcsFrameWaitResourceScope::QueueGeneration {
                owner: EcsCrossDomainFrameOwner::Rvelte,
                generation: input.queue_generation,
            });
        }
        EcsCrossDomainWaitTokenKind::VoxelPhysicsProxyReady => {
            for page in &input.pages {
                scopes.push(EcsFrameWaitResourceScope::Page(*page));
            }
            scopes.push(EcsFrameWaitResourceScope::FixedStep {
                generation: input.fixed_step_generation,
            });
        }
        EcsCrossDomainWaitTokenKind::VoxelPageDecoded
        | EcsCrossDomainWaitTokenKind::VoxelSurfaceArtifactReady
        | EcsCrossDomainWaitTokenKind::VoxelRendererArtifactPublished
        | EcsCrossDomainWaitTokenKind::VoxelLuxInvalidationPublished
        | EcsCrossDomainWaitTokenKind::VoxelShadowReady
        | EcsCrossDomainWaitTokenKind::VoxelLoadAnimationPublished => {
            for page in &input.pages {
                scopes.push(EcsFrameWaitResourceScope::Page(*page));
            }
        }
    }
    scopes.push(EcsFrameWaitResourceScope::FrameRevision {
        revision: input.frame_revision,
    });

    let mut out = Vec::new();
    for scope in scopes {
        let virtual_resource = frame_virtual_resource_key(token_kind, scope, input);
        out.push(EcsFrameWaitPlan {
            producer: edge.producer,
            consumer: edge.consumer,
            token_kind,
            scheduler_token: token_kind.scheduler_kind().to_wait_token(virtual_resource),
            virtual_resource,
            scope,
            requiredness: edge.requiredness,
        });
    }
    out
}

fn frame_virtual_resource_key(
    token_kind: EcsCrossDomainWaitTokenKind,
    scope: EcsFrameWaitResourceScope,
    input: &EcsFrameBuildInput,
) -> EcsVirtualResourceKey {
    match scope {
        EcsFrameWaitResourceScope::Page(page) => page.virtual_resource_key(input.queue_generation),
        EcsFrameWaitResourceScope::Region(region) => EcsVirtualResourceKey::new(
            region.domain,
            region.grid_id.get() as u16,
            region.level,
            EcsPageChannel::NetworkRelevance as u16,
            region.chunk_key(),
            input.queue_generation,
        ),
        EcsFrameWaitResourceScope::QueueGeneration { owner, generation } => {
            frame_queue_virtual_resource_key(owner, token_kind, generation)
        }
        EcsFrameWaitResourceScope::FixedStep { generation } => EcsVirtualResourceKey::new(
            EcsSpatialDomainKind::CollisionSdf,
            0,
            0,
            token_kind as u16,
            EcsChunkKey::new(u64::from(generation)),
            generation,
        ),
        EcsFrameWaitResourceScope::FrameRevision { revision } => EcsVirtualResourceKey::new(
            EcsSpatialDomainKind::Debug,
            0,
            0,
            token_kind as u16,
            EcsChunkKey::new(revision),
            input.queue_generation,
        ),
    }
}

const fn frame_queue_virtual_resource_key(
    owner: EcsCrossDomainFrameOwner,
    token_kind: EcsCrossDomainWaitTokenKind,
    generation: u32,
) -> EcsVirtualResourceKey {
    EcsVirtualResourceKey::new(
        EcsSpatialDomainKind::Debug,
        owner as u16,
        0,
        token_kind as u16,
        EcsChunkKey::new(generation as u64),
        generation,
    )
}

const fn rvelte_virtual_resource_key(
    token_kind: EcsCrossDomainWaitTokenKind,
    packet_generation: u32,
    generation: u32,
) -> EcsVirtualResourceKey {
    EcsVirtualResourceKey::new(
        EcsSpatialDomainKind::Debug,
        EcsCrossDomainFrameOwner::Rvelte as u16,
        0,
        token_kind as u16,
        EcsChunkKey::new(packet_generation as u64),
        generation,
    )
}

fn attach_produced_token(
    graph: &mut EcsCrossDomainFrameWorkGraph,
    node_id: WorkNodeId,
    wait: EcsFrameWaitPlan,
) {
    let node = &mut graph.nodes[node_id.get() as usize];
    push_unique_token(
        &mut node.work_descriptor.produced_tokens,
        wait.scheduler_token,
    );
    push_unique_token(&mut node.liveness.produced_tokens, wait.scheduler_token);
    push_unique_key(
        &mut node.work_descriptor.virtual_writes,
        wait.virtual_resource,
    );
}

fn attach_awaited_token(
    graph: &mut EcsCrossDomainFrameWorkGraph,
    node_id: WorkNodeId,
    wait: EcsFrameWaitPlan,
) {
    let node = &mut graph.nodes[node_id.get() as usize];
    push_unique_token(
        &mut node.work_descriptor.awaited_tokens,
        wait.scheduler_token,
    );
    push_unique_token(&mut node.liveness.awaited_tokens, wait.scheduler_token);
    push_unique_key(
        &mut node.work_descriptor.virtual_reads,
        wait.virtual_resource,
    );
    node.liveness.may_wait = true;
}

fn add_work_edge_once(graph: &mut EcsCrossDomainFrameWorkGraph, edge: WorkEdge) {
    if !graph.edges.contains(&edge) {
        graph.add_edge(edge);
    }
}

fn add_wait_for_edge_once(graph: &mut EcsCrossDomainFrameWorkGraph, edge: WaitForEdge) {
    if !graph.wait_for_edges.contains(&edge) {
        graph.add_wait_for_edge(edge);
    }
}

fn push_unique_token(tokens: &mut Vec<WorkWaitToken>, token: WorkWaitToken) {
    if !tokens.contains(&token) {
        tokens.push(token);
    }
}

fn push_unique_key(keys: &mut Vec<EcsVirtualResourceKey>, key: EcsVirtualResourceKey) {
    if !keys.contains(&key) {
        keys.push(key);
    }
}

fn node_id_for_stage(
    graph: &EcsCrossDomainFrameWorkGraph,
    stage: EcsCrossDomainFrameStage,
) -> WorkNodeId {
    graph
        .nodes
        .iter()
        .find(|node| node.work_descriptor.stage == stage)
        .map(|node| node.id)
        .expect("frame compiler emits every declared stage")
}

fn policy_error(
    edge: EcsCrossDomainFrameEdge,
    reason: EcsCrossDomainWaitRejectReason,
) -> EcsFrameCompileError {
    EcsFrameCompileError::Policy {
        producer: edge.producer,
        consumer: edge.consumer,
        reason,
    }
}

const fn stage_generation(stage: EcsCrossDomainFrameStage, input: &EcsFrameBuildInput) -> u32 {
    if stage.is_rvelte_retained_mutation() {
        input.queue_generation.saturating_add(1)
    } else if matches!(
        stage,
        EcsCrossDomainFrameStage::RendererConsumeRveltePackets
            | EcsCrossDomainFrameStage::RendererExecuteFrameGraph
    ) {
        input.queue_generation
    } else if matches!(
        stage,
        EcsCrossDomainFrameStage::PhysicsConsumeCookRequests
            | EcsCrossDomainFrameStage::PhysicsPublishCollisionProxies
    ) {
        input.fixed_step_generation
    } else {
        input.frame_revision as u32
    }
}

const fn frame_stage_phase(stage: EcsCrossDomainFrameStage) -> WorkPhase {
    match stage {
        EcsCrossDomainFrameStage::RendererExecuteFrameGraph
        | EcsCrossDomainFrameStage::PhysicsPublishCollisionProxies
        | EcsCrossDomainFrameStage::ThunderConsumeNetworkRows => WorkPhase::Commit,
        EcsCrossDomainFrameStage::RendererConsumeHandoffs
        | EcsCrossDomainFrameStage::RendererConsumeRveltePackets
        | EcsCrossDomainFrameStage::LuxConsumeHandoffs
        | EcsCrossDomainFrameStage::PhysicsConsumeCookRequests => WorkPhase::Setup,
        _ => WorkPhase::Compute,
    }
}

const fn priority_for_stage(stage: EcsCrossDomainFrameStage) -> TaskPriority {
    match stage {
        EcsCrossDomainFrameStage::RendererExecuteFrameGraph
        | EcsCrossDomainFrameStage::PhysicsPublishCollisionProxies
        | EcsCrossDomainFrameStage::RvelteConsumeInputState
        | EcsCrossDomainFrameStage::RendererConsumeRveltePackets => TaskPriority::Critical,
        EcsCrossDomainFrameStage::ThunderConsumeNetworkRows => TaskPriority::High,
        EcsCrossDomainFrameStage::RendererUpdateLoadAnimationBuffers
        | EcsCrossDomainFrameStage::RvelteAccessibilityPackets => TaskPriority::Idle,
        _ => TaskPriority::Normal,
    }
}

const fn deadline_for_stage(stage: EcsCrossDomainFrameStage) -> ScheduleDeadline {
    match stage {
        EcsCrossDomainFrameStage::RendererExecuteFrameGraph
        | EcsCrossDomainFrameStage::RvelteConsumeInputState
        | EcsCrossDomainFrameStage::RendererConsumeRveltePackets => ScheduleDeadline::Frame,
        EcsCrossDomainFrameStage::PhysicsPublishCollisionProxies => ScheduleDeadline::FixedStep,
        EcsCrossDomainFrameStage::RendererUpdateLoadAnimationBuffers
        | EcsCrossDomainFrameStage::RvelteAccessibilityPackets => ScheduleDeadline::IdleWindow,
        _ => ScheduleDeadline::Stream,
    }
}

const fn frame_hash_u8(hash: u64, value: u8) -> u64 {
    (hash ^ value as u64).wrapping_mul(0x0000_0100_0000_01b3)
}

const fn frame_hash_u64(hash: u64, value: u64) -> u64 {
    let mut hash = hash;
    let mut shift = 0;
    while shift < 64 {
        hash = frame_hash_u8(hash, ((value >> shift) & 0xff) as u8);
        shift += 8;
    }
    hash
}

#[cfg(test)]
mod tests {
    use crate::{EcsSpatialDomainKind, EcsSpatialGridId, EcsSpatialPageKey, EcsSpatialRegionKey};

    use super::*;

    fn terrain_page(channel: EcsPageChannel) -> EcsSpatialPageKey {
        EcsSpatialPageKey::new(
            EcsSpatialDomainKind::Terrain,
            EcsSpatialGridId::new(1),
            0,
            4,
            2,
            1,
            channel,
        )
    }

    // Doctrine gates, not unit trivia: cross-domain waits must stay typed,
    // renderer present only waits on required render artifacts without fallback,
    // physics fixed step may use conservative fallback, and optional Lux
    // refinement cannot gate renderer or present.
    #[test]
    fn cross_domain_frame_flow_declares_requested_owner_sequence() {
        assert_eq!(
            EcsCrossDomainFrameStage::all().len(),
            ECS_CROSS_DOMAIN_FRAME_STAGE_COUNT
        );
        assert_eq!(
            EcsCrossDomainFrameStage::all()[0],
            EcsCrossDomainFrameStage::EcsSenseSources
        );
        assert_eq!(
            EcsCrossDomainFrameStage::all()[6],
            EcsCrossDomainFrameStage::EcsPublishHandoffQueues
        );
        assert_eq!(
            EcsCrossDomainFrameStage::RendererExecuteFrameGraph.owner(),
            EcsCrossDomainFrameOwner::FunRenderer
        );
        assert_eq!(
            EcsCrossDomainFrameStage::LuxEmitFramePlan.owner(),
            EcsCrossDomainFrameOwner::FunLux
        );
        assert_eq!(
            EcsCrossDomainFrameStage::PhysicsPublishCollisionProxies.owner(),
            EcsCrossDomainFrameOwner::AvisPhysics
        );
        assert_eq!(
            EcsCrossDomainFrameStage::ThunderConsumeNetworkRows.owner(),
            EcsCrossDomainFrameOwner::Thunder
        );
        assert_eq!(
            EcsCrossDomainFrameStage::RendererExecuteFrameGraph.domain(),
            ScheduleDomain::Renderer
        );
        assert_eq!(
            EcsCrossDomainFrameStage::ThunderConsumeNetworkRows.lane(),
            ScheduleLane::NetworkRealtime
        );
    }

    #[test]
    fn cross_domain_wait_tokens_cover_requested_edges_and_scheduler_tokens() {
        let labels: Vec<&'static str> = EcsCrossDomainWaitTokenKind::all()
            .iter()
            .map(|token| token.label())
            .collect();
        assert_eq!(
            labels,
            vec![
                "voxel_page_decoded",
                "voxel_surface_artifact_ready",
                "voxel_renderer_artifact_published",
                "voxel_lux_invalidation_published",
                "voxel_shadow_ready",
                "voxel_physics_proxy_ready",
                "voxel_network_delta_published",
                "voxel_load_animation_published",
                "rvelte_layout_snapshot_ready",
                "rvelte_paint_packet_ready",
                "rvelte_accessibility_packet_ready",
                "rvelte_renderer_ui_packet_published",
            ]
        );
        assert_eq!(
            EcsCrossDomainWaitTokenKind::VoxelPageDecoded.scheduler_kind(),
            EcsSpatialWaitTokenKind::PageDecoded
        );
        assert_eq!(
            EcsCrossDomainWaitTokenKind::VoxelLuxInvalidationPublished.scheduler_kind(),
            EcsSpatialWaitTokenKind::DerivedArtifactReady
        );
        assert_eq!(
            EcsCrossDomainWaitTokenKind::VoxelShadowReady.scheduler_kind(),
            EcsSpatialWaitTokenKind::ShadowArtifactReady
        );
        assert_eq!(
            EcsCrossDomainWaitTokenKind::VoxelNetworkDeltaPublished.producer_stage(),
            EcsCrossDomainFrameStage::EcsPublishHandoffQueues
        );

        let page_token = EcsCrossDomainWaitToken::VoxelRendererArtifactPublished(terrain_page(
            EcsPageChannel::Surface,
        ))
        .scheduler_token(7);
        let region = EcsSpatialRegionKey::new(
            EcsSpatialDomainKind::Terrain,
            EcsSpatialGridId::new(1),
            0,
            1,
            0,
            0,
        );
        let region_token =
            EcsCrossDomainWaitToken::VoxelNetworkDeltaPublished(region).scheduler_token(7);
        assert_ne!(page_token.get(), 0);
        assert_ne!(region_token.get(), 0);
        assert_ne!(page_token, region_token);
    }

    #[test]
    fn renderer_present_waits_only_on_required_render_artifacts_without_fallback() {
        let required = EcsCrossDomainWaitPolicy::new(
            EcsCrossDomainWaitTokenKind::VoxelRendererArtifactPublished,
            EcsCrossDomainWaitClass::RequiredRenderArtifact,
            WorkRequiredness::Required,
            false,
        );
        assert_eq!(
            required.renderer_present_decision(),
            EcsCrossDomainWaitDecision::Wait
        );

        let with_fallback = EcsCrossDomainWaitPolicy {
            has_valid_fallback: true,
            ..required
        };
        assert_eq!(
            with_fallback.renderer_present_decision(),
            EcsCrossDomainWaitDecision::UseFallback(
                EcsCrossDomainFallbackKind::RenderArtifactFallback
            )
        );

        for class in [
            EcsCrossDomainWaitClass::OptionalFoliage,
            EcsCrossDomainWaitClass::OptionalFineOverlay,
            EcsCrossDomainWaitClass::DiagnosticOverlay,
            EcsCrossDomainWaitClass::FarSdf,
            EcsCrossDomainWaitClass::FarGi,
            EcsCrossDomainWaitClass::NonCriticalPhysicsCook,
        ] {
            let policy = EcsCrossDomainWaitPolicy::new(
                EcsCrossDomainWaitTokenKind::VoxelSurfaceArtifactReady,
                class,
                WorkRequiredness::Optional,
                false,
            );
            assert_eq!(
                policy.renderer_present_decision(),
                EcsCrossDomainWaitDecision::Reject(
                    EcsCrossDomainWaitRejectReason::NonRenderPresentDependency
                )
            );
        }
    }

    #[test]
    fn physics_fixed_step_waits_on_critical_proxy_or_uses_conservative_fallback() {
        let critical = EcsCrossDomainWaitPolicy::new(
            EcsCrossDomainWaitTokenKind::VoxelPhysicsProxyReady,
            EcsCrossDomainWaitClass::CollisionCriticalTerrainProxy,
            WorkRequiredness::Required,
            false,
        );
        assert_eq!(
            critical.physics_fixed_step_decision(),
            EcsCrossDomainWaitDecision::Wait
        );

        let conservative = EcsCrossDomainWaitPolicy {
            has_valid_fallback: true,
            ..critical
        };
        assert_eq!(
            conservative.physics_fixed_step_decision(),
            EcsCrossDomainWaitDecision::UseFallback(
                EcsCrossDomainFallbackKind::ConservativePhysicsProxy
            )
        );

        let noncritical = EcsCrossDomainWaitPolicy::new(
            EcsCrossDomainWaitTokenKind::VoxelPhysicsProxyReady,
            EcsCrossDomainWaitClass::NonCriticalPhysicsCook,
            WorkRequiredness::Optional,
            false,
        );
        assert_eq!(
            noncritical.physics_fixed_step_decision(),
            EcsCrossDomainWaitDecision::UseFallback(
                EcsCrossDomainFallbackKind::ConservativePhysicsProxy
            )
        );
    }

    #[test]
    fn lux_and_renderer_do_not_wait_on_present_or_optional_lux_refinement() {
        let renderer_present = EcsCrossDomainWaitPolicy::new(
            EcsCrossDomainWaitTokenKind::VoxelRendererArtifactPublished,
            EcsCrossDomainWaitClass::RendererPresent,
            WorkRequiredness::Required,
            false,
        );
        assert_eq!(
            renderer_present.lux_wait_decision(),
            EcsCrossDomainWaitDecision::Reject(
                EcsCrossDomainWaitRejectReason::LuxWaitsOnRendererPresent
            )
        );

        let optional_lux = EcsCrossDomainWaitPolicy::new(
            EcsCrossDomainWaitTokenKind::VoxelShadowReady,
            EcsCrossDomainWaitClass::LuxRefinement,
            WorkRequiredness::Optional,
            false,
        );
        assert_eq!(
            optional_lux.renderer_lux_refinement_decision(),
            EcsCrossDomainWaitDecision::Reject(
                EcsCrossDomainWaitRejectReason::RendererWaitsOnOptionalLuxRefinement
            )
        );
    }
}
