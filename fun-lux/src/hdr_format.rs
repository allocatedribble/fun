//! Pass 5 — typed HDR scene-color format + pipeline ordering.
//!
//! Encodes the user's "5.1 Internal HDR color" rule: scene
//! color renders into typed HDR targets (RGBA16F preferred,
//! R11G11B10F for performance where alpha is unused), all
//! lighting accumulates into HDR, emissives are allowed to
//! exceed `1.0` naturally, bloom is extracted from HDR
//! luminance (not from already-tonemapped color), and the
//! final output is in display format only AFTER tonemap.
//!
//! The typed `FunLuxHdrPipelineStage` enum + `order_key`
//! predicate enforce the pipeline ordering at the
//! type-system layer.

pub const FUN_LUX_HDR_FORMAT_SCHEMA_VERSION: u16 = 1;
pub const FUN_LUX_HDR_FORMAT_COUNT: usize = 3;
pub const FUN_LUX_HDR_PIPELINE_STAGE_COUNT: usize = 7;
pub const FUN_LUX_BLOOM_SOURCE_COUNT: usize = 2;
pub const FUN_LUX_EMISSIVE_INTENSITY_POLICY_COUNT: usize = 2;

// ============================================================================
// Section 1 — Typed HDR format
// ============================================================================

/// Typed HDR scene-color format. The renderer maps the typed
/// variant to a concrete `wgpu::TextureFormat`; the typed
/// value here names the format without exposing any backend
/// handle.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunLuxHdrFormat {
    /// `Rgba16Float` — HDR-capable + preserves alpha. The
    /// typed product default for scene color targets that
    /// carry transparency.
    #[default]
    Rgba16F,
    /// `R11G11B10Float` — HDR-capable, no alpha, smaller
    /// memory footprint. Preferred when alpha is unused.
    R11G11B10F,
    /// `Rgba32Float` — full-precision HDR. Reserved for
    /// reference / debug captures; production rarely needs
    /// 32-bit float scene color.
    Rgba32F,
}

impl FunLuxHdrFormat {
    pub const ALL: [Self; FUN_LUX_HDR_FORMAT_COUNT] =
        [Self::Rgba16F, Self::R11G11B10F, Self::Rgba32F];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rgba16F => "rgba16f",
            Self::R11G11B10F => "r11g11b10f",
            Self::Rgba32F => "rgba32f",
        }
    }

    /// Typed predicate: is this format HDR-capable? (All
    /// three Pass 5 formats are HDR by definition.)
    #[must_use]
    pub const fn is_hdr_capable(self) -> bool {
        true
    }

    /// Typed predicate: does this format carry an alpha
    /// channel?
    #[must_use]
    pub const fn preserves_alpha(self) -> bool {
        matches!(self, Self::Rgba16F | Self::Rgba32F)
    }

    /// Typed byte-size per pixel.
    #[must_use]
    pub const fn bytes_per_pixel(self) -> u8 {
        match self {
            Self::Rgba16F => 8,
            Self::R11G11B10F => 4,
            Self::Rgba32F => 16,
        }
    }

    /// Pass 5 typed predicate: the typed product-default
    /// scene color format that the renderer should pick
    /// when no override is set. Returns `true` for
    /// `Rgba16F` only.
    #[must_use]
    pub const fn is_product_default(self) -> bool {
        matches!(self, Self::Rgba16F)
    }

    /// Pass 5 typed predicate: this format is acceptable
    /// for HDR scene color accumulation (every Pass 5
    /// format is — non-HDR formats would fail this
    /// predicate at the typed contract layer).
    #[must_use]
    pub const fn is_acceptable_for_hdr_accumulation(self) -> bool {
        self.is_hdr_capable()
    }
}

// ============================================================================
// Section 2 — Typed HDR scene-color target descriptor
// ============================================================================

/// Typed HDR scene-color target. Names the renderer
/// allocation without exposing any wgpu handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunLuxHdrSceneColorTarget {
    pub schema_version: u16,
    pub stable_id: &'static str,
    pub format: FunLuxHdrFormat,
    pub width: u32,
    pub height: u32,
    pub mip_count: u8,
    pub sample_count: u8,
    pub is_alpha_used: bool,
    pub is_storage_writable: bool,
    pub is_sampleable: bool,
}

impl FunLuxHdrSceneColorTarget {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_HDR_FORMAT_SCHEMA_VERSION,
        stable_id: "fun_lux.hdr.scene_color_target",
        format: FunLuxHdrFormat::Rgba16F,
        width: 1920,
        height: 1080,
        mip_count: 1,
        sample_count: 1,
        is_alpha_used: true,
        is_storage_writable: false,
        is_sampleable: true,
    };

    /// Typed builder: pick the typed format based on whether
    /// alpha is used. `Rgba16F` when alpha is needed;
    /// `R11G11B10F` otherwise (smaller memory footprint).
    #[must_use]
    pub const fn pick_format_for_alpha_usage(is_alpha_used: bool) -> FunLuxHdrFormat {
        if is_alpha_used {
            FunLuxHdrFormat::Rgba16F
        } else {
            FunLuxHdrFormat::R11G11B10F
        }
    }

    /// Typed predicate: does this target use a format
    /// compatible with its typed `is_alpha_used` flag?
    #[must_use]
    pub const fn format_matches_alpha_usage(&self) -> bool {
        if self.is_alpha_used {
            self.format.preserves_alpha()
        } else {
            true
        }
    }

    /// Typed predicate: is this target ready for HDR
    /// accumulation? Requires HDR format + non-zero extent.
    #[must_use]
    pub const fn ready_for_hdr_accumulation(&self) -> bool {
        self.format.is_acceptable_for_hdr_accumulation() && self.width > 0 && self.height > 0
    }
}

// ============================================================================
// Section 3 — Typed bloom source policy
// ============================================================================

/// Typed bloom source. The user spec demands bloom be
/// extracted from HDR luminance (NOT from already-tonemapped
/// color); the typed predicate
/// [`FunLuxBloomSource::is_production_acceptable`] returns
/// `false` for `TonemappedColor`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunLuxBloomSource {
    /// **PROHIBITED for production.** Reads bloom from
    /// already-tonemapped LDR color; loses HDR-induced
    /// natural bloom from values above 1.0.
    TonemappedColor,
    /// Typed product-default: bloom extracted from HDR
    /// scene-color luminance before tonemap.
    #[default]
    HdrLuminance,
}

impl FunLuxBloomSource {
    pub const ALL: [Self; FUN_LUX_BLOOM_SOURCE_COUNT] = [Self::TonemappedColor, Self::HdrLuminance];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TonemappedColor => "tonemapped_color",
            Self::HdrLuminance => "hdr_luminance",
        }
    }

    /// Pass 5 typed predicate: "Bloom should be extracted
    /// from HDR luminance, not from already tonemapped
    /// color." Returns `true` only for `HdrLuminance`.
    #[must_use]
    pub const fn is_production_acceptable(self) -> bool {
        matches!(self, Self::HdrLuminance)
    }
}

// ============================================================================
// Section 4 — Typed emissive intensity policy
// ============================================================================

/// Typed emissive intensity policy. The user spec: "Emissives
/// should exceed 1.0 naturally." The typed contract refuses
/// a clamped-to-unit policy in production.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunLuxEmissiveIntensityPolicy {
    /// **PROHIBITED for production.** Clamps emissive
    /// intensity to `[0, 1]` — loses HDR overflow that
    /// drives natural bloom.
    ClampToUnitInterval,
    /// Typed product-default: emissive intensity above
    /// `1.0` is allowed (and necessary for HDR bloom +
    /// auto-exposure to behave correctly).
    #[default]
    AllowAboveUnit,
}

impl FunLuxEmissiveIntensityPolicy {
    pub const ALL: [Self; FUN_LUX_EMISSIVE_INTENSITY_POLICY_COUNT] =
        [Self::ClampToUnitInterval, Self::AllowAboveUnit];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClampToUnitInterval => "clamp_to_unit_interval",
            Self::AllowAboveUnit => "allow_above_unit",
        }
    }

    /// Pass 5 typed predicate: "Emissives should exceed 1.0
    /// naturally."
    #[must_use]
    pub const fn is_production_acceptable(self) -> bool {
        matches!(self, Self::AllowAboveUnit)
    }
}

// ============================================================================
// Section 5 — Typed HDR pipeline stage ordering
// ============================================================================

/// Typed HDR pipeline stage. The renderer dispatches each
/// stage in order; the typed `order_key` enforces the
/// pipeline ordering at the type-system layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunLuxHdrPipelineStage {
    HdrSceneColorAccumulation,
    BloomExtractFromHdr,
    BloomBlur,
    HdrComposite,
    Exposure,
    Tonemap,
    DisplayOutputEncoding,
}

impl FunLuxHdrPipelineStage {
    pub const ALL: [Self; FUN_LUX_HDR_PIPELINE_STAGE_COUNT] = [
        Self::HdrSceneColorAccumulation,
        Self::BloomExtractFromHdr,
        Self::BloomBlur,
        Self::HdrComposite,
        Self::Exposure,
        Self::Tonemap,
        Self::DisplayOutputEncoding,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HdrSceneColorAccumulation => "hdr_scene_color_accumulation",
            Self::BloomExtractFromHdr => "bloom_extract_from_hdr",
            Self::BloomBlur => "bloom_blur",
            Self::HdrComposite => "hdr_composite",
            Self::Exposure => "exposure",
            Self::Tonemap => "tonemap",
            Self::DisplayOutputEncoding => "display_output_encoding",
        }
    }

    /// Typed order key (100..600 range). The Pass 5 spec
    /// requires:
    ///
    /// - Lighting accumulates into HDR (stage 100).
    /// - Bloom is extracted from HDR pre-tonemap (stages
    ///   200/210).
    /// - HDR composite happens before tonemap (stage 300).
    /// - Exposure runs before tonemap (stage 400).
    /// - Tonemap runs before display encoding (stages
    ///   500/600).
    #[must_use]
    pub const fn order_key(self) -> u16 {
        match self {
            Self::HdrSceneColorAccumulation => 100,
            Self::BloomExtractFromHdr => 200,
            Self::BloomBlur => 210,
            Self::HdrComposite => 300,
            Self::Exposure => 400,
            Self::Tonemap => 500,
            Self::DisplayOutputEncoding => 600,
        }
    }

    /// Typed predicate: does this stage operate on HDR
    /// values (pre-tonemap)?
    #[must_use]
    pub const fn is_hdr_pre_tonemap(self) -> bool {
        self.order_key() < Self::Tonemap.order_key()
    }

    /// Typed predicate: does this stage operate on LDR /
    /// display-format values (post-tonemap)?
    #[must_use]
    pub const fn is_ldr_post_tonemap(self) -> bool {
        self.order_key() > Self::Tonemap.order_key()
    }
}

/// Pass 5 typed contract: "Keep tonemapping after bloom and
/// HDR composite."
#[must_use]
pub const fn tonemap_runs_after_bloom_and_composite() -> bool {
    let tonemap = FunLuxHdrPipelineStage::Tonemap.order_key();
    let bloom_blur = FunLuxHdrPipelineStage::BloomBlur.order_key();
    let composite = FunLuxHdrPipelineStage::HdrComposite.order_key();
    let exposure = FunLuxHdrPipelineStage::Exposure.order_key();
    tonemap > bloom_blur && tonemap > composite && tonemap > exposure
}

/// Pass 5 typed contract: tonemap runs before display
/// encoding.
#[must_use]
pub const fn tonemap_runs_before_display_encoding() -> bool {
    FunLuxHdrPipelineStage::Tonemap.order_key()
        < FunLuxHdrPipelineStage::DisplayOutputEncoding.order_key()
}

// ============================================================================
// Section 6 — Typed HDR policy bundle
// ============================================================================

/// Typed bundle of every Pass 5 HDR-format policy. The
/// renderer reads this bundle at boot to pick formats +
/// allocate targets + enforce bloom-source rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunLuxHdrPolicy {
    pub schema_version: u16,
    pub scene_color_format_with_alpha: FunLuxHdrFormat,
    pub scene_color_format_without_alpha: FunLuxHdrFormat,
    pub bloom_source: FunLuxBloomSource,
    pub emissive_intensity_policy: FunLuxEmissiveIntensityPolicy,
}

impl FunLuxHdrPolicy {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_HDR_FORMAT_SCHEMA_VERSION,
        scene_color_format_with_alpha: FunLuxHdrFormat::Rgba16F,
        scene_color_format_without_alpha: FunLuxHdrFormat::R11G11B10F,
        bloom_source: FunLuxBloomSource::HdrLuminance,
        emissive_intensity_policy: FunLuxEmissiveIntensityPolicy::AllowAboveUnit,
    };

    /// Pass 5 typed predicate: every typed sub-policy is
    /// production-acceptable.
    #[must_use]
    pub const fn is_production_acceptable(&self) -> bool {
        self.scene_color_format_with_alpha
            .is_acceptable_for_hdr_accumulation()
            && self
                .scene_color_format_without_alpha
                .is_acceptable_for_hdr_accumulation()
            && self.bloom_source.is_production_acceptable()
            && self.emissive_intensity_policy.is_production_acceptable()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_LUX_HDR_FORMAT_SCHEMA_VERSION, 1);
        assert_eq!(FUN_LUX_HDR_FORMAT_COUNT, 3);
        assert_eq!(FunLuxHdrFormat::ALL.len(), FUN_LUX_HDR_FORMAT_COUNT);
        assert_eq!(FUN_LUX_HDR_PIPELINE_STAGE_COUNT, 7);
        assert_eq!(
            FunLuxHdrPipelineStage::ALL.len(),
            FUN_LUX_HDR_PIPELINE_STAGE_COUNT,
        );
    }

    #[test]
    fn hdr_format_taxonomy_strings_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for f in FunLuxHdrFormat::ALL {
            assert!(seen.insert(f.as_str()), "duplicate: {}", f.as_str());
        }
    }

    #[test]
    fn hdr_format_predicates() {
        assert!(FunLuxHdrFormat::Rgba16F.preserves_alpha());
        assert!(!FunLuxHdrFormat::R11G11B10F.preserves_alpha());
        assert!(FunLuxHdrFormat::Rgba32F.preserves_alpha());
        assert_eq!(FunLuxHdrFormat::Rgba16F.bytes_per_pixel(), 8);
        assert_eq!(FunLuxHdrFormat::R11G11B10F.bytes_per_pixel(), 4);
        assert_eq!(FunLuxHdrFormat::Rgba32F.bytes_per_pixel(), 16);
        assert!(FunLuxHdrFormat::Rgba16F.is_product_default());
        assert!(!FunLuxHdrFormat::Rgba32F.is_product_default());
    }

    #[test]
    fn pick_format_for_alpha_usage() {
        assert_eq!(
            FunLuxHdrSceneColorTarget::pick_format_for_alpha_usage(true),
            FunLuxHdrFormat::Rgba16F,
        );
        assert_eq!(
            FunLuxHdrSceneColorTarget::pick_format_for_alpha_usage(false),
            FunLuxHdrFormat::R11G11B10F,
        );
    }

    #[test]
    fn scene_color_target_default_is_ready() {
        let target = FunLuxHdrSceneColorTarget::PRODUCT_DEFAULT;
        assert!(target.ready_for_hdr_accumulation());
        assert!(target.format_matches_alpha_usage());
    }

    /// Pass 5 acceptance: bloom MUST be extracted from HDR
    /// luminance, not tonemapped color.
    #[test]
    fn bloom_source_tonemapped_is_rejected_for_production() {
        assert!(!FunLuxBloomSource::TonemappedColor.is_production_acceptable());
        assert!(FunLuxBloomSource::HdrLuminance.is_production_acceptable());
    }

    /// Pass 5 acceptance: emissives must exceed 1.0
    /// naturally.
    #[test]
    fn emissive_intensity_clamp_to_unit_is_rejected() {
        assert!(!FunLuxEmissiveIntensityPolicy::ClampToUnitInterval.is_production_acceptable());
        assert!(FunLuxEmissiveIntensityPolicy::AllowAboveUnit.is_production_acceptable());
    }

    /// Pass 5 acceptance: typed pipeline ordering rule.
    #[test]
    fn tonemap_runs_after_bloom_composite_exposure() {
        assert!(tonemap_runs_after_bloom_and_composite());
        assert!(tonemap_runs_before_display_encoding());
    }

    #[test]
    fn pipeline_stage_pre_post_tonemap_predicates() {
        for stage in [
            FunLuxHdrPipelineStage::HdrSceneColorAccumulation,
            FunLuxHdrPipelineStage::BloomExtractFromHdr,
            FunLuxHdrPipelineStage::BloomBlur,
            FunLuxHdrPipelineStage::HdrComposite,
            FunLuxHdrPipelineStage::Exposure,
        ] {
            assert!(stage.is_hdr_pre_tonemap(), "{}", stage.as_str());
            assert!(!stage.is_ldr_post_tonemap(), "{}", stage.as_str());
        }
        assert!(!FunLuxHdrPipelineStage::Tonemap.is_hdr_pre_tonemap());
        assert!(!FunLuxHdrPipelineStage::Tonemap.is_ldr_post_tonemap());
        assert!(FunLuxHdrPipelineStage::DisplayOutputEncoding.is_ldr_post_tonemap());
    }

    #[test]
    fn pipeline_stage_order_is_monotonic() {
        let mut last = 0u16;
        for stage in FunLuxHdrPipelineStage::ALL {
            let k = stage.order_key();
            assert!(k > last, "{}", stage.as_str());
            last = k;
        }
    }

    /// Pass 5 acceptance: typed HDR policy bundle.
    #[test]
    fn product_default_hdr_policy_is_acceptable() {
        let policy = FunLuxHdrPolicy::PRODUCT_DEFAULT;
        assert!(policy.is_production_acceptable());
    }

    #[test]
    fn hdr_policy_rejects_tonemapped_bloom() {
        let mut policy = FunLuxHdrPolicy::PRODUCT_DEFAULT;
        policy.bloom_source = FunLuxBloomSource::TonemappedColor;
        assert!(!policy.is_production_acceptable());
    }

    #[test]
    fn hdr_policy_rejects_clamped_emissive() {
        let mut policy = FunLuxHdrPolicy::PRODUCT_DEFAULT;
        policy.emissive_intensity_policy = FunLuxEmissiveIntensityPolicy::ClampToUnitInterval;
        assert!(!policy.is_production_acceptable());
    }
}
