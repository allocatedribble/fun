//! Pass 4 — typed GPU buffer schemas for the canonical
//! lighting buffers.
//!
//! Encodes the user's "4.3 GPU light buffers" rule:
//!
//! - `FunLuxDirectionalLightBufferSchema`
//! - `FunLuxLocalLightBufferSchema`
//! - `FunLuxLightIndexBufferSchema`
//! - `FunLuxClusterGridBufferSchema`
//! - `FunLuxShadowDataBufferSchema`
//! - `FunLuxVolumetricLightBufferSchema`
//! - `FunLuxSceneLightingConstantsSchema`
//!
//! Each schema is a typed *descriptor* — it names a buffer
//! the renderer must allocate but never owns a `wgpu::Buffer`
//! handle (fun-lux stays backend-neutral per Pass 0's
//! `LuxBackendContract`).
//!
//! The user's typed routing rules — "use structure-of-arrays
//! or tightly packed structures where beneficial," "avoid
//! reallocating light buffers every frame," "use ring buffers
//! or persistent mapped buffers for dynamic updates" — land as
//! the typed `FunLuxGpuBufferLayoutMode` +
//! `FunLuxGpuUploadStrategy` enums + typed predicates that
//! reject the `ReallocateEveryFrame` strategy in production.

pub const FUN_LUX_GPU_BUFFER_SCHEMA_VERSION: u16 = 1;
pub const FUN_LUX_GPU_BUFFER_LAYOUT_MODE_COUNT: usize = 3;
pub const FUN_LUX_GPU_UPLOAD_STRATEGY_COUNT: usize = 3;
pub const FUN_LUX_GPU_BUFFER_KIND_COUNT: usize = 7;

// ============================================================================
// Section 1 — Typed layout mode (4.3 SoA / packed)
// ============================================================================

/// Typed buffer-layout strategy. SoA + TightlyPacked are
/// production-acceptable; ArrayOfStructures is the simple
/// default but loses SIMD-friendly access patterns.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunLuxGpuBufferLayoutMode {
    /// Array-of-Structures (each record laid out contiguously).
    /// Easiest to author; loses SIMD-friendly access.
    ArrayOfStructures,
    /// Structure-of-Arrays. Recommended for hot lighting
    /// buffers that the renderer iterates one field at a
    /// time (position[], color[], range[]).
    #[default]
    StructureOfArrays,
    /// Tightly packed (sub-32-bit field encoding). Saves
    /// bandwidth on cluster grids + reservoir buffers; needs
    /// careful unpacking shader code.
    TightlyPacked,
}

impl FunLuxGpuBufferLayoutMode {
    pub const ALL: [Self; FUN_LUX_GPU_BUFFER_LAYOUT_MODE_COUNT] = [
        Self::ArrayOfStructures,
        Self::StructureOfArrays,
        Self::TightlyPacked,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ArrayOfStructures => "array_of_structures",
            Self::StructureOfArrays => "structure_of_arrays",
            Self::TightlyPacked => "tightly_packed",
        }
    }
}

// ============================================================================
// Section 2 — Typed upload strategy (4.3 ring buffer / persistent mapped)
// ============================================================================

/// Typed GPU upload strategy. The user spec rejects
/// `ReallocateEveryFrame` for production — the typed
/// predicate `is_production_acceptable` returns `false` for
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunLuxGpuUploadStrategy {
    /// **PROHIBITED for production.** Allocates a new
    /// `wgpu::Buffer` every frame. Acceptable only in tests
    /// + early-development scaffolding.
    ReallocateEveryFrame,
    /// Typed product-default for dynamic buffers. Rotates
    /// through `ring_capacity_frames` typed pre-allocated
    /// slots; the renderer writes to the next free slot
    /// each frame.
    RingBuffer { ring_capacity_frames: u8 },
    /// Persistent mapped buffer. The renderer maps once at
    /// allocation and reads/writes via the mapped slice.
    /// Best for buffers the renderer writes once per frame
    /// + reads continuously (e.g. cluster grids).
    PersistentMapped,
}

impl FunLuxGpuUploadStrategy {
    pub const PRODUCT_RING_BUFFER: Self = Self::RingBuffer {
        ring_capacity_frames: 3,
    };

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReallocateEveryFrame => "reallocate_every_frame",
            Self::RingBuffer { .. } => "ring_buffer",
            Self::PersistentMapped => "persistent_mapped",
        }
    }

    /// Pass 4 acceptance: typed predicate — "avoid
    /// reallocating light buffers every frame."
    /// `ReallocateEveryFrame` returns `false`; every other
    /// strategy returns `true`.
    #[must_use]
    pub const fn is_production_acceptable(self) -> bool {
        !matches!(self, Self::ReallocateEveryFrame)
    }

    /// Typed predicate: does this strategy avoid GPU
    /// allocation churn (the goal of the typed acceptance
    /// rule above)?
    #[must_use]
    pub const fn avoids_per_frame_reallocation(self) -> bool {
        self.is_production_acceptable()
    }
}

// ============================================================================
// Section 3 — Typed buffer kind tag
// ============================================================================

/// Typed kind tag for one of the seven canonical lighting
/// buffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunLuxGpuBufferKind {
    DirectionalLight,
    LocalLight,
    LightIndex,
    ClusterGrid,
    ShadowData,
    VolumetricLight,
    SceneLightingConstants,
}

impl FunLuxGpuBufferKind {
    pub const ALL: [Self; FUN_LUX_GPU_BUFFER_KIND_COUNT] = [
        Self::DirectionalLight,
        Self::LocalLight,
        Self::LightIndex,
        Self::ClusterGrid,
        Self::ShadowData,
        Self::VolumetricLight,
        Self::SceneLightingConstants,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DirectionalLight => "directional_light",
            Self::LocalLight => "local_light",
            Self::LightIndex => "light_index",
            Self::ClusterGrid => "cluster_grid",
            Self::ShadowData => "shadow_data",
            Self::VolumetricLight => "volumetric_light",
            Self::SceneLightingConstants => "scene_lighting_constants",
        }
    }
}

// ============================================================================
// Section 4 — Per-buffer schema records
// ============================================================================

/// Typed directional light buffer schema. Stores the global
/// directional lights (typically sun + sky-fill).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunLuxDirectionalLightBufferSchema {
    pub schema_version: u16,
    pub stable_id: &'static str,
    pub max_directional_lights: u32,
    pub bytes_per_directional_light: u32,
    pub layout_mode: FunLuxGpuBufferLayoutMode,
    pub upload_strategy: FunLuxGpuUploadStrategy,
}

impl FunLuxDirectionalLightBufferSchema {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_GPU_BUFFER_SCHEMA_VERSION,
        stable_id: "fun_lux.gpu.directional_light_buffer",
        max_directional_lights: 4,
        bytes_per_directional_light: 64,
        layout_mode: FunLuxGpuBufferLayoutMode::StructureOfArrays,
        upload_strategy: FunLuxGpuUploadStrategy::PRODUCT_RING_BUFFER,
    };

    #[must_use]
    pub const fn total_byte_size(&self) -> u64 {
        (self.max_directional_lights as u64) * (self.bytes_per_directional_light as u64)
    }
}

/// Typed local-light buffer schema. Stores point + spot +
/// area lights (everything that's not a directional light).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunLuxLocalLightBufferSchema {
    pub schema_version: u16,
    pub stable_id: &'static str,
    pub max_local_lights: u32,
    pub bytes_per_local_light: u32,
    pub layout_mode: FunLuxGpuBufferLayoutMode,
    pub upload_strategy: FunLuxGpuUploadStrategy,
}

impl FunLuxLocalLightBufferSchema {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_GPU_BUFFER_SCHEMA_VERSION,
        stable_id: "fun_lux.gpu.local_light_buffer",
        max_local_lights: 65_536,
        bytes_per_local_light: 80,
        layout_mode: FunLuxGpuBufferLayoutMode::StructureOfArrays,
        upload_strategy: FunLuxGpuUploadStrategy::PRODUCT_RING_BUFFER,
    };

    #[must_use]
    pub const fn total_byte_size(&self) -> u64 {
        (self.max_local_lights as u64) * (self.bytes_per_local_light as u64)
    }
}

/// Typed light-index buffer schema. Per-cluster index lists
/// into the local light buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunLuxLightIndexBufferSchema {
    pub schema_version: u16,
    pub stable_id: &'static str,
    pub max_cluster_count: u32,
    pub max_lights_per_cluster: u32,
    pub bytes_per_index: u32,
    pub layout_mode: FunLuxGpuBufferLayoutMode,
    pub upload_strategy: FunLuxGpuUploadStrategy,
}

impl FunLuxLightIndexBufferSchema {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_GPU_BUFFER_SCHEMA_VERSION,
        stable_id: "fun_lux.gpu.light_index_buffer",
        // Pass 6 harmonization: 16 × 9 × 24 matches
        // `LuxClusterGridTier::High`.
        max_cluster_count: 16 * 9 * 24,
        max_lights_per_cluster: 32,
        bytes_per_index: 4,
        layout_mode: FunLuxGpuBufferLayoutMode::TightlyPacked,
        upload_strategy: FunLuxGpuUploadStrategy::PersistentMapped,
    };

    #[must_use]
    pub const fn total_byte_size(&self) -> u64 {
        (self.max_cluster_count as u64)
            * (self.max_lights_per_cluster as u64)
            * (self.bytes_per_index as u64)
    }
}

/// Typed cluster-grid buffer schema. Cluster AABBs + light
/// list offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunLuxClusterGridBufferSchema {
    pub schema_version: u16,
    pub stable_id: &'static str,
    pub clusters_x: u32,
    pub clusters_y: u32,
    pub clusters_z: u32,
    pub bytes_per_cluster_record: u32,
    pub layout_mode: FunLuxGpuBufferLayoutMode,
    pub upload_strategy: FunLuxGpuUploadStrategy,
}

impl FunLuxClusterGridBufferSchema {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_GPU_BUFFER_SCHEMA_VERSION,
        stable_id: "fun_lux.gpu.cluster_grid_buffer",
        // Pass 6 harmonization: matches
        // `LuxClusterGridTier::High` (16 × 9 × 24) and
        // fun-renderer's `lighting_stack` constants. Pass 4
        // shipped 16 × 8 × 24; the typed
        // `LuxClusterGridLayout` in `gpu_layout` is the
        // source of truth and this buffer schema mirrors it.
        clusters_x: 16,
        clusters_y: 9,
        clusters_z: 24,
        bytes_per_cluster_record: 32,
        layout_mode: FunLuxGpuBufferLayoutMode::StructureOfArrays,
        upload_strategy: FunLuxGpuUploadStrategy::PersistentMapped,
    };

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

/// Typed shadow-data buffer schema. Per-shadow-caster
/// matrices + slot indices + filter kernel hints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunLuxShadowDataBufferSchema {
    pub schema_version: u16,
    pub stable_id: &'static str,
    pub max_shadow_casters: u32,
    pub bytes_per_shadow_record: u32,
    pub layout_mode: FunLuxGpuBufferLayoutMode,
    pub upload_strategy: FunLuxGpuUploadStrategy,
}

impl FunLuxShadowDataBufferSchema {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_GPU_BUFFER_SCHEMA_VERSION,
        stable_id: "fun_lux.gpu.shadow_data_buffer",
        max_shadow_casters: 1024,
        bytes_per_shadow_record: 96,
        layout_mode: FunLuxGpuBufferLayoutMode::ArrayOfStructures,
        upload_strategy: FunLuxGpuUploadStrategy::PRODUCT_RING_BUFFER,
    };

    #[must_use]
    pub const fn total_byte_size(&self) -> u64 {
        (self.max_shadow_casters as u64) * (self.bytes_per_shadow_record as u64)
    }
}

/// Typed volumetric-light buffer schema. Subset of local
/// lights that contribute to the volumetric pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunLuxVolumetricLightBufferSchema {
    pub schema_version: u16,
    pub stable_id: &'static str,
    pub max_volumetric_lights: u32,
    pub bytes_per_volumetric_light: u32,
    pub layout_mode: FunLuxGpuBufferLayoutMode,
    pub upload_strategy: FunLuxGpuUploadStrategy,
}

impl FunLuxVolumetricLightBufferSchema {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_GPU_BUFFER_SCHEMA_VERSION,
        stable_id: "fun_lux.gpu.volumetric_light_buffer",
        max_volumetric_lights: 256,
        bytes_per_volumetric_light: 48,
        layout_mode: FunLuxGpuBufferLayoutMode::StructureOfArrays,
        upload_strategy: FunLuxGpuUploadStrategy::PRODUCT_RING_BUFFER,
    };

    #[must_use]
    pub const fn total_byte_size(&self) -> u64 {
        (self.max_volumetric_lights as u64) * (self.bytes_per_volumetric_light as u64)
    }
}

/// Typed per-scene lighting constants buffer schema. The
/// small uniform buffer the renderer binds per scene with
/// scene-wide constants (exposure, ambient, fog settings,
/// directional light count, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunLuxSceneLightingConstantsSchema {
    pub schema_version: u16,
    pub stable_id: &'static str,
    pub byte_size: u32,
    pub layout_mode: FunLuxGpuBufferLayoutMode,
    pub upload_strategy: FunLuxGpuUploadStrategy,
}

impl FunLuxSceneLightingConstantsSchema {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_GPU_BUFFER_SCHEMA_VERSION,
        stable_id: "fun_lux.gpu.scene_lighting_constants",
        byte_size: 256,
        layout_mode: FunLuxGpuBufferLayoutMode::TightlyPacked,
        upload_strategy: FunLuxGpuUploadStrategy::PRODUCT_RING_BUFFER,
    };

    #[must_use]
    pub const fn total_byte_size(&self) -> u64 {
        self.byte_size as u64
    }
}

// ============================================================================
// Section 5 — Typed contract bundle
// ============================================================================

/// Typed bundle of all seven lighting buffer schemas. The
/// renderer reads the typed bundle to allocate the seven
/// real `wgpu::Buffer`s at boot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunLuxGpuBufferSchemaBundle {
    pub schema_version: u16,
    pub directional_light: FunLuxDirectionalLightBufferSchema,
    pub local_light: FunLuxLocalLightBufferSchema,
    pub light_index: FunLuxLightIndexBufferSchema,
    pub cluster_grid: FunLuxClusterGridBufferSchema,
    pub shadow_data: FunLuxShadowDataBufferSchema,
    pub volumetric_light: FunLuxVolumetricLightBufferSchema,
    pub scene_lighting_constants: FunLuxSceneLightingConstantsSchema,
}

impl FunLuxGpuBufferSchemaBundle {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_GPU_BUFFER_SCHEMA_VERSION,
        directional_light: FunLuxDirectionalLightBufferSchema::PRODUCT_DEFAULT,
        local_light: FunLuxLocalLightBufferSchema::PRODUCT_DEFAULT,
        light_index: FunLuxLightIndexBufferSchema::PRODUCT_DEFAULT,
        cluster_grid: FunLuxClusterGridBufferSchema::PRODUCT_DEFAULT,
        shadow_data: FunLuxShadowDataBufferSchema::PRODUCT_DEFAULT,
        volumetric_light: FunLuxVolumetricLightBufferSchema::PRODUCT_DEFAULT,
        scene_lighting_constants: FunLuxSceneLightingConstantsSchema::PRODUCT_DEFAULT,
    };

    /// Pass 4 acceptance: typed predicate — every typed
    /// buffer in the bundle uses a production-acceptable
    /// upload strategy (no `ReallocateEveryFrame` anywhere).
    #[must_use]
    pub const fn every_buffer_avoids_per_frame_reallocation(&self) -> bool {
        self.directional_light
            .upload_strategy
            .is_production_acceptable()
            && self.local_light.upload_strategy.is_production_acceptable()
            && self.light_index.upload_strategy.is_production_acceptable()
            && self.cluster_grid.upload_strategy.is_production_acceptable()
            && self.shadow_data.upload_strategy.is_production_acceptable()
            && self
                .volumetric_light
                .upload_strategy
                .is_production_acceptable()
            && self
                .scene_lighting_constants
                .upload_strategy
                .is_production_acceptable()
    }

    /// Typed total byte budget across every typed buffer in
    /// the bundle.
    #[must_use]
    pub const fn total_byte_budget(&self) -> u64 {
        self.directional_light.total_byte_size()
            + self.local_light.total_byte_size()
            + self.light_index.total_byte_size()
            + self.cluster_grid.total_byte_size()
            + self.shadow_data.total_byte_size()
            + self.volumetric_light.total_byte_size()
            + self.scene_lighting_constants.total_byte_size()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_LUX_GPU_BUFFER_SCHEMA_VERSION, 1);
        assert_eq!(FUN_LUX_GPU_BUFFER_LAYOUT_MODE_COUNT, 3);
        assert_eq!(
            FunLuxGpuBufferLayoutMode::ALL.len(),
            FUN_LUX_GPU_BUFFER_LAYOUT_MODE_COUNT,
        );
        assert_eq!(FUN_LUX_GPU_BUFFER_KIND_COUNT, 7);
        assert_eq!(
            FunLuxGpuBufferKind::ALL.len(),
            FUN_LUX_GPU_BUFFER_KIND_COUNT
        );
    }

    #[test]
    fn layout_mode_taxonomy_strings_are_unique() {
        let mut seen = hashbrown::HashSet::new();
        for mode in FunLuxGpuBufferLayoutMode::ALL {
            assert!(seen.insert(mode.as_str()), "duplicate: {}", mode.as_str());
        }
    }

    /// Pass 4 acceptance: "avoid reallocating light buffers
    /// every frame."
    #[test]
    fn reallocate_every_frame_is_not_production_acceptable() {
        assert!(!FunLuxGpuUploadStrategy::ReallocateEveryFrame.is_production_acceptable());
        assert!(!FunLuxGpuUploadStrategy::ReallocateEveryFrame.avoids_per_frame_reallocation());
    }

    /// Pass 4 acceptance: "use ring buffers or persistent
    /// mapped buffers for dynamic updates."
    #[test]
    fn ring_buffer_and_persistent_mapped_are_production_acceptable() {
        assert!(FunLuxGpuUploadStrategy::PRODUCT_RING_BUFFER.is_production_acceptable());
        assert!(FunLuxGpuUploadStrategy::PersistentMapped.is_production_acceptable());
        assert!(
            FunLuxGpuUploadStrategy::RingBuffer {
                ring_capacity_frames: 2
            }
            .is_production_acceptable()
        );
    }

    #[test]
    fn buffer_kind_taxonomy_strings_are_unique() {
        let mut seen = hashbrown::HashSet::new();
        for kind in FunLuxGpuBufferKind::ALL {
            assert!(seen.insert(kind.as_str()), "duplicate: {}", kind.as_str());
        }
    }

    #[test]
    fn directional_light_schema_default_is_production_acceptable() {
        let schema = FunLuxDirectionalLightBufferSchema::PRODUCT_DEFAULT;
        assert!(schema.upload_strategy.is_production_acceptable());
        assert_eq!(schema.total_byte_size(), 4 * 64);
    }

    #[test]
    fn local_light_schema_default_uses_soa() {
        let schema = FunLuxLocalLightBufferSchema::PRODUCT_DEFAULT;
        assert_eq!(
            schema.layout_mode,
            FunLuxGpuBufferLayoutMode::StructureOfArrays,
        );
        assert!(schema.upload_strategy.is_production_acceptable());
    }

    #[test]
    fn light_index_schema_default_uses_tightly_packed() {
        let schema = FunLuxLightIndexBufferSchema::PRODUCT_DEFAULT;
        assert_eq!(schema.layout_mode, FunLuxGpuBufferLayoutMode::TightlyPacked,);
        // Pass 6 harmonization: cluster grid is 16 × 9 × 24.
        assert_eq!(schema.total_byte_size(), (16u64 * 9 * 24) * 32 * 4,);
    }

    #[test]
    fn cluster_grid_schema_cluster_count_walks_dimensions() {
        let schema = FunLuxClusterGridBufferSchema::PRODUCT_DEFAULT;
        // Pass 6 harmonization: cluster grid is 16 × 9 × 24
        // (matches `LuxClusterGridTier::High`).
        assert_eq!(schema.cluster_count(), 16 * 9 * 24);
    }

    /// Pass 4 acceptance: typed bundle predicate — every
    /// typed buffer in `PRODUCT_DEFAULT` avoids
    /// per-frame reallocation.
    #[test]
    fn product_default_bundle_avoids_per_frame_reallocation() {
        let bundle = FunLuxGpuBufferSchemaBundle::PRODUCT_DEFAULT;
        assert!(bundle.every_buffer_avoids_per_frame_reallocation());
        // Total budget is non-zero.
        assert!(bundle.total_byte_budget() > 0);
    }

    /// Pass 4 acceptance: if any typed buffer in the bundle
    /// drops to `ReallocateEveryFrame`, the typed predicate
    /// flips to false.
    #[test]
    fn bundle_predicate_flips_when_any_buffer_reallocates_per_frame() {
        let mut bundle = FunLuxGpuBufferSchemaBundle::PRODUCT_DEFAULT;
        bundle.local_light.upload_strategy = FunLuxGpuUploadStrategy::ReallocateEveryFrame;
        assert!(!bundle.every_buffer_avoids_per_frame_reallocation());
    }
}
