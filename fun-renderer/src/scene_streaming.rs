//! Pass 22 — Visibility, Render Queues, and GPU Scene.
//!
//! Convert ECS scene data into compact draw work without arbitrary
//! per-object iteration during recording. Inputs are render-world
//! tables (object id / mesh id / material id / world transform /
//! world AABB / layer mask / opacity mode / shadow caster) plus a
//! per-view camera frustum. Outputs are dense render-queue buckets
//! with typed `RenderQueueDrawPacket` records and the matching GPU
//! scene byte buffers (object / transform / previous-transform /
//! material / light / draw packet / indirect args) carrying typed
//! dirty upload ranges and named full-rebuild reasons.
//!
//! Design rules:
//!
//! 1. The renderer does not iterate ECS at recording time. The
//!    render queues and GPU scene buffers are dense tables; the
//!    record API is the only place that touches per-renderable
//!    data.
//! 2. No per-object descriptor creation. Pipelines and bind groups
//!    are referenced by typed indices only.
//! 3. No per-draw bridge translation. The bridge consumes the
//!    finished `RenderQueueDrawPacket` slices and the GPU scene
//!    upload ranges directly.
//! 4. Named full-rebuild reasons. A rebuild that bypasses the
//!    dirty-range path must declare a `GpuSceneFullRebuildReason`
//!    so diagnostics can name *why* the renderer regenerated all
//!    records.

use bevy_ecs::prelude::Resource;

use crate::component_api::*;

pub const SCENE_STREAMING_SCHEMA_VERSION: u16 = 1;

pub const RENDER_QUEUE_BUCKET_COUNT: usize = 7;
pub const GPU_SCENE_BUFFER_KIND_COUNT: usize = 7;
pub const RENDERER_VISIBILITY_CLASSIFICATION_COUNT: usize = 6;

// ============================================================================
// Section 1 — Visibility
// ============================================================================

/// Six-plane camera frustum produced from a `RenderCamera` /
/// `CameraProjection` pair. Planes are stored in camera space with
/// the convention `dot(plane_normal, point) + plane_distance >= 0`
/// indicating the point is inside the half-space; a renderable is
/// inside the frustum if it is inside every half-space.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct RendererCullingFrustum {
    pub planes: [RendererFrustumPlane; 6],
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct RendererFrustumPlane {
    pub normal: RenderVec3,
    pub distance: f32,
}

impl RendererFrustumPlane {
    #[must_use]
    pub const fn new(normal: RenderVec3, distance: f32) -> Self {
        Self { normal, distance }
    }

    #[must_use]
    pub fn signed_distance_to(self, point: RenderVec3) -> f32 {
        self.normal.x * point.x + self.normal.y * point.y + self.normal.z * point.z + self.distance
    }
}

impl RendererCullingFrustum {
    pub const ACCEPT_ALL: Self = Self {
        planes: [RendererFrustumPlane {
            normal: RenderVec3::new(0.0, 0.0, 1.0),
            distance: f32::MAX,
        }; 6],
    };

    /// Test the AABB against the frustum. Returns `true` if any part
    /// of the AABB is inside every plane's positive half-space.
    #[must_use]
    pub fn contains_aabb(&self, aabb: RenderAabb) -> bool {
        for plane in self.planes {
            let pushed = RenderVec3::new(
                aabb.center.x + plane.normal.x.signum() * aabb.half_extents.x.abs(),
                aabb.center.y + plane.normal.y.signum() * aabb.half_extents.y.abs(),
                aabb.center.z + plane.normal.z.signum() * aabb.half_extents.z.abs(),
            );
            if plane.signed_distance_to(pushed) < 0.0 {
                return false;
            }
        }
        true
    }
}

/// Opacity classification for a renderable. Drives queue assignment.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererOpacityMode {
    #[default]
    Opaque,
    AlphaTested,
    AlphaBlended,
}

impl RendererOpacityMode {
    #[must_use]
    pub const fn is_translucent(self) -> bool {
        matches!(self, Self::AlphaBlended)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Opaque => "opaque",
            Self::AlphaTested => "alpha_tested",
            Self::AlphaBlended => "alpha_blended",
        }
    }
}

/// Per-camera shadow-caster filter policy.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererShadowFilter {
    #[default]
    AcceptAll,
    OnlyShadowCasters,
    NoShadowCasters,
}

/// Visibility classification result for one renderable / view pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererVisibilityClassification {
    Visible,
    CulledByVisibilityState,
    CulledByLayerMask,
    CulledByFrustum,
    CulledByCameraOrder,
    CulledByShadowFilter,
}

impl RendererVisibilityClassification {
    pub const ALL: [Self; RENDERER_VISIBILITY_CLASSIFICATION_COUNT] = [
        Self::Visible,
        Self::CulledByVisibilityState,
        Self::CulledByLayerMask,
        Self::CulledByFrustum,
        Self::CulledByCameraOrder,
        Self::CulledByShadowFilter,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Visible => "visible",
            Self::CulledByVisibilityState => "culled_by_visibility_state",
            Self::CulledByLayerMask => "culled_by_layer_mask",
            Self::CulledByFrustum => "culled_by_frustum",
            Self::CulledByCameraOrder => "culled_by_camera_order",
            Self::CulledByShadowFilter => "culled_by_shadow_filter",
        }
    }

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Visible => 0,
            Self::CulledByVisibilityState => 1,
            Self::CulledByLayerMask => 2,
            Self::CulledByFrustum => 3,
            Self::CulledByCameraOrder => 4,
            Self::CulledByShadowFilter => 5,
        }
    }

    #[must_use]
    pub const fn is_visible(self) -> bool {
        matches!(self, Self::Visible)
    }
}

/// Renderable input for visibility classification. The fields are
/// the minimum needed to classify against a camera + frustum + layer
/// mask without touching the ECS or asset registry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RendererRenderableInput {
    pub object_id: RenderObjectId,
    pub mesh_id: RenderMeshId,
    pub material_id: RenderMaterialId,
    pub layers: RenderLayerMask,
    pub world_aabb: RenderAabb,
    pub world_position: RenderVec3,
    pub visibility: RenderVisibilityState,
    pub opacity: RendererOpacityMode,
    pub shadow_caster: bool,
    pub camera_order_min: i16,
    pub camera_order_max: i16,
}

impl RendererRenderableInput {
    #[must_use]
    pub const fn opaque_static(
        object_id: RenderObjectId,
        mesh_id: RenderMeshId,
        material_id: RenderMaterialId,
        world_aabb: RenderAabb,
        world_position: RenderVec3,
    ) -> Self {
        Self {
            object_id,
            mesh_id,
            material_id,
            layers: RenderLayerMask::DEFAULT,
            world_aabb,
            world_position,
            visibility: RenderVisibilityState::Visible,
            opacity: RendererOpacityMode::Opaque,
            shadow_caster: true,
            camera_order_min: i16::MIN,
            camera_order_max: i16::MAX,
        }
    }
}

/// Per-camera input for visibility classification.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RendererCameraInput {
    pub view_id: RenderViewId,
    pub layers: RenderLayerMask,
    pub frustum: RendererCullingFrustum,
    pub shadow_filter: RendererShadowFilter,
    pub order: i16,
    pub is_shadow_pass: bool,
}

impl Default for RendererCameraInput {
    fn default() -> Self {
        Self {
            view_id: RenderViewId::INVALID,
            layers: RenderLayerMask::DEFAULT,
            frustum: RendererCullingFrustum::ACCEPT_ALL,
            shadow_filter: RendererShadowFilter::AcceptAll,
            order: 0,
            is_shadow_pass: false,
        }
    }
}

/// Visibility verdict produced by `classify_renderable_for_view`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RendererVisibilityVerdict {
    pub object_id: RenderObjectId,
    pub view_id: RenderViewId,
    pub classification: RendererVisibilityClassification,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub struct RendererVisibilityConfig {
    pub frustum_culling_enabled: bool,
    pub layer_mask_filtering_enabled: bool,
    pub shadow_caster_filtering_enabled: bool,
    pub camera_order_filtering_enabled: bool,
}

impl RendererVisibilityConfig {
    pub const PRODUCT_DEFAULT: Self = Self {
        frustum_culling_enabled: true,
        layer_mask_filtering_enabled: true,
        shadow_caster_filtering_enabled: true,
        camera_order_filtering_enabled: true,
    };

    pub const ALL_ACCEPT: Self = Self {
        frustum_culling_enabled: false,
        layer_mask_filtering_enabled: false,
        shadow_caster_filtering_enabled: false,
        camera_order_filtering_enabled: false,
    };
}

#[must_use]
fn shadow_filter_culls(camera: &RendererCameraInput, shadow_caster: bool) -> bool {
    match camera.shadow_filter {
        RendererShadowFilter::AcceptAll => false,
        RendererShadowFilter::OnlyShadowCasters => camera.is_shadow_pass && !shadow_caster,
        RendererShadowFilter::NoShadowCasters => !camera.is_shadow_pass && shadow_caster,
    }
}

/// Pure classification function. The order of checks is fixed so
/// the resulting `classification` field names the *first* reason
/// the renderable was rejected, which keeps diagnostics
/// deterministic regardless of camera or layer changes.
#[must_use]
pub fn classify_renderable_for_view(
    renderable: &RendererRenderableInput,
    camera: &RendererCameraInput,
    config: &RendererVisibilityConfig,
) -> RendererVisibilityVerdict {
    let classification = if matches!(renderable.visibility, RenderVisibilityState::Hidden) {
        RendererVisibilityClassification::CulledByVisibilityState
    } else if config.layer_mask_filtering_enabled && (renderable.layers.0 & camera.layers.0 == 0) {
        RendererVisibilityClassification::CulledByLayerMask
    } else if config.camera_order_filtering_enabled
        && (camera.order < renderable.camera_order_min
            || camera.order > renderable.camera_order_max)
    {
        RendererVisibilityClassification::CulledByCameraOrder
    } else if config.shadow_caster_filtering_enabled
        && shadow_filter_culls(camera, renderable.shadow_caster)
    {
        RendererVisibilityClassification::CulledByShadowFilter
    } else if config.frustum_culling_enabled && !camera.frustum.contains_aabb(renderable.world_aabb)
    {
        RendererVisibilityClassification::CulledByFrustum
    } else {
        RendererVisibilityClassification::Visible
    };

    RendererVisibilityVerdict {
        object_id: renderable.object_id,
        view_id: camera.view_id,
        classification,
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RendererVisibilityClassificationCounts {
    pub counts: [u32; RENDERER_VISIBILITY_CLASSIFICATION_COUNT],
}

impl RendererVisibilityClassificationCounts {
    pub fn record(&mut self, classification: RendererVisibilityClassification) {
        let slot = &mut self.counts[classification.index()];
        *slot = slot.saturating_add(1);
    }

    #[must_use]
    pub const fn count(&self, classification: RendererVisibilityClassification) -> u32 {
        self.counts[classification.index()]
    }

    #[must_use]
    pub fn total(&self) -> u32 {
        self.counts
            .iter()
            .copied()
            .fold(0u32, |acc, value| acc.saturating_add(value))
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub struct RendererVisibilityDiagnostics {
    pub schema_version: u16,
    pub view_count: u32,
    pub renderable_count: u32,
    pub classification_counts: RendererVisibilityClassificationCounts,
}

// ============================================================================
// Section 2 — Render queues
// ============================================================================

/// Draw-call queue bucket. This is *not* the GPU device queue (that
/// is `queue_scheduler::RenderQueueKind`) — these are draw-call
/// buckets the renderer writes into for sorted submission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderQueueBucket {
    Depth,
    Opaque,
    AlphaTest,
    Transparent,
    Shadow,
    Ui,
    Debug,
}

impl RenderQueueBucket {
    pub const ALL: [Self; RENDER_QUEUE_BUCKET_COUNT] = [
        Self::Depth,
        Self::Opaque,
        Self::AlphaTest,
        Self::Transparent,
        Self::Shadow,
        Self::Ui,
        Self::Debug,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Depth => 0,
            Self::Opaque => 1,
            Self::AlphaTest => 2,
            Self::Transparent => 3,
            Self::Shadow => 4,
            Self::Ui => 5,
            Self::Debug => 6,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Depth => "depth",
            Self::Opaque => "opaque",
            Self::AlphaTest => "alpha_test",
            Self::Transparent => "transparent",
            Self::Shadow => "shadow",
            Self::Ui => "ui",
            Self::Debug => "debug",
        }
    }

    /// Default sort order for the bucket. Front-to-back for opaque /
    /// alpha-test / depth (early-z friendly), back-to-front for
    /// transparent (correct alpha blending), state-sorted for shadow
    /// (pipeline / mesh sort minimises state changes), submit-order
    /// for UI / debug (author intent / overlay order).
    #[must_use]
    pub const fn default_sort_order(self) -> RenderQueueSortOrder {
        match self {
            Self::Depth | Self::Opaque | Self::AlphaTest => RenderQueueSortOrder::FrontToBack,
            Self::Transparent => RenderQueueSortOrder::BackToFront,
            Self::Shadow => RenderQueueSortOrder::StateSorted,
            Self::Ui | Self::Debug => RenderQueueSortOrder::SubmitOrder,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderQueueSortOrder {
    #[default]
    FrontToBack,
    BackToFront,
    StateSorted,
    SubmitOrder,
}

impl RenderQueueSortOrder {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FrontToBack => "front_to_back",
            Self::BackToFront => "back_to_front",
            Self::StateSorted => "state_sorted",
            Self::SubmitOrder => "submit_order",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderQueueDrawFlags(pub u32);

impl RenderQueueDrawFlags {
    pub const NONE: Self = Self(0);
    pub const VISIBLE: Self = Self(1 << 0);
    pub const SHADOW_CASTER: Self = Self(1 << 1);
    pub const DOUBLE_SIDED: Self = Self(1 << 2);
    pub const TRANSPARENT_BLEND: Self = Self(1 << 3);
    pub const ALPHA_TEST: Self = Self(1 << 4);
    pub const UI_OVERLAY: Self = Self(1 << 5);
    pub const DEBUG_OVERLAY: Self = Self(1 << 6);
    pub const STATIC_GEOMETRY: Self = Self(1 << 7);

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// Pipeline reference owned by the queue. The renderer never holds a
/// wgpu pipeline — it holds a typed pipeline slot index and the
/// bridge resolves to the actual GPU pipeline.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderPipelineSlot {
    pub slot: u32,
    pub generation: u32,
}

impl RenderPipelineSlot {
    pub const INVALID: Self = Self {
        slot: u32::MAX,
        generation: 0,
    };

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.slot != u32::MAX && self.generation != 0
    }

    #[must_use]
    pub const fn new(slot: u32, generation: u32) -> Self {
        Self { slot, generation }
    }
}

/// Range of contiguous instance entries in the GPU instance table.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderInstanceRange {
    pub first_instance: u32,
    pub instance_count: u32,
}

impl RenderInstanceRange {
    #[must_use]
    pub const fn single(first_instance: u32) -> Self {
        Self {
            first_instance,
            instance_count: 1,
        }
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.instance_count == 0
    }
}

/// Composed sort key used to order draw packets within a bucket.
/// Layout (high to low bits):
///   * 16 bits — pipeline slot
///   * 16 bits — material slot
///   * 32 bits — depth quantised to u32
///
/// For back-to-front buckets the depth bits are inverted at compose
/// time so that ascending sort produces correct draw order.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RenderQueueSortKey(pub u64);

impl RenderQueueSortKey {
    #[must_use]
    pub const fn new(pipeline_slot: u16, material_slot: u16, depth_quantised: u32) -> Self {
        let pipeline = (pipeline_slot as u64) << 48;
        let material = (material_slot as u64) << 32;
        let depth = depth_quantised as u64;
        Self(pipeline | material | depth)
    }

    #[must_use]
    pub const fn pipeline_slot(self) -> u16 {
        ((self.0 >> 48) & 0xffff) as u16
    }

    #[must_use]
    pub const fn material_slot(self) -> u16 {
        ((self.0 >> 32) & 0xffff) as u16
    }

    #[must_use]
    pub const fn depth_quantised(self) -> u32 {
        (self.0 & 0xffff_ffff) as u32
    }
}

/// One draw packet in a render queue bucket. Every field is a typed
/// index — no descriptors, no wgpu handles, no bridge state. The
/// bridge consumes the packet by dereferencing each slot through
/// the GPU scene buffer set.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderQueueDrawPacket {
    pub object_id: RenderObjectId,
    pub mesh_id: RenderMeshId,
    pub material_id: RenderMaterialId,
    pub pipeline_slot: RenderPipelineSlot,
    pub sort_key: RenderQueueSortKey,
    pub instance_range: RenderInstanceRange,
    pub flags: RenderQueueDrawFlags,
}

#[must_use]
pub fn quantise_depth_for_view(world_position: RenderVec3, camera_origin: RenderVec3) -> u32 {
    let dx = world_position.x - camera_origin.x;
    let dy = world_position.y - camera_origin.y;
    let dz = world_position.z - camera_origin.z;
    let distance_squared = dx * dx + dy * dy + dz * dz;
    if !distance_squared.is_finite() {
        return u32::MAX;
    }
    let distance = distance_squared.sqrt();
    if distance <= 0.0 {
        return 0;
    }
    let scaled = distance * 16.0;
    if scaled >= u32::MAX as f32 {
        u32::MAX
    } else {
        scaled as u32
    }
}

/// Pick the queue bucket for a renderable + camera pair. Visibility
/// classification produces one entry per visible renderable; this
/// step decides which queue records the entry.
#[must_use]
pub fn assign_to_queue(
    renderable: &RendererRenderableInput,
    camera: &RendererCameraInput,
) -> RenderQueueBucket {
    if camera.is_shadow_pass {
        return RenderQueueBucket::Shadow;
    }
    match renderable.opacity {
        RendererOpacityMode::Opaque => RenderQueueBucket::Opaque,
        RendererOpacityMode::AlphaTested => RenderQueueBucket::AlphaTest,
        RendererOpacityMode::AlphaBlended => RenderQueueBucket::Transparent,
    }
}

#[must_use]
pub fn render_queue_flags_for(
    renderable: &RendererRenderableInput,
    is_static: bool,
) -> RenderQueueDrawFlags {
    let mut flags = RenderQueueDrawFlags::VISIBLE;
    if renderable.shadow_caster {
        flags = flags.union(RenderQueueDrawFlags::SHADOW_CASTER);
    }
    if matches!(renderable.opacity, RendererOpacityMode::AlphaBlended) {
        flags = flags.union(RenderQueueDrawFlags::TRANSPARENT_BLEND);
    }
    if matches!(renderable.opacity, RendererOpacityMode::AlphaTested) {
        flags = flags.union(RenderQueueDrawFlags::ALPHA_TEST);
    }
    if is_static {
        flags = flags.union(RenderQueueDrawFlags::STATIC_GEOMETRY);
    }
    flags
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RenderQueueBucketRecords {
    pub bucket: Option<RenderQueueBucket>,
    pub sort_order: RenderQueueSortOrder,
    pub draws: Vec<RenderQueueDrawPacket>,
    pub sorted: bool,
}

impl RenderQueueBucketRecords {
    fn for_bucket(bucket: RenderQueueBucket) -> Self {
        Self {
            bucket: Some(bucket),
            sort_order: bucket.default_sort_order(),
            draws: Vec::new(),
            sorted: true,
        }
    }

    fn clear(&mut self) {
        self.draws.clear();
        self.sorted = true;
    }

    fn record(&mut self, packet: RenderQueueDrawPacket) {
        self.draws.push(packet);
        self.sorted = false;
    }

    pub fn sort(&mut self) {
        match self.sort_order {
            RenderQueueSortOrder::FrontToBack | RenderQueueSortOrder::StateSorted => {
                self.draws.sort_by_key(|packet| packet.sort_key);
            }
            RenderQueueSortOrder::BackToFront => {
                self.draws
                    .sort_by_key(|packet| core::cmp::Reverse(packet.sort_key));
            }
            RenderQueueSortOrder::SubmitOrder => {}
        }
        self.sorted = true;
    }

    #[must_use]
    pub fn draws(&self) -> &[RenderQueueDrawPacket] {
        &self.draws
    }
}

/// Render-queue table — one bucket per `RenderQueueBucket`. The
/// renderer recording side iterates this dense table; it never
/// queries the ECS.
#[derive(Debug, Clone, PartialEq, Eq, Resource)]
pub struct RendererRenderQueues {
    buckets: [RenderQueueBucketRecords; RENDER_QUEUE_BUCKET_COUNT],
    diagnostics: RendererQueueDiagnostics,
}

impl Default for RendererRenderQueues {
    fn default() -> Self {
        let buckets = [
            RenderQueueBucketRecords::for_bucket(RenderQueueBucket::Depth),
            RenderQueueBucketRecords::for_bucket(RenderQueueBucket::Opaque),
            RenderQueueBucketRecords::for_bucket(RenderQueueBucket::AlphaTest),
            RenderQueueBucketRecords::for_bucket(RenderQueueBucket::Transparent),
            RenderQueueBucketRecords::for_bucket(RenderQueueBucket::Shadow),
            RenderQueueBucketRecords::for_bucket(RenderQueueBucket::Ui),
            RenderQueueBucketRecords::for_bucket(RenderQueueBucket::Debug),
        ];
        Self {
            buckets,
            diagnostics: RendererQueueDiagnostics::default(),
        }
    }
}

impl RendererRenderQueues {
    pub fn clear_all(&mut self) {
        for bucket in &mut self.buckets {
            bucket.clear();
        }
        self.diagnostics = RendererQueueDiagnostics::default();
    }

    pub fn record(&mut self, bucket: RenderQueueBucket, packet: RenderQueueDrawPacket) {
        self.buckets[bucket.index()].record(packet);
    }

    pub fn sort_all(&mut self) {
        for bucket in &mut self.buckets {
            bucket.sort();
        }
    }

    #[must_use]
    pub fn bucket(&self, bucket: RenderQueueBucket) -> &RenderQueueBucketRecords {
        &self.buckets[bucket.index()]
    }

    #[must_use]
    pub fn draws(&self, bucket: RenderQueueBucket) -> &[RenderQueueDrawPacket] {
        self.buckets[bucket.index()].draws()
    }

    #[must_use]
    pub fn total_draw_packets(&self) -> u32 {
        self.buckets.iter().fold(0u32, |acc, bucket| {
            acc.saturating_add(bucket.draws.len() as u32)
        })
    }

    pub fn refresh_diagnostics(&mut self) {
        let mut diagnostics = RendererQueueDiagnostics {
            schema_version: SCENE_STREAMING_SCHEMA_VERSION,
            ..RendererQueueDiagnostics::default()
        };
        for bucket in &self.buckets {
            if let Some(kind) = bucket.bucket {
                let count = bucket.draws.len() as u32;
                diagnostics.draw_counts[kind.index()] = count;
                diagnostics.bucket_sorted[kind.index()] = bucket.sorted;
                diagnostics.total_draws = diagnostics.total_draws.saturating_add(count);
            }
        }
        self.diagnostics = diagnostics;
    }

    #[must_use]
    pub fn diagnostics(&self) -> RendererQueueDiagnostics {
        self.diagnostics
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub struct RendererQueueDiagnostics {
    pub schema_version: u16,
    pub total_draws: u32,
    pub draw_counts: [u32; RENDER_QUEUE_BUCKET_COUNT],
    pub bucket_sorted: [bool; RENDER_QUEUE_BUCKET_COUNT],
}

impl RendererQueueDiagnostics {
    #[must_use]
    pub const fn count(&self, bucket: RenderQueueBucket) -> u32 {
        self.draw_counts[bucket.index()]
    }

    #[must_use]
    pub const fn is_sorted(&self, bucket: RenderQueueBucket) -> bool {
        self.bucket_sorted[bucket.index()]
    }
}

// ============================================================================
// Section 3 — GPU scene buffers
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuSceneBufferKind {
    Object,
    Transform,
    PreviousTransform,
    Material,
    Light,
    DrawPacket,
    IndirectArgs,
}

impl GpuSceneBufferKind {
    pub const ALL: [Self; GPU_SCENE_BUFFER_KIND_COUNT] = [
        Self::Object,
        Self::Transform,
        Self::PreviousTransform,
        Self::Material,
        Self::Light,
        Self::DrawPacket,
        Self::IndirectArgs,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Object => 0,
            Self::Transform => 1,
            Self::PreviousTransform => 2,
            Self::Material => 3,
            Self::Light => 4,
            Self::DrawPacket => 5,
            Self::IndirectArgs => 6,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Object => "object",
            Self::Transform => "transform",
            Self::PreviousTransform => "previous_transform",
            Self::Material => "material",
            Self::Light => "light",
            Self::DrawPacket => "draw_packet",
            Self::IndirectArgs => "indirect_args",
        }
    }

    #[must_use]
    pub const fn record_stride_bytes(self) -> u32 {
        match self {
            Self::Object => 32,            // 4*u32 + 4*u32 = 32 bytes (object header)
            Self::Transform => 64,         // mat4 = 16 floats = 64 bytes
            Self::PreviousTransform => 64, // mat4 = 16 floats = 64 bytes
            Self::Material => 80, // PBR table: 4 colours + 4 factors + 4 texture refs = 80 bytes
            Self::Light => 48,    // 12 floats = 48 bytes
            Self::DrawPacket => 32, // u32*8 = 32 bytes per indirect-style packet
            Self::IndirectArgs => 20, // D3D12_DRAW_INDEXED_ARGUMENTS = 5*u32 = 20 bytes
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuSceneBufferLayout {
    pub kind: GpuSceneBufferKindOption,
    pub stride_bytes: u32,
    pub capacity_records: u32,
    pub byte_size: u64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuSceneBufferKindOption {
    #[default]
    Object,
    Transform,
    PreviousTransform,
    Material,
    Light,
    DrawPacket,
    IndirectArgs,
}

impl From<GpuSceneBufferKind> for GpuSceneBufferKindOption {
    fn from(kind: GpuSceneBufferKind) -> Self {
        match kind {
            GpuSceneBufferKind::Object => Self::Object,
            GpuSceneBufferKind::Transform => Self::Transform,
            GpuSceneBufferKind::PreviousTransform => Self::PreviousTransform,
            GpuSceneBufferKind::Material => Self::Material,
            GpuSceneBufferKind::Light => Self::Light,
            GpuSceneBufferKind::DrawPacket => Self::DrawPacket,
            GpuSceneBufferKind::IndirectArgs => Self::IndirectArgs,
        }
    }
}

impl From<GpuSceneBufferKindOption> for GpuSceneBufferKind {
    fn from(option: GpuSceneBufferKindOption) -> Self {
        match option {
            GpuSceneBufferKindOption::Object => Self::Object,
            GpuSceneBufferKindOption::Transform => Self::Transform,
            GpuSceneBufferKindOption::PreviousTransform => Self::PreviousTransform,
            GpuSceneBufferKindOption::Material => Self::Material,
            GpuSceneBufferKindOption::Light => Self::Light,
            GpuSceneBufferKindOption::DrawPacket => Self::DrawPacket,
            GpuSceneBufferKindOption::IndirectArgs => Self::IndirectArgs,
        }
    }
}

impl GpuSceneBufferLayout {
    #[must_use]
    pub fn new(kind: GpuSceneBufferKind, capacity_records: u32) -> Self {
        let stride_bytes = kind.record_stride_bytes();
        let byte_size = (stride_bytes as u64).saturating_mul(capacity_records as u64);
        Self {
            kind: kind.into(),
            stride_bytes,
            capacity_records,
            byte_size,
        }
    }

    #[must_use]
    pub fn kind(self) -> GpuSceneBufferKind {
        self.kind.into()
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuSceneBufferSet {
    pub layouts: [GpuSceneBufferLayout; GPU_SCENE_BUFFER_KIND_COUNT],
}

impl GpuSceneBufferSet {
    #[must_use]
    pub fn from_capacities(capacities: [u32; GPU_SCENE_BUFFER_KIND_COUNT]) -> Self {
        let mut layouts = [GpuSceneBufferLayout::default(); GPU_SCENE_BUFFER_KIND_COUNT];
        for kind in GpuSceneBufferKind::ALL {
            layouts[kind.index()] = GpuSceneBufferLayout::new(kind, capacities[kind.index()]);
        }
        Self { layouts }
    }

    #[must_use]
    pub fn layout(&self, kind: GpuSceneBufferKind) -> GpuSceneBufferLayout {
        self.layouts[kind.index()]
    }

    #[must_use]
    pub fn total_byte_size(&self) -> u64 {
        self.layouts
            .iter()
            .fold(0u64, |acc, layout| acc.saturating_add(layout.byte_size))
    }
}

// --- Per-buffer record types ---------------------------------------------

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuSceneObjectRecord {
    pub object_id: RenderObjectId,
    pub mesh_id: RenderMeshId,
    pub material_id: RenderMaterialId,
    pub layers: RenderLayerMask,
    pub flags: RenderQueueDrawFlags,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct GpuSceneTransformRecord {
    pub object_id: RenderObjectId,
    pub world_matrix: [f32; 16],
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct GpuScenePreviousTransformRecord {
    pub object_id: RenderObjectId,
    pub previous_world_matrix: [f32; 16],
    pub history_id: RenderStableId,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct GpuSceneMaterialRecord {
    pub material_id: RenderMaterialId,
    pub base_color: RenderColor,
    pub emissive: RenderColor,
    pub metallic: f32,
    pub roughness: f32,
    pub alpha_cutoff: f32,
    pub feature_mask: MaterialFeatureMask,
    pub texture_refs: [RenderTextureId; 4],
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct GpuSceneLightRecord {
    pub light_id: RenderLightId,
    pub color: RenderColor,
    pub intensity: f32,
    pub range: f32,
    pub kind: GpuSceneLightKind,
    pub layers: RenderLayerMask,
    pub bounds: RenderAabb,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuSceneLightKind {
    #[default]
    Directional,
    Point,
    Spot,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuSceneDrawPacketRecord {
    pub bucket: GpuSceneDrawPacketBucket,
    pub object_id: RenderObjectId,
    pub mesh_id: RenderMeshId,
    pub material_id: RenderMaterialId,
    pub pipeline_slot: RenderPipelineSlot,
    pub indirect_args_index: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuSceneDrawPacketBucket {
    #[default]
    Depth,
    Opaque,
    AlphaTest,
    Transparent,
    Shadow,
    Ui,
    Debug,
}

impl From<RenderQueueBucket> for GpuSceneDrawPacketBucket {
    fn from(bucket: RenderQueueBucket) -> Self {
        match bucket {
            RenderQueueBucket::Depth => Self::Depth,
            RenderQueueBucket::Opaque => Self::Opaque,
            RenderQueueBucket::AlphaTest => Self::AlphaTest,
            RenderQueueBucket::Transparent => Self::Transparent,
            RenderQueueBucket::Shadow => Self::Shadow,
            RenderQueueBucket::Ui => Self::Ui,
            RenderQueueBucket::Debug => Self::Debug,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuSceneIndirectArgsRecord {
    pub index_count: u32,
    pub instance_count: u32,
    pub first_index: u32,
    pub base_vertex: i32,
    pub first_instance: u32,
}

// --- Dirty range and full-rebuild reasons --------------------------------

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuSceneFullRebuildReason {
    #[default]
    None,
    ExplicitOperatorRequest,
    SchemaVersionChanged,
    AssetRegistryInvalidated,
    StableIdAllocatorReset,
    DeviceLost,
    BufferCapacityShrunk,
    NewView,
    PreviousFrameUnavailable,
}

impl GpuSceneFullRebuildReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ExplicitOperatorRequest => "explicit_operator_request",
            Self::SchemaVersionChanged => "schema_version_changed",
            Self::AssetRegistryInvalidated => "asset_registry_invalidated",
            Self::StableIdAllocatorReset => "stable_id_allocator_reset",
            Self::DeviceLost => "device_lost",
            Self::BufferCapacityShrunk => "buffer_capacity_shrunk",
            Self::NewView => "new_view",
            Self::PreviousFrameUnavailable => "previous_frame_unavailable",
        }
    }

    #[must_use]
    pub const fn requires_full_rebuild(self) -> bool {
        !matches!(self, Self::None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuSceneDirtyReason {
    InitialPopulate,
    EntityAdded,
    EntityRemoved,
    TransformChanged,
    MaterialChanged,
    LightChanged,
    QueueRebuilt,
    PreviousFrameRotated,
    FullRebuild(GpuSceneFullRebuildReason),
}

impl GpuSceneDirtyReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InitialPopulate => "initial_populate",
            Self::EntityAdded => "entity_added",
            Self::EntityRemoved => "entity_removed",
            Self::TransformChanged => "transform_changed",
            Self::MaterialChanged => "material_changed",
            Self::LightChanged => "light_changed",
            Self::QueueRebuilt => "queue_rebuilt",
            Self::PreviousFrameRotated => "previous_frame_rotated",
            Self::FullRebuild(_) => "full_rebuild",
        }
    }

    #[must_use]
    pub const fn full_rebuild_reason(self) -> GpuSceneFullRebuildReason {
        match self {
            Self::FullRebuild(reason) => reason,
            _ => GpuSceneFullRebuildReason::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuSceneUploadRange {
    pub kind: GpuSceneBufferKind,
    pub start_record: u32,
    pub end_record_exclusive: u32,
    pub byte_size: u64,
    pub reason: GpuSceneDirtyReason,
}

impl GpuSceneUploadRange {
    #[must_use]
    pub fn new(
        kind: GpuSceneBufferKind,
        start_record: u32,
        end_record_exclusive: u32,
        reason: GpuSceneDirtyReason,
    ) -> Self {
        let record_count = end_record_exclusive.saturating_sub(start_record);
        let byte_size = (kind.record_stride_bytes() as u64).saturating_mul(record_count as u64);
        Self {
            kind,
            start_record,
            end_record_exclusive,
            byte_size,
            reason,
        }
    }

    #[must_use]
    pub const fn record_count(self) -> u32 {
        self.end_record_exclusive.saturating_sub(self.start_record)
    }
}

// --- The CPU shadow tables backing each GPU scene buffer -----------------

#[derive(Debug, Default, Clone, PartialEq, Resource)]
pub struct GpuSceneRecordBuffers {
    pub objects: Vec<GpuSceneObjectRecord>,
    pub transforms: Vec<GpuSceneTransformRecord>,
    pub previous_transforms: Vec<GpuScenePreviousTransformRecord>,
    pub materials: Vec<GpuSceneMaterialRecord>,
    pub lights: Vec<GpuSceneLightRecord>,
    pub draw_packets: Vec<GpuSceneDrawPacketRecord>,
    pub indirect_args: Vec<GpuSceneIndirectArgsRecord>,
    dirty_ranges: Vec<GpuSceneUploadRange>,
    full_rebuild_reason: GpuSceneFullRebuildReason,
    upload_generation: u64,
    diagnostics: GpuSceneBufferDiagnostics,
}

impl GpuSceneRecordBuffers {
    #[must_use]
    pub fn buffer_set(&self) -> GpuSceneBufferSet {
        GpuSceneBufferSet::from_capacities([
            self.objects.len() as u32,
            self.transforms.len() as u32,
            self.previous_transforms.len() as u32,
            self.materials.len() as u32,
            self.lights.len() as u32,
            self.draw_packets.len() as u32,
            self.indirect_args.len() as u32,
        ])
    }

    pub fn declare_full_rebuild(&mut self, reason: GpuSceneFullRebuildReason) {
        self.full_rebuild_reason = reason;
        self.dirty_ranges.push(GpuSceneUploadRange::new(
            GpuSceneBufferKind::Object,
            0,
            self.objects.len() as u32,
            GpuSceneDirtyReason::FullRebuild(reason),
        ));
        self.dirty_ranges.push(GpuSceneUploadRange::new(
            GpuSceneBufferKind::Transform,
            0,
            self.transforms.len() as u32,
            GpuSceneDirtyReason::FullRebuild(reason),
        ));
        self.dirty_ranges.push(GpuSceneUploadRange::new(
            GpuSceneBufferKind::PreviousTransform,
            0,
            self.previous_transforms.len() as u32,
            GpuSceneDirtyReason::FullRebuild(reason),
        ));
        self.dirty_ranges.push(GpuSceneUploadRange::new(
            GpuSceneBufferKind::Material,
            0,
            self.materials.len() as u32,
            GpuSceneDirtyReason::FullRebuild(reason),
        ));
        self.dirty_ranges.push(GpuSceneUploadRange::new(
            GpuSceneBufferKind::Light,
            0,
            self.lights.len() as u32,
            GpuSceneDirtyReason::FullRebuild(reason),
        ));
        self.dirty_ranges.push(GpuSceneUploadRange::new(
            GpuSceneBufferKind::DrawPacket,
            0,
            self.draw_packets.len() as u32,
            GpuSceneDirtyReason::FullRebuild(reason),
        ));
        self.dirty_ranges.push(GpuSceneUploadRange::new(
            GpuSceneBufferKind::IndirectArgs,
            0,
            self.indirect_args.len() as u32,
            GpuSceneDirtyReason::FullRebuild(reason),
        ));
    }

    pub fn mark_object_dirty(&mut self, record: u32, reason: GpuSceneDirtyReason) {
        self.dirty_ranges.push(GpuSceneUploadRange::new(
            GpuSceneBufferKind::Object,
            record,
            record.saturating_add(1),
            reason,
        ));
    }

    pub fn mark_transform_dirty(&mut self, record: u32) {
        self.dirty_ranges.push(GpuSceneUploadRange::new(
            GpuSceneBufferKind::Transform,
            record,
            record.saturating_add(1),
            GpuSceneDirtyReason::TransformChanged,
        ));
    }

    pub fn mark_previous_transform_rotated(&mut self) {
        self.dirty_ranges.push(GpuSceneUploadRange::new(
            GpuSceneBufferKind::PreviousTransform,
            0,
            self.previous_transforms.len() as u32,
            GpuSceneDirtyReason::PreviousFrameRotated,
        ));
    }

    pub fn mark_material_dirty(&mut self, record: u32) {
        self.dirty_ranges.push(GpuSceneUploadRange::new(
            GpuSceneBufferKind::Material,
            record,
            record.saturating_add(1),
            GpuSceneDirtyReason::MaterialChanged,
        ));
    }

    pub fn mark_light_dirty(&mut self, record: u32) {
        self.dirty_ranges.push(GpuSceneUploadRange::new(
            GpuSceneBufferKind::Light,
            record,
            record.saturating_add(1),
            GpuSceneDirtyReason::LightChanged,
        ));
    }

    pub fn mark_draw_packet_range_dirty(&mut self, start: u32, end: u32) {
        self.dirty_ranges.push(GpuSceneUploadRange::new(
            GpuSceneBufferKind::DrawPacket,
            start,
            end,
            GpuSceneDirtyReason::QueueRebuilt,
        ));
    }

    pub fn mark_indirect_args_range_dirty(&mut self, start: u32, end: u32) {
        self.dirty_ranges.push(GpuSceneUploadRange::new(
            GpuSceneBufferKind::IndirectArgs,
            start,
            end,
            GpuSceneDirtyReason::QueueRebuilt,
        ));
    }

    pub fn record_object_added(
        &mut self,
        object: GpuSceneObjectRecord,
        transform: GpuSceneTransformRecord,
    ) -> u32 {
        let index = self.objects.len() as u32;
        self.objects.push(object);
        self.transforms.push(transform);
        self.previous_transforms
            .push(GpuScenePreviousTransformRecord {
                object_id: object.object_id,
                previous_world_matrix: transform.world_matrix,
                history_id: RenderStableId::INVALID,
            });
        self.dirty_ranges.push(GpuSceneUploadRange::new(
            GpuSceneBufferKind::Object,
            index,
            index.saturating_add(1),
            GpuSceneDirtyReason::EntityAdded,
        ));
        self.dirty_ranges.push(GpuSceneUploadRange::new(
            GpuSceneBufferKind::Transform,
            index,
            index.saturating_add(1),
            GpuSceneDirtyReason::EntityAdded,
        ));
        self.dirty_ranges.push(GpuSceneUploadRange::new(
            GpuSceneBufferKind::PreviousTransform,
            index,
            index.saturating_add(1),
            GpuSceneDirtyReason::EntityAdded,
        ));
        index
    }

    pub fn record_object_removed(&mut self, record: u32) {
        let last = self.objects.len().saturating_sub(1) as u32;
        if record > last {
            return;
        }
        self.objects.swap_remove(record as usize);
        self.transforms.swap_remove(record as usize);
        self.previous_transforms.swap_remove(record as usize);
        self.dirty_ranges.push(GpuSceneUploadRange::new(
            GpuSceneBufferKind::Object,
            record,
            record.saturating_add(1),
            GpuSceneDirtyReason::EntityRemoved,
        ));
        if record < self.objects.len() as u32 {
            // The swapped-in entry needs an upload too because it
            // moved index; mark its new home dirty so the GPU table
            // matches the CPU shadow.
            self.dirty_ranges.push(GpuSceneUploadRange::new(
                GpuSceneBufferKind::Object,
                record,
                record.saturating_add(1),
                GpuSceneDirtyReason::EntityAdded,
            ));
            self.dirty_ranges.push(GpuSceneUploadRange::new(
                GpuSceneBufferKind::Transform,
                record,
                record.saturating_add(1),
                GpuSceneDirtyReason::EntityAdded,
            ));
        }
    }

    pub fn replace_draw_packets(&mut self, packets: Vec<GpuSceneDrawPacketRecord>) {
        let new_len = packets.len() as u32;
        self.draw_packets = packets;
        self.dirty_ranges.push(GpuSceneUploadRange::new(
            GpuSceneBufferKind::DrawPacket,
            0,
            new_len,
            GpuSceneDirtyReason::QueueRebuilt,
        ));
    }

    pub fn replace_indirect_args(&mut self, args: Vec<GpuSceneIndirectArgsRecord>) {
        let new_len = args.len() as u32;
        self.indirect_args = args;
        self.dirty_ranges.push(GpuSceneUploadRange::new(
            GpuSceneBufferKind::IndirectArgs,
            0,
            new_len,
            GpuSceneDirtyReason::QueueRebuilt,
        ));
    }

    pub fn drain_uploads(&mut self) -> Vec<GpuSceneUploadRange> {
        let drained = core::mem::take(&mut self.dirty_ranges);
        self.upload_generation = self.upload_generation.saturating_add(1);
        self.full_rebuild_reason = GpuSceneFullRebuildReason::None;
        self.refresh_diagnostics(&drained);
        drained
    }

    fn refresh_diagnostics(&mut self, drained: &[GpuSceneUploadRange]) {
        let mut diagnostics = GpuSceneBufferDiagnostics {
            schema_version: SCENE_STREAMING_SCHEMA_VERSION,
            upload_generation: self.upload_generation,
            full_rebuild_reason: GpuSceneFullRebuildReason::None,
            ..GpuSceneBufferDiagnostics::default()
        };
        for range in drained {
            let bytes = range.byte_size;
            let kind_index = range.kind.index();
            diagnostics.uploaded_bytes_by_kind[kind_index] =
                diagnostics.uploaded_bytes_by_kind[kind_index].saturating_add(bytes);
            diagnostics.uploaded_records_by_kind[kind_index] = diagnostics.uploaded_records_by_kind
                [kind_index]
                .saturating_add(range.record_count());
            diagnostics.dirty_range_count = diagnostics.dirty_range_count.saturating_add(1);
            diagnostics.uploaded_bytes_total =
                diagnostics.uploaded_bytes_total.saturating_add(bytes);
            if let GpuSceneDirtyReason::FullRebuild(reason) = range.reason {
                diagnostics.full_rebuild_reason = reason;
                diagnostics.full_rebuild_range_count =
                    diagnostics.full_rebuild_range_count.saturating_add(1);
            }
        }
        self.diagnostics = diagnostics;
    }

    #[must_use]
    pub fn pending_dirty_ranges(&self) -> &[GpuSceneUploadRange] {
        &self.dirty_ranges
    }

    #[must_use]
    pub const fn upload_generation(&self) -> u64 {
        self.upload_generation
    }

    #[must_use]
    pub const fn last_full_rebuild_reason(&self) -> GpuSceneFullRebuildReason {
        self.full_rebuild_reason
    }

    #[must_use]
    pub fn diagnostics(&self) -> GpuSceneBufferDiagnostics {
        self.diagnostics
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Resource)]
pub struct GpuSceneBufferDiagnostics {
    pub schema_version: u16,
    pub upload_generation: u64,
    pub uploaded_bytes_total: u64,
    pub dirty_range_count: u32,
    pub full_rebuild_range_count: u32,
    pub full_rebuild_reason: GpuSceneFullRebuildReason,
    pub uploaded_bytes_by_kind: [u64; GPU_SCENE_BUFFER_KIND_COUNT],
    pub uploaded_records_by_kind: [u32; GPU_SCENE_BUFFER_KIND_COUNT],
}

impl GpuSceneBufferDiagnostics {
    #[must_use]
    pub const fn bytes_for(&self, kind: GpuSceneBufferKind) -> u64 {
        self.uploaded_bytes_by_kind[kind.index()]
    }

    #[must_use]
    pub const fn records_for(&self, kind: GpuSceneBufferKind) -> u32 {
        self.uploaded_records_by_kind[kind.index()]
    }
}

// ============================================================================
// Section 4 — Glue helpers
// ============================================================================

/// Run the visibility / queue / GPU-scene pipeline for a single
/// camera + renderable list. Test surface — production wiring is a
/// Bevy schedule but we keep the data-shape helper here so a unit
/// test can exercise the contract without spinning up a `World`.
pub fn populate_render_queues_for_view(
    renderables: &[RendererRenderableInput],
    camera: &RendererCameraInput,
    config: &RendererVisibilityConfig,
    queues: &mut RendererRenderQueues,
    visibility_diagnostics: &mut RendererVisibilityDiagnostics,
) -> u32 {
    visibility_diagnostics.schema_version = SCENE_STREAMING_SCHEMA_VERSION;
    visibility_diagnostics.view_count = visibility_diagnostics.view_count.saturating_add(1);
    let mut visible_count = 0u32;
    for renderable in renderables {
        visibility_diagnostics.renderable_count =
            visibility_diagnostics.renderable_count.saturating_add(1);
        let verdict = classify_renderable_for_view(renderable, camera, config);
        visibility_diagnostics
            .classification_counts
            .record(verdict.classification);
        if !verdict.classification.is_visible() {
            continue;
        }
        let bucket = assign_to_queue(renderable, camera);
        let depth =
            quantise_depth_for_view(renderable.world_position, RenderVec3::new(0.0, 0.0, 0.0));
        let pipeline_slot = renderable.material_id.slot;
        let sort_key = RenderQueueSortKey::new(
            (pipeline_slot & 0xffff) as u16,
            (renderable.material_id.slot & 0xffff) as u16,
            depth,
        );
        let flags = render_queue_flags_for(
            renderable,
            !matches!(renderable.opacity, RendererOpacityMode::AlphaBlended),
        );
        let packet = RenderQueueDrawPacket {
            object_id: renderable.object_id,
            mesh_id: renderable.mesh_id,
            material_id: renderable.material_id,
            pipeline_slot: RenderPipelineSlot::new(pipeline_slot, 1),
            sort_key,
            instance_range: RenderInstanceRange::single(renderable.object_id.slot),
            flags,
        };
        queues.record(bucket, packet);
        visible_count = visible_count.saturating_add(1);
    }
    queues.sort_all();
    queues.refresh_diagnostics();
    visible_count
}

#[cfg(test)]
mod tests {
    use super::*;

    fn renderable_at(
        position: RenderVec3,
        opacity: RendererOpacityMode,
    ) -> RendererRenderableInput {
        let object = RenderObjectId::new(1, 1);
        let mesh = RenderMeshId::new(2, 1);
        let material = RenderMaterialId::new(3, 1);
        RendererRenderableInput {
            object_id: object,
            mesh_id: mesh,
            material_id: material,
            layers: RenderLayerMask::DEFAULT,
            world_aabb: RenderAabb::new(position, RenderVec3::new(0.5, 0.5, 0.5)),
            world_position: position,
            visibility: RenderVisibilityState::Visible,
            opacity,
            shadow_caster: true,
            camera_order_min: i16::MIN,
            camera_order_max: i16::MAX,
        }
    }

    fn accept_all_camera() -> RendererCameraInput {
        RendererCameraInput {
            view_id: RenderViewId::new(1, 1),
            ..Default::default()
        }
    }

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(SCENE_STREAMING_SCHEMA_VERSION, 1);
        assert_eq!(RENDER_QUEUE_BUCKET_COUNT, 7);
        assert_eq!(GPU_SCENE_BUFFER_KIND_COUNT, 7);
    }

    #[test]
    fn frustum_accept_all_passes_arbitrary_aabbs() {
        let frustum = RendererCullingFrustum::ACCEPT_ALL;
        let aabb = RenderAabb::new(
            RenderVec3::new(1_000.0, 0.0, 0.0),
            RenderVec3::new(1.0, 1.0, 1.0),
        );
        assert!(frustum.contains_aabb(aabb));
    }

    #[test]
    fn hidden_visibility_state_is_culled_first() {
        let mut renderable =
            renderable_at(RenderVec3::new(0.0, 0.0, 0.0), RendererOpacityMode::Opaque);
        renderable.visibility = RenderVisibilityState::Hidden;
        let camera = accept_all_camera();
        let verdict = classify_renderable_for_view(
            &renderable,
            &camera,
            &RendererVisibilityConfig::PRODUCT_DEFAULT,
        );
        assert_eq!(
            verdict.classification,
            RendererVisibilityClassification::CulledByVisibilityState,
        );
    }

    #[test]
    fn layer_mask_filtering_culls_when_no_overlap() {
        let mut renderable =
            renderable_at(RenderVec3::new(0.0, 0.0, 0.0), RendererOpacityMode::Opaque);
        renderable.layers = RenderLayerMask(0b0010);
        let mut camera = accept_all_camera();
        camera.layers = RenderLayerMask(0b0001);
        let verdict = classify_renderable_for_view(
            &renderable,
            &camera,
            &RendererVisibilityConfig::PRODUCT_DEFAULT,
        );
        assert_eq!(
            verdict.classification,
            RendererVisibilityClassification::CulledByLayerMask,
        );
    }

    #[test]
    fn camera_order_filtering_rejects_out_of_range_camera() {
        let mut renderable =
            renderable_at(RenderVec3::new(0.0, 0.0, 0.0), RendererOpacityMode::Opaque);
        renderable.camera_order_min = 10;
        renderable.camera_order_max = 20;
        let mut camera = accept_all_camera();
        camera.order = 5;
        let verdict = classify_renderable_for_view(
            &renderable,
            &camera,
            &RendererVisibilityConfig::PRODUCT_DEFAULT,
        );
        assert_eq!(
            verdict.classification,
            RendererVisibilityClassification::CulledByCameraOrder,
        );
    }

    #[test]
    fn shadow_pass_with_only_shadow_casters_culls_non_caster() {
        let mut renderable =
            renderable_at(RenderVec3::new(0.0, 0.0, 0.0), RendererOpacityMode::Opaque);
        renderable.shadow_caster = false;
        let mut camera = accept_all_camera();
        camera.is_shadow_pass = true;
        camera.shadow_filter = RendererShadowFilter::OnlyShadowCasters;
        let verdict = classify_renderable_for_view(
            &renderable,
            &camera,
            &RendererVisibilityConfig::PRODUCT_DEFAULT,
        );
        assert_eq!(
            verdict.classification,
            RendererVisibilityClassification::CulledByShadowFilter,
        );
    }

    #[test]
    fn frustum_culling_rejects_aabb_outside_plane() {
        let renderable = renderable_at(
            RenderVec3::new(100.0, 0.0, 0.0),
            RendererOpacityMode::Opaque,
        );
        let mut camera = accept_all_camera();
        camera.frustum = RendererCullingFrustum {
            planes: [
                RendererFrustumPlane::new(RenderVec3::new(-1.0, 0.0, 0.0), 1.0),
                RendererFrustumPlane::new(RenderVec3::new(0.0, 0.0, 1.0), f32::MAX),
                RendererFrustumPlane::new(RenderVec3::new(0.0, 0.0, 1.0), f32::MAX),
                RendererFrustumPlane::new(RenderVec3::new(0.0, 0.0, 1.0), f32::MAX),
                RendererFrustumPlane::new(RenderVec3::new(0.0, 0.0, 1.0), f32::MAX),
                RendererFrustumPlane::new(RenderVec3::new(0.0, 0.0, 1.0), f32::MAX),
            ],
        };
        let verdict = classify_renderable_for_view(
            &renderable,
            &camera,
            &RendererVisibilityConfig::PRODUCT_DEFAULT,
        );
        assert_eq!(
            verdict.classification,
            RendererVisibilityClassification::CulledByFrustum,
        );
    }

    #[test]
    fn opaque_renderable_routes_to_opaque_bucket() {
        let renderable = renderable_at(RenderVec3::new(0.0, 0.0, 0.0), RendererOpacityMode::Opaque);
        let camera = accept_all_camera();
        assert_eq!(
            assign_to_queue(&renderable, &camera),
            RenderQueueBucket::Opaque
        );
    }

    #[test]
    fn alpha_tested_renderable_routes_to_alpha_test_bucket() {
        let renderable = renderable_at(
            RenderVec3::new(0.0, 0.0, 0.0),
            RendererOpacityMode::AlphaTested,
        );
        let camera = accept_all_camera();
        assert_eq!(
            assign_to_queue(&renderable, &camera),
            RenderQueueBucket::AlphaTest,
        );
    }

    #[test]
    fn alpha_blended_renderable_routes_to_transparent_bucket() {
        let renderable = renderable_at(
            RenderVec3::new(0.0, 0.0, 0.0),
            RendererOpacityMode::AlphaBlended,
        );
        let camera = accept_all_camera();
        assert_eq!(
            assign_to_queue(&renderable, &camera),
            RenderQueueBucket::Transparent,
        );
    }

    #[test]
    fn shadow_pass_camera_routes_to_shadow_bucket() {
        let renderable = renderable_at(RenderVec3::new(0.0, 0.0, 0.0), RendererOpacityMode::Opaque);
        let mut camera = accept_all_camera();
        camera.is_shadow_pass = true;
        assert_eq!(
            assign_to_queue(&renderable, &camera),
            RenderQueueBucket::Shadow
        );
    }

    #[test]
    fn render_queue_default_sort_orders_match_pass_22_contract() {
        assert_eq!(
            RenderQueueBucket::Depth.default_sort_order(),
            RenderQueueSortOrder::FrontToBack,
        );
        assert_eq!(
            RenderQueueBucket::Opaque.default_sort_order(),
            RenderQueueSortOrder::FrontToBack,
        );
        assert_eq!(
            RenderQueueBucket::AlphaTest.default_sort_order(),
            RenderQueueSortOrder::FrontToBack,
        );
        assert_eq!(
            RenderQueueBucket::Transparent.default_sort_order(),
            RenderQueueSortOrder::BackToFront,
        );
        assert_eq!(
            RenderQueueBucket::Shadow.default_sort_order(),
            RenderQueueSortOrder::StateSorted,
        );
        assert_eq!(
            RenderQueueBucket::Ui.default_sort_order(),
            RenderQueueSortOrder::SubmitOrder,
        );
        assert_eq!(
            RenderQueueBucket::Debug.default_sort_order(),
            RenderQueueSortOrder::SubmitOrder,
        );
    }

    #[test]
    fn sort_key_round_trips_components() {
        let key = RenderQueueSortKey::new(0x0a0b, 0x1234, 0xdead_beef);
        assert_eq!(key.pipeline_slot(), 0x0a0b);
        assert_eq!(key.material_slot(), 0x1234);
        assert_eq!(key.depth_quantised(), 0xdead_beef);
    }

    #[test]
    fn sort_key_higher_pipeline_slot_dominates_lower_material_slot() {
        let later_pipeline = RenderQueueSortKey::new(2, 0, 0);
        let earlier_pipeline = RenderQueueSortKey::new(1, u16::MAX, u32::MAX);
        assert!(earlier_pipeline < later_pipeline);
    }

    #[test]
    fn opaque_bucket_sort_uses_front_to_back() {
        let mut queues = RendererRenderQueues::default();
        let near = RenderQueueDrawPacket {
            sort_key: RenderQueueSortKey::new(0, 0, 100),
            ..RenderQueueDrawPacket::default()
        };
        let far = RenderQueueDrawPacket {
            sort_key: RenderQueueSortKey::new(0, 0, 10_000),
            ..RenderQueueDrawPacket::default()
        };
        queues.record(RenderQueueBucket::Opaque, far);
        queues.record(RenderQueueBucket::Opaque, near);
        queues.sort_all();
        let draws = queues.draws(RenderQueueBucket::Opaque);
        assert_eq!(draws[0].sort_key.depth_quantised(), 100);
        assert_eq!(draws[1].sort_key.depth_quantised(), 10_000);
    }

    #[test]
    fn transparent_bucket_sort_uses_back_to_front() {
        let mut queues = RendererRenderQueues::default();
        let near = RenderQueueDrawPacket {
            sort_key: RenderQueueSortKey::new(0, 0, 100),
            ..RenderQueueDrawPacket::default()
        };
        let far = RenderQueueDrawPacket {
            sort_key: RenderQueueSortKey::new(0, 0, 10_000),
            ..RenderQueueDrawPacket::default()
        };
        queues.record(RenderQueueBucket::Transparent, near);
        queues.record(RenderQueueBucket::Transparent, far);
        queues.sort_all();
        let draws = queues.draws(RenderQueueBucket::Transparent);
        assert_eq!(draws[0].sort_key.depth_quantised(), 10_000);
        assert_eq!(draws[1].sort_key.depth_quantised(), 100);
    }

    #[test]
    fn pipeline_slot_invalid_sentinel_is_invalid() {
        assert!(!RenderPipelineSlot::INVALID.is_valid());
        assert!(RenderPipelineSlot::new(0, 1).is_valid());
        assert!(!RenderPipelineSlot::new(u32::MAX, 1).is_valid());
    }

    #[test]
    fn render_queue_flags_compose_via_union() {
        let flags = RenderQueueDrawFlags::VISIBLE
            .union(RenderQueueDrawFlags::SHADOW_CASTER)
            .union(RenderQueueDrawFlags::ALPHA_TEST);
        assert!(flags.contains(RenderQueueDrawFlags::VISIBLE));
        assert!(flags.contains(RenderQueueDrawFlags::SHADOW_CASTER));
        assert!(flags.contains(RenderQueueDrawFlags::ALPHA_TEST));
        assert!(!flags.contains(RenderQueueDrawFlags::TRANSPARENT_BLEND));
    }

    #[test]
    fn populate_render_queues_records_visible_renderables() {
        let renderables = vec![
            renderable_at(RenderVec3::new(0.0, 0.0, 1.0), RendererOpacityMode::Opaque),
            renderable_at(
                RenderVec3::new(0.0, 0.0, 5.0),
                RendererOpacityMode::AlphaTested,
            ),
            renderable_at(
                RenderVec3::new(0.0, 0.0, 9.0),
                RendererOpacityMode::AlphaBlended,
            ),
        ];
        let camera = accept_all_camera();
        let mut queues = RendererRenderQueues::default();
        let mut diagnostics = RendererVisibilityDiagnostics::default();
        let visible = populate_render_queues_for_view(
            &renderables,
            &camera,
            &RendererVisibilityConfig::PRODUCT_DEFAULT,
            &mut queues,
            &mut diagnostics,
        );
        assert_eq!(visible, 3);
        assert_eq!(queues.draws(RenderQueueBucket::Opaque).len(), 1);
        assert_eq!(queues.draws(RenderQueueBucket::AlphaTest).len(), 1);
        assert_eq!(queues.draws(RenderQueueBucket::Transparent).len(), 1);
        let queue_diagnostics = queues.diagnostics();
        assert_eq!(queue_diagnostics.total_draws, 3);
        assert!(queue_diagnostics.is_sorted(RenderQueueBucket::Opaque));
    }

    #[test]
    fn visibility_diagnostics_record_classification_counts() {
        let mut renderables = vec![renderable_at(
            RenderVec3::new(0.0, 0.0, 0.0),
            RendererOpacityMode::Opaque,
        )];
        renderables[0].visibility = RenderVisibilityState::Hidden;
        let camera = accept_all_camera();
        let mut queues = RendererRenderQueues::default();
        let mut diagnostics = RendererVisibilityDiagnostics::default();
        populate_render_queues_for_view(
            &renderables,
            &camera,
            &RendererVisibilityConfig::PRODUCT_DEFAULT,
            &mut queues,
            &mut diagnostics,
        );
        assert_eq!(
            diagnostics
                .classification_counts
                .count(RendererVisibilityClassification::CulledByVisibilityState),
            1,
        );
        assert_eq!(diagnostics.renderable_count, 1);
        assert_eq!(diagnostics.view_count, 1);
    }

    #[test]
    fn gpu_scene_buffer_strides_match_pass_22_contract() {
        assert_eq!(GpuSceneBufferKind::Object.record_stride_bytes(), 32);
        assert_eq!(GpuSceneBufferKind::Transform.record_stride_bytes(), 64);
        assert_eq!(
            GpuSceneBufferKind::PreviousTransform.record_stride_bytes(),
            64,
        );
        assert_eq!(GpuSceneBufferKind::Material.record_stride_bytes(), 80);
        assert_eq!(GpuSceneBufferKind::Light.record_stride_bytes(), 48);
        assert_eq!(GpuSceneBufferKind::DrawPacket.record_stride_bytes(), 32);
        assert_eq!(GpuSceneBufferKind::IndirectArgs.record_stride_bytes(), 20);
    }

    #[test]
    fn buffer_set_total_byte_size_matches_capacities() {
        let set = GpuSceneBufferSet::from_capacities([10, 10, 10, 10, 10, 10, 10]);
        let expected = (32 + 64 + 64 + 80 + 48 + 32 + 20) * 10;
        assert_eq!(set.total_byte_size(), expected as u64);
    }

    #[test]
    fn record_object_added_pushes_object_transform_and_previous_transform_dirty_ranges() {
        let mut buffers = GpuSceneRecordBuffers::default();
        let object = GpuSceneObjectRecord {
            object_id: RenderObjectId::new(1, 1),
            mesh_id: RenderMeshId::new(2, 1),
            material_id: RenderMaterialId::new(3, 1),
            layers: RenderLayerMask::DEFAULT,
            flags: RenderQueueDrawFlags::VISIBLE,
        };
        let transform = GpuSceneTransformRecord {
            object_id: object.object_id,
            world_matrix: [0.0; 16],
        };
        let index = buffers.record_object_added(object, transform);
        assert_eq!(index, 0);
        assert_eq!(buffers.objects.len(), 1);
        assert_eq!(buffers.transforms.len(), 1);
        assert_eq!(buffers.previous_transforms.len(), 1);
        assert_eq!(buffers.pending_dirty_ranges().len(), 3);
    }

    #[test]
    fn drain_uploads_clears_pending_and_advances_generation() {
        let mut buffers = GpuSceneRecordBuffers::default();
        buffers.objects.push(GpuSceneObjectRecord::default());
        buffers.transforms.push(GpuSceneTransformRecord::default());
        buffers.mark_transform_dirty(0);
        let drained = buffers.drain_uploads();
        assert_eq!(drained.len(), 1);
        assert_eq!(buffers.upload_generation(), 1);
        assert_eq!(buffers.pending_dirty_ranges().len(), 0);
    }

    #[test]
    fn declare_full_rebuild_emits_named_reason_per_kind() {
        let mut buffers = GpuSceneRecordBuffers::default();
        buffers.objects.push(GpuSceneObjectRecord::default());
        buffers.transforms.push(GpuSceneTransformRecord::default());
        buffers
            .previous_transforms
            .push(GpuScenePreviousTransformRecord::default());
        buffers.materials.push(GpuSceneMaterialRecord::default());
        buffers.lights.push(GpuSceneLightRecord::default());
        buffers
            .draw_packets
            .push(GpuSceneDrawPacketRecord::default());
        buffers
            .indirect_args
            .push(GpuSceneIndirectArgsRecord::default());
        buffers.declare_full_rebuild(GpuSceneFullRebuildReason::DeviceLost);
        assert_eq!(buffers.pending_dirty_ranges().len(), 7);
        assert_eq!(
            buffers.last_full_rebuild_reason(),
            GpuSceneFullRebuildReason::DeviceLost,
        );
        let drained = buffers.drain_uploads();
        let diagnostics = buffers.diagnostics();
        assert_eq!(diagnostics.full_rebuild_range_count, 7);
        assert_eq!(
            diagnostics.full_rebuild_reason,
            GpuSceneFullRebuildReason::DeviceLost,
        );
        assert_eq!(drained.len(), 7);
    }

    #[test]
    fn dirty_reason_full_rebuild_carries_named_reason() {
        let reason =
            GpuSceneDirtyReason::FullRebuild(GpuSceneFullRebuildReason::SchemaVersionChanged);
        assert_eq!(
            reason.full_rebuild_reason(),
            GpuSceneFullRebuildReason::SchemaVersionChanged,
        );
        assert!(GpuSceneFullRebuildReason::SchemaVersionChanged.requires_full_rebuild());
        assert!(!GpuSceneFullRebuildReason::None.requires_full_rebuild());
    }

    #[test]
    fn record_object_removed_emits_swap_remove_dirty_ranges() {
        let mut buffers = GpuSceneRecordBuffers::default();
        let object_a = GpuSceneObjectRecord {
            object_id: RenderObjectId::new(1, 1),
            ..GpuSceneObjectRecord::default()
        };
        let object_b = GpuSceneObjectRecord {
            object_id: RenderObjectId::new(2, 1),
            ..GpuSceneObjectRecord::default()
        };
        buffers.record_object_added(object_a, GpuSceneTransformRecord::default());
        buffers.record_object_added(object_b, GpuSceneTransformRecord::default());
        let _ = buffers.drain_uploads();
        buffers.record_object_removed(0);
        assert_eq!(buffers.objects.len(), 1);
        assert_eq!(buffers.objects[0].object_id, RenderObjectId::new(2, 1));
        let pending = buffers.pending_dirty_ranges();
        assert_eq!(pending.len(), 3);
    }

    #[test]
    fn replace_draw_packets_dirties_full_range_with_queue_rebuild_reason() {
        let mut buffers = GpuSceneRecordBuffers::default();
        let packets = vec![
            GpuSceneDrawPacketRecord {
                bucket: GpuSceneDrawPacketBucket::Opaque,
                ..GpuSceneDrawPacketRecord::default()
            },
            GpuSceneDrawPacketRecord {
                bucket: GpuSceneDrawPacketBucket::AlphaTest,
                ..GpuSceneDrawPacketRecord::default()
            },
        ];
        buffers.replace_draw_packets(packets);
        let pending = buffers.pending_dirty_ranges();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].kind, GpuSceneBufferKind::DrawPacket);
        assert_eq!(pending[0].record_count(), 2);
        assert!(matches!(
            pending[0].reason,
            GpuSceneDirtyReason::QueueRebuilt
        ));
    }

    #[test]
    fn diagnostics_attributes_bytes_per_kind() {
        let mut buffers = GpuSceneRecordBuffers::default();
        buffers.objects.push(GpuSceneObjectRecord::default());
        buffers.transforms.push(GpuSceneTransformRecord::default());
        buffers.materials.push(GpuSceneMaterialRecord::default());
        buffers.mark_transform_dirty(0);
        buffers.mark_material_dirty(0);
        let _ = buffers.drain_uploads();
        let diagnostics = buffers.diagnostics();
        assert_eq!(
            diagnostics.bytes_for(GpuSceneBufferKind::Transform),
            GpuSceneBufferKind::Transform.record_stride_bytes() as u64,
        );
        assert_eq!(
            diagnostics.bytes_for(GpuSceneBufferKind::Material),
            GpuSceneBufferKind::Material.record_stride_bytes() as u64,
        );
        assert_eq!(diagnostics.dirty_range_count, 2);
    }

    #[test]
    fn render_queue_flags_for_static_opaque_renderable() {
        let renderable = renderable_at(RenderVec3::ZERO, RendererOpacityMode::Opaque);
        let flags = render_queue_flags_for(&renderable, true);
        assert!(flags.contains(RenderQueueDrawFlags::VISIBLE));
        assert!(flags.contains(RenderQueueDrawFlags::SHADOW_CASTER));
        assert!(flags.contains(RenderQueueDrawFlags::STATIC_GEOMETRY));
        assert!(!flags.contains(RenderQueueDrawFlags::TRANSPARENT_BLEND));
        assert!(!flags.contains(RenderQueueDrawFlags::ALPHA_TEST));
    }

    #[test]
    fn render_queue_flags_for_alpha_blended_dynamic_renderable() {
        let renderable = renderable_at(RenderVec3::ZERO, RendererOpacityMode::AlphaBlended);
        let flags = render_queue_flags_for(&renderable, false);
        assert!(flags.contains(RenderQueueDrawFlags::TRANSPARENT_BLEND));
        assert!(!flags.contains(RenderQueueDrawFlags::STATIC_GEOMETRY));
    }

    #[test]
    fn previous_frame_unavailable_is_named_full_rebuild_reason() {
        let mut buffers = GpuSceneRecordBuffers::default();
        buffers
            .previous_transforms
            .push(GpuScenePreviousTransformRecord::default());
        buffers.declare_full_rebuild(GpuSceneFullRebuildReason::PreviousFrameUnavailable);
        let pending = buffers.pending_dirty_ranges();
        let mut found = false;
        for range in pending {
            if let GpuSceneDirtyReason::FullRebuild(reason) = range.reason
                && matches!(reason, GpuSceneFullRebuildReason::PreviousFrameUnavailable)
            {
                found = true;
            }
        }
        assert!(found);
    }
}
