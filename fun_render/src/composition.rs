#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunRenderCompositionStage {
    WorldRender,
    DepthMotionVectors,
    SolariLighting,
    DlssReconstruction,
    PostProcessing,
    NativeUi,
    HudUi,
    DebugOverlays,
    Present,
}

impl FunRenderCompositionStage {
    #[must_use]
    pub const fn order_key(self) -> i32 {
        match self {
            Self::WorldRender => 10,
            Self::DepthMotionVectors => 20,
            Self::SolariLighting => 30,
            Self::DlssReconstruction => 40,
            Self::PostProcessing => 50,
            Self::NativeUi => 60,
            Self::HudUi => 62,
            Self::DebugOverlays => 70,
            Self::Present => 80,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WorldRender => "world_render",
            Self::DepthMotionVectors => "depth_motion_vectors",
            Self::SolariLighting => "solari_lighting",
            Self::DlssReconstruction => "dlss_reconstruction",
            Self::PostProcessing => "post_processing",
            Self::NativeUi => "native_ui",
            Self::HudUi => "hud_ui",
            Self::DebugOverlays => "debug_overlays",
            Self::Present => "present",
        }
    }

    #[must_use]
    pub const fn feeds_dlss_input_color(self) -> bool {
        matches!(self, Self::WorldRender | Self::SolariLighting)
    }

    #[must_use]
    pub const fn feeds_dlss_depth_or_motion_vectors(self) -> bool {
        matches!(self, Self::DepthMotionVectors)
    }

    #[must_use]
    pub const fn feeds_dlss_rr_guide_buffers(self) -> bool {
        matches!(self, Self::DepthMotionVectors | Self::SolariLighting)
    }

    #[must_use]
    pub const fn feeds_temporal_reconstruction(self) -> bool {
        self.feeds_dlss_input_color()
            || self.feeds_dlss_depth_or_motion_vectors()
            || self.feeds_dlss_rr_guide_buffers()
    }
}

pub const FUN_RENDER_COMPOSITION_ORDER: [FunRenderCompositionStage; 9] = [
    FunRenderCompositionStage::WorldRender,
    FunRenderCompositionStage::DepthMotionVectors,
    FunRenderCompositionStage::SolariLighting,
    FunRenderCompositionStage::DlssReconstruction,
    FunRenderCompositionStage::PostProcessing,
    FunRenderCompositionStage::NativeUi,
    FunRenderCompositionStage::HudUi,
    FunRenderCompositionStage::DebugOverlays,
    FunRenderCompositionStage::Present,
];

pub const FUN_RENDER_NATIVE_UI_STAGE: FunRenderCompositionStage =
    FunRenderCompositionStage::NativeUi;
pub const FUN_RENDER_HUD_UI_STAGE: FunRenderCompositionStage = FunRenderCompositionStage::HudUi;
pub const FUN_RENDER_DEBUG_OVERLAY_STAGE: FunRenderCompositionStage =
    FunRenderCompositionStage::DebugOverlays;

pub const FUN_RENDER_HUD_UI_Z_INDEX: i32 = 900_000;
pub const FUN_RENDER_NATIVE_UI_Z_INDEX: i32 = FUN_RENDER_HUD_UI_Z_INDEX;
pub const FUN_RENDER_DEBUG_OVERLAY_Z_INDEX: i32 = FUN_RENDER_NATIVE_UI_Z_INDEX + 20;

const _: () = {
    assert!(FUN_RENDER_HUD_UI_STAGE.order_key() > FUN_RENDER_NATIVE_UI_STAGE.order_key());
    assert!(FUN_RENDER_DEBUG_OVERLAY_Z_INDEX > FUN_RENDER_HUD_UI_Z_INDEX);
    assert!(FUN_RENDER_DEBUG_OVERLAY_Z_INDEX > FUN_RENDER_NATIVE_UI_Z_INDEX);
    assert!(
        FUN_RENDER_NATIVE_UI_STAGE.order_key()
            > FunRenderCompositionStage::PostProcessing.order_key()
    );
    assert!(FUN_RENDER_HUD_UI_STAGE.order_key() < FUN_RENDER_DEBUG_OVERLAY_STAGE.order_key());
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composition_order_keeps_hud_after_temporal_reconstruction() {
        let mut previous_key = 0;
        for stage in FUN_RENDER_COMPOSITION_ORDER {
            assert!(stage.order_key() > previous_key, "{}", stage.as_str());
            previous_key = stage.order_key();
        }

        assert!(
            FunRenderCompositionStage::HudUi.order_key()
                > FunRenderCompositionStage::DlssReconstruction.order_key()
        );
        assert!(
            FunRenderCompositionStage::HudUi.order_key()
                > FunRenderCompositionStage::PostProcessing.order_key()
        );
        assert!(
            FunRenderCompositionStage::NativeUi.order_key()
                > FunRenderCompositionStage::PostProcessing.order_key()
        );
        assert!(
            FunRenderCompositionStage::DebugOverlays.order_key()
                > FunRenderCompositionStage::HudUi.order_key()
        );
    }

    #[test]
    fn native_ui_does_not_feed_temporal_reconstruction() {
        assert!(!FunRenderCompositionStage::NativeUi.feeds_dlss_input_color());
        assert!(!FunRenderCompositionStage::NativeUi.feeds_dlss_depth_or_motion_vectors());
        assert!(!FunRenderCompositionStage::NativeUi.feeds_dlss_rr_guide_buffers());
        assert!(!FunRenderCompositionStage::NativeUi.feeds_temporal_reconstruction());
    }

    #[test]
    fn hud_ui_does_not_feed_temporal_reconstruction() {
        assert!(!FunRenderCompositionStage::HudUi.feeds_dlss_input_color());
        assert!(!FunRenderCompositionStage::HudUi.feeds_dlss_depth_or_motion_vectors());
        assert!(!FunRenderCompositionStage::HudUi.feeds_dlss_rr_guide_buffers());
        assert!(!FunRenderCompositionStage::HudUi.feeds_temporal_reconstruction());
    }

    #[test]
    fn debug_overlay_layer_sits_above_hud_ui_layer() {
        let native_ui_position = FUN_RENDER_COMPOSITION_ORDER
            .iter()
            .position(|stage| *stage == FUN_RENDER_NATIVE_UI_STAGE)
            .expect("NATIVE_UI UI stage must be in the composition order");
        let hud_position = FUN_RENDER_COMPOSITION_ORDER
            .iter()
            .position(|stage| *stage == FUN_RENDER_HUD_UI_STAGE)
            .expect("HUD UI stage must be in the composition order");
        let debug_position = FUN_RENDER_COMPOSITION_ORDER
            .iter()
            .position(|stage| *stage == FUN_RENDER_DEBUG_OVERLAY_STAGE)
            .expect("debug overlay stage must be in the composition order");

        assert!(hud_position > native_ui_position);
        assert!(debug_position > hud_position);
    }
}
