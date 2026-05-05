use std::collections::BTreeMap;

use bevy::{
    prelude::*,
    render::render_resource::{Buffer, BufferAddress, CommandEncoder, ShaderType},
};
use thunder::prelude::*;

use crate::{
    FunUploadArena, FunUploadArenaWriteRequest, FunUploadBudgetClass, FunUploadBudgetDecision,
    FunUploadBudgetTracker, FunUploadFrameReportBuilder, FunUploadSubsystem, MaterialInstanceTint,
    PersistentBufferGrowthPolicy, StaticRenderBatch, StaticRenderCellId,
    StaticRenderGpuInstanceBufferHandle, UPLOAD_INSTANCE_DIRTY_RANGE,
    UPLOAD_WORLD_STREAM_STATIC_MESH, UploadArenaError, UploadWriteLabel,
    persistent_buffer_capacity_plan,
};

pub const FUN_INSTANCE_TABLE_SCHEMA_VERSION: u16 = 1;
pub const DYNAMIC_INSTANCE_RING_FRAMES: usize = 3;
pub const INSTANCE_SMALL_WRITE_MAX_BYTES: u64 = 64 * 1024;
pub const INSTANCE_INDEX_NONE: u32 = u32::MAX;

pub const INSTANCE_FLAG_STATIC: u32 = 1 << 0;
pub const INSTANCE_FLAG_DYNAMIC: u32 = 1 << 1;
pub const INSTANCE_FLAG_SHADOW: u32 = 1 << 2;
pub const INSTANCE_FLAG_RAY_PROXY: u32 = 1 << 3;
pub const INSTANCE_FLAG_MESHLET: u32 = 1 << 4;
pub const INSTANCE_FLAG_RASTER: u32 = 1 << 5;
pub const INSTANCE_FLAG_HAS_TINT: u32 = 1 << 6;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstanceId(pub u32);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstanceRange {
    pub start: u32,
    pub count: u32,
}

impl InstanceRange {
    pub const fn new(start: u32, count: u32) -> Self {
        Self { start, count }
    }

    pub const fn end(self) -> u32 {
        self.start.saturating_add(self.count)
    }

    pub const fn is_empty(self) -> bool {
        self.count == 0
    }

    pub const fn byte_offset(self) -> u64 {
        self.start as u64 * InstanceGpuRecord::SIZE_BYTES
    }

    pub const fn byte_len(self) -> u64 {
        self.count as u64 * InstanceGpuRecord::SIZE_BYTES
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InstanceDirtyReason {
    StreamCellResident,
    StreamCellSwapped,
    DestructionGeometryChanged,
    MaterialVariantChanged,
    LodResidencyChanged,
    DynamicSpawn,
    DynamicTransformChanged,
    DynamicClassChanged,
}

impl InstanceDirtyReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StreamCellResident => "stream_cell_resident",
            Self::StreamCellSwapped => "stream_cell_swapped",
            Self::DestructionGeometryChanged => "destruction_geometry_changed",
            Self::MaterialVariantChanged => "material_variant_changed",
            Self::LodResidencyChanged => "lod_residency_changed",
            Self::DynamicSpawn => "dynamic_spawn",
            Self::DynamicTransformChanged => "dynamic_transform_changed",
            Self::DynamicClassChanged => "dynamic_class_changed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstanceDirtyRange {
    pub range: InstanceRange,
    pub byte_offset: u64,
    pub byte_len: u64,
    pub reason: InstanceDirtyReason,
}

impl InstanceDirtyRange {
    pub const fn new(range: InstanceRange, reason: InstanceDirtyReason) -> Self {
        Self {
            range,
            byte_offset: range.byte_offset(),
            byte_len: range.byte_len(),
            reason,
        }
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, PartialEq, ShaderType)]
pub struct InstanceGpuRecord {
    pub transform: [[f32; 4]; 4],
    pub table_indices: [u32; 4],
    pub flags_lod: [u32; 4],
}

impl Default for InstanceGpuRecord {
    fn default() -> Self {
        Self {
            transform: Mat4::IDENTITY.to_cols_array_2d(),
            table_indices: [INSTANCE_INDEX_NONE, 0, 0, 0],
            flags_lod: [0; 4],
        }
    }
}

impl InstanceGpuRecord {
    pub const SIZE_BYTES: u64 = std::mem::size_of::<Self>() as u64;
    pub const ALIGN_BYTES: u64 = std::mem::align_of::<Self>() as u64;

    pub fn from_stream_spec(spec: &WorldEntitySpec, dynamic_class: DynamicInstanceClass) -> Self {
        let material_index = spec.catalog.map(|catalog| catalog.material_id).unwrap_or(0);
        let mesh_index = spec
            .catalog
            .map(|catalog| catalog.asset_id)
            .or_else(|| spec.render.map(stream_primitive_mesh_index))
            .unwrap_or(0);
        let tint = MaterialInstanceTint::from_packed_color(spec.color);
        let mut flags = INSTANCE_FLAG_DYNAMIC | dynamic_class.flag_bits();
        if !tint.is_neutral() {
            flags |= INSTANCE_FLAG_HAS_TINT;
        }
        Self::from_quantized_transform(
            spec.transform,
            [INSTANCE_INDEX_NONE, material_index, mesh_index, 0],
            [flags, 0, 0, tint.packed_rgba8()],
        )
    }

    pub fn from_static_batch_instance(batch: &StaticRenderBatch, instance_index: usize) -> Self {
        let transform = batch
            .transforms
            .get(instance_index)
            .copied()
            .unwrap_or_default();
        let mut flags = INSTANCE_FLAG_STATIC;
        if batch.key.shadow_participation {
            flags |= INSTANCE_FLAG_SHADOW;
        }
        if batch.key.solari_ray_proxy_participation {
            flags |= INSTANCE_FLAG_RAY_PROXY;
        }
        if batch.key.render_path.uses_meshlet() {
            flags |= INSTANCE_FLAG_MESHLET;
        } else if batch.key.render_path.emits_visible_raster() {
            flags |= INSTANCE_FLAG_RASTER;
        }
        let tint = batch
            .instance_tints
            .get(instance_index)
            .copied()
            .unwrap_or(MaterialInstanceTint::WHITE);
        if !tint.is_neutral() {
            flags |= INSTANCE_FLAG_HAS_TINT;
        }
        Self::from_quantized_transform(
            transform,
            [
                INSTANCE_INDEX_NONE,
                batch.key.material_key.material_preset_id.unwrap_or(0),
                batch.key.catalog_asset_id,
                instance_index.min(INSTANCE_INDEX_NONE as usize) as u32,
            ],
            [
                flags,
                0,
                fun_geometry_class_bits(batch.key.geometry_class),
                tint.packed_rgba8(),
            ],
        )
    }

    pub fn update_transform(
        &mut self,
        transform: QuantizedTransform3,
        previous_transform_index: u32,
    ) {
        self.transform = matrix_from_quantized(transform).to_cols_array_2d();
        self.table_indices[0] = previous_transform_index;
    }

    fn from_quantized_transform(
        transform: QuantizedTransform3,
        table_indices: [u32; 4],
        flags_lod: [u32; 4],
    ) -> Self {
        Self {
            transform: matrix_from_quantized(transform).to_cols_array_2d(),
            table_indices,
            flags_lod,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DynamicInstanceClass {
    Character,
    Vehicle,
    Projectile,
    PhysicsProp,
}

impl DynamicInstanceClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Character => "character",
            Self::Vehicle => "vehicle",
            Self::Projectile => "projectile",
            Self::PhysicsProp => "physics_prop",
        }
    }

    pub const fn flag_bits(self) -> u32 {
        match self {
            Self::Character => 1 << 8,
            Self::Vehicle => 1 << 9,
            Self::Projectile => 1 << 10,
            Self::PhysicsProp => 1 << 11,
        }
    }

    pub const fn from_replication_class(class: ReplicationClass) -> Option<Self> {
        match class {
            ReplicationClass::Pawn => Some(Self::Character),
            ReplicationClass::Vehicle => Some(Self::Vehicle),
            ReplicationClass::Projectile => Some(Self::Projectile),
            ReplicationClass::Destructible
            | ReplicationClass::Objective
            | ReplicationClass::Custom(_) => Some(Self::PhysicsProp),
            ReplicationClass::World => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceTableKind {
    Static,
    Dynamic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceUploadPath {
    Noop,
    UseExistingEncoderWithUploadArena,
    PersistentBufferDirtyRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstanceUploadPlan {
    pub table: InstanceTableKind,
    pub path: InstanceUploadPath,
    pub label: UploadWriteLabel,
    pub subsystem: FunUploadSubsystem,
    pub budget_class: FunUploadBudgetClass,
    pub range: InstanceRange,
    pub bytes: u64,
    pub requires_existing_encoder: bool,
    pub creates_ad_hoc_encoder: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticInstanceAllocation {
    pub allocation_index: u32,
    pub cell_id: StaticRenderCellId,
    pub range: InstanceRange,
    pub gpu_buffer_handle: Option<StaticRenderGpuInstanceBufferHandle>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StaticInstanceAllocationRecord {
    pub cell_id: StaticRenderCellId,
    pub range: InstanceRange,
    pub uploaded_once: bool,
    pub batch_key_asset_id: u32,
}

#[derive(Debug, Clone, Resource, Default)]
pub struct StaticInstanceTable {
    records: Vec<InstanceGpuRecord>,
    capacity_records: u32,
    free_ranges: Vec<InstanceRange>,
    allocations: Vec<StaticInstanceAllocationRecord>,
    dirty_ranges: Vec<InstanceDirtyRange>,
    generation: u64,
    uploaded_generation: u64,
    gpu_buffer_handle: Option<StaticRenderGpuInstanceBufferHandle>,
}

impl StaticInstanceTable {
    pub fn clear_for_world_swap(&mut self) {
        self.records.clear();
        self.allocations.clear();
        self.free_ranges.clear();
        self.dirty_ranges.clear();
        self.generation = self.generation.saturating_add(1);
        self.uploaded_generation = 0;
        self.gpu_buffer_handle = None;
    }

    pub fn register_static_batch(&mut self, batch: &StaticRenderBatch) -> StaticInstanceAllocation {
        let count = batch.instance_count().min(u32::MAX as usize) as u32;
        let range = self.allocate_static_range(count);
        let start = range.start as usize;
        let end = range.end() as usize;
        if self.records.len() < end {
            self.records.resize(end, InstanceGpuRecord::default());
        }
        for (instance_index, record) in self.records[start..end].iter_mut().enumerate() {
            *record = InstanceGpuRecord::from_static_batch_instance(batch, instance_index);
        }
        let allocation_index = self.allocations.len().min(u32::MAX as usize) as u32;
        self.allocations.push(StaticInstanceAllocationRecord {
            cell_id: batch.cell_id,
            range,
            uploaded_once: false,
            batch_key_asset_id: batch.key.catalog_asset_id,
        });
        self.record_dirty(InstanceDirtyRange::new(
            range,
            InstanceDirtyReason::StreamCellResident,
        ));
        self.generation = self.generation.saturating_add(1);
        StaticInstanceAllocation {
            allocation_index,
            cell_id: batch.cell_id,
            range,
            gpu_buffer_handle: self.gpu_buffer_handle,
        }
    }

    pub fn mark_cell_swapped(&mut self, cell_id: StaticRenderCellId) {
        self.mark_cell_dirty(cell_id, InstanceDirtyReason::StreamCellSwapped);
    }

    pub fn mark_destruction_geometry_changed(&mut self, cell_id: StaticRenderCellId) {
        self.mark_cell_dirty(cell_id, InstanceDirtyReason::DestructionGeometryChanged);
    }

    pub fn mark_material_variant_changed(&mut self, range: InstanceRange) {
        self.record_dirty(InstanceDirtyRange::new(
            range,
            InstanceDirtyReason::MaterialVariantChanged,
        ));
    }

    pub fn mark_lod_residency_changed(&mut self, range: InstanceRange) {
        self.record_dirty(InstanceDirtyRange::new(
            range,
            InstanceDirtyReason::LodResidencyChanged,
        ));
    }

    pub fn release_cell(&mut self, cell_id: StaticRenderCellId) -> u32 {
        let mut released_ranges = Vec::new();
        self.allocations.retain(|allocation| {
            if allocation.cell_id == cell_id {
                released_ranges.push(allocation.range);
                false
            } else {
                true
            }
        });

        let released_instances = released_ranges
            .iter()
            .map(|range| range.count)
            .fold(0u32, u32::saturating_add);
        for range in released_ranges {
            self.clear_records(range);
            self.push_free_range(range);
        }
        if released_instances != 0 {
            self.generation = self.generation.saturating_add(1);
        }
        released_instances
    }

    pub fn complete_uploads(&mut self, handle: StaticRenderGpuInstanceBufferHandle) {
        self.gpu_buffer_handle = Some(handle);
        for allocation in &mut self.allocations {
            allocation.uploaded_once = true;
        }
        self.dirty_ranges.clear();
        self.uploaded_generation = self.generation;
    }

    pub fn records(&self) -> &[InstanceGpuRecord] {
        &self.records
    }

    pub fn allocations(&self) -> &[StaticInstanceAllocationRecord] {
        &self.allocations
    }

    pub fn dirty_ranges(&self) -> &[InstanceDirtyRange] {
        &self.dirty_ranges
    }

    pub fn free_ranges(&self) -> &[InstanceRange] {
        &self.free_ranges
    }

    pub fn pending_upload_plans(&self) -> Vec<InstanceUploadPlan> {
        self.dirty_ranges
            .iter()
            .copied()
            .map(static_instance_upload_plan)
            .collect()
    }

    pub fn total_bytes(&self) -> u64 {
        self.records.len() as u64 * InstanceGpuRecord::SIZE_BYTES
    }

    pub fn capacity_records(&self) -> u32 {
        self.capacity_records
    }

    pub fn allocated_buffer_bytes(&self) -> u64 {
        self.capacity_records as u64 * InstanceGpuRecord::SIZE_BYTES
    }

    pub fn uploaded_generation(&self) -> u64 {
        self.uploaded_generation
    }

    fn allocate_static_range(&mut self, count: u32) -> InstanceRange {
        if count == 0 {
            return InstanceRange::default();
        }
        if let Some(range) = self.reuse_free_range(count) {
            return range;
        }

        let start = self.records.len().min(u32::MAX as usize) as u32;
        let range = InstanceRange::new(start, count);
        let capacity_plan = persistent_buffer_capacity_plan(
            self.capacity_records,
            range.end(),
            PersistentBufferGrowthPolicy::PowerOfTwo,
        );
        self.capacity_records = capacity_plan.next_capacity_records;
        range
    }

    fn reuse_free_range(&mut self, count: u32) -> Option<InstanceRange> {
        let index = self
            .free_ranges
            .iter()
            .position(|range| range.count >= count)?;
        let free = self.free_ranges[index];
        let allocated = InstanceRange::new(free.start, count);
        if free.count == count {
            self.free_ranges.remove(index);
        } else {
            self.free_ranges[index] = InstanceRange::new(
                free.start.saturating_add(count),
                free.count.saturating_sub(count),
            );
        }
        Some(allocated)
    }

    fn push_free_range(&mut self, range: InstanceRange) {
        if range.is_empty() {
            return;
        }
        self.free_ranges.push(range);
        self.free_ranges.sort_by_key(|range| range.start);
        let mut merged = Vec::<InstanceRange>::with_capacity(self.free_ranges.len());
        for range in self.free_ranges.drain(..) {
            if let Some(last) = merged.last_mut()
                && last.end() >= range.start
            {
                let end = last.end().max(range.end());
                last.count = end.saturating_sub(last.start);
                continue;
            }
            merged.push(range);
        }
        self.free_ranges = merged;
    }

    fn clear_records(&mut self, range: InstanceRange) {
        let start = range.start as usize;
        let end = range
            .end()
            .min(self.records.len().min(u32::MAX as usize) as u32) as usize;
        for record in &mut self.records[start..end] {
            *record = InstanceGpuRecord::default();
        }
    }

    fn mark_cell_dirty(&mut self, cell_id: StaticRenderCellId, reason: InstanceDirtyReason) {
        let ranges = self
            .allocations
            .iter()
            .filter(|allocation| allocation.cell_id == cell_id)
            .map(|allocation| allocation.range)
            .collect::<Vec<_>>();
        for range in ranges {
            self.record_dirty(InstanceDirtyRange::new(range, reason));
        }
    }

    fn record_dirty(&mut self, dirty: InstanceDirtyRange) {
        push_dirty_range(&mut self.dirty_ranges, dirty);
    }
}

#[derive(Debug, Clone, Default, Resource)]
pub struct DynamicInstanceTable {
    frame_index: u64,
    ring_slot: u8,
    records: Vec<InstanceGpuRecord>,
    entity_to_instance: BTreeMap<NetEntity, InstanceId>,
    classes: Vec<DynamicInstanceClass>,
    dirty_ranges_by_class: BTreeMap<DynamicInstanceClass, Vec<InstanceDirtyRange>>,
}

impl DynamicInstanceTable {
    pub fn clear_for_world_swap(&mut self) {
        self.records.clear();
        self.entity_to_instance.clear();
        self.classes.clear();
        self.dirty_ranges_by_class.clear();
        self.frame_index = 0;
        self.ring_slot = 0;
    }

    pub fn begin_frame(&mut self, frame_index: u64) {
        self.frame_index = frame_index;
        self.ring_slot = (frame_index as usize % DYNAMIC_INSTANCE_RING_FRAMES) as u8;
        self.dirty_ranges_by_class.clear();
    }

    pub fn upsert_instance(
        &mut self,
        entity: NetEntity,
        class: DynamicInstanceClass,
        record: InstanceGpuRecord,
    ) -> InstanceId {
        if let Some(instance_id) = self.entity_to_instance.get(&entity).copied() {
            let index = instance_id.0 as usize;
            if let Some(existing) = self.records.get_mut(index) {
                *existing = record;
            }
            if let Some(existing_class) = self.classes.get_mut(index) {
                let reason = if *existing_class == class {
                    InstanceDirtyReason::DynamicTransformChanged
                } else {
                    *existing_class = class;
                    InstanceDirtyReason::DynamicClassChanged
                };
                self.record_dirty(class, InstanceRange::new(instance_id.0, 1), reason);
            }
            return instance_id;
        }

        let instance_id = InstanceId(self.records.len().min(u32::MAX as usize) as u32);
        self.records.push(record);
        self.classes.push(class);
        self.entity_to_instance.insert(entity, instance_id);
        self.record_dirty(
            class,
            InstanceRange::new(instance_id.0, 1),
            InstanceDirtyReason::DynamicSpawn,
        );
        instance_id
    }

    pub fn mark_transform_dirty(
        &mut self,
        entity: NetEntity,
        transform: QuantizedTransform3,
    ) -> Option<InstanceDirtyRange> {
        let instance_id = self.entity_to_instance.get(&entity).copied()?;
        let index = instance_id.0 as usize;
        let class = *self.classes.get(index)?;
        let record = self.records.get_mut(index)?;
        record.update_transform(transform, instance_id.0);
        let dirty = InstanceDirtyRange::new(
            InstanceRange::new(instance_id.0, 1),
            InstanceDirtyReason::DynamicTransformChanged,
        );
        self.record_dirty(class, dirty.range, dirty.reason);
        Some(dirty)
    }

    pub fn instance_id(&self, entity: NetEntity) -> Option<InstanceId> {
        self.entity_to_instance.get(&entity).copied()
    }

    pub fn records(&self) -> &[InstanceGpuRecord] {
        &self.records
    }

    pub fn dirty_ranges_for_class(&self, class: DynamicInstanceClass) -> &[InstanceDirtyRange] {
        self.dirty_ranges_by_class
            .get(&class)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn pending_upload_plans(&self) -> Vec<InstanceUploadPlan> {
        self.dirty_ranges_by_class
            .values()
            .flat_map(|ranges| ranges.iter().copied())
            .map(dynamic_instance_upload_plan)
            .collect()
    }

    pub const fn frame_index(&self) -> u64 {
        self.frame_index
    }

    pub const fn ring_slot(&self) -> u8 {
        self.ring_slot
    }

    fn record_dirty(
        &mut self,
        class: DynamicInstanceClass,
        range: InstanceRange,
        reason: InstanceDirtyReason,
    ) {
        push_dirty_range(
            self.dirty_ranges_by_class.entry(class).or_default(),
            InstanceDirtyRange::new(range, reason),
        );
    }
}

pub struct InstanceTableArenaWriteRequest<'a> {
    pub plan: InstanceUploadPlan,
    pub encoder: &'a mut CommandEncoder,
    pub target: &'a Buffer,
    pub data: &'a [u8],
    pub buffer_size: u64,
    pub buffer_created_or_resized: bool,
}

pub fn write_instance_range_with_existing_encoder(
    arena: &mut FunUploadArena,
    budget: &mut FunUploadBudgetTracker,
    report: &mut FunUploadFrameReportBuilder,
    request: InstanceTableArenaWriteRequest<'_>,
) -> Result<FunUploadBudgetDecision, UploadArenaError> {
    arena.write_buffer_budgeted(
        budget,
        report,
        FunUploadArenaWriteRequest {
            label: request.plan.label,
            subsystem: request.plan.subsystem,
            budget_class: request.plan.budget_class,
            encoder: request.encoder,
            target: request.target,
            offset: request.plan.range.byte_offset() as BufferAddress,
            data: request.data,
            buffer_size: request.buffer_size,
            dirty_bytes: request.plan.bytes,
            buffer_created_or_resized: request.buffer_created_or_resized,
        },
    )
}

fn static_instance_upload_plan(dirty: InstanceDirtyRange) -> InstanceUploadPlan {
    InstanceUploadPlan {
        table: InstanceTableKind::Static,
        path: InstanceUploadPath::PersistentBufferDirtyRange,
        label: UPLOAD_WORLD_STREAM_STATIC_MESH,
        subsystem: FunUploadSubsystem::WorldStream,
        budget_class: FunUploadBudgetClass::LargeBuffer,
        range: dirty.range,
        bytes: dirty.byte_len,
        requires_existing_encoder: false,
        creates_ad_hoc_encoder: false,
    }
}

fn dynamic_instance_upload_plan(dirty: InstanceDirtyRange) -> InstanceUploadPlan {
    let path = if dirty.byte_len <= INSTANCE_SMALL_WRITE_MAX_BYTES {
        InstanceUploadPath::UseExistingEncoderWithUploadArena
    } else {
        InstanceUploadPath::PersistentBufferDirtyRange
    };
    InstanceUploadPlan {
        table: InstanceTableKind::Dynamic,
        path,
        label: UPLOAD_INSTANCE_DIRTY_RANGE,
        subsystem: FunUploadSubsystem::MeshletInstance,
        budget_class: if path == InstanceUploadPath::UseExistingEncoderWithUploadArena {
            FunUploadBudgetClass::SmallBuffer
        } else {
            FunUploadBudgetClass::LargeBuffer
        },
        range: dirty.range,
        bytes: dirty.byte_len,
        requires_existing_encoder: path == InstanceUploadPath::UseExistingEncoderWithUploadArena,
        creates_ad_hoc_encoder: false,
    }
}

fn push_dirty_range(ranges: &mut Vec<InstanceDirtyRange>, dirty: InstanceDirtyRange) {
    if dirty.range.is_empty() {
        return;
    }
    if let Some(last) = ranges.last_mut()
        && last.reason == dirty.reason
        && last.range.end() == dirty.range.start
    {
        last.range.count = last.range.count.saturating_add(dirty.range.count);
        last.byte_len = last.range.byte_len();
        return;
    }
    ranges.push(dirty);
}

fn matrix_from_quantized(transform: QuantizedTransform3) -> Mat4 {
    let rotation = transform.rotation.to_f32();
    Mat4::from_rotation_translation(
        Quat::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3]),
        Vec3::from_array(transform.translation.to_f32(Quantization::MILLIMETERS)),
    )
}

const fn stream_primitive_mesh_index(primitive: WorldPrimitive) -> u32 {
    match primitive {
        WorldPrimitive::Plane { .. } => 0x8000_0001,
        WorldPrimitive::Cuboid { .. } => 0x8000_0002,
    }
}

const fn fun_geometry_class_bits(class: crate::FunGeometryClass) -> u32 {
    match class {
        crate::FunGeometryClass::StaticOpaqueDense => 1,
        crate::FunGeometryClass::StaticOpaqueSimple => 2,
        crate::FunGeometryClass::DynamicOpaque => 3,
        crate::FunGeometryClass::SkinnedCharacter => 4,
        crate::FunGeometryClass::Vehicle => 5,
        crate::FunGeometryClass::Foliage => 6,
        crate::FunGeometryClass::Particle => 7,
        crate::FunGeometryClass::Decal => 8,
        crate::FunGeometryClass::Viewmodel => 9,
        crate::FunGeometryClass::Ui => 10,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ClientOpaqueRenderer, FunGeometryClass, FunMeshletFormat, FunRenderPath,
        FunRenderPhaseKind, MaterialInstanceTint, MaterialKey, RenderBatchKey,
        RenderBatchShadowMode, RenderGeometryClass, SIMPLE_OPAQUE_STATIC_PIPELINE,
        StaticRenderBatchBounds, StaticRenderBatchKey, TextureTableId, material_signature_for_key,
    };

    fn static_batch(instance_count: u32) -> StaticRenderBatch {
        StaticRenderBatch {
            cell_id: StaticRenderCellId(3),
            key: StaticRenderBatchKey {
                catalog_asset_id: 11,
                mesh_handle_hash: 101,
                meshlet_handle_hash: 0,
                material_key: MaterialKey::from_rgba8([1, 2, 3, 255]),
                material_handle_hash: 202,
                render_batch_key: RenderBatchKey {
                    pipeline_signature: SIMPLE_OPAQUE_STATIC_PIPELINE,
                    material_signature: material_signature_for_key(MaterialKey::from_rgba8([
                        1, 2, 3, 255,
                    ])),
                    mesh_format: FunMeshletFormat::RasterMesh,
                    geometry_class: RenderGeometryClass::SimpleRaster,
                    pass_type: FunRenderPhaseKind::MainOpaque,
                    shadow_mode: RenderBatchShadowMode::CastsAndReceives,
                    texture_table_id: TextureTableId(0),
                },
                geometry_class: FunGeometryClass::StaticOpaqueSimple,
                render_geometry_class: RenderGeometryClass::SimpleRaster,
                render_path: FunRenderPath::StandardRaster,
                shadow_participation: true,
                solari_ray_proxy_participation: true,
                opaque_renderer: ClientOpaqueRenderer::Deferred,
            },
            transforms: (0..instance_count)
                .map(|index| QuantizedTransform3 {
                    translation: QuantizedVec3::from_f32(
                        [index as f32, 0.0, 0.0],
                        Quantization::MILLIMETERS,
                    ),
                    rotation: Default::default(),
                })
                .collect(),
            instance_tints: vec![MaterialInstanceTint::WHITE; instance_count as usize],
            instance_bounds: vec![StaticRenderBatchBounds::default(); instance_count as usize],
            instance_entities: (0..instance_count)
                .map(|index| NetEntity(index as u64 + 1))
                .collect(),
            aggregate_bounds: StaticRenderBatchBounds::default(),
            mesh: None,
            meshlet_mesh: None,
            material: None,
            ray_proxy: None,
            gpu_instance_buffer_handle: None,
            instance_range: None,
        }
    }

    fn dynamic_spec(entity: u64, class: ReplicationClass) -> WorldEntitySpec {
        WorldEntitySpec {
            entity: NetEntity(entity),
            name: String::from("dynamic"),
            class,
            authority: AuthorityMode::ServerOnly,
            transform: QuantizedTransform3 {
                translation: QuantizedVec3::from_f32([0.0, 0.0, 0.0], Quantization::MILLIMETERS),
                rotation: Default::default(),
            },
            catalog: None,
            render: Some(WorldPrimitive::Cuboid {
                size: QuantizedVec3::from_f32([1.0, 1.0, 1.0], Quantization::MILLIMETERS),
            }),
            collider: None,
            color: None,
        }
    }

    #[test]
    fn instance_gpu_record_layout_is_aligned_and_stable() {
        assert_eq!(InstanceGpuRecord::SIZE_BYTES, 96);
        assert_eq!(InstanceGpuRecord::ALIGN_BYTES, 16);
        assert_eq!(
            InstanceGpuRecord::default().table_indices[0],
            INSTANCE_INDEX_NONE
        );
    }

    #[test]
    fn static_instance_table_uploads_static_batch_once() {
        let mut table = StaticInstanceTable::default();
        let allocation = table.register_static_batch(&static_batch(4));

        assert_eq!(allocation.range, InstanceRange::new(0, 4));
        assert_eq!(table.records().len(), 4);
        assert_eq!(table.dirty_ranges().len(), 1);
        assert_eq!(
            table.pending_upload_plans()[0].path,
            InstanceUploadPath::PersistentBufferDirtyRange
        );

        table.complete_uploads(StaticRenderGpuInstanceBufferHandle(77));
        assert!(table.dirty_ranges().is_empty());
        assert_eq!(table.uploaded_generation(), 1);
    }

    #[test]
    fn static_instance_table_does_not_dirty_clean_static_instances_per_frame() {
        let mut table = StaticInstanceTable::default();
        table.register_static_batch(&static_batch(2));
        table.complete_uploads(StaticRenderGpuInstanceBufferHandle(1));

        assert!(table.dirty_ranges().is_empty());
        assert_eq!(table.total_bytes(), InstanceGpuRecord::SIZE_BYTES * 2);
    }

    #[test]
    fn static_instance_table_grows_capacity_without_shrinking() {
        let mut table = StaticInstanceTable::default();
        table.register_static_batch(&static_batch(3));
        let first_capacity = table.capacity_records();
        table.clear_for_world_swap();
        table.register_static_batch(&static_batch(1));

        assert_eq!(first_capacity, 4);
        assert_eq!(table.capacity_records(), first_capacity);
        assert_eq!(
            table.allocated_buffer_bytes(),
            first_capacity as u64 * InstanceGpuRecord::SIZE_BYTES
        );
    }

    #[test]
    fn static_instance_table_reuses_freed_ranges() {
        let mut table = StaticInstanceTable::default();
        let first = table.register_static_batch(&static_batch(4));
        table.complete_uploads(StaticRenderGpuInstanceBufferHandle(1));

        assert_eq!(table.release_cell(StaticRenderCellId(3)), 4);
        assert_eq!(table.free_ranges(), &[InstanceRange::new(0, 4)]);

        let second = table.register_static_batch(&static_batch(2));

        assert_eq!(second.range, InstanceRange::new(0, 2));
        assert_eq!(table.free_ranges(), &[InstanceRange::new(2, 2)]);
        assert_eq!(table.capacity_records(), 4);
        assert_eq!(first.range.start, second.range.start);
    }

    #[test]
    fn static_instance_table_marks_cell_swap_dirty() {
        let mut table = StaticInstanceTable::default();
        table.register_static_batch(&static_batch(2));
        table.complete_uploads(StaticRenderGpuInstanceBufferHandle(1));

        table.mark_cell_swapped(StaticRenderCellId(3));

        assert_eq!(table.dirty_ranges().len(), 1);
        assert_eq!(
            table.dirty_ranges()[0].reason,
            InstanceDirtyReason::StreamCellSwapped
        );
    }

    #[test]
    fn dynamic_instance_table_merges_adjacent_dirty_ranges_by_class() {
        let mut table = DynamicInstanceTable::default();
        for entity in 1..=3 {
            let spec = dynamic_spec(entity, ReplicationClass::Pawn);
            let class = DynamicInstanceClass::from_replication_class(spec.class).unwrap();
            table.upsert_instance(
                NetEntity(entity),
                class,
                InstanceGpuRecord::from_stream_spec(&spec, class),
            );
        }

        let ranges = table.dirty_ranges_for_class(DynamicInstanceClass::Character);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].range, InstanceRange::new(0, 3));
    }

    #[test]
    fn instance_gpu_record_packs_dynamic_tint_without_unique_material() {
        let mut spec = dynamic_spec(1, ReplicationClass::Vehicle);
        spec.color = Some(PackedColorRgba8::srgb(9, 8, 7));
        let class = DynamicInstanceClass::from_replication_class(spec.class).unwrap();

        let record = InstanceGpuRecord::from_stream_spec(&spec, class);

        assert_ne!(record.flags_lod[0] & INSTANCE_FLAG_HAS_TINT, 0);
        assert_eq!(
            record.flags_lod[3],
            MaterialInstanceTint::from_rgba8([9, 8, 7, 255]).packed_rgba8()
        );
    }

    #[test]
    fn static_instance_record_packs_batch_instance_tint() {
        let mut batch = static_batch(2);
        batch.instance_tints[1] = MaterialInstanceTint::from_rgba8([1, 2, 3, 255]);

        let record = InstanceGpuRecord::from_static_batch_instance(&batch, 1);

        assert_ne!(record.flags_lod[0] & INSTANCE_FLAG_HAS_TINT, 0);
        assert_eq!(
            record.flags_lod[3],
            MaterialInstanceTint::from_rgba8([1, 2, 3, 255]).packed_rgba8()
        );
    }

    #[test]
    fn dynamic_instance_table_updates_only_changed_transform_range() {
        let mut table = DynamicInstanceTable::default();
        for entity in 1..=4 {
            let spec = dynamic_spec(entity, ReplicationClass::Vehicle);
            let class = DynamicInstanceClass::from_replication_class(spec.class).unwrap();
            table.upsert_instance(
                NetEntity(entity),
                class,
                InstanceGpuRecord::from_stream_spec(&spec, class),
            );
        }
        table.begin_frame(9);

        let changed = table
            .mark_transform_dirty(
                NetEntity(3),
                QuantizedTransform3 {
                    translation: QuantizedVec3::from_f32(
                        [3.0, 0.0, 0.0],
                        Quantization::MILLIMETERS,
                    ),
                    rotation: Default::default(),
                },
            )
            .expect("registered dynamic entity should mark dirty");

        assert_eq!(changed.range, InstanceRange::new(2, 1));
        let ranges = table.dirty_ranges_for_class(DynamicInstanceClass::Vehicle);
        assert_eq!(ranges, &[changed]);
        assert_eq!(table.ring_slot(), 0);
    }

    #[test]
    fn small_dynamic_dirty_ranges_use_upload_arena_without_creating_encoders() {
        let dirty = InstanceDirtyRange::new(
            InstanceRange::new(4, 2),
            InstanceDirtyReason::DynamicTransformChanged,
        );
        let plan = dynamic_instance_upload_plan(dirty);

        assert_eq!(
            plan.path,
            InstanceUploadPath::UseExistingEncoderWithUploadArena
        );
        assert!(plan.requires_existing_encoder);
        assert!(!plan.creates_ad_hoc_encoder);
        assert_eq!(plan.label, UPLOAD_INSTANCE_DIRTY_RANGE);
    }

    #[test]
    fn large_instance_dirty_ranges_use_persistent_buffer_updates() {
        let count = (INSTANCE_SMALL_WRITE_MAX_BYTES / InstanceGpuRecord::SIZE_BYTES + 1) as u32;
        let dirty = InstanceDirtyRange::new(
            InstanceRange::new(0, count),
            InstanceDirtyReason::DynamicTransformChanged,
        );
        let plan = dynamic_instance_upload_plan(dirty);

        assert_eq!(plan.path, InstanceUploadPath::PersistentBufferDirtyRange);
        assert!(!plan.creates_ad_hoc_encoder);
        assert_eq!(plan.budget_class, FunUploadBudgetClass::LargeBuffer);
    }
}
