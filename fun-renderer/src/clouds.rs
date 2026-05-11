//! Pass C0 + C1 — typed cloud renderer ownership + the
//! typed `CloudRenderSettings` product surface.
//!
//! The product cloud renderer is owned by `fun-renderer`.
//! The typed `fun_render::sky` module stays as a typed
//! bridge / migration donor (it extracts settings,
//! weather, and signals from the typed Bevy app world);
//! actual cloud execution belongs here.
//!
//! Pass C0 codifies ownership through the typed
//! [`FunCloudRendererContract`].  Pass C1 ports the typed
//! cloud settings, weather profile IDs, debug overlays,
//! world-shadow settings, and Lux-lighting settings into
//! the typed renderer-owned record set.
//!
//! Pass C1 also exposes typed conversion helpers from the
//! typed legacy `fun_render::sky::FunCloudSettings` shape
//! so callers can migrate without losing typed env-parsed
//! state.

use crate::cloud_shadow::CloudWorldShadowSettings;

pub const FUN_RENDERER_CLOUDS_SCHEMA_VERSION: u16 = 1;

// ============================================================================
// Section 1 — Pass C0 typed cloud ownership contract
// ============================================================================

/// Typed Pass C0 cloud-ownership contract.  Codifies the
/// typed product cloud-renderer ownership rules so callers
/// can audit them at compile time + at runtime.
///
/// The typed `CURRENT` constant is the typed source of
/// truth: every flag is `true` in the product surface and
/// `false` only for typed regression captures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunCloudRendererContract {
    pub schema_version: u16,
    pub fun_renderer_owns_cloud_execution: bool,
    pub fun_render_extracts_only: bool,
    pub clouds_use_renderer_frame_graph: bool,
    pub clouds_can_consume_lux_volumetric_lighting: bool,
    pub clouds_can_cast_world_shadows: bool,
}

impl FunCloudRendererContract {
    /// Typed Pass C0 product contract — every flag is
    /// `true`.
    pub const CURRENT: Self = Self {
        schema_version: FUN_RENDERER_CLOUDS_SCHEMA_VERSION,
        fun_renderer_owns_cloud_execution: true,
        fun_render_extracts_only: true,
        clouds_use_renderer_frame_graph: true,
        clouds_can_consume_lux_volumetric_lighting: true,
        clouds_can_cast_world_shadows: true,
    };

    /// Typed regression-capture contract — every flag is
    /// `false`.  Reserved for tests that prove the typed
    /// contract is wired (and that flipping a flag in the
    /// typed product surface flips the typed predicate).
    pub const REGRESSION_CAPTURE: Self = Self {
        schema_version: FUN_RENDERER_CLOUDS_SCHEMA_VERSION,
        fun_renderer_owns_cloud_execution: false,
        fun_render_extracts_only: false,
        clouds_use_renderer_frame_graph: false,
        clouds_can_consume_lux_volumetric_lighting: false,
        clouds_can_cast_world_shadows: false,
    };

    /// Typed predicate: every Pass C0 cloud-ownership
    /// rule holds.
    #[must_use]
    pub const fn obeys_all_ownership_rules(&self) -> bool {
        self.fun_renderer_owns_cloud_execution
            && self.fun_render_extracts_only
            && self.clouds_use_renderer_frame_graph
            && self.clouds_can_consume_lux_volumetric_lighting
            && self.clouds_can_cast_world_shadows
    }
}

// ============================================================================
// Section 2 — Pass C1 typed cloud quality / scale / overlay enums
// ============================================================================

/// Typed Pass C1 cloud quality tier.  Drives the typed
/// primary + light step counts, the typed temporal
/// reconstruction budget, and the typed shader profile.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CloudQuality {
    /// Off — typed cloud path is disabled.  Selecting Off
    /// also disables the typed world-shadow + Lux-lighting
    /// sub-paths regardless of their individual settings.
    Off,
    /// Cheap — typed minimum step counts for competitive
    /// targets.
    Cheap,
    /// Balanced — typed product default.
    #[default]
    Balanced,
    /// Cinematic — typed maximum step counts + typed extra
    /// reconstruction passes.
    Cinematic,
}

impl CloudQuality {
    pub const ALL: [Self; 4] = [Self::Off, Self::Cheap, Self::Balanced, Self::Cinematic];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Cheap => "cheap",
            Self::Balanced => "balanced",
            Self::Cinematic => "cinematic",
        }
    }

    /// Typed primary ray-march step count per pixel.
    #[must_use]
    pub const fn primary_step_count(self) -> u32 {
        match self {
            Self::Off => 0,
            Self::Cheap => 10,
            Self::Balanced => 20,
            Self::Cinematic => 40,
        }
    }

    /// Typed sun-direction light-step count per primary
    /// step.
    #[must_use]
    pub const fn light_step_count(self) -> u32 {
        match self {
            Self::Off => 0,
            Self::Cheap => 2,
            Self::Balanced => 4,
            Self::Cinematic => 8,
        }
    }

    /// Typed predicate: is the typed cloud pipeline active
    /// at all?
    #[must_use]
    pub const fn is_active(self) -> bool {
        !matches!(self, Self::Off)
    }
}

/// Typed Pass C1 cloud internal resolution scale.  Drives
/// the typed offscreen target sizing for the typed
/// raymarch + composite passes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CloudInternalScale {
    Full,
    ThreeQuarter,
    #[default]
    Half,
    Third,
}

impl CloudInternalScale {
    pub const ALL: [Self; 4] =
        [Self::Full, Self::ThreeQuarter, Self::Half, Self::Third];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::ThreeQuarter => "three_quarter",
            Self::Half => "half",
            Self::Third => "third",
        }
    }

    /// Typed `(numerator, denominator)` of the typed scale
    /// ratio.
    #[must_use]
    pub const fn numerator_denominator(self) -> (u32, u32) {
        match self {
            Self::Full => (1, 1),
            Self::ThreeQuarter => (3, 4),
            Self::Half => (1, 2),
            Self::Third => (1, 3),
        }
    }

    /// Typed scale of a 1D extent — saturating-multiply +
    /// ceil-divide so the typed scaled extent never
    /// rounds to zero.
    #[must_use]
    pub const fn scale_extent(self, extent: u32) -> u32 {
        let (numerator, denominator) = self.numerator_denominator();
        let scaled = extent.saturating_mul(numerator);
        let rounded = scaled.saturating_add(denominator - 1) / denominator;
        if rounded == 0 { 1 } else { rounded }
    }
}

/// Typed Pass C1 cloud debug overlay mode.  Extends the
/// typed legacy `FunCloudDebugOverlay` taxonomy with two
/// new variants the user spec calls out:
///
/// - `ShadowMask` — visualizes the typed cloud world-
///   shadow mask (Pass C2+).
/// - `LuxLighting` — visualizes the typed Lux-lighting
///   contribution to the cloud raymarch (Pass C3+).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CloudDebugOverlay {
    #[default]
    None,
    Coverage,
    Density,
    Steps,
    History,
    Weather,
    ShadowMask,
    LuxLighting,
}

impl CloudDebugOverlay {
    pub const ALL: [Self; 8] = [
        Self::None,
        Self::Coverage,
        Self::Density,
        Self::Steps,
        Self::History,
        Self::Weather,
        Self::ShadowMask,
        Self::LuxLighting,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Coverage => "coverage",
            Self::Density => "density",
            Self::Steps => "steps",
            Self::History => "history",
            Self::Weather => "weather",
            Self::ShadowMask => "shadow_mask",
            Self::LuxLighting => "lux_lighting",
        }
    }

    /// Typed predicate: is this typed overlay one of the
    /// typed Pass C1 *new* overlays (vs the typed legacy
    /// set)?
    #[must_use]
    pub const fn is_new_in_pass_c1(self) -> bool {
        matches!(self, Self::ShadowMask | Self::LuxLighting)
    }
}

// ============================================================================
// Section 3 — Pass C1 typed weather profile id (renderer-owned mirror)
// ============================================================================

/// Typed Pass C1 cloud weather profile id.  Renderer-owned
/// mirror of the typed legacy `fun_render::sky::weather::FunWeatherProfileId`
/// — the typed legacy enum stays in `fun_render::sky`
/// for env parsing.  The typed renderer surface uses this
/// typed renderer-owned name so the typed product code
/// reaches the typed canonical home directly.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CloudWeatherProfileId {
    Clear,
    #[default]
    Scattered,
    Overcast,
    StormFront,
    CinematicSunset,
    Custom,
}

impl CloudWeatherProfileId {
    pub const ALL: [Self; 6] = [
        Self::Clear,
        Self::Scattered,
        Self::Overcast,
        Self::StormFront,
        Self::CinematicSunset,
        Self::Custom,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::Scattered => "scattered",
            Self::Overcast => "overcast",
            Self::StormFront => "storm_front",
            Self::CinematicSunset => "cinematic_sunset",
            Self::Custom => "custom",
        }
    }

    #[must_use]
    pub const fn is_builtin(self) -> bool {
        !matches!(self, Self::Custom)
    }

    /// Typed Pass C7.4 cloud slab — typed `(base, top)`
    /// altitudes in meters where the typed profile's cloud
    /// volume sits.  Drives the typed
    /// `CloudShadowProjectionConstants::cloud_base_meters` +
    /// `cloud_top_meters` fields.  Every typed profile
    /// returns a typed non-empty slab (`top > base`) — an
    /// invalid slab is reserved for the typed `DISABLED`
    /// projection constant.
    #[must_use]
    pub const fn cloud_slab_meters(self) -> (u32, u32) {
        match self {
            Self::Clear => (2_500, 4_500),
            Self::Scattered => (1_500, 3_500),
            Self::Overcast => (1_200, 4_000),
            Self::StormFront => (800, 8_000),
            Self::CinematicSunset => (2_000, 5_500),
            Self::Custom => (1_500, 3_500),
        }
    }
}

// ============================================================================
// Section 4 — Pass C1 typed Lux-lighting settings
// ============================================================================

/// Typed Pass C1 Lux-lighting sub-settings.  Drives how
/// the typed cloud raymarch consumes the typed fun-lux
/// volumetric pipeline:
///
/// - `receive_directional_lux` — feed the typed
///   directional sun light through the typed
///   `LuxVolumetricLightInject` directional half.
/// - `receive_local_lux` — feed the typed clustered local
///   lights through the typed `LuxVolumetricLightInject`
///   local half.
/// - `receive_lux_shadows` — sample the typed
///   `LuxVirtualShadowPages` for occlusion.
/// - `inject_into_lux_volumetric` — feed the typed cloud
///   transmittance into the typed Pass 8 volumetric
///   scattering froxel so the typed integrate pass picks
///   up cloud shadowing on volumetric fog.
/// - `scattering_scale_q16` — Q16 scale on the typed
///   Lux-lighting contribution (lets the artist dial down
///   Lux's effect on clouds without disabling it).
/// - `phase_g_q8` — Q8 Henyey-Greenstein g override for
///   the cloud raymarch's Lux-lighting term.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CloudLuxLightingSettings {
    pub schema_version: u16,
    pub receive_directional_lux: bool,
    pub receive_local_lux: bool,
    pub receive_lux_shadows: bool,
    pub inject_into_lux_volumetric: bool,
    pub scattering_scale_q16: u16,
    pub phase_g_q8: i16,
}

impl CloudLuxLightingSettings {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_RENDERER_CLOUDS_SCHEMA_VERSION,
        receive_directional_lux: true,
        receive_local_lux: true,
        receive_lux_shadows: true,
        inject_into_lux_volumetric: true,
        scattering_scale_q16: u16::MAX,
        phase_g_q8: 51, // ~0.2 forward
    };

    pub const COLD_DEFAULT: Self = Self {
        schema_version: FUN_RENDERER_CLOUDS_SCHEMA_VERSION,
        receive_directional_lux: false,
        receive_local_lux: false,
        receive_lux_shadows: false,
        inject_into_lux_volumetric: false,
        scattering_scale_q16: 0,
        phase_g_q8: 0,
    };

    /// Typed predicate: does the typed cloud raymarch
    /// receive any Lux-lighting contribution this frame?
    #[must_use]
    pub const fn receives_any_lux_lighting(&self) -> bool {
        self.scattering_scale_q16 > 0
            && (self.receive_directional_lux || self.receive_local_lux)
    }
}

impl Default for CloudLuxLightingSettings {
    fn default() -> Self {
        Self::PRODUCT_DEFAULT
    }
}

// ============================================================================
// Section 5 — Pass C1 typed top-level CloudRenderSettings
// ============================================================================

/// Typed Pass C1 cloud render settings.  Renderer-owned
/// top-level record consumed by the typed
/// `cloud_executor` + `cloud_passes` modules to schedule
/// + dispatch the typed cloud frame graph.
///
/// Replaces the typed legacy `fun_render::sky::FunCloudSettings`
/// as the typed product source of truth.  A typed
/// `From<FunCloudSettings>` conversion lives in
/// `fun_render::sky` so the typed env-parsing path in
/// `fun_render` continues to feed the typed renderer-owned
/// record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CloudRenderSettings {
    pub schema_version: u16,
    pub enabled: bool,
    pub quality: CloudQuality,
    pub internal_scale: CloudInternalScale,
    pub temporal_enabled: bool,
    pub world_shadows: CloudWorldShadowSettings,
    pub receive_lux_volumetric_lighting: bool,
    pub lux_lighting: CloudLuxLightingSettings,
    pub profile_id: CloudWeatherProfileId,
    pub debug_overlay: CloudDebugOverlay,
}

impl CloudRenderSettings {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_RENDERER_CLOUDS_SCHEMA_VERSION,
        enabled: true,
        quality: CloudQuality::Balanced,
        internal_scale: CloudInternalScale::Half,
        temporal_enabled: true,
        world_shadows: CloudWorldShadowSettings::PRODUCT_DEFAULT,
        receive_lux_volumetric_lighting: true,
        lux_lighting: CloudLuxLightingSettings::PRODUCT_DEFAULT,
        profile_id: CloudWeatherProfileId::Scattered,
        debug_overlay: CloudDebugOverlay::None,
    };

    pub const DISABLED: Self = Self {
        schema_version: FUN_RENDERER_CLOUDS_SCHEMA_VERSION,
        enabled: false,
        quality: CloudQuality::Off,
        internal_scale: CloudInternalScale::Half,
        temporal_enabled: false,
        world_shadows: CloudWorldShadowSettings::DISABLED,
        receive_lux_volumetric_lighting: false,
        lux_lighting: CloudLuxLightingSettings::COLD_DEFAULT,
        profile_id: CloudWeatherProfileId::Clear,
        debug_overlay: CloudDebugOverlay::None,
    };

    /// Typed predicate: is the typed cloud pipeline active
    /// this frame?  Off and `enabled=false` both gate the
    /// typed path.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.enabled && self.quality.is_active()
    }

    /// Typed predicate: should the typed renderer register
    /// the typed cloud world-shadow pass this frame?
    #[must_use]
    pub const fn registers_world_shadow_pass(&self) -> bool {
        self.is_active() && self.world_shadows.enabled
    }

    /// Typed predicate: should the typed renderer wire the
    /// typed Lux-volumetric → cloud lighting bridge this
    /// frame?
    #[must_use]
    pub const fn consumes_lux_volumetric_lighting(&self) -> bool {
        self.is_active()
            && self.receive_lux_volumetric_lighting
            && self.lux_lighting.receives_any_lux_lighting()
    }
}

impl Default for CloudRenderSettings {
    fn default() -> Self {
        Self::PRODUCT_DEFAULT
    }
}

// ============================================================================
// Tests — Pass C0 + C1 typed acceptance
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Pass C0 acceptance — typed cloud ownership is
    /// codified.
    #[test]
    fn pass_c0_cloud_ownership_contract_is_codified() {
        let contract = FunCloudRendererContract::CURRENT;
        assert!(contract.fun_renderer_owns_cloud_execution);
        assert!(contract.fun_render_extracts_only);
        assert!(contract.clouds_use_renderer_frame_graph);
        assert!(contract.clouds_can_consume_lux_volumetric_lighting);
        assert!(contract.clouds_can_cast_world_shadows);
        assert!(contract.obeys_all_ownership_rules());
        // Typed regression capture flips every flag.
        assert!(!FunCloudRendererContract::REGRESSION_CAPTURE.obeys_all_ownership_rules());
    }

    /// Pass C1 acceptance — typed quality tier walks the
    /// user-spec primary + light step counts.
    #[test]
    fn cloud_quality_step_counts_walk_user_spec() {
        assert_eq!(CloudQuality::Off.primary_step_count(), 0);
        assert_eq!(CloudQuality::Cheap.primary_step_count(), 10);
        assert_eq!(CloudQuality::Balanced.primary_step_count(), 20);
        assert_eq!(CloudQuality::Cinematic.primary_step_count(), 40);
        assert_eq!(CloudQuality::Off.light_step_count(), 0);
        assert_eq!(CloudQuality::Cheap.light_step_count(), 2);
        assert_eq!(CloudQuality::Balanced.light_step_count(), 4);
        assert_eq!(CloudQuality::Cinematic.light_step_count(), 8);
        // Off disables the pipeline; every other tier
        // enables it.
        assert!(!CloudQuality::Off.is_active());
        for q in [CloudQuality::Cheap, CloudQuality::Balanced, CloudQuality::Cinematic] {
            assert!(q.is_active());
        }
    }

    /// Pass C1 acceptance — typed internal scale walks
    /// the user-spec ratios with typed ceil rounding.
    #[test]
    fn cloud_internal_scale_extent_walks_user_spec() {
        assert_eq!(CloudInternalScale::Full.scale_extent(1920), 1920);
        // 3/4: 1920 * 3 = 5760; (5760 + 3) / 4 = 1440.
        assert_eq!(CloudInternalScale::ThreeQuarter.scale_extent(1920), 1440);
        assert_eq!(CloudInternalScale::Half.scale_extent(1920), 960);
        // 1/3: (1920 + 2) / 3 = 640.
        assert_eq!(CloudInternalScale::Third.scale_extent(1920), 640);
        // Typed predicate: scale never rounds to zero even
        // for the typed `scale_extent(0)` edge case (we
        // clamp to 1 since the typed offscreen target
        // requires a non-zero extent).
        for scale in CloudInternalScale::ALL {
            assert!(scale.scale_extent(0) >= 1);
        }
    }

    /// Pass C1 acceptance — typed debug overlay adds the
    /// typed new `ShadowMask` and `LuxLighting` variants.
    #[test]
    fn cloud_debug_overlay_adds_shadow_mask_and_lux_lighting() {
        assert!(CloudDebugOverlay::ShadowMask.is_new_in_pass_c1());
        assert!(CloudDebugOverlay::LuxLighting.is_new_in_pass_c1());
        // Typed legacy overlays return `false`.
        for legacy in [
            CloudDebugOverlay::None,
            CloudDebugOverlay::Coverage,
            CloudDebugOverlay::Density,
            CloudDebugOverlay::Steps,
            CloudDebugOverlay::History,
            CloudDebugOverlay::Weather,
        ] {
            assert!(!legacy.is_new_in_pass_c1());
        }
        // Typed taxonomy has exactly 8 typed variants.
        assert_eq!(CloudDebugOverlay::ALL.len(), 8);
    }

    /// Pass C1 acceptance — typed weather profile id has 6
    /// typed variants; 5 typed builtin + 1 typed custom.
    #[test]
    fn cloud_weather_profile_id_taxonomy_is_dense() {
        assert_eq!(CloudWeatherProfileId::ALL.len(), 6);
        let mut builtin_count = 0u32;
        for p in CloudWeatherProfileId::ALL {
            if p.is_builtin() {
                builtin_count += 1;
            }
        }
        assert_eq!(builtin_count, 5);
        assert!(!CloudWeatherProfileId::Custom.is_builtin());
    }

    /// Pass C7.4 acceptance — every typed weather profile
    /// returns a typed non-empty cloud slab.  Drives the
    /// typed `CloudShadowProjectionConstants::has_valid_cloud_slab`
    /// predicate downstream.
    #[test]
    fn cloud_weather_profile_id_cloud_slabs_are_non_empty() {
        for p in CloudWeatherProfileId::ALL {
            let (base, top) = p.cloud_slab_meters();
            assert!(
                top > base,
                "{:?} reports invalid slab base={} top={}",
                p,
                base,
                top,
            );
            assert!(base > 0, "{:?} reports zero base altitude", p);
        }
    }

    /// Pass C1 acceptance — typed Lux-lighting product
    /// default enables every typed sub-flag with a typed
    /// non-zero scattering scale.
    #[test]
    fn cloud_lux_lighting_product_default_receives_lux() {
        let lux = CloudLuxLightingSettings::PRODUCT_DEFAULT;
        assert!(lux.receive_directional_lux);
        assert!(lux.receive_local_lux);
        assert!(lux.receive_lux_shadows);
        assert!(lux.inject_into_lux_volumetric);
        assert!(lux.scattering_scale_q16 > 0);
        assert!(lux.receives_any_lux_lighting());
        // Typed cold default disables everything.
        let cold = CloudLuxLightingSettings::COLD_DEFAULT;
        assert!(!cold.receives_any_lux_lighting());
    }

    /// Pass C1 acceptance — typed top-level
    /// `CloudRenderSettings` product default has every
    /// typed sub-flag active.
    #[test]
    fn cloud_render_settings_product_default_is_active() {
        let s = CloudRenderSettings::PRODUCT_DEFAULT;
        assert!(s.is_active());
        assert!(s.registers_world_shadow_pass());
        assert!(s.consumes_lux_volumetric_lighting());
        // Typed `DISABLED` flips every typed predicate.
        let d = CloudRenderSettings::DISABLED;
        assert!(!d.is_active());
        assert!(!d.registers_world_shadow_pass());
        assert!(!d.consumes_lux_volumetric_lighting());
    }

    /// Pass C1 acceptance — settings.is_active() gates the
    /// world-shadow + Lux-lighting sub-paths even when
    /// the typed sub-settings themselves are enabled.
    /// Selecting `CloudQuality::Off` MUST cascade-disable.
    #[test]
    fn cloud_off_quality_cascade_disables_sub_paths() {
        let mut s = CloudRenderSettings::PRODUCT_DEFAULT;
        s.quality = CloudQuality::Off;
        assert!(!s.is_active());
        assert!(!s.registers_world_shadow_pass());
        assert!(!s.consumes_lux_volumetric_lighting());
    }
}
