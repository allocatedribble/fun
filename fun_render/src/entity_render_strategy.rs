use bevy::prelude::Resource;

use crate::{indirect_draw::FunDrawBudgetLane, signature::FunRenderPath};

pub const FUN_ENTITY_RENDER_STRATEGY_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FunEntityRenderClass {
    StaticOpaqueWorld,
    RepeatedProp,
    VehicleSharedPart,
    VehicleUniqueDamagedPart,
    Character,
    FoliageAggregate,
    Particle,
    TransparentGeometry,
    CefUi,
    DebugOverlay,
}

impl FunEntityRenderClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StaticOpaqueWorld => "static_opaque_world",
            Self::RepeatedProp => "repeated_prop",
            Self::VehicleSharedPart => "vehicle_shared_part",
            Self::VehicleUniqueDamagedPart => "vehicle_unique_damaged_part",
            Self::Character => "character",
            Self::FoliageAggregate => "foliage_aggregate",
            Self::Particle => "particle",
            Self::TransparentGeometry => "transparent_geometry",
            Self::CefUi => "cef_ui",
            Self::DebugOverlay => "debug_overlay",
        }
    }

    const fn strategy_index(self) -> usize {
        match self {
            Self::StaticOpaqueWorld => 0,
            Self::RepeatedProp => 1,
            Self::VehicleSharedPart => 2,
            Self::VehicleUniqueDamagedPart => 3,
            Self::Character => 4,
            Self::FoliageAggregate => 5,
            Self::Particle => 6,
            Self::TransparentGeometry => 7,
            Self::CefUi => 8,
            Self::DebugOverlay => 9,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FunEntityRenderPathTarget {
    StandardRaster,
    InstancedRaster,
    GpuCulledIndirect,
    MeshletStaticDense,
    FunVG,
    GpuParticleBuffers,
    TransparentPipeline,
    CefPostWorldComposition,
    DebugPhase,
}

impl FunEntityRenderPathTarget {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StandardRaster => "standard_raster",
            Self::InstancedRaster => "instanced_raster",
            Self::GpuCulledIndirect => "gpu_culled_indirect",
            Self::MeshletStaticDense => "meshlet_static_dense",
            Self::FunVG => "fun_vg",
            Self::GpuParticleBuffers => "gpu_particle_buffers",
            Self::TransparentPipeline => "transparent_pipeline",
            Self::CefPostWorldComposition => "cef_post_world_composition",
            Self::DebugPhase => "debug_phase",
        }
    }

    pub const fn fun_render_path(self) -> Option<FunRenderPath> {
        match self {
            Self::StandardRaster => Some(FunRenderPath::StandardRaster),
            Self::InstancedRaster => Some(FunRenderPath::InstancedRaster),
            Self::GpuCulledIndirect => Some(FunRenderPath::GpuCulledIndirect),
            Self::MeshletStaticDense => Some(FunRenderPath::MeshletStaticDense),
            Self::CefPostWorldComposition => Some(FunRenderPath::CefUi),
            Self::DebugPhase => Some(FunRenderPath::DebugOnly),
            Self::FunVG | Self::GpuParticleBuffers | Self::TransparentPipeline => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FunDrawCallScalingTarget {
    MaterialBucketsByPass,
    MeshMaterialBuckets,
    VehicleSharedPartBuckets,
    VehicleUniquePartDraws,
    LodMaterialBuckets,
    FoliageWindMaterialBuckets,
    ParticleBuckets,
    TransparentBuckets,
    PostWorldComposition,
    IsolatedDebugPhase,
}

impl FunDrawCallScalingTarget {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MaterialBucketsByPass => "material_buckets_by_pass",
            Self::MeshMaterialBuckets => "mesh_material_buckets",
            Self::VehicleSharedPartBuckets => "vehicle_shared_part_buckets",
            Self::VehicleUniquePartDraws => "vehicle_unique_part_draws",
            Self::LodMaterialBuckets => "lod_material_buckets",
            Self::FoliageWindMaterialBuckets => "foliage_wind_material_buckets",
            Self::ParticleBuckets => "particle_buckets",
            Self::TransparentBuckets => "transparent_buckets",
            Self::PostWorldComposition => "post_world_composition",
            Self::IsolatedDebugPhase => "isolated_debug_phase",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunEntityRenderFeatureFlags {
    pub bake_clusters: bool,
    pub gpu_culling: bool,
    pub material_bucketing: bool,
    pub indirect_draws: bool,
    pub page_streaming: bool,
    pub instance_tables: bool,
    pub dirty_transform_ranges: bool,
    pub structured_per_entity_buffers: bool,
    pub split_stable_render_parts: bool,
    pub keep_skinned_path_separate: bool,
    pub lod_and_impostors: bool,
    pub specialized_instancing: bool,
    pub alpha_overdraw_aware_lod: bool,
    pub density_throttling: bool,
    pub wind_material_buckets: bool,
    pub gpu_particle_buffers: bool,
    pub compute_simulation: bool,
    pub particle_overdraw_report: bool,
    pub transparent_draw_report: bool,
    pub post_world_composition: bool,
    pub gpu_copy_path: bool,
    pub cpu_fallback_measured: bool,
    pub isolated_debug_phase: bool,
    pub disabled_in_perf_lanes: bool,
    pub cef_excluded_from_temporal_reconstruction: bool,
    pub cef_excluded_from_world_geometry_metrics: bool,
    pub never_virtual_geometry: bool,
}

impl FunEntityRenderFeatureFlags {
    pub const NONE: Self = Self {
        bake_clusters: false,
        gpu_culling: false,
        material_bucketing: false,
        indirect_draws: false,
        page_streaming: false,
        instance_tables: false,
        dirty_transform_ranges: false,
        structured_per_entity_buffers: false,
        split_stable_render_parts: false,
        keep_skinned_path_separate: false,
        lod_and_impostors: false,
        specialized_instancing: false,
        alpha_overdraw_aware_lod: false,
        density_throttling: false,
        wind_material_buckets: false,
        gpu_particle_buffers: false,
        compute_simulation: false,
        particle_overdraw_report: false,
        transparent_draw_report: false,
        post_world_composition: false,
        gpu_copy_path: false,
        cpu_fallback_measured: false,
        isolated_debug_phase: false,
        disabled_in_perf_lanes: false,
        cef_excluded_from_temporal_reconstruction: false,
        cef_excluded_from_world_geometry_metrics: false,
        never_virtual_geometry: false,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunEntityRenderStrategy {
    pub schema_version: u16,
    pub entity_class: FunEntityRenderClass,
    pub target_paths: &'static [FunEntityRenderPathTarget],
    pub draw_call_target: FunDrawCallScalingTarget,
    pub features: FunEntityRenderFeatureFlags,
    pub perf_lane_enabled_by_default: bool,
    pub benchmark_label: &'static str,
    pub notes: &'static [&'static str],
}

const STATIC_OPAQUE_WORLD_TARGET_PATHS: &[FunEntityRenderPathTarget] = &[
    FunEntityRenderPathTarget::GpuCulledIndirect,
    FunEntityRenderPathTarget::MeshletStaticDense,
    FunEntityRenderPathTarget::FunVG,
];

const REPEATED_PROP_TARGET_PATHS: &[FunEntityRenderPathTarget] = &[
    FunEntityRenderPathTarget::InstancedRaster,
    FunEntityRenderPathTarget::GpuCulledIndirect,
];

const VEHICLE_SHARED_PART_TARGET_PATHS: &[FunEntityRenderPathTarget] = &[
    FunEntityRenderPathTarget::InstancedRaster,
    FunEntityRenderPathTarget::GpuCulledIndirect,
];

const VEHICLE_UNIQUE_DAMAGED_PART_TARGET_PATHS: &[FunEntityRenderPathTarget] =
    &[FunEntityRenderPathTarget::StandardRaster];

const CHARACTER_TARGET_PATHS: &[FunEntityRenderPathTarget] =
    &[FunEntityRenderPathTarget::StandardRaster];

const FOLIAGE_AGGREGATE_TARGET_PATHS: &[FunEntityRenderPathTarget] =
    &[FunEntityRenderPathTarget::InstancedRaster];

const PARTICLE_TARGET_PATHS: &[FunEntityRenderPathTarget] =
    &[FunEntityRenderPathTarget::GpuParticleBuffers];

const TRANSPARENT_GEOMETRY_TARGET_PATHS: &[FunEntityRenderPathTarget] =
    &[FunEntityRenderPathTarget::TransparentPipeline];

const CEF_UI_TARGET_PATHS: &[FunEntityRenderPathTarget] =
    &[FunEntityRenderPathTarget::CefPostWorldComposition];

const DEBUG_OVERLAY_TARGET_PATHS: &[FunEntityRenderPathTarget] =
    &[FunEntityRenderPathTarget::DebugPhase];

pub static FUN_ENTITY_RENDER_STRATEGIES: [FunEntityRenderStrategy; 10] = [
    FunEntityRenderStrategy {
        schema_version: FUN_ENTITY_RENDER_STRATEGY_SCHEMA_VERSION,
        entity_class: FunEntityRenderClass::StaticOpaqueWorld,
        target_paths: STATIC_OPAQUE_WORLD_TARGET_PATHS,
        draw_call_target: FunDrawCallScalingTarget::MaterialBucketsByPass,
        features: FunEntityRenderFeatureFlags {
            bake_clusters: true,
            gpu_culling: true,
            material_bucketing: true,
            indirect_draws: true,
            page_streaming: true,
            ..FunEntityRenderFeatureFlags::NONE
        },
        perf_lane_enabled_by_default: true,
        benchmark_label: "static_opaque_world",
        notes: &[
            "draws scale with material buckets by pass",
            "cluster baking, gpu culling, indirect draws, and page streaming are required before FunVG promotion",
        ],
    },
    FunEntityRenderStrategy {
        schema_version: FUN_ENTITY_RENDER_STRATEGY_SCHEMA_VERSION,
        entity_class: FunEntityRenderClass::RepeatedProp,
        target_paths: REPEATED_PROP_TARGET_PATHS,
        draw_call_target: FunDrawCallScalingTarget::MeshMaterialBuckets,
        features: FunEntityRenderFeatureFlags {
            gpu_culling: true,
            material_bucketing: true,
            indirect_draws: true,
            instance_tables: true,
            dirty_transform_ranges: true,
            ..FunEntityRenderFeatureFlags::NONE
        },
        perf_lane_enabled_by_default: true,
        benchmark_label: "repeated_prop",
        notes: &[
            "draws scale with mesh/material buckets",
            "dirty transform ranges should feed resident instance tables",
        ],
    },
    FunEntityRenderStrategy {
        schema_version: FUN_ENTITY_RENDER_STRATEGY_SCHEMA_VERSION,
        entity_class: FunEntityRenderClass::VehicleSharedPart,
        target_paths: VEHICLE_SHARED_PART_TARGET_PATHS,
        draw_call_target: FunDrawCallScalingTarget::VehicleSharedPartBuckets,
        features: FunEntityRenderFeatureFlags {
            gpu_culling: true,
            material_bucketing: true,
            indirect_draws: true,
            instance_tables: true,
            structured_per_entity_buffers: true,
            split_stable_render_parts: true,
            ..FunEntityRenderFeatureFlags::NONE
        },
        perf_lane_enabled_by_default: true,
        benchmark_label: "vehicle_shared_part",
        notes: &[
            "shared vehicle parts use instancing when mesh/material identity is stable",
            "per-vehicle state belongs in structured buffers",
        ],
    },
    FunEntityRenderStrategy {
        schema_version: FUN_ENTITY_RENDER_STRATEGY_SCHEMA_VERSION,
        entity_class: FunEntityRenderClass::VehicleUniqueDamagedPart,
        target_paths: VEHICLE_UNIQUE_DAMAGED_PART_TARGET_PATHS,
        draw_call_target: FunDrawCallScalingTarget::VehicleUniquePartDraws,
        features: FunEntityRenderFeatureFlags {
            structured_per_entity_buffers: true,
            split_stable_render_parts: true,
            never_virtual_geometry: true,
            ..FunEntityRenderFeatureFlags::NONE
        },
        perf_lane_enabled_by_default: true,
        benchmark_label: "vehicle_unique_damaged_part",
        notes: &[
            "unique damaged or customized vehicle parts stay on standard raster until material diversity is reduced",
            "avoid turning damage variation into a material uniqueness explosion",
        ],
    },
    FunEntityRenderStrategy {
        schema_version: FUN_ENTITY_RENDER_STRATEGY_SCHEMA_VERSION,
        entity_class: FunEntityRenderClass::Character,
        target_paths: CHARACTER_TARGET_PATHS,
        draw_call_target: FunDrawCallScalingTarget::LodMaterialBuckets,
        features: FunEntityRenderFeatureFlags {
            gpu_culling: true,
            material_bucketing: true,
            keep_skinned_path_separate: true,
            lod_and_impostors: true,
            never_virtual_geometry: true,
            ..FunEntityRenderFeatureFlags::NONE
        },
        perf_lane_enabled_by_default: true,
        benchmark_label: "character",
        notes: &[
            "skinned characters stay separate from static meshlet and FunVG paths",
            "far characters should reduce cost through LOD, impostors, and skeleton/material buckets",
        ],
    },
    FunEntityRenderStrategy {
        schema_version: FUN_ENTITY_RENDER_STRATEGY_SCHEMA_VERSION,
        entity_class: FunEntityRenderClass::FoliageAggregate,
        target_paths: FOLIAGE_AGGREGATE_TARGET_PATHS,
        draw_call_target: FunDrawCallScalingTarget::FoliageWindMaterialBuckets,
        features: FunEntityRenderFeatureFlags {
            material_bucketing: true,
            specialized_instancing: true,
            alpha_overdraw_aware_lod: true,
            density_throttling: true,
            wind_material_buckets: true,
            never_virtual_geometry: true,
            ..FunEntityRenderFeatureFlags::NONE
        },
        perf_lane_enabled_by_default: true,
        benchmark_label: "foliage_aggregate",
        notes: &[
            "foliage and alpha-heavy aggregate geometry are hostile to Nanite-style simplification",
            "cluster culling is only a candidate after measurement",
        ],
    },
    FunEntityRenderStrategy {
        schema_version: FUN_ENTITY_RENDER_STRATEGY_SCHEMA_VERSION,
        entity_class: FunEntityRenderClass::Particle,
        target_paths: PARTICLE_TARGET_PATHS,
        draw_call_target: FunDrawCallScalingTarget::ParticleBuckets,
        features: FunEntityRenderFeatureFlags {
            gpu_particle_buffers: true,
            compute_simulation: true,
            particle_overdraw_report: true,
            never_virtual_geometry: true,
            ..FunEntityRenderFeatureFlags::NONE
        },
        perf_lane_enabled_by_default: true,
        benchmark_label: "particle",
        notes: &[
            "particles use GPU particle buffers and separate overdraw reporting",
            "particles never enter virtual geometry",
        ],
    },
    FunEntityRenderStrategy {
        schema_version: FUN_ENTITY_RENDER_STRATEGY_SCHEMA_VERSION,
        entity_class: FunEntityRenderClass::TransparentGeometry,
        target_paths: TRANSPARENT_GEOMETRY_TARGET_PATHS,
        draw_call_target: FunDrawCallScalingTarget::TransparentBuckets,
        features: FunEntityRenderFeatureFlags {
            transparent_draw_report: true,
            never_virtual_geometry: true,
            ..FunEntityRenderFeatureFlags::NONE
        },
        perf_lane_enabled_by_default: true,
        benchmark_label: "transparent_geometry",
        notes: &[
            "transparent geometry remains separate from opaque virtual geometry until an explicit OIT strategy exists",
            "transparent draw count is reported separately",
        ],
    },
    FunEntityRenderStrategy {
        schema_version: FUN_ENTITY_RENDER_STRATEGY_SCHEMA_VERSION,
        entity_class: FunEntityRenderClass::CefUi,
        target_paths: CEF_UI_TARGET_PATHS,
        draw_call_target: FunDrawCallScalingTarget::PostWorldComposition,
        features: FunEntityRenderFeatureFlags {
            post_world_composition: true,
            gpu_copy_path: true,
            cpu_fallback_measured: true,
            cef_excluded_from_temporal_reconstruction: true,
            cef_excluded_from_world_geometry_metrics: true,
            never_virtual_geometry: true,
            ..FunEntityRenderFeatureFlags::NONE
        },
        perf_lane_enabled_by_default: true,
        benchmark_label: "cef_ui",
        notes: &[
            "CEF/UI is composed after the world",
            "CEF CPU fallback is measured explicitly and excluded from world geometry metrics",
        ],
    },
    FunEntityRenderStrategy {
        schema_version: FUN_ENTITY_RENDER_STRATEGY_SCHEMA_VERSION,
        entity_class: FunEntityRenderClass::DebugOverlay,
        target_paths: DEBUG_OVERLAY_TARGET_PATHS,
        draw_call_target: FunDrawCallScalingTarget::IsolatedDebugPhase,
        features: FunEntityRenderFeatureFlags {
            isolated_debug_phase: true,
            disabled_in_perf_lanes: true,
            never_virtual_geometry: true,
            ..FunEntityRenderFeatureFlags::NONE
        },
        perf_lane_enabled_by_default: false,
        benchmark_label: "debug_overlay",
        notes: &[
            "debug overlays live in an isolated debug phase",
            "debug draw calls are disabled in perf lanes unless explicitly requested",
        ],
    },
];

pub fn fun_entity_render_strategies() -> &'static [FunEntityRenderStrategy] {
    &FUN_ENTITY_RENDER_STRATEGIES
}

pub fn strategy_for_entity_render_class(
    entity_class: FunEntityRenderClass,
) -> &'static FunEntityRenderStrategy {
    &FUN_ENTITY_RENDER_STRATEGIES[entity_class.strategy_index()]
}

#[derive(Debug, Clone, Copy, Default, Resource)]
pub struct FunEntityRenderStrategyRegistry;

impl FunEntityRenderStrategyRegistry {
    pub fn strategies(&self) -> &'static [FunEntityRenderStrategy] {
        fun_entity_render_strategies()
    }

    pub fn strategy_for(
        &self,
        entity_class: FunEntityRenderClass,
    ) -> &'static FunEntityRenderStrategy {
        strategy_for_entity_render_class(entity_class)
    }

    pub fn evaluate_budget(
        &self,
        input: FunEntityRenderStrategyBudgetInput,
    ) -> FunEntityRenderStrategyBudgetReport {
        evaluate_entity_render_strategy_budget(input)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunEntityRenderStrategyBudgetInput {
    pub entity_class: FunEntityRenderClass,
    pub lane: FunDrawBudgetLane,
    pub object_count: u32,
    pub material_bucket_count: u32,
    pub mesh_material_bucket_count: u32,
    pub skeleton_material_bucket_count: u32,
    pub particle_bucket_count: u32,
    pub transparent_bucket_count: u32,
    pub pass_count: u32,
    pub debug_requested: bool,
}

impl Default for FunEntityRenderStrategyBudgetInput {
    fn default() -> Self {
        Self {
            entity_class: FunEntityRenderClass::StaticOpaqueWorld,
            lane: FunDrawBudgetLane::FullRuntime,
            object_count: 1,
            material_bucket_count: 1,
            mesh_material_bucket_count: 1,
            skeleton_material_bucket_count: 1,
            particle_bucket_count: 1,
            transparent_bucket_count: 1,
            pass_count: 1,
            debug_requested: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunEntityRenderStrategyBudgetReport {
    pub schema_version: u16,
    pub entity_class: FunEntityRenderClass,
    pub lane: FunDrawBudgetLane,
    pub draw_call_target: FunDrawCallScalingTarget,
    pub reference_object_count: u32,
    pub max_expected_draw_calls: u32,
    pub draw_calls_scale_with_raw_entities: bool,
    pub perf_lane_allowed: bool,
    pub strategy: &'static FunEntityRenderStrategy,
}

pub fn evaluate_entity_render_strategy_budget(
    input: FunEntityRenderStrategyBudgetInput,
) -> FunEntityRenderStrategyBudgetReport {
    let strategy = strategy_for_entity_render_class(input.entity_class);
    let max_expected_draw_calls = match input.entity_class {
        FunEntityRenderClass::StaticOpaqueWorld => {
            input.material_bucket_count.saturating_mul(input.pass_count)
        }
        FunEntityRenderClass::RepeatedProp => input.mesh_material_bucket_count,
        FunEntityRenderClass::VehicleSharedPart => input.mesh_material_bucket_count,
        FunEntityRenderClass::VehicleUniqueDamagedPart => input.object_count,
        FunEntityRenderClass::Character => input.skeleton_material_bucket_count,
        FunEntityRenderClass::FoliageAggregate => input.mesh_material_bucket_count,
        FunEntityRenderClass::Particle => input.particle_bucket_count,
        FunEntityRenderClass::TransparentGeometry => input.transparent_bucket_count,
        FunEntityRenderClass::CefUi => u32::from(input.object_count > 0),
        FunEntityRenderClass::DebugOverlay => {
            if input.debug_requested {
                input.material_bucket_count.max(1)
            } else {
                0
            }
        }
    };

    FunEntityRenderStrategyBudgetReport {
        schema_version: FUN_ENTITY_RENDER_STRATEGY_SCHEMA_VERSION,
        entity_class: input.entity_class,
        lane: input.lane,
        draw_call_target: strategy.draw_call_target,
        reference_object_count: input.object_count,
        max_expected_draw_calls,
        draw_calls_scale_with_raw_entities: matches!(
            input.entity_class,
            FunEntityRenderClass::VehicleUniqueDamagedPart
        ),
        perf_lane_allowed: strategy.perf_lane_enabled_by_default || input.debug_requested,
        strategy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_render_strategy_schema_version_is_stable() {
        assert_eq!(FUN_ENTITY_RENDER_STRATEGY_SCHEMA_VERSION, 1);
        assert!(
            fun_entity_render_strategies().iter().all(
                |strategy| strategy.schema_version == FUN_ENTITY_RENDER_STRATEGY_SCHEMA_VERSION
            )
        );
    }

    #[test]
    fn static_opaque_world_targets_gpu_indirect_meshlets_then_funvg() {
        let strategy = strategy_for_entity_render_class(FunEntityRenderClass::StaticOpaqueWorld);

        assert_eq!(
            strategy.target_paths,
            &[
                FunEntityRenderPathTarget::GpuCulledIndirect,
                FunEntityRenderPathTarget::MeshletStaticDense,
                FunEntityRenderPathTarget::FunVG,
            ]
        );
        assert!(strategy.features.bake_clusters);
        assert!(strategy.features.gpu_culling);
        assert!(strategy.features.material_bucketing);
        assert!(strategy.features.indirect_draws);
        assert!(strategy.features.page_streaming);
        assert_eq!(
            strategy.draw_call_target,
            FunDrawCallScalingTarget::MaterialBucketsByPass
        );
    }

    #[test]
    fn static_opaque_world_draw_target_scales_with_material_buckets_not_objects() {
        let report = evaluate_entity_render_strategy_budget(FunEntityRenderStrategyBudgetInput {
            entity_class: FunEntityRenderClass::StaticOpaqueWorld,
            object_count: 10_000,
            material_bucket_count: 9,
            pass_count: 3,
            ..Default::default()
        });

        assert_eq!(report.max_expected_draw_calls, 27);
        assert_eq!(report.reference_object_count, 10_000);
        assert!(!report.draw_calls_scale_with_raw_entities);
    }

    #[test]
    fn repeated_props_target_instancing_and_indirect_not_prop_instances() {
        let strategy = strategy_for_entity_render_class(FunEntityRenderClass::RepeatedProp);
        let report = evaluate_entity_render_strategy_budget(FunEntityRenderStrategyBudgetInput {
            entity_class: FunEntityRenderClass::RepeatedProp,
            object_count: 50_000,
            mesh_material_bucket_count: 42,
            ..Default::default()
        });

        assert_eq!(
            strategy.target_paths,
            &[
                FunEntityRenderPathTarget::InstancedRaster,
                FunEntityRenderPathTarget::GpuCulledIndirect,
            ]
        );
        assert!(strategy.features.instance_tables);
        assert!(strategy.features.dirty_transform_ranges);
        assert_eq!(report.max_expected_draw_calls, 42);
        assert!(!report.draw_calls_scale_with_raw_entities);
    }

    #[test]
    fn vehicles_split_shared_and_unique_parts() {
        let shared = strategy_for_entity_render_class(FunEntityRenderClass::VehicleSharedPart);
        let unique =
            strategy_for_entity_render_class(FunEntityRenderClass::VehicleUniqueDamagedPart);

        assert!(shared.features.split_stable_render_parts);
        assert!(shared.features.structured_per_entity_buffers);
        assert!(
            shared
                .target_paths
                .contains(&FunEntityRenderPathTarget::InstancedRaster)
        );
        assert_eq!(
            shared.draw_call_target,
            FunDrawCallScalingTarget::VehicleSharedPartBuckets
        );

        assert_eq!(
            unique.target_paths,
            &[FunEntityRenderPathTarget::StandardRaster]
        );
        assert!(unique.features.never_virtual_geometry);
        assert_eq!(
            unique.draw_call_target,
            FunDrawCallScalingTarget::VehicleUniquePartDraws
        );
    }

    #[test]
    fn characters_do_not_use_funvg_initially() {
        let strategy = strategy_for_entity_render_class(FunEntityRenderClass::Character);
        let report = evaluate_entity_render_strategy_budget(FunEntityRenderStrategyBudgetInput {
            entity_class: FunEntityRenderClass::Character,
            object_count: 512,
            skeleton_material_bucket_count: 31,
            ..Default::default()
        });

        assert_eq!(
            strategy.target_paths,
            &[FunEntityRenderPathTarget::StandardRaster]
        );
        assert!(strategy.features.keep_skinned_path_separate);
        assert!(strategy.features.lod_and_impostors);
        assert!(strategy.features.never_virtual_geometry);
        assert_eq!(report.max_expected_draw_calls, 31);
    }

    #[test]
    fn foliage_and_aggregate_geometry_is_hostile_to_funvg() {
        let strategy = strategy_for_entity_render_class(FunEntityRenderClass::FoliageAggregate);

        assert_eq!(
            strategy.target_paths,
            &[FunEntityRenderPathTarget::InstancedRaster]
        );
        assert!(strategy.features.specialized_instancing);
        assert!(strategy.features.alpha_overdraw_aware_lod);
        assert!(strategy.features.density_throttling);
        assert!(strategy.features.wind_material_buckets);
        assert!(strategy.features.never_virtual_geometry);
    }

    #[test]
    fn particles_do_not_interact_with_virtual_geometry() {
        let strategy = strategy_for_entity_render_class(FunEntityRenderClass::Particle);

        assert_eq!(
            strategy.target_paths,
            &[FunEntityRenderPathTarget::GpuParticleBuffers]
        );
        assert!(strategy.features.gpu_particle_buffers);
        assert!(strategy.features.compute_simulation);
        assert!(strategy.features.particle_overdraw_report);
        assert!(strategy.features.never_virtual_geometry);
    }

    #[test]
    fn transparent_geometry_is_separate_from_opaque_virtual_geometry() {
        let strategy = strategy_for_entity_render_class(FunEntityRenderClass::TransparentGeometry);

        assert_eq!(
            strategy.target_paths,
            &[FunEntityRenderPathTarget::TransparentPipeline]
        );
        assert!(strategy.features.transparent_draw_report);
        assert!(strategy.features.never_virtual_geometry);
    }

    #[test]
    fn cef_ui_is_post_world_and_excluded_from_temporal_and_world_metrics() {
        let strategy = strategy_for_entity_render_class(FunEntityRenderClass::CefUi);
        let report = evaluate_entity_render_strategy_budget(FunEntityRenderStrategyBudgetInput {
            entity_class: FunEntityRenderClass::CefUi,
            object_count: 1,
            ..Default::default()
        });

        assert_eq!(
            strategy.target_paths,
            &[FunEntityRenderPathTarget::CefPostWorldComposition]
        );
        assert!(strategy.features.post_world_composition);
        assert!(strategy.features.gpu_copy_path);
        assert!(strategy.features.cpu_fallback_measured);
        assert!(strategy.features.cef_excluded_from_temporal_reconstruction);
        assert!(strategy.features.cef_excluded_from_world_geometry_metrics);
        assert_eq!(report.max_expected_draw_calls, 1);
    }

    #[test]
    fn debug_overlays_are_disabled_in_perf_lanes_unless_requested() {
        let strategy = strategy_for_entity_render_class(FunEntityRenderClass::DebugOverlay);
        let disabled = evaluate_entity_render_strategy_budget(FunEntityRenderStrategyBudgetInput {
            entity_class: FunEntityRenderClass::DebugOverlay,
            material_bucket_count: 8,
            debug_requested: false,
            ..Default::default()
        });
        let requested =
            evaluate_entity_render_strategy_budget(FunEntityRenderStrategyBudgetInput {
                entity_class: FunEntityRenderClass::DebugOverlay,
                material_bucket_count: 8,
                debug_requested: true,
                ..Default::default()
            });

        assert_eq!(
            strategy.target_paths,
            &[FunEntityRenderPathTarget::DebugPhase]
        );
        assert!(strategy.features.isolated_debug_phase);
        assert!(strategy.features.disabled_in_perf_lanes);
        assert_eq!(disabled.max_expected_draw_calls, 0);
        assert!(!disabled.perf_lane_allowed);
        assert_eq!(requested.max_expected_draw_calls, 8);
        assert!(requested.perf_lane_allowed);
    }
}
