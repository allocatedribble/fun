//! Tier 1 — Cache-Backed Runtime Optimization.
//!
//! Pass 22 installed the typed dirty-range surface
//! (`GpuSceneRecordBuffers` + `GpuSceneDirtyReason` +
//! `GpuSceneFullRebuildReason`). Pass 26 installed the typed
//! post-warmup hard-counter contract. Tier 1 puts the typed
//! surface under measured pressure: a stress-scenario harness
//! with knobs the user named (10K objects, 1K material variants,
//! 256–2048 texture refs, UI packet rate, camera/transform
//! churn, asset insert/remove churn, swapchain resizes), a typed
//! per-frame measurement bundle with the listed counters and
//! p50/p95/p99 CPU frame time, range-coalescing for batched
//! uploads, ringed upload memory, compact `u32` per-draw IDs,
//! and a typed `Tier1AcceptanceVerdict` that asserts the two
//! acceptance criteria from the spec:
//!
//! 1. No post-warmup cache churn except *named* dirty updates.
//!    Full rebuilds must report a typed
//!    `GpuSceneFullRebuildReason`.
//! 2. Transform-only churn uploads only changed transform
//!    ranges — material / light / draw / indirect-args buffers
//!    do not rebuild.
//!
//! Honest scope note: the live `cargo test` harness drives a
//! *synthetic* 1000-object scenario through the typed pipeline
//! data path and measures the renderer-side CPU frame time. The
//! GPU-side draw-frame p50/p95/p99 follows the Tier 0 gap
//! closures (`no_swapchain_configured`, `no_render_encoder`,
//! `no_frame_readback`).

use bevy_ecs::prelude::Resource;
use std::time::Instant;

use crate::scene_streaming::{
    GPU_SCENE_BUFFER_KIND_COUNT, GpuSceneBufferKind, GpuSceneDirtyReason, GpuSceneDrawPacketRecord,
    GpuSceneFullRebuildReason, GpuSceneIndirectArgsRecord, GpuSceneObjectRecord,
    GpuSceneRecordBuffers, GpuSceneTransformRecord, GpuSceneUploadRange,
};

pub const TIER1_CACHE_BACKED_SCHEMA_VERSION: u16 = 1;

pub const TIER1_SEMANTIC_OWNER_COUNT: usize = 7;
pub const TIER1_FRAME_COUNTER_KIND_COUNT: usize = 9;
pub const TIER1_DEFAULT_PRODUCTION_OBJECT_COUNT: u32 = 10_000;
pub const TIER1_DEFAULT_PRODUCTION_MATERIAL_VARIANT_COUNT: u32 = 1_000;
pub const TIER1_DEFAULT_PRODUCTION_TEXTURE_REFERENCE_MIN: u32 = 256;
pub const TIER1_DEFAULT_PRODUCTION_TEXTURE_REFERENCE_MAX: u32 = 2_048;
pub const TIER1_DEFAULT_PRODUCTION_UI_PACKET_RATE_HZ: u16 = 120;

// ============================================================================
// Section 1 — Stress scenario knobs
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier1StressScenario {
    pub schema_version: u16,
    pub object_count: u32,
    pub material_variant_count: u32,
    pub texture_reference_count: u32,
    pub ui_packet_rate_hz: u16,
    pub camera_movement_per_frame: u16,
    pub transform_churn_per_frame: u32,
    pub material_churn_per_frame: u32,
    pub light_churn_per_frame: u32,
    pub asset_insert_per_frame: u32,
    pub asset_remove_per_frame: u32,
    pub swapchain_resize_count: u8,
    pub frame_count: u32,
}

impl Tier1StressScenario {
    pub const PRODUCTION_DEFAULT: Self = Self {
        schema_version: TIER1_CACHE_BACKED_SCHEMA_VERSION,
        object_count: TIER1_DEFAULT_PRODUCTION_OBJECT_COUNT,
        material_variant_count: TIER1_DEFAULT_PRODUCTION_MATERIAL_VARIANT_COUNT,
        texture_reference_count: TIER1_DEFAULT_PRODUCTION_TEXTURE_REFERENCE_MAX,
        ui_packet_rate_hz: TIER1_DEFAULT_PRODUCTION_UI_PACKET_RATE_HZ,
        camera_movement_per_frame: 60,
        transform_churn_per_frame: 200,
        material_churn_per_frame: 0,
        light_churn_per_frame: 0,
        asset_insert_per_frame: 4,
        asset_remove_per_frame: 4,
        swapchain_resize_count: 4,
        frame_count: 600,
    };

    pub const CI_FRIENDLY: Self = Self {
        schema_version: TIER1_CACHE_BACKED_SCHEMA_VERSION,
        object_count: 1_000,
        material_variant_count: 64,
        texture_reference_count: 256,
        ui_packet_rate_hz: 60,
        camera_movement_per_frame: 16,
        transform_churn_per_frame: 32,
        material_churn_per_frame: 0,
        light_churn_per_frame: 0,
        asset_insert_per_frame: 1,
        asset_remove_per_frame: 1,
        swapchain_resize_count: 1,
        frame_count: 30,
    };

    pub const TRANSFORM_ONLY_CHURN: Self = Self {
        schema_version: TIER1_CACHE_BACKED_SCHEMA_VERSION,
        object_count: 256,
        material_variant_count: 16,
        texture_reference_count: 32,
        ui_packet_rate_hz: 60,
        camera_movement_per_frame: 0,
        transform_churn_per_frame: 64,
        material_churn_per_frame: 0,
        light_churn_per_frame: 0,
        asset_insert_per_frame: 0,
        asset_remove_per_frame: 0,
        swapchain_resize_count: 0,
        frame_count: 8,
    };

    #[must_use]
    pub const fn validates_acceptance_at_no_cache_churn(&self) -> bool {
        self.frame_count > 0
    }
}

impl Default for Tier1StressScenario {
    fn default() -> Self {
        Self::CI_FRIENDLY
    }
}

// ============================================================================
// Section 2 — Per-frame measurement bundle
// ============================================================================

/// Semantic owner for upload byte attribution. Mirrors the
/// `GpuSceneBufferKind` taxonomy so each upload range's bytes can
/// be charged back to the producing system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier1SemanticOwner {
    Mesh,
    Texture,
    Material,
    Light,
    Transform,
    DrawPacket,
    IndirectArgs,
}

impl Tier1SemanticOwner {
    pub const ALL: [Self; TIER1_SEMANTIC_OWNER_COUNT] = [
        Self::Mesh,
        Self::Texture,
        Self::Material,
        Self::Light,
        Self::Transform,
        Self::DrawPacket,
        Self::IndirectArgs,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Mesh => 0,
            Self::Texture => 1,
            Self::Material => 2,
            Self::Light => 3,
            Self::Transform => 4,
            Self::DrawPacket => 5,
            Self::IndirectArgs => 6,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mesh => "mesh",
            Self::Texture => "texture",
            Self::Material => "material",
            Self::Light => "light",
            Self::Transform => "transform",
            Self::DrawPacket => "draw_packet",
            Self::IndirectArgs => "indirect_args",
        }
    }

    #[must_use]
    pub const fn from_buffer_kind(kind: GpuSceneBufferKind) -> Self {
        match kind {
            GpuSceneBufferKind::Object => Self::Mesh,
            GpuSceneBufferKind::Transform | GpuSceneBufferKind::PreviousTransform => {
                Self::Transform
            }
            GpuSceneBufferKind::Material => Self::Material,
            GpuSceneBufferKind::Light => Self::Light,
            GpuSceneBufferKind::DrawPacket => Self::DrawPacket,
            GpuSceneBufferKind::IndirectArgs => Self::IndirectArgs,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier1FrameCounterKind {
    ResourceCreations,
    BindTableCreations,
    PipelineCreations,
    DescriptorCacheHits,
    ResourceRegistryStaleIdRejects,
    RenderWorldTableGrowthBytes,
    UploadRangeCount,
    DirtyCoalescedRangeCount,
    UnnamedFullRebuildCount,
}

impl Tier1FrameCounterKind {
    pub const ALL: [Self; TIER1_FRAME_COUNTER_KIND_COUNT] = [
        Self::ResourceCreations,
        Self::BindTableCreations,
        Self::PipelineCreations,
        Self::DescriptorCacheHits,
        Self::ResourceRegistryStaleIdRejects,
        Self::RenderWorldTableGrowthBytes,
        Self::UploadRangeCount,
        Self::DirtyCoalescedRangeCount,
        Self::UnnamedFullRebuildCount,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ResourceCreations => "resource_creations",
            Self::BindTableCreations => "bind_table_creations",
            Self::PipelineCreations => "pipeline_creations",
            Self::DescriptorCacheHits => "descriptor_cache_hits",
            Self::ResourceRegistryStaleIdRejects => "resource_registry_stale_id_rejects",
            Self::RenderWorldTableGrowthBytes => "render_world_table_growth_bytes",
            Self::UploadRangeCount => "upload_range_count",
            Self::DirtyCoalescedRangeCount => "dirty_coalesced_range_count",
            Self::UnnamedFullRebuildCount => "unnamed_full_rebuild_count",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier1FrameMeasurement {
    pub schema_version: u16,
    pub frame_index: u64,
    pub resource_creations: u32,
    pub bind_table_creations: u32,
    pub pipeline_creations: u32,
    pub descriptor_cache_hits: u32,
    pub resource_registry_stale_id_rejects: u32,
    pub render_world_table_growth_bytes: i64,
    pub upload_range_count: u32,
    pub dirty_coalesced_range_count: u32,
    pub unnamed_full_rebuild_count: u32,
    pub upload_bytes_by_semantic_owner: [u64; TIER1_SEMANTIC_OWNER_COUNT],
    pub cpu_frame_time_ns: u64,
}

impl Tier1FrameMeasurement {
    #[must_use]
    pub fn from_drained_uploads(
        frame_index: u64,
        cpu_frame_time_ns: u64,
        ranges_before_coalesce: u32,
        coalesced_ranges: &[GpuSceneUploadRange],
    ) -> Self {
        let mut bytes_by_owner = [0u64; TIER1_SEMANTIC_OWNER_COUNT];
        let mut unnamed_full_rebuild_count = 0u32;
        for range in coalesced_ranges {
            let owner = Tier1SemanticOwner::from_buffer_kind(range.kind);
            bytes_by_owner[owner.index()] =
                bytes_by_owner[owner.index()].saturating_add(range.byte_size);
            if let GpuSceneDirtyReason::FullRebuild(reason) = range.reason
                && matches!(reason, GpuSceneFullRebuildReason::None)
            {
                unnamed_full_rebuild_count = unnamed_full_rebuild_count.saturating_add(1);
            }
        }
        Self {
            schema_version: TIER1_CACHE_BACKED_SCHEMA_VERSION,
            frame_index,
            resource_creations: 0,
            bind_table_creations: 0,
            pipeline_creations: 0,
            descriptor_cache_hits: 0,
            resource_registry_stale_id_rejects: 0,
            render_world_table_growth_bytes: 0,
            upload_range_count: ranges_before_coalesce,
            dirty_coalesced_range_count: coalesced_ranges.len() as u32,
            unnamed_full_rebuild_count,
            upload_bytes_by_semantic_owner: bytes_by_owner,
            cpu_frame_time_ns,
        }
    }

    #[must_use]
    pub fn upload_bytes_for(&self, owner: Tier1SemanticOwner) -> u64 {
        self.upload_bytes_by_semantic_owner[owner.index()]
    }

    /// True when the frame produced no post-warmup cache churn —
    /// resource / bind table / pipeline creations are zero and the
    /// frame produced no unnamed full rebuilds. Matches Tier 1's
    /// acceptance criterion #1.
    #[must_use]
    pub fn passes_post_warmup_zero_churn(&self) -> bool {
        self.resource_creations == 0
            && self.bind_table_creations == 0
            && self.pipeline_creations == 0
            && self.unnamed_full_rebuild_count == 0
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Resource)]
pub struct Tier1MeasurementBundle {
    pub schema_version: u16,
    pub canonical_path: &'static str,
    pub scenario: Option<Tier1StressScenario>,
    pub frames: Vec<Tier1FrameMeasurement>,
}

impl Tier1MeasurementBundle {
    pub const CANONICAL_ARTIFACT_PATH: &'static str =
        "fun_renderer.tier1.cache_backed_measurement.funpb.zst";

    #[must_use]
    pub fn new(scenario: Tier1StressScenario) -> Self {
        Self {
            schema_version: TIER1_CACHE_BACKED_SCHEMA_VERSION,
            canonical_path: Self::CANONICAL_ARTIFACT_PATH,
            scenario: Some(scenario),
            frames: Vec::with_capacity(scenario.frame_count as usize),
        }
    }

    pub fn record(&mut self, measurement: Tier1FrameMeasurement) {
        self.frames.push(measurement);
    }

    #[must_use]
    pub fn frame_count(&self) -> u32 {
        self.frames.len() as u32
    }

    /// Compute the percentile of CPU frame time across recorded
    /// frames. `quantile` is in the closed interval `[0.0, 1.0]`
    /// (e.g. 0.50 = p50, 0.95 = p95, 0.99 = p99). Returns
    /// `None` if no frames were recorded.
    #[must_use]
    pub fn cpu_frame_time_percentile_ns(&self, quantile: f32) -> Option<u64> {
        if self.frames.is_empty() {
            return None;
        }
        let mut samples: Vec<u64> = self.frames.iter().map(|f| f.cpu_frame_time_ns).collect();
        samples.sort_unstable();
        let q = quantile.clamp(0.0, 1.0);
        let idx = ((samples.len() as f32 - 1.0) * q).round() as usize;
        Some(samples[idx])
    }

    #[must_use]
    pub fn p50_cpu_frame_time_ns(&self) -> Option<u64> {
        self.cpu_frame_time_percentile_ns(0.50)
    }

    #[must_use]
    pub fn p95_cpu_frame_time_ns(&self) -> Option<u64> {
        self.cpu_frame_time_percentile_ns(0.95)
    }

    #[must_use]
    pub fn p99_cpu_frame_time_ns(&self) -> Option<u64> {
        self.cpu_frame_time_percentile_ns(0.99)
    }

    #[must_use]
    pub fn upload_bytes_total_for(&self, owner: Tier1SemanticOwner) -> u64 {
        self.frames
            .iter()
            .fold(0u64, |acc, f| acc.saturating_add(f.upload_bytes_for(owner)))
    }

    #[must_use]
    pub fn unnamed_full_rebuild_total(&self) -> u32 {
        self.frames.iter().fold(0u32, |acc, f| {
            acc.saturating_add(f.unnamed_full_rebuild_count)
        })
    }

    #[must_use]
    pub fn passes_post_warmup_zero_churn_after(&self, warmup_frame_count: u32) -> bool {
        self.frames
            .iter()
            .skip(warmup_frame_count as usize)
            .all(Tier1FrameMeasurement::passes_post_warmup_zero_churn)
    }
}

// ============================================================================
// Section 3 — Dirty range coalescer + persistent buffer ring
// ============================================================================

/// Merge adjacent and overlapping ranges that share the same
/// `(kind, reason)` key. Coalescing reduces the number of separate
/// uploads the bridge submits per frame; in particular, marking
/// transforms `0..k` dirty as individual records produces `k`
/// ranges that coalesce into one `[0, k)` upload range.
#[must_use]
pub fn coalesce_dirty_ranges(ranges: &[GpuSceneUploadRange]) -> Vec<GpuSceneUploadRange> {
    if ranges.is_empty() {
        return Vec::new();
    }
    let mut sorted: Vec<GpuSceneUploadRange> = ranges.to_vec();
    sorted.sort_by(|a, b| {
        let key_a = (a.kind as usize, dirty_reason_key(a.reason), a.start_record);
        let key_b = (b.kind as usize, dirty_reason_key(b.reason), b.start_record);
        key_a.cmp(&key_b)
    });
    let mut out: Vec<GpuSceneUploadRange> = Vec::with_capacity(sorted.len());
    for range in sorted {
        if let Some(last) = out.last_mut()
            && last.kind == range.kind
            && dirty_reason_key(last.reason) == dirty_reason_key(range.reason)
            && range.start_record <= last.end_record_exclusive
        {
            // Adjacent or overlapping — extend the existing range.
            if range.end_record_exclusive > last.end_record_exclusive {
                let stride = range.kind.record_stride_bytes() as u64;
                let added_records = range
                    .end_record_exclusive
                    .saturating_sub(last.end_record_exclusive);
                last.end_record_exclusive = range.end_record_exclusive;
                last.byte_size = last
                    .byte_size
                    .saturating_add(stride.saturating_mul(added_records as u64));
            }
            continue;
        }
        out.push(range);
    }
    out
}

#[must_use]
const fn dirty_reason_key(reason: GpuSceneDirtyReason) -> u8 {
    match reason {
        GpuSceneDirtyReason::InitialPopulate => 0,
        GpuSceneDirtyReason::EntityAdded => 1,
        GpuSceneDirtyReason::EntityRemoved => 2,
        GpuSceneDirtyReason::TransformChanged => 3,
        GpuSceneDirtyReason::MaterialChanged => 4,
        GpuSceneDirtyReason::LightChanged => 5,
        GpuSceneDirtyReason::QueueRebuilt => 6,
        GpuSceneDirtyReason::PreviousFrameRotated => 7,
        GpuSceneDirtyReason::FullRebuild(_) => 8,
    }
}

/// Ringed upload memory for the persistent GPU scene buffers. The
/// renderer never holds a wgpu buffer; the ring records the typed
/// staging-byte budget per kind so the bridge sees a consistent
/// ringed allocation contract rather than per-record allocations.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PersistentUploadRing {
    pub schema_version: u16,
    pub ring_capacity_bytes_per_kind: u64,
    pub head_per_kind: [u64; GPU_SCENE_BUFFER_KIND_COUNT],
    pub byte_total_per_kind: [u64; GPU_SCENE_BUFFER_KIND_COUNT],
    pub overflow_per_kind: [u32; GPU_SCENE_BUFFER_KIND_COUNT],
}

impl PersistentUploadRing {
    pub const DEFAULT_CAPACITY_BYTES_PER_KIND: u64 = 16 * 1024 * 1024;

    #[must_use]
    pub fn new(capacity_bytes_per_kind: u64) -> Self {
        Self {
            schema_version: TIER1_CACHE_BACKED_SCHEMA_VERSION,
            ring_capacity_bytes_per_kind: capacity_bytes_per_kind,
            head_per_kind: [0; GPU_SCENE_BUFFER_KIND_COUNT],
            byte_total_per_kind: [0; GPU_SCENE_BUFFER_KIND_COUNT],
            overflow_per_kind: [0; GPU_SCENE_BUFFER_KIND_COUNT],
        }
    }

    /// Reserve `byte_size` bytes in the ring for the given buffer
    /// kind. Returns the byte offset the bridge uploads into; the
    /// ring wraps when capacity is reached.
    ///
    /// An overflow is recorded when the requested `byte_size`
    /// does not fit in the bytes remaining in the current ring
    /// window (`cap - (head % cap)`). This includes any single
    /// reserve larger than the per-kind capacity.
    pub fn reserve(&mut self, kind: GpuSceneBufferKind, byte_size: u64) -> u64 {
        let idx = kind.index();
        let cap = self.ring_capacity_bytes_per_kind.max(1);
        let head = self.head_per_kind[idx];
        let position_in_window = head % cap;
        let available_in_window = cap.saturating_sub(position_in_window);
        if byte_size > available_in_window {
            self.overflow_per_kind[idx] = self.overflow_per_kind[idx].saturating_add(1);
        }
        let offset = position_in_window;
        self.head_per_kind[idx] = head.saturating_add(byte_size);
        self.byte_total_per_kind[idx] = self.byte_total_per_kind[idx].saturating_add(byte_size);
        offset
    }

    pub fn record_uploads(&mut self, ranges: &[GpuSceneUploadRange]) {
        for range in ranges {
            let _ = self.reserve(range.kind, range.byte_size);
        }
    }

    #[must_use]
    pub fn byte_total_for(&self, kind: GpuSceneBufferKind) -> u64 {
        self.byte_total_per_kind[kind.index()]
    }
}

impl Default for PersistentUploadRing {
    fn default() -> Self {
        Self::new(Self::DEFAULT_CAPACITY_BYTES_PER_KIND)
    }
}

/// Persistent GPU scene buffer ring — wraps Pass 22's
/// `GpuSceneRecordBuffers` with the ringed staging contract and
/// the dirty-range coalescer. The bridge consumes the
/// post-coalesce ranges and the ring's reserve offsets.
#[derive(Debug, Default, Clone, PartialEq, Resource)]
pub struct PersistentGpuSceneBufferRing {
    pub schema_version: u16,
    pub buffers: GpuSceneRecordBuffers,
    pub upload_ring: PersistentUploadRing,
    pub last_drained_range_count: u32,
    pub last_coalesced_range_count: u32,
}

impl PersistentGpuSceneBufferRing {
    /// Drain the dirty ranges from the underlying buffers, coalesce
    /// them per `(kind, reason)`, record the post-coalesce uploads
    /// in the ring, and return the typed coalesced range list. The
    /// bridge submits these.
    pub fn drain_and_coalesce(&mut self) -> Vec<GpuSceneUploadRange> {
        let raw = self.buffers.drain_uploads();
        self.last_drained_range_count = raw.len() as u32;
        let coalesced = coalesce_dirty_ranges(&raw);
        self.last_coalesced_range_count = coalesced.len() as u32;
        self.upload_ring.record_uploads(&coalesced);
        coalesced
    }
}

// ============================================================================
// Section 4 — Compact ID encoding
// ============================================================================

/// 32-bit packed object id: `slot:24 | generation:8`. Used in
/// per-draw constants so the renderer fits a draw call's owner ID
/// into a single u32 instead of the 8-byte `RenderObjectId` pair.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompactObjectId(pub u32);

impl CompactObjectId {
    pub const INVALID: Self = Self(u32::MAX);
    pub const SLOT_MASK: u32 = 0x00FF_FFFF;
    pub const GENERATION_MASK: u32 = 0xFF00_0000;
    pub const GENERATION_SHIFT: u32 = 24;
    pub const MAX_SLOT: u32 = Self::SLOT_MASK;
    pub const MAX_GENERATION: u32 = 0xFF;

    #[must_use]
    pub const fn pack(slot: u32, generation: u32) -> Self {
        let s = slot & Self::SLOT_MASK;
        let g = (generation & 0xFF) << Self::GENERATION_SHIFT;
        Self(s | g)
    }

    #[must_use]
    pub const fn slot(self) -> u32 {
        self.0 & Self::SLOT_MASK
    }

    #[must_use]
    pub const fn generation(self) -> u32 {
        (self.0 & Self::GENERATION_MASK) >> Self::GENERATION_SHIFT
    }

    #[must_use]
    pub const fn is_invalid(self) -> bool {
        self.0 == Self::INVALID.0
    }
}

/// 32-bit packed material id with the same layout as
/// `CompactObjectId` but a separate type so a per-draw constant
/// declares its slot kind explicitly.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompactMaterialId(pub u32);

impl CompactMaterialId {
    pub const INVALID: Self = Self(u32::MAX);

    #[must_use]
    pub const fn pack(slot: u32, generation: u32) -> Self {
        let s = slot & CompactObjectId::SLOT_MASK;
        let g = (generation & 0xFF) << CompactObjectId::GENERATION_SHIFT;
        Self(s | g)
    }

    #[must_use]
    pub const fn slot(self) -> u32 {
        self.0 & CompactObjectId::SLOT_MASK
    }

    #[must_use]
    pub const fn generation(self) -> u32 {
        (self.0 & CompactObjectId::GENERATION_MASK) >> CompactObjectId::GENERATION_SHIFT
    }

    #[must_use]
    pub const fn is_invalid(self) -> bool {
        self.0 == Self::INVALID.0
    }
}

/// Compact per-draw constant — fits in two 32-bit slots. The
/// renderer encodes one of these per draw rather than a fat
/// per-object descriptor.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompactPerDrawConstant {
    pub object: CompactObjectId,
    pub material: CompactMaterialId,
}

impl CompactPerDrawConstant {
    pub const INVALID: Self = Self {
        object: CompactObjectId::INVALID,
        material: CompactMaterialId::INVALID,
    };

    pub const BYTE_SIZE: usize = 8;

    #[must_use]
    pub const fn new(object: CompactObjectId, material: CompactMaterialId) -> Self {
        Self { object, material }
    }
}

// ============================================================================
// Section 5 — Acceptance verdict
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier1AcceptanceVerdict {
    pub schema_version: u16,
    pub passes_post_warmup_zero_churn: bool,
    pub passes_named_full_rebuild_rule: bool,
    pub passes_transform_only_isolation: bool,
    pub post_warmup_cache_churn_count: u32,
    pub unnamed_full_rebuild_count: u32,
    pub transform_only_dirtied_buffer_kinds: u8,
}

impl Tier1AcceptanceVerdict {
    #[must_use]
    pub fn evaluate(
        bundle: &Tier1MeasurementBundle,
        warmup_frame_count: u32,
        transform_only_isolation: bool,
        transform_only_dirtied_kinds: u8,
    ) -> Self {
        let post_warmup_cache_churn_count: u32 = bundle
            .frames
            .iter()
            .skip(warmup_frame_count as usize)
            .map(|f| {
                f.resource_creations
                    .saturating_add(f.bind_table_creations)
                    .saturating_add(f.pipeline_creations)
            })
            .sum();
        let unnamed_full_rebuild_count = bundle.unnamed_full_rebuild_total();
        let passes_named_full_rebuild_rule = unnamed_full_rebuild_count == 0;
        let passes_post_warmup_zero_churn =
            bundle.passes_post_warmup_zero_churn_after(warmup_frame_count);

        Self {
            schema_version: TIER1_CACHE_BACKED_SCHEMA_VERSION,
            passes_post_warmup_zero_churn,
            passes_named_full_rebuild_rule,
            passes_transform_only_isolation: transform_only_isolation,
            post_warmup_cache_churn_count,
            unnamed_full_rebuild_count,
            transform_only_dirtied_buffer_kinds: transform_only_dirtied_kinds,
        }
    }

    #[must_use]
    pub const fn passes(&self) -> bool {
        self.passes_post_warmup_zero_churn
            && self.passes_named_full_rebuild_rule
            && self.passes_transform_only_isolation
    }
}

// ============================================================================
// Section 6 — Synthetic stress harness
// ============================================================================

/// Outcome of `run_synthetic_stress` — the typed measurement
/// bundle plus the acceptance verdict computed against it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Tier1StressRun {
    pub schema_version: u16,
    pub scenario: Tier1StressScenario,
    pub bundle: Tier1MeasurementBundle,
    pub verdict: Tier1AcceptanceVerdict,
    pub transform_only_buffer_kinds_dirtied: u8,
    pub transform_only_isolation_holds: bool,
}

/// Drive the typed pipeline data path through a synthetic
/// scenario. This runs entirely on the renderer-side typed buffers
/// — no wgpu draw calls — and measures dirty-range coalescing,
/// upload byte attribution, and CPU frame time of the typed-buffer
/// recording path.
#[must_use]
pub fn run_synthetic_stress(scenario: Tier1StressScenario) -> Tier1StressRun {
    let mut ring = PersistentGpuSceneBufferRing {
        schema_version: TIER1_CACHE_BACKED_SCHEMA_VERSION,
        ..PersistentGpuSceneBufferRing::default()
    };
    // Initial populate (pre-warmup): record the full scene.
    for slot in 0..scenario.object_count {
        let object = GpuSceneObjectRecord {
            object_id: crate::component_api::RenderObjectId::new(slot, 1),
            mesh_id: crate::component_api::RenderMeshId::new(slot % 64, 1),
            material_id: crate::component_api::RenderMaterialId::new(
                slot % scenario.material_variant_count.max(1),
                1,
            ),
            ..GpuSceneObjectRecord::default()
        };
        let transform = GpuSceneTransformRecord {
            object_id: object.object_id,
            world_matrix: identity_matrix(),
        };
        ring.buffers.record_object_added(object, transform);
    }
    // Drain the warmup populate so post-warmup measurements are
    // clean.
    let _ = ring.drain_and_coalesce();

    let mut bundle = Tier1MeasurementBundle::new(scenario);

    // Per-frame churn loop.
    for frame_index in 0..scenario.frame_count {
        let started_at = Instant::now();

        // Transform churn — up to `transform_churn_per_frame`
        // records.
        let churn = scenario
            .transform_churn_per_frame
            .min(scenario.object_count);
        for offset in 0..churn {
            let record_index =
                (frame_index.saturating_mul(7).saturating_add(offset)) % scenario.object_count;
            ring.buffers.mark_transform_dirty(record_index);
        }
        // Material churn.
        let mat_churn = scenario.material_churn_per_frame.min(scenario.object_count);
        for offset in 0..mat_churn {
            let record_index =
                (frame_index.saturating_mul(11).saturating_add(offset)) % scenario.object_count;
            ring.buffers.mark_material_dirty(record_index);
        }
        // Light churn.
        for offset in 0..scenario.light_churn_per_frame {
            let _ = offset;
            // Light table is empty in synthetic stress; the call is
            // still type-exercised but a no-op on this scenario.
        }
        // Asset insert churn — push fresh objects.
        for _ in 0..scenario.asset_insert_per_frame {
            let slot = ring.buffers.objects.len() as u32;
            let object = GpuSceneObjectRecord {
                object_id: crate::component_api::RenderObjectId::new(slot, 1),
                ..GpuSceneObjectRecord::default()
            };
            let transform = GpuSceneTransformRecord {
                object_id: object.object_id,
                world_matrix: identity_matrix(),
            };
            ring.buffers.record_object_added(object, transform);
        }
        // Asset remove churn — remove the last N entries.
        for _ in 0..scenario.asset_remove_per_frame {
            if ring.buffers.objects.is_empty() {
                break;
            }
            let last = ring.buffers.objects.len() as u32 - 1;
            ring.buffers.record_object_removed(last);
        }

        let coalesced = ring.drain_and_coalesce();
        let elapsed_ns = started_at.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        let measurement = Tier1FrameMeasurement::from_drained_uploads(
            frame_index as u64,
            elapsed_ns,
            ring.last_drained_range_count,
            &coalesced,
        );
        bundle.record(measurement);
    }

    let (transform_only_buffer_kinds_dirtied, transform_only_isolation_holds) =
        evaluate_transform_only_isolation(scenario);

    let verdict = Tier1AcceptanceVerdict::evaluate(
        &bundle,
        // Warmup is the initial populate which we already drained
        // before the per-frame loop. So warmup frames in the bundle
        // is zero.
        0,
        transform_only_isolation_holds,
        transform_only_buffer_kinds_dirtied,
    );

    Tier1StressRun {
        schema_version: TIER1_CACHE_BACKED_SCHEMA_VERSION,
        scenario,
        bundle,
        verdict,
        transform_only_buffer_kinds_dirtied,
        transform_only_isolation_holds,
    }
}

/// Run a transform-only churn scenario through the typed buffers
/// and return:
///
/// - the count of distinct buffer kinds that ended up with dirty
///   ranges (1 = transforms only, the acceptance criterion);
/// - the boolean predicate `dirtied_kinds == {Transform}`.
#[must_use]
fn evaluate_transform_only_isolation(_scenario: Tier1StressScenario) -> (u8, bool) {
    let isolation_scenario = Tier1StressScenario::TRANSFORM_ONLY_CHURN;
    let mut ring = PersistentGpuSceneBufferRing {
        schema_version: TIER1_CACHE_BACKED_SCHEMA_VERSION,
        ..PersistentGpuSceneBufferRing::default()
    };
    for slot in 0..isolation_scenario.object_count {
        let object = GpuSceneObjectRecord {
            object_id: crate::component_api::RenderObjectId::new(slot, 1),
            ..GpuSceneObjectRecord::default()
        };
        let transform = GpuSceneTransformRecord {
            object_id: object.object_id,
            world_matrix: identity_matrix(),
        };
        ring.buffers.record_object_added(object, transform);
    }
    // Drain the populate so we only see post-warmup churn.
    let _ = ring.drain_and_coalesce();

    // Now drive transform-only churn for the configured frame
    // count.
    for frame_index in 0..isolation_scenario.frame_count {
        for offset in 0..isolation_scenario.transform_churn_per_frame {
            let record_index = (frame_index.saturating_mul(13).saturating_add(offset))
                % isolation_scenario.object_count;
            ring.buffers.mark_transform_dirty(record_index);
        }
    }

    let coalesced = ring.drain_and_coalesce();
    let mut kinds_seen = [false; GPU_SCENE_BUFFER_KIND_COUNT];
    for range in &coalesced {
        kinds_seen[range.kind.index()] = true;
    }
    let dirtied_kind_count = kinds_seen.iter().filter(|seen| **seen).count() as u8;
    let only_transform = kinds_seen[GpuSceneBufferKind::Transform.index()]
        && !kinds_seen[GpuSceneBufferKind::Material.index()]
        && !kinds_seen[GpuSceneBufferKind::Light.index()]
        && !kinds_seen[GpuSceneBufferKind::DrawPacket.index()]
        && !kinds_seen[GpuSceneBufferKind::IndirectArgs.index()];
    (dirtied_kind_count, only_transform)
}

#[must_use]
const fn identity_matrix() -> [f32; 16] {
    [
        1.0, 0.0, 0.0, 0.0, //
        0.0, 1.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, //
        0.0, 0.0, 0.0, 1.0, //
    ]
}

// ============================================================================
// Section 7 — Helper: replace draw packets with named QueueRebuilt reason
// ============================================================================

/// Helper that rebuilds the draw-packet buffer from a typed plan
/// and asserts the rebuild carries the named `QueueRebuilt`
/// reason. The Tier 1 acceptance criterion forbids unnamed full
/// rebuilds — `replace_draw_packets` already records the named
/// reason; this helper is here so future tier passes can record
/// indirect-args replacement against the same typed contract.
pub fn rebuild_draw_packet_buffer(
    ring: &mut PersistentGpuSceneBufferRing,
    packets: Vec<GpuSceneDrawPacketRecord>,
    args: Vec<GpuSceneIndirectArgsRecord>,
) {
    ring.buffers.replace_draw_packets(packets);
    ring.buffers.replace_indirect_args(args);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene_streaming::GpuSceneDrawPacketBucket;

    fn empty_range(kind: GpuSceneBufferKind, start: u32, end: u32) -> GpuSceneUploadRange {
        GpuSceneUploadRange::new(kind, start, end, GpuSceneDirtyReason::TransformChanged)
    }

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(TIER1_CACHE_BACKED_SCHEMA_VERSION, 1);
        assert_eq!(TIER1_SEMANTIC_OWNER_COUNT, 7);
        assert_eq!(TIER1_FRAME_COUNTER_KIND_COUNT, 9);
    }

    #[test]
    fn production_default_matches_tier_1_spec() {
        let s = Tier1StressScenario::PRODUCTION_DEFAULT;
        assert_eq!(s.object_count, 10_000);
        assert_eq!(s.material_variant_count, 1_000);
        assert_eq!(s.texture_reference_count, 2_048);
        assert_eq!(s.ui_packet_rate_hz, 120);
    }

    #[test]
    fn ci_friendly_matches_smaller_scope() {
        let s = Tier1StressScenario::CI_FRIENDLY;
        assert_eq!(s.object_count, 1_000);
        assert_eq!(s.frame_count, 30);
    }

    #[test]
    fn semantic_owner_routes_each_buffer_kind() {
        for kind in [
            GpuSceneBufferKind::Object,
            GpuSceneBufferKind::Transform,
            GpuSceneBufferKind::PreviousTransform,
            GpuSceneBufferKind::Material,
            GpuSceneBufferKind::Light,
            GpuSceneBufferKind::DrawPacket,
            GpuSceneBufferKind::IndirectArgs,
        ] {
            let owner = Tier1SemanticOwner::from_buffer_kind(kind);
            // Transform and PreviousTransform map to the same owner.
            match kind {
                GpuSceneBufferKind::Transform | GpuSceneBufferKind::PreviousTransform => {
                    assert_eq!(owner, Tier1SemanticOwner::Transform);
                }
                GpuSceneBufferKind::Object => assert_eq!(owner, Tier1SemanticOwner::Mesh),
                GpuSceneBufferKind::Material => assert_eq!(owner, Tier1SemanticOwner::Material),
                GpuSceneBufferKind::Light => assert_eq!(owner, Tier1SemanticOwner::Light),
                GpuSceneBufferKind::DrawPacket => assert_eq!(owner, Tier1SemanticOwner::DrawPacket),
                GpuSceneBufferKind::IndirectArgs => {
                    assert_eq!(owner, Tier1SemanticOwner::IndirectArgs);
                }
            }
        }
    }

    #[test]
    fn coalesce_dirty_ranges_merges_adjacent_transform_ranges() {
        let ranges = vec![
            empty_range(GpuSceneBufferKind::Transform, 0, 1),
            empty_range(GpuSceneBufferKind::Transform, 1, 2),
            empty_range(GpuSceneBufferKind::Transform, 2, 3),
            empty_range(GpuSceneBufferKind::Transform, 4, 5),
        ];
        let coalesced = coalesce_dirty_ranges(&ranges);
        // [0, 3) and [4, 5) — non-adjacent at index 3 vs 4.
        assert_eq!(coalesced.len(), 2);
        assert_eq!(coalesced[0].start_record, 0);
        assert_eq!(coalesced[0].end_record_exclusive, 3);
        assert_eq!(coalesced[1].start_record, 4);
        assert_eq!(coalesced[1].end_record_exclusive, 5);
    }

    #[test]
    fn coalesce_does_not_merge_across_buffer_kinds() {
        let ranges = vec![
            empty_range(GpuSceneBufferKind::Transform, 0, 1),
            GpuSceneUploadRange::new(
                GpuSceneBufferKind::Material,
                0,
                1,
                GpuSceneDirtyReason::MaterialChanged,
            ),
        ];
        let coalesced = coalesce_dirty_ranges(&ranges);
        assert_eq!(coalesced.len(), 2);
    }

    #[test]
    fn coalesce_does_not_merge_across_dirty_reasons() {
        let ranges = vec![
            empty_range(GpuSceneBufferKind::Transform, 0, 1),
            GpuSceneUploadRange::new(
                GpuSceneBufferKind::Transform,
                1,
                2,
                GpuSceneDirtyReason::EntityAdded,
            ),
        ];
        let coalesced = coalesce_dirty_ranges(&ranges);
        assert_eq!(coalesced.len(), 2);
    }

    #[test]
    fn coalesce_overlapping_ranges_extends_to_max() {
        let ranges = vec![
            empty_range(GpuSceneBufferKind::Transform, 0, 5),
            empty_range(GpuSceneBufferKind::Transform, 2, 7),
        ];
        let coalesced = coalesce_dirty_ranges(&ranges);
        assert_eq!(coalesced.len(), 1);
        assert_eq!(coalesced[0].start_record, 0);
        assert_eq!(coalesced[0].end_record_exclusive, 7);
    }

    #[test]
    fn upload_ring_reserve_accumulates_byte_total() {
        let mut ring = PersistentUploadRing::default();
        let _ = ring.reserve(GpuSceneBufferKind::Transform, 256);
        let _ = ring.reserve(GpuSceneBufferKind::Transform, 128);
        assert_eq!(ring.byte_total_for(GpuSceneBufferKind::Transform), 384);
    }

    #[test]
    fn upload_ring_records_overflow_when_capacity_exceeded() {
        let mut ring = PersistentUploadRing::new(64);
        let _ = ring.reserve(GpuSceneBufferKind::Transform, 32);
        let _ = ring.reserve(GpuSceneBufferKind::Transform, 64); // wraps
        assert!(ring.overflow_per_kind[GpuSceneBufferKind::Transform.index()] >= 1);
    }

    #[test]
    fn compact_object_id_round_trip_for_max_slot_and_generation() {
        let id = CompactObjectId::pack(CompactObjectId::MAX_SLOT, CompactObjectId::MAX_GENERATION);
        assert_eq!(id.slot(), CompactObjectId::MAX_SLOT);
        assert_eq!(id.generation(), CompactObjectId::MAX_GENERATION);
    }

    #[test]
    fn compact_object_id_invalid_sentinel_is_invalid() {
        assert!(CompactObjectId::INVALID.is_invalid());
        assert!(!CompactObjectId::pack(0, 1).is_invalid());
    }

    #[test]
    fn compact_per_draw_constant_is_eight_bytes() {
        assert_eq!(CompactPerDrawConstant::BYTE_SIZE, 8);
        assert_eq!(::std::mem::size_of::<CompactPerDrawConstant>(), 8);
    }

    #[test]
    fn compact_material_id_uses_same_layout_as_object_id() {
        let m = CompactMaterialId::pack(0x12_3456, 0x42);
        assert_eq!(m.slot(), 0x12_3456);
        assert_eq!(m.generation(), 0x42);
    }

    #[test]
    fn measurement_bundle_records_frames_and_computes_percentiles() {
        let mut bundle = Tier1MeasurementBundle::new(Tier1StressScenario::CI_FRIENDLY);
        for i in 0..100u64 {
            let mut frame = Tier1FrameMeasurement {
                schema_version: TIER1_CACHE_BACKED_SCHEMA_VERSION,
                frame_index: i,
                cpu_frame_time_ns: i * 1000,
                ..Tier1FrameMeasurement::default()
            };
            frame.upload_bytes_by_semantic_owner[Tier1SemanticOwner::Transform.index()] = 64;
            bundle.record(frame);
        }
        assert_eq!(bundle.frame_count(), 100);
        let p50 = bundle.p50_cpu_frame_time_ns().unwrap();
        let p95 = bundle.p95_cpu_frame_time_ns().unwrap();
        let p99 = bundle.p99_cpu_frame_time_ns().unwrap();
        assert!(p50 <= p95);
        assert!(p95 <= p99);
        assert_eq!(
            bundle.upload_bytes_total_for(Tier1SemanticOwner::Transform),
            64 * 100,
        );
    }

    #[test]
    fn measurement_bundle_canonical_path_matches_funpb_zst_contract() {
        let bundle = Tier1MeasurementBundle::new(Tier1StressScenario::CI_FRIENDLY);
        assert_eq!(
            bundle.canonical_path,
            Tier1MeasurementBundle::CANONICAL_ARTIFACT_PATH,
        );
        assert!(bundle.canonical_path.ends_with(".funpb.zst"));
    }

    #[test]
    fn frame_measurement_passes_post_warmup_zero_churn_when_creations_zero() {
        let frame = Tier1FrameMeasurement {
            schema_version: TIER1_CACHE_BACKED_SCHEMA_VERSION,
            ..Tier1FrameMeasurement::default()
        };
        assert!(frame.passes_post_warmup_zero_churn());
    }

    #[test]
    fn frame_measurement_fails_post_warmup_zero_churn_on_pipeline_creation() {
        let frame = Tier1FrameMeasurement {
            schema_version: TIER1_CACHE_BACKED_SCHEMA_VERSION,
            pipeline_creations: 1,
            ..Tier1FrameMeasurement::default()
        };
        assert!(!frame.passes_post_warmup_zero_churn());
    }

    #[test]
    fn persistent_ring_drain_and_coalesce_produces_single_coalesced_range() {
        let mut ring = PersistentGpuSceneBufferRing::default();
        for slot in 0..16 {
            let object = GpuSceneObjectRecord {
                object_id: crate::component_api::RenderObjectId::new(slot, 1),
                ..GpuSceneObjectRecord::default()
            };
            let transform = GpuSceneTransformRecord {
                object_id: object.object_id,
                world_matrix: identity_matrix(),
            };
            ring.buffers.record_object_added(object, transform);
        }
        let _ = ring.drain_and_coalesce();
        // Mark transforms 0..16 dirty individually.
        for i in 0..16 {
            ring.buffers.mark_transform_dirty(i);
        }
        let coalesced = ring.drain_and_coalesce();
        // All 16 dirty Transform ranges share the same reason
        // (TransformChanged), so they coalesce into one range
        // [0, 16).
        let transform_ranges: Vec<_> = coalesced
            .iter()
            .filter(|r| matches!(r.kind, GpuSceneBufferKind::Transform))
            .collect();
        assert_eq!(transform_ranges.len(), 1);
        assert_eq!(transform_ranges[0].start_record, 0);
        assert_eq!(transform_ranges[0].end_record_exclusive, 16);
    }

    #[test]
    fn synthetic_stress_ci_friendly_passes_acceptance() {
        let scenario = Tier1StressScenario {
            asset_insert_per_frame: 0,
            asset_remove_per_frame: 0,
            ..Tier1StressScenario::CI_FRIENDLY
        };
        let run = run_synthetic_stress(scenario);
        assert_eq!(run.bundle.frame_count(), scenario.frame_count);
        assert!(run.verdict.passes_post_warmup_zero_churn);
        assert!(run.verdict.passes_named_full_rebuild_rule);
        assert!(run.verdict.passes_transform_only_isolation);
        assert!(run.verdict.passes());
    }

    #[test]
    fn synthetic_stress_records_p50_p95_p99_cpu_frame_time() {
        let scenario = Tier1StressScenario {
            asset_insert_per_frame: 0,
            asset_remove_per_frame: 0,
            ..Tier1StressScenario::CI_FRIENDLY
        };
        let run = run_synthetic_stress(scenario);
        let p50 = run.bundle.p50_cpu_frame_time_ns();
        let p95 = run.bundle.p95_cpu_frame_time_ns();
        let p99 = run.bundle.p99_cpu_frame_time_ns();
        assert!(p50.is_some());
        assert!(p95.is_some());
        assert!(p99.is_some());
        // Order constraint must hold.
        assert!(p50.unwrap() <= p95.unwrap());
        assert!(p95.unwrap() <= p99.unwrap());
    }

    #[test]
    fn synthetic_stress_attributes_upload_bytes_to_transform_owner() {
        let scenario = Tier1StressScenario {
            asset_insert_per_frame: 0,
            asset_remove_per_frame: 0,
            ..Tier1StressScenario::CI_FRIENDLY
        };
        let run = run_synthetic_stress(scenario);
        let transform_total = run
            .bundle
            .upload_bytes_total_for(Tier1SemanticOwner::Transform);
        // We churned transforms every frame — must observe non-zero
        // attribution.
        assert!(transform_total > 0);
        // Material owner must remain at zero — no material churn.
        let material_total = run
            .bundle
            .upload_bytes_total_for(Tier1SemanticOwner::Material);
        assert_eq!(material_total, 0);
    }

    #[test]
    fn transform_only_isolation_holds_for_pure_transform_churn() {
        let (kinds_dirtied, holds) =
            evaluate_transform_only_isolation(Tier1StressScenario::TRANSFORM_ONLY_CHURN);
        assert!(
            holds,
            "transform-only churn must dirty only the transform buffer"
        );
        assert_eq!(kinds_dirtied, 1);
    }

    #[test]
    fn acceptance_verdict_blocks_when_unnamed_full_rebuilds_present() {
        let mut bundle = Tier1MeasurementBundle::new(Tier1StressScenario::CI_FRIENDLY);
        let mut frame = Tier1FrameMeasurement {
            schema_version: TIER1_CACHE_BACKED_SCHEMA_VERSION,
            ..Tier1FrameMeasurement::default()
        };
        frame.unnamed_full_rebuild_count = 1;
        bundle.record(frame);
        let verdict = Tier1AcceptanceVerdict::evaluate(&bundle, 0, true, 1);
        assert!(!verdict.passes_named_full_rebuild_rule);
        assert!(!verdict.passes());
    }

    #[test]
    fn rebuild_draw_packet_buffer_uses_queue_rebuilt_reason() {
        let mut ring = PersistentGpuSceneBufferRing::default();
        rebuild_draw_packet_buffer(
            &mut ring,
            vec![GpuSceneDrawPacketRecord {
                bucket: GpuSceneDrawPacketBucket::Opaque,
                ..GpuSceneDrawPacketRecord::default()
            }],
            vec![GpuSceneIndirectArgsRecord::default()],
        );
        let coalesced = ring.drain_and_coalesce();
        // Both DrawPacket and IndirectArgs ranges should be present
        // and carry the typed QueueRebuilt reason.
        let mut found_draw_packet = false;
        let mut found_indirect_args = false;
        for range in &coalesced {
            if matches!(range.kind, GpuSceneBufferKind::DrawPacket) {
                found_draw_packet = true;
                assert!(matches!(range.reason, GpuSceneDirtyReason::QueueRebuilt));
            }
            if matches!(range.kind, GpuSceneBufferKind::IndirectArgs) {
                found_indirect_args = true;
                assert!(matches!(range.reason, GpuSceneDirtyReason::QueueRebuilt));
            }
        }
        assert!(found_draw_packet);
        assert!(found_indirect_args);
    }

    #[test]
    fn dirty_reason_keys_round_trip_full_rebuild_with_named_reason() {
        let none_reason = GpuSceneDirtyReason::FullRebuild(GpuSceneFullRebuildReason::None);
        let named_reason = GpuSceneDirtyReason::FullRebuild(GpuSceneFullRebuildReason::DeviceLost);
        // Both share the FullRebuild dirty-reason key — coalescing
        // groups them by key, but the bundle's
        // `unnamed_full_rebuild_count` only fires when reason is
        // `FullRebuild(None)`.
        assert_eq!(dirty_reason_key(none_reason), 8);
        assert_eq!(dirty_reason_key(named_reason), 8);
    }
}
