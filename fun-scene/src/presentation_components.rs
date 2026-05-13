use fun_ecs::Component;

use crate::FunFromTemplate;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SuperResolutionMode {
    Off,
    #[default]
    Auto,
    Dlss,
    Fsr,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameGenerationMode {
    #[default]
    Off,
    Auto,
    Native,
}

#[derive(Debug, Clone, FunFromTemplate, Component)]
pub struct UpscalePolicy {
    pub sr: SuperResolutionMode,
    pub frame_generation: FrameGenerationMode,
    pub hudless_required: bool,
}

impl UpscalePolicy {
    pub const PRODUCT_DEFAULT: Self = Self {
        sr: SuperResolutionMode::Auto,
        frame_generation: FrameGenerationMode::Off,
        hudless_required: true,
    };
}

impl Default for UpscalePolicy {
    fn default() -> Self {
        Self::PRODUCT_DEFAULT
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererQuality {
    Performance,
    #[default]
    Balanced,
    Quality,
    Cinematic,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LatencyPolicy {
    LowLatency,
    #[default]
    Balanced,
    Throughput,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditorViewportMode {
    #[default]
    GameView,
    EditorDocked,
    AssetPreview,
}

#[derive(Debug, Default, Clone, FunFromTemplate, Component)]
pub struct ViewportRenderPolicy {
    pub quality: RendererQuality,
    pub latency: LatencyPolicy,
    pub editor_mode: EditorViewportMode,
}
