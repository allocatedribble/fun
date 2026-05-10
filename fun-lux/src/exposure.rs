//! Pass 5 — typed exposure system.
//!
//! Encodes the user's "5.2 Exposure system" rules:
//!
//! - Five typed modes (Manual / Auto / CameraWeightedAuto /
//!   GameplayZoneOverride / CinematicLocked).
//! - Typed auto-exposure config: luminance histogram bin
//!   count, outlier rejection, time-smoothing, separate
//!   adaptation speeds for brightening + darkening.
//! - Typed controls: `min_exposure_ev`, `max_exposure_ev`,
//!   `target_middle_gray`, `adaptation_up_seconds`,
//!   `adaptation_down_seconds`, `histogram_low_percentile`,
//!   `histogram_high_percentile`.

pub const FUN_LUX_EXPOSURE_SCHEMA_VERSION: u16 = 1;
pub const FUN_LUX_EXPOSURE_MODE_COUNT: usize = 5;

// ============================================================================
// Section 1 — Typed exposure mode
// ============================================================================

/// Typed exposure mode. Each mode names a typed runtime
/// strategy the renderer uses to pick the per-frame
/// exposure value.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunLuxExposureMode {
    /// Manual: the typed `manual_exposure_ev` field on
    /// [`FunLuxExposureSettings`] is the per-frame
    /// exposure. No auto adjustment.
    Manual,
    /// Typed product-default: full-frame luminance
    /// histogram drives the per-frame exposure.
    #[default]
    Auto,
    /// Camera-weighted auto exposure: the histogram is
    /// weighted by camera-screen-center-ness (center
    /// pixels count more than edges).
    CameraWeightedAuto,
    /// Gameplay-zone exposure override: the level can
    /// place typed zones in world space that override the
    /// auto-exposure value when the camera enters the
    /// zone.
    GameplayZoneOverride,
    /// Cinematic locked: the typed `manual_exposure_ev`
    /// is used + no auto adaptation runs. The renderer
    /// also refuses to interpolate to a new value until
    /// the typed cinematic state ends.
    CinematicLocked,
}

impl FunLuxExposureMode {
    pub const ALL: [Self; FUN_LUX_EXPOSURE_MODE_COUNT] = [
        Self::Manual,
        Self::Auto,
        Self::CameraWeightedAuto,
        Self::GameplayZoneOverride,
        Self::CinematicLocked,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Auto => "auto",
            Self::CameraWeightedAuto => "camera_weighted_auto",
            Self::GameplayZoneOverride => "gameplay_zone_override",
            Self::CinematicLocked => "cinematic_locked",
        }
    }

    /// Typed predicate: does this mode read the luminance
    /// histogram each frame?
    #[must_use]
    pub const fn requires_histogram(self) -> bool {
        matches!(
            self,
            Self::Auto | Self::CameraWeightedAuto | Self::GameplayZoneOverride
        )
    }

    /// Typed predicate: does this mode read the typed
    /// `manual_exposure_ev` field?
    #[must_use]
    pub const fn uses_manual_value(self) -> bool {
        matches!(self, Self::Manual | Self::CinematicLocked)
    }

    /// Typed predicate: does this mode interpolate exposure
    /// over time?
    #[must_use]
    pub const fn is_smoothed_over_time(self) -> bool {
        matches!(
            self,
            Self::Auto | Self::CameraWeightedAuto | Self::GameplayZoneOverride
        )
    }

    /// Typed predicate: is this mode locked (no
    /// adaptation)? Used by the renderer to skip the
    /// adaptation systems.
    #[must_use]
    pub const fn is_locked(self) -> bool {
        matches!(self, Self::CinematicLocked | Self::Manual)
    }
}

// ============================================================================
// Section 2 — Typed histogram config
// ============================================================================

/// Typed luminance histogram configuration. The renderer
/// reads this to size the histogram compute buffer + reject
/// outliers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunLuxAutoExposureHistogramConfig {
    pub schema_version: u16,
    /// Number of typed luminance bins. Typical: 256.
    pub bin_count: u16,
    /// Typed min luminance in EV (log2 cd/m²).
    pub min_luminance_ev_q8: i16,
    /// Typed max luminance in EV.
    pub max_luminance_ev_q8: i16,
    /// Typed low percentile in Q16 fixed-point
    /// (`32768 = 50.0%`, `6553 = 10.0%`). The auto-exposure
    /// solver discards bins below this percentile to avoid
    /// being dragged down by extreme darks.
    pub low_percentile_q16: u16,
    /// Typed high percentile in Q16. The solver discards
    /// bins above this percentile to avoid being dragged
    /// up by extreme highs.
    pub high_percentile_q16: u16,
    /// Typed outlier rejection strength. `0 = no rejection`;
    /// `255 = aggressive`. Drives the typed bin-weight
    /// roll-off the solver uses to ignore single-pixel
    /// flicker.
    pub outlier_rejection_strength_q8: u8,
}

impl FunLuxAutoExposureHistogramConfig {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_EXPOSURE_SCHEMA_VERSION,
        bin_count: 256,
        // EV range: [-8, +12] stops (in Q8 fixed-point).
        min_luminance_ev_q8: -8 * 256,
        max_luminance_ev_q8: 12 * 256,
        // 10% — 90% percentile bracket.
        low_percentile_q16: 6553,   // 10.0%
        high_percentile_q16: 58982, // 90.0%
        outlier_rejection_strength_q8: 128,
    };

    /// Typed predicate: are the typed percentiles valid?
    /// (`low < high`, both within `[0, 65535]`.)
    #[must_use]
    pub const fn percentiles_valid(&self) -> bool {
        self.low_percentile_q16 < self.high_percentile_q16
    }

    /// Typed EV span of the histogram.
    #[must_use]
    pub const fn ev_span_q8(&self) -> i32 {
        self.max_luminance_ev_q8 as i32 - self.min_luminance_ev_q8 as i32
    }
}

// ============================================================================
// Section 3 — Typed adaptation
// ============================================================================

/// Typed exposure adaptation timing. Separate speeds for
/// brightening (eye-dilation) and darkening (eye-squinting)
/// — the human eye adapts faster from dark to light than
/// the reverse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunLuxAutoExposureAdaptation {
    pub schema_version: u16,
    /// Typed seconds the renderer takes to interpolate
    /// exposure toward a brighter target (Q8 fixed-point;
    /// `256 = 1.0s`).
    pub adaptation_up_seconds_q8: u16,
    /// Typed seconds for darker target.
    pub adaptation_down_seconds_q8: u16,
}

impl FunLuxAutoExposureAdaptation {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_EXPOSURE_SCHEMA_VERSION,
        // Brightening: 0.6s — eye dilates fast in bright
        // light.
        adaptation_up_seconds_q8: 154, // ~0.6s
        // Darkening: 2.4s — eye adapts slower to dark.
        adaptation_down_seconds_q8: 614, // ~2.4s
    };

    /// Typed predicate: are the typed adaptation speeds
    /// non-zero?
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.adaptation_up_seconds_q8 > 0 && self.adaptation_down_seconds_q8 > 0
    }

    /// Typed predicate: does this adaptation honour the
    /// "human eye brightens faster than it darkens" rule?
    #[must_use]
    pub const fn brightening_is_faster_than_darkening(&self) -> bool {
        self.adaptation_up_seconds_q8 < self.adaptation_down_seconds_q8
    }
}

// ============================================================================
// Section 4 — Typed exposure controls (5.2 listed dials)
// ============================================================================

/// Typed exposure controls. Encodes every dial from the
/// user's 5.2 list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunLuxExposureControls {
    pub schema_version: u16,
    /// Typed minimum exposure (Q8 EV). The auto-exposure
    /// solver clamps below this value to avoid going
    /// blindingly bright in dark scenes.
    pub min_exposure_ev_q8: i16,
    /// Typed maximum exposure (Q8 EV).
    pub max_exposure_ev_q8: i16,
    /// Typed target middle gray in Q16 (`32768 = 0.5`).
    /// The solver picks an exposure such that the histogram
    /// median maps to this value.
    pub target_middle_gray_q16: u16,
    /// Typed adaptation speeds.
    pub adaptation: FunLuxAutoExposureAdaptation,
    /// Typed histogram config.
    pub histogram: FunLuxAutoExposureHistogramConfig,
}

impl FunLuxExposureControls {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_EXPOSURE_SCHEMA_VERSION,
        // EV range: [-6, +9] stops.
        min_exposure_ev_q8: -6 * 256,
        max_exposure_ev_q8: 9 * 256,
        // Middle gray ≈ 0.18 (typical photography target).
        // 0.18 * 65536 ≈ 11796.
        target_middle_gray_q16: 11796,
        adaptation: FunLuxAutoExposureAdaptation::PRODUCT_DEFAULT,
        histogram: FunLuxAutoExposureHistogramConfig::PRODUCT_DEFAULT,
    };

    /// Typed predicate: are the typed exposure bounds
    /// well-formed (`min < max`)?
    #[must_use]
    pub const fn exposure_bounds_valid(&self) -> bool {
        self.min_exposure_ev_q8 < self.max_exposure_ev_q8
    }

    /// Typed predicate: do every typed sub-control pass
    /// their typed acceptance tests?
    #[must_use]
    pub const fn is_production_acceptable(&self) -> bool {
        self.exposure_bounds_valid()
            && self.histogram.percentiles_valid()
            && self.adaptation.is_active()
            && self.adaptation.brightening_is_faster_than_darkening()
    }
}

// ============================================================================
// Section 5 — Typed exposure settings
// ============================================================================

/// Typed exposure settings — the typed handle the renderer
/// reads each frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunLuxExposureSettings {
    pub schema_version: u16,
    pub mode: FunLuxExposureMode,
    /// Typed manual exposure value (Q8 EV). Only consulted
    /// when `mode.uses_manual_value()`.
    pub manual_exposure_ev_q8: i16,
    pub controls: FunLuxExposureControls,
}

impl FunLuxExposureSettings {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_EXPOSURE_SCHEMA_VERSION,
        mode: FunLuxExposureMode::Auto,
        manual_exposure_ev_q8: 0,
        controls: FunLuxExposureControls::PRODUCT_DEFAULT,
    };

    pub const COLD_DEFAULT: Self = Self {
        schema_version: FUN_LUX_EXPOSURE_SCHEMA_VERSION,
        mode: FunLuxExposureMode::Manual,
        manual_exposure_ev_q8: 0,
        controls: FunLuxExposureControls::PRODUCT_DEFAULT,
    };

    /// Typed predicate: does this exposure setting drive a
    /// real auto-exposure pass this frame? `Manual` and
    /// `CinematicLocked` return `false`.
    #[must_use]
    pub const fn drives_auto_exposure_pass(&self) -> bool {
        self.mode.requires_histogram()
    }

    /// Typed predicate: do every typed sub-control pass
    /// their typed acceptance tests?
    #[must_use]
    pub const fn is_production_acceptable(&self) -> bool {
        self.controls.is_production_acceptable()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_LUX_EXPOSURE_SCHEMA_VERSION, 1);
        assert_eq!(FUN_LUX_EXPOSURE_MODE_COUNT, 5);
        assert_eq!(FunLuxExposureMode::ALL.len(), FUN_LUX_EXPOSURE_MODE_COUNT);
    }

    #[test]
    fn exposure_mode_taxonomy_strings_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for m in FunLuxExposureMode::ALL {
            assert!(seen.insert(m.as_str()), "duplicate: {}", m.as_str());
        }
    }

    /// Pass 5 acceptance: typed mode predicates.
    #[test]
    fn exposure_mode_predicates_match_taxonomy() {
        // Auto-family modes require the histogram.
        assert!(FunLuxExposureMode::Auto.requires_histogram());
        assert!(FunLuxExposureMode::CameraWeightedAuto.requires_histogram());
        assert!(FunLuxExposureMode::GameplayZoneOverride.requires_histogram());
        // Manual + Cinematic do not.
        assert!(!FunLuxExposureMode::Manual.requires_histogram());
        assert!(!FunLuxExposureMode::CinematicLocked.requires_histogram());

        // Manual + Cinematic use the typed manual value.
        assert!(FunLuxExposureMode::Manual.uses_manual_value());
        assert!(FunLuxExposureMode::CinematicLocked.uses_manual_value());
        assert!(!FunLuxExposureMode::Auto.uses_manual_value());

        // Auto-family is smoothed; Manual + Cinematic are
        // locked.
        assert!(FunLuxExposureMode::Auto.is_smoothed_over_time());
        assert!(FunLuxExposureMode::CinematicLocked.is_locked());
        assert!(FunLuxExposureMode::Manual.is_locked());
        assert!(!FunLuxExposureMode::Auto.is_locked());
    }

    /// Pass 5 acceptance: typed histogram config.
    #[test]
    fn histogram_config_percentiles_valid() {
        let cfg = FunLuxAutoExposureHistogramConfig::PRODUCT_DEFAULT;
        assert!(cfg.percentiles_valid());
        assert_eq!(cfg.bin_count, 256);
        assert!(cfg.ev_span_q8() > 0);
    }

    #[test]
    fn histogram_config_invalid_percentiles_rejected() {
        let mut cfg = FunLuxAutoExposureHistogramConfig::PRODUCT_DEFAULT;
        cfg.low_percentile_q16 = 60000;
        cfg.high_percentile_q16 = 6553;
        assert!(!cfg.percentiles_valid());
    }

    /// Pass 5 acceptance: typed separate adaptation speeds.
    #[test]
    fn adaptation_brightening_faster_than_darkening() {
        let adapt = FunLuxAutoExposureAdaptation::PRODUCT_DEFAULT;
        assert!(adapt.is_active());
        assert!(adapt.brightening_is_faster_than_darkening());
    }

    #[test]
    fn adaptation_inactive_when_zero() {
        let adapt = FunLuxAutoExposureAdaptation {
            schema_version: FUN_LUX_EXPOSURE_SCHEMA_VERSION,
            adaptation_up_seconds_q8: 0,
            adaptation_down_seconds_q8: 0,
        };
        assert!(!adapt.is_active());
    }

    /// Pass 5 acceptance: typed control dials present.
    #[test]
    fn exposure_controls_carry_every_listed_dial() {
        let controls = FunLuxExposureControls::PRODUCT_DEFAULT;
        assert!(controls.exposure_bounds_valid());
        assert!(controls.is_production_acceptable());
        // Spot check every typed dial is non-default.
        assert!(controls.min_exposure_ev_q8 < 0);
        assert!(controls.max_exposure_ev_q8 > 0);
        assert!(controls.target_middle_gray_q16 > 0);
        assert!(controls.adaptation.adaptation_up_seconds_q8 > 0);
        assert!(controls.adaptation.adaptation_down_seconds_q8 > 0);
        assert!(controls.histogram.low_percentile_q16 > 0);
        assert!(controls.histogram.high_percentile_q16 < u16::MAX);
    }

    #[test]
    fn exposure_controls_reject_invalid_bounds() {
        let mut controls = FunLuxExposureControls::PRODUCT_DEFAULT;
        controls.min_exposure_ev_q8 = 100;
        controls.max_exposure_ev_q8 = -100;
        assert!(!controls.exposure_bounds_valid());
        assert!(!controls.is_production_acceptable());
    }

    /// Pass 5 acceptance: typed exposure settings drive
    /// auto-exposure passes only under Auto-family modes.
    #[test]
    fn exposure_settings_drives_auto_exposure_pass_predicate() {
        let mut settings = FunLuxExposureSettings::PRODUCT_DEFAULT;
        assert!(settings.drives_auto_exposure_pass());
        settings.mode = FunLuxExposureMode::Manual;
        assert!(!settings.drives_auto_exposure_pass());
        settings.mode = FunLuxExposureMode::CinematicLocked;
        assert!(!settings.drives_auto_exposure_pass());
        settings.mode = FunLuxExposureMode::CameraWeightedAuto;
        assert!(settings.drives_auto_exposure_pass());
    }

    #[test]
    fn cold_default_settings_use_manual_mode() {
        let cold = FunLuxExposureSettings::COLD_DEFAULT;
        assert_eq!(cold.mode, FunLuxExposureMode::Manual);
        assert_eq!(cold.manual_exposure_ev_q8, 0);
        assert!(!cold.drives_auto_exposure_pass());
    }
}
