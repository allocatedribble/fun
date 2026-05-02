use bevy::prelude::default;
use bevy::solari::prelude::{
    SolariArchitecture, SolariDebugOverlay, SolariDenoiseMode, SolariDirectVisibilityMode,
    SolariInternalScale, SolariRuntimeParams, SolariSettings, SolariVisualTarget,
};
use tracing::{info, warn};

pub fn solari_settings_from_env() -> SolariSettings {
    let visual_target = solari_visual_target_from_env();
    let mut settings = SolariSettings {
        denoise_mode: solari_denoise_mode_from_env(),
        internal_scale: solari_internal_scale_from_env(),
        debug_direct_visibility: std::env::var_os("FUN_SOLARI_DEBUG_DIRECT_VISIBILITY").is_some(),
        ..default()
    };
    apply_direct_lighting_visual_target(visual_target, &mut settings);

    apply_u32_env(
        "FUN_SOLARI_WORLD_CACHE_SIZE",
        &mut settings.world_cache_size,
    );
    apply_u32_env(
        "FUN_SOLARI_WORLD_CACHE_UPDATES",
        &mut settings.world_cache_cell_updates_soft_cap,
    );
    apply_u32_env(
        "FUN_SOLARI_WORLD_CACHE_LIGHT_SAMPLES",
        &mut settings.world_cache_direct_light_sample_count,
    );
    apply_u32_env(
        "FUN_SOLARI_WORLD_CACHE_FRAME_SLICES",
        &mut settings.world_cache_frame_slice_count,
    );
    apply_u32_env(
        "FUN_SOLARI_WORLD_CACHE_NEAR_METERS",
        &mut settings.world_cache_near_camera_distance_meters,
    );
    apply_u32_env(
        "FUN_SOLARI_WORLD_CACHE_MID_METERS",
        &mut settings.world_cache_mid_camera_distance_meters,
    );
    apply_u32_env(
        "FUN_SOLARI_WORLD_CACHE_FAR_METERS",
        &mut settings.world_cache_far_camera_distance_meters,
    );
    apply_u32_env(
        "FUN_SOLARI_LIGHT_TILE_BLOCKS",
        &mut settings.light_tile_blocks,
    );
    apply_u32_env(
        "FUN_SOLARI_LIGHT_TILE_SAMPLES",
        &mut settings.light_tile_samples_per_block,
    );
    apply_u32_env(
        "FUN_SOLARI_BLAS_COMPACTION_VERTICES",
        &mut settings.max_blas_compaction_budget_vertices,
    );
    apply_u32_env(
        "FUN_SOLARI_DIRECT_INITIAL_SAMPLES",
        &mut settings.direct_initial_samples,
    );
    apply_u32_env(
        "FUN_SOLARI_DIRECT_SPATIAL_SAMPLES",
        &mut settings.direct_spatial_samples,
    );
    apply_u32_env(
        "FUN_SOLARI_DIRECT_SPATIAL_BOOST_SAMPLES",
        &mut settings.direct_spatial_samples_boost,
    );
    apply_direct_visibility_env(
        "FUN_SOLARI_DIRECT_INITIAL_VISIBILITY",
        &mut settings.direct_initial_visibility_mode,
    );

    settings
}

pub fn solari_runtime_params_from_env(settings: &SolariSettings) -> SolariRuntimeParams {
    let architecture = solari_architecture_from_env();
    let visual_target = solari_visual_target_from_env();
    let target_fps = env_u32("FUN_SOLARI_TARGET_FPS").unwrap_or(144);
    let frame_budget_ns = env_u32("FUN_SOLARI_FRAME_BUDGET_NS")
        .unwrap_or_else(|| 1_000_000_000u32.saturating_div(target_fps.max(1)));
    let gpu_budget_ns = env_u32("FUN_SOLARI_GPU_BUDGET_NS").unwrap_or(3_000_000);

    let mut params = match architecture {
        SolariArchitecture::Legacy => SolariRuntimeParams::legacy_from_settings(settings),
        SolariArchitecture::Budgeted => SolariRuntimeParams::budgeted(
            settings,
            visual_target,
            target_fps,
            frame_budget_ns,
            gpu_budget_ns,
        ),
    };

    apply_runtime_f32_env(
        "FUN_SOLARI_RECONSTRUCTION_STRENGTH",
        &mut params.reconstruction_strength,
    );
    apply_runtime_u32_env(
        "FUN_SOLARI_CACHE_UPDATE_BUDGET",
        &mut params.cache_update_budget,
    );
    apply_runtime_u32_env(
        "FUN_SOLARI_SPECULAR_REFRESH_BUDGET",
        &mut params.specular_refresh_budget,
    );
    apply_runtime_f32_env(
        "FUN_SOLARI_DI_REUSE_RADIUS",
        &mut params.di_spatial_reuse_radius_pixels,
    );
    apply_runtime_f32_env(
        "FUN_SOLARI_GI_REUSE_RADIUS",
        &mut params.gi_spatial_reuse_radius_pixels,
    );
    apply_runtime_f32_env(
        "FUN_SOLARI_DI_CONFIDENCE_CAP",
        &mut params.di_temporal_confidence_cap,
    );
    apply_runtime_f32_env(
        "FUN_SOLARI_GI_CONFIDENCE_CAP",
        &mut params.gi_temporal_confidence_cap,
    );
    apply_runtime_f32_env(
        "FUN_SOLARI_DIRECT_BOILING_FILTER_STRENGTH",
        &mut params.direct_boiling_filter_strength,
    );
    params.debug_overlay = solari_debug_overlay_from_env();

    params.validated()
}

fn apply_direct_lighting_visual_target(
    visual_target: SolariVisualTarget,
    settings: &mut SolariSettings,
) {
    match visual_target {
        SolariVisualTarget::Competitive => {
            settings.direct_initial_samples = 4;
            settings.direct_spatial_samples = 1;
            settings.direct_spatial_samples_boost = 1;
            settings.direct_initial_visibility_mode = SolariDirectVisibilityMode::Selected;
        }
        SolariVisualTarget::Balanced => {}
        SolariVisualTarget::Cinematic => {
            settings.direct_initial_samples = 16;
            settings.direct_spatial_samples = 3;
            settings.direct_spatial_samples_boost = 2;
            settings.direct_initial_visibility_mode = SolariDirectVisibilityMode::Selected;
        }
    }
}

fn solari_architecture_from_env() -> SolariArchitecture {
    match std::env::var("FUN_SOLARI_ARCH")
        .ok()
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("budgeted") | Some("budget") | Some("v3") => SolariArchitecture::Budgeted,
        Some("legacy") | None => SolariArchitecture::Legacy,
        Some(unknown) => {
            warn!(
                target: "fun::render",
                value = unknown,
                "unknown FUN_SOLARI_ARCH; using legacy Solari architecture"
            );
            SolariArchitecture::Legacy
        }
    }
}

fn solari_visual_target_from_env() -> SolariVisualTarget {
    match std::env::var("FUN_SOLARI_VISUAL_TARGET")
        .ok()
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("competitive") | Some("comp") | Some("fps") => SolariVisualTarget::Competitive,
        Some("cinematic") | Some("quality") => SolariVisualTarget::Cinematic,
        Some("balanced") | None => SolariVisualTarget::Balanced,
        Some(unknown) => {
            warn!(
                target: "fun::render",
                value = unknown,
                "unknown FUN_SOLARI_VISUAL_TARGET; using balanced Solari visual target"
            );
            SolariVisualTarget::Balanced
        }
    }
}

fn solari_debug_overlay_from_env() -> SolariDebugOverlay {
    match std::env::var("FUN_SOLARI_DEBUG_OVERLAY")
        .ok()
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("surface")
        | Some("surface-classification")
        | Some("surface_classification")
        | Some("classification") => SolariDebugOverlay::SurfaceClassification,
        Some("queue") | Some("queues") | Some("work-queue") | Some("work_queue")
        | Some("work-queues") | Some("work_queues") => SolariDebugOverlay::WorkQueues,
        Some("none") | Some("off") | None => SolariDebugOverlay::None,
        Some(unknown) => {
            warn!(
                target: "fun::render",
                value = unknown,
                "unknown FUN_SOLARI_DEBUG_OVERLAY; debug overlay disabled"
            );
            SolariDebugOverlay::None
        }
    }
}

fn apply_runtime_u32_env(name: &'static str, value: &mut u32) {
    if let Some(parsed) = env_u32(name) {
        *value = parsed;
        info!(target: "fun::render", setting = name, value = parsed, "applied Solari runtime integer setting");
    }
}

fn apply_runtime_f32_env(name: &'static str, value: &mut f32) {
    let Some(raw) = std::env::var_os(name) else {
        return;
    };
    let raw = raw.to_string_lossy();
    match raw.parse::<f32>() {
        Ok(parsed) => {
            *value = parsed;
            info!(target: "fun::render", setting = name, value = parsed, "applied Solari runtime float setting");
        }
        Err(error) => {
            warn!(
                target: "fun::render",
                setting = name,
                value = %raw,
                %error,
                "ignored invalid Solari runtime float setting"
            );
        }
    }
}

fn apply_direct_visibility_env(name: &'static str, value: &mut SolariDirectVisibilityMode) {
    let Some(raw) = std::env::var_os(name) else {
        return;
    };
    let raw = raw.to_string_lossy();
    match parse_solari_direct_visibility_mode(Some(&raw)) {
        Some(parsed) => {
            *value = parsed;
            info!(
                target: "fun::render",
                setting = name,
                value = parsed.as_str(),
                "applied Solari direct visibility setting"
            );
        }
        None => {
            warn!(
                target: "fun::render",
                setting = name,
                value = %raw,
                "ignored invalid Solari direct visibility setting"
            );
        }
    }
}

fn parse_solari_direct_visibility_mode(raw: Option<&str>) -> Option<SolariDirectVisibilityMode> {
    match raw.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("none") | Some("off") | Some("0") => Some(SolariDirectVisibilityMode::None),
        Some("selected") | Some("selected-light") | Some("selected_light") | Some("1") => {
            Some(SolariDirectVisibilityMode::Selected)
        }
        None => Some(SolariDirectVisibilityMode::Selected),
        Some(_) => None,
    }
}

fn env_u32(name: &'static str) -> Option<u32> {
    let raw = std::env::var_os(name)?;
    let raw = raw.to_string_lossy();
    match raw.parse::<u32>() {
        Ok(parsed) => Some(parsed),
        Err(error) => {
            warn!(
                target: "fun::render",
                setting = name,
                value = %raw,
                %error,
                "ignored invalid Solari integer setting"
            );
            None
        }
    }
}

fn solari_internal_scale_from_env() -> SolariInternalScale {
    let Some(raw) = std::env::var("FUN_SOLARI_INTERNAL_SCALE")
        .ok()
        .map(|value| value.to_ascii_lowercase())
    else {
        return SolariInternalScale::Full;
    };

    match raw.as_str() {
        "1" | "1.0" | "full" | "native" => SolariInternalScale::Full,
        "0.75" | ".75" | "75" | "3/4" | "three-quarter" | "three_quarter" => {
            SolariInternalScale::ThreeQuarter
        }
        "0.66" | "0.666" | "0.67" | ".66" | ".666" | ".67" | "66" | "2/3" | "two-thirds"
        | "two_thirds" => SolariInternalScale::TwoThirds,
        "0.5" | ".5" | "50" | "1/2" | "half" => SolariInternalScale::Half,
        unknown => {
            warn!(
                "Unknown FUN_SOLARI_INTERNAL_SCALE={unknown}; using full-resolution Solari GI reservoirs"
            );
            SolariInternalScale::Full
        }
    }
}

fn apply_u32_env(name: &'static str, value: &mut u32) {
    let Some(raw) = std::env::var_os(name) else {
        return;
    };
    let raw = raw.to_string_lossy();
    match raw.parse::<u32>() {
        Ok(parsed) => {
            *value = parsed;
            info!(target: "fun::render", setting = name, value = parsed, "applied Solari numeric setting");
        }
        Err(error) => {
            warn!(
                target: "fun::render",
                setting = name,
                value = %raw,
                %error,
                "ignored invalid Solari numeric setting"
            );
        }
    }
}

fn solari_denoise_mode_from_env() -> SolariDenoiseMode {
    let mode = std::env::var("FUN_SOLARI_DENOISE_MODE")
        .ok()
        .map(|mode| mode.to_ascii_lowercase());

    parse_solari_denoise_mode(
        mode.as_deref(),
        std::env::var_os("FUN_DISABLE_DLSS_RR").is_some(),
    )
}

pub fn parse_solari_denoise_mode(
    mode: Option<&str>,
    _dlss_ray_reconstruction_disabled: bool,
) -> SolariDenoiseMode {
    let Some(mode) = mode else {
        return SolariDenoiseMode::BalancedFast;
    };

    match mode {
        "off" | "raw" => SolariDenoiseMode::Off,
        "cheap" | "cheap-temporal" | "cheap_temporal" => SolariDenoiseMode::CheapTemporal,
        "fast" | "balanced-fast" | "balanced_fast" | "balancedfast" => {
            SolariDenoiseMode::BalancedFast
        }
        "balanced" | "svgf" | "svgf-lite" | "svgf_lite" => SolariDenoiseMode::Balanced,
        "quality" | "svgf-quality" | "svgf_quality" => SolariDenoiseMode::Quality,
        "rr" | "dlss" | "dlss-rr" | "dlss_rr" | "ray-reconstruction" => {
            SolariDenoiseMode::DlssRayReconstruction
        }
        unknown => {
            warn!(
                "Unknown FUN_SOLARI_DENOISE_MODE={unknown}; falling back to balanced-fast Solari denoising"
            );
            SolariDenoiseMode::BalancedFast
        }
    }
}

pub fn benchmark_parse_solari_denoise_mode(
    mode: Option<&str>,
    dlss_ray_reconstruction_disabled: bool,
) -> SolariDenoiseMode {
    parse_solari_denoise_mode(mode, dlss_ray_reconstruction_disabled)
}

#[cfg(test)]
mod tests {
    use super::{
        SolariDirectVisibilityMode, SolariSettings, SolariVisualTarget,
        apply_direct_lighting_visual_target, parse_solari_direct_visibility_mode,
    };

    #[test]
    fn visual_targets_map_to_direct_lighting_quality_knobs() {
        let mut competitive = SolariSettings::default();
        apply_direct_lighting_visual_target(SolariVisualTarget::Competitive, &mut competitive);
        assert_eq!(competitive.direct_initial_samples, 4);
        assert_eq!(competitive.direct_spatial_samples, 1);
        assert_eq!(competitive.direct_spatial_samples_boost, 1);

        let mut balanced = SolariSettings::default();
        apply_direct_lighting_visual_target(SolariVisualTarget::Balanced, &mut balanced);
        assert_eq!(balanced.direct_initial_samples, 8);
        assert_eq!(balanced.direct_spatial_samples, 1);
        assert_eq!(balanced.direct_spatial_samples_boost, 0);

        let mut cinematic = SolariSettings::default();
        apply_direct_lighting_visual_target(SolariVisualTarget::Cinematic, &mut cinematic);
        assert_eq!(cinematic.direct_initial_samples, 16);
        assert_eq!(cinematic.direct_spatial_samples, 3);
        assert_eq!(cinematic.direct_spatial_samples_boost, 2);
    }

    #[test]
    fn direct_visibility_mode_parser_accepts_safe_values() {
        assert_eq!(
            parse_solari_direct_visibility_mode(Some("selected_light")),
            Some(SolariDirectVisibilityMode::Selected)
        );
        assert_eq!(
            parse_solari_direct_visibility_mode(Some("none")),
            Some(SolariDirectVisibilityMode::None)
        );
        assert_eq!(parse_solari_direct_visibility_mode(Some("all")), None);
    }
}
