use std::{fmt::Write as _, io, path::Path};

use serde::{Deserialize, Serialize};

use crate::{
    FunRendererBackend, FunRendererRuntimeBackend, RendererFeatureToggles,
    frame_generation::FrameGenerationMode,
    upscaling::{UpscalerCapabilities, UpscalerMode},
};

pub const RENDERER_SETTINGS_SCHEMA: &str = "fun.renderer.settings.v1";
pub const RENDERER_SETTINGS_SCHEMA_VERSION: u16 = 1;
pub const RENDERER_SETTINGS_CAPABILITY_ARTIFACT_ENV: &str =
    "FUN_RENDERER_SETTINGS_CAPABILITY_ARTIFACT";
pub const RENDERER_QUALITY_TIER_COUNT: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RendererQualityTier {
    Baseline,
    Hybrid,
    High,
    RtAssisted,
    VendorEnhanced,
    Experimental,
}

impl RendererQualityTier {
    pub const ORDER: [Self; RENDERER_QUALITY_TIER_COUNT] = [
        Self::Baseline,
        Self::Hybrid,
        Self::High,
        Self::RtAssisted,
        Self::VendorEnhanced,
        Self::Experimental,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Hybrid => "hybrid",
            Self::High => "high",
            Self::RtAssisted => "rt_assisted",
            Self::VendorEnhanced => "vendor_enhanced",
            Self::Experimental => "experimental",
        }
    }

    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Self::Baseline => 0,
            Self::Hybrid => 1,
            Self::High => 2,
            Self::RtAssisted => 3,
            Self::VendorEnhanced => 4,
            Self::Experimental => 5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererQualityTierDescriptor {
    pub tier: RendererQualityTier,
    pub stable_id: &'static str,
    pub classic_dynamic_geometry: bool,
    pub simple_shadows: bool,
    pub clustered_lighting: bool,
    pub native_fallback_upscaling: bool,
    pub virtual_geometry: bool,
    pub virtual_shadows: bool,
    pub many_light_direct: bool,
    pub hybrid_gi: bool,
    pub larger_page_budgets: bool,
    pub better_gi_reflections: bool,
    pub selective_rt: bool,
    pub vendor_upscaling_or_frame_generation: bool,
    pub experimental_models_or_work_graphs: bool,
}

pub const RENDERER_QUALITY_TIERS: [RendererQualityTierDescriptor; RENDERER_QUALITY_TIER_COUNT] = [
    RendererQualityTierDescriptor {
        tier: RendererQualityTier::Baseline,
        stable_id: "renderer.quality.baseline",
        classic_dynamic_geometry: true,
        simple_shadows: true,
        clustered_lighting: true,
        native_fallback_upscaling: true,
        virtual_geometry: false,
        virtual_shadows: false,
        many_light_direct: false,
        hybrid_gi: false,
        larger_page_budgets: false,
        better_gi_reflections: false,
        selective_rt: false,
        vendor_upscaling_or_frame_generation: false,
        experimental_models_or_work_graphs: false,
    },
    RendererQualityTierDescriptor {
        tier: RendererQualityTier::Hybrid,
        stable_id: "renderer.quality.hybrid",
        classic_dynamic_geometry: true,
        simple_shadows: true,
        clustered_lighting: true,
        native_fallback_upscaling: true,
        virtual_geometry: true,
        virtual_shadows: true,
        many_light_direct: true,
        hybrid_gi: true,
        larger_page_budgets: false,
        better_gi_reflections: false,
        selective_rt: false,
        vendor_upscaling_or_frame_generation: false,
        experimental_models_or_work_graphs: false,
    },
    RendererQualityTierDescriptor {
        tier: RendererQualityTier::High,
        stable_id: "renderer.quality.high",
        classic_dynamic_geometry: true,
        simple_shadows: true,
        clustered_lighting: true,
        native_fallback_upscaling: true,
        virtual_geometry: true,
        virtual_shadows: true,
        many_light_direct: true,
        hybrid_gi: true,
        larger_page_budgets: true,
        better_gi_reflections: true,
        selective_rt: false,
        vendor_upscaling_or_frame_generation: false,
        experimental_models_or_work_graphs: false,
    },
    RendererQualityTierDescriptor {
        tier: RendererQualityTier::RtAssisted,
        stable_id: "renderer.quality.rt_assisted",
        classic_dynamic_geometry: true,
        simple_shadows: true,
        clustered_lighting: true,
        native_fallback_upscaling: true,
        virtual_geometry: true,
        virtual_shadows: true,
        many_light_direct: true,
        hybrid_gi: true,
        larger_page_budgets: true,
        better_gi_reflections: true,
        selective_rt: true,
        vendor_upscaling_or_frame_generation: false,
        experimental_models_or_work_graphs: false,
    },
    RendererQualityTierDescriptor {
        tier: RendererQualityTier::VendorEnhanced,
        stable_id: "renderer.quality.vendor_enhanced",
        classic_dynamic_geometry: true,
        simple_shadows: true,
        clustered_lighting: true,
        native_fallback_upscaling: true,
        virtual_geometry: true,
        virtual_shadows: true,
        many_light_direct: true,
        hybrid_gi: true,
        larger_page_budgets: true,
        better_gi_reflections: true,
        selective_rt: false,
        vendor_upscaling_or_frame_generation: true,
        experimental_models_or_work_graphs: false,
    },
    RendererQualityTierDescriptor {
        tier: RendererQualityTier::Experimental,
        stable_id: "renderer.quality.experimental",
        classic_dynamic_geometry: true,
        simple_shadows: true,
        clustered_lighting: true,
        native_fallback_upscaling: true,
        virtual_geometry: true,
        virtual_shadows: true,
        many_light_direct: true,
        hybrid_gi: true,
        larger_page_budgets: true,
        better_gi_reflections: true,
        selective_rt: true,
        vendor_upscaling_or_frame_generation: true,
        experimental_models_or_work_graphs: true,
    },
];

#[must_use]
pub fn renderer_quality_tier_descriptor(
    tier: RendererQualityTier,
) -> &'static RendererQualityTierDescriptor {
    RENDERER_QUALITY_TIERS
        .iter()
        .find(|descriptor| descriptor.tier == tier)
        .expect("quality tier table must cover every tier")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeBackendSetting {
    Auto,
    Legacy,
    NewCore,
}

impl RuntimeBackendSetting {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Legacy => "legacy",
            Self::NewCore => "fun",
        }
    }

    #[must_use]
    pub const fn to_runtime_backend(self) -> FunRendererRuntimeBackend {
        match self {
            Self::Auto => FunRendererRuntimeBackend::Auto,
            Self::Legacy => FunRendererRuntimeBackend::Legacy,
            Self::NewCore => FunRendererRuntimeBackend::Fun,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphicsBackendSetting {
    Auto,
    Dx12,
    Vulkan,
    Metal,
}

impl GraphicsBackendSetting {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Dx12 => "dx12",
            Self::Vulkan => "vulkan",
            Self::Metal => "metal",
        }
    }

    #[must_use]
    pub const fn to_renderer_backend(self) -> Option<FunRendererBackend> {
        match self {
            Self::Auto => None,
            Self::Dx12 => Some(FunRendererBackend::Dx12),
            Self::Vulkan => Some(FunRendererBackend::Vulkan),
            Self::Metal => Some(FunRendererBackend::Metal),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpscalerSetting {
    Disabled,
    Auto,
    NativeFallback,
    DlssSuperResolution,
    Fsr2,
    Fsr3,
}

impl UpscalerSetting {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Auto => "auto",
            Self::NativeFallback => "native_fallback",
            Self::DlssSuperResolution => "dlss_super_resolution",
            Self::Fsr2 => "fsr2",
            Self::Fsr3 => "fsr3",
        }
    }

    #[must_use]
    pub const fn to_upscaler_mode(self) -> UpscalerMode {
        match self {
            Self::Disabled => UpscalerMode::Disabled,
            Self::Auto | Self::NativeFallback => UpscalerMode::NativeBilinear,
            Self::DlssSuperResolution => UpscalerMode::DlssSuperResolution,
            Self::Fsr2 => UpscalerMode::Fsr2,
            Self::Fsr3 => UpscalerMode::Fsr3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameGenerationSetting {
    Disabled,
    Auto,
    DlssFrameGeneration,
    FsrFrameGeneration,
}

impl FrameGenerationSetting {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Auto => "auto",
            Self::DlssFrameGeneration => "dlss_frame_generation",
            Self::FsrFrameGeneration => "fsr_frame_generation",
        }
    }

    #[must_use]
    pub const fn to_frame_generation_mode(self) -> FrameGenerationMode {
        match self {
            Self::Disabled | Self::Auto => FrameGenerationMode::Disabled,
            Self::DlssFrameGeneration => FrameGenerationMode::DlssFrameGeneration,
            Self::FsrFrameGeneration => FrameGenerationMode::FsrFrameGeneration,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShadowQuality {
    Simple,
    VirtualLow,
    VirtualHigh,
    RtAssisted,
}

impl ShadowQuality {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Simple => "simple",
            Self::VirtualLow => "virtual_low",
            Self::VirtualHigh => "virtual_high",
            Self::RtAssisted => "rt_assisted",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GiQuality {
    Off,
    AmbientProbe,
    ScreenSpace,
    SurfaceCache,
    RtAssisted,
    ExperimentalRadiance,
}

impl GiQuality {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::AmbientProbe => "ambient_probe",
            Self::ScreenSpace => "screen_space",
            Self::SurfaceCache => "surface_cache",
            Self::RtAssisted => "rt_assisted",
            Self::ExperimentalRadiance => "experimental_radiance",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReflectionQuality {
    Off,
    ScreenSpace,
    SurfaceCache,
    RtAssisted,
}

impl ReflectionQuality {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::ScreenSpace => "screen_space",
            Self::SurfaceCache => "surface_cache",
            Self::RtAssisted => "rt_assisted",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetPreset {
    Minimum,
    Balanced,
    High,
    Extreme,
}

impl BudgetPreset {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Minimum => "minimum",
            Self::Balanced => "balanced",
            Self::High => "high",
            Self::Extreme => "extreme",
        }
    }

    #[must_use]
    pub const fn scale_per_mille(self) -> u16 {
        match self {
            Self::Minimum => 500,
            Self::Balanced => 1000,
            Self::High => 1400,
            Self::Extreme => 2000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticsVisibility {
    Hidden,
    Compact,
    Detailed,
    BenchmarkCapture,
}

impl DiagnosticsVisibility {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hidden => "hidden",
            Self::Compact => "compact",
            Self::Detailed => "detailed",
            Self::BenchmarkCapture => "benchmark_capture",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RendererUserSettings {
    pub runtime_backend: RuntimeBackendSetting,
    pub graphics_backend: GraphicsBackendSetting,
    pub quality_preset: RendererQualityTier,
    pub upscaler: UpscalerSetting,
    pub frame_generation: FrameGenerationSetting,
    pub shadow_quality: ShadowQuality,
    pub gi_quality: GiQuality,
    pub reflection_quality: ReflectionQuality,
    pub virtual_geometry_budget: BudgetPreset,
    pub texture_streaming_budget: BudgetPreset,
    pub diagnostics_visibility: DiagnosticsVisibility,
}

impl RendererUserSettings {
    #[must_use]
    pub const fn baseline() -> Self {
        Self {
            runtime_backend: RuntimeBackendSetting::Auto,
            graphics_backend: GraphicsBackendSetting::Auto,
            quality_preset: RendererQualityTier::Baseline,
            upscaler: UpscalerSetting::NativeFallback,
            frame_generation: FrameGenerationSetting::Disabled,
            shadow_quality: ShadowQuality::Simple,
            gi_quality: GiQuality::AmbientProbe,
            reflection_quality: ReflectionQuality::ScreenSpace,
            virtual_geometry_budget: BudgetPreset::Minimum,
            texture_streaming_budget: BudgetPreset::Balanced,
            diagnostics_visibility: DiagnosticsVisibility::Hidden,
        }
    }
}

impl Default for RendererUserSettings {
    fn default() -> Self {
        Self::baseline()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineWarmupPolicySetting {
    RendererInitialization,
    SceneBoundary,
    QualityOrBackendChange,
    Exhaustive,
}

impl PipelineWarmupPolicySetting {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RendererInitialization => "renderer_initialization",
            Self::SceneBoundary => "scene_boundary",
            Self::QualityOrBackendChange => "quality_or_backend_change",
            Self::Exhaustive => "exhaustive",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimePipelineCreationPolicy {
    Forbidden,
    BenchmarkWhitelistOnly,
    DeveloperOverride,
}

impl RuntimePipelineCreationPolicy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Forbidden => "forbidden",
            Self::BenchmarkWhitelistOnly => "benchmark_whitelist_only",
            Self::DeveloperOverride => "developer_override",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FallbackStrictness {
    ProductFailClosed,
    ExplainAndFallback,
    DeveloperOverride,
}

impl FallbackStrictness {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProductFailClosed => "product_fail_closed",
            Self::ExplainAndFallback => "explain_and_fallback",
            Self::DeveloperOverride => "developer_override",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RendererInternalSettings {
    pub page_pool_pages: u32,
    pub shadow_page_budget: u32,
    pub light_candidate_budget: u32,
    pub gi_cache_update_budget: u32,
    pub pipeline_warmup_policy: PipelineWarmupPolicySetting,
    pub runtime_pipeline_creation: RuntimePipelineCreationPolicy,
    pub fallback_strictness: FallbackStrictness,
}

impl RendererInternalSettings {
    #[must_use]
    pub const fn baseline() -> Self {
        Self {
            page_pool_pages: 256,
            shadow_page_budget: 64,
            light_candidate_budget: 256,
            gi_cache_update_budget: 16,
            pipeline_warmup_policy: PipelineWarmupPolicySetting::RendererInitialization,
            runtime_pipeline_creation: RuntimePipelineCreationPolicy::Forbidden,
            fallback_strictness: FallbackStrictness::ProductFailClosed,
        }
    }

    #[must_use]
    pub const fn for_tier(tier: RendererQualityTier) -> Self {
        match tier {
            RendererQualityTier::Baseline => Self::baseline(),
            RendererQualityTier::Hybrid => Self {
                page_pool_pages: 1_024,
                shadow_page_budget: 256,
                light_candidate_budget: 1_024,
                gi_cache_update_budget: 64,
                pipeline_warmup_policy: PipelineWarmupPolicySetting::SceneBoundary,
                runtime_pipeline_creation: RuntimePipelineCreationPolicy::Forbidden,
                fallback_strictness: FallbackStrictness::ProductFailClosed,
            },
            RendererQualityTier::High => Self {
                page_pool_pages: 2_048,
                shadow_page_budget: 512,
                light_candidate_budget: 2_048,
                gi_cache_update_budget: 128,
                pipeline_warmup_policy: PipelineWarmupPolicySetting::QualityOrBackendChange,
                runtime_pipeline_creation: RuntimePipelineCreationPolicy::Forbidden,
                fallback_strictness: FallbackStrictness::ProductFailClosed,
            },
            RendererQualityTier::RtAssisted | RendererQualityTier::VendorEnhanced => Self {
                page_pool_pages: 2_048,
                shadow_page_budget: 768,
                light_candidate_budget: 2_048,
                gi_cache_update_budget: 160,
                pipeline_warmup_policy: PipelineWarmupPolicySetting::QualityOrBackendChange,
                runtime_pipeline_creation: RuntimePipelineCreationPolicy::Forbidden,
                fallback_strictness: FallbackStrictness::ProductFailClosed,
            },
            RendererQualityTier::Experimental => Self {
                page_pool_pages: 3_072,
                shadow_page_budget: 1_024,
                light_candidate_budget: 3_072,
                gi_cache_update_budget: 192,
                pipeline_warmup_policy: PipelineWarmupPolicySetting::Exhaustive,
                runtime_pipeline_creation: RuntimePipelineCreationPolicy::BenchmarkWhitelistOnly,
                fallback_strictness: FallbackStrictness::ProductFailClosed,
            },
        }
    }
}

impl Default for RendererInternalSettings {
    fn default() -> Self {
        Self::baseline()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RendererSettingsRequest {
    pub user: RendererUserSettings,
    pub internal: RendererInternalSettings,
}

impl RendererSettingsRequest {
    #[must_use]
    pub const fn baseline() -> Self {
        Self {
            user: RendererUserSettings::baseline(),
            internal: RendererInternalSettings::baseline(),
        }
    }

    #[must_use]
    pub const fn for_tier(tier: RendererQualityTier) -> Self {
        let user = RendererUserSettings {
            quality_preset: tier,
            ..RendererUserSettings::baseline()
        };
        Self {
            user,
            internal: RendererInternalSettings::for_tier(tier),
        }
    }
}

impl Default for RendererSettingsRequest {
    fn default() -> Self {
        Self::baseline()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterVendor {
    Unknown,
    Nvidia,
    Amd,
    Intel,
    Other,
}

impl AdapterVendor {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Nvidia => "nvidia",
            Self::Amd => "amd",
            Self::Intel => "intel",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplayMode {
    Sdr,
    Hdr,
}

impl DisplayMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sdr => "sdr",
            Self::Hdr => "hdr",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RendererRuntimeMode {
    Product,
    EditorDocked,
    EditorImmersive,
    Benchmark,
}

impl RendererRuntimeMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Product => "product",
            Self::EditorDocked => "editor_docked",
            Self::EditorImmersive => "editor_immersive",
            Self::Benchmark => "benchmark",
        }
    }

    #[must_use]
    pub const fn blocks_default_frame_generation(self) -> bool {
        matches!(self, Self::EditorDocked)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RendererCompiledFeatureSupport {
    pub new_core: bool,
    pub dx12: bool,
    pub vulkan: bool,
    pub metal: bool,
    pub native_ui_gpu_only: bool,
    pub upscaling: bool,
    pub dlss: bool,
    pub fsr: bool,
    pub frame_generation: bool,
    pub many_light: bool,
    pub virtual_geometry: bool,
    pub virtual_shadows: bool,
    pub hybrid_gi: bool,
    pub experimental_ml: bool,
    pub experimental_work_graphs: bool,
    pub mesh_shader_path: bool,
    pub learned_page_priority_predictor: bool,
    pub neural_texture_compression: bool,
}

impl RendererCompiledFeatureSupport {
    pub const COMPILED: Self = Self {
        new_core: cfg!(feature = "fun_renderer_core"),
        dx12: cfg!(feature = "dx12_native_interop"),
        vulkan: cfg!(feature = "vulkan_backend"),
        metal: cfg!(feature = "metal_backend"),
        native_ui_gpu_only: cfg!(feature = "native_ui_gpu_only"),
        upscaling: cfg!(feature = "upscaling"),
        dlss: cfg!(feature = "dlss"),
        fsr: cfg!(feature = "fsr"),
        frame_generation: cfg!(feature = "frame_generation"),
        many_light: cfg!(feature = "many_light"),
        virtual_geometry: cfg!(feature = "virtual_geometry"),
        virtual_shadows: cfg!(feature = "virtual_shadows"),
        hybrid_gi: cfg!(feature = "hybrid_gi"),
        experimental_ml: cfg!(feature = "experimental_renderer_ml"),
        experimental_work_graphs: cfg!(feature = "experimental_work_graphs"),
        mesh_shader_path: cfg!(feature = "mesh_shader_path"),
        learned_page_priority_predictor: cfg!(feature = "learned_page_priority_predictor"),
        neural_texture_compression: cfg!(feature = "neural_texture_compression"),
    };

    #[must_use]
    pub const fn from_renderer_features(features: RendererFeatureToggles) -> Self {
        Self {
            new_core: features.new_core,
            dx12: features.dx12,
            vulkan: features.vulkan,
            metal: features.metal,
            native_ui_gpu_only: features.native_ui_gpu_only,
            upscaling: features.upscale,
            dlss: features.dlss,
            fsr: features.fsr,
            frame_generation: features.frame_generation,
            many_light: cfg!(feature = "many_light"),
            virtual_geometry: cfg!(feature = "virtual_geometry"),
            virtual_shadows: cfg!(feature = "virtual_shadows"),
            hybrid_gi: cfg!(feature = "hybrid_gi"),
            experimental_ml: features.experimental_ml,
            experimental_work_graphs: cfg!(feature = "experimental_work_graphs"),
            mesh_shader_path: cfg!(feature = "mesh_shader_path"),
            learned_page_priority_predictor: cfg!(feature = "learned_page_priority_predictor"),
            neural_texture_compression: cfg!(feature = "neural_texture_compression"),
        }
    }
}

impl Default for RendererCompiledFeatureSupport {
    fn default() -> Self {
        Self::COMPILED
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameGenerationCapabilitySupport {
    pub dlss_fg: bool,
    pub fsr_fg: bool,
    pub valid_present_lifetimes: bool,
    pub stable_ui_until_present: bool,
    pub stable_frame_pacing: bool,
}

impl FrameGenerationCapabilitySupport {
    #[must_use]
    pub const fn none() -> Self {
        Self {
            dlss_fg: false,
            fsr_fg: false,
            valid_present_lifetimes: false,
            stable_ui_until_present: false,
            stable_frame_pacing: false,
        }
    }

    #[must_use]
    pub const fn any_vendor_path(self) -> bool {
        self.dlss_fg || self.fsr_fg
    }

    #[must_use]
    pub const fn ready(self) -> bool {
        self.any_vendor_path()
            && self.valid_present_lifetimes
            && self.stable_ui_until_present
            && self.stable_frame_pacing
    }
}

impl Default for FrameGenerationCapabilitySupport {
    fn default() -> Self {
        Self::none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RendererCapabilityFacts {
    pub runtime_backend: RuntimeBackendSetting,
    pub actual_backend: GraphicsBackendSetting,
    pub adapter_vendor: AdapterVendor,
    pub vram_mb: u32,
    pub display_mode: DisplayMode,
    pub runtime_mode: RendererRuntimeMode,
    pub native_ui_gpu_transport_available: bool,
    pub bindless_supported: bool,
    pub descriptor_indexing_supported: bool,
    pub sampler_feedback_supported: bool,
    pub mesh_shader_supported: bool,
    pub ray_tracing_supported: bool,
    pub variable_rate_shading_supported: bool,
    pub work_graphs_supported: bool,
    pub neural_texture_compression_supported: bool,
    pub upscalers: UpscalerCapabilities,
    pub frame_generation: FrameGenerationCapabilitySupport,
    pub compiled_features: RendererCompiledFeatureSupport,
}

impl RendererCapabilityFacts {
    #[must_use]
    pub const fn minimal() -> Self {
        Self {
            runtime_backend: RuntimeBackendSetting::Auto,
            actual_backend: GraphicsBackendSetting::Auto,
            adapter_vendor: AdapterVendor::Unknown,
            vram_mb: 0,
            display_mode: DisplayMode::Sdr,
            runtime_mode: RendererRuntimeMode::Product,
            native_ui_gpu_transport_available: false,
            bindless_supported: false,
            descriptor_indexing_supported: false,
            sampler_feedback_supported: false,
            mesh_shader_supported: false,
            ray_tracing_supported: false,
            variable_rate_shading_supported: false,
            work_graphs_supported: false,
            neural_texture_compression_supported: false,
            upscalers: UpscalerCapabilities::software_only(),
            frame_generation: FrameGenerationCapabilitySupport::none(),
            compiled_features: RendererCompiledFeatureSupport::COMPILED,
        }
    }

    #[must_use]
    pub const fn from_renderer_features(
        features: RendererFeatureToggles,
        runtime_backend: RuntimeBackendSetting,
        actual_backend: GraphicsBackendSetting,
        runtime_mode: RendererRuntimeMode,
        native_ui_gpu_transport_available: bool,
    ) -> Self {
        let compiled_features = RendererCompiledFeatureSupport::from_renderer_features(features);
        Self {
            runtime_backend,
            actual_backend,
            adapter_vendor: AdapterVendor::Unknown,
            vram_mb: 0,
            display_mode: DisplayMode::Sdr,
            runtime_mode,
            native_ui_gpu_transport_available,
            bindless_supported: false,
            descriptor_indexing_supported: false,
            sampler_feedback_supported: false,
            mesh_shader_supported: false,
            ray_tracing_supported: false,
            variable_rate_shading_supported: false,
            work_graphs_supported: false,
            neural_texture_compression_supported: false,
            upscalers: UpscalerCapabilities {
                native: true,
                dlss_sr: compiled_features.dlss,
                fsr2: compiled_features.fsr,
                fsr3: compiled_features.fsr,
                hdr: true,
                reactive_mask: compiled_features.fsr,
                transparency_mask: compiled_features.fsr,
            },
            frame_generation: FrameGenerationCapabilitySupport {
                dlss_fg: compiled_features.frame_generation && compiled_features.dlss,
                fsr_fg: compiled_features.frame_generation && compiled_features.fsr,
                valid_present_lifetimes: false,
                stable_ui_until_present: false,
                stable_frame_pacing: false,
            },
            compiled_features,
        }
    }

    #[must_use]
    pub const fn high_end_dx12_nvidia() -> Self {
        Self {
            runtime_backend: RuntimeBackendSetting::NewCore,
            actual_backend: GraphicsBackendSetting::Dx12,
            adapter_vendor: AdapterVendor::Nvidia,
            vram_mb: 16_384,
            display_mode: DisplayMode::Hdr,
            runtime_mode: RendererRuntimeMode::Product,
            native_ui_gpu_transport_available: true,
            bindless_supported: true,
            descriptor_indexing_supported: true,
            sampler_feedback_supported: true,
            mesh_shader_supported: true,
            ray_tracing_supported: true,
            variable_rate_shading_supported: true,
            work_graphs_supported: false,
            neural_texture_compression_supported: false,
            upscalers: UpscalerCapabilities {
                native: true,
                dlss_sr: true,
                fsr2: true,
                fsr3: true,
                hdr: true,
                reactive_mask: true,
                transparency_mask: true,
            },
            frame_generation: FrameGenerationCapabilitySupport {
                dlss_fg: true,
                fsr_fg: true,
                valid_present_lifetimes: true,
                stable_ui_until_present: true,
                stable_frame_pacing: true,
            },
            compiled_features: RendererCompiledFeatureSupport {
                new_core: true,
                dx12: true,
                vulkan: true,
                metal: true,
                native_ui_gpu_only: true,
                upscaling: true,
                dlss: true,
                fsr: true,
                frame_generation: true,
                many_light: true,
                virtual_geometry: true,
                virtual_shadows: true,
                hybrid_gi: true,
                experimental_ml: true,
                experimental_work_graphs: true,
                mesh_shader_path: true,
                learned_page_priority_predictor: true,
                neural_texture_compression: true,
            },
        }
    }

    #[must_use]
    pub const fn supports_vendor_upscaling(self) -> bool {
        self.upscalers.dlss_sr || self.upscalers.fsr2 || self.upscalers.fsr3
    }

    #[must_use]
    pub const fn supports_experimental_features(self) -> bool {
        self.compiled_features.experimental_ml
            || self.compiled_features.experimental_work_graphs
            || self.compiled_features.mesh_shader_path
            || self.compiled_features.learned_page_priority_predictor
            || self.compiled_features.neural_texture_compression
            || self.work_graphs_supported
            || self.neural_texture_compression_supported
    }
}

impl Default for RendererCapabilityFacts {
    fn default() -> Self {
        Self::minimal()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RendererSettingKind {
    RuntimeBackend,
    GraphicsBackend,
    QualityPreset,
    Upscaler,
    FrameGeneration,
    ShadowQuality,
    GiQuality,
    ReflectionQuality,
    VirtualGeometryBudget,
    TextureStreamingBudget,
    DiagnosticsVisibility,
    InternalBudget,
    FallbackStrictness,
}

impl RendererSettingKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RuntimeBackend => "runtime_backend",
            Self::GraphicsBackend => "graphics_backend",
            Self::QualityPreset => "quality_preset",
            Self::Upscaler => "upscaler",
            Self::FrameGeneration => "frame_generation",
            Self::ShadowQuality => "shadow_quality",
            Self::GiQuality => "gi_quality",
            Self::ReflectionQuality => "reflection_quality",
            Self::VirtualGeometryBudget => "virtual_geometry_budget",
            Self::TextureStreamingBudget => "texture_streaming_budget",
            Self::DiagnosticsVisibility => "diagnostics_visibility",
            Self::InternalBudget => "internal_budget",
            Self::FallbackStrictness => "fallback_strictness",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnsupportedRendererSettingReason {
    RuntimeBackendUnavailable,
    GraphicsBackendUnavailable,
    NativeUiGpuTransportRequired,
    VirtualGeometryUnavailable,
    VirtualShadowsUnavailable,
    ManyLightUnavailable,
    HybridGiUnavailable,
    RayTracingUnavailable,
    VendorUpscalerUnavailable,
    FrameGenerationUnavailable,
    FrameGenerationPolicyBlocked,
    ExperimentalFeatureUnavailable,
}

impl UnsupportedRendererSettingReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RuntimeBackendUnavailable => "runtime_backend_unavailable",
            Self::GraphicsBackendUnavailable => "graphics_backend_unavailable",
            Self::NativeUiGpuTransportRequired => "native_ui_gpu_transport_required",
            Self::VirtualGeometryUnavailable => "virtual_geometry_unavailable",
            Self::VirtualShadowsUnavailable => "virtual_shadows_unavailable",
            Self::ManyLightUnavailable => "many_light_unavailable",
            Self::HybridGiUnavailable => "hybrid_gi_unavailable",
            Self::RayTracingUnavailable => "ray_tracing_unavailable",
            Self::VendorUpscalerUnavailable => "vendor_upscaler_unavailable",
            Self::FrameGenerationUnavailable => "frame_generation_unavailable",
            Self::FrameGenerationPolicyBlocked => "frame_generation_policy_blocked",
            Self::ExperimentalFeatureUnavailable => "experimental_feature_unavailable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RendererSettingsAdjustment {
    pub setting: RendererSettingKind,
    pub requested: &'static str,
    pub selected: &'static str,
    pub reason: UnsupportedRendererSettingReason,
}

impl RendererSettingsAdjustment {
    #[must_use]
    pub const fn new(
        setting: RendererSettingKind,
        requested: &'static str,
        selected: &'static str,
        reason: UnsupportedRendererSettingReason,
    ) -> Self {
        Self {
            setting,
            requested,
            selected,
            reason,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RendererSettingsSelection {
    pub schema: &'static str,
    pub schema_version: u16,
    pub requested: RendererSettingsRequest,
    pub selected_user: RendererUserSettings,
    pub selected_internal: RendererInternalSettings,
    pub capabilities: RendererCapabilityFacts,
    pub adjustments: Vec<RendererSettingsAdjustment>,
    pub product_ui_usable: bool,
    pub benchmark_repro: RendererBenchmarkReproSettings,
}

impl RendererSettingsSelection {
    #[must_use]
    pub fn rejected(&self) -> bool {
        !self.adjustments.is_empty()
    }

    #[must_use]
    pub fn has_reason(&self, reason: UnsupportedRendererSettingReason) -> bool {
        self.adjustments
            .iter()
            .any(|adjustment| adjustment.reason == reason)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RendererBenchmarkReproSettings {
    pub runtime_backend: RuntimeBackendSetting,
    pub graphics_backend: GraphicsBackendSetting,
    pub quality_preset: RendererQualityTier,
    pub upscaler: UpscalerSetting,
    pub frame_generation: FrameGenerationSetting,
    pub page_pool_pages: u32,
    pub shadow_page_budget: u32,
    pub light_candidate_budget: u32,
    pub gi_cache_update_budget: u32,
    pub pipeline_warmup_policy: PipelineWarmupPolicySetting,
    pub runtime_pipeline_creation: RuntimePipelineCreationPolicy,
    pub fallback_strictness: FallbackStrictness,
}

impl RendererBenchmarkReproSettings {
    #[must_use]
    pub const fn from_selection(
        user: RendererUserSettings,
        internal: RendererInternalSettings,
    ) -> Self {
        Self {
            runtime_backend: user.runtime_backend,
            graphics_backend: user.graphics_backend,
            quality_preset: user.quality_preset,
            upscaler: user.upscaler,
            frame_generation: user.frame_generation,
            page_pool_pages: internal.page_pool_pages,
            shadow_page_budget: internal.shadow_page_budget,
            light_candidate_budget: internal.light_candidate_budget,
            gi_cache_update_budget: internal.gi_cache_update_budget,
            pipeline_warmup_policy: internal.pipeline_warmup_policy,
            runtime_pipeline_creation: internal.runtime_pipeline_creation,
            fallback_strictness: internal.fallback_strictness,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RendererSettingsCapabilityArtifact {
    pub schema: &'static str,
    pub schema_version: u16,
    pub content: String,
}

impl RendererSettingsCapabilityArtifact {
    #[must_use]
    pub fn from_selection(selection: &RendererSettingsSelection) -> Self {
        let mut content = String::new();
        let _ = writeln!(content, "schema={}", RENDERER_SETTINGS_SCHEMA);
        let _ = writeln!(
            content,
            "schema_version={}",
            RENDERER_SETTINGS_SCHEMA_VERSION
        );
        let _ = writeln!(
            content,
            "runtime_backend={}",
            selection.selected_user.runtime_backend.as_str()
        );
        let _ = writeln!(
            content,
            "graphics_backend={}",
            selection.selected_user.graphics_backend.as_str()
        );
        let _ = writeln!(
            content,
            "quality_preset={}",
            selection.selected_user.quality_preset.as_str()
        );
        let _ = writeln!(
            content,
            "upscaler={}",
            selection.selected_user.upscaler.as_str()
        );
        let _ = writeln!(
            content,
            "frame_generation={}",
            selection.selected_user.frame_generation.as_str()
        );
        let _ = writeln!(
            content,
            "shadow_quality={}",
            selection.selected_user.shadow_quality.as_str()
        );
        let _ = writeln!(
            content,
            "gi_quality={}",
            selection.selected_user.gi_quality.as_str()
        );
        let _ = writeln!(
            content,
            "reflection_quality={}",
            selection.selected_user.reflection_quality.as_str()
        );
        let _ = writeln!(
            content,
            "virtual_geometry_budget={}",
            selection.selected_user.virtual_geometry_budget.as_str()
        );
        let _ = writeln!(
            content,
            "texture_streaming_budget={}",
            selection.selected_user.texture_streaming_budget.as_str()
        );
        let _ = writeln!(
            content,
            "diagnostics_visibility={}",
            selection.selected_user.diagnostics_visibility.as_str()
        );
        let _ = writeln!(
            content,
            "page_pool_pages={}",
            selection.selected_internal.page_pool_pages
        );
        let _ = writeln!(
            content,
            "shadow_page_budget={}",
            selection.selected_internal.shadow_page_budget
        );
        let _ = writeln!(
            content,
            "light_candidate_budget={}",
            selection.selected_internal.light_candidate_budget
        );
        let _ = writeln!(
            content,
            "gi_cache_update_budget={}",
            selection.selected_internal.gi_cache_update_budget
        );
        let _ = writeln!(
            content,
            "pipeline_warmup_policy={}",
            selection.selected_internal.pipeline_warmup_policy.as_str()
        );
        let _ = writeln!(
            content,
            "runtime_pipeline_creation={}",
            selection
                .selected_internal
                .runtime_pipeline_creation
                .as_str()
        );
        let _ = writeln!(
            content,
            "fallback_strictness={}",
            selection.selected_internal.fallback_strictness.as_str()
        );
        let _ = writeln!(
            content,
            "capability actual_backend={}",
            selection.capabilities.actual_backend.as_str()
        );
        let _ = writeln!(
            content,
            "capability adapter_vendor={}",
            selection.capabilities.adapter_vendor.as_str()
        );
        let _ = writeln!(
            content,
            "capability vram_mb={}",
            selection.capabilities.vram_mb
        );
        let _ = writeln!(
            content,
            "capability runtime_mode={}",
            selection.capabilities.runtime_mode.as_str()
        );
        let _ = writeln!(
            content,
            "capability native_ui_gpu_transport_available={}",
            selection.capabilities.native_ui_gpu_transport_available
        );
        let _ = writeln!(
            content,
            "capability dlss_sr={} fsr2={} fsr3={} dlss_fg={} fsr_fg={} ray_tracing={} work_graphs={}",
            selection.capabilities.upscalers.dlss_sr,
            selection.capabilities.upscalers.fsr2,
            selection.capabilities.upscalers.fsr3,
            selection.capabilities.frame_generation.dlss_fg,
            selection.capabilities.frame_generation.fsr_fg,
            selection.capabilities.ray_tracing_supported,
            selection.capabilities.work_graphs_supported
        );
        let _ = writeln!(content, "product_ui_usable={}", selection.product_ui_usable);
        let _ = writeln!(content, "adjustment_count={}", selection.adjustments.len());
        for adjustment in &selection.adjustments {
            let _ = writeln!(
                content,
                "adjustment setting={} requested={} selected={} reason={}",
                adjustment.setting.as_str(),
                adjustment.requested,
                adjustment.selected,
                adjustment.reason.as_str()
            );
        }
        Self {
            schema: RENDERER_SETTINGS_SCHEMA,
            schema_version: RENDERER_SETTINGS_SCHEMA_VERSION,
            content,
        }
    }
}

pub fn write_renderer_settings_capability_artifact(
    path: impl AsRef<Path>,
    artifact: &RendererSettingsCapabilityArtifact,
) -> io::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, &artifact.content)
}

#[must_use]
pub fn select_capability_aware_renderer_defaults(
    capabilities: RendererCapabilityFacts,
) -> RendererSettingsSelection {
    let tier = default_quality_tier(capabilities);
    let mut request = RendererSettingsRequest::for_tier(tier);
    request.user.runtime_backend = capabilities.runtime_backend;
    request.user.graphics_backend = capabilities.actual_backend;
    request.user.upscaler = default_upscaler_setting(capabilities);
    request.user.frame_generation = FrameGenerationSetting::Disabled;
    request.user.shadow_quality = default_shadow_quality(tier, capabilities);
    request.user.gi_quality = default_gi_quality(tier, capabilities);
    request.user.reflection_quality = default_reflection_quality(tier, capabilities);
    request.user.virtual_geometry_budget = default_virtual_geometry_budget(tier);
    request.user.texture_streaming_budget = default_texture_streaming_budget(tier);
    resolve_renderer_settings(request, capabilities)
}

#[must_use]
pub fn resolve_renderer_settings(
    request: RendererSettingsRequest,
    capabilities: RendererCapabilityFacts,
) -> RendererSettingsSelection {
    let mut selected_user = request.user;
    let mut selected_internal = request.internal;
    let mut adjustments = Vec::new();

    if selected_user.runtime_backend == RuntimeBackendSetting::NewCore
        && !capabilities.compiled_features.new_core
    {
        adjustments.push(RendererSettingsAdjustment::new(
            RendererSettingKind::RuntimeBackend,
            selected_user.runtime_backend.as_str(),
            RuntimeBackendSetting::Legacy.as_str(),
            UnsupportedRendererSettingReason::RuntimeBackendUnavailable,
        ));
        selected_user.runtime_backend = RuntimeBackendSetting::Legacy;
    }

    if !graphics_backend_supported(selected_user.graphics_backend, capabilities) {
        let fallback = fallback_graphics_backend(capabilities);
        adjustments.push(RendererSettingsAdjustment::new(
            RendererSettingKind::GraphicsBackend,
            selected_user.graphics_backend.as_str(),
            fallback.as_str(),
            UnsupportedRendererSettingReason::GraphicsBackendUnavailable,
        ));
        selected_user.graphics_backend = fallback;
    }

    if capabilities.runtime_mode == RendererRuntimeMode::Product
        && !capabilities.native_ui_gpu_transport_available
    {
        adjustments.push(RendererSettingsAdjustment::new(
            RendererSettingKind::FallbackStrictness,
            FallbackStrictness::ProductFailClosed.as_str(),
            FallbackStrictness::ProductFailClosed.as_str(),
            UnsupportedRendererSettingReason::NativeUiGpuTransportRequired,
        ));
        selected_internal.fallback_strictness = FallbackStrictness::ProductFailClosed;
    }

    let requested_tier = selected_user.quality_preset;
    let selected_tier = supported_quality_tier(requested_tier, capabilities, &mut adjustments);
    if selected_tier != requested_tier {
        selected_user.quality_preset = selected_tier;
        selected_internal = RendererInternalSettings::for_tier(selected_tier);
    }

    selected_user.upscaler =
        resolve_upscaler(selected_user.upscaler, capabilities, &mut adjustments);
    selected_user.frame_generation = resolve_frame_generation(
        selected_user.frame_generation,
        capabilities,
        &mut adjustments,
    );
    selected_user.shadow_quality =
        resolve_shadow_quality(selected_user.shadow_quality, capabilities, &mut adjustments);
    selected_user.gi_quality =
        resolve_gi_quality(selected_user.gi_quality, capabilities, &mut adjustments);
    selected_user.reflection_quality = resolve_reflection_quality(
        selected_user.reflection_quality,
        capabilities,
        &mut adjustments,
    );

    clamp_internal_budgets(&mut selected_internal, selected_user.quality_preset);

    let benchmark_repro =
        RendererBenchmarkReproSettings::from_selection(selected_user, selected_internal);
    let product_ui_usable = capabilities.runtime_mode != RendererRuntimeMode::Product
        || capabilities.native_ui_gpu_transport_available;

    RendererSettingsSelection {
        schema: RENDERER_SETTINGS_SCHEMA,
        schema_version: RENDERER_SETTINGS_SCHEMA_VERSION,
        requested: request,
        selected_user,
        selected_internal,
        capabilities,
        adjustments,
        product_ui_usable,
        benchmark_repro,
    }
}

fn default_quality_tier(capabilities: RendererCapabilityFacts) -> RendererQualityTier {
    if !capabilities.native_ui_gpu_transport_available || !capabilities.compiled_features.new_core {
        return RendererQualityTier::Baseline;
    }
    if capabilities.vram_mb >= 12_288
        && tier_supported(RendererQualityTier::High, capabilities).is_none()
    {
        return RendererQualityTier::High;
    }
    if capabilities.vram_mb >= 6_144
        && tier_supported(RendererQualityTier::Hybrid, capabilities).is_none()
    {
        return RendererQualityTier::Hybrid;
    }
    RendererQualityTier::Baseline
}

fn default_upscaler_setting(capabilities: RendererCapabilityFacts) -> UpscalerSetting {
    if !capabilities.upscalers.native {
        UpscalerSetting::Disabled
    } else {
        UpscalerSetting::NativeFallback
    }
}

fn default_shadow_quality(
    tier: RendererQualityTier,
    capabilities: RendererCapabilityFacts,
) -> ShadowQuality {
    if tier.rank() >= RendererQualityTier::High.rank()
        && capabilities.compiled_features.virtual_shadows
    {
        ShadowQuality::VirtualHigh
    } else if tier.rank() >= RendererQualityTier::Hybrid.rank()
        && capabilities.compiled_features.virtual_shadows
    {
        ShadowQuality::VirtualLow
    } else {
        ShadowQuality::Simple
    }
}

fn default_gi_quality(
    tier: RendererQualityTier,
    capabilities: RendererCapabilityFacts,
) -> GiQuality {
    if tier.rank() >= RendererQualityTier::High.rank() && capabilities.compiled_features.hybrid_gi {
        GiQuality::SurfaceCache
    } else if tier.rank() >= RendererQualityTier::Hybrid.rank() {
        GiQuality::ScreenSpace
    } else {
        GiQuality::AmbientProbe
    }
}

fn default_reflection_quality(
    tier: RendererQualityTier,
    _capabilities: RendererCapabilityFacts,
) -> ReflectionQuality {
    if tier.rank() >= RendererQualityTier::High.rank() {
        ReflectionQuality::SurfaceCache
    } else {
        ReflectionQuality::ScreenSpace
    }
}

const fn default_virtual_geometry_budget(tier: RendererQualityTier) -> BudgetPreset {
    match tier {
        RendererQualityTier::Baseline => BudgetPreset::Minimum,
        RendererQualityTier::Hybrid => BudgetPreset::Balanced,
        RendererQualityTier::High
        | RendererQualityTier::RtAssisted
        | RendererQualityTier::VendorEnhanced => BudgetPreset::High,
        RendererQualityTier::Experimental => BudgetPreset::Extreme,
    }
}

const fn default_texture_streaming_budget(tier: RendererQualityTier) -> BudgetPreset {
    match tier {
        RendererQualityTier::Baseline => BudgetPreset::Balanced,
        RendererQualityTier::Hybrid | RendererQualityTier::High => BudgetPreset::High,
        RendererQualityTier::RtAssisted
        | RendererQualityTier::VendorEnhanced
        | RendererQualityTier::Experimental => BudgetPreset::Extreme,
    }
}

fn graphics_backend_supported(
    setting: GraphicsBackendSetting,
    capabilities: RendererCapabilityFacts,
) -> bool {
    match setting {
        GraphicsBackendSetting::Auto => true,
        GraphicsBackendSetting::Dx12 => {
            capabilities.compiled_features.dx12
                || capabilities.actual_backend == GraphicsBackendSetting::Dx12
        }
        GraphicsBackendSetting::Vulkan => {
            capabilities.compiled_features.vulkan
                || capabilities.actual_backend == GraphicsBackendSetting::Vulkan
        }
        GraphicsBackendSetting::Metal => {
            capabilities.compiled_features.metal
                || capabilities.actual_backend == GraphicsBackendSetting::Metal
        }
    }
}

fn fallback_graphics_backend(capabilities: RendererCapabilityFacts) -> GraphicsBackendSetting {
    if capabilities.actual_backend != GraphicsBackendSetting::Auto {
        return capabilities.actual_backend;
    }
    if capabilities.compiled_features.dx12 {
        GraphicsBackendSetting::Dx12
    } else if capabilities.compiled_features.vulkan {
        GraphicsBackendSetting::Vulkan
    } else if capabilities.compiled_features.metal {
        GraphicsBackendSetting::Metal
    } else {
        GraphicsBackendSetting::Auto
    }
}

fn supported_quality_tier(
    requested: RendererQualityTier,
    capabilities: RendererCapabilityFacts,
    adjustments: &mut Vec<RendererSettingsAdjustment>,
) -> RendererQualityTier {
    if tier_supported(requested, capabilities).is_none() {
        return requested;
    }
    for tier in RendererQualityTier::ORDER.iter().rev().copied() {
        if tier.rank() <= requested.rank() && tier_supported(tier, capabilities).is_none() {
            let reason = tier_supported(requested, capabilities)
                .expect("requested tier is known unsupported");
            adjustments.push(RendererSettingsAdjustment::new(
                RendererSettingKind::QualityPreset,
                requested.as_str(),
                tier.as_str(),
                reason,
            ));
            return tier;
        }
    }
    RendererQualityTier::Baseline
}

fn tier_supported(
    tier: RendererQualityTier,
    capabilities: RendererCapabilityFacts,
) -> Option<UnsupportedRendererSettingReason> {
    match tier {
        RendererQualityTier::Baseline => None,
        RendererQualityTier::Hybrid | RendererQualityTier::High => {
            hybrid_support_reason(capabilities)
        }
        RendererQualityTier::RtAssisted => {
            hybrid_support_reason(capabilities).or(if capabilities.ray_tracing_supported {
                None
            } else {
                Some(UnsupportedRendererSettingReason::RayTracingUnavailable)
            })
        }
        RendererQualityTier::VendorEnhanced => hybrid_support_reason(capabilities).or(
            if capabilities.supports_vendor_upscaling()
                || capabilities.frame_generation.any_vendor_path()
            {
                None
            } else {
                Some(UnsupportedRendererSettingReason::VendorUpscalerUnavailable)
            },
        ),
        RendererQualityTier::Experimental => hybrid_support_reason(capabilities).or(
            if capabilities.supports_experimental_features() {
                None
            } else {
                Some(UnsupportedRendererSettingReason::ExperimentalFeatureUnavailable)
            },
        ),
    }
}

fn hybrid_support_reason(
    capabilities: RendererCapabilityFacts,
) -> Option<UnsupportedRendererSettingReason> {
    if !capabilities.compiled_features.virtual_geometry {
        return Some(UnsupportedRendererSettingReason::VirtualGeometryUnavailable);
    }
    if !capabilities.compiled_features.virtual_shadows {
        return Some(UnsupportedRendererSettingReason::VirtualShadowsUnavailable);
    }
    if !capabilities.compiled_features.many_light {
        return Some(UnsupportedRendererSettingReason::ManyLightUnavailable);
    }
    if !capabilities.compiled_features.hybrid_gi {
        return Some(UnsupportedRendererSettingReason::HybridGiUnavailable);
    }
    None
}

fn resolve_upscaler(
    requested: UpscalerSetting,
    capabilities: RendererCapabilityFacts,
    adjustments: &mut Vec<RendererSettingsAdjustment>,
) -> UpscalerSetting {
    let selected = match requested {
        UpscalerSetting::Disabled => UpscalerSetting::Disabled,
        UpscalerSetting::Auto => vendor_upscaler_auto(capabilities),
        UpscalerSetting::NativeFallback => {
            if capabilities.upscalers.native {
                UpscalerSetting::NativeFallback
            } else {
                UpscalerSetting::Disabled
            }
        }
        UpscalerSetting::DlssSuperResolution => {
            if capabilities.upscalers.dlss_sr {
                requested
            } else {
                UpscalerSetting::NativeFallback
            }
        }
        UpscalerSetting::Fsr2 => {
            if capabilities.upscalers.fsr2 {
                requested
            } else {
                UpscalerSetting::NativeFallback
            }
        }
        UpscalerSetting::Fsr3 => {
            if capabilities.upscalers.fsr3 {
                requested
            } else {
                UpscalerSetting::NativeFallback
            }
        }
    };
    if selected != requested && requested != UpscalerSetting::Auto {
        adjustments.push(RendererSettingsAdjustment::new(
            RendererSettingKind::Upscaler,
            requested.as_str(),
            selected.as_str(),
            UnsupportedRendererSettingReason::VendorUpscalerUnavailable,
        ));
    }
    selected
}

fn vendor_upscaler_auto(capabilities: RendererCapabilityFacts) -> UpscalerSetting {
    match capabilities.adapter_vendor {
        AdapterVendor::Nvidia if capabilities.upscalers.dlss_sr => {
            UpscalerSetting::DlssSuperResolution
        }
        AdapterVendor::Amd if capabilities.upscalers.fsr3 => UpscalerSetting::Fsr3,
        _ if capabilities.upscalers.fsr3 => UpscalerSetting::Fsr3,
        _ if capabilities.upscalers.fsr2 => UpscalerSetting::Fsr2,
        _ if capabilities.upscalers.native => UpscalerSetting::NativeFallback,
        _ => UpscalerSetting::Disabled,
    }
}

fn resolve_frame_generation(
    requested: FrameGenerationSetting,
    capabilities: RendererCapabilityFacts,
    adjustments: &mut Vec<RendererSettingsAdjustment>,
) -> FrameGenerationSetting {
    let selected = match requested {
        FrameGenerationSetting::Disabled => FrameGenerationSetting::Disabled,
        FrameGenerationSetting::Auto => FrameGenerationSetting::Disabled,
        FrameGenerationSetting::DlssFrameGeneration => {
            if capabilities.frame_generation.ready()
                && capabilities.frame_generation.dlss_fg
                && !capabilities.runtime_mode.blocks_default_frame_generation()
            {
                requested
            } else {
                FrameGenerationSetting::Disabled
            }
        }
        FrameGenerationSetting::FsrFrameGeneration => {
            if capabilities.frame_generation.ready()
                && capabilities.frame_generation.fsr_fg
                && !capabilities.runtime_mode.blocks_default_frame_generation()
            {
                requested
            } else {
                FrameGenerationSetting::Disabled
            }
        }
    };
    if selected != requested && requested != FrameGenerationSetting::Auto {
        let reason = if capabilities.runtime_mode.blocks_default_frame_generation() {
            UnsupportedRendererSettingReason::FrameGenerationPolicyBlocked
        } else {
            UnsupportedRendererSettingReason::FrameGenerationUnavailable
        };
        adjustments.push(RendererSettingsAdjustment::new(
            RendererSettingKind::FrameGeneration,
            requested.as_str(),
            selected.as_str(),
            reason,
        ));
    }
    selected
}

fn resolve_shadow_quality(
    requested: ShadowQuality,
    capabilities: RendererCapabilityFacts,
    adjustments: &mut Vec<RendererSettingsAdjustment>,
) -> ShadowQuality {
    let selected = match requested {
        ShadowQuality::Simple => requested,
        ShadowQuality::VirtualLow | ShadowQuality::VirtualHigh => {
            if capabilities.compiled_features.virtual_shadows {
                requested
            } else {
                ShadowQuality::Simple
            }
        }
        ShadowQuality::RtAssisted => {
            if capabilities.ray_tracing_supported {
                requested
            } else if capabilities.compiled_features.virtual_shadows {
                ShadowQuality::VirtualHigh
            } else {
                ShadowQuality::Simple
            }
        }
    };
    if selected != requested {
        adjustments.push(RendererSettingsAdjustment::new(
            RendererSettingKind::ShadowQuality,
            requested.as_str(),
            selected.as_str(),
            if matches!(requested, ShadowQuality::RtAssisted) {
                UnsupportedRendererSettingReason::RayTracingUnavailable
            } else {
                UnsupportedRendererSettingReason::VirtualShadowsUnavailable
            },
        ));
    }
    selected
}

fn resolve_gi_quality(
    requested: GiQuality,
    capabilities: RendererCapabilityFacts,
    adjustments: &mut Vec<RendererSettingsAdjustment>,
) -> GiQuality {
    let selected = match requested {
        GiQuality::Off | GiQuality::AmbientProbe | GiQuality::ScreenSpace => requested,
        GiQuality::SurfaceCache => {
            if capabilities.compiled_features.hybrid_gi {
                requested
            } else {
                GiQuality::ScreenSpace
            }
        }
        GiQuality::RtAssisted => {
            if capabilities.ray_tracing_supported {
                requested
            } else if capabilities.compiled_features.hybrid_gi {
                GiQuality::SurfaceCache
            } else {
                GiQuality::ScreenSpace
            }
        }
        GiQuality::ExperimentalRadiance => {
            if capabilities.supports_experimental_features() {
                requested
            } else if capabilities.compiled_features.hybrid_gi {
                GiQuality::SurfaceCache
            } else {
                GiQuality::ScreenSpace
            }
        }
    };
    if selected != requested {
        let reason = match requested {
            GiQuality::RtAssisted => UnsupportedRendererSettingReason::RayTracingUnavailable,
            GiQuality::ExperimentalRadiance => {
                UnsupportedRendererSettingReason::ExperimentalFeatureUnavailable
            }
            _ => UnsupportedRendererSettingReason::HybridGiUnavailable,
        };
        adjustments.push(RendererSettingsAdjustment::new(
            RendererSettingKind::GiQuality,
            requested.as_str(),
            selected.as_str(),
            reason,
        ));
    }
    selected
}

fn resolve_reflection_quality(
    requested: ReflectionQuality,
    capabilities: RendererCapabilityFacts,
    adjustments: &mut Vec<RendererSettingsAdjustment>,
) -> ReflectionQuality {
    let selected = match requested {
        ReflectionQuality::Off | ReflectionQuality::ScreenSpace => requested,
        ReflectionQuality::SurfaceCache => {
            if capabilities.compiled_features.hybrid_gi {
                requested
            } else {
                ReflectionQuality::ScreenSpace
            }
        }
        ReflectionQuality::RtAssisted => {
            if capabilities.ray_tracing_supported {
                requested
            } else if capabilities.compiled_features.hybrid_gi {
                ReflectionQuality::SurfaceCache
            } else {
                ReflectionQuality::ScreenSpace
            }
        }
    };
    if selected != requested {
        adjustments.push(RendererSettingsAdjustment::new(
            RendererSettingKind::ReflectionQuality,
            requested.as_str(),
            selected.as_str(),
            if matches!(requested, ReflectionQuality::RtAssisted) {
                UnsupportedRendererSettingReason::RayTracingUnavailable
            } else {
                UnsupportedRendererSettingReason::HybridGiUnavailable
            },
        ));
    }
    selected
}

fn clamp_internal_budgets(
    selected_internal: &mut RendererInternalSettings,
    tier: RendererQualityTier,
) {
    let max = RendererInternalSettings::for_tier(tier);
    selected_internal.page_pool_pages = selected_internal.page_pool_pages.min(max.page_pool_pages);
    selected_internal.shadow_page_budget = selected_internal
        .shadow_page_budget
        .min(max.shadow_page_budget);
    selected_internal.light_candidate_budget = selected_internal
        .light_candidate_budget
        .min(max.light_candidate_budget);
    selected_internal.gi_cache_update_budget = selected_internal
        .gi_cache_update_budget
        .min(max.gi_cache_update_budget);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_settings_round_trip_through_json() {
        let mut request = RendererSettingsRequest::for_tier(RendererQualityTier::VendorEnhanced);
        request.user.upscaler = UpscalerSetting::DlssSuperResolution;
        request.user.diagnostics_visibility = DiagnosticsVisibility::BenchmarkCapture;

        let payload = serde_json::to_string(&request).expect("settings must serialize");
        assert!(payload.contains("\"quality_preset\":\"vendor_enhanced\""));
        assert!(payload.contains("\"upscaler\":\"dlss_super_resolution\""));

        let decoded: RendererSettingsRequest =
            serde_json::from_str(&payload).expect("settings must deserialize");
        assert_eq!(decoded, request);
    }

    #[test]
    fn quality_tier_table_covers_requested_ladder() {
        assert_eq!(
            RendererQualityTier::ORDER,
            [
                RendererQualityTier::Baseline,
                RendererQualityTier::Hybrid,
                RendererQualityTier::High,
                RendererQualityTier::RtAssisted,
                RendererQualityTier::VendorEnhanced,
                RendererQualityTier::Experimental
            ]
        );
        assert!(
            renderer_quality_tier_descriptor(RendererQualityTier::Baseline)
                .classic_dynamic_geometry
        );
        assert!(renderer_quality_tier_descriptor(RendererQualityTier::Hybrid).virtual_geometry);
        assert!(renderer_quality_tier_descriptor(RendererQualityTier::High).larger_page_budgets);
        assert!(renderer_quality_tier_descriptor(RendererQualityTier::RtAssisted).selective_rt);
        assert!(
            renderer_quality_tier_descriptor(RendererQualityTier::VendorEnhanced)
                .vendor_upscaling_or_frame_generation
        );
        assert!(
            renderer_quality_tier_descriptor(RendererQualityTier::Experimental)
                .experimental_models_or_work_graphs
        );
    }

    #[test]
    fn capability_aware_defaults_stay_safe_and_do_not_enable_frame_generation() {
        let selection = select_capability_aware_renderer_defaults(
            RendererCapabilityFacts::high_end_dx12_nvidia(),
        );

        assert_eq!(
            selection.selected_user.quality_preset,
            RendererQualityTier::High
        );
        assert_eq!(
            selection.selected_user.upscaler,
            UpscalerSetting::NativeFallback
        );
        assert_eq!(
            selection.selected_user.frame_generation,
            FrameGenerationSetting::Disabled
        );
        assert_eq!(
            selection.selected_internal.runtime_pipeline_creation,
            RuntimePipelineCreationPolicy::Forbidden
        );
        assert!(selection.product_ui_usable);
    }

    #[test]
    fn unsupported_vendor_and_rt_settings_are_rejected_with_reasons() {
        let mut capabilities = RendererCapabilityFacts::minimal();
        capabilities.compiled_features.new_core = true;
        capabilities.native_ui_gpu_transport_available = true;
        capabilities.upscalers = UpscalerCapabilities::software_only();
        let mut request = RendererSettingsRequest::for_tier(RendererQualityTier::RtAssisted);
        request.user.upscaler = UpscalerSetting::DlssSuperResolution;
        request.user.frame_generation = FrameGenerationSetting::DlssFrameGeneration;
        request.user.shadow_quality = ShadowQuality::RtAssisted;
        request.user.gi_quality = GiQuality::RtAssisted;

        let selection = resolve_renderer_settings(request, capabilities);

        assert!(selection.rejected());
        assert_eq!(
            selection.selected_user.quality_preset,
            RendererQualityTier::Baseline
        );
        assert_eq!(
            selection.selected_user.upscaler,
            UpscalerSetting::NativeFallback
        );
        assert_eq!(
            selection.selected_user.frame_generation,
            FrameGenerationSetting::Disabled
        );
        assert_eq!(
            selection.selected_user.shadow_quality,
            ShadowQuality::Simple
        );
        assert_eq!(selection.selected_user.gi_quality, GiQuality::ScreenSpace);
        assert!(
            selection.has_reason(UnsupportedRendererSettingReason::VirtualGeometryUnavailable)
                || selection.has_reason(UnsupportedRendererSettingReason::RayTracingUnavailable)
        );
        assert!(selection.has_reason(UnsupportedRendererSettingReason::VendorUpscalerUnavailable));
        assert!(selection.has_reason(UnsupportedRendererSettingReason::FrameGenerationUnavailable));
    }

    #[test]
    fn product_native_ui_gpu_transport_fails_closed_in_settings_selection() {
        let mut capabilities = RendererCapabilityFacts::high_end_dx12_nvidia();
        capabilities.native_ui_gpu_transport_available = false;

        let selection = select_capability_aware_renderer_defaults(capabilities);

        assert!(!selection.product_ui_usable);
        assert!(
            selection.has_reason(UnsupportedRendererSettingReason::NativeUiGpuTransportRequired)
        );
        assert_eq!(
            selection.selected_internal.fallback_strictness,
            FallbackStrictness::ProductFailClosed
        );
    }

    #[test]
    fn settings_capability_artifact_records_reproducible_settings() {
        let mut request = RendererSettingsRequest::for_tier(RendererQualityTier::Experimental);
        request.user.upscaler = UpscalerSetting::Auto;
        request.user.frame_generation = FrameGenerationSetting::FsrFrameGeneration;
        request.user.diagnostics_visibility = DiagnosticsVisibility::BenchmarkCapture;
        let mut capabilities = RendererCapabilityFacts::high_end_dx12_nvidia();
        capabilities.work_graphs_supported = true;

        let selection = resolve_renderer_settings(request, capabilities);
        let artifact = RendererSettingsCapabilityArtifact::from_selection(&selection);

        assert!(artifact.content.contains("schema=fun.renderer.settings.v1"));
        assert!(artifact.content.contains("quality_preset=experimental"));
        assert!(artifact.content.contains("upscaler=dlss_super_resolution"));
        assert!(
            artifact
                .content
                .contains("frame_generation=fsr_frame_generation")
        );
        assert!(artifact.content.contains("page_pool_pages=3072"));
        assert!(
            artifact
                .content
                .contains("runtime_pipeline_creation=benchmark_whitelist_only")
        );
        assert!(artifact.content.contains("capability actual_backend=dx12"));
        assert!(artifact.content.contains("product_ui_usable=true"));

        if let Ok(path) = std::env::var(RENDERER_SETTINGS_CAPABILITY_ARTIFACT_ENV) {
            write_renderer_settings_capability_artifact(path, &artifact)
                .expect("settings artifact should be writable");
        }
    }

    #[test]
    fn editor_docked_blocks_frame_generation_even_when_supported() {
        let mut request = RendererSettingsRequest::for_tier(RendererQualityTier::VendorEnhanced);
        request.user.frame_generation = FrameGenerationSetting::DlssFrameGeneration;
        let mut capabilities = RendererCapabilityFacts::high_end_dx12_nvidia();
        capabilities.runtime_mode = RendererRuntimeMode::EditorDocked;

        let selection = resolve_renderer_settings(request, capabilities);

        assert_eq!(
            selection.selected_user.frame_generation,
            FrameGenerationSetting::Disabled
        );
        assert!(
            selection.has_reason(UnsupportedRendererSettingReason::FrameGenerationPolicyBlocked)
        );
    }
}
