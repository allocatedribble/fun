use bevy_ecs::prelude::{Component, Resource};
use fun_scheduler_types::EcsChunkKey;

use crate::{
    EcsAabbI64, EcsAssetId, EcsAuthoringCommandId, EcsBoundsUm, EcsDerivedArtifactId,
    EcsDerivedArtifactRecord, EcsDirtyRegion, EcsHandoffQueueId, EcsSpatialPageKey,
    EcsSpatialValidationError, EcsSpatialVolumeId, LuxArtifactHandoff, RendererArtifactHandoff,
    TerrainMaterialId, VoxelPhysicsCookRequest, streaming::IVec3,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct EcsAuthoringTool {
    pub command_queue: EcsHandoffQueueId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct EcsDebugPin {
    pub page: EcsSpatialPageKey,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsAuthoringCommandKind {
    #[default]
    SculptTerrain = 0,
    PaintVoxelMaterial = 1,
    PlaceFoliage = 2,
    MarkDirtyRegion = 3,
    RequestDerivedArtifactBake = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EcsAuthoringEditCommand {
    pub id: EcsAuthoringCommandId,
    pub kind: EcsAuthoringCommandKind,
    pub target_volume: EcsSpatialVolumeId,
    pub bounds: EcsBoundsUm,
    pub chunk_key: EcsChunkKey,
    pub observed_revision: u64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsAssetRef {
    pub asset_id: EcsAssetId,
    pub version: u32,
}

impl EcsAssetRef {
    #[must_use]
    pub const fn new(asset_id: EcsAssetId, version: u32) -> Self {
        Self { asset_id, version }
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.asset_id.is_valid()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PackedTransform {
    pub translation_voxels: IVec3,
    pub rotation_quat_q15: [i16; 4],
    pub scale_q16: [u16; 3],
    pub bounds: EcsAabbI64,
}

impl PackedTransform {
    pub const IDENTITY: Self = Self {
        translation_voxels: IVec3::zero(),
        rotation_quat_q15: [0, 0, 0, i16::MAX],
        scale_q16: [u16::MAX, u16::MAX, u16::MAX],
        bounds: EcsAabbI64::new([0, 0, 0], [1, 1, 1]),
    };

    #[must_use]
    pub const fn new(
        translation_voxels: IVec3,
        rotation_quat_q15: [i16; 4],
        scale_q16: [u16; 3],
        bounds: EcsAabbI64,
    ) -> Self {
        Self {
            translation_voxels,
            rotation_quat_q15,
            scale_q16,
            bounds,
        }
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.scale_q16[0] != 0
            && self.scale_q16[1] != 0
            && self.scale_q16[2] != 0
            && self.bounds.is_bounded_non_empty()
    }
}

impl Default for PackedTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VoxelEditOp {
    FillAabb {
        bounds: EcsAabbI64,
        material: TerrainMaterialId,
    },
    CarveSphere {
        center: IVec3,
        radius_voxels: u16,
    },
    PaintMaterial {
        bounds: EcsAabbI64,
        material: TerrainMaterialId,
    },
    StampSdf {
        transform: PackedTransform,
        sdf_asset: EcsAssetRef,
        material: TerrainMaterialId,
    },
    ApplyExplosion {
        center: IVec3,
        radius_voxels: u16,
        impulse_q: u16,
    },
    RestoreFromSource {
        bounds: EcsAabbI64,
    },
}

impl VoxelEditOp {
    #[must_use]
    pub fn dirty_bounds(self) -> EcsAabbI64 {
        match self {
            Self::FillAabb { bounds, .. }
            | Self::PaintMaterial { bounds, .. }
            | Self::RestoreFromSource { bounds } => bounds,
            Self::CarveSphere {
                center,
                radius_voxels,
            }
            | Self::ApplyExplosion {
                center,
                radius_voxels,
                ..
            } => sphere_bounds(center, radius_voxels),
            Self::StampSdf { transform, .. } => transform.bounds,
        }
    }

    pub fn validate(self) -> Result<(), EcsSpatialValidationError> {
        match self {
            Self::FillAabb { bounds, material } | Self::PaintMaterial { bounds, material } => {
                validate_bounds(bounds)?;
                validate_material(material)
            }
            Self::CarveSphere { radius_voxels, .. } => validate_radius(radius_voxels),
            Self::StampSdf {
                transform,
                sdf_asset,
                material,
            } => {
                if !transform.is_valid() {
                    return Err(EcsSpatialValidationError::InvalidTransform);
                }
                if !sdf_asset.is_valid() {
                    return Err(EcsSpatialValidationError::InvalidAssetRef);
                }
                validate_material(material)
            }
            Self::ApplyExplosion {
                radius_voxels,
                impulse_q,
                ..
            } => {
                validate_radius(radius_voxels)?;
                if impulse_q == 0 {
                    return Err(EcsSpatialValidationError::InvalidEditRadius);
                }
                Ok(())
            }
            Self::RestoreFromSource { bounds } => validate_bounds(bounds),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsVoxelEditLogEntry {
    pub command_id: EcsAuthoringCommandId,
    pub op: VoxelEditOp,
    pub dirty_epoch: u32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct EcsVoxelEditLog {
    pub entries: Vec<EcsVoxelEditLogEntry>,
}

impl EcsVoxelEditLog {
    pub fn push(&mut self, entry: EcsVoxelEditLogEntry) -> Result<(), EcsSpatialValidationError> {
        entry.op.validate()?;
        if self.entries.len() >= crate::ECS_SPATIAL_MAX_HANDOFF_ROWS {
            return Err(EcsSpatialValidationError::HandoffQueueFull);
        }
        self.entries.push(entry);
        Ok(())
    }
}

fn validate_bounds(bounds: EcsAabbI64) -> Result<(), EcsSpatialValidationError> {
    if bounds.is_bounded_non_empty() {
        Ok(())
    } else {
        Err(EcsSpatialValidationError::InvalidEditBounds)
    }
}

fn validate_material(material: TerrainMaterialId) -> Result<(), EcsSpatialValidationError> {
    if material.is_valid() {
        Ok(())
    } else {
        Err(EcsSpatialValidationError::InvalidMaterial)
    }
}

fn validate_radius(radius_voxels: u16) -> Result<(), EcsSpatialValidationError> {
    if radius_voxels != 0 {
        Ok(())
    } else {
        Err(EcsSpatialValidationError::InvalidEditRadius)
    }
}

fn sphere_bounds(center: IVec3, radius_voxels: u16) -> EcsAabbI64 {
    let radius = i64::from(radius_voxels);
    let x = i64::from(center.x);
    let y = i64::from(center.y);
    let z = i64::from(center.z);
    EcsAabbI64::new(
        [
            x.saturating_sub(radius),
            y.saturating_sub(radius),
            z.saturating_sub(radius),
        ],
        [
            x.saturating_add(radius).saturating_add(1),
            y.saturating_add(radius).saturating_add(1),
            z.saturating_add(radius).saturating_add(1),
        ],
    )
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EcsSpatialCommand {
    RequestPage(EcsSpatialPageKey),
    CancelPage(EcsSpatialPageKey),
    PinPage(EcsSpatialPageKey),
    UnpinPage(EcsSpatialPageKey),
    PublishArtifact(EcsDerivedArtifactRecord),
    RetireArtifact(EcsDerivedArtifactId),
    MarkDirty(EcsDirtyRegion),
    ApplyVoxelEdit(VoxelEditOp),
    PublishRendererHandoff(RendererArtifactHandoff),
    PublishLuxHandoff(LuxArtifactHandoff),
    PublishPhysicsCook(VoxelPhysicsCookRequest),
}

#[derive(Debug, Default, Clone, PartialEq, Resource)]
pub struct EcsSpatialCommandBuffer {
    pub commands: Vec<EcsSpatialCommand>,
}

impl EcsSpatialCommandBuffer {
    pub fn push(&mut self, command: EcsSpatialCommand) -> Result<(), EcsSpatialValidationError> {
        if self.commands.len() >= crate::ECS_SPATIAL_MAX_HANDOFF_ROWS {
            return Err(EcsSpatialValidationError::HandoffQueueFull);
        }
        self.commands.push(command);
        Ok(())
    }
}
