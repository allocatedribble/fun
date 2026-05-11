//! Pass 5 — typed tonemapping system.
//!
//! Encodes the user's "5.3 Tonemapping" rules:
//!
//! - At least three typed modes: `NeutralReinhard` (for
//!   debugging), `AcesFilmic` (industry-standard), and
//!   `CustomFunLux` (the project's authored curve).
//! - The typed `FunLuxCustomTonemapCurve` exposes every
//!   listed dial: toe strength, shoulder strength,
//!   highlight saturation preservation, shadow color bias,
//!   white point, contrast, midtone lift.
//! - The pipeline ordering rule "Keep tonemapping after
//!   bloom and HDR composite" is enforced by the typed
//!   `FunLuxHdrPipelineStage` ordering in
//!   [`crate::hdr_format`]; this module reaffirms the rule
//!   via the typed `tonemap_pipeline_ordering_holds`
//!   predicate.

use crate::hdr_format::{
    FunLuxHdrPipelineStage, tonemap_runs_after_bloom_and_composite,
    tonemap_runs_before_display_encoding,
};

pub const FUN_LUX_TONEMAP_SCHEMA_VERSION: u16 = 1;
pub const FUN_LUX_TONEMAP_MODE_COUNT: usize = 3;

// ============================================================================
// Section 1 — Typed tonemap mode
// ============================================================================

/// Typed tonemap mode. Pass 5 requires at least these three.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunLuxTonemapMode {
    /// Neutral / Reinhard — debugging mode. Preserves HDR
    /// energy with the typed Reinhard curve so the renderer
    /// can verify HDR accumulation + bloom paths without
    /// tonemap-induced artifacts.
    NeutralReinhard,
    /// ACES filmic — industry-standard cinema curve.
    AcesFilmic,
    /// Custom FunLux curve. Reads the typed
    /// [`FunLuxCustomTonemapCurve`] dials on
    /// [`FunLuxTonemapSettings::custom_curve`].
    #[default]
    CustomFunLux,
}

impl FunLuxTonemapMode {
    pub const ALL: [Self; FUN_LUX_TONEMAP_MODE_COUNT] =
        [Self::NeutralReinhard, Self::AcesFilmic, Self::CustomFunLux];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NeutralReinhard => "neutral_reinhard",
            Self::AcesFilmic => "aces_filmic",
            Self::CustomFunLux => "custom_fun_lux",
        }
    }

    /// Typed predicate: is this the debug-only neutral
    /// mode? Production product builds should typically use
    /// `AcesFilmic` or `CustomFunLux`.
    #[must_use]
    pub const fn is_debug_neutral(self) -> bool {
        matches!(self, Self::NeutralReinhard)
    }

    /// Typed predicate: does this mode read the typed
    /// custom curve dials?
    #[must_use]
    pub const fn reads_custom_curve(self) -> bool {
        matches!(self, Self::CustomFunLux)
    }

    /// Typed predicate: is this mode suitable for cinematic
    /// product output? `AcesFilmic` and `CustomFunLux` are.
    #[must_use]
    pub const fn is_cinematic(self) -> bool {
        matches!(self, Self::AcesFilmic | Self::CustomFunLux)
    }
}

// ============================================================================
// Section 2 — Typed custom FunLux tonemap curve (5.3 7-dial)
// ============================================================================

/// Typed custom FunLux tonemap curve. Exposes every dial
/// from the user's 5.3 list:
///
/// - Toe strength
/// - Shoulder strength
/// - Highlight saturation preservation
/// - Shadow color bias
/// - White point
/// - Contrast
/// - Midtone lift
///
/// Every dial is stored in Q8 fixed-point (or as plain f32
/// for white point) so the typed record stays Hash-stable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FunLuxCustomTonemapCurve {
    pub schema_version: u16,
    /// Toe strength — controls how darks roll into black.
    /// Q8: `0 = no toe`, `256 = full toe`. Higher values
    /// preserve more shadow detail.
    pub toe_strength_q8: u16,
    /// Shoulder strength — controls how highlights roll
    /// into white. Q8 fixed-point. Higher values
    /// compress highlights more aggressively.
    pub shoulder_strength_q8: u16,
    /// Highlight saturation preservation. `0 =
    /// desaturate highlights to white`, `256 = preserve
    /// full saturation`. Higher values are more cinematic
    /// but can produce neon highlights.
    pub highlight_saturation_preservation_q8: u16,
    /// Shadow color bias — RGB additive shift to shadows.
    /// Typical use: warm shadows (positive R, slight
    /// negative B) or cool shadows (negative R, positive
    /// B). Each component is f32 in
    /// `[-0.25, +0.25]` typical range.
    pub shadow_color_bias_rgb: [f32; 3],
    /// White point — luminance value that maps to display
    /// white (`1.0`). Higher values stretch the dynamic
    /// range. Typical: `4.0` (4 stops above middle gray).
    pub white_point_luminance: f32,
    /// Contrast — `0 = neutral`, positive increases
    /// contrast, negative decreases. Q8 signed:
    /// `[-256, +256]`.
    pub contrast_q8: i16,
    /// Midtone lift — `0 = neutral`, positive lifts
    /// midtones, negative crushes them. Q8 signed.
    pub midtone_lift_q8: i16,
}

impl FunLuxCustomTonemapCurve {
    /// Typed neutral curve — every dial at zero / identity.
    /// Useful as a starting point for authoring or as a
    /// "passthrough" baseline.
    pub const NEUTRAL: Self = Self {
        schema_version: FUN_LUX_TONEMAP_SCHEMA_VERSION,
        toe_strength_q8: 0,
        shoulder_strength_q8: 0,
        highlight_saturation_preservation_q8: 256,
        shadow_color_bias_rgb: [0.0, 0.0, 0.0],
        white_point_luminance: 1.0,
        contrast_q8: 0,
        midtone_lift_q8: 0,
    };

    /// Typed product-default curve — moderate toe + shoulder,
    /// full highlight saturation, neutral color bias, 4-stop
    /// white point, slight midtone lift for cinematic feel.
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_TONEMAP_SCHEMA_VERSION,
        toe_strength_q8: 128,
        shoulder_strength_q8: 178,
        highlight_saturation_preservation_q8: 220,
        shadow_color_bias_rgb: [0.0, 0.0, 0.0],
        white_point_luminance: 4.0,
        contrast_q8: 25,
        midtone_lift_q8: 10,
    };

    /// Typed predicate: is this curve the neutral baseline?
    #[must_use]
    pub fn is_neutral(&self) -> bool {
        self.toe_strength_q8 == 0
            && self.shoulder_strength_q8 == 0
            && self.contrast_q8 == 0
            && self.midtone_lift_q8 == 0
            && self.shadow_color_bias_rgb.iter().all(|c| *c == 0.0)
            && self.white_point_luminance == 1.0
    }

    /// Typed predicate: does this curve preserve highlight
    /// saturation (preservation > 50%)?
    #[must_use]
    pub const fn preserves_highlight_saturation(&self) -> bool {
        self.highlight_saturation_preservation_q8 >= 128
    }

    /// Typed predicate: is the white-point value valid
    /// (positive + finite)?
    #[must_use]
    pub fn white_point_valid(&self) -> bool {
        self.white_point_luminance > 0.0 && self.white_point_luminance.is_finite()
    }
}

// ============================================================================
// Section 3 — Typed tonemap settings
// ============================================================================

/// Typed tonemap settings. The typed handle the renderer
/// reads each frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FunLuxTonemapSettings {
    pub schema_version: u16,
    pub mode: FunLuxTonemapMode,
    pub custom_curve: FunLuxCustomTonemapCurve,
}

impl FunLuxTonemapSettings {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_TONEMAP_SCHEMA_VERSION,
        mode: FunLuxTonemapMode::CustomFunLux,
        custom_curve: FunLuxCustomTonemapCurve::PRODUCT_DEFAULT,
    };

    /// Typed debug-default — Reinhard with neutral curve.
    /// Reserved for verifying HDR accumulation + bloom
    /// paths without tonemap-induced artifacts.
    pub const DEBUG_NEUTRAL: Self = Self {
        schema_version: FUN_LUX_TONEMAP_SCHEMA_VERSION,
        mode: FunLuxTonemapMode::NeutralReinhard,
        custom_curve: FunLuxCustomTonemapCurve::NEUTRAL,
    };

    /// Typed cinematic-default — ACES filmic with the
    /// product custom curve. Reserved for cinematic
    /// captures + finished product builds.
    pub const CINEMATIC_ACES: Self = Self {
        schema_version: FUN_LUX_TONEMAP_SCHEMA_VERSION,
        mode: FunLuxTonemapMode::AcesFilmic,
        custom_curve: FunLuxCustomTonemapCurve::PRODUCT_DEFAULT,
    };

    /// Typed predicate: is this setting suitable for
    /// production output? Returns `false` for the
    /// debug-only `NeutralReinhard` mode.
    #[must_use]
    pub fn is_production_acceptable(&self) -> bool {
        self.mode.is_cinematic() && self.custom_curve.white_point_valid()
    }

    /// Pass 5 typed acceptance: "Keep tonemapping after
    /// bloom and HDR composite." Returns `true` when the
    /// typed pipeline ordering invariants from
    /// [`crate::hdr_format`] hold.
    #[must_use]
    pub const fn pipeline_ordering_holds() -> bool {
        tonemap_runs_after_bloom_and_composite() && tonemap_runs_before_display_encoding()
    }

    /// Typed stage selector: which stage runs this
    /// tonemap? Always `FunLuxHdrPipelineStage::Tonemap`.
    #[must_use]
    pub const fn pipeline_stage() -> FunLuxHdrPipelineStage {
        FunLuxHdrPipelineStage::Tonemap
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_LUX_TONEMAP_SCHEMA_VERSION, 1);
        assert_eq!(FUN_LUX_TONEMAP_MODE_COUNT, 3);
        assert_eq!(FunLuxTonemapMode::ALL.len(), FUN_LUX_TONEMAP_MODE_COUNT);
    }

    #[test]
    fn tonemap_mode_taxonomy_strings_are_unique() {
        let mut seen = hashbrown::HashSet::new();
        for m in FunLuxTonemapMode::ALL {
            assert!(seen.insert(m.as_str()), "duplicate: {}", m.as_str());
        }
    }

    /// Pass 5 acceptance: typed mode predicates.
    #[test]
    fn tonemap_mode_predicates_match_taxonomy() {
        assert!(FunLuxTonemapMode::NeutralReinhard.is_debug_neutral());
        assert!(!FunLuxTonemapMode::AcesFilmic.is_debug_neutral());
        assert!(!FunLuxTonemapMode::CustomFunLux.is_debug_neutral());

        assert!(FunLuxTonemapMode::CustomFunLux.reads_custom_curve());
        assert!(!FunLuxTonemapMode::AcesFilmic.reads_custom_curve());

        assert!(FunLuxTonemapMode::AcesFilmic.is_cinematic());
        assert!(FunLuxTonemapMode::CustomFunLux.is_cinematic());
        assert!(!FunLuxTonemapMode::NeutralReinhard.is_cinematic());
    }

    /// Pass 5 acceptance: typed neutral curve.
    #[test]
    fn neutral_curve_is_neutral_predicate() {
        let curve = FunLuxCustomTonemapCurve::NEUTRAL;
        assert!(curve.is_neutral());
        assert!(curve.preserves_highlight_saturation());
        assert!(curve.white_point_valid());
    }

    /// Pass 5 acceptance: typed product-default curve.
    #[test]
    fn product_default_curve_is_not_neutral() {
        let curve = FunLuxCustomTonemapCurve::PRODUCT_DEFAULT;
        assert!(!curve.is_neutral());
        assert!(curve.preserves_highlight_saturation());
        assert!(curve.white_point_valid());
    }

    /// Pass 5 acceptance: every 5.3 dial lives in the typed
    /// curve.
    #[test]
    fn custom_curve_carries_every_listed_dial() {
        let curve = FunLuxCustomTonemapCurve::PRODUCT_DEFAULT;
        // Toe strength.
        assert!(curve.toe_strength_q8 > 0);
        // Shoulder strength.
        assert!(curve.shoulder_strength_q8 > 0);
        // Highlight saturation preservation.
        assert!(curve.highlight_saturation_preservation_q8 > 0);
        // Shadow color bias (typed RGB).
        assert_eq!(curve.shadow_color_bias_rgb.len(), 3);
        // White point.
        assert!(curve.white_point_luminance > 0.0);
        // Contrast.
        assert!(curve.contrast_q8 != 0);
        // Midtone lift.
        assert!(curve.midtone_lift_q8 != 0);
    }

    #[test]
    fn product_default_settings_use_custom_fun_lux() {
        let settings = FunLuxTonemapSettings::PRODUCT_DEFAULT;
        assert_eq!(settings.mode, FunLuxTonemapMode::CustomFunLux);
        assert!(settings.is_production_acceptable());
    }

    #[test]
    fn debug_neutral_settings_not_production_acceptable() {
        let settings = FunLuxTonemapSettings::DEBUG_NEUTRAL;
        assert_eq!(settings.mode, FunLuxTonemapMode::NeutralReinhard);
        assert!(!settings.is_production_acceptable());
    }

    #[test]
    fn cinematic_aces_settings_are_production_acceptable() {
        let settings = FunLuxTonemapSettings::CINEMATIC_ACES;
        assert_eq!(settings.mode, FunLuxTonemapMode::AcesFilmic);
        assert!(settings.is_production_acceptable());
    }

    /// Pass 5 acceptance: "Keep tonemapping after bloom and
    /// HDR composite."
    #[test]
    fn tonemap_pipeline_ordering_holds() {
        assert!(FunLuxTonemapSettings::pipeline_ordering_holds());
        assert_eq!(
            FunLuxTonemapSettings::pipeline_stage(),
            FunLuxHdrPipelineStage::Tonemap,
        );
    }

    #[test]
    fn invalid_white_point_rejects_production_acceptable() {
        let mut settings = FunLuxTonemapSettings::PRODUCT_DEFAULT;
        settings.custom_curve.white_point_luminance = -1.0;
        assert!(!settings.is_production_acceptable());
        settings.custom_curve.white_point_luminance = f32::NAN;
        assert!(!settings.is_production_acceptable());
    }
}
