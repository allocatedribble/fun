use crate::{Component, Resource};
use fun_scheduler_types::EcsChunkKey;

use crate::{
    EcsAabbI64, EcsAssetId, EcsAuthoringCommandId, EcsBoundsUm, EcsDerivedArtifactId,
    EcsDerivedArtifactRecord, EcsDerivedArtifactRegistry, EcsDirtyRegion, EcsHandoffQueueId,
    EcsLuxHandoffQueue, EcsPhysicsCookQueue, EcsRendererHandoffQueue, EcsSpatialCommandBarrierKind,
    EcsSpatialPageKey, EcsSpatialValidationError, EcsSpatialVolumeId, FunArtifactId,
    FunCommandApplyRevisionReport, FunResourceId, FunResourceTableId, FunRevision,
    FunStorageChunkId, LuxArtifactHandoff, RendererArtifactHandoff, RevisionCategory,
    TerrainMaterialId, VoxelPhysicsCookRequest, WorldRevisionLedger, streaming::IVec3,
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
    pub observed_revision: FunRevision,
}

impl EcsSpatialCommandBuffer {
    #[must_use]
    pub fn new(observed_revision: FunRevision) -> Self {
        Self {
            commands: Vec::new(),
            observed_revision,
        }
    }

    pub fn set_observed_revision(&mut self, revision: FunRevision) {
        self.observed_revision = revision;
    }

    pub fn push(&mut self, command: EcsSpatialCommand) -> Result<(), EcsSpatialValidationError> {
        if self.commands.len() >= crate::ECS_SPATIAL_MAX_HANDOFF_ROWS {
            return Err(EcsSpatialValidationError::HandoffQueueFull);
        }
        self.commands.push(command);
        Ok(())
    }

    #[must_use]
    pub fn drain_for_barrier(
        &mut self,
        barrier: EcsSpatialCommandBarrierKind,
    ) -> Vec<EcsSpatialCommand> {
        let mut selected = Vec::new();
        let mut retained = Vec::new();
        for command in self.commands.drain(..) {
            if command_matches_barrier(command, barrier) {
                selected.push(command);
            } else {
                retained.push(command);
            }
        }
        selected.sort_by_key(|command| command.sort_key());
        self.commands = retained;
        selected
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsSpatialCommandApplyDigest {
    pub value: u64,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EcsSpatialCommandApplyReport {
    pub barrier: EcsSpatialCommandBarrierKind,
    pub inspected: u32,
    pub applied: u32,
    pub ignored: u32,
    pub retained: u32,
    pub digest: EcsSpatialCommandApplyDigest,
    pub revision: FunCommandApplyRevisionReport,
}

pub fn apply_artifact_commands(
    commands: &mut EcsSpatialCommandBuffer,
    registry: &mut EcsDerivedArtifactRegistry,
) -> Result<EcsSpatialCommandApplyReport, EcsSpatialValidationError> {
    let mut ledger = WorldRevisionLedger::at_revision(commands.observed_revision);
    apply_artifact_commands_with_revisions(commands, registry, &mut ledger)
}

pub fn apply_artifact_commands_with_revisions(
    commands: &mut EcsSpatialCommandBuffer,
    registry: &mut EcsDerivedArtifactRegistry,
    revisions: &mut WorldRevisionLedger,
) -> Result<EcsSpatialCommandApplyReport, EcsSpatialValidationError> {
    let barrier = EcsSpatialCommandBarrierKind::ApplyArtifactCommands;
    validate_command_revision(commands, revisions, RevisionCategory::Artifact)?;
    let inspected = commands.commands.len() as u32;
    let selected = commands.drain_for_barrier(barrier);
    let digest = EcsSpatialCommandApplyDigest::from_commands(&selected);
    let mut revision = command_revision_report(commands, revisions, RevisionCategory::Artifact);
    let mut report = EcsSpatialCommandApplyReport {
        barrier,
        inspected,
        retained: commands.commands.len() as u32,
        digest,
        revision: FunCommandApplyRevisionReport::default(),
        ..EcsSpatialCommandApplyReport::default()
    };
    for command in selected {
        match command {
            EcsSpatialCommand::PublishArtifact(artifact) => {
                registry.push(artifact)?;
                touch_artifact_registry(&mut revision, artifact.artifact_id);
                report.applied += 1;
            }
            EcsSpatialCommand::RetireArtifact(artifact_id) => {
                if registry.retire(artifact_id) {
                    touch_artifact_registry(&mut revision, artifact_id);
                    report.applied += 1;
                } else {
                    report.ignored += 1;
                }
            }
            _ => report.ignored += 1,
        }
    }
    finish_command_revision_report(
        &mut revision,
        report.applied,
        revisions,
        RevisionCategory::Artifact,
    )?;
    report.revision = revision;
    Ok(report)
}

pub fn apply_dirty_propagation_commands(
    commands: &mut EcsSpatialCommandBuffer,
    registry: &mut EcsDerivedArtifactRegistry,
    physics_cooks: &mut EcsPhysicsCookQueue,
) -> Result<EcsSpatialCommandApplyReport, EcsSpatialValidationError> {
    let mut ledger = WorldRevisionLedger::at_revision(commands.observed_revision);
    apply_dirty_propagation_commands_with_revisions(commands, registry, physics_cooks, &mut ledger)
}

pub fn apply_dirty_propagation_commands_with_revisions(
    commands: &mut EcsSpatialCommandBuffer,
    registry: &mut EcsDerivedArtifactRegistry,
    physics_cooks: &mut EcsPhysicsCookQueue,
    revisions: &mut WorldRevisionLedger,
) -> Result<EcsSpatialCommandApplyReport, EcsSpatialValidationError> {
    let barrier = EcsSpatialCommandBarrierKind::ApplyDirtyPropagationCommands;
    validate_command_revision(commands, revisions, RevisionCategory::Spatial)?;
    let inspected = commands.commands.len() as u32;
    let selected = commands.drain_for_barrier(barrier);
    let digest = EcsSpatialCommandApplyDigest::from_commands(&selected);
    let mut revision = command_revision_report(commands, revisions, RevisionCategory::Spatial);
    let mut report = EcsSpatialCommandApplyReport {
        barrier,
        inspected,
        retained: commands.commands.len() as u32,
        digest,
        revision: FunCommandApplyRevisionReport::default(),
        ..EcsSpatialCommandApplyReport::default()
    };
    for command in selected {
        match command {
            EcsSpatialCommand::PublishArtifact(artifact) => {
                registry.push(artifact)?;
                touch_artifact_registry(&mut revision, artifact.artifact_id);
                report.applied += 1;
            }
            EcsSpatialCommand::PublishPhysicsCook(request) => {
                physics_cooks.push(request)?;
                touch_physics_cook_queue(&mut revision);
                report.applied += 1;
            }
            EcsSpatialCommand::MarkDirty(_) | EcsSpatialCommand::ApplyVoxelEdit(_) => {
                report.ignored += 1;
            }
            _ => report.ignored += 1,
        }
    }
    finish_command_revision_report(
        &mut revision,
        report.applied,
        revisions,
        RevisionCategory::Spatial,
    )?;
    report.revision = revision;
    Ok(report)
}

pub fn apply_handoff_commands(
    commands: &mut EcsSpatialCommandBuffer,
    renderer_handoffs: &mut EcsRendererHandoffQueue,
    lux_handoffs: &mut EcsLuxHandoffQueue,
    physics_cooks: &mut EcsPhysicsCookQueue,
) -> Result<EcsSpatialCommandApplyReport, EcsSpatialValidationError> {
    let mut ledger = WorldRevisionLedger::at_revision(commands.observed_revision);
    apply_handoff_commands_with_revisions(
        commands,
        renderer_handoffs,
        lux_handoffs,
        physics_cooks,
        &mut ledger,
    )
}

pub fn apply_handoff_commands_with_revisions(
    commands: &mut EcsSpatialCommandBuffer,
    renderer_handoffs: &mut EcsRendererHandoffQueue,
    lux_handoffs: &mut EcsLuxHandoffQueue,
    physics_cooks: &mut EcsPhysicsCookQueue,
    revisions: &mut WorldRevisionLedger,
) -> Result<EcsSpatialCommandApplyReport, EcsSpatialValidationError> {
    let barrier = EcsSpatialCommandBarrierKind::ApplyHandoffCommands;
    validate_command_revision(commands, revisions, RevisionCategory::Handoff)?;
    let inspected = commands.commands.len() as u32;
    let selected = commands.drain_for_barrier(barrier);
    let digest = EcsSpatialCommandApplyDigest::from_commands(&selected);
    let mut revision = command_revision_report(commands, revisions, RevisionCategory::Handoff);
    let mut report = EcsSpatialCommandApplyReport {
        barrier,
        inspected,
        retained: commands.commands.len() as u32,
        digest,
        revision: FunCommandApplyRevisionReport::default(),
        ..EcsSpatialCommandApplyReport::default()
    };
    for command in selected {
        match command {
            EcsSpatialCommand::PublishRendererHandoff(handoff) => {
                revision
                    .touch_artifact(FunArtifactId::from_derived_artifact_id(handoff.artifact_id));
                touch_renderer_handoff_queue(&mut revision);
                renderer_handoffs.push(handoff)?;
                report.applied += 1;
            }
            EcsSpatialCommand::PublishLuxHandoff(handoff) => {
                touch_lux_handoff_queue(&mut revision);
                lux_handoffs.push(handoff)?;
                report.applied += 1;
            }
            EcsSpatialCommand::PublishPhysicsCook(request) => {
                touch_physics_cook_queue(&mut revision);
                physics_cooks.push(request)?;
                report.applied += 1;
            }
            _ => report.ignored += 1,
        }
    }
    finish_command_revision_report(
        &mut revision,
        report.applied,
        revisions,
        RevisionCategory::Handoff,
    )?;
    report.revision = revision;
    Ok(report)
}

fn validate_command_revision(
    commands: &EcsSpatialCommandBuffer,
    revisions: &WorldRevisionLedger,
    category: RevisionCategory,
) -> Result<(), EcsSpatialValidationError> {
    if commands.observed_revision == revisions.category_revision(category) {
        Ok(())
    } else {
        Err(EcsSpatialValidationError::StaleCommandBuffer)
    }
}

fn command_revision_report(
    commands: &EcsSpatialCommandBuffer,
    revisions: &WorldRevisionLedger,
    category: RevisionCategory,
) -> FunCommandApplyRevisionReport {
    FunCommandApplyRevisionReport::new(
        commands.observed_revision,
        revisions.category_revision(category),
    )
}

fn finish_command_revision_report(
    revision: &mut FunCommandApplyRevisionReport,
    applied: u32,
    revisions: &mut WorldRevisionLedger,
    category: RevisionCategory,
) -> Result<(), EcsSpatialValidationError> {
    let new_revision = if applied == 0 {
        revisions.category_revision(category)
    } else {
        let (_previous, new) = revisions.advance_category(category);
        for table in &revision.touched_tables {
            revisions
                .resource_revisions
                .touch(
                    FunResourceId::new(table.get()),
                    *table,
                    FunStorageChunkId::new(1),
                    new,
                )
                .map_err(|_error| EcsSpatialValidationError::RevisionLedgerFull)?;
        }
        new
    };
    revision.finish(new_revision);
    Ok(())
}

fn touch_artifact_registry(
    revision: &mut FunCommandApplyRevisionReport,
    artifact_id: EcsDerivedArtifactId,
) {
    touch_resource_table(revision, crate::FunEcsResourceKind::DerivedArtifactRegistry);
    revision.touch_artifact(FunArtifactId::from_derived_artifact_id(artifact_id));
}

fn touch_renderer_handoff_queue(revision: &mut FunCommandApplyRevisionReport) {
    touch_resource_table(revision, crate::FunEcsResourceKind::RendererHandoffQueue);
}

fn touch_lux_handoff_queue(revision: &mut FunCommandApplyRevisionReport) {
    touch_resource_table(revision, crate::FunEcsResourceKind::LuxHandoffQueue);
}

fn touch_physics_cook_queue(revision: &mut FunCommandApplyRevisionReport) {
    touch_resource_table(revision, crate::FunEcsResourceKind::PhysicsCookQueue);
}

fn touch_resource_table(
    revision: &mut FunCommandApplyRevisionReport,
    resource: crate::FunEcsResourceKind,
) {
    revision.touch_resource(FunResourceId::from_resource_kind(resource));
    revision.touch_table(FunResourceTableId::from_resource_kind(resource));
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EcsSpatialCommandSortKey {
    parts: [u64; 16],
}

impl EcsSpatialCommand {
    #[must_use]
    pub fn sort_key(self) -> EcsSpatialCommandSortKey {
        match self {
            Self::RequestPage(page) => command_key(0, page, [0; 8]),
            Self::CancelPage(page) => command_key(1, page, [0; 8]),
            Self::PinPage(page) => command_key(2, page, [0; 8]),
            Self::UnpinPage(page) => command_key(3, page, [0; 8]),
            Self::PublishArtifact(artifact) => command_key(
                4,
                artifact.source_page,
                [
                    artifact.artifact_id.get(),
                    artifact.kind as u64,
                    artifact.source_epoch.into(),
                    artifact.artifact_epoch.into(),
                    artifact.state as u64,
                    requiredness_code(artifact.requiredness),
                    artifact.consumer as u64,
                    0,
                ],
            ),
            Self::RetireArtifact(artifact_id) => key_from_parts([
                5,
                artifact_id.get(),
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            ]),
            Self::MarkDirty(region) => command_key(
                6,
                region.page.unwrap_or(EcsSpatialPageKey::new(
                    region.domain,
                    crate::EcsSpatialGridId::INVALID,
                    0,
                    0,
                    0,
                    0,
                    crate::EcsPageChannel::Debug,
                )),
                [
                    region.chunk_key.get(),
                    region.dirty_epoch.into(),
                    i64_bits(region.bounds.min[0]),
                    i64_bits(region.bounds.min[1]),
                    i64_bits(region.bounds.min[2]),
                    i64_bits(region.bounds.max[0]),
                    i64_bits(region.bounds.max[1]),
                    i64_bits(region.bounds.max[2]),
                ],
            ),
            Self::ApplyVoxelEdit(edit) => key_from_parts([
                7,
                voxel_edit_code(edit),
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            ]),
            Self::PublishRendererHandoff(handoff) => command_key(
                8,
                handoff.source_page,
                [
                    handoff.artifact_id.get(),
                    handoff.kind as u64,
                    handoff.source_epoch.into(),
                    handoff.artifact_epoch.into(),
                    requiredness_code(handoff.requiredness),
                    handoff.visibility_hint as u64,
                    0,
                    0,
                ],
            ),
            Self::PublishLuxHandoff(handoff) => command_key(
                9,
                handoff.source_page,
                [
                    handoff.kind as u64,
                    handoff.dirty_epoch.into(),
                    requiredness_code(handoff.requiredness),
                    u64::from(handoff.bounds_world.min[0].to_bits()),
                    u64::from(handoff.bounds_world.min[1].to_bits()),
                    u64::from(handoff.bounds_world.min[2].to_bits()),
                    u64::from(handoff.bounds_world.max[0].to_bits()),
                    u64::from(handoff.bounds_world.max[1].to_bits()),
                ],
            ),
            Self::PublishPhysicsCook(request) => command_key(
                10,
                request.source_page,
                [
                    request.dirty_epoch.into(),
                    request.mode as u64,
                    requiredness_code(request.requiredness),
                    request.fixed_step_deadline.map_or(0, |step| step.get()),
                    0,
                    0,
                    0,
                    0,
                ],
            ),
        }
    }
}

impl EcsSpatialCommandApplyDigest {
    #[must_use]
    pub fn from_commands(commands: &[EcsSpatialCommand]) -> Self {
        if commands.is_empty() {
            return Self { value: 0 };
        }
        let mut sorted = commands.to_vec();
        sorted.sort_by_key(|command| command.sort_key());
        let mut value = 0xcbf2_9ce4_8422_2325_u64;
        for command in sorted {
            for part in command.sort_key().parts {
                value ^= part;
                value = value.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        Self { value }
    }
}

fn command_matches_barrier(
    command: EcsSpatialCommand,
    barrier: EcsSpatialCommandBarrierKind,
) -> bool {
    match barrier {
        EcsSpatialCommandBarrierKind::ApplyRequestCommands => matches!(
            command,
            EcsSpatialCommand::RequestPage(_)
                | EcsSpatialCommand::CancelPage(_)
                | EcsSpatialCommand::PinPage(_)
                | EcsSpatialCommand::UnpinPage(_)
        ),
        EcsSpatialCommandBarrierKind::ApplyArtifactCommands => matches!(
            command,
            EcsSpatialCommand::PublishArtifact(_) | EcsSpatialCommand::RetireArtifact(_)
        ),
        EcsSpatialCommandBarrierKind::ApplyDirtyPropagationCommands => matches!(
            command,
            EcsSpatialCommand::MarkDirty(_)
                | EcsSpatialCommand::ApplyVoxelEdit(_)
                | EcsSpatialCommand::PublishArtifact(_)
                | EcsSpatialCommand::PublishPhysicsCook(_)
        ),
        EcsSpatialCommandBarrierKind::ApplyHandoffCommands => matches!(
            command,
            EcsSpatialCommand::PublishRendererHandoff(_)
                | EcsSpatialCommand::PublishLuxHandoff(_)
                | EcsSpatialCommand::PublishPhysicsCook(_)
        ),
    }
}

fn command_key(kind: u64, page: EcsSpatialPageKey, extra: [u64; 8]) -> EcsSpatialCommandSortKey {
    key_from_parts([
        kind,
        page.domain as u64,
        page.grid_id.get(),
        u64::from(page.level),
        page.x as u32 as u64,
        page.y as u32 as u64,
        page.z as u32 as u64,
        page.channel as u64,
        extra[0],
        extra[1],
        extra[2],
        extra[3],
        extra[4],
        extra[5],
        extra[6],
        extra[7],
    ])
}

const fn key_from_parts(parts: [u64; 16]) -> EcsSpatialCommandSortKey {
    EcsSpatialCommandSortKey { parts }
}

fn requiredness_code(requiredness: fun_scheduler_types::WorkRequiredness) -> u64 {
    if requiredness.is_optional() { 1 } else { 0 }
}

fn voxel_edit_code(edit: VoxelEditOp) -> u64 {
    match edit {
        VoxelEditOp::FillAabb { material, bounds }
        | VoxelEditOp::PaintMaterial { material, bounds } => {
            u64::from(material.get()) ^ i64_bits(bounds.min[0]) ^ i64_bits(bounds.max[0])
        }
        VoxelEditOp::CarveSphere {
            center,
            radius_voxels,
        } => {
            1 ^ (center.x as u32 as u64)
                ^ ((center.y as u32 as u64) << 11)
                ^ ((center.z as u32 as u64) << 22)
                ^ u64::from(radius_voxels)
        }
        VoxelEditOp::StampSdf {
            transform,
            sdf_asset,
            material,
        } => {
            2 ^ i64_bits(transform.bounds.min[0])
                ^ i64_bits(transform.bounds.max[0])
                ^ sdf_asset.asset_id.get()
                ^ u64::from(material.get())
        }
        VoxelEditOp::ApplyExplosion {
            center,
            radius_voxels,
            impulse_q,
        } => {
            3 ^ (center.x as u32 as u64)
                ^ ((center.y as u32 as u64) << 11)
                ^ ((center.z as u32 as u64) << 22)
                ^ u64::from(radius_voxels)
                ^ u64::from(impulse_q)
        }
        VoxelEditOp::RestoreFromSource { bounds } => {
            4 ^ i64_bits(bounds.min[0]) ^ i64_bits(bounds.min[1]) ^ i64_bits(bounds.min[2])
        }
    }
}

fn i64_bits(value: i64) -> u64 {
    value as u64
}

#[cfg(test)]
mod tests {
    use fun_scheduler_types::{EcsSpatialDomainKind, WorkRequiredness};

    use crate::{
        ArtifactRevision, EcsArtifactConsumer, EcsArtifactState, EcsDerivedArtifactKind,
        EcsDerivedArtifactRecord, EcsSpatialGridId, FunEcsResourceKind,
    };

    use super::*;

    fn page() -> EcsSpatialPageKey {
        EcsSpatialPageKey::new(
            EcsSpatialDomainKind::Terrain,
            EcsSpatialGridId::new(1),
            0,
            0,
            0,
            0,
            crate::EcsPageChannel::Surface,
        )
    }

    fn artifact(id: u64) -> EcsDerivedArtifactRecord {
        EcsDerivedArtifactRecord {
            artifact_id: EcsDerivedArtifactId::new(id),
            source_page: page(),
            kind: EcsDerivedArtifactKind::TerrainSurfacePackets,
            source_epoch: 1,
            source_digest: crate::derived_artifact_source_digest(page(), 1, 1),
            artifact_epoch: 1,
            state: EcsArtifactState::Ready,
            requiredness: WorkRequiredness::Required,
            consumer: EcsArtifactConsumer::Renderer,
        }
    }

    #[test]
    fn command_apply_reports_revisions_and_touched_artifact_keys() {
        let mut ledger = WorldRevisionLedger::default();
        let mut commands = EcsSpatialCommandBuffer::new(ledger.artifact_revision.revision());
        commands
            .push(EcsSpatialCommand::PublishArtifact(artifact(11)))
            .expect("publish artifact command");
        let mut registry = EcsDerivedArtifactRegistry::default();

        let report =
            apply_artifact_commands_with_revisions(&mut commands, &mut registry, &mut ledger)
                .expect("apply artifact commands");

        assert_eq!(report.applied, 1);
        assert_eq!(report.revision.previous_revision, FunRevision::INITIAL);
        assert_eq!(report.revision.new_revision, FunRevision::new(1));
        assert_eq!(
            report.revision.touched_resources,
            vec![FunResourceId::from_resource_kind(
                FunEcsResourceKind::DerivedArtifactRegistry
            )]
        );
        assert_eq!(
            report.revision.touched_tables,
            vec![FunResourceTableId::from_resource_kind(
                FunEcsResourceKind::DerivedArtifactRegistry
            )]
        );
        assert_eq!(
            report.revision.touched_artifact_keys,
            vec![FunArtifactId::new(11)]
        );
        assert_eq!(ledger.artifact_revision, ArtifactRevision::new(1));
        assert_eq!(ledger.resource_revisions.rows.len(), 1);
    }

    #[test]
    fn stale_command_buffers_reject_before_mutating_registries() {
        let mut ledger = WorldRevisionLedger::default();
        let mut first = EcsSpatialCommandBuffer::new(ledger.artifact_revision.revision());
        first
            .push(EcsSpatialCommand::PublishArtifact(artifact(1)))
            .expect("first command");
        let mut registry = EcsDerivedArtifactRegistry::default();
        apply_artifact_commands_with_revisions(&mut first, &mut registry, &mut ledger)
            .expect("first apply");

        let mut stale = EcsSpatialCommandBuffer::new(FunRevision::INITIAL);
        stale
            .push(EcsSpatialCommand::PublishArtifact(artifact(2)))
            .expect("stale command");
        let before = registry.clone();
        let err = apply_artifact_commands_with_revisions(&mut stale, &mut registry, &mut ledger)
            .expect_err("stale buffer rejects");

        assert_eq!(err, EcsSpatialValidationError::StaleCommandBuffer);
        assert_eq!(registry, before);
        assert_eq!(ledger.artifact_revision, ArtifactRevision::new(1));
    }
}
