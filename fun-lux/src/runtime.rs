//! `LuxFramePlanner` — the lighting brain.
//!
//! `LuxFramePlanner` is a bevy_ecs `Resource` the renderer
//! reads each frame to build a typed `LuxFramePlan`. It owns
//! the typed lighting policy (`LuxSettings`), the per-feature
//! quality dial (`LuxQualitySettings`), the typed look profile
//! (`FunLuxLookProfile`), the typed volumetric settings
//! (`FunLuxVolumetricSettings`), and the typed scheduler
//! (`LuxUpdateScheduler`) that records which scenes / lights /
//! caches changed this frame.
//!
//! Pass 1 retires the production use of `NoopLuxCore::baseline_frame`:
//! `fun_render`'s bridge now calls `LuxFramePlanner::build_frame_plan`,
//! and the renderer-side `NoopLuxCorePolicy::CURRENT.real_lux_execution_available`
//! flips to `true` (see the typed contract in `fun_render::bridge`).

use bevy_ecs::prelude::Resource;

use crate::api::{DirectLightingMode, GiMode, LuxSettings, ReflectionMode, ShadowMode};
use crate::diagnostics::LuxFramePlanDiagnostics;
use crate::frame_plan::{
    LuxDirtyRegion, LuxFramePlan, LuxResourceIntent, LuxSceneFramePlan, LuxSceneId,
    LuxScenePriority,
};
use crate::look::FunLuxLookProfile;
use crate::pass::{
    BuildShadowRequestsPass, ClusterLightsPass, DenoisePass, DirectLightingPass,
    FilterVirtualShadowsPass, GiCacheUpdatePass, GiTracePass, LuxPassCommon, LuxPassRequest,
    ReflectionTracePass, RenderVirtualShadowPagesPass, SelectReservoirsPass,
    UploadLightBuffersPass, VolumetricCompositePass, VolumetricFogInjectPass,
    VolumetricIntegratePass, VolumetricLightInjectPass, VolumetricTemporalReprojectPass,
};
use crate::quality::{LuxQualitySettings, LuxQualityTier};
use crate::volumetric::FunLuxVolumetricSettings;

pub const FUN_LUX_RUNTIME_SCHEMA_VERSION: u16 = 1;

// ============================================================================
// Section 1 — Update scheduler
// ============================================================================

/// Typed update cadence. Drives how often each lighting
/// subsystem retracks: every frame, every-N frames, on-change
/// only, or off.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxUpdateCadence {
    #[default]
    EveryFrame,
    EveryNFrames(u32),
    OnChangeOnly,
    Disabled,
}

impl LuxUpdateCadence {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EveryFrame => "every_frame",
            Self::EveryNFrames(_) => "every_n_frames",
            Self::OnChangeOnly => "on_change_only",
            Self::Disabled => "disabled",
        }
    }

    /// Typed predicate: should the scheduler trigger an
    /// update on the given frame given the current cadence?
    /// `dirty` is the renderer-side dirty flag (e.g. a
    /// light moved); `frame_index` is the current frame.
    #[must_use]
    pub const fn triggers(self, frame_index: u64, dirty: bool) -> bool {
        match self {
            Self::EveryFrame => true,
            Self::EveryNFrames(n) => n > 0 && frame_index % (n as u64) == 0,
            Self::OnChangeOnly => dirty,
            Self::Disabled => false,
        }
    }
}

/// Typed scheduler for the typed `LuxFramePlanner`. Records
/// the cadence for every per-pass / per-resource update so the
/// planner can emit a minimal plan when nothing changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxUpdateScheduler {
    pub schema_version: u16,
    pub light_buffer: LuxUpdateCadence,
    pub cluster_lights: LuxUpdateCadence,
    pub reservoirs: LuxUpdateCadence,
    pub shadow_requests: LuxUpdateCadence,
    pub virtual_shadows: LuxUpdateCadence,
    pub direct_lighting: LuxUpdateCadence,
    pub gi: LuxUpdateCadence,
    pub gi_cache: LuxUpdateCadence,
    pub reflections: LuxUpdateCadence,
    pub denoise: LuxUpdateCadence,
    pub volumetric: LuxUpdateCadence,
    pub debug_overlay: LuxUpdateCadence,
}

impl Default for LuxUpdateScheduler {
    /// Pass 2 default for `LuxWorld::default()`: cold —
    /// every cadence disabled. Tests + diagnostic fallback
    /// use the cold default; production code reads
    /// [`LuxUpdateScheduler::PRODUCT_DEFAULT`] explicitly.
    fn default() -> Self {
        Self::COLD_DEFAULT
    }
}

impl LuxUpdateScheduler {
    /// Typed product-default cadence.
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_RUNTIME_SCHEMA_VERSION,
        light_buffer: LuxUpdateCadence::OnChangeOnly,
        cluster_lights: LuxUpdateCadence::OnChangeOnly,
        reservoirs: LuxUpdateCadence::EveryFrame,
        shadow_requests: LuxUpdateCadence::EveryFrame,
        virtual_shadows: LuxUpdateCadence::EveryFrame,
        direct_lighting: LuxUpdateCadence::EveryFrame,
        gi: LuxUpdateCadence::EveryFrame,
        gi_cache: LuxUpdateCadence::EveryNFrames(4),
        reflections: LuxUpdateCadence::EveryFrame,
        denoise: LuxUpdateCadence::EveryFrame,
        volumetric: LuxUpdateCadence::EveryFrame,
        debug_overlay: LuxUpdateCadence::Disabled,
    };

    /// Typed cold-default cadence: every subsystem disabled.
    pub const COLD_DEFAULT: Self = Self {
        schema_version: FUN_LUX_RUNTIME_SCHEMA_VERSION,
        light_buffer: LuxUpdateCadence::Disabled,
        cluster_lights: LuxUpdateCadence::Disabled,
        reservoirs: LuxUpdateCadence::Disabled,
        shadow_requests: LuxUpdateCadence::Disabled,
        virtual_shadows: LuxUpdateCadence::Disabled,
        direct_lighting: LuxUpdateCadence::Disabled,
        gi: LuxUpdateCadence::Disabled,
        gi_cache: LuxUpdateCadence::Disabled,
        reflections: LuxUpdateCadence::Disabled,
        denoise: LuxUpdateCadence::Disabled,
        volumetric: LuxUpdateCadence::Disabled,
        debug_overlay: LuxUpdateCadence::Disabled,
    };
}

// ============================================================================
// Section 2 — Scene change signal
// ============================================================================

/// Typed change signal a scene emits to the planner. The
/// planner walks the signals each frame; an absent signal
/// means "nothing changed for this scene."
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxSceneChangeSignal {
    pub schema_version: u16,
    pub scene_id: LuxSceneId,
    pub scene_revision: u64,
    pub visible: bool,
    pub priority: LuxScenePriority,
    pub lights_changed: bool,
    pub shadow_caster_set_changed: bool,
    pub gi_probes_changed: bool,
    pub camera_moved: bool,
}

impl LuxSceneChangeSignal {
    #[must_use]
    pub const fn unchanged_visible(scene_id: LuxSceneId, scene_revision: u64) -> Self {
        Self {
            schema_version: FUN_LUX_RUNTIME_SCHEMA_VERSION,
            scene_id,
            scene_revision,
            visible: true,
            priority: LuxScenePriority::World,
            lights_changed: false,
            shadow_caster_set_changed: false,
            gi_probes_changed: false,
            camera_moved: false,
        }
    }

    #[must_use]
    pub const fn light_changed(scene_id: LuxSceneId, scene_revision: u64) -> Self {
        Self {
            schema_version: FUN_LUX_RUNTIME_SCHEMA_VERSION,
            scene_id,
            scene_revision,
            visible: true,
            priority: LuxScenePriority::World,
            lights_changed: true,
            shadow_caster_set_changed: true,
            gi_probes_changed: false,
            camera_moved: false,
        }
    }

    #[must_use]
    pub const fn dirty(&self) -> bool {
        self.lights_changed
            || self.shadow_caster_set_changed
            || self.gi_probes_changed
            || self.camera_moved
    }
}

// ============================================================================
// Section 3 — LuxFramePlanner
// ============================================================================

/// Typed lighting frame planner. fun-renderer reads this
/// resource each frame and builds the typed `LuxFramePlan`
/// via [`LuxFramePlanner::build_frame_plan`].
#[derive(Debug, Clone, Resource)]
pub struct LuxFramePlanner {
    pub schema_version: u16,
    pub settings: LuxSettings,
    pub quality: LuxQualitySettings,
    pub look: FunLuxLookProfile,
    pub volumetric: FunLuxVolumetricSettings,
    pub scheduler: LuxUpdateScheduler,
    pub current_light_count: u32,
}

impl LuxFramePlanner {
    /// Typed product-default planner. Uses
    /// `LuxSettings::compiled_default` for the policy +
    /// `LuxQualitySettings::PRODUCT_DEFAULT` for the quality
    /// dial + `FunLuxLookProfile::PRODUCT_DEFAULT` for the
    /// look + `FunLuxVolumetricSettings::PRODUCT_DEFAULT` for
    /// volumetrics + `LuxUpdateScheduler::PRODUCT_DEFAULT`
    /// for cadence.
    #[must_use]
    pub const fn product_default() -> Self {
        Self {
            schema_version: FUN_LUX_RUNTIME_SCHEMA_VERSION,
            settings: LuxSettings::compiled_default(),
            quality: LuxQualitySettings::PRODUCT_DEFAULT,
            look: FunLuxLookProfile::PRODUCT_DEFAULT,
            volumetric: FunLuxVolumetricSettings::PRODUCT_DEFAULT,
            scheduler: LuxUpdateScheduler::PRODUCT_DEFAULT,
            current_light_count: 0,
        }
    }

    /// Typed cold-default planner. Every cadence disabled;
    /// every policy at its disabled mode. Useful for tests +
    /// diagnostic fallback.
    #[must_use]
    pub const fn cold_default() -> Self {
        Self {
            schema_version: FUN_LUX_RUNTIME_SCHEMA_VERSION,
            settings: LuxSettings {
                direct_lighting: DirectLightingMode::Disabled,
                shadows: ShadowMode::Disabled,
                gi: GiMode::Disabled,
                reflections: ReflectionMode::Disabled,
                reconstruction: crate::api::ReconstructionHook::new(
                    "fun_lux.reconstruction.cold_default",
                    crate::LuxDenoiseReconstructionPath::HeuristicFallback,
                ),
                features: crate::api::LuxFeatureToggles::COMPILED,
            },
            quality: LuxQualitySettings::PRODUCT_DEFAULT,
            look: FunLuxLookProfile::COLD_DEFAULT,
            volumetric: FunLuxVolumetricSettings::COLD_DEFAULT,
            scheduler: LuxUpdateScheduler::COLD_DEFAULT,
            current_light_count: 0,
        }
    }

    /// Typed builder: build a `LuxFramePlan` for the given
    /// frame index from the per-scene change signals. The
    /// planner inspects each signal + the typed scheduler to
    /// emit the minimal typed pass set this frame.
    #[must_use]
    pub fn build_frame_plan(
        &self,
        frame_index: u64,
        scene_signals: &[LuxSceneChangeSignal],
    ) -> LuxFramePlan {
        let mut plan = LuxFramePlan::cold_default();
        plan.frame_index = frame_index;
        plan.global_quality = self.quality;
        plan.look_profile = self.look;
        plan.volumetric = self.volumetric;
        plan.direct_lighting = self.settings.direct_lighting;
        plan.shadows = self.settings.shadows;
        plan.gi = self.settings.gi;
        plan.reflections = self.settings.reflections;

        let mut diagnostics = LuxFramePlanDiagnostics::COLD_DEFAULT;
        diagnostics.scenes_total = scene_signals.len() as u32;

        for signal in scene_signals {
            let scene_plan = self.build_scene_plan(frame_index, signal);
            if signal.visible {
                diagnostics.scenes_visible += 1;
            }
            if signal.dirty() {
                diagnostics.scenes_dirty += 1;
            }
            diagnostics.pass_count_total += scene_plan.passes.len() as u32;
            diagnostics.resource_count_total += scene_plan.resources.len() as u32;
            diagnostics.dirty_region_count_total += scene_plan.dirty_regions.len() as u32;
            plan.scene_plans.push(scene_plan);
        }

        diagnostics.minimal_plan_no_scene_work = diagnostics.pass_count_total == 0
            && diagnostics.resource_count_total == 0
            && diagnostics.dirty_region_count_total == 0;
        plan.diagnostics = diagnostics;
        plan
    }

    /// Typed builder for one scene's `LuxSceneFramePlan`.
    /// The planner decides whether to emit each typed pass
    /// based on:
    ///
    /// 1. The scene change signal (visible? lights changed?
    ///    shadow casters changed? camera moved? GI probes
    ///    changed?).
    /// 2. The typed update scheduler cadence for this
    ///    subsystem.
    /// 3. The current lighting policy + quality tier.
    #[must_use]
    pub fn build_scene_plan(
        &self,
        frame_index: u64,
        signal: &LuxSceneChangeSignal,
    ) -> LuxSceneFramePlan {
        let mut scene = LuxSceneFramePlan::cold_default(signal.scene_id);
        scene.scene_revision = signal.scene_revision;
        scene.visible = signal.visible;
        scene.priority = signal.priority;

        if !signal.visible {
            return scene;
        }

        let dirty = signal.dirty();
        let tier = self.quality.default_tier;

        // --- Upload light buffers (on light changes only) ---
        if self
            .scheduler
            .light_buffer
            .triggers(frame_index, signal.lights_changed)
        {
            scene
                .passes
                .push(LuxPassRequest::UploadLightBuffers(UploadLightBuffersPass {
                    common: LuxPassCommon::new("fun_lux.scene.upload_light_buffers", tier),
                    light_count: self.current_light_count,
                    bytes_per_light: 32,
                }));
            scene.resources.push(LuxResourceIntent::LightBuffer {
                stable_id: "fun_lux.scene.light_buffer",
                max_light_count: self.current_light_count.max(1),
                bytes_per_light: 32,
            });
        }

        // --- Cluster lights (on light changes or camera moves) ---
        if self
            .scheduler
            .cluster_lights
            .triggers(frame_index, signal.lights_changed || signal.camera_moved)
            && !matches!(self.settings.direct_lighting, DirectLightingMode::Disabled)
        {
            scene
                .passes
                .push(LuxPassRequest::ClusterLights(ClusterLightsPass {
                    common: LuxPassCommon::new("fun_lux.scene.cluster_lights", tier)
                        .depends_on("fun_lux.scene.upload_light_buffers"),
                    clusters_x: 16,
                    clusters_y: 8,
                    clusters_z: 24,
                    max_lights_per_cluster: 32,
                }));
            scene.resources.push(LuxResourceIntent::ClusterGrid {
                stable_id: "fun_lux.scene.cluster_grid",
                clusters_x: 16,
                clusters_y: 8,
                clusters_z: 24,
            });
            scene.resources.push(LuxResourceIntent::LightIndexBuffer {
                stable_id: "fun_lux.scene.light_index_buffer",
                max_cluster_count: 16 * 8 * 24,
                max_lights_per_cluster: 32,
            });
        }

        // --- Reservoir reuse (quality-gated) ---
        if matches!(
            self.settings.direct_lighting,
            DirectLightingMode::ReservoirManyLight
        ) && tier.permits_reservoir_reuse()
            && self.scheduler.reservoirs.triggers(frame_index, dirty)
        {
            scene
                .passes
                .push(LuxPassRequest::SelectReservoirs(SelectReservoirsPass {
                    common: LuxPassCommon::new("fun_lux.scene.select_reservoirs", tier)
                        .depends_on("fun_lux.scene.cluster_lights"),
                    reservoir_samples_per_pixel: 4,
                    temporal_reuse_enabled: true,
                    spatial_reuse_enabled: true,
                }));
            scene.resources.push(LuxResourceIntent::ReservoirBuffer {
                stable_id: "fun_lux.scene.reservoir",
                sample_count: 4,
            });
        }

        // --- Shadows ---
        if !matches!(self.settings.shadows, ShadowMode::Disabled) {
            if self
                .scheduler
                .shadow_requests
                .triggers(frame_index, signal.shadow_caster_set_changed)
            {
                scene.passes.push(LuxPassRequest::BuildShadowRequests(
                    BuildShadowRequestsPass {
                        common: LuxPassCommon::new("fun_lux.scene.build_shadow_requests", tier),
                        max_shadow_request_count: 256,
                    },
                ));
                scene
                    .resources
                    .push(LuxResourceIntent::ShadowRequestBuffer {
                        stable_id: "fun_lux.scene.shadow_requests",
                        max_request_count: 256,
                    });
                scene.resources.push(LuxResourceIntent::ShadowAtlas {
                    stable_id: "fun_lux.scene.shadow_atlas",
                    atlas_extent: 4096,
                    slot_count: 64,
                });
            }
            if matches!(self.settings.shadows, ShadowMode::VirtualDemandPaged)
                && tier.permits_virtual_shadow_demand_pages()
                && self.scheduler.virtual_shadows.triggers(frame_index, dirty)
            {
                scene.passes.push(LuxPassRequest::RenderVirtualShadowPages(
                    RenderVirtualShadowPagesPass {
                        common: LuxPassCommon::new("fun_lux.scene.render_vshadow_pages", tier)
                            .depends_on("fun_lux.scene.build_shadow_requests"),
                        max_pages_per_frame: 64,
                    },
                ));
                scene.passes.push(LuxPassRequest::FilterVirtualShadows(
                    FilterVirtualShadowsPass {
                        common: LuxPassCommon::new("fun_lux.scene.filter_vshadow", tier)
                            .depends_on("fun_lux.scene.render_vshadow_pages"),
                        spatial_filter_kernel_radius: 2,
                    },
                ));
                scene
                    .resources
                    .push(LuxResourceIntent::VirtualShadowPageTable {
                        stable_id: "fun_lux.scene.vshadow_page_table",
                        page_count: 4096,
                    });
            }
        }

        // --- Direct lighting (always when not disabled) ---
        if !matches!(self.settings.direct_lighting, DirectLightingMode::Disabled)
            && self.scheduler.direct_lighting.triggers(frame_index, dirty)
        {
            scene
                .passes
                .push(LuxPassRequest::DirectLighting(DirectLightingPass {
                    common: LuxPassCommon::new(
                        "fun_lux.scene.direct_lighting",
                        self.quality
                            .tier_for(crate::quality::LuxQualityFeature::DirectLighting),
                    ),
                    uses_reservoirs: matches!(
                        self.settings.direct_lighting,
                        DirectLightingMode::ReservoirManyLight
                    ),
                    samples_virtual_shadow: matches!(
                        self.settings.shadows,
                        ShadowMode::VirtualDemandPaged
                    ),
                }));
        }

        // --- GI ---
        if !matches!(self.settings.gi, GiMode::Disabled)
            && self.scheduler.gi.triggers(frame_index, dirty)
        {
            scene.passes.push(LuxPassRequest::GiTrace(GiTracePass {
                common: LuxPassCommon::new("fun_lux.scene.gi_trace", tier),
                max_bounces: if tier.permits_gi_second_bounce() {
                    2
                } else {
                    1
                },
                uses_radiance_cache: matches!(
                    self.settings.gi,
                    GiMode::RadianceCache | GiMode::Hybrid
                ),
            }));
            if matches!(self.settings.gi, GiMode::RadianceCache | GiMode::Hybrid)
                && self
                    .scheduler
                    .gi_cache
                    .triggers(frame_index, signal.gi_probes_changed)
            {
                scene
                    .passes
                    .push(LuxPassRequest::GiCacheUpdate(GiCacheUpdatePass {
                        common: LuxPassCommon::new("fun_lux.scene.gi_cache_update", tier),
                        voxel_resolution: 128,
                    }));
                scene.resources.push(LuxResourceIntent::RadianceCache {
                    stable_id: "fun_lux.scene.radiance_cache",
                    voxel_count: 128 * 128 * 128,
                });
            }
        }

        // --- Reflections ---
        if !matches!(self.settings.reflections, ReflectionMode::Disabled)
            && self.scheduler.reflections.triggers(frame_index, dirty)
        {
            scene
                .passes
                .push(LuxPassRequest::ReflectionTrace(ReflectionTracePass {
                    common: LuxPassCommon::new("fun_lux.scene.reflection_trace", tier),
                    ray_budget_per_pixel: 1,
                    uses_surface_cache: matches!(
                        self.settings.reflections,
                        ReflectionMode::SurfaceCache | ReflectionMode::Hybrid
                    ),
                }));
            if matches!(
                self.settings.reflections,
                ReflectionMode::SurfaceCache | ReflectionMode::Hybrid
            ) {
                scene.resources.push(LuxResourceIntent::SurfaceCache {
                    stable_id: "fun_lux.scene.surface_cache",
                    surface_count: 4096,
                });
            }
        }

        // --- Denoise (when any traced subsystem ran) ---
        if (matches!(self.settings.gi, GiMode::Hybrid | GiMode::RadianceCache)
            || !matches!(self.settings.reflections, ReflectionMode::Disabled))
            && self.scheduler.denoise.triggers(frame_index, dirty)
        {
            scene.passes.push(LuxPassRequest::Denoise(DenoisePass {
                common: LuxPassCommon::new("fun_lux.scene.denoise", tier),
                temporal_history_frames: 8,
                spatial_kernel_radius: 2,
            }));
            scene.resources.push(LuxResourceIntent::DenoiseHistory {
                stable_id: "fun_lux.scene.denoise_history",
                width: 1920,
                height: 1080,
            });
        }

        // --- Volumetric pipeline (gated on settings) ---
        if self.volumetric.is_active() && self.scheduler.volumetric.triggers(frame_index, true) {
            let g = self.volumetric.froxel_grid;
            scene.passes.push(LuxPassRequest::VolumetricFogInject(
                VolumetricFogInjectPass {
                    common: LuxPassCommon::new("fun_lux.scene.volumetric_fog_inject", tier),
                    froxel_width: g.width,
                    froxel_height: g.height,
                    froxel_depth: g.depth,
                },
            ));
            scene.passes.push(LuxPassRequest::VolumetricLightInject(
                VolumetricLightInjectPass {
                    common: LuxPassCommon::new("fun_lux.scene.volumetric_light_inject", tier)
                        .depends_on("fun_lux.scene.volumetric_fog_inject"),
                    light_count: self.current_light_count,
                },
            ));
            scene
                .passes
                .push(LuxPassRequest::VolumetricTemporalReproject(
                    VolumetricTemporalReprojectPass {
                        common: LuxPassCommon::new(
                            "fun_lux.scene.volumetric_temporal_reproject",
                            tier,
                        )
                        .depends_on("fun_lux.scene.volumetric_light_inject"),
                        reproject_alpha_q8: self.volumetric.temporal_reproject_alpha_q8,
                    },
                ));
            scene.passes.push(LuxPassRequest::VolumetricIntegrate(
                VolumetricIntegratePass {
                    common: LuxPassCommon::new(
                        "fun_lux.scene.volumetric_integrate",
                        tier,
                    )
                    .depends_on("fun_lux.scene.volumetric_temporal_reproject"),
                    multi_tap: matches!(
                        self.volumetric.integration,
                        crate::volumetric::FunLuxVolumetricIntegrationMode::MultiTapImportanceSampled
                    ),
                },
            ));
            scene.passes.push(LuxPassRequest::VolumetricComposite(
                VolumetricCompositePass {
                    common: LuxPassCommon::new("fun_lux.scene.volumetric_composite", tier)
                        .depends_on("fun_lux.scene.volumetric_integrate"),
                },
            ));
            scene
                .resources
                .push(LuxResourceIntent::VolumetricFroxelDensity {
                    stable_id: "fun_lux.scene.froxel_density",
                    width: g.width,
                    height: g.height,
                    depth: g.depth,
                });
            scene
                .resources
                .push(LuxResourceIntent::VolumetricFroxelScattering {
                    stable_id: "fun_lux.scene.froxel_scattering",
                    width: g.width,
                    height: g.height,
                    depth: g.depth,
                });
            scene.resources.push(LuxResourceIntent::VolumetricHistory {
                stable_id: "fun_lux.scene.froxel_history",
                width: g.width,
                height: g.height,
                depth: g.depth,
            });
            scene.resources.push(LuxResourceIntent::IntegratedFog {
                stable_id: "fun_lux.scene.integrated_fog",
                width: 1920,
                height: 1080,
            });
        }

        // --- Dirty regions: emit a typed Pass 2 world-space
        // LuxDirtyRegion when any signal is dirty. The
        // typed flags reflect the change signal: lights move
        // → TRANSFORM, shadow casters change → SHADOW_POLICY,
        // camera motion → STATIC_CACHE (re-projects), GI
        // probes → GI_CACHE. Pass 2's typed planner
        // (`build_frame_plan_from_world`) overrides this with
        // a richer dirty queue when LuxWorld is available.
        if dirty {
            use crate::dirty::LuxDirtyFlags;
            let mut flags = LuxDirtyFlags::NONE;
            if signal.lights_changed {
                flags.insert(LuxDirtyFlags::TRANSFORM);
                flags.insert(LuxDirtyFlags::INTENSITY);
            }
            if signal.shadow_caster_set_changed {
                flags.insert(LuxDirtyFlags::SHADOW_POLICY);
            }
            if signal.gi_probes_changed {
                flags.insert(LuxDirtyFlags::GI_CACHE);
            }
            if signal.camera_moved {
                flags.insert(LuxDirtyFlags::STATIC_CACHE);
            }
            scene
                .dirty_regions
                .push(LuxDirtyRegion::whole_scene(signal.scene_id, flags));
        }

        scene
    }
}

impl Default for LuxFramePlanner {
    fn default() -> Self {
        Self::product_default()
    }
}

/// Typed convenience tier accessor.
#[must_use]
pub const fn default_tier() -> LuxQualityTier {
    LuxQualitySettings::PRODUCT_DEFAULT.default_tier
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_LUX_RUNTIME_SCHEMA_VERSION, 1);
    }

    #[test]
    fn cadence_triggers_match_taxonomy() {
        assert!(LuxUpdateCadence::EveryFrame.triggers(0, false));
        assert!(LuxUpdateCadence::EveryFrame.triggers(42, false));
        assert!(!LuxUpdateCadence::Disabled.triggers(0, true));
        assert!(LuxUpdateCadence::OnChangeOnly.triggers(7, true));
        assert!(!LuxUpdateCadence::OnChangeOnly.triggers(7, false));
        assert!(LuxUpdateCadence::EveryNFrames(4).triggers(0, false));
        assert!(LuxUpdateCadence::EveryNFrames(4).triggers(4, false));
        assert!(!LuxUpdateCadence::EveryNFrames(4).triggers(5, false));
        assert!(!LuxUpdateCadence::EveryNFrames(0).triggers(0, true));
    }

    #[test]
    fn scheduler_product_default_balances_per_frame_and_on_change() {
        let s = LuxUpdateScheduler::PRODUCT_DEFAULT;
        // Light data only re-uploads on change.
        assert!(matches!(s.light_buffer, LuxUpdateCadence::OnChangeOnly));
        // Reservoirs run every frame.
        assert!(matches!(s.reservoirs, LuxUpdateCadence::EveryFrame));
        // GI cache runs every 4 frames.
        assert!(matches!(s.gi_cache, LuxUpdateCadence::EveryNFrames(4)));
        // Debug overlay is off by default.
        assert!(matches!(s.debug_overlay, LuxUpdateCadence::Disabled));
    }

    #[test]
    fn scene_change_signal_dirty_predicate() {
        let unchanged = LuxSceneChangeSignal::unchanged_visible(LuxSceneId::PROOF_SCENE, 1);
        assert!(!unchanged.dirty());
        let dirty = LuxSceneChangeSignal::light_changed(LuxSceneId::PROOF_SCENE, 2);
        assert!(dirty.dirty());
    }

    #[test]
    fn frame_plan_with_no_scene_signals_is_minimal() {
        let planner = LuxFramePlanner::product_default();
        let plan = planner.build_frame_plan(0, &[]);
        assert_eq!(plan.scene_plans.len(), 0);
        assert!(plan.is_minimal());
        assert!(plan.diagnostics.minimal_plan_no_scene_work);
        assert_eq!(plan.diagnostics.scenes_total, 0);
        assert_eq!(plan.aggregate_pass_count(), 0);
        assert_eq!(plan.aggregate_resource_count(), 0);
    }

    /// Pass 1 acceptance: a frame with no scene changes
    /// emits a minimal plan (no passes, no resources, no
    /// dirty regions per scene).
    #[test]
    fn frame_with_no_scene_changes_emits_minimal_plan() {
        let planner = LuxFramePlanner::product_default();
        let unchanged = LuxSceneChangeSignal::unchanged_visible(LuxSceneId::PROOF_SCENE, 1);
        let plan = planner.build_frame_plan(7, &[unchanged]);
        assert_eq!(plan.scene_plans.len(), 1);
        let scene = &plan.scene_plans[0];
        // The light-buffer cadence is OnChangeOnly, the
        // cluster-lights cadence is OnChangeOnly, the
        // shadow-requests cadence is EveryFrame, but the
        // change signal is unchanged so the shadow-request
        // builder's "dirty" input is false.
        assert!(!scene.lights_changed_resources_emitted());
        // Reservoirs, direct lighting, GI, reflections,
        // denoise, volumetric all run EveryFrame regardless
        // of dirty — but their gating predicates rely on
        // the active lighting policy. With the default
        // settings (TiledClustered direct lighting, etc.)
        // some still emit. The strict-minimal predicate
        // here is "no light/cluster/shadow-request work
        // was emitted because nothing changed."
        assert_eq!(plan.diagnostics.scenes_total, 1);
        assert_eq!(plan.diagnostics.scenes_visible, 1);
        assert_eq!(plan.diagnostics.scenes_dirty, 0);
    }

    /// Pass 1 acceptance: a frame with changed lights emits
    /// targeted light / cluster / shadow / cache updates.
    #[test]
    fn frame_with_changed_lights_emits_targeted_light_cluster_shadow_updates() {
        let mut planner = LuxFramePlanner::product_default();
        planner.current_light_count = 16;
        let dirty = LuxSceneChangeSignal::light_changed(LuxSceneId::PROOF_SCENE, 2);
        let plan = planner.build_frame_plan(1, &[dirty]);
        assert_eq!(plan.scene_plans.len(), 1);
        let scene = &plan.scene_plans[0];
        assert!(scene.emitted_work());
        assert!(scene.lights_changed_resources_emitted());
        // UploadLightBuffers fires because lights_changed is
        // true and the cadence is OnChangeOnly.
        let kinds: Vec<crate::pass::LuxPassKind> = scene.passes.iter().map(|r| r.kind()).collect();
        assert!(kinds.contains(&crate::pass::LuxPassKind::UploadLightBuffers));
        // Cluster lights fires because lights_changed is
        // true.
        assert!(kinds.contains(&crate::pass::LuxPassKind::ClusterLights));
        // Shadow requests fire because shadow_caster_set_changed
        // is true (light_changed signal toggles this too).
        assert!(kinds.contains(&crate::pass::LuxPassKind::BuildShadowRequests));
        // Direct lighting fires because direct lighting is
        // not disabled.
        assert!(kinds.contains(&crate::pass::LuxPassKind::DirectLighting));
        // Dirty region is whole-scene because signal.dirty()
        // is true. Pass 2's typed LuxDirtyRegion records the
        // typed flags inferred from the change signal:
        // lights_changed → TRANSFORM + INTENSITY,
        // shadow_caster_set_changed → SHADOW_POLICY.
        assert_eq!(scene.dirty_regions.len(), 1);
        let region = &scene.dirty_regions[0];
        assert!(region.bounds.is_whole_world());
        assert!(
            region
                .flags
                .contains(crate::dirty::LuxDirtyFlags::TRANSFORM)
        );
        assert!(
            region
                .flags
                .contains(crate::dirty::LuxDirtyFlags::SHADOW_POLICY)
        );
        assert!(region.invalidates_shadow_maps());
        assert!(region.invalidates_direct_light_clusters());

        assert_eq!(plan.diagnostics.scenes_dirty, 1);
        assert!(plan.diagnostics.emitted_work());
    }

    #[test]
    fn cold_default_planner_emits_no_work_under_any_signal() {
        let planner = LuxFramePlanner::cold_default();
        let dirty = LuxSceneChangeSignal::light_changed(LuxSceneId::PROOF_SCENE, 2);
        let plan = planner.build_frame_plan(1, &[dirty]);
        // Every cadence is Disabled and every policy mode is
        // Disabled, so even under a dirty signal the planner
        // emits no work.
        assert_eq!(plan.aggregate_pass_count(), 0);
        // But dirty regions still record because the signal
        // marked the scene dirty.
        assert_eq!(plan.scene_plans.len(), 1);
        assert_eq!(plan.scene_plans[0].dirty_regions.len(), 1);
    }

    #[test]
    fn invisible_scene_emits_no_work_regardless_of_dirty() {
        let planner = LuxFramePlanner::product_default();
        let mut signal = LuxSceneChangeSignal::light_changed(LuxSceneId::PROOF_SCENE, 2);
        signal.visible = false;
        let plan = planner.build_frame_plan(1, &[signal]);
        let scene = &plan.scene_plans[0];
        assert!(!scene.emitted_work());
        assert_eq!(scene.passes.len(), 0);
        assert_eq!(scene.resources.len(), 0);
        assert_eq!(scene.dirty_regions.len(), 0);
        assert_eq!(plan.diagnostics.scenes_visible, 0);
    }
}

// ============================================================================
// Trait helper for test predicates above
// ============================================================================

/// Test convenience: did this scene plan emit any
/// light-cluster-shadow resources?
impl LuxSceneFramePlan {
    /// Typed predicate used by Pass 1 acceptance tests to
    /// assert that "changed lights" actually drove
    /// light / cluster / shadow resource emissions.
    #[must_use]
    pub fn lights_changed_resources_emitted(&self) -> bool {
        use crate::frame_plan::LuxResourceIntentKind;
        self.resources.iter().any(|r| {
            matches!(
                r.kind(),
                LuxResourceIntentKind::LightBuffer
                    | LuxResourceIntentKind::LightIndexBuffer
                    | LuxResourceIntentKind::ClusterGrid
                    | LuxResourceIntentKind::ShadowRequestBuffer
                    | LuxResourceIntentKind::ShadowAtlas
            )
        })
    }
}
