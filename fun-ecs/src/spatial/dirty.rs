use bevy_ecs::prelude::Resource;
use fun_scheduler_types::EcsChunkKey;

use crate::{
    CollisionCookMode, ECS_SPATIAL_MAX_DIRTY_REGIONS, EcsArtifactConsumer, EcsArtifactState,
    EcsBoundsUm, EcsDerivedArtifactId, EcsDerivedArtifactKind, EcsDerivedArtifactRecord,
    EcsPageChannel, EcsPageResidencyTable, EcsSpatialCommand, EcsSpatialCommandBuffer,
    EcsSpatialDomainKind, EcsSpatialPageKey, EcsSpatialValidationError, FixedStepId, VoxelEditOp,
    VoxelPhysicsCookRequest, derived_artifact_source_digest,
};
use fun_scheduler_types::WorkRequiredness;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EcsDirtyRegion {
    pub domain: EcsSpatialDomainKind,
    pub page: Option<EcsSpatialPageKey>,
    pub bounds: EcsBoundsUm,
    pub chunk_key: EcsChunkKey,
    pub dirty_epoch: u32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct EcsDirtyRegionLedger {
    pub regions: Vec<EcsDirtyRegion>,
    pub epoch: u32,
}

impl EcsDirtyRegionLedger {
    pub fn push(&mut self, region: EcsDirtyRegion) -> Result<(), EcsSpatialValidationError> {
        if self.regions.len() >= ECS_SPATIAL_MAX_DIRTY_REGIONS {
            return Err(EcsSpatialValidationError::DirtyRegionQueueFull);
        }
        self.regions.push(region);
        self.epoch = self.epoch.max(region.dirty_epoch);
        Ok(())
    }

    pub fn advance_epoch(&mut self) -> u32 {
        self.epoch = self.epoch.saturating_add(1);
        self.epoch
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsVoxelEditImpactMask {
    bits: u16,
}

impl EcsVoxelEditImpactMask {
    pub const NONE: Self = Self { bits: 0 };
    pub const SURFACE: Self = Self { bits: 1 << 0 };
    pub const MATERIAL: Self = Self { bits: 1 << 1 };
    pub const SDF: Self = Self { bits: 1 << 2 };
    pub const SHADOW: Self = Self { bits: 1 << 3 };
    pub const FOLIAGE: Self = Self { bits: 1 << 4 };
    pub const PHYSICS: Self = Self { bits: 1 << 5 };
    pub const NAVIGATION: Self = Self { bits: 1 << 6 };
    pub const AUDIO: Self = Self { bits: 1 << 7 };
    pub const NETWORK: Self = Self { bits: 1 << 8 };

    #[must_use]
    pub const fn from_bits(bits: u16) -> Self {
        Self { bits }
    }

    #[must_use]
    pub const fn bits(self) -> u16 {
        self.bits
    }

    #[must_use]
    pub const fn contains(self, flag: Self) -> bool {
        (self.bits & flag.bits) != 0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }

    #[must_use]
    pub const fn for_edit(edit: VoxelEditOp) -> Self {
        match edit {
            VoxelEditOp::FillAabb { .. } | VoxelEditOp::RestoreFromSource { .. } => Self {
                bits: Self::SURFACE.bits
                    | Self::MATERIAL.bits
                    | Self::SDF.bits
                    | Self::SHADOW.bits
                    | Self::FOLIAGE.bits
                    | Self::PHYSICS.bits
                    | Self::NAVIGATION.bits
                    | Self::AUDIO.bits
                    | Self::NETWORK.bits,
            },
            VoxelEditOp::CarveSphere { .. } | VoxelEditOp::ApplyExplosion { .. } => Self {
                bits: Self::SURFACE.bits
                    | Self::SDF.bits
                    | Self::SHADOW.bits
                    | Self::FOLIAGE.bits
                    | Self::PHYSICS.bits
                    | Self::NAVIGATION.bits
                    | Self::AUDIO.bits
                    | Self::NETWORK.bits,
            },
            VoxelEditOp::PaintMaterial { .. } => Self {
                bits: Self::SURFACE.bits
                    | Self::MATERIAL.bits
                    | Self::SHADOW.bits
                    | Self::FOLIAGE.bits
                    | Self::NETWORK.bits,
            },
            VoxelEditOp::StampSdf { .. } => Self {
                bits: Self::SURFACE.bits
                    | Self::MATERIAL.bits
                    | Self::SDF.bits
                    | Self::SHADOW.bits
                    | Self::PHYSICS.bits
                    | Self::NAVIGATION.bits
                    | Self::AUDIO.bits
                    | Self::NETWORK.bits,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EcsVoxelEditPropagationOptions {
    pub source_page: EcsSpatialPageKey,
    pub dirty_epoch: u32,
    pub source_epoch: u32,
    pub collision_cook_mode: CollisionCookMode,
    pub collision_requiredness: WorkRequiredness,
    pub fixed_step_deadline: Option<FixedStepId>,
    pub navigation_enabled: bool,
    pub audio_enabled: bool,
    pub network_enabled: bool,
    pub conservative_proxy_allowed: bool,
}

impl EcsVoxelEditPropagationOptions {
    #[must_use]
    pub const fn collision_critical(
        source_page: EcsSpatialPageKey,
        dirty_epoch: u32,
        fixed_step_deadline: FixedStepId,
    ) -> Self {
        Self {
            source_page,
            dirty_epoch,
            source_epoch: 0,
            collision_cook_mode: CollisionCookMode::NarrowBandSdf,
            collision_requiredness: WorkRequiredness::Required,
            fixed_step_deadline: Some(fixed_step_deadline),
            navigation_enabled: false,
            audio_enabled: false,
            network_enabled: false,
            conservative_proxy_allowed: true,
        }
    }

    #[must_use]
    pub const fn far_background(source_page: EcsSpatialPageKey, dirty_epoch: u32) -> Self {
        Self {
            source_page,
            dirty_epoch,
            source_epoch: 0,
            collision_cook_mode: CollisionCookMode::HeightProxy,
            collision_requiredness: WorkRequiredness::Optional,
            fixed_step_deadline: None,
            navigation_enabled: false,
            audio_enabled: false,
            network_enabled: false,
            conservative_proxy_allowed: true,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EcsVoxelEditPropagationReport {
    pub dirty_regions_marked: u32,
    pub page_epochs_advanced: u32,
    pub artifacts_marked_dirty: u32,
    pub physics_cooks_published: u32,
    pub conservative_proxy_allowed: bool,
}

pub fn propagate_voxel_edit(
    edit: VoxelEditOp,
    options: EcsVoxelEditPropagationOptions,
    page_table: &mut EcsPageResidencyTable,
    dirty_ledger: &mut EcsDirtyRegionLedger,
    next_artifact_id: &mut u64,
    commands: &mut EcsSpatialCommandBuffer,
) -> Result<EcsVoxelEditPropagationReport, EcsSpatialValidationError> {
    edit.validate()?;
    let mut report = EcsVoxelEditPropagationReport {
        conservative_proxy_allowed: options.conservative_proxy_allowed,
        ..EcsVoxelEditPropagationReport::default()
    };
    let impact = EcsVoxelEditImpactMask::for_edit(edit);
    let dirty_epoch = options.dirty_epoch.max(1);
    let dirty_region = EcsDirtyRegion {
        domain: EcsSpatialDomainKind::Terrain,
        page: Some(options.source_page),
        bounds: edit.dirty_bounds(),
        chunk_key: options.source_page.chunk_key(),
        dirty_epoch,
    };
    dirty_ledger.push(dirty_region)?;
    commands.push(EcsSpatialCommand::MarkDirty(dirty_region))?;
    report.dirty_regions_marked = 1;

    if page_table.mark_dirty_by_key(options.source_page, dirty_epoch) {
        report.page_epochs_advanced = 1;
    }

    if impact.contains(EcsVoxelEditImpactMask::SURFACE) {
        push_dirty_artifact(
            EcsDerivedArtifactKind::TerrainSurfacePackets,
            EcsArtifactConsumer::Renderer,
            WorkRequiredness::Required,
            options,
            next_artifact_id,
            commands,
            &mut report,
        )?;
    }
    if impact.contains(EcsVoxelEditImpactMask::MATERIAL) {
        push_dirty_artifact(
            EcsDerivedArtifactKind::TerrainMaterialPage,
            EcsArtifactConsumer::Renderer,
            WorkRequiredness::Required,
            options,
            next_artifact_id,
            commands,
            &mut report,
        )?;
    }
    if impact.contains(EcsVoxelEditImpactMask::SDF) {
        push_dirty_artifact(
            EcsDerivedArtifactKind::TerrainSdf,
            EcsArtifactConsumer::Lux,
            WorkRequiredness::Required,
            options,
            next_artifact_id,
            commands,
            &mut report,
        )?;
        push_dirty_artifact(
            EcsDerivedArtifactKind::CollisionSdfProxy,
            EcsArtifactConsumer::AvisPhysics,
            options.collision_requiredness,
            options,
            next_artifact_id,
            commands,
            &mut report,
        )?;
    }
    if impact.contains(EcsVoxelEditImpactMask::SHADOW) {
        push_dirty_artifact(
            EcsDerivedArtifactKind::ShadowInvalidationRows,
            EcsArtifactConsumer::Lux,
            WorkRequiredness::Optional,
            options,
            next_artifact_id,
            commands,
            &mut report,
        )?;
    }
    if impact.contains(EcsVoxelEditImpactMask::FOLIAGE) {
        push_dirty_artifact(
            EcsDerivedArtifactKind::FoliageClusters,
            EcsArtifactConsumer::Renderer,
            WorkRequiredness::Optional,
            options,
            next_artifact_id,
            commands,
            &mut report,
        )?;
        push_dirty_artifact(
            EcsDerivedArtifactKind::CanopyOpacity,
            EcsArtifactConsumer::Lux,
            WorkRequiredness::Optional,
            options,
            next_artifact_id,
            commands,
            &mut report,
        )?;
    }
    if impact.contains(EcsVoxelEditImpactMask::PHYSICS) {
        push_dirty_artifact(
            EcsDerivedArtifactKind::PhysicsCookRequests,
            EcsArtifactConsumer::AvisPhysics,
            options.collision_requiredness,
            options,
            next_artifact_id,
            commands,
            &mut report,
        )?;
        commands.push(EcsSpatialCommand::PublishPhysicsCook(
            VoxelPhysicsCookRequest {
                source_page: options.source_page,
                dirty_epoch,
                mode: options.collision_cook_mode,
                requiredness: options.collision_requiredness,
                fixed_step_deadline: options.fixed_step_deadline,
            },
        ))?;
        report.physics_cooks_published += 1;
    }
    if options.navigation_enabled && impact.contains(EcsVoxelEditImpactMask::NAVIGATION) {
        push_dirty_artifact(
            EcsDerivedArtifactKind::NavTile,
            EcsArtifactConsumer::Navigation,
            WorkRequiredness::Optional,
            options,
            next_artifact_id,
            commands,
            &mut report,
        )?;
    }
    if options.audio_enabled && impact.contains(EcsVoxelEditImpactMask::AUDIO) {
        push_dirty_artifact(
            EcsDerivedArtifactKind::AudioOcclusionTile,
            EcsArtifactConsumer::Audio,
            WorkRequiredness::Optional,
            options,
            next_artifact_id,
            commands,
            &mut report,
        )?;
    }
    if options.network_enabled && impact.contains(EcsVoxelEditImpactMask::NETWORK) {
        push_dirty_artifact(
            EcsDerivedArtifactKind::VoxelEditDeltaRows,
            EcsArtifactConsumer::ThunderNetwork,
            WorkRequiredness::Required,
            options,
            next_artifact_id,
            commands,
            &mut report,
        )?;
        push_dirty_artifact(
            EcsDerivedArtifactKind::NetworkRelevanceRows,
            EcsArtifactConsumer::ThunderNetwork,
            WorkRequiredness::Required,
            options,
            next_artifact_id,
            commands,
            &mut report,
        )?;
    }

    Ok(report)
}

fn push_dirty_artifact(
    kind: EcsDerivedArtifactKind,
    consumer: EcsArtifactConsumer,
    requiredness: WorkRequiredness,
    options: EcsVoxelEditPropagationOptions,
    next_artifact_id: &mut u64,
    commands: &mut EcsSpatialCommandBuffer,
    report: &mut EcsVoxelEditPropagationReport,
) -> Result<(), EcsSpatialValidationError> {
    *next_artifact_id = next_artifact_id.saturating_add(1).max(1);
    commands.push(EcsSpatialCommand::PublishArtifact(
        EcsDerivedArtifactRecord {
            artifact_id: EcsDerivedArtifactId::new(*next_artifact_id),
            source_page: options.source_page_for_kind(kind),
            kind,
            source_epoch: options.source_epoch,
            source_digest: derived_artifact_source_digest(
                options.source_page_for_kind(kind),
                options.source_epoch,
                options.dirty_epoch.max(1),
            ),
            artifact_epoch: options.dirty_epoch.max(1),
            state: EcsArtifactState::Requested,
            requiredness,
            consumer,
        },
    ))?;
    report.artifacts_marked_dirty += 1;
    Ok(())
}

trait EcsVoxelEditPropagationPageExt {
    fn source_page_for_kind(self, kind: EcsDerivedArtifactKind) -> EcsSpatialPageKey;
}

impl EcsVoxelEditPropagationPageExt for EcsVoxelEditPropagationOptions {
    fn source_page_for_kind(self, kind: EcsDerivedArtifactKind) -> EcsSpatialPageKey {
        let channel = match kind {
            EcsDerivedArtifactKind::TerrainSurfacePackets
            | EcsDerivedArtifactKind::TerrainCoarseProxy
            | EcsDerivedArtifactKind::FoliageClusters
            | EcsDerivedArtifactKind::FoliageSeeds => EcsPageChannel::Surface,
            EcsDerivedArtifactKind::TerrainMaterialPage => EcsPageChannel::Material,
            EcsDerivedArtifactKind::TerrainSdf
            | EcsDerivedArtifactKind::PhysicsCollisionProxy
            | EcsDerivedArtifactKind::PhysicsCookRequests
            | EcsDerivedArtifactKind::CollisionSdfProxy => EcsPageChannel::NarrowBandSdf,
            EcsDerivedArtifactKind::FoliageTrunkBranchVirtualGeometry
            | EcsDerivedArtifactKind::FoliageGrassBrushInstanceClusters
            | EcsDerivedArtifactKind::FoliageCardsImpostors
            | EcsDerivedArtifactKind::FoliageClusteredAnimation
            | EcsDerivedArtifactKind::FoliageBiomeTint
            | EcsDerivedArtifactKind::FoliageHorizonImpostorField => {
                EcsPageChannel::FoliageGeometry
            }
            EcsDerivedArtifactKind::FoliageCollisionLargeObjectProxy => EcsPageChannel::CoarseProxy,
            EcsDerivedArtifactKind::CanopyOpacity
            | EcsDerivedArtifactKind::FoliageCanopyTransmittance => EcsPageChannel::CanopyOpacity,
            EcsDerivedArtifactKind::VirtualShadowInvalidation
            | EcsDerivedArtifactKind::ShadowInvalidationRows
            | EcsDerivedArtifactKind::FoliageSdfOpacityShadows => EcsPageChannel::VirtualShadow,
            EcsDerivedArtifactKind::RadianceCacheUpdate
            | EcsDerivedArtifactKind::RadianceUpdateRows => EcsPageChannel::Radiance,
            EcsDerivedArtifactKind::StormExtinctionDirtyRows => EcsPageChannel::WeatherExtinction,
            EcsDerivedArtifactKind::NavTile => EcsPageChannel::Navigation,
            EcsDerivedArtifactKind::AudioOcclusionTile => EcsPageChannel::AudioOcclusion,
            EcsDerivedArtifactKind::NetworkDeltaRows
            | EcsDerivedArtifactKind::VoxelEditDeltaRows
            | EcsDerivedArtifactKind::NetworkRelevanceRows => EcsPageChannel::NetworkRelevance,
            EcsDerivedArtifactKind::LoadAnimationRecord => EcsPageChannel::Surface,
        };
        EcsSpatialPageKey {
            channel,
            ..self.source_page
        }
    }
}
