//! Typed look profile.
//!
//! `FunLuxLookProfile` carries the renderer-neutral artistic
//! look knobs that `fun-renderer` consumes when it composes the
//! final image: exposure compensation, white point, tone-map
//! operator, color-grading LUT, contrast, saturation. The
//! profile is part of `LuxFramePlan::look_profile` so renderers
//! never invent look policy — they read it.

pub const FUN_LUX_LOOK_SCHEMA_VERSION: u16 = 1;

/// Typed tone-map operator. The renderer maps these to its
/// concrete tone-map shader; the typed value names the
/// algorithm, not any GPU pipeline.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunLuxToneMapOperator {
    /// Linear: identity mapping, useful for HDR-direct
    /// pipelines.
    Linear,
    /// Reinhard: classic global tone map.
    Reinhard,
    /// ACES Filmic: industry-standard tone map.
    #[default]
    AcesFilmic,
    /// Agx: Blender-default tone map; soft shoulder.
    Agx,
}

impl FunLuxToneMapOperator {
    pub const ALL: [Self; 4] = [Self::Linear, Self::Reinhard, Self::AcesFilmic, Self::Agx];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Reinhard => "reinhard",
            Self::AcesFilmic => "aces_filmic",
            Self::Agx => "agx",
        }
    }

    /// Typed predicate: does this operator preserve HDR
    /// dynamic range linearly?
    #[must_use]
    pub const fn is_linear(self) -> bool {
        matches!(self, Self::Linear)
    }
}

/// Typed white-point preset. Names the chromaticity target
/// rather than a CIE xy coordinate; the renderer maps the
/// preset to a 3×3 chromatic adaptation matrix.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunLuxWhitePoint {
    /// D65 — sRGB / Rec.709 reference white.
    #[default]
    D65,
    /// D60 — ACES reference white.
    D60,
    /// D50 — print reference white.
    D50,
    /// Tungsten (~2856K).
    Tungsten,
}

impl FunLuxWhitePoint {
    pub const ALL: [Self; 4] = [Self::D65, Self::D60, Self::D50, Self::Tungsten];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::D65 => "d65",
            Self::D60 => "d60",
            Self::D50 => "d50",
            Self::Tungsten => "tungsten",
        }
    }

    /// Typed correlated colour temperature in kelvin.
    #[must_use]
    pub const fn correlated_color_temperature_kelvin(self) -> u32 {
        match self {
            Self::D65 => 6504,
            Self::D60 => 6000,
            Self::D50 => 5003,
            Self::Tungsten => 2856,
        }
    }
}

/// Typed color-grading LUT handle. The renderer maps the typed
/// stable id to a real LUT texture; the profile here names the
/// LUT without exposing any backend handle.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunLuxColorGradingLut {
    pub schema_version: u16,
    pub stable_id: &'static str,
    pub strength: i16,
    pub three_dimensional: bool,
}

impl FunLuxColorGradingLut {
    pub const IDENTITY: Self = Self {
        schema_version: FUN_LUX_LOOK_SCHEMA_VERSION,
        stable_id: "fun_lux.look.lut.identity",
        // Strength is a fixed-point [-100, +100] dial; 0 = identity.
        strength: 0,
        three_dimensional: true,
    };

    #[must_use]
    pub const fn new(stable_id: &'static str, strength: i16, three_dimensional: bool) -> Self {
        Self {
            schema_version: FUN_LUX_LOOK_SCHEMA_VERSION,
            stable_id,
            strength,
            three_dimensional,
        }
    }

    /// Typed predicate: is this LUT the identity (no effect)?
    #[must_use]
    pub const fn is_identity(self) -> bool {
        self.strength == 0
    }
}

/// Typed look profile.
///
/// `FunLuxLookProfile` is part of `LuxFramePlan::look_profile`;
/// the renderer reads it and applies tone mapping, white point
/// adaptation, color grading. None of the fields name a backend
/// handle — the typed values describe artistic intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunLuxLookProfile {
    pub schema_version: u16,
    /// Exposure compensation in stops (EV). Positive
    /// brightens, negative darkens. Typical range: ±6.0
    /// stops; the renderer clamps to its hardware range.
    pub exposure_ev_compensation_q8: i16,
    /// Tone-map operator selecting the HDR → LDR mapping.
    pub tone_map: FunLuxToneMapOperator,
    /// White point preset for chromatic adaptation.
    pub white_point: FunLuxWhitePoint,
    /// Color-grading LUT handle.
    pub color_grading_lut: FunLuxColorGradingLut,
    /// Contrast dial. `0` = neutral; positive increases
    /// contrast; negative decreases. Range: [-100, +100].
    pub contrast_q8: i16,
    /// Saturation dial. `0` = neutral; positive increases
    /// saturation; negative desaturates. Range: [-100, +100].
    pub saturation_q8: i16,
    /// Vignette strength. `0` disabled. Range: [0, 100].
    pub vignette_strength_q8: u16,
}

impl FunLuxLookProfile {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_LOOK_SCHEMA_VERSION,
        exposure_ev_compensation_q8: 0,
        tone_map: FunLuxToneMapOperator::AcesFilmic,
        white_point: FunLuxWhitePoint::D65,
        color_grading_lut: FunLuxColorGradingLut::IDENTITY,
        contrast_q8: 0,
        saturation_q8: 0,
        vignette_strength_q8: 0,
    };

    /// Typed cold-default look profile: no look applied.
    /// Equivalent to the renderer's "linear passthrough" path.
    pub const COLD_DEFAULT: Self = Self {
        schema_version: FUN_LUX_LOOK_SCHEMA_VERSION,
        exposure_ev_compensation_q8: 0,
        tone_map: FunLuxToneMapOperator::Linear,
        white_point: FunLuxWhitePoint::D65,
        color_grading_lut: FunLuxColorGradingLut::IDENTITY,
        contrast_q8: 0,
        saturation_q8: 0,
        vignette_strength_q8: 0,
    };

    /// Typed predicate: is this profile the cold-default
    /// (no look applied at all)?
    #[must_use]
    pub const fn is_cold_default(self) -> bool {
        self.exposure_ev_compensation_q8 == 0
            && self.tone_map.is_linear()
            && self.color_grading_lut.is_identity()
            && self.contrast_q8 == 0
            && self.saturation_q8 == 0
            && self.vignette_strength_q8 == 0
    }

    /// Typed predicate: does this profile drive a real look
    /// pipeline (one or more dials are non-neutral)?
    #[must_use]
    pub const fn drives_look_pipeline(self) -> bool {
        !self.is_cold_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_LUX_LOOK_SCHEMA_VERSION, 1);
        assert_eq!(FunLuxToneMapOperator::ALL.len(), 4);
        assert_eq!(FunLuxWhitePoint::ALL.len(), 4);
    }

    #[test]
    fn tone_map_taxonomy_strings_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for op in FunLuxToneMapOperator::ALL {
            assert!(seen.insert(op.as_str()), "duplicate: {}", op.as_str());
        }
        // Linear is the only tone-map operator that preserves HDR linearly.
        for op in FunLuxToneMapOperator::ALL {
            let expect = matches!(op, FunLuxToneMapOperator::Linear);
            assert_eq!(op.is_linear(), expect, "{}", op.as_str());
        }
    }

    #[test]
    fn white_point_correlated_color_temperatures_match_reference() {
        assert_eq!(
            FunLuxWhitePoint::D65.correlated_color_temperature_kelvin(),
            6504
        );
        assert_eq!(
            FunLuxWhitePoint::D60.correlated_color_temperature_kelvin(),
            6000
        );
        assert_eq!(
            FunLuxWhitePoint::D50.correlated_color_temperature_kelvin(),
            5003
        );
        assert_eq!(
            FunLuxWhitePoint::Tungsten.correlated_color_temperature_kelvin(),
            2856,
        );
    }

    #[test]
    fn color_grading_lut_identity_predicate() {
        let lut = FunLuxColorGradingLut::IDENTITY;
        assert!(lut.is_identity());
        let active = FunLuxColorGradingLut::new("fun_lux.look.lut.warm", 50, true);
        assert!(!active.is_identity());
    }

    #[test]
    fn product_default_profile_drives_look_pipeline() {
        let p = FunLuxLookProfile::PRODUCT_DEFAULT;
        // AcesFilmic + D65 + identity LUT: still a look
        // pipeline because AcesFilmic is non-linear.
        assert!(p.drives_look_pipeline());
    }

    #[test]
    fn cold_default_profile_is_neutral_passthrough() {
        let p = FunLuxLookProfile::COLD_DEFAULT;
        assert!(p.is_cold_default());
        assert!(!p.drives_look_pipeline());
    }
}
