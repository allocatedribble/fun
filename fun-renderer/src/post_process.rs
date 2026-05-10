use crate::component_api::{
    BloomSettings, CameraDebugView, ColorGradingSettings, ExposureSettings, HdrOutputSettings,
    SharpeningSettings, TaaSettings, ToneMappingSettings, UpscalerSettings,
};
use crate::extraction::ExtractedPostProcessVolume;
use crate::frame_graph::FrameGraphResourceType;

pub const POST_PROCESS_SCHEMA_VERSION: u16 = 1;
pub const MAX_POST_PROCESS_PASSES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PostProcessPassKind {
    Exposure,
    ToneMapping,
    Bloom,
    ColorGradingLut,
    Sharpening,
    DebugOverlay,
    FinalOutputTransform,
}

impl PostProcessPassKind {
    pub const ALL: [Self; 7] = [
        Self::Exposure,
        Self::Bloom,
        Self::ToneMapping,
        Self::ColorGradingLut,
        Self::Sharpening,
        Self::DebugOverlay,
        Self::FinalOutputTransform,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exposure => "exposure",
            Self::ToneMapping => "tone_mapping",
            Self::Bloom => "bloom",
            Self::ColorGradingLut => "color_grading_lut",
            Self::Sharpening => "sharpening",
            Self::DebugOverlay => "debug_overlay",
            Self::FinalOutputTransform => "final_output_transform",
        }
    }

    #[must_use]
    pub const fn order_key(self) -> u16 {
        match self {
            Self::Exposure => 10,
            Self::Bloom => 20,
            Self::ToneMapping => 30,
            Self::ColorGradingLut => 40,
            Self::Sharpening => 50,
            Self::DebugOverlay => 60,
            Self::FinalOutputTransform => 70,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PostProcessPassEnableReason {
    AlwaysOn,
    AutoExposureEnabled,
    BloomIntensityAboveZero,
    ColorGradingNonIdentity,
    SharpenAlgorithmEnabled,
    CameraDebugViewActive,
    HdrOutputEnabled,
    ToneMappingActive,
}

impl PostProcessPassEnableReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AlwaysOn => "always_on",
            Self::AutoExposureEnabled => "auto_exposure_enabled",
            Self::BloomIntensityAboveZero => "bloom_intensity_above_zero",
            Self::ColorGradingNonIdentity => "color_grading_non_identity",
            Self::SharpenAlgorithmEnabled => "sharpen_algorithm_enabled",
            Self::CameraDebugViewActive => "camera_debug_view_active",
            Self::HdrOutputEnabled => "hdr_output_enabled",
            Self::ToneMappingActive => "tone_mapping_active",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PostProcessPassPlan {
    pub kind: PostProcessPassKind,
    pub order: u16,
    pub enable_reason: PostProcessPassEnableReason,
    pub reads_render_resolution_scene_color: bool,
    pub reads_display_resolution_scene_color: bool,
    pub writes_display_resolution_scene_color: bool,
    pub writes_final_composed_output: bool,
    pub uses_history_buffer: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PostProcessGraphPlan {
    schema_version: u16,
    view_id: crate::component_api::RenderStableId,
    passes: Vec<PostProcessPassPlan>,
    debug_view: CameraDebugView,
    color_grading_uses_lut: bool,
    sharpening_enabled: bool,
    bloom_active: bool,
    auto_exposure_enabled: bool,
    requires_history_buffer: bool,
    requires_motion_vectors: bool,
    hdr_output_enabled: bool,
}

impl PostProcessGraphPlan {
    #[must_use]
    pub fn from_camera_settings(
        view_id: crate::component_api::RenderStableId,
        volume: ExtractedPostProcessVolume,
        debug_view: CameraDebugView,
    ) -> Self {
        let mut passes = Vec::with_capacity(MAX_POST_PROCESS_PASSES);

        let auto_exposure_enabled = volume.exposure.auto_exposure;
        let bloom_active = volume.bloom.intensity > 0.0;
        let color_grading_uses_lut = volume.color_grading.uses_lut();
        let color_grading_non_identity = !volume.color_grading.is_identity();
        let sharpening_enabled = volume.sharpening.is_enabled();
        let hdr_output_enabled = volume.hdr_output.enabled;
        let debug_view_active = !matches!(debug_view, CameraDebugView::None);
        let requires_history_buffer =
            taa_requires_history(volume.taa) || upscaler_requires_history(volume.upscaler);
        let requires_motion_vectors =
            requires_history_buffer || matches!(debug_view, CameraDebugView::MotionVectors);

        if auto_exposure_enabled {
            passes.push(plan_for(
                PostProcessPassKind::Exposure,
                PostProcessPassEnableReason::AutoExposureEnabled,
                requires_history_buffer,
            ));
        }
        if bloom_active {
            passes.push(plan_for(
                PostProcessPassKind::Bloom,
                PostProcessPassEnableReason::BloomIntensityAboveZero,
                requires_history_buffer,
            ));
        }
        if tone_mapping_active(volume.tone_mapping) {
            passes.push(plan_for(
                PostProcessPassKind::ToneMapping,
                PostProcessPassEnableReason::ToneMappingActive,
                requires_history_buffer,
            ));
        }
        if color_grading_non_identity {
            passes.push(plan_for(
                PostProcessPassKind::ColorGradingLut,
                PostProcessPassEnableReason::ColorGradingNonIdentity,
                requires_history_buffer,
            ));
        }
        if sharpening_enabled {
            passes.push(plan_for(
                PostProcessPassKind::Sharpening,
                PostProcessPassEnableReason::SharpenAlgorithmEnabled,
                requires_history_buffer,
            ));
        }
        if debug_view_active {
            passes.push(plan_for(
                PostProcessPassKind::DebugOverlay,
                PostProcessPassEnableReason::CameraDebugViewActive,
                requires_history_buffer,
            ));
        }
        // Final output transform is always present so a renderer-owned post stack
        // is the only path to the swapchain. HDR output flips this pass on regardless
        // of every other setting.
        let final_reason = if hdr_output_enabled {
            PostProcessPassEnableReason::HdrOutputEnabled
        } else {
            PostProcessPassEnableReason::AlwaysOn
        };
        passes.push(plan_for(
            PostProcessPassKind::FinalOutputTransform,
            final_reason,
            requires_history_buffer,
        ));

        passes.sort_by_key(|pass| pass.order);

        Self {
            schema_version: POST_PROCESS_SCHEMA_VERSION,
            view_id,
            passes,
            debug_view,
            color_grading_uses_lut,
            sharpening_enabled,
            bloom_active,
            auto_exposure_enabled,
            requires_history_buffer,
            requires_motion_vectors,
            hdr_output_enabled,
        }
    }

    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }

    #[must_use]
    pub const fn view_id(&self) -> crate::component_api::RenderStableId {
        self.view_id
    }

    #[must_use]
    pub fn passes(&self) -> &[PostProcessPassPlan] {
        &self.passes
    }

    #[must_use]
    pub const fn debug_view(&self) -> CameraDebugView {
        self.debug_view
    }

    #[must_use]
    pub const fn requires_history_buffer(&self) -> bool {
        self.requires_history_buffer
    }

    #[must_use]
    pub const fn requires_motion_vectors(&self) -> bool {
        self.requires_motion_vectors
    }

    #[must_use]
    pub const fn hdr_output_enabled(&self) -> bool {
        self.hdr_output_enabled
    }

    #[must_use]
    pub const fn color_grading_uses_lut(&self) -> bool {
        self.color_grading_uses_lut
    }

    #[must_use]
    pub const fn sharpening_enabled(&self) -> bool {
        self.sharpening_enabled
    }

    #[must_use]
    pub const fn bloom_active(&self) -> bool {
        self.bloom_active
    }

    #[must_use]
    pub const fn auto_exposure_enabled(&self) -> bool {
        self.auto_exposure_enabled
    }

    #[must_use]
    pub fn has_pass(&self, kind: PostProcessPassKind) -> bool {
        self.passes.iter().any(|pass| pass.kind == kind)
    }

    #[must_use]
    pub fn pass_for(&self, kind: PostProcessPassKind) -> Option<PostProcessPassPlan> {
        self.passes.iter().copied().find(|pass| pass.kind == kind)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PostProcessGraphDiagnostics {
    pub schema_version: u16,
    pub pass_count: u16,
    pub bloom_active: bool,
    pub auto_exposure_enabled: bool,
    pub sharpening_enabled: bool,
    pub color_grading_uses_lut: bool,
    pub hdr_output_enabled: bool,
    pub debug_overlay_active: bool,
    pub requires_history_buffer: bool,
    pub requires_motion_vectors: bool,
}

impl PostProcessGraphDiagnostics {
    #[must_use]
    pub fn from_plan(plan: &PostProcessGraphPlan) -> Self {
        Self {
            schema_version: POST_PROCESS_SCHEMA_VERSION,
            pass_count: u16::try_from(plan.passes.len()).unwrap_or(u16::MAX),
            bloom_active: plan.bloom_active,
            auto_exposure_enabled: plan.auto_exposure_enabled,
            sharpening_enabled: plan.sharpening_enabled,
            color_grading_uses_lut: plan.color_grading_uses_lut,
            hdr_output_enabled: plan.hdr_output_enabled,
            debug_overlay_active: plan.has_pass(PostProcessPassKind::DebugOverlay),
            requires_history_buffer: plan.requires_history_buffer,
            requires_motion_vectors: plan.requires_motion_vectors,
        }
    }
}

#[must_use]
pub const fn pass_writes(kind: PostProcessPassKind) -> FrameGraphResourceType {
    match kind {
        PostProcessPassKind::Exposure => FrameGraphResourceType::Exposure,
        PostProcessPassKind::Bloom
        | PostProcessPassKind::ToneMapping
        | PostProcessPassKind::ColorGradingLut
        | PostProcessPassKind::Sharpening
        | PostProcessPassKind::DebugOverlay => FrameGraphResourceType::DisplayResolutionSceneColor,
        PostProcessPassKind::FinalOutputTransform => FrameGraphResourceType::FinalComposedOutput,
    }
}

#[must_use]
pub const fn pass_reads(kind: PostProcessPassKind) -> FrameGraphResourceType {
    match kind {
        PostProcessPassKind::Exposure => FrameGraphResourceType::RenderResolutionSceneColor,
        PostProcessPassKind::Bloom
        | PostProcessPassKind::ToneMapping
        | PostProcessPassKind::ColorGradingLut
        | PostProcessPassKind::Sharpening => FrameGraphResourceType::DisplayResolutionSceneColor,
        PostProcessPassKind::DebugOverlay => FrameGraphResourceType::DisplayResolutionSceneColor,
        PostProcessPassKind::FinalOutputTransform => {
            FrameGraphResourceType::DisplayResolutionSceneColor
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToneMappingOperatorRedacted {
    AcesFitted,
    Reinhard,
    Linear,
    None,
}

impl ToneMappingOperatorRedacted {
    #[must_use]
    pub const fn from_settings(settings: ToneMappingSettings) -> Self {
        match settings.operator {
            crate::component_api::ToneMappingOperator::AcesFitted => Self::AcesFitted,
            crate::component_api::ToneMappingOperator::Reinhard => Self::Reinhard,
            crate::component_api::ToneMappingOperator::Linear => Self::Linear,
        }
    }
}

#[must_use]
pub fn tone_mapping_active(settings: ToneMappingSettings) -> bool {
    !matches!(
        ToneMappingOperatorRedacted::from_settings(settings),
        ToneMappingOperatorRedacted::None
    )
}

#[must_use]
pub const fn taa_requires_history(taa: TaaSettings) -> bool {
    taa.enabled
}

#[must_use]
pub fn upscaler_requires_history(upscaler: UpscalerSettings) -> bool {
    !matches!(
        upscaler.mode,
        crate::component_api::UpscalerMode::Native
            | crate::component_api::UpscalerMode::DebugNearest
    )
}

#[must_use]
pub const fn bloom_intensity_active(bloom: BloomSettings) -> bool {
    bloom.intensity > 0.0
}

#[must_use]
pub fn color_grading_active(grading: ColorGradingSettings) -> bool {
    !grading.is_identity()
}

#[must_use]
pub const fn sharpening_active(sharpening: SharpeningSettings) -> bool {
    sharpening.is_enabled()
}

#[must_use]
pub const fn exposure_pass_required(exposure: ExposureSettings) -> bool {
    exposure.auto_exposure
}

#[must_use]
pub const fn hdr_output_active(hdr: HdrOutputSettings) -> bool {
    hdr.enabled
}

const fn plan_for(
    kind: PostProcessPassKind,
    enable_reason: PostProcessPassEnableReason,
    requires_history_buffer: bool,
) -> PostProcessPassPlan {
    let order = kind.order_key();
    let reads_render_resolution_scene_color = matches!(kind, PostProcessPassKind::Exposure);
    let reads_display_resolution_scene_color = !matches!(kind, PostProcessPassKind::Exposure);
    let writes_display_resolution_scene_color = matches!(
        kind,
        PostProcessPassKind::Bloom
            | PostProcessPassKind::ToneMapping
            | PostProcessPassKind::ColorGradingLut
            | PostProcessPassKind::Sharpening
            | PostProcessPassKind::DebugOverlay
    );
    let writes_final_composed_output = matches!(kind, PostProcessPassKind::FinalOutputTransform);
    let uses_history_buffer =
        matches!(kind, PostProcessPassKind::ToneMapping) && requires_history_buffer;
    PostProcessPassPlan {
        kind,
        order,
        enable_reason,
        reads_render_resolution_scene_color,
        reads_display_resolution_scene_color,
        writes_display_resolution_scene_color,
        writes_final_composed_output,
        uses_history_buffer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component_api::{
        BloomSettings, CameraDebugView, ColorGradingSettings, ExposureSettings, HdrOutputSettings,
        RenderColor, RenderStableId, RenderTextureAssetId, SharpenAlgorithm, SharpeningSettings,
        TaaSettings, ToneMappingOperator, ToneMappingSettings, UpscalerMode, UpscalerSettings,
    };

    fn baseline_volume() -> ExtractedPostProcessVolume {
        ExtractedPostProcessVolume {
            view_id: crate::component_api::RenderViewId::INVALID,
            stable_id: RenderStableId::new(7),
            tone_mapping: ToneMappingSettings::default(),
            exposure: ExposureSettings {
                auto_exposure: false,
                ..ExposureSettings::default()
            },
            color_grading: ColorGradingSettings::default(),
            sharpening: SharpeningSettings::default(),
            bloom: BloomSettings {
                intensity: 0.0,
                ..BloomSettings::default()
            },
            taa: TaaSettings {
                enabled: false,
                ..TaaSettings::default()
            },
            hdr_output: HdrOutputSettings::default(),
            upscaler: UpscalerSettings {
                mode: UpscalerMode::Native,
                ..UpscalerSettings::default()
            },
        }
    }

    #[test]
    fn baseline_camera_emits_only_tonemap_and_final_output_transform() {
        let plan = PostProcessGraphPlan::from_camera_settings(
            RenderStableId::new(1),
            baseline_volume(),
            CameraDebugView::None,
        );

        let kinds: Vec<_> = plan.passes().iter().map(|pass| pass.kind).collect();
        assert_eq!(
            kinds,
            [
                PostProcessPassKind::ToneMapping,
                PostProcessPassKind::FinalOutputTransform,
            ]
        );
        assert!(!plan.requires_history_buffer());
        assert!(!plan.requires_motion_vectors());
    }

    #[test]
    fn auto_exposure_bloom_color_grading_and_sharpening_get_dedicated_passes() {
        let mut volume = baseline_volume();
        volume.exposure.auto_exposure = true;
        volume.bloom = BloomSettings {
            intensity: 0.1,
            ..BloomSettings::default()
        };
        volume.color_grading = ColorGradingSettings {
            saturation: 1.2,
            ..ColorGradingSettings::default()
        };
        volume.sharpening = SharpeningSettings {
            algorithm: SharpenAlgorithm::ContrastAdaptive,
            strength: 0.5,
            denoise: 0.0,
        };

        let plan = PostProcessGraphPlan::from_camera_settings(
            RenderStableId::new(2),
            volume,
            CameraDebugView::None,
        );

        let kinds: Vec<_> = plan.passes().iter().map(|pass| pass.kind).collect();
        assert_eq!(
            kinds,
            [
                PostProcessPassKind::Exposure,
                PostProcessPassKind::Bloom,
                PostProcessPassKind::ToneMapping,
                PostProcessPassKind::ColorGradingLut,
                PostProcessPassKind::Sharpening,
                PostProcessPassKind::FinalOutputTransform,
            ]
        );
    }

    #[test]
    fn debug_view_inserts_overlay_pass_and_camera_debug_motion_vectors_request_motion() {
        let plan = PostProcessGraphPlan::from_camera_settings(
            RenderStableId::new(3),
            baseline_volume(),
            CameraDebugView::MotionVectors,
        );

        assert!(plan.has_pass(PostProcessPassKind::DebugOverlay));
        assert!(plan.requires_motion_vectors());
    }

    #[test]
    fn taa_or_upscaler_enables_history_buffer_request() {
        let mut volume = baseline_volume();
        volume.taa = TaaSettings {
            enabled: true,
            history_weight: 0.9,
            clamp_strength: 1.0,
        };

        let plan = PostProcessGraphPlan::from_camera_settings(
            RenderStableId::new(4),
            volume,
            CameraDebugView::None,
        );

        assert!(plan.requires_history_buffer());
        let tone = plan
            .pass_for(PostProcessPassKind::ToneMapping)
            .expect("tone mapping pass exists");
        assert!(tone.uses_history_buffer);

        let mut upscaler_volume = baseline_volume();
        upscaler_volume.upscaler = UpscalerSettings {
            mode: UpscalerMode::Dlss,
            ..UpscalerSettings::default()
        };
        let upscaler_plan = PostProcessGraphPlan::from_camera_settings(
            RenderStableId::new(5),
            upscaler_volume,
            CameraDebugView::None,
        );
        assert!(upscaler_plan.requires_history_buffer());
    }

    #[test]
    fn hdr_output_uses_dedicated_final_pass_reason() {
        let mut volume = baseline_volume();
        volume.hdr_output = HdrOutputSettings {
            enabled: true,
            max_nits: 1_000.0,
            paper_white_nits: 200.0,
        };

        let plan = PostProcessGraphPlan::from_camera_settings(
            RenderStableId::new(6),
            volume,
            CameraDebugView::None,
        );

        let final_pass = plan
            .pass_for(PostProcessPassKind::FinalOutputTransform)
            .expect("final pass");
        assert_eq!(
            final_pass.enable_reason,
            PostProcessPassEnableReason::HdrOutputEnabled
        );
        assert!(plan.hdr_output_enabled());
    }

    #[test]
    fn color_grading_lut_pass_only_when_lut_or_non_identity_settings() {
        let mut volume = baseline_volume();
        volume.color_grading = ColorGradingSettings {
            lut_id: RenderTextureAssetId::first(11),
            lut_strength: 0.7,
            ..ColorGradingSettings::default()
        };
        let plan = PostProcessGraphPlan::from_camera_settings(
            RenderStableId::new(7),
            volume,
            CameraDebugView::None,
        );
        assert!(plan.has_pass(PostProcessPassKind::ColorGradingLut));
        assert!(plan.color_grading_uses_lut());

        let identity = baseline_volume();
        let identity_plan = PostProcessGraphPlan::from_camera_settings(
            RenderStableId::new(8),
            identity,
            CameraDebugView::None,
        );
        assert!(!identity_plan.has_pass(PostProcessPassKind::ColorGradingLut));
    }

    #[test]
    fn diagnostics_summarize_active_passes_for_telemetry_consumers() {
        let mut volume = baseline_volume();
        volume.bloom.intensity = 0.2;
        volume.exposure.auto_exposure = true;

        let plan = PostProcessGraphPlan::from_camera_settings(
            RenderStableId::new(9),
            volume,
            CameraDebugView::Depth,
        );
        let diagnostics = PostProcessGraphDiagnostics::from_plan(&plan);

        assert_eq!(diagnostics.schema_version, POST_PROCESS_SCHEMA_VERSION);
        assert!(diagnostics.bloom_active);
        assert!(diagnostics.auto_exposure_enabled);
        assert!(diagnostics.debug_overlay_active);
        assert!(!diagnostics.hdr_output_enabled);
        assert_eq!(diagnostics.pass_count, plan.passes().len() as u16);
    }

    #[test]
    fn redacted_tone_mapping_operator_covers_all_supported_operators() {
        for op in [
            ToneMappingOperator::AcesFitted,
            ToneMappingOperator::Reinhard,
            ToneMappingOperator::Linear,
        ] {
            let settings = ToneMappingSettings {
                operator: op,
                exposure_bias: 0.0,
            };
            assert!(tone_mapping_active(settings));
        }
    }

    #[test]
    fn pass_writes_and_reads_route_through_correct_frame_graph_resources() {
        for kind in PostProcessPassKind::ALL {
            let writes = pass_writes(kind);
            let reads = pass_reads(kind);
            match kind {
                PostProcessPassKind::Exposure => {
                    assert_eq!(writes, FrameGraphResourceType::Exposure);
                    assert_eq!(reads, FrameGraphResourceType::RenderResolutionSceneColor);
                }
                PostProcessPassKind::FinalOutputTransform => {
                    assert_eq!(writes, FrameGraphResourceType::FinalComposedOutput);
                }
                _ => {
                    assert_eq!(writes, FrameGraphResourceType::DisplayResolutionSceneColor);
                }
            }
            // Avoid the unused-binding warning when match arms don't bind reads.
            let _ = reads;
        }

        let _ = RenderColor::WHITE;
    }
}
