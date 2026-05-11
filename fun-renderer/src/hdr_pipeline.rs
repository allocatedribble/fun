//! Pass V2.5 — typed HDR post-path renderer-graph translation.
//!
//! Translates the typed fun-lux Pass 5 HDR policy
//! (`FunLuxHdrPolicy`, `FunLuxBloomSource`,
//! `FunLuxEmissiveIntensityPolicy`,
//! `FunLuxHdrPipelineStage`) into typed renderer
//! frame-graph passes + ordering.  The typed renderer
//! consumes the typed policy at boot to:
//!
//! - Pick the HDR scene-color format
//!   (Rgba16F when alpha is used, R11G11B10F otherwise).
//! - Refuse the `TonemappedColor` bloom source (the typed
//!   Pass 5 rule "bloom is extracted from HDR luminance,
//!   not from already tonemapped color").
//! - Refuse the `ClampToUnitInterval` emissive policy.
//! - Enforce the typed pipeline ordering:
//!     LuxDirectLighting
//!       → LuxVolumetricComposite
//!       → BloomExtractFromHdr
//!       → BloomBlur
//!       → BloomCompositeIntoHdr
//!       → Exposure
//!       → Tonemap
//!       → DisplayOutputEncoding
//!
//! The typed `LuxHdrPipelinePlan` is the typed bundle the
//! typed graph compiler reads to register the typed
//! post-process passes.

use fun_lux::{
    FunLuxBloomSource, FunLuxEmissiveIntensityPolicy, FunLuxHdrFormat, FunLuxHdrPipelineStage,
    FunLuxHdrPolicy,
};

use crate::frame_graph::FrameGraphPassRole;

pub const FUN_RENDERER_HDR_PIPELINE_SCHEMA_VERSION: u16 = 1;

/// Typed Pass V2.5 HDR-pipeline plan.  The typed renderer
/// reads this plan after the typed Lux compile + before
/// the typed post-process pass registration so the typed
/// frame graph carries the right HDR scene-color format,
/// bloom source, and ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxHdrPipelinePlan {
    pub schema_version: u16,
    pub policy: FunLuxHdrPolicy,
    /// Typed flag — does the typed scene need an alpha
    /// channel in the HDR target this frame?  The typed
    /// `select_scene_color_format` predicate consumes this
    /// to pick Rgba16F vs R11G11B10F.
    pub scene_uses_alpha: bool,
}

impl LuxHdrPipelinePlan {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_RENDERER_HDR_PIPELINE_SCHEMA_VERSION,
        policy: FunLuxHdrPolicy::PRODUCT_DEFAULT,
        scene_uses_alpha: false,
    };

    /// Typed builder: derive a typed plan from a typed
    /// fun-lux HDR policy + the typed scene-alpha flag.
    #[must_use]
    pub const fn from_policy(policy: FunLuxHdrPolicy, scene_uses_alpha: bool) -> Self {
        Self {
            schema_version: FUN_RENDERER_HDR_PIPELINE_SCHEMA_VERSION,
            policy,
            scene_uses_alpha,
        }
    }

    /// Typed HDR scene-color format selector.  Returns
    /// `Rgba16F` when the typed scene uses alpha,
    /// `R11G11B10F` otherwise — matches the typed Pass 5
    /// rule encoded in
    /// `FunLuxHdrPolicy::PRODUCT_DEFAULT` (scene_color_format_with_alpha
    /// = Rgba16F, scene_color_format_without_alpha =
    /// R11G11B10F).
    #[must_use]
    pub const fn select_scene_color_format(&self) -> FunLuxHdrFormat {
        if self.scene_uses_alpha {
            self.policy.scene_color_format_with_alpha
        } else {
            self.policy.scene_color_format_without_alpha
        }
    }

    /// Typed predicate: rule — HDR scene color uses an HDR
    /// format (`Rgba16F` or `R11G11B10F` or `Rgba32F`),
    /// never `Rgba8Unorm` etc.  The typed `FunLuxHdrFormat`
    /// taxonomy intentionally only contains HDR formats,
    /// so the typed selector always returns an HDR-capable
    /// format.  The predicate exposes the contract.
    #[must_use]
    pub const fn hdr_scene_color_is_hdr_format(&self) -> bool {
        self.select_scene_color_format().is_hdr_capable()
    }

    /// Typed predicate: rule — bloom is extracted from
    /// HDR luminance, not from already-tonemapped color
    /// (Pass 5).
    #[must_use]
    pub const fn bloom_reads_hdr_luminance(&self) -> bool {
        matches!(self.policy.bloom_source, FunLuxBloomSource::HdrLuminance)
    }

    /// Typed predicate: rule — emissive policy allows
    /// values above `1.0` (Pass 5).
    #[must_use]
    pub const fn emissive_policy_allows_above_one(&self) -> bool {
        matches!(
            self.policy.emissive_intensity_policy,
            FunLuxEmissiveIntensityPolicy::AllowAboveUnit,
        )
    }
}

/// Typed Pass V2.5 frame-graph role ordering helper.
/// Returns the typed order key for a typed Lux / HDR post
/// role.  The typed order encodes the user spec ordering:
///
///   LuxDirectLighting
///     → LuxVolumetricComposite
///     → BloomExtractFromHdr (PostProcessBloomPrefilter)
///     → BloomBlur (PostProcessBloomDownsample / Upsample)
///     → BloomCompositeIntoHdr (PostProcessBloomComposite)
///     → Exposure (PostProcessExposureHistogram + Adapt)
///     → Tonemap (PostProcessToneMapping)
///     → DisplayOutputEncoding (PostProcessFinalOutputTransform)
///
/// Mirrors `FunLuxHdrPipelineStage::order_key` for the
/// typed Pass 5 / V2.5 contract.  Returns `None` for typed
/// roles that aren't in the typed Lux / HDR post lane.
#[must_use]
pub const fn hdr_pipeline_order_key(role: FrameGraphPassRole) -> Option<u16> {
    match role {
        // Lux lighting accumulates into HDR (Pass 5 stage
        // 100 — HdrSceneColorAccumulation).
        FrameGraphPassRole::LuxDirectLighting => Some(100),
        // Volumetric composite into HDR (typed Pass 8
        // `VolumetricCompositeIntoHdr` = order_key 900,
        // but here we collapse onto stage 150 since it
        // happens between lighting and bloom).
        FrameGraphPassRole::LuxVolumetricComposite => Some(150),
        // Bloom extract (Pass 5 stage 200).
        FrameGraphPassRole::PostProcessBloomPrefilter => Some(200),
        // Bloom blur (downsample then upsample), Pass 5
        // stage 210.
        FrameGraphPassRole::PostProcessBloomDownsample => Some(210),
        FrameGraphPassRole::PostProcessBloomUpsample => Some(220),
        // Bloom composite into HDR (Pass 5 stage 300 —
        // HdrComposite).
        FrameGraphPassRole::PostProcessBloomComposite => Some(300),
        // Exposure (Pass 5 stage 400).  Histogram before
        // adapt.
        FrameGraphPassRole::PostProcessExposureHistogram => Some(390),
        FrameGraphPassRole::PostProcessExposureAdapt => Some(400),
        // Tonemap (Pass 5 stage 500).
        FrameGraphPassRole::PostProcessToneMapping => Some(500),
        // Display output transform (Pass 5 stage 600 —
        // DisplayOutputEncoding).
        FrameGraphPassRole::PostProcessFinalOutputTransform => Some(600),
        // Coarse umbrellas (kept for legacy compatibility)
        // share their typed granular peer's key.
        FrameGraphPassRole::PostProcessExposure => Some(400),
        FrameGraphPassRole::PostProcessBloom => Some(210),
        _ => None,
    }
}

/// Pass V2.5 typed predicate — tonemap runs after bloom +
/// HDR composite + volumetric composite.
#[must_use]
pub const fn tonemap_runs_after_bloom_and_hdr_composite() -> bool {
    let tonemap = match hdr_pipeline_order_key(FrameGraphPassRole::PostProcessToneMapping) {
        Some(k) => k,
        None => return false,
    };
    let bloom_composite =
        match hdr_pipeline_order_key(FrameGraphPassRole::PostProcessBloomComposite) {
            Some(k) => k,
            None => return false,
        };
    let bloom_upsample =
        match hdr_pipeline_order_key(FrameGraphPassRole::PostProcessBloomUpsample) {
            Some(k) => k,
            None => return false,
        };
    let volumetric =
        match hdr_pipeline_order_key(FrameGraphPassRole::LuxVolumetricComposite) {
            Some(k) => k,
            None => return false,
        };
    tonemap > bloom_composite && tonemap > bloom_upsample && tonemap > volumetric
}

/// Pass V2.5 typed predicate — final output runs after
/// tonemap.
#[must_use]
pub const fn final_output_runs_after_tonemap() -> bool {
    let tonemap = match hdr_pipeline_order_key(FrameGraphPassRole::PostProcessToneMapping) {
        Some(k) => k,
        None => return false,
    };
    let final_output =
        match hdr_pipeline_order_key(FrameGraphPassRole::PostProcessFinalOutputTransform) {
            Some(k) => k,
            None => return false,
        };
    final_output > tonemap
}

/// Typed Pass V2.5 debug section.  Returns the typed
/// "HDR Pipeline" multi-line section the typed debug
/// artifact carries when V2.5 is wired (acceptance rule
/// #3 — debug artifact shows HDR path).
#[must_use]
pub fn hdr_pipeline_debug_section(plan: &LuxHdrPipelinePlan) -> String {
    use core::fmt::Write as _;
    let mut content = String::new();
    let _ = writeln!(content, "HDR Pipeline");
    let _ = writeln!(content, "------------");
    let _ = writeln!(
        content,
        "scene_color_format: {}",
        plan.select_scene_color_format().as_str(),
    );
    let _ = writeln!(content, "scene_uses_alpha: {}", plan.scene_uses_alpha);
    let _ = writeln!(
        content,
        "bloom_source: {}",
        plan.policy.bloom_source.as_str(),
    );
    let _ = writeln!(
        content,
        "emissive_policy: {}",
        plan.policy.emissive_intensity_policy.as_str(),
    );
    let _ = writeln!(content, "stage_order:");
    for stage in FunLuxHdrPipelineStage::ALL {
        let _ = writeln!(content, "  {} = {}", stage.as_str(), stage.order_key());
    }
    content
}

// ============================================================================
// Tests — Pass V2.5 typed acceptance
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Pass V2.5 acceptance — HDR scene color is an HDR
    /// format (always: every typed `FunLuxHdrFormat` is
    /// HDR-capable; the typed selector picks between
    /// Rgba16F / R11G11B10F based on the typed alpha
    /// requirement).
    #[test]
    fn hdr_scene_color_is_hdr_format() {
        let with_alpha = LuxHdrPipelinePlan::from_policy(FunLuxHdrPolicy::PRODUCT_DEFAULT, true);
        assert!(with_alpha.hdr_scene_color_is_hdr_format());
        assert_eq!(
            with_alpha.select_scene_color_format(),
            FunLuxHdrFormat::Rgba16F,
        );

        let without_alpha =
            LuxHdrPipelinePlan::from_policy(FunLuxHdrPolicy::PRODUCT_DEFAULT, false);
        assert!(without_alpha.hdr_scene_color_is_hdr_format());
        assert_eq!(
            without_alpha.select_scene_color_format(),
            FunLuxHdrFormat::R11G11B10F,
        );
    }

    /// Pass V2.5 acceptance — bloom reads HDR luminance,
    /// not tonemapped color.
    #[test]
    fn bloom_reads_hdr_luminance_not_tonemapped_color() {
        let plan = LuxHdrPipelinePlan::PRODUCT_DEFAULT;
        assert!(plan.bloom_reads_hdr_luminance());
        // Inverse: a typed plan with TonemappedColor bloom
        // source flips the predicate.
        let mut prohibited = FunLuxHdrPolicy::PRODUCT_DEFAULT;
        prohibited.bloom_source = FunLuxBloomSource::TonemappedColor;
        let plan = LuxHdrPipelinePlan::from_policy(prohibited, false);
        assert!(!plan.bloom_reads_hdr_luminance());
    }

    /// Pass V2.5 acceptance — emissive policy allows
    /// values above `1.0`.
    #[test]
    fn emissive_policy_allows_above_one() {
        let plan = LuxHdrPipelinePlan::PRODUCT_DEFAULT;
        assert!(plan.emissive_policy_allows_above_one());
        // Inverse: a typed plan with the prohibited
        // ClampToUnitInterval policy flips the predicate.
        let mut prohibited = FunLuxHdrPolicy::PRODUCT_DEFAULT;
        prohibited.emissive_intensity_policy = FunLuxEmissiveIntensityPolicy::ClampToUnitInterval;
        let plan = LuxHdrPipelinePlan::from_policy(prohibited, false);
        assert!(!plan.emissive_policy_allows_above_one());
    }

    /// Pass V2.5 acceptance — tonemap runs after bloom and
    /// HDR composite.
    #[test]
    fn tonemap_runs_after_bloom_and_hdr_composite_predicate_holds() {
        assert!(tonemap_runs_after_bloom_and_hdr_composite());
        // Tonemap (500) > BloomComposite (300) > BloomUpsample (220) > VolumetricComposite (150).
        assert!(
            hdr_pipeline_order_key(FrameGraphPassRole::PostProcessToneMapping)
                > hdr_pipeline_order_key(FrameGraphPassRole::PostProcessBloomComposite),
        );
        assert!(
            hdr_pipeline_order_key(FrameGraphPassRole::PostProcessBloomComposite)
                > hdr_pipeline_order_key(FrameGraphPassRole::PostProcessBloomUpsample),
        );
        assert!(
            hdr_pipeline_order_key(FrameGraphPassRole::PostProcessBloomUpsample)
                > hdr_pipeline_order_key(FrameGraphPassRole::PostProcessBloomDownsample),
        );
    }

    /// Pass V2.5 acceptance — final output runs after
    /// tonemap.
    #[test]
    fn final_output_runs_after_tonemap_predicate_holds() {
        assert!(final_output_runs_after_tonemap());
        assert!(
            hdr_pipeline_order_key(FrameGraphPassRole::PostProcessFinalOutputTransform)
                > hdr_pipeline_order_key(FrameGraphPassRole::PostProcessToneMapping),
        );
    }

    /// Pass V2.5 acceptance — typed user-spec pipeline
    /// ordering holds end-to-end:
    ///   LuxDirectLighting (100)
    ///     → LuxVolumetricComposite (150)
    ///     → BloomExtract / Prefilter (200)
    ///     → BloomDownsample (210)
    ///     → BloomUpsample (220)
    ///     → BloomComposite (300)
    ///     → ExposureHistogram (390)
    ///     → ExposureAdapt (400)
    ///     → Tonemap (500)
    ///     → FinalOutputTransform (600)
    #[test]
    fn full_pipeline_ordering_is_monotonic() {
        let chain = [
            FrameGraphPassRole::LuxDirectLighting,
            FrameGraphPassRole::LuxVolumetricComposite,
            FrameGraphPassRole::PostProcessBloomPrefilter,
            FrameGraphPassRole::PostProcessBloomDownsample,
            FrameGraphPassRole::PostProcessBloomUpsample,
            FrameGraphPassRole::PostProcessBloomComposite,
            FrameGraphPassRole::PostProcessExposureHistogram,
            FrameGraphPassRole::PostProcessExposureAdapt,
            FrameGraphPassRole::PostProcessToneMapping,
            FrameGraphPassRole::PostProcessFinalOutputTransform,
        ];
        let mut prev = 0u16;
        for role in chain {
            let k = hdr_pipeline_order_key(role)
                .expect("every typed Pass V2.5 role has an order key");
            assert!(
                k >= prev,
                "non-monotonic order: {:?} key {} < prev {}",
                role,
                k,
                prev,
            );
            prev = k;
        }
    }

    /// Pass V2.5 acceptance #3 — debug artifact shows the
    /// typed HDR path.
    #[test]
    fn hdr_pipeline_debug_section_emits_typed_layout() {
        let plan = LuxHdrPipelinePlan::PRODUCT_DEFAULT;
        let section = hdr_pipeline_debug_section(&plan);
        assert!(section.contains("HDR Pipeline"));
        assert!(section.contains("scene_color_format:"));
        assert!(section.contains("bloom_source:"));
        assert!(section.contains("emissive_policy:"));
        assert!(section.contains("stage_order:"));
        assert!(section.contains("hdr_scene_color_accumulation"));
        assert!(section.contains("bloom_extract_from_hdr"));
        assert!(section.contains("tonemap"));
        assert!(section.contains("display_output_encoding"));
    }
}
