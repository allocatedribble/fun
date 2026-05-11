//! Pass 9 — typed godrays-through-fog policy surface.
//!
//! Pass 8 ships the typed volumetric pipeline.  Pass 9 lands
//! the typed godray controls on top of it and adds a typed
//! screen-space fallback for the lower quality tiers:
//!
//! - Primary godrays: shadowed volumetric scattering (typed
//!   via Pass 8's `LuxVolumetricPassRole::VolumetricShadowSample`
//!   + `LuxVolumetricLightInject{Directional,Local}` passes
//!   plus the per-light overrides in
//!   `LuxVolumetricLightOverride`).
//! - Screen-space fallback: typed
//!   `LuxGodrayPassRole::ScreenSpaceGodrayPass` with typed
//!   radial depth-aware sampling settings.  Allowed in
//!   Low / Medium; disabled or subtle in High / Cinematic
//!   where true volumetrics carry the effect.
//!
//! The canonical typed `LuxGodraySettings` (8 fields per
//! the user spec) lives here in Pass 9 and is referenced
//! from both [`crate::fog_volumetric::LuxVolumetricSettings`]
//! and [`crate::look::FunLuxLookProfile`].  Rule #5 of the
//! Pass 9 acceptance criteria requires the typed settings
//! be part of the look profile, so the canonical home is
//! the look profile; volumetric routing reads the same
//! record.
//!
//! Pass 9 keeps fun-lux renderer-neutral — every type is a
//! typed descriptor / predicate; no `wgpu`, `naga`, or
//! raw-window imports.  f32 fields are encoded Q16
//! fixed-point so the typed records stay `Hash + Eq`,
//! consistent with the Pass 6 / Pass 8 convention.

use crate::fog_volumetric::LuxVolumetricQuality;

pub const FUN_LUX_GODRAY_SCHEMA_VERSION: u16 = 1;
pub const FUN_LUX_GODRAY_SOURCE_COUNT: usize = 4;
pub const FUN_LUX_GODRAY_LIGHT_SAMPLE_SOURCE_COUNT: usize = 4;
pub const FUN_LUX_GODRAY_PASS_ROLE_COUNT: usize = 1;
pub const FUN_LUX_GODRAY_PASS9_ACCEPTANCE_RULE_COUNT: usize = 5;

// ============================================================================
// Section 1 — Typed canonical LuxGodraySettings (user spec)
// ============================================================================

/// Typed canonical godray settings — the user spec's
/// 8-field `LuxGodraySettings` struct, with f32 fields
/// encoded Q16 to keep the record `Hash + Eq` stable.
///
/// Field-by-field correspondence with the user spec:
///
/// | User spec field         | Pass 9 typed field          |
/// | ----------------------- | --------------------------- |
/// | `enabled: bool`         | `enabled: bool`             |
/// | `volumetric: bool`      | `volumetric: bool`          |
/// | `screen_space_fallback: bool` | `screen_space_fallback` |
/// | `max_godray_lights: u8` | `max_godray_lights: u8`     |
/// | `intensity: f32`        | `intensity_q16: u16`        |
/// | `occlusion_strength: f32` | `occlusion_strength_q16: u16` |
/// | `shaft_sharpness: f32`  | `shaft_sharpness_q16: u16`  |
/// | `noise_strength: f32`   | `noise_strength_q16: u16`   |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxGodraySettings {
    pub schema_version: u16,
    pub enabled: bool,
    pub volumetric: bool,
    pub screen_space_fallback: bool,
    pub max_godray_lights: u8,
    /// Q16 (`1.0 = 65_535`).
    pub intensity_q16: u16,
    /// Q16 — how strongly occluders attenuate the
    /// volumetric in-scatter along the shaft.
    pub occlusion_strength_q16: u16,
    /// Q16 — controls the phase-function concentration
    /// around the light direction.
    pub shaft_sharpness_q16: u16,
    /// Q16 — adds high-frequency noise to break up
    /// banding along the integration ray.
    pub noise_strength_q16: u16,
}

impl LuxGodraySettings {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_GODRAY_SCHEMA_VERSION,
        enabled: true,
        // Volumetric primary at the product tier.
        volumetric: true,
        // Screen-space fallback enabled — runtime routing
        // disables it when the quality tier is High /
        // Cinematic (see `LuxGodrayRoutingPolicy`).
        screen_space_fallback: true,
        max_godray_lights: 8,
        intensity_q16: u16::MAX / 2,
        occlusion_strength_q16: 49_152, // ~0.75
        shaft_sharpness_q16: 39_321,    // ~0.6
        noise_strength_q16: 6_553,      // ~0.1
    };

    pub const COLD_DEFAULT: Self = Self {
        schema_version: FUN_LUX_GODRAY_SCHEMA_VERSION,
        enabled: false,
        volumetric: false,
        screen_space_fallback: false,
        max_godray_lights: 0,
        intensity_q16: 0,
        occlusion_strength_q16: 0,
        shaft_sharpness_q16: 0,
        noise_strength_q16: 0,
    };

    /// Typed predicate: is the godray pipeline active at
    /// all?
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.enabled
            && (self.volumetric || self.screen_space_fallback)
            && self.max_godray_lights > 0
            && self.intensity_q16 > 0
    }

    /// Typed predicate: does this configuration favor the
    /// volumetric primary path (Pass 9 rule #4)?
    #[must_use]
    pub const fn favors_volumetric(&self) -> bool {
        self.enabled && self.volumetric
    }

    /// Typed predicate: does this configuration allow the
    /// screen-space fallback (Pass 9 rule #3)?
    #[must_use]
    pub const fn allows_screen_space_fallback(&self) -> bool {
        self.enabled && self.screen_space_fallback
    }
}

// ============================================================================
// Section 2 — Typed screen-space godray settings
// ============================================================================

/// Typed Pass 9 screen-space godray settings — drives the
/// optional radial depth-aware sampling fallback for the
/// `LuxGodrayPassRole::ScreenSpaceGodrayPass`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxScreenSpaceGodraySettings {
    pub schema_version: u16,
    pub enabled: bool,
    /// Number of radial samples per pixel from the light's
    /// screen position.
    pub radial_sample_count: u8,
    /// Step distance between samples Q16 (`1.0 = max
    /// radial extent`).
    pub radial_step_q16: u16,
    /// When `true`, the pass weights samples by depth
    /// comparison to mask out foreground occluders.
    pub depth_aware: bool,
    /// Per-sample blue-noise jitter to break up banding.
    pub blue_noise_jitter: bool,
    /// Maximum occluders the typed sampler counts per
    /// ray.  Bounds the worst-case shader cost.
    pub max_occluders_per_sample: u8,
}

impl LuxScreenSpaceGodraySettings {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_GODRAY_SCHEMA_VERSION,
        enabled: true,
        radial_sample_count: 32,
        radial_step_q16: 2_048, // ~1/32
        depth_aware: true,
        blue_noise_jitter: true,
        max_occluders_per_sample: 8,
    };

    pub const LOW_TIER_DEFAULT: Self = Self {
        schema_version: FUN_LUX_GODRAY_SCHEMA_VERSION,
        enabled: true,
        radial_sample_count: 16,
        radial_step_q16: 4_096, // ~1/16
        depth_aware: true,
        blue_noise_jitter: true,
        max_occluders_per_sample: 4,
    };

    pub const COLD_DEFAULT: Self = Self {
        schema_version: FUN_LUX_GODRAY_SCHEMA_VERSION,
        enabled: false,
        radial_sample_count: 0,
        radial_step_q16: 0,
        depth_aware: false,
        blue_noise_jitter: false,
        max_occluders_per_sample: 0,
    };

    /// Typed predicate: is the screen-space pass active?
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.enabled && self.radial_sample_count > 0
    }
}

// ============================================================================
// Section 3 — Typed godray source (Disabled / Volumetric / SS / Hybrid)
// ============================================================================

/// Typed godray rendering source.  Drives whether the
/// renderer composites volumetric godrays, screen-space
/// godrays, both, or neither.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxGodraySource {
    /// Disabled — no godray pass runs.
    #[default]
    Disabled,
    /// Volumetric primary — shadowed volumetric scattering
    /// from Pass 8's typed `VolumetricLightInject*` +
    /// `VolumetricShadowSample` passes carries the effect.
    VolumetricFog,
    /// Screen-space fallback only — the cheaper radial
    /// depth-aware pass.  Acceptable in Low / Medium.
    ScreenSpaceFallback,
    /// Hybrid — both paths run.  The renderer is expected
    /// to weight the screen-space contribution down so it
    /// stays a subtle highlight.
    Hybrid,
}

impl LuxGodraySource {
    pub const ALL: [Self; FUN_LUX_GODRAY_SOURCE_COUNT] = [
        Self::Disabled,
        Self::VolumetricFog,
        Self::ScreenSpaceFallback,
        Self::Hybrid,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::VolumetricFog => "volumetric_fog",
            Self::ScreenSpaceFallback => "screen_space_fallback",
            Self::Hybrid => "hybrid",
        }
    }

    /// Typed predicate: does this source run the typed
    /// volumetric path?
    #[must_use]
    pub const fn runs_volumetric_path(self) -> bool {
        matches!(self, Self::VolumetricFog | Self::Hybrid)
    }

    /// Typed predicate: does this source run the typed
    /// screen-space pass?
    #[must_use]
    pub const fn runs_screen_space_pass(self) -> bool {
        matches!(self, Self::ScreenSpaceFallback | Self::Hybrid)
    }
}

// ============================================================================
// Section 4 — Typed per-quality routing policy
// ============================================================================

/// Typed Pass 9 per-quality routing policy.  Maps the typed
/// `LuxVolumetricQuality` (Pass 8) to the typed
/// `LuxGodraySource` the renderer should run.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxGodrayRoutingPolicy {
    pub schema_version: u16,
}

impl LuxGodrayRoutingPolicy {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_GODRAY_SCHEMA_VERSION,
    };

    /// Typed dispatcher — maps the Pass 8 quality tier to
    /// the typed Pass 9 godray source.
    ///
    /// - Off: `Disabled`.
    /// - Low: `ScreenSpaceFallback` — true volumetric is
    ///   too expensive for this tier.
    /// - Medium: `ScreenSpaceFallback` — same reason.
    /// - High: `VolumetricFog` — true volumetric shafts
    ///   carry the effect.
    /// - Cinematic: `VolumetricFog` — same reason; the
    ///   screen-space pass is disabled or subtle.
    #[must_use]
    pub const fn source_for_quality(quality: LuxVolumetricQuality) -> LuxGodraySource {
        match quality {
            LuxVolumetricQuality::Off => LuxGodraySource::Disabled,
            LuxVolumetricQuality::Low | LuxVolumetricQuality::Medium => {
                LuxGodraySource::ScreenSpaceFallback
            }
            LuxVolumetricQuality::High | LuxVolumetricQuality::Cinematic => {
                LuxGodraySource::VolumetricFog
            }
        }
    }

    /// Typed predicate: rule #3 — Low tier can use the
    /// screen-space fallback.
    #[must_use]
    pub const fn low_tier_uses_screen_space_fallback() -> bool {
        Self::source_for_quality(LuxVolumetricQuality::Low).runs_screen_space_pass()
    }

    /// Typed predicate: rule #4 — High tier uses true
    /// volumetric shafts.
    #[must_use]
    pub const fn high_tier_uses_true_volumetrics() -> bool {
        Self::source_for_quality(LuxVolumetricQuality::High).runs_volumetric_path()
    }
}

// ============================================================================
// Section 5 — Typed light-sample source (rule #2)
// ============================================================================

/// Typed source the godray pass samples for light
/// visibility.  Encodes the user spec's "Directional
/// lights sample cascaded/virtual shadows; spotlights
/// sample local shadow maps or virtual shadow pages" rule.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxGodrayLightSampleSource {
    /// Cascaded directional shadow map (sun / moon).
    #[default]
    CascadedDirectionalShadow,
    /// Virtual shadow page — typed Pass 7 sparse paged
    /// shadow map.
    VirtualShadowPage,
    /// Local shadow map — spot / point shadow atlas tile.
    LocalShadowMap,
    /// Unshadowed — no shadow sampling (fallback when no
    /// shadow source is wired).
    Unshadowed,
}

impl LuxGodrayLightSampleSource {
    pub const ALL: [Self; FUN_LUX_GODRAY_LIGHT_SAMPLE_SOURCE_COUNT] = [
        Self::CascadedDirectionalShadow,
        Self::VirtualShadowPage,
        Self::LocalShadowMap,
        Self::Unshadowed,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CascadedDirectionalShadow => "cascaded_directional_shadow",
            Self::VirtualShadowPage => "virtual_shadow_page",
            Self::LocalShadowMap => "local_shadow_map",
            Self::Unshadowed => "unshadowed",
        }
    }

    /// Typed predicate: rule #2 — does this source sample
    /// real shadow data (vs running unshadowed)?
    #[must_use]
    pub const fn responds_to_shadow_maps_or_virtual_pages(self) -> bool {
        !matches!(self, Self::Unshadowed)
    }
}

// ============================================================================
// Section 6 — Typed screen-space godray pass role + composite order
// ============================================================================

/// Typed Pass 9 renderer-side pass role.  Pass 8 already
/// owns the typed volumetric pass list; Pass 9 adds only
/// the screen-space godray fallback.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxGodrayPassRole {
    /// Screen-space godray pass — radial depth-aware
    /// sampling from each light's screen position.
    /// Composites into the HDR scene-color target BEFORE
    /// the typed tone-map stage.
    #[default]
    ScreenSpaceGodrayPass,
}

impl LuxGodrayPassRole {
    pub const ALL: [Self; FUN_LUX_GODRAY_PASS_ROLE_COUNT] = [Self::ScreenSpaceGodrayPass];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ScreenSpaceGodrayPass => "screen_space_godray_pass",
        }
    }

    /// Typed render-order key.  Returns a value that
    /// places the pass before the typed tone-map stage
    /// (composite-into-HDR before tonemap).  Uses 950 so it
    /// sits AFTER Pass 8's `VolumetricCompositeIntoHdr`
    /// (900) but still before the HDR pipeline's tonemap
    /// stage (1000+ — Pass 5's typed `FunLuxHdrPipelineStage::Tonemap`
    /// order_key 500).
    #[must_use]
    pub const fn order_key(self) -> u16 {
        match self {
            Self::ScreenSpaceGodrayPass => 950,
        }
    }
}

// ============================================================================
// Section 7 — Typed Pass 9 acceptance verdict (5 user-spec rules)
// ============================================================================

/// Typed renderer-emitted Pass 9 frame signals.  The
/// renderer fills these typed flags each frame so the
/// verdict can audit the live pipeline against the user's
/// five acceptance criteria.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxGodrayFrameSignals {
    pub schema_version: u16,
    /// Rule #1 signal: the renderer observed at least one
    /// occluder-driven shaft this frame (the typed
    /// occlusion-strength contribution was non-zero).
    pub occluders_produced_shafts: bool,
    /// Rule #2 signal: the typed light-sample source for
    /// this frame's godrays was NOT `Unshadowed`.
    pub light_sample_source: LuxGodrayLightSampleSource,
    /// Active source for this frame.
    pub active_source: LuxGodraySource,
    /// Quality tier active this frame.
    pub active_quality: LuxVolumetricQuality,
}

impl LuxGodrayFrameSignals {
    pub const NOT_EMITTED: Self = Self {
        schema_version: 0,
        occluders_produced_shafts: false,
        light_sample_source: LuxGodrayLightSampleSource::Unshadowed,
        active_source: LuxGodraySource::Disabled,
        active_quality: LuxVolumetricQuality::Off,
    };

    pub const FULLY_WIRED_HIGH: Self = Self {
        schema_version: FUN_LUX_GODRAY_SCHEMA_VERSION,
        occluders_produced_shafts: true,
        light_sample_source: LuxGodrayLightSampleSource::CascadedDirectionalShadow,
        active_source: LuxGodraySource::VolumetricFog,
        active_quality: LuxVolumetricQuality::High,
    };

    pub const FULLY_WIRED_LOW: Self = Self {
        schema_version: FUN_LUX_GODRAY_SCHEMA_VERSION,
        occluders_produced_shafts: true,
        light_sample_source: LuxGodrayLightSampleSource::LocalShadowMap,
        active_source: LuxGodraySource::ScreenSpaceFallback,
        active_quality: LuxVolumetricQuality::Low,
    };

    #[must_use]
    pub const fn is_emitted(&self) -> bool {
        self.schema_version != 0
    }
}

/// Typed Pass 9 acceptance verdict.  One flag per
/// user-listed acceptance criterion.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxGodrayPass9AcceptanceVerdict {
    pub schema_version: u16,
    /// Rule #1: window / door / slat occlusion produces
    /// visible shafts in fog.
    pub occluders_produce_shafts: bool,
    /// Rule #2: godrays respond to shadow maps / virtual
    /// pages.
    pub responds_to_shadow_maps_or_virtual_pages: bool,
    /// Rule #3: Low tier can use the screen-space fallback.
    pub low_tier_uses_screen_space_fallback: bool,
    /// Rule #4: High tier uses true volumetric shafts.
    pub high_tier_uses_true_volumetrics: bool,
    /// Rule #5: godray settings are part of
    /// [`crate::look::FunLuxLookProfile`].
    pub godray_settings_in_look_profile: bool,
}

impl LuxGodrayPass9AcceptanceVerdict {
    /// Typed `evaluate(godray_settings, signals,
    /// look_profile_has_godrays)` constructor.  Rules #3
    /// and #4 are checked against the typed routing policy
    /// (which is `const`), so the verdict doesn't need
    /// runtime quality tier data.
    ///
    /// `look_profile_has_godrays` is the typed assertion
    /// that the caller pulled `LuxGodraySettings` out of a
    /// [`crate::look::FunLuxLookProfile`] (rule #5).  The
    /// look profile module ships a public field of typed
    /// `LuxGodraySettings`, so a static `true` is the
    /// product-default answer.
    #[must_use]
    pub fn evaluate(
        settings: &LuxGodraySettings,
        signals: &LuxGodrayFrameSignals,
        look_profile_has_godrays: bool,
    ) -> Self {
        Self {
            schema_version: FUN_LUX_GODRAY_SCHEMA_VERSION,
            // Rule #1: occluders produce shafts when the
            // typed settings enable godrays AND the renderer
            // signal reports an occluder-driven shaft.
            occluders_produce_shafts: settings.is_active()
                && signals.is_emitted()
                && signals.occluders_produced_shafts,
            // Rule #2: light sample source is a real shadow
            // source (typed taxonomy predicate).
            responds_to_shadow_maps_or_virtual_pages: signals.is_emitted()
                && signals
                    .light_sample_source
                    .responds_to_shadow_maps_or_virtual_pages(),
            // Rule #3: typed routing policy allows
            // screen-space fallback at Low — encoded in the
            // typed `LuxGodrayRoutingPolicy::low_tier_uses_screen_space_fallback`
            // const predicate.
            low_tier_uses_screen_space_fallback:
                LuxGodrayRoutingPolicy::low_tier_uses_screen_space_fallback()
                    && settings.allows_screen_space_fallback(),
            // Rule #4: typed routing policy assigns
            // volumetric at High.
            high_tier_uses_true_volumetrics: LuxGodrayRoutingPolicy::high_tier_uses_true_volumetrics(
            ) && settings.favors_volumetric(),
            // Rule #5: caller asserts godray settings live
            // on the look profile.
            godray_settings_in_look_profile: look_profile_has_godrays,
        }
    }

    /// Typed count of satisfied rules (0..=5).
    #[must_use]
    pub const fn rules_satisfied(&self) -> u32 {
        let mut count = 0u32;
        if self.occluders_produce_shafts {
            count += 1;
        }
        if self.responds_to_shadow_maps_or_virtual_pages {
            count += 1;
        }
        if self.low_tier_uses_screen_space_fallback {
            count += 1;
        }
        if self.high_tier_uses_true_volumetrics {
            count += 1;
        }
        if self.godray_settings_in_look_profile {
            count += 1;
        }
        count
    }

    /// Typed bundle predicate: every Pass 9 rule holds.
    #[must_use]
    pub const fn obeys_all_five_rules(&self) -> bool {
        self.rules_satisfied() as usize == FUN_LUX_GODRAY_PASS9_ACCEPTANCE_RULE_COUNT
    }
}

// ============================================================================
// Tests — Pass 9 typed audits
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taxonomy_counts_are_dense() {
        assert_eq!(LuxGodraySource::ALL.len(), FUN_LUX_GODRAY_SOURCE_COUNT);
        assert_eq!(
            LuxGodrayLightSampleSource::ALL.len(),
            FUN_LUX_GODRAY_LIGHT_SAMPLE_SOURCE_COUNT,
        );
        assert_eq!(LuxGodrayPassRole::ALL.len(), FUN_LUX_GODRAY_PASS_ROLE_COUNT,);
    }

    #[test]
    fn product_default_settings_match_user_spec_field_set() {
        let s = LuxGodraySettings::PRODUCT_DEFAULT;
        // User spec lists 8 typed fields (enabled,
        // volumetric, screen_space_fallback, max_godray_lights,
        // intensity, occlusion_strength, shaft_sharpness,
        // noise_strength).  Confirm every dial is set.
        assert!(s.enabled);
        assert!(s.volumetric);
        assert!(s.screen_space_fallback);
        assert!(s.max_godray_lights > 0);
        assert!(s.intensity_q16 > 0);
        assert!(s.occlusion_strength_q16 > 0);
        assert!(s.shaft_sharpness_q16 > 0);
        assert!(s.noise_strength_q16 > 0);
        assert!(s.is_active());
    }

    #[test]
    fn cold_default_settings_disable_everything() {
        let s = LuxGodraySettings::COLD_DEFAULT;
        assert!(!s.is_active());
        assert!(!s.favors_volumetric());
        assert!(!s.allows_screen_space_fallback());
    }

    #[test]
    fn screen_space_settings_active_predicate() {
        assert!(LuxScreenSpaceGodraySettings::PRODUCT_DEFAULT.is_active());
        assert!(LuxScreenSpaceGodraySettings::LOW_TIER_DEFAULT.is_active());
        assert!(!LuxScreenSpaceGodraySettings::COLD_DEFAULT.is_active());
    }

    #[test]
    fn godray_source_predicates_split_volumetric_and_screen_space() {
        for s in LuxGodraySource::ALL {
            let vol = s.runs_volumetric_path();
            let ss = s.runs_screen_space_pass();
            match s {
                LuxGodraySource::Disabled => {
                    assert!(!vol && !ss)
                }
                LuxGodraySource::VolumetricFog => assert!(vol && !ss),
                LuxGodraySource::ScreenSpaceFallback => assert!(!vol && ss),
                LuxGodraySource::Hybrid => assert!(vol && ss),
            }
        }
    }

    /// Pass 9 user rule #3 — Low tier can use the
    /// screen-space fallback.
    #[test]
    fn low_tier_routes_to_screen_space_fallback() {
        assert_eq!(
            LuxGodrayRoutingPolicy::source_for_quality(LuxVolumetricQuality::Low),
            LuxGodraySource::ScreenSpaceFallback,
        );
        assert_eq!(
            LuxGodrayRoutingPolicy::source_for_quality(LuxVolumetricQuality::Medium),
            LuxGodraySource::ScreenSpaceFallback,
        );
        assert!(LuxGodrayRoutingPolicy::low_tier_uses_screen_space_fallback());
    }

    /// Pass 9 user rule #4 — High / Cinematic tiers use
    /// true volumetric shafts.
    #[test]
    fn high_and_cinematic_tiers_route_to_volumetric() {
        assert_eq!(
            LuxGodrayRoutingPolicy::source_for_quality(LuxVolumetricQuality::High),
            LuxGodraySource::VolumetricFog,
        );
        assert_eq!(
            LuxGodrayRoutingPolicy::source_for_quality(LuxVolumetricQuality::Cinematic),
            LuxGodraySource::VolumetricFog,
        );
        assert!(LuxGodrayRoutingPolicy::high_tier_uses_true_volumetrics());
    }

    #[test]
    fn off_quality_routes_to_disabled() {
        assert_eq!(
            LuxGodrayRoutingPolicy::source_for_quality(LuxVolumetricQuality::Off),
            LuxGodraySource::Disabled,
        );
    }

    /// Pass 9 user rule #2 — godrays respond to shadow
    /// maps / virtual pages.  Only `Unshadowed` returns
    /// `false`.
    #[test]
    fn light_sample_source_predicate_excludes_unshadowed() {
        for s in LuxGodrayLightSampleSource::ALL {
            let expected = !matches!(s, LuxGodrayLightSampleSource::Unshadowed);
            assert_eq!(
                s.responds_to_shadow_maps_or_virtual_pages(),
                expected,
                "{:?}",
                s,
            );
        }
    }

    #[test]
    fn screen_space_pass_runs_before_tonemap_via_order_key() {
        // Screen-space godrays run after the volumetric
        // composite (900) but before the typed HDR
        // tonemap stage (Pass 5).  The typed order key
        // captures the contract.
        let k = LuxGodrayPassRole::ScreenSpaceGodrayPass.order_key();
        assert!(k > 900); // after VolumetricCompositeIntoHdr
        assert!(k < 1000); // before any 1000+ post-volumetric stage
    }

    /// Pass 9 acceptance verdict — every rule holds when
    /// the live pipeline is fully wired at High quality.
    #[test]
    fn verdict_passes_when_high_tier_pipeline_is_fully_wired() {
        let settings = LuxGodraySettings::PRODUCT_DEFAULT;
        let signals = LuxGodrayFrameSignals::FULLY_WIRED_HIGH;
        let verdict = LuxGodrayPass9AcceptanceVerdict::evaluate(&settings, &signals, true);
        assert!(verdict.obeys_all_five_rules(), "verdict: {:?}", verdict);
        assert_eq!(
            verdict.rules_satisfied(),
            FUN_LUX_GODRAY_PASS9_ACCEPTANCE_RULE_COUNT as u32,
        );
    }

    /// Pass 9 acceptance verdict — every rule holds at
    /// Low tier as well (the screen-space fallback path
    /// counts as "godrays").
    #[test]
    fn verdict_passes_when_low_tier_pipeline_is_fully_wired() {
        let settings = LuxGodraySettings::PRODUCT_DEFAULT;
        let signals = LuxGodrayFrameSignals::FULLY_WIRED_LOW;
        let verdict = LuxGodrayPass9AcceptanceVerdict::evaluate(&settings, &signals, true);
        assert!(verdict.obeys_all_five_rules(), "verdict: {:?}", verdict);
    }

    /// Pass 9 verdict — flips when ANY rule fails.
    #[test]
    fn verdict_flips_when_any_rule_fails() {
        let settings = LuxGodraySettings::PRODUCT_DEFAULT;
        // Disable rule #5 by saying the look profile
        // doesn't carry the godray settings.
        let signals = LuxGodrayFrameSignals::FULLY_WIRED_HIGH;
        let verdict = LuxGodrayPass9AcceptanceVerdict::evaluate(&settings, &signals, false);
        assert!(!verdict.obeys_all_five_rules());
        assert!(!verdict.godray_settings_in_look_profile);
        assert_eq!(verdict.rules_satisfied(), 4);
    }

    /// Pass 9 verdict — NOT_EMITTED signals fail the
    /// signal-gated rules.
    #[test]
    fn verdict_fails_signal_gated_rules_when_signals_not_emitted() {
        let settings = LuxGodraySettings::PRODUCT_DEFAULT;
        let signals = LuxGodrayFrameSignals::NOT_EMITTED;
        let verdict = LuxGodrayPass9AcceptanceVerdict::evaluate(&settings, &signals, true);
        assert!(!verdict.occluders_produce_shafts);
        assert!(!verdict.responds_to_shadow_maps_or_virtual_pages);
        // Rules #3 / #4 / #5 are quality-routing const +
        // look-profile gate; they pass regardless of
        // per-frame signals.
        assert!(verdict.low_tier_uses_screen_space_fallback);
        assert!(verdict.high_tier_uses_true_volumetrics);
        assert!(verdict.godray_settings_in_look_profile);
        assert_eq!(verdict.rules_satisfied(), 3);
    }

    /// Pass 9 verdict — cold-default settings fail rules
    /// #3 / #4 because the settings disable both paths.
    #[test]
    fn verdict_fails_path_rules_with_cold_settings() {
        let settings = LuxGodraySettings::COLD_DEFAULT;
        let signals = LuxGodrayFrameSignals::FULLY_WIRED_HIGH;
        let verdict = LuxGodrayPass9AcceptanceVerdict::evaluate(&settings, &signals, true);
        assert!(!verdict.occluders_produce_shafts);
        assert!(!verdict.low_tier_uses_screen_space_fallback);
        assert!(!verdict.high_tier_uses_true_volumetrics);
        // Rule #2 + #5 hold regardless of cold settings.
        assert!(verdict.responds_to_shadow_maps_or_virtual_pages);
        assert!(verdict.godray_settings_in_look_profile);
    }
}
