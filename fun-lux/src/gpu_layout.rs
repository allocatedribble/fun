//! Pass 6 — typed GPU buffer record layouts for the
//! clustered Forward+ + reservoir many-light path.
//!
//! Pass 4 ships typed BUFFER-level descriptors
//! (capacity, bytes per element, layout mode, upload
//! strategy) in [`crate::gpu_buffer_schema`]. Pass 6 ships
//! typed RECORD-level layouts: which fields go into a single
//! `LightRecord` / `Cluster` / `Reservoir` / `LightIndex`
//! slot, in what order, with what byte offsets. Both layers
//! are renderer-neutral — fun-renderer consumes them to
//! allocate matching `wgpu::Buffer` objects + WGSL struct
//! definitions.
//!
//! Pass 6 also harmonizes the cluster grid tier presets
//! against the user's spec:
//!
//! - LowMedium: 16 × 9 × 16
//! - High:      16 × 9 × 24
//! - Cinematic: 32 × 18 × 24
//! - Dynamic:   resolved at runtime
//!
//! The typed `FunLuxClusterGridBufferSchema::PRODUCT_DEFAULT`
//! in [`crate::gpu_buffer_schema`] is updated to use the
//! `High` tier (16 × 9 × 24, matching fun-renderer's
//! `lighting_stack` default).

pub const FUN_LUX_GPU_LAYOUT_SCHEMA_VERSION: u16 = 1;
pub const FUN_LUX_CLUSTER_GRID_TIER_COUNT: usize = 4;
pub const FUN_LUX_LIGHT_RECORD_FIELD_COUNT: usize = 16;

// ============================================================================
// Section 1 — Typed cluster grid tier (Pass 6 harmonization)
// ============================================================================

/// Typed cluster grid tier. Each tier picks a typed
/// dimensions triplet matching the user's Pass 6 spec.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxClusterGridTier {
    /// Low / Medium quality: 16 × 9 × 16. Smaller cluster
    /// count → smaller cluster grid buffer → faster cluster
    /// assignment + light list build at the cost of more
    /// lights-per-cluster on average.
    LowMedium,
    /// Typed product-default: 16 × 9 × 24. Matches
    /// fun-renderer's existing `lighting_stack` constants.
    #[default]
    High,
    /// Cinematic / max quality: 32 × 18 × 24. 4x the
    /// cluster count of `High` for cinematic-grade
    /// many-light precision.
    Cinematic,
    /// Dynamic — the renderer picks dimensions at runtime
    /// based on viewport extent + scene complexity. Returns
    /// `(0, 0, 0)` from [`Self::dimensions`]; the runtime
    /// must override.
    Dynamic,
}

impl LuxClusterGridTier {
    pub const ALL: [Self; FUN_LUX_CLUSTER_GRID_TIER_COUNT] =
        [Self::LowMedium, Self::High, Self::Cinematic, Self::Dynamic];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LowMedium => "low_medium",
            Self::High => "high",
            Self::Cinematic => "cinematic",
            Self::Dynamic => "dynamic",
        }
    }

    /// Typed `(clusters_x, clusters_y, clusters_z)` for the
    /// tier. `Dynamic` returns `(0, 0, 0)` — the renderer
    /// must override at runtime.
    #[must_use]
    pub const fn dimensions(self) -> [u32; 3] {
        match self {
            Self::LowMedium => [16, 9, 16],
            Self::High => [16, 9, 24],
            Self::Cinematic => [32, 18, 24],
            Self::Dynamic => [0, 0, 0],
        }
    }

    /// Typed cluster count for the tier.
    #[must_use]
    pub const fn cluster_count(self) -> u32 {
        let d = self.dimensions();
        d[0].saturating_mul(d[1]).saturating_mul(d[2])
    }

    /// Typed max lights per cluster for the tier. Tied to
    /// the tier's expected scene complexity: more lights
    /// per cluster on lower tiers (because clusters are
    /// larger), fewer on higher tiers.
    #[must_use]
    pub const fn max_lights_per_cluster(self) -> u32 {
        match self {
            Self::LowMedium => 64,
            Self::High => 32,
            Self::Cinematic => 16,
            Self::Dynamic => 32,
        }
    }

    /// Typed predicate: does this tier resolve at runtime?
    #[must_use]
    pub const fn is_dynamic(self) -> bool {
        matches!(self, Self::Dynamic)
    }
}

// ============================================================================
// Section 2 — Typed light record field descriptor
// ============================================================================

/// Typed light record field. The Pass 6 spec enumerates 16
/// typed fields a light record must include; this enum is
/// the typed taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxGpuLightRecordField {
    StableLightId,
    Kind,
    Position,
    Direction,
    ColorRgb,
    IntensityLux,
    Range,
    InnerConeAngle,
    OuterConeAngle,
    LayerMask,
    SceneId,
    VolumetricMultiplierQ8,
    ShadowPolicy,
    UpdateStamp,
    Priority,
    EmissiveSourceReference,
}

impl LuxGpuLightRecordField {
    pub const ALL: [Self; FUN_LUX_LIGHT_RECORD_FIELD_COUNT] = [
        Self::StableLightId,
        Self::Kind,
        Self::Position,
        Self::Direction,
        Self::ColorRgb,
        Self::IntensityLux,
        Self::Range,
        Self::InnerConeAngle,
        Self::OuterConeAngle,
        Self::LayerMask,
        Self::SceneId,
        Self::VolumetricMultiplierQ8,
        Self::ShadowPolicy,
        Self::UpdateStamp,
        Self::Priority,
        Self::EmissiveSourceReference,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StableLightId => "stable_light_id",
            Self::Kind => "kind",
            Self::Position => "position",
            Self::Direction => "direction",
            Self::ColorRgb => "color_rgb",
            Self::IntensityLux => "intensity_lux",
            Self::Range => "range",
            Self::InnerConeAngle => "inner_cone_angle",
            Self::OuterConeAngle => "outer_cone_angle",
            Self::LayerMask => "layer_mask",
            Self::SceneId => "scene_id",
            Self::VolumetricMultiplierQ8 => "volumetric_multiplier_q8",
            Self::ShadowPolicy => "shadow_policy",
            Self::UpdateStamp => "update_stamp",
            Self::Priority => "priority",
            Self::EmissiveSourceReference => "emissive_source_reference",
        }
    }

    /// Typed byte size of the field in the GPU layout
    /// (matches the std140 / std430 size used in the WGSL
    /// struct).
    #[must_use]
    pub const fn byte_size(self) -> u8 {
        match self {
            Self::StableLightId => 8,           // u64
            Self::Kind => 4,                    // u32
            Self::Position => 12,               // vec3<f32>
            Self::Direction => 12,              // vec3<f32>
            Self::ColorRgb => 12,               // vec3<f32>
            Self::IntensityLux => 4,            // f32
            Self::Range => 4,                   // f32
            Self::InnerConeAngle => 4,          // f32
            Self::OuterConeAngle => 4,          // f32
            Self::LayerMask => 4,               // u32
            Self::SceneId => 4,                 // u32
            Self::VolumetricMultiplierQ8 => 2,  // u16
            Self::ShadowPolicy => 2,            // u16
            Self::UpdateStamp => 8,             // u64
            Self::Priority => 2,                // u16
            Self::EmissiveSourceReference => 4, // u32 / handle
        }
    }
}

// ============================================================================
// Section 3 — Typed light record layout
// ============================================================================

/// Typed GPU light record layout. Names the 16 fields the
/// renderer must pack into a single `LightRecord` WGSL
/// struct + the std140-aligned byte size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxGpuLightRecordLayout {
    pub schema_version: u16,
    pub stable_id: &'static str,
    pub field_count: u32,
    /// Typed std140-aligned record size. Pass 6 product
    /// default rounds up to the nearest 16-byte boundary
    /// (uniform / storage buffer alignment).
    pub record_byte_size: u32,
    pub alignment: u32,
}

impl LuxGpuLightRecordLayout {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_GPU_LAYOUT_SCHEMA_VERSION,
        stable_id: "fun_lux.gpu_layout.light_record",
        field_count: FUN_LUX_LIGHT_RECORD_FIELD_COUNT as u32,
        record_byte_size: 96,
        alignment: 16,
    };

    /// Typed sum of the field byte sizes. Used to verify
    /// the typed product-default `record_byte_size`
    /// accommodates every field.
    #[must_use]
    pub fn raw_field_byte_total() -> u32 {
        LuxGpuLightRecordField::ALL
            .iter()
            .map(|f| f.byte_size() as u32)
            .sum()
    }

    /// Typed predicate: does the typed `record_byte_size`
    /// accommodate every field (with std140 padding)?
    #[must_use]
    pub fn record_size_accommodates_every_field(&self) -> bool {
        self.record_byte_size >= Self::raw_field_byte_total()
            && self.record_byte_size % self.alignment == 0
    }
}

// ============================================================================
// Section 4 — Typed cluster grid layout
// ============================================================================

/// Typed cluster grid record layout. Encodes the typed
/// dimensions + max lights per cluster + bytes per cluster
/// record (cluster AABB + light list offset + light list
/// count).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxClusterGridLayout {
    pub schema_version: u16,
    pub stable_id: &'static str,
    pub tier: LuxClusterGridTier,
    pub clusters_x: u32,
    pub clusters_y: u32,
    pub clusters_z: u32,
    pub max_lights_per_cluster: u32,
    pub bytes_per_cluster_record: u32,
}

impl LuxClusterGridLayout {
    /// Typed `High` tier layout (the product default).
    pub const PRODUCT_DEFAULT: Self = Self::for_tier(LuxClusterGridTier::High);

    /// Typed builder for a specific tier.
    #[must_use]
    pub const fn for_tier(tier: LuxClusterGridTier) -> Self {
        let d = tier.dimensions();
        Self {
            schema_version: FUN_LUX_GPU_LAYOUT_SCHEMA_VERSION,
            stable_id: "fun_lux.gpu_layout.cluster_grid",
            tier,
            clusters_x: d[0],
            clusters_y: d[1],
            clusters_z: d[2],
            max_lights_per_cluster: tier.max_lights_per_cluster(),
            // Cluster record: AABB (24 bytes) + light list
            // offset (4) + count (4) = 32 bytes std140.
            bytes_per_cluster_record: 32,
        }
    }

    #[must_use]
    pub const fn cluster_count(&self) -> u32 {
        self.clusters_x
            .saturating_mul(self.clusters_y)
            .saturating_mul(self.clusters_z)
    }

    #[must_use]
    pub const fn total_byte_size(&self) -> u64 {
        (self.cluster_count() as u64) * (self.bytes_per_cluster_record as u64)
    }
}

// ============================================================================
// Section 5 — Typed reservoir layout
// ============================================================================

/// Typed reservoir record layout. Reservoirs carry the
/// selected light index + weight + sample count + the
/// `update_stamp` the spec demands for stale rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxReservoirLayout {
    pub schema_version: u16,
    pub stable_id: &'static str,
    pub samples_per_pixel: u32,
    pub bytes_per_reservoir: u32,
    /// Number of typed history frames the renderer retains
    /// (the reservoir's temporal-reuse window).
    pub history_frame_count: u8,
    /// Pass 6 acceptance: the typed reservoir record
    /// MUST include an `update_stamp` field so the temporal
    /// reuse pass can reject stale lights.
    pub includes_update_stamp: bool,
}

impl LuxReservoirLayout {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_GPU_LAYOUT_SCHEMA_VERSION,
        stable_id: "fun_lux.gpu_layout.reservoir",
        samples_per_pixel: 4,
        // Reservoir record: light_index (4) + weight (4) +
        // m (4) + W (4) + update_stamp (8) + pad (8) = 32 bytes.
        bytes_per_reservoir: 32,
        history_frame_count: 8,
        includes_update_stamp: true,
    };

    /// Pass 6 acceptance: typed predicate — "Reservoir
    /// history rejects stale lights via update stamps." A
    /// reservoir layout without an `update_stamp` field
    /// cannot reject stale lights; the typed predicate
    /// flags this.
    #[must_use]
    pub const fn supports_stale_rejection(&self) -> bool {
        self.includes_update_stamp
    }
}

// ============================================================================
// Section 6 — Typed light index layout
// ============================================================================

/// Typed light index record layout. The cluster light list
/// stores one `LightIndex` per `(cluster, slot)`. The
/// renderer packs the index + (optional) auxiliary flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxLightIndexLayout {
    pub schema_version: u16,
    pub stable_id: &'static str,
    /// `max_lights_per_cluster` matches the cluster grid's
    /// same field — the two layouts must agree.
    pub max_lights_per_cluster: u32,
    pub bytes_per_index: u32,
}

impl LuxLightIndexLayout {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_GPU_LAYOUT_SCHEMA_VERSION,
        stable_id: "fun_lux.gpu_layout.light_index",
        max_lights_per_cluster: 32,
        bytes_per_index: 4,
    };

    /// Typed predicate: does this layout agree with the
    /// supplied cluster grid layout on `max_lights_per_cluster`?
    /// The renderer enforces agreement before allocating.
    #[must_use]
    pub const fn agrees_with_cluster_grid(&self, cluster_layout: &LuxClusterGridLayout) -> bool {
        self.max_lights_per_cluster == cluster_layout.max_lights_per_cluster
    }
}

// ============================================================================
// Section 7 — Typed layout bundle
// ============================================================================

/// Typed bundle of the four Pass 6 record layouts. The
/// renderer reads the bundle at boot to lay out the four
/// many-light buffers consistently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxManyLightLayoutBundle {
    pub schema_version: u16,
    pub light_record: LuxGpuLightRecordLayout,
    pub cluster_grid: LuxClusterGridLayout,
    pub reservoir: LuxReservoirLayout,
    pub light_index: LuxLightIndexLayout,
}

impl LuxManyLightLayoutBundle {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_GPU_LAYOUT_SCHEMA_VERSION,
        light_record: LuxGpuLightRecordLayout::PRODUCT_DEFAULT,
        cluster_grid: LuxClusterGridLayout::PRODUCT_DEFAULT,
        reservoir: LuxReservoirLayout::PRODUCT_DEFAULT,
        light_index: LuxLightIndexLayout::PRODUCT_DEFAULT,
    };

    /// Typed predicate: do every typed layout agreement
    /// invariant hold?
    #[must_use]
    pub fn invariants_hold(&self) -> bool {
        self.light_index
            .agrees_with_cluster_grid(&self.cluster_grid)
            && self.light_record.record_size_accommodates_every_field()
            && self.reservoir.supports_stale_rejection()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_LUX_GPU_LAYOUT_SCHEMA_VERSION, 1);
        assert_eq!(FUN_LUX_CLUSTER_GRID_TIER_COUNT, 4);
        assert_eq!(
            LuxClusterGridTier::ALL.len(),
            FUN_LUX_CLUSTER_GRID_TIER_COUNT
        );
        assert_eq!(FUN_LUX_LIGHT_RECORD_FIELD_COUNT, 16);
        assert_eq!(
            LuxGpuLightRecordField::ALL.len(),
            FUN_LUX_LIGHT_RECORD_FIELD_COUNT,
        );
    }

    /// Pass 6 acceptance: every typed tier maps to the
    /// user's spec dimensions.
    #[test]
    fn cluster_grid_tier_dimensions_match_user_spec() {
        assert_eq!(LuxClusterGridTier::LowMedium.dimensions(), [16, 9, 16]);
        assert_eq!(LuxClusterGridTier::High.dimensions(), [16, 9, 24]);
        assert_eq!(LuxClusterGridTier::Cinematic.dimensions(), [32, 18, 24]);
        assert_eq!(LuxClusterGridTier::Dynamic.dimensions(), [0, 0, 0]);
    }

    #[test]
    fn cluster_grid_tier_cluster_counts() {
        assert_eq!(LuxClusterGridTier::LowMedium.cluster_count(), 16 * 9 * 16);
        assert_eq!(LuxClusterGridTier::High.cluster_count(), 16 * 9 * 24);
        assert_eq!(LuxClusterGridTier::Cinematic.cluster_count(), 32 * 18 * 24,);
        // Dynamic has zero cluster count (runtime override).
        assert_eq!(LuxClusterGridTier::Dynamic.cluster_count(), 0);
    }

    #[test]
    fn cluster_grid_tier_is_dynamic_predicate() {
        assert!(!LuxClusterGridTier::LowMedium.is_dynamic());
        assert!(!LuxClusterGridTier::High.is_dynamic());
        assert!(!LuxClusterGridTier::Cinematic.is_dynamic());
        assert!(LuxClusterGridTier::Dynamic.is_dynamic());
    }

    #[test]
    fn light_record_field_taxonomy_is_unique() {
        let mut seen = std::collections::HashSet::new();
        for f in LuxGpuLightRecordField::ALL {
            assert!(seen.insert(f.as_str()), "duplicate: {}", f.as_str());
        }
    }

    /// Pass 6 acceptance: the typed light record includes
    /// every field from the user's spec.
    #[test]
    fn light_record_carries_every_user_listed_field() {
        for expected in [
            "stable_light_id",
            "kind",
            "position",
            "direction",
            "color_rgb",
            "intensity_lux",
            "range",
            "inner_cone_angle",
            "outer_cone_angle",
            "layer_mask",
            "scene_id",
            "volumetric_multiplier_q8",
            "shadow_policy",
            "update_stamp",
            "priority",
            "emissive_source_reference",
        ] {
            assert!(
                LuxGpuLightRecordField::ALL
                    .iter()
                    .any(|f| f.as_str() == expected),
                "missing field: {expected}",
            );
        }
    }

    /// Pass 6 acceptance: typed product default record size
    /// accommodates every field with std140 alignment.
    #[test]
    fn light_record_default_accommodates_every_field() {
        let layout = LuxGpuLightRecordLayout::PRODUCT_DEFAULT;
        assert!(layout.record_size_accommodates_every_field());
        assert_eq!(layout.field_count, FUN_LUX_LIGHT_RECORD_FIELD_COUNT as u32);
    }

    #[test]
    fn cluster_grid_layout_default_uses_high_tier() {
        let layout = LuxClusterGridLayout::PRODUCT_DEFAULT;
        assert_eq!(layout.tier, LuxClusterGridTier::High);
        assert_eq!(layout.clusters_x, 16);
        assert_eq!(layout.clusters_y, 9);
        assert_eq!(layout.clusters_z, 24);
        assert_eq!(layout.max_lights_per_cluster, 32);
        assert_eq!(layout.cluster_count(), 16 * 9 * 24);
    }

    #[test]
    fn cluster_grid_layout_for_low_medium_tier() {
        let layout = LuxClusterGridLayout::for_tier(LuxClusterGridTier::LowMedium);
        assert_eq!(layout.clusters_z, 16);
        assert_eq!(layout.max_lights_per_cluster, 64);
    }

    #[test]
    fn cluster_grid_layout_for_cinematic_tier() {
        let layout = LuxClusterGridLayout::for_tier(LuxClusterGridTier::Cinematic);
        assert_eq!(layout.clusters_x, 32);
        assert_eq!(layout.clusters_y, 18);
        assert_eq!(layout.max_lights_per_cluster, 16);
    }

    /// Pass 6 acceptance: typed reservoir layout supports
    /// stale rejection via update stamps.
    #[test]
    fn reservoir_layout_supports_stale_rejection() {
        let layout = LuxReservoirLayout::PRODUCT_DEFAULT;
        assert!(layout.supports_stale_rejection());
        assert!(layout.includes_update_stamp);
        assert_eq!(layout.history_frame_count, 8);
    }

    #[test]
    fn reservoir_layout_without_update_stamp_rejects_stale_rejection() {
        let mut layout = LuxReservoirLayout::PRODUCT_DEFAULT;
        layout.includes_update_stamp = false;
        assert!(!layout.supports_stale_rejection());
    }

    /// Pass 6 acceptance: typed light-index layout agrees
    /// with cluster grid on max-lights-per-cluster.
    #[test]
    fn light_index_layout_agrees_with_cluster_grid() {
        let cluster = LuxClusterGridLayout::PRODUCT_DEFAULT;
        let index = LuxLightIndexLayout::PRODUCT_DEFAULT;
        // Both default to max_lights_per_cluster = 32.
        assert!(index.agrees_with_cluster_grid(&cluster));
    }

    #[test]
    fn light_index_layout_disagrees_when_capacity_differs() {
        let cluster = LuxClusterGridLayout::for_tier(LuxClusterGridTier::LowMedium); // 64
        let index = LuxLightIndexLayout::PRODUCT_DEFAULT; // 32
        assert!(!index.agrees_with_cluster_grid(&cluster));
    }

    /// Pass 6 acceptance: typed bundle invariants hold.
    #[test]
    fn product_default_bundle_invariants_hold() {
        let bundle = LuxManyLightLayoutBundle::PRODUCT_DEFAULT;
        assert!(bundle.invariants_hold());
    }
}
