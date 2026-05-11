//! Pass C0 / C1 — typed conversion bridge.
//!
//! Maps the typed `fun_render::sky::FunCloudSettings`
//! (typed legacy env-parsed shape) into the typed
//! `fun_renderer::clouds::CloudRenderSettings` (typed
//! product source of truth).
//!
//! The typed conversion is one-way: typed legacy
//! settings drift into the typed renderer-owned record.
//! The typed renderer-owned record is the typed source
//! of truth for the typed cloud executor + the typed
//! cloud frame graph.
//!
//! Pass C0's typed contract — `fun_render` extracts only;
//! `fun_renderer` owns cloud execution — is mechanically
//! enforced by this typed bridge: the typed bridge maps
//! INTO the typed renderer record, never the other way.

use fun_renderer::{
    CloudDebugOverlay, CloudInternalScale, CloudLuxLightingSettings, CloudQuality,
    CloudRenderSettings, CloudWeatherProfileId, CloudWorldShadowSettings,
};

use super::config::{FunCloudDebugOverlay, FunCloudInternalScale, FunCloudQuality, FunCloudSettings};
use super::weather::FunWeatherProfileId;

/// Typed conversion: legacy quality → renderer-owned
/// quality.  1:1 mapping.
#[must_use]
pub const fn cloud_quality_from_fun(quality: FunCloudQuality) -> CloudQuality {
    match quality {
        FunCloudQuality::Off => CloudQuality::Off,
        FunCloudQuality::Cheap => CloudQuality::Cheap,
        FunCloudQuality::Balanced => CloudQuality::Balanced,
        FunCloudQuality::Cinematic => CloudQuality::Cinematic,
    }
}

/// Typed conversion: legacy internal scale → renderer-
/// owned internal scale.  1:1 mapping.
#[must_use]
pub const fn cloud_internal_scale_from_fun(scale: FunCloudInternalScale) -> CloudInternalScale {
    match scale {
        FunCloudInternalScale::Full => CloudInternalScale::Full,
        FunCloudInternalScale::ThreeQuarter => CloudInternalScale::ThreeQuarter,
        FunCloudInternalScale::Half => CloudInternalScale::Half,
        FunCloudInternalScale::Third => CloudInternalScale::Third,
    }
}

/// Typed conversion: legacy debug overlay → renderer-
/// owned debug overlay.  The typed renderer adds the
/// typed `ShadowMask` and `LuxLighting` variants that
/// the typed legacy enum does not carry; the typed
/// conversion preserves the typed legacy variants and
/// leaves the typed new variants reserved for callers
/// that opt in directly on `CloudRenderSettings`.
#[must_use]
pub const fn cloud_debug_overlay_from_fun(overlay: FunCloudDebugOverlay) -> CloudDebugOverlay {
    match overlay {
        FunCloudDebugOverlay::None => CloudDebugOverlay::None,
        FunCloudDebugOverlay::Coverage => CloudDebugOverlay::Coverage,
        FunCloudDebugOverlay::Density => CloudDebugOverlay::Density,
        FunCloudDebugOverlay::Steps => CloudDebugOverlay::Steps,
        FunCloudDebugOverlay::History => CloudDebugOverlay::History,
        FunCloudDebugOverlay::Weather => CloudDebugOverlay::Weather,
    }
}

/// Typed conversion: legacy weather profile id →
/// renderer-owned weather profile id.  1:1 mapping.
#[must_use]
pub const fn cloud_weather_profile_id_from_fun(id: FunWeatherProfileId) -> CloudWeatherProfileId {
    match id {
        FunWeatherProfileId::Clear => CloudWeatherProfileId::Clear,
        FunWeatherProfileId::Scattered => CloudWeatherProfileId::Scattered,
        FunWeatherProfileId::Overcast => CloudWeatherProfileId::Overcast,
        FunWeatherProfileId::StormFront => CloudWeatherProfileId::StormFront,
        FunWeatherProfileId::CinematicSunset => CloudWeatherProfileId::CinematicSunset,
        FunWeatherProfileId::Custom => CloudWeatherProfileId::Custom,
    }
}

impl From<FunCloudSettings> for CloudRenderSettings {
    fn from(value: FunCloudSettings) -> Self {
        Self {
            schema_version: CloudRenderSettings::PRODUCT_DEFAULT.schema_version,
            enabled: value.enabled,
            quality: cloud_quality_from_fun(value.quality),
            internal_scale: cloud_internal_scale_from_fun(value.internal_scale),
            temporal_enabled: value.temporal_enabled,
            world_shadows: if value.shadows_enabled {
                CloudWorldShadowSettings::PRODUCT_DEFAULT
            } else {
                CloudWorldShadowSettings::DISABLED
            },
            // Typed legacy settings did not expose typed
            // Lux-lighting controls; the typed renderer
            // default carries every typed sub-flag on but
            // gated by `is_active()` (which keys off the
            // typed top-level `enabled` flag).  When the
            // typed legacy settings have `enabled = false`,
            // the typed `consumes_lux_volumetric_lighting`
            // predicate flips to false regardless.
            receive_lux_volumetric_lighting: true,
            lux_lighting: CloudLuxLightingSettings::PRODUCT_DEFAULT,
            profile_id: cloud_weather_profile_id_from_fun(value.profile_id),
            debug_overlay: cloud_debug_overlay_from_fun(value.debug_overlay),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_fun_cloud_settings_round_trips_legacy_fields() {
        let legacy = FunCloudSettings {
            enabled: true,
            quality: FunCloudQuality::Cinematic,
            internal_scale: FunCloudInternalScale::ThreeQuarter,
            temporal_enabled: false,
            shadows_enabled: true,
            profile_id: FunWeatherProfileId::StormFront,
            debug_overlay: FunCloudDebugOverlay::Density,
        };
        let renderer: CloudRenderSettings = legacy.into();
        assert!(renderer.enabled);
        assert_eq!(renderer.quality, CloudQuality::Cinematic);
        assert_eq!(renderer.internal_scale, CloudInternalScale::ThreeQuarter);
        assert!(!renderer.temporal_enabled);
        assert!(renderer.world_shadows.enabled);
        assert_eq!(renderer.profile_id, CloudWeatherProfileId::StormFront);
        assert_eq!(renderer.debug_overlay, CloudDebugOverlay::Density);
    }

    #[test]
    fn from_fun_cloud_settings_disabled_collapses_to_disabled() {
        let legacy = FunCloudSettings::disabled();
        let renderer: CloudRenderSettings = legacy.into();
        assert!(!renderer.enabled);
        assert!(!renderer.is_active());
        assert!(!renderer.registers_world_shadow_pass());
        assert!(!renderer.consumes_lux_volumetric_lighting());
    }

    #[test]
    fn from_fun_cloud_settings_shadows_disabled_collapses_shadow_settings() {
        let legacy = FunCloudSettings {
            enabled: true,
            quality: FunCloudQuality::Balanced,
            internal_scale: FunCloudInternalScale::Half,
            temporal_enabled: true,
            shadows_enabled: false,
            profile_id: FunWeatherProfileId::Clear,
            debug_overlay: FunCloudDebugOverlay::None,
        };
        let renderer: CloudRenderSettings = legacy.into();
        assert!(renderer.enabled);
        assert!(!renderer.world_shadows.enabled);
        assert!(!renderer.registers_world_shadow_pass());
    }
}
