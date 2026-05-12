use fun_scheduler_types::{
    EcsSpatialWaitTokenKind, EcsVirtualResourceKey, ScheduleDomain, ScheduleLane, WorkRequiredness,
    WorkWaitToken,
};

use crate::{EcsPageChannel, EcsSpatialPageKey, EcsSpatialRegionKey};

pub const ECS_CROSS_DOMAIN_FRAME_STAGE_COUNT: usize = 18;
pub const ECS_CROSS_DOMAIN_FRAME_EDGE_COUNT: usize = 19;

pub const ECS_CROSS_DOMAIN_FRAME_FLOW: [EcsCrossDomainFrameStage;
    ECS_CROSS_DOMAIN_FRAME_STAGE_COUNT] = [
    EcsCrossDomainFrameStage::EcsSenseSources,
    EcsCrossDomainFrameStage::EcsBuildStreamInterest,
    EcsCrossDomainFrameStage::EcsDiffRequests,
    EcsCrossDomainFrameStage::EcsDecodePages,
    EcsCrossDomainFrameStage::EcsBuildArtifacts,
    EcsCrossDomainFrameStage::EcsPropagateDirty,
    EcsCrossDomainFrameStage::EcsPublishHandoffQueues,
    EcsCrossDomainFrameStage::RendererConsumeHandoffs,
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
        }
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
        }
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
        }
    }

    #[must_use]
    pub const fn can_gate_present(self) -> bool {
        matches!(self, Self::RequiredRenderArtifact)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsCrossDomainWaitPolicy {
    pub token: EcsCrossDomainWaitTokenKind,
    pub class: EcsCrossDomainWaitClass,
    pub requiredness: WorkRequiredness,
    pub has_valid_fallback: bool,
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
        }
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EcsCrossDomainWaitDecision {
    Wait,
    UseFallback(EcsCrossDomainFallbackKind),
    Reject(EcsCrossDomainWaitRejectReason),
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
