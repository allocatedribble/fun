use bevy::solari::prelude::{SolariInternalScale, SolariSettings};

use crate::{ClientOpaqueRenderer, ClientRenderConfig, ClientRenderProfile, FunRenderAppOptions};

const FUN_RENDER_PATH_SIGNATURE_ID: &str = "fun-render-path-v1";
const FUN_RENDERER_ID: &str = "fun_render";
const BEVY_RENDER_FEATURES: &str = "local-bevy:bevy_solari+meshlet+meshlet_processor";
const MATERIAL_PIPELINE_ID: &str = "standard-pbr+meshlet+solari-v1";
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
}
