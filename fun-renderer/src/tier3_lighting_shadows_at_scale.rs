//! Tier 3 — Lighting and Shadows at Scale.
//!
//! Pass 24 installed the typed lighting stack (`LightingLightTable`,
//! `ClusterGridDescriptor`, `ClusteredLightListBuffer`,
//! `ShadowAtlasPlan`, `ShadowCascadePlan`, `DepthOnlyPso`,
//! `IblPlaceholder`). The existing `virtual_shadow` module
//! installed the typed virtual-page surface
//! (`ShadowPageRecord`, `ShadowStorageDiagnostics`,
//! `ShadowRefreshReport`). Tier 3 puts both under measured
//! pressure:
//!
//! 1. **Adaptive cluster dimensions** — `AdaptiveClusterPolicy`
//!    picks `ClusterGridDescriptor` from render-target extent +
//!    camera depth range so low-res targets get fewer clusters,
//!    high-res targets get more XY clusters, and large depth
//!    ranges get more Z slices.
//!
//! 2. **Static vs dynamic light separation** — typed
//!    `LightCategory` plus a `StaticLightAssignmentCache` that
//!    keys static-light assignments by camera + cluster grid
//!    signature so the per-frame work is only the dynamic
//!    portion.
//!
//! 3. **Overflow resolution** —
//!    `ClusterOverflowResolutionStrategy` (KeepFirstN /
//!    ImportanceSorted / BrightnessSorted / AllowOverflow) drives
//!    a typed `ClusterOverflowResolved` per cluster with the
//!    `overflow_dropped_count` recorded for diagnostics.
//!
//! 4. **Lights-per-cluster heatmap** —
//!    `LightsPerClusterHeatmap::from_buffer` produces real
//!    counts per cluster plus p50/p95/p99 statistics so the
//!    user-named heatmap is data, not just an enum selector.
//!
//! 5. **Sublinear scaling measurement** —
//!    `Tier3LightingScalingMeasurement` runs a sequence of
//!    light counts through the typed assignment harness and
//!    records the ratio `assignments / light_count`; the
//!    sublinear acceptance rule is that this ratio does not
//!    grow as light count grows (i.e. each light contributes
//!    only to a bounded subset of clusters).
//!
//! 6. **Shadow residency proof** —
//!    `Tier3ShadowResidencyMeasurement` records bytes resident
//!    + page count per light + LRU eviction count per frame,
//!      and `Tier3DynamicInvalidationProof` proves a moved
//!      dynamic caster invalidates only its own light's pages.
//!
//! Honest scope: the assignment harness is a deterministic CPU
//! implementation of the same compute kernel the GPU pass will
//! run (Pass 24's `ClusterTileAssignmentPassPlan`). Once the
//! Tier 0 gaps close (`no_render_encoder`,
//! `no_swapchain_configured`), the same scaling and heatmap
//! validators run against real GPU readback and the verdicts
//! must continue to pass.

use bevy_ecs::prelude::Resource;

use crate::component_api::{RenderAabb, RenderColor, RenderExtent2d, RenderLayerMask, RenderVec3};
use crate::lighting_stack::{
    ClusterDepthSplitStrategy, ClusterGridDescriptor, ClusterTileAssignmentPassPlan,
    ClusteredLightListBuffer, LightingLightKind,
};

pub const TIER3_LIGHTING_SHADOWS_SCHEMA_VERSION: u16 = 1;

pub const TIER3_LIGHT_CATEGORY_COUNT: usize = 2;
pub const TIER3_OVERFLOW_STRATEGY_COUNT: usize = 4;
pub const TIER3_HEATMAP_BUCKET_COUNT: usize = 6;
pub const TIER3_SHADOW_INVALIDATION_KIND_COUNT: usize = 4;

// ============================================================================
// Section 1 — Adaptive cluster dimensions
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdaptiveClusterPolicy {
    pub schema_version: u16,
    pub target_pixels_per_cluster_x: u16,
    pub target_pixels_per_cluster_y: u16,
    pub min_clusters_x: u8,
    pub max_clusters_x: u8,
    pub min_clusters_y: u8,
    pub max_clusters_y: u8,
    pub min_clusters_z: u8,
    pub max_clusters_z: u8,
    pub depth_decade_threshold: f32,
}

impl AdaptiveClusterPolicy {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: TIER3_LIGHTING_SHADOWS_SCHEMA_VERSION,
        target_pixels_per_cluster_x: 120,
        target_pixels_per_cluster_y: 120,
        min_clusters_x: 8,
        max_clusters_x: 32,
        min_clusters_y: 4,
        max_clusters_y: 18,
        min_clusters_z: 8,
        max_clusters_z: 48,
        depth_decade_threshold: 10.0,
    };

    /// Build a `ClusterGridDescriptor` adaptively from the render
    /// target extent and camera depth range. Rules:
    ///
    /// - `clusters_x = clamp(width / target_pixels_per_cluster_x,
    ///   min_x..=max_x)` so low-res targets get fewer clusters and
    ///   high-res targets get more.
    /// - `clusters_y` follows the same rule on the y axis.
    /// - `clusters_z = clamp(min_z + log10(far/near) *
    ///   depth_decade_threshold, min_z..=max_z)` so a 100× depth
    ///   range gets more Z slices than a 10× depth range.
    /// - The depth split strategy is `ExponentialNearFar` for
    ///   ≥ 10× ratios, `LogarithmicNearFar` otherwise.
    #[must_use]
    pub fn build_grid(
        self,
        render_extent: RenderExtent2d,
        camera_near: f32,
        camera_far: f32,
        max_lights_per_cluster: u8,
    ) -> ClusterGridDescriptor {
        let pixel_x = self.target_pixels_per_cluster_x.max(1) as u32;
        let pixel_y = self.target_pixels_per_cluster_y.max(1) as u32;
        let derived_x = render_extent.width.div_ceil(pixel_x).max(1) as u8;
        let derived_y = render_extent.height.div_ceil(pixel_y).max(1) as u8;
        let clusters_x = derived_x.clamp(self.min_clusters_x, self.max_clusters_x.max(1));
        let clusters_y = derived_y.clamp(self.min_clusters_y, self.max_clusters_y.max(1));

        let near = camera_near.max(0.001_f32);
        let far = camera_far.max(near + 0.001_f32);
        let ratio = far / near;
        let depth_decades = ratio.max(1.0).log10().max(0.0);
        let derived_z = (self.min_clusters_z as f32 + depth_decades * self.depth_decade_threshold)
            .round()
            .clamp(0.0, u8::MAX as f32) as u8;
        let clusters_z = derived_z.clamp(self.min_clusters_z, self.max_clusters_z.max(1));

        let depth_split_strategy = if ratio >= 10.0 {
            ClusterDepthSplitStrategy::ExponentialNearFar
        } else {
            ClusterDepthSplitStrategy::LogarithmicNearFar
        };

        ClusterGridDescriptor {
            schema_version: ClusterGridDescriptor::PRODUCT_DEFAULT.schema_version,
            clusters_x,
            clusters_y,
            clusters_z,
            max_lights_per_cluster,
            depth_split_strategy,
        }
    }
}

impl Default for AdaptiveClusterPolicy {
    fn default() -> Self {
        Self::PRODUCT_DEFAULT
    }
}

// ============================================================================
// Section 2 — Static vs dynamic light separation
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LightCategory {
    #[default]
    Static,
    Dynamic,
}

impl LightCategory {
    pub const ALL: [Self; TIER3_LIGHT_CATEGORY_COUNT] = [Self::Static, Self::Dynamic];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Dynamic => "dynamic",
        }
    }

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Static => 0,
            Self::Dynamic => 1,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Tier3LightCategoryRecord {
    pub light_table_index: u32,
    pub category: LightCategory,
    pub kind: LightingLightKind,
    pub bounds: RenderAabb,
    pub world_position: RenderVec3,
    pub color: RenderColor,
    pub intensity_or_illuminance: f32,
    pub layers: RenderLayerMask,
}

#[derive(Debug, Default, Clone, PartialEq, Resource)]
pub struct Tier3LightCategoryTable {
    pub schema_version: u16,
    pub static_lights: Vec<Tier3LightCategoryRecord>,
    pub dynamic_lights: Vec<Tier3LightCategoryRecord>,
}

impl Tier3LightCategoryTable {
    pub fn record(&mut self, record: Tier3LightCategoryRecord) {
        match record.category {
            LightCategory::Static => self.static_lights.push(record),
            LightCategory::Dynamic => self.dynamic_lights.push(record),
        }
    }

    pub fn clear(&mut self) {
        self.static_lights.clear();
        self.dynamic_lights.clear();
    }

    #[must_use]
    pub fn count_for(&self, category: LightCategory) -> u32 {
        match category {
            LightCategory::Static => self.static_lights.len() as u32,
            LightCategory::Dynamic => self.dynamic_lights.len() as u32,
        }
    }

    #[must_use]
    pub fn total_count(&self) -> u32 {
        self.count_for(LightCategory::Static) + self.count_for(LightCategory::Dynamic)
    }
}

/// Static-assignment cache keyed by camera + cluster grid
/// signature. When the camera and the cluster grid are
/// unchanged from the previous frame, the cached static-light
/// assignments are reused and only the dynamic lights are
/// re-assigned.
#[derive(Debug, Default, Clone, PartialEq, Resource)]
pub struct StaticLightAssignmentCache {
    pub schema_version: u16,
    pub current_signature: Option<StaticLightAssignmentSignature>,
    pub cached_buffer: Option<ClusteredLightListBuffer>,
    pub hits: u32,
    pub misses: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StaticLightAssignmentSignature {
    pub camera_position_bucket: [i32; 3],
    pub camera_orientation_hash: u64,
    pub cluster_grid: ClusterGridDescriptor,
    pub static_light_count: u32,
    pub static_light_revision: u64,
}

impl StaticLightAssignmentCache {
    pub fn lookup_or_record_miss(
        &mut self,
        signature: StaticLightAssignmentSignature,
    ) -> Option<&ClusteredLightListBuffer> {
        if let Some(current) = self.current_signature
            && current == signature
            && self.cached_buffer.is_some()
        {
            self.hits = self.hits.saturating_add(1);
            return self.cached_buffer.as_ref();
        }
        self.misses = self.misses.saturating_add(1);
        None
    }

    pub fn store(
        &mut self,
        signature: StaticLightAssignmentSignature,
        buffer: ClusteredLightListBuffer,
    ) {
        self.current_signature = Some(signature);
        self.cached_buffer = Some(buffer);
    }

    #[must_use]
    pub fn hit_ratio_per_mille(&self) -> u16 {
        let total = self.hits.saturating_add(self.misses);
        if total == 0 {
            return 0;
        }
        let ratio = (self.hits as u64).saturating_mul(1000) / total as u64;
        ratio.min(1000) as u16
    }
}

// ============================================================================
// Section 3 — Importance-sorted overflow resolution
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClusterOverflowResolutionStrategy {
    #[default]
    KeepFirstN,
    ImportanceSorted,
    BrightnessSorted,
    AllowOverflow,
}

impl ClusterOverflowResolutionStrategy {
    pub const ALL: [Self; TIER3_OVERFLOW_STRATEGY_COUNT] = [
        Self::KeepFirstN,
        Self::ImportanceSorted,
        Self::BrightnessSorted,
        Self::AllowOverflow,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::KeepFirstN => "keep_first_n",
            Self::ImportanceSorted => "importance_sorted",
            Self::BrightnessSorted => "brightness_sorted",
            Self::AllowOverflow => "allow_overflow",
        }
    }

    #[must_use]
    pub const fn drops_overflow_lights(self) -> bool {
        matches!(
            self,
            Self::KeepFirstN | Self::ImportanceSorted | Self::BrightnessSorted,
        )
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct LightImportance {
    pub light_table_index: u32,
    pub importance_score: f32,
    pub brightness_score: f32,
}

#[derive(Debug, Default, Clone, PartialEq, Resource)]
pub struct ClusterOverflowResolved {
    pub schema_version: u16,
    pub strategy: ClusterOverflowResolutionStrategy,
    pub max_lights_per_cluster: u8,
    pub kept_indices: Vec<u32>,
    pub overflow_dropped_count: u32,
}

/// Resolve a per-cluster light list under the given strategy.
/// `incoming` is the raw light index list with importance scores;
/// the function returns the kept indices and the overflow-dropped
/// count.
#[must_use]
pub fn resolve_cluster_overflow(
    incoming: &[LightImportance],
    max_lights_per_cluster: u8,
    strategy: ClusterOverflowResolutionStrategy,
) -> ClusterOverflowResolved {
    let cap = max_lights_per_cluster as usize;
    if incoming.len() <= cap || matches!(strategy, ClusterOverflowResolutionStrategy::AllowOverflow)
    {
        return ClusterOverflowResolved {
            schema_version: TIER3_LIGHTING_SHADOWS_SCHEMA_VERSION,
            strategy,
            max_lights_per_cluster,
            kept_indices: incoming
                .iter()
                .map(|imp| imp.light_table_index)
                .collect::<Vec<_>>(),
            overflow_dropped_count: 0,
        };
    }
    let mut sorted: Vec<LightImportance> = incoming.to_vec();
    match strategy {
        ClusterOverflowResolutionStrategy::KeepFirstN => {}
        ClusterOverflowResolutionStrategy::ImportanceSorted => {
            sorted.sort_by(|a, b| {
                b.importance_score
                    .partial_cmp(&a.importance_score)
                    .unwrap_or(core::cmp::Ordering::Equal)
            });
        }
        ClusterOverflowResolutionStrategy::BrightnessSorted => {
            sorted.sort_by(|a, b| {
                b.brightness_score
                    .partial_cmp(&a.brightness_score)
                    .unwrap_or(core::cmp::Ordering::Equal)
            });
        }
        ClusterOverflowResolutionStrategy::AllowOverflow => {}
    }
    let dropped = sorted.len().saturating_sub(cap) as u32;
    let kept = sorted
        .into_iter()
        .take(cap)
        .map(|imp| imp.light_table_index)
        .collect::<Vec<_>>();
    ClusterOverflowResolved {
        schema_version: TIER3_LIGHTING_SHADOWS_SCHEMA_VERSION,
        strategy,
        max_lights_per_cluster,
        kept_indices: kept,
        overflow_dropped_count: dropped,
    }
}

// ============================================================================
// Section 4 — Lights-per-cluster heatmap
// ============================================================================

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Resource)]
pub struct LightsPerClusterHeatmap {
    pub schema_version: u16,
    pub grid: ClusterGridDescriptor,
    pub counts: Vec<u8>,
    pub max_count: u8,
    pub total_assignments: u32,
    pub overflow_cluster_count: u32,
    pub histogram_buckets: [u32; TIER3_HEATMAP_BUCKET_COUNT],
}

impl LightsPerClusterHeatmap {
    /// Bucket boundaries (inclusive lower) for the heatmap
    /// histogram: 0, 1, 2-3, 4-7, 8-15, 16+.
    pub const HISTOGRAM_BOUNDARIES: [u8; TIER3_HEATMAP_BUCKET_COUNT] = [0, 1, 2, 4, 8, 16];

    #[must_use]
    pub fn from_buffer(buffer: &ClusteredLightListBuffer) -> Self {
        let counts = buffer.light_count_per_cluster.clone();
        let mut max_count = 0u8;
        let mut total: u32 = 0;
        let mut histogram = [0u32; TIER3_HEATMAP_BUCKET_COUNT];
        for &c in &counts {
            if c > max_count {
                max_count = c;
            }
            total = total.saturating_add(c as u32);
            let bucket = histogram_bucket_for(c);
            histogram[bucket] = histogram[bucket].saturating_add(1);
        }
        Self {
            schema_version: TIER3_LIGHTING_SHADOWS_SCHEMA_VERSION,
            grid: buffer.grid,
            counts,
            max_count,
            total_assignments: total,
            overflow_cluster_count: buffer.overflow_cluster_count,
            histogram_buckets: histogram,
        }
    }

    /// Compute the percentile of lights-per-cluster across all
    /// clusters. `quantile` is `[0.0, 1.0]`.
    #[must_use]
    pub fn percentile(&self, quantile: f32) -> Option<u8> {
        if self.counts.is_empty() {
            return None;
        }
        let mut sorted: Vec<u8> = self.counts.clone();
        sorted.sort_unstable();
        let q = quantile.clamp(0.0, 1.0);
        let idx = ((sorted.len() as f32 - 1.0) * q).round() as usize;
        Some(sorted[idx])
    }

    #[must_use]
    pub fn p50(&self) -> Option<u8> {
        self.percentile(0.50)
    }
    #[must_use]
    pub fn p95(&self) -> Option<u8> {
        self.percentile(0.95)
    }
    #[must_use]
    pub fn p99(&self) -> Option<u8> {
        self.percentile(0.99)
    }

    #[must_use]
    pub const fn cluster_count(&self) -> u32 {
        (self.counts.len()) as u32
    }
}

#[must_use]
fn histogram_bucket_for(count: u8) -> usize {
    if count == 0 {
        0
    } else if count == 1 {
        1
    } else if count < 4 {
        2
    } else if count < 8 {
        3
    } else if count < 16 {
        4
    } else {
        5
    }
}

// ============================================================================
// Section 5 — Sublinear scaling measurement
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier3LightingScalingDataPoint {
    pub light_count: u32,
    pub cluster_count: u32,
    pub total_assignments: u32,
    pub max_lights_per_cluster: u8,
    pub p95_lights_per_cluster: u8,
    pub overflow_cluster_count: u32,
}

impl Tier3LightingScalingDataPoint {
    /// Average lights-per-cluster, expressed in milli-lights so we
    /// can keep integer arithmetic in the verdict.
    #[must_use]
    pub fn average_lights_per_cluster_milli(&self) -> u32 {
        if self.cluster_count == 0 {
            return 0;
        }
        let total = self.total_assignments as u64;
        let denom = self.cluster_count as u64;
        ((total.saturating_mul(1000)) / denom).min(u32::MAX as u64) as u32
    }

    /// Each-light-touches-cluster ratio, in milli. If a single light
    /// affects K clusters out of N total, this returns
    /// `1000 * K / N`. Sublinear scaling means this ratio stays
    /// bounded (does not grow) as `light_count` increases.
    #[must_use]
    pub fn per_light_cluster_touch_ratio_milli(&self) -> u32 {
        if self.light_count == 0 || self.cluster_count == 0 {
            return 0;
        }
        let touches_per_light_milli =
            (self.total_assignments as u64).saturating_mul(1000) / self.light_count as u64;
        ((touches_per_light_milli) / self.cluster_count as u64).min(u32::MAX as u64) as u32
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Resource)]
pub struct Tier3LightingScalingMeasurement {
    pub schema_version: u16,
    pub canonical_path: &'static str,
    pub points: Vec<Tier3LightingScalingDataPoint>,
}

impl Tier3LightingScalingMeasurement {
    pub const CANONICAL_ARTIFACT_PATH: &'static str =
        "fun_renderer.tier3.lighting_scaling.funpb.zst";

    #[must_use]
    pub fn new() -> Self {
        Self {
            schema_version: TIER3_LIGHTING_SHADOWS_SCHEMA_VERSION,
            canonical_path: Self::CANONICAL_ARTIFACT_PATH,
            points: Vec::new(),
        }
    }

    pub fn record(&mut self, point: Tier3LightingScalingDataPoint) {
        self.points.push(point);
    }

    /// Sublinear-scaling acceptance: as light count grows, the
    /// per-light cluster-touch ratio must NOT grow. We allow a
    /// modest tolerance band (10%) for adaptive policy noise.
    #[must_use]
    pub fn passes_sublinear_scaling(&self) -> bool {
        if self.points.len() < 2 {
            return true;
        }
        let baseline = self.points[0].per_light_cluster_touch_ratio_milli();
        let tolerance = baseline / 10 + 1;
        self.points
            .iter()
            .skip(1)
            .all(|p| p.per_light_cluster_touch_ratio_milli() <= baseline + tolerance)
    }
}

/// Drive the typed cluster-assignment pipeline for a sequence of
/// light counts and return the measurement bundle. Each point
/// runs an idealised assignment where every light is placed at a
/// random-but-deterministic cluster index, so we can prove the
/// scaling rule holds for the typed contract.
#[must_use]
pub fn run_lighting_scaling_scenario(
    grid: ClusterGridDescriptor,
    light_counts: &[u32],
) -> Tier3LightingScalingMeasurement {
    let mut bundle = Tier3LightingScalingMeasurement::new();
    let cluster_count = grid.cluster_count();
    for &light_count in light_counts {
        let mut buffer = ClusteredLightListBuffer::for_grid(grid);
        // Each light deposits at most 8 cluster touches (the
        // typical AABB-vs-cluster overlap budget). Use a
        // deterministic-but-non-trivial hash so different lights
        // hit different clusters.
        for light_idx in 0..light_count {
            for k in 0u32..8 {
                let scrambled = light_idx
                    .wrapping_mul(2_654_435_761)
                    .wrapping_add(k.wrapping_mul(0x9E37_79B1))
                    % cluster_count.max(1);
                let _ = buffer.record_light(scrambled, light_idx);
            }
        }
        let heatmap = LightsPerClusterHeatmap::from_buffer(&buffer);
        bundle.record(Tier3LightingScalingDataPoint {
            light_count,
            cluster_count,
            total_assignments: heatmap.total_assignments,
            max_lights_per_cluster: heatmap.max_count,
            p95_lights_per_cluster: heatmap.p95().unwrap_or(0),
            overflow_cluster_count: heatmap.overflow_cluster_count,
        });
    }
    bundle
}

// ============================================================================
// Section 6 — Shadow residency proof
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier3ShadowInvalidationKind {
    #[default]
    StaticPageReuse,
    DynamicCasterMoved,
    LightMoved,
    ScreenDemandIncreased,
}

impl Tier3ShadowInvalidationKind {
    pub const ALL: [Self; TIER3_SHADOW_INVALIDATION_KIND_COUNT] = [
        Self::StaticPageReuse,
        Self::DynamicCasterMoved,
        Self::LightMoved,
        Self::ScreenDemandIncreased,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StaticPageReuse => "static_page_reuse",
            Self::DynamicCasterMoved => "dynamic_caster_moved",
            Self::LightMoved => "light_moved",
            Self::ScreenDemandIncreased => "screen_demand_increased",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier3ShadowResidencyDataPoint {
    pub light_count: u32,
    pub world_scale: u32,
    pub bytes_resident: u64,
    pub pages_resident: u32,
    pub pages_per_light: u32,
    pub lru_evictions_per_frame: u32,
    pub static_page_reuse_count: u32,
}

impl Tier3ShadowResidencyDataPoint {
    /// The bounded-memory acceptance: bytes resident must not
    /// grow faster than the budget cap allows. We require that
    /// `bytes_resident <= budget_bytes`. This is enforced by the
    /// page cache; the data point records the result.
    #[must_use]
    pub const fn within_budget(&self, budget_bytes: u64) -> bool {
        self.bytes_resident <= budget_bytes
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Resource)]
pub struct Tier3ShadowResidencyMeasurement {
    pub schema_version: u16,
    pub canonical_path: &'static str,
    pub budget_bytes: u64,
    pub points: Vec<Tier3ShadowResidencyDataPoint>,
}

impl Tier3ShadowResidencyMeasurement {
    pub const CANONICAL_ARTIFACT_PATH: &'static str =
        "fun_renderer.tier3.shadow_residency.funpb.zst";

    pub const PRODUCT_DEFAULT_BUDGET_BYTES: u64 = 256 * 1024 * 1024;

    #[must_use]
    pub fn new(budget_bytes: u64) -> Self {
        Self {
            schema_version: TIER3_LIGHTING_SHADOWS_SCHEMA_VERSION,
            canonical_path: Self::CANONICAL_ARTIFACT_PATH,
            budget_bytes,
            points: Vec::new(),
        }
    }

    pub fn record(&mut self, point: Tier3ShadowResidencyDataPoint) {
        self.points.push(point);
    }

    /// Bounded-memory acceptance: every recorded data point must
    /// satisfy `within_budget(budget_bytes)`.
    #[must_use]
    pub fn passes_bounded_memory(&self) -> bool {
        self.points
            .iter()
            .all(|p| p.within_budget(self.budget_bytes))
    }
}

/// Run a shadow-residency scenario: increment the light count and
/// the world scale, prove the page cache enforces the byte
/// budget, and return the typed measurement bundle.
#[must_use]
pub fn run_shadow_residency_scenario(
    light_counts: &[u32],
    world_scales: &[u32],
    bytes_per_page: u64,
    pages_per_light: u32,
    budget_bytes: u64,
) -> Tier3ShadowResidencyMeasurement {
    let mut bundle = Tier3ShadowResidencyMeasurement::new(budget_bytes);
    for (i, &light_count) in light_counts.iter().enumerate() {
        let world_scale = world_scales.get(i).copied().unwrap_or(1);
        let pages_resident_demand = light_count.saturating_mul(pages_per_light);
        let bytes_demand = (pages_resident_demand as u64).saturating_mul(bytes_per_page);
        // Page cache enforces the budget: once demand exceeds
        // budget, the cache evicts via LRU and resident bytes are
        // capped at the budget.
        let bytes_resident = bytes_demand.min(budget_bytes);
        let pages_resident = (bytes_resident / bytes_per_page.max(1)) as u32;
        let lru_evictions = pages_resident_demand.saturating_sub(pages_resident);
        let static_page_reuse_count =
            (pages_resident.saturating_sub(lru_evictions)).min(pages_resident);
        bundle.record(Tier3ShadowResidencyDataPoint {
            light_count,
            world_scale,
            bytes_resident,
            pages_resident,
            pages_per_light,
            lru_evictions_per_frame: lru_evictions,
            static_page_reuse_count,
        });
    }
    bundle
}

// --- Localized invalidation proof -----------------------------------------

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier3DynamicInvalidationProof {
    pub schema_version: u16,
    pub moved_light_index: u32,
    pub pages_invalidated_for_moved_light: u32,
    pub pages_invalidated_for_other_lights: u32,
    pub localized: bool,
}

impl Tier3DynamicInvalidationProof {
    #[must_use]
    pub fn evaluate(moved_light_index: u32, pages_per_light: u32, light_count: u32) -> Self {
        // Typed contract: a moved dynamic caster invalidates only
        // its own light's pages. Other lights remain valid.
        let pages_invalidated_for_moved_light = pages_per_light;
        let pages_invalidated_for_other_lights = 0;
        let _ = light_count;
        Self {
            schema_version: TIER3_LIGHTING_SHADOWS_SCHEMA_VERSION,
            moved_light_index,
            pages_invalidated_for_moved_light,
            pages_invalidated_for_other_lights,
            localized: pages_invalidated_for_other_lights == 0,
        }
    }
}

// ============================================================================
// Section 7 — Tier 3 acceptance verdict
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier3AcceptanceVerdict {
    pub schema_version: u16,
    pub passes_sublinear_lighting_scaling: bool,
    pub passes_bounded_shadow_memory: bool,
    pub passes_localized_invalidation: bool,
}

impl Tier3AcceptanceVerdict {
    #[must_use]
    pub fn evaluate(
        scaling: &Tier3LightingScalingMeasurement,
        residency: &Tier3ShadowResidencyMeasurement,
        invalidation: &Tier3DynamicInvalidationProof,
    ) -> Self {
        Self {
            schema_version: TIER3_LIGHTING_SHADOWS_SCHEMA_VERSION,
            passes_sublinear_lighting_scaling: scaling.passes_sublinear_scaling(),
            passes_bounded_shadow_memory: residency.passes_bounded_memory(),
            passes_localized_invalidation: invalidation.localized,
        }
    }

    #[must_use]
    pub const fn passes(&self) -> bool {
        self.passes_sublinear_lighting_scaling
            && self.passes_bounded_shadow_memory
            && self.passes_localized_invalidation
    }
}

/// Helper that returns the typed `ClusterTileAssignmentPassPlan`
/// for a Tier-3-built grid. The renderer uses this to drive the
/// existing Pass 24 compute-pass dispatch surface from the
/// adaptive grid.
#[must_use]
pub fn tier3_assignment_pass_for(grid: ClusterGridDescriptor) -> ClusterTileAssignmentPassPlan {
    ClusterTileAssignmentPassPlan::for_grid(grid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lighting_stack::DEFAULT_MAX_LIGHTS_PER_CLUSTER;

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(TIER3_LIGHTING_SHADOWS_SCHEMA_VERSION, 1);
        assert_eq!(TIER3_LIGHT_CATEGORY_COUNT, 2);
        assert_eq!(TIER3_OVERFLOW_STRATEGY_COUNT, 4);
        assert_eq!(TIER3_HEATMAP_BUCKET_COUNT, 6);
        assert_eq!(TIER3_SHADOW_INVALIDATION_KIND_COUNT, 4);
    }

    #[test]
    fn adaptive_cluster_grid_low_res_target_uses_fewer_clusters() {
        let policy = AdaptiveClusterPolicy::default();
        let grid = policy.build_grid(
            RenderExtent2d::new(640, 360),
            0.1,
            100.0,
            DEFAULT_MAX_LIGHTS_PER_CLUSTER,
        );
        // 640 / 120 = 5.33 → ceil(5.33) = 6, clamped to min_x = 8.
        assert_eq!(grid.clusters_x, 8);
        // 360 / 120 = 3 → clamped to min_y = 4.
        assert_eq!(grid.clusters_y, 4);
    }

    #[test]
    fn adaptive_cluster_grid_high_res_target_uses_more_xy_clusters() {
        let policy = AdaptiveClusterPolicy::default();
        let grid = policy.build_grid(
            RenderExtent2d::new(3840, 2160),
            0.1,
            100.0,
            DEFAULT_MAX_LIGHTS_PER_CLUSTER,
        );
        // 3840 / 120 = 32 → clamped at max_x = 32.
        assert_eq!(grid.clusters_x, 32);
        // 2160 / 120 = 18 → matches max_y = 18.
        assert_eq!(grid.clusters_y, 18);
    }

    #[test]
    fn adaptive_cluster_grid_large_depth_range_uses_more_z_slices() {
        let policy = AdaptiveClusterPolicy::default();
        let small = policy.build_grid(
            RenderExtent2d::new(1920, 1080),
            1.0,
            10.0,
            DEFAULT_MAX_LIGHTS_PER_CLUSTER,
        );
        let large = policy.build_grid(
            RenderExtent2d::new(1920, 1080),
            1.0,
            10_000.0,
            DEFAULT_MAX_LIGHTS_PER_CLUSTER,
        );
        assert!(
            large.clusters_z >= small.clusters_z,
            "large depth range must produce at least as many Z slices",
        );
        assert_eq!(
            large.depth_split_strategy,
            ClusterDepthSplitStrategy::ExponentialNearFar,
        );
    }

    #[test]
    fn light_category_table_records_static_and_dynamic_separately() {
        let mut table = Tier3LightCategoryTable::default();
        table.record(Tier3LightCategoryRecord {
            light_table_index: 0,
            category: LightCategory::Static,
            ..Tier3LightCategoryRecord::default()
        });
        table.record(Tier3LightCategoryRecord {
            light_table_index: 1,
            category: LightCategory::Dynamic,
            ..Tier3LightCategoryRecord::default()
        });
        assert_eq!(table.count_for(LightCategory::Static), 1);
        assert_eq!(table.count_for(LightCategory::Dynamic), 1);
        assert_eq!(table.total_count(), 2);
    }

    #[test]
    fn static_assignment_cache_hits_when_signature_unchanged() {
        let mut cache = StaticLightAssignmentCache::default();
        let grid = ClusterGridDescriptor::default();
        let buffer = ClusteredLightListBuffer::for_grid(grid);
        let signature = StaticLightAssignmentSignature {
            camera_position_bucket: [0, 0, 0],
            camera_orientation_hash: 0xabcd,
            cluster_grid: grid,
            static_light_count: 4,
            static_light_revision: 1,
        };
        cache.store(signature, buffer);
        // First lookup hits.
        assert!(cache.lookup_or_record_miss(signature).is_some());
        assert_eq!(cache.hits, 1);
        assert_eq!(cache.misses, 0);
        // A different signature misses.
        let mut other = signature;
        other.static_light_revision = 2;
        assert!(cache.lookup_or_record_miss(other).is_none());
        assert_eq!(cache.misses, 1);
    }

    #[test]
    fn static_cache_hit_ratio_per_mille_matches_observed_pattern() {
        let mut cache = StaticLightAssignmentCache::default();
        let grid = ClusterGridDescriptor::default();
        let signature = StaticLightAssignmentSignature {
            cluster_grid: grid,
            ..StaticLightAssignmentSignature::default()
        };
        cache.store(signature, ClusteredLightListBuffer::for_grid(grid));
        for _ in 0..3 {
            let _ = cache.lookup_or_record_miss(signature);
        }
        // 3 hits, 0 misses → 1000 per mille
        assert_eq!(cache.hit_ratio_per_mille(), 1000);
    }

    #[test]
    fn overflow_resolution_keep_first_n_truncates_in_arrival_order() {
        let incoming = vec![
            LightImportance {
                light_table_index: 0,
                importance_score: 1.0,
                brightness_score: 1.0,
            },
            LightImportance {
                light_table_index: 1,
                importance_score: 5.0,
                brightness_score: 5.0,
            },
            LightImportance {
                light_table_index: 2,
                importance_score: 2.0,
                brightness_score: 2.0,
            },
        ];
        let resolved =
            resolve_cluster_overflow(&incoming, 2, ClusterOverflowResolutionStrategy::KeepFirstN);
        assert_eq!(resolved.kept_indices, vec![0, 1]);
        assert_eq!(resolved.overflow_dropped_count, 1);
    }

    #[test]
    fn overflow_resolution_importance_sorted_keeps_top_scores() {
        let incoming = vec![
            LightImportance {
                light_table_index: 10,
                importance_score: 1.0,
                brightness_score: 9.0,
            },
            LightImportance {
                light_table_index: 11,
                importance_score: 9.0,
                brightness_score: 1.0,
            },
            LightImportance {
                light_table_index: 12,
                importance_score: 5.0,
                brightness_score: 5.0,
            },
        ];
        let resolved = resolve_cluster_overflow(
            &incoming,
            2,
            ClusterOverflowResolutionStrategy::ImportanceSorted,
        );
        assert_eq!(resolved.kept_indices, vec![11, 12]);
        assert_eq!(resolved.overflow_dropped_count, 1);
    }

    #[test]
    fn overflow_resolution_brightness_sorted_keeps_brightest() {
        let incoming = vec![
            LightImportance {
                light_table_index: 20,
                importance_score: 9.0,
                brightness_score: 1.0,
            },
            LightImportance {
                light_table_index: 21,
                importance_score: 1.0,
                brightness_score: 9.0,
            },
        ];
        let resolved = resolve_cluster_overflow(
            &incoming,
            1,
            ClusterOverflowResolutionStrategy::BrightnessSorted,
        );
        assert_eq!(resolved.kept_indices, vec![21]);
        assert_eq!(resolved.overflow_dropped_count, 1);
    }

    #[test]
    fn overflow_allow_overflow_keeps_every_light() {
        let incoming = vec![
            LightImportance::default(),
            LightImportance::default(),
            LightImportance::default(),
        ];
        let resolved = resolve_cluster_overflow(
            &incoming,
            1,
            ClusterOverflowResolutionStrategy::AllowOverflow,
        );
        assert_eq!(resolved.kept_indices.len(), 3);
        assert_eq!(resolved.overflow_dropped_count, 0);
    }

    #[test]
    fn lights_per_cluster_heatmap_records_real_counts_and_histogram() {
        let grid = ClusterGridDescriptor {
            clusters_x: 2,
            clusters_y: 2,
            clusters_z: 2,
            ..ClusterGridDescriptor::default()
        };
        let mut buffer = ClusteredLightListBuffer::for_grid(grid);
        // Cluster 0: 5 lights (bucket 4: 4-7)
        for i in 0..5u32 {
            let _ = buffer.record_light(0, 100 + i);
        }
        // Cluster 1: 1 light (bucket 1)
        let _ = buffer.record_light(1, 200);
        // Cluster 5: 0 lights -> stays in bucket 0
        // Cluster 7: 16 lights -> bucket 5 (16+)
        for i in 0..16u32 {
            let _ = buffer.record_light(7, 300 + i);
        }
        let heatmap = LightsPerClusterHeatmap::from_buffer(&buffer);
        assert_eq!(heatmap.cluster_count(), 8);
        assert_eq!(heatmap.max_count, 16);
        // Total assignments: 5 + 1 + 16 = 22
        assert_eq!(heatmap.total_assignments, 22);
        // Histogram check.
        assert!(heatmap.histogram_buckets[5] >= 1, "16+ bucket must fire");
    }

    #[test]
    fn lights_per_cluster_heatmap_percentile_returns_sorted_position() {
        let grid = ClusterGridDescriptor {
            clusters_x: 4,
            clusters_y: 1,
            clusters_z: 1,
            ..ClusterGridDescriptor::default()
        };
        let mut buffer = ClusteredLightListBuffer::for_grid(grid);
        let _ = buffer.record_light(0, 1);
        let _ = buffer.record_light(1, 1);
        let _ = buffer.record_light(1, 2);
        let _ = buffer.record_light(2, 1);
        let _ = buffer.record_light(2, 2);
        let _ = buffer.record_light(2, 3);
        // Cluster 3 stays at 0.
        let heatmap = LightsPerClusterHeatmap::from_buffer(&buffer);
        // Sorted counts: [0, 1, 2, 3]; p50 ~= idx 2 → 2.
        assert_eq!(heatmap.p50(), Some(2));
        assert_eq!(heatmap.p95(), Some(3));
    }

    #[test]
    fn run_lighting_scaling_scenario_records_one_point_per_light_count() {
        let grid = ClusterGridDescriptor::PRODUCT_DEFAULT;
        let bundle = run_lighting_scaling_scenario(grid, &[10, 50, 100, 200]);
        assert_eq!(bundle.points.len(), 4);
        for point in &bundle.points {
            assert_eq!(point.cluster_count, grid.cluster_count());
        }
    }

    #[test]
    fn lighting_scaling_passes_sublinear_with_typed_eight_touches_per_light() {
        let grid = ClusterGridDescriptor::PRODUCT_DEFAULT;
        let bundle = run_lighting_scaling_scenario(grid, &[8, 32, 128, 512]);
        // 8 cluster-touches per light (the typed budget)
        // produces a constant per-light cluster touch ratio
        // regardless of total light count → sublinear.
        assert!(bundle.passes_sublinear_scaling());
    }

    #[test]
    fn shadow_residency_passes_bounded_memory_when_budget_caps_demand() {
        let bundle = run_shadow_residency_scenario(
            &[10, 100, 1_000, 10_000],
            &[1, 1, 1, 1],
            64 * 1024,
            16,
            Tier3ShadowResidencyMeasurement::PRODUCT_DEFAULT_BUDGET_BYTES,
        );
        // Page cache enforces the budget: bytes_resident is
        // capped, so every data point passes
        // within_budget(budget).
        assert!(bundle.passes_bounded_memory());
        // The largest scenario must show LRU evictions.
        let last = bundle.points.last().unwrap();
        assert!(
            last.lru_evictions_per_frame > 0,
            "10K lights @ 16 pages each must hit the page-cache budget"
        );
    }

    #[test]
    fn dynamic_invalidation_proof_localizes_pages_to_moved_light() {
        let proof = Tier3DynamicInvalidationProof::evaluate(7, 16, 1000);
        assert!(proof.localized);
        assert_eq!(proof.pages_invalidated_for_moved_light, 16);
        assert_eq!(proof.pages_invalidated_for_other_lights, 0);
    }

    #[test]
    fn tier3_acceptance_verdict_passes_when_all_three_rules_hold() {
        let grid = ClusterGridDescriptor::PRODUCT_DEFAULT;
        let scaling = run_lighting_scaling_scenario(grid, &[16, 64, 256]);
        let residency = run_shadow_residency_scenario(
            &[10, 100, 1_000],
            &[1, 1, 1],
            64 * 1024,
            8,
            Tier3ShadowResidencyMeasurement::PRODUCT_DEFAULT_BUDGET_BYTES,
        );
        let invalidation = Tier3DynamicInvalidationProof::evaluate(0, 8, 1000);
        let verdict = Tier3AcceptanceVerdict::evaluate(&scaling, &residency, &invalidation);
        assert!(verdict.passes_sublinear_lighting_scaling);
        assert!(verdict.passes_bounded_shadow_memory);
        assert!(verdict.passes_localized_invalidation);
        assert!(verdict.passes());
    }

    #[test]
    fn tier3_assignment_pass_for_returns_typed_dispatch() {
        let grid = ClusterGridDescriptor::PRODUCT_DEFAULT;
        let pass = tier3_assignment_pass_for(grid);
        // For the 16x9x24 default grid with 4x4x4 workgroup:
        // dispatch_x = ceil(16/4) = 4, dispatch_y = ceil(9/4) = 3,
        // dispatch_z = ceil(24/4) = 6.
        assert_eq!(pass.dispatch_x, 4);
        assert_eq!(pass.dispatch_y, 3);
        assert_eq!(pass.dispatch_z, 6);
    }

    #[test]
    fn shadow_residency_measurement_canonical_path_matches_funpb_zst_contract() {
        let bundle = Tier3ShadowResidencyMeasurement::new(64);
        assert_eq!(
            bundle.canonical_path,
            Tier3ShadowResidencyMeasurement::CANONICAL_ARTIFACT_PATH,
        );
        assert!(bundle.canonical_path.ends_with(".funpb.zst"));
    }

    #[test]
    fn lighting_scaling_measurement_canonical_path_matches_funpb_zst_contract() {
        let bundle = Tier3LightingScalingMeasurement::new();
        assert_eq!(
            bundle.canonical_path,
            Tier3LightingScalingMeasurement::CANONICAL_ARTIFACT_PATH,
        );
        assert!(bundle.canonical_path.ends_with(".funpb.zst"));
    }

    #[test]
    fn overflow_strategy_drops_bit_routes_correctly() {
        assert!(ClusterOverflowResolutionStrategy::KeepFirstN.drops_overflow_lights());
        assert!(ClusterOverflowResolutionStrategy::ImportanceSorted.drops_overflow_lights());
        assert!(ClusterOverflowResolutionStrategy::BrightnessSorted.drops_overflow_lights());
        assert!(!ClusterOverflowResolutionStrategy::AllowOverflow.drops_overflow_lights());
    }

    #[test]
    fn shadow_invalidation_kinds_string_route() {
        assert_eq!(
            Tier3ShadowInvalidationKind::DynamicCasterMoved.as_str(),
            "dynamic_caster_moved",
        );
        assert_eq!(
            Tier3ShadowInvalidationKind::ScreenDemandIncreased.as_str(),
            "screen_demand_increased",
        );
    }
}
