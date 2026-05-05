use bevy::solari::prelude::{SolariInternalScale, SolariSettings};

use crate::{ClientOpaqueRenderer, ClientRenderConfig, ClientRenderProfile, FunRenderAppOptions};

/// Stable draw/material bucket used by the game-facing render path arbiter and draw counters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FunMaterialClass {
    #[default]
    OpaqueSimple,
    OpaqueComplex,
    Transparent,
    Emissive,
    Viewmodel,
    Ui,
    Debug,
}

impl FunMaterialClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpaqueSimple => "opaque_simple",
            Self::OpaqueComplex => "opaque_complex",
            Self::Transparent => "transparent",
            Self::Emissive => "emissive",
            Self::Viewmodel => "viewmodel",
            Self::Ui => "ui",
            Self::Debug => "debug",
        }
    }
}

/// Stable entity/geometry bucket for draw-call accounting.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, bevy::prelude::Component,
)]
pub enum FunGeometryClass {
    StaticOpaqueDense,
    #[default]
    StaticOpaqueSimple,
    DynamicOpaque,
    SkinnedCharacter,
    Vehicle,
    Foliage,
    Particle,
    Decal,
    Viewmodel,
    Ui,
}

impl FunGeometryClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StaticOpaqueDense => "static_opaque_dense",
            Self::StaticOpaqueSimple => "static_opaque_simple",
            Self::DynamicOpaque => "dynamic_opaque",
            Self::SkinnedCharacter => "skinned_character",
            Self::Vehicle => "vehicle",
            Self::Foliage => "foliage",
            Self::Particle => "particle",
            Self::Decal => "decal",
            Self::Viewmodel => "viewmodel",
            Self::Ui => "ui",
        }
    }

    pub const fn is_static(self) -> bool {
        matches!(
            self,
            Self::StaticOpaqueDense | Self::StaticOpaqueSimple | Self::Foliage
        )
    }
}

/// Approximate camera distance band used by [`FunRenderPathArbiter`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FunRenderDistanceBand {
    Near,
    #[default]
    Mid,
    Far,
}

impl FunRenderDistanceBand {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Near => "near",
            Self::Mid => "mid",
            Self::Far => "far",
        }
    }
}

/// Stable game-facing render path selected before Bevy-specific render setup.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, bevy::prelude::Component,
)]
pub enum FunRenderPath {
    #[default]
    StandardRaster,
    InstancedRaster,
    GpuCulledIndirect,
    MeshletStaticDense,
    MeshletDynamicDense,
    RayProxyOnly,
    Viewmodel,
    CefUi,
    DebugOnly,
}

impl FunRenderPath {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StandardRaster => "standard_raster",
            Self::InstancedRaster => "instanced_raster",
            Self::GpuCulledIndirect => "gpu_culled_indirect",
            Self::MeshletStaticDense => "meshlet_static_dense",
            Self::MeshletDynamicDense => "meshlet_dynamic_dense",
            Self::RayProxyOnly => "ray_proxy_only",
            Self::Viewmodel => "viewmodel",
            Self::CefUi => "cef_ui",
            Self::DebugOnly => "debug_only",
        }
    }

    pub const fn uses_meshlet(self) -> bool {
        matches!(self, Self::MeshletStaticDense | Self::MeshletDynamicDense)
    }

    pub const fn emits_visible_raster(self) -> bool {
        !matches!(self, Self::RayProxyOnly)
    }
}

/// Quantized policy inputs for choosing the game-owned render path.
#[derive(Debug, Clone, Copy)]
pub struct FunRenderPathInput {
    pub triangle_count: u32,
    pub meshlet_count: u32,
    pub projected_screen_area: f32,
    pub material_class: FunMaterialClass,
    pub geometry_class: FunGeometryClass,
    pub is_static: bool,
    pub instance_count: u32,
    pub shared_mesh_material: bool,
    pub simple_transforms: bool,
    pub high_cpu_culling_cost: bool,
    pub stable_buffer_layout: bool,
    pub overdraw_estimate: f32,
    pub distance_band: FunRenderDistanceBand,
    pub ray_proxy_only: bool,
    pub viewmodel: bool,
    pub cef_ui: bool,
    pub debug_only: bool,
    pub msaa_required: bool,
}

impl Default for FunRenderPathInput {
    fn default() -> Self {
        Self {
            triangle_count: 0,
            meshlet_count: 0,
            projected_screen_area: 0.0,
            material_class: FunMaterialClass::OpaqueSimple,
            geometry_class: FunGeometryClass::StaticOpaqueSimple,
            is_static: true,
            instance_count: 1,
            shared_mesh_material: false,
            simple_transforms: true,
            high_cpu_culling_cost: false,
            stable_buffer_layout: true,
            overdraw_estimate: 1.0,
            distance_band: FunRenderDistanceBand::Mid,
            ray_proxy_only: false,
            viewmodel: false,
            cef_ui: false,
            debug_only: false,
            msaa_required: false,
        }
    }
}

/// Game-owned render path policy. The values mirror Bevy's meshlet arbiter but
/// keep game-specific shortcuts visible to diagnostics and benchmarks.
#[derive(Debug, Clone, Copy)]
pub struct FunRenderPathArbiter {
    pub static_meshlet_min_triangles: u32,
    pub dynamic_meshlet_min_triangles: u32,
    pub min_meshlets: u32,
    pub min_projected_screen_area: f32,
    pub high_overdraw_threshold: f32,
    pub instanced_min_instances: u32,
    pub gpu_culled_min_instances: u32,
}

impl Default for FunRenderPathArbiter {
    fn default() -> Self {
        Self {
            static_meshlet_min_triangles: 512,
            dynamic_meshlet_min_triangles: 2_048,
            min_meshlets: 8,
            min_projected_screen_area: 0.03,
            high_overdraw_threshold: 1.6,
            instanced_min_instances: 8,
            gpu_culled_min_instances: 128,
        }
    }
}

impl FunRenderPathArbiter {
    pub fn decide(&self, input: FunRenderPathInput) -> FunRenderPath {
        if input.debug_only || input.material_class == FunMaterialClass::Debug {
            return FunRenderPath::DebugOnly;
        }

        if input.cef_ui || input.geometry_class == FunGeometryClass::Ui {
            return FunRenderPath::CefUi;
        }

        if input.ray_proxy_only {
            return FunRenderPath::RayProxyOnly;
        }

        if input.viewmodel
            || input.material_class == FunMaterialClass::Viewmodel
            || input.geometry_class == FunGeometryClass::Viewmodel
        {
            return FunRenderPath::Viewmodel;
        }

        if input.material_class == FunMaterialClass::Transparent
            || input.geometry_class == FunGeometryClass::Decal
        {
            return FunRenderPath::StandardRaster;
        }

        if self.meshlet_candidate_is_dense(input) {
            return if input.is_static {
                FunRenderPath::MeshletStaticDense
            } else {
                FunRenderPath::MeshletDynamicDense
            };
        }

        if self.gpu_culled_raster_candidate(input) {
            return FunRenderPath::GpuCulledIndirect;
        }

        if input.shared_mesh_material
            && input.simple_transforms
            && input.instance_count >= self.instanced_min_instances
        {
            return FunRenderPath::InstancedRaster;
        }

        FunRenderPath::StandardRaster
    }

    fn meshlet_candidate_is_dense(&self, input: FunRenderPathInput) -> bool {
        if input.msaa_required {
            return false;
        }

        if !matches!(
            input.material_class,
            FunMaterialClass::OpaqueSimple
                | FunMaterialClass::OpaqueComplex
                | FunMaterialClass::Emissive
        ) {
            return false;
        }

        if input.is_static && input.geometry_class != FunGeometryClass::StaticOpaqueDense {
            return false;
        }

        if !input.is_static && input.geometry_class != FunGeometryClass::DynamicOpaque {
            return false;
        }

        if input.distance_band == FunRenderDistanceBand::Far
            && input.projected_screen_area < self.min_projected_screen_area * 2.0
        {
            return false;
        }

        let triangle_threshold = if input.is_static {
            self.static_meshlet_min_triangles
        } else {
            self.dynamic_meshlet_min_triangles
        };
        let far_penalty = u32::from(input.distance_band == FunRenderDistanceBand::Far);
        let complex_material_bias = u32::from(matches!(
            input.material_class,
            FunMaterialClass::OpaqueComplex | FunMaterialClass::Emissive
        ));
        let overdraw_bias = u32::from(input.overdraw_estimate >= self.high_overdraw_threshold);
        let effective_meshlet_count = input
            .meshlet_count
            .saturating_add(complex_material_bias)
            .saturating_add(overdraw_bias)
            .saturating_sub(far_penalty);

        input.triangle_count >= triangle_threshold
            && effective_meshlet_count >= self.min_meshlets
            && input.projected_screen_area >= self.min_projected_screen_area
    }

    fn gpu_culled_raster_candidate(&self, input: FunRenderPathInput) -> bool {
        matches!(
            input.material_class,
            FunMaterialClass::OpaqueSimple
                | FunMaterialClass::OpaqueComplex
                | FunMaterialClass::Emissive
        ) && !matches!(
            input.geometry_class,
            FunGeometryClass::Viewmodel
                | FunGeometryClass::Ui
                | FunGeometryClass::Particle
                | FunGeometryClass::Decal
        ) && input.high_cpu_culling_cost
            && input.stable_buffer_layout
            && input.shared_mesh_material
            && input.instance_count >= self.gpu_culled_min_instances
    }
}

const FUN_RENDER_PATH_SIGNATURE_ID: &str = "fun-render-path-v1";
const FUN_RENDERER_ID: &str = "fun_render";
const BEVY_RENDER_FEATURES: &str = "local-bevy:bevy_solari+meshlet+meshlet_processor";
const MATERIAL_PIPELINE_ID: &str = "fun-material-pso-consolidated-v1";
const RENDER_TARGET_FORMAT: &str = "fun-main-color-hdr-rgba16f";
const HDR_TONEMAPPING: &str = "hdr-camera-default-tonemapping";
const SHADOW_RAYTRACING_CONFIG: &str = "solari-ray-proxy";
const CATALOG_VERSION: &str = "demo-render-catalog-v1";
const SHADER_PROFILE_FLAGS: &str = "fun-render-default-shader-profile";

#[derive(Debug, Clone, Copy, PartialEq, Eq, bevy::prelude::Resource)]
pub struct RenderPathSignature {
    pub id: &'static str,
    pub renderer_id: &'static str,
    pub bevy_render_features: &'static str,
    pub render_profile: ClientRenderProfile,
    pub solari_enabled: bool,
    pub dlss_rr_enabled: bool,
    pub meshlets_enabled: bool,
    pub clouds_enabled: bool,
    pub cloud_quality: &'static str,
    pub cloud_internal_scale: &'static str,
    pub cloud_profile_id: &'static str,
    pub cloud_temporal_enabled: bool,
    pub cloud_shadows_enabled: bool,
    pub cloud_debug_overlay: &'static str,
    pub geometry_policy: crate::RenderGeometryPolicy,
    pub meshlet_min_triangles: usize,
    pub opaque_renderer: ClientOpaqueRenderer,
    pub rt_feature_hash: u64,
    pub material_pipeline_id: &'static str,
    pub render_target_format: &'static str,
    pub hdr_tonemapping: &'static str,
    pub internal_scale: &'static str,
    pub shadow_raytracing_config: &'static str,
    pub catalog_version: &'static str,
    pub shader_profile_flags: &'static str,
    pub cloud_shader_profile_flags: &'static str,
}

pub fn render_path_signature_for_options(
    options: &FunRenderAppOptions,
    render_config: &ClientRenderConfig,
    solari_settings: &SolariSettings,
    opaque_renderer: ClientOpaqueRenderer,
) -> RenderPathSignature {
    RenderPathSignature {
        id: FUN_RENDER_PATH_SIGNATURE_ID,
        renderer_id: FUN_RENDERER_ID,
        bevy_render_features: BEVY_RENDER_FEATURES,
        render_profile: options.render_profile,
        solari_enabled: render_config.solari_enabled,
        dlss_rr_enabled: render_config.dlss_rr_enabled,
        meshlets_enabled: render_config.meshlets_enabled,
        clouds_enabled: render_config.clouds_enabled,
        cloud_quality: render_config.cloud_quality.as_env_value(),
        cloud_internal_scale: render_config.cloud_internal_scale.as_env_value(),
        cloud_profile_id: render_config.cloud_profile_id.as_str(),
        cloud_temporal_enabled: render_config.cloud_temporal_enabled,
        cloud_shadows_enabled: render_config.cloud_shadows_enabled,
        cloud_debug_overlay: render_config.cloud_debug_overlay.as_env_value(),
        geometry_policy: render_config.geometry_policy,
        meshlet_min_triangles: render_config.meshlet_min_triangles,
        opaque_renderer,
        rt_feature_hash: render_config.rt_features.rt_feature_hash(),
        material_pipeline_id: MATERIAL_PIPELINE_ID,
        render_target_format: RENDER_TARGET_FORMAT,
        hdr_tonemapping: HDR_TONEMAPPING,
        internal_scale: internal_scale_label(solari_settings.internal_scale),
        shadow_raytracing_config: SHADOW_RAYTRACING_CONFIG,
        catalog_version: CATALOG_VERSION,
        shader_profile_flags: SHADER_PROFILE_FLAGS,
        cloud_shader_profile_flags: render_config.cloud_settings().shader_profile_flags(),
    }
}

const fn internal_scale_label(internal_scale: SolariInternalScale) -> &'static str {
    match internal_scale {
        SolariInternalScale::Full => "1.0",
        SolariInternalScale::ThreeQuarter => "0.75",
        SolariInternalScale::TwoThirds => "0.66",
        SolariInternalScale::Half => "0.5",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        FunCloudDebugOverlay, FunCloudInternalScale, FunCloudQuality, FunRenderPresentation,
        FunRenderRtFeatures, FunWeatherProfileId, NativeDlssConfig, RenderGeometryPolicy,
    };

    fn render_config_with_clouds(
        quality: FunCloudQuality,
        profile_id: FunWeatherProfileId,
    ) -> ClientRenderConfig {
        ClientRenderConfig {
            solari_enabled: true,
            dlss_rr_enabled: false,
            native_dlss: NativeDlssConfig::default(),
            meshlets_enabled: true,
            clouds_enabled: quality != FunCloudQuality::Off,
            cloud_quality: quality,
            cloud_internal_scale: FunCloudInternalScale::Half,
            cloud_temporal_enabled: true,
            cloud_shadows_enabled: false,
            cloud_profile_id: profile_id,
            cloud_debug_overlay: FunCloudDebugOverlay::None,
            dlss_rr_disabled_by_denoise_mode: false,
            #[cfg(all(feature = "render_diagnostics", debug_assertions))]
            render_profile_verbose: false,
            geometry_policy: RenderGeometryPolicy::Hybrid,
            meshlet_min_triangles: 512,
            rt_features: FunRenderRtFeatures::default(),
            #[cfg(debug_assertions)]
            fps_overlay_enabled: true,
        }
    }

    #[test]
    fn render_path_signature_changes_when_cloud_config_changes() {
        let options = FunRenderAppOptions {
            runtime_mode: "test",
            render_profile: ClientRenderProfile::Default,
            hosted_by_editor: false,
            presentation: FunRenderPresentation::EditorOffscreen,
        };
        let solari_settings = SolariSettings::default();

        let scattered = render_path_signature_for_options(
            &options,
            &render_config_with_clouds(FunCloudQuality::Balanced, FunWeatherProfileId::Scattered),
            &solari_settings,
            ClientOpaqueRenderer::Deferred,
        );
        let storm = render_path_signature_for_options(
            &options,
            &render_config_with_clouds(FunCloudQuality::Cinematic, FunWeatherProfileId::StormFront),
            &solari_settings,
            ClientOpaqueRenderer::Deferred,
        );

        assert_ne!(scattered.cloud_quality, storm.cloud_quality);
        assert_ne!(scattered.cloud_profile_id, storm.cloud_profile_id);
    }

    #[test]
    fn small_transparent_object_uses_standard_raster() {
        let path = FunRenderPathArbiter::default().decide(FunRenderPathInput {
            triangle_count: 64,
            meshlet_count: 1,
            material_class: FunMaterialClass::Transparent,
            ..Default::default()
        });

        assert_eq!(path, FunRenderPath::StandardRaster);
    }

    #[test]
    fn dense_static_opaque_object_uses_meshlet_static_dense() {
        let path = FunRenderPathArbiter::default().decide(FunRenderPathInput {
            triangle_count: 4_096,
            meshlet_count: 64,
            projected_screen_area: 0.2,
            material_class: FunMaterialClass::OpaqueComplex,
            geometry_class: FunGeometryClass::StaticOpaqueDense,
            is_static: true,
            ..Default::default()
        });

        assert_eq!(path, FunRenderPath::MeshletStaticDense);
    }

    #[test]
    fn many_static_opaque_instances_use_gpu_culled_raster_when_not_meshlet_dense() {
        let path = FunRenderPathArbiter::default().decide(FunRenderPathInput {
            triangle_count: 128,
            meshlet_count: 2,
            projected_screen_area: 0.08,
            material_class: FunMaterialClass::OpaqueSimple,
            geometry_class: FunGeometryClass::StaticOpaqueSimple,
            is_static: true,
            instance_count: 256,
            shared_mesh_material: true,
            high_cpu_culling_cost: true,
            stable_buffer_layout: true,
            ..Default::default()
        });

        assert_eq!(path, FunRenderPath::GpuCulledIndirect);
    }

    #[test]
    fn dense_dynamic_requires_higher_threshold() {
        let arbiter = FunRenderPathArbiter::default();
        let below_dynamic_threshold = FunRenderPathInput {
            triangle_count: arbiter.static_meshlet_min_triangles,
            meshlet_count: 16,
            projected_screen_area: 0.2,
            geometry_class: FunGeometryClass::DynamicOpaque,
            is_static: false,
            ..Default::default()
        };

        assert_eq!(
            arbiter.decide(below_dynamic_threshold),
            FunRenderPath::StandardRaster
        );
        assert_eq!(
            arbiter.decide(FunRenderPathInput {
                triangle_count: arbiter.dynamic_meshlet_min_triangles,
                material_class: FunMaterialClass::OpaqueComplex,
                ..below_dynamic_threshold
            }),
            FunRenderPath::MeshletDynamicDense
        );
    }

    #[test]
    fn viewmodel_never_uses_meshlet() {
        let path = FunRenderPathArbiter::default().decide(FunRenderPathInput {
            triangle_count: 100_000,
            meshlet_count: 4_096,
            projected_screen_area: 0.9,
            material_class: FunMaterialClass::Viewmodel,
            geometry_class: FunGeometryClass::Viewmodel,
            viewmodel: true,
            ..Default::default()
        });

        assert_eq!(path, FunRenderPath::Viewmodel);
    }

    #[test]
    fn ray_proxy_short_circuits_visible_raster_path() {
        let path = FunRenderPathArbiter::default().decide(FunRenderPathInput {
            triangle_count: 100_000,
            meshlet_count: 4_096,
            projected_screen_area: 0.9,
            ray_proxy_only: true,
            ..Default::default()
        });

        assert_eq!(path, FunRenderPath::RayProxyOnly);
        assert!(!path.emits_visible_raster());
    }

    #[test]
    fn far_small_object_does_not_enter_meshlet_path() {
        let path = FunRenderPathArbiter::default().decide(FunRenderPathInput {
            triangle_count: 1_024,
            meshlet_count: 16,
            projected_screen_area: 0.01,
            distance_band: FunRenderDistanceBand::Far,
            ..Default::default()
        });

        assert_eq!(path, FunRenderPath::StandardRaster);
    }
}
