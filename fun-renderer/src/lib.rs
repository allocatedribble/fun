#![forbid(unsafe_code)]

pub mod api;
#[cfg(feature = "bevy_ecs")]
pub mod ecs;
pub mod frame_graph;
#[cfg(feature = "bevy_ecs")]
pub mod heuristics;
pub mod pipeline;
pub mod resource;
pub mod scene;

pub use fun_lux;
pub use fun_scene;

pub const FUN_RENDERER_SCHEMA_VERSION: u16 = 1;
pub const FUN_RENDERER_PACKAGE_NAME: &str = "fun-renderer";
pub const FUN_RENDERER_CRATE_NAME: &str = "fun_renderer";
pub const FUN_RENDER_BRIDGE_PACKAGE_NAME: &str = "fun_render";
pub const FUN_RENDERER_AI_OWNER_PACKAGE_NAME: &str = "fun-ai";
pub const FUN_RENDERER_SCENE_OWNER_PACKAGE_NAME: &str = fun_scene::FUN_SCENE_PACKAGE_NAME;
pub const FUN_RENDERER_REQUIRES_BEVY_ECS: bool = true;
pub const FUN_RENDERER_RUNTIME_BACKEND_ENV: &str = "FUN_RENDERER_BACKEND";
pub const FUN_RENDERER_BACKEND_FUTURE_DEFAULT_FLIP_LOCATION: &str =
    "fun_render::bridge::RendererBridgeSettings::from_env";
pub const FUN_RENDERER_CURRENT_AUTO_RESOLUTION: FunRendererRuntimeBackend =
    FunRendererRuntimeBackend::Legacy;

const _: () = {
    assert!(FUN_RENDERER_REQUIRES_BEVY_ECS);
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FunRendererRuntimeBackend {
    #[default]
    Auto,
    Fun,
    Legacy,
}

impl FunRendererRuntimeBackend {
    #[must_use]
    pub fn from_env() -> Self {
        Self::selection_from_env().resolved
    }

    #[must_use]
    pub fn from_env_reader<'a>(mut read: impl FnMut(&'static str) -> Option<&'a str>) -> Self {
        Self::selection_from_env_reader(|name| read(name)).resolved
    }

    #[must_use]
    pub fn selection_from_env() -> FunRendererBackendSelection {
        let value = std::env::var(FUN_RENDERER_RUNTIME_BACKEND_ENV).ok();
        Self::selection_from_env_value(value.as_deref())
    }

    #[must_use]
    pub fn selection_from_env_reader<'a>(
        mut read: impl FnMut(&'static str) -> Option<&'a str>,
    ) -> FunRendererBackendSelection {
        match read(FUN_RENDERER_RUNTIME_BACKEND_ENV) {
            Some(value) => Self::selection_from_env_value(Some(value)),
            None => FunRendererBackendSelection::default_auto(),
        }
    }

    #[must_use]
    pub fn selection_from_env_value(value: Option<&str>) -> FunRendererBackendSelection {
        match value {
            None => FunRendererBackendSelection::default_auto(),
            Some(value) => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return FunRendererBackendSelection::default_auto();
                }
                match Self::from_env_value(Some(trimmed)) {
                    Some(Self::Fun) => FunRendererBackendSelection::explicit_fun(),
                    Some(Self::Legacy) => FunRendererBackendSelection::explicit_legacy(),
                    Some(Self::Auto) => FunRendererBackendSelection::explicit_auto(),
                    None => FunRendererBackendSelection::invalid_defaulted_to_auto(),
                }
            }
        }
    }

    #[must_use]
    pub fn from_env_value(value: Option<&str>) -> Option<Self> {
        let Some(value) = value.map(str::trim) else {
            return Some(Self::Auto);
        };
        if value.is_empty() || value.eq_ignore_ascii_case("auto") {
            return Some(Self::Auto);
        }
        if value.eq_ignore_ascii_case("fun") {
            return Some(Self::Fun);
        }
        if value.eq_ignore_ascii_case("legacy") {
            return Some(Self::Legacy);
        }
        None
    }

    #[must_use]
    pub const fn as_env_value(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Fun => "fun",
            Self::Legacy => "legacy",
        }
    }

    #[must_use]
    pub const fn is_transition_only(self) -> bool {
        matches!(self, Self::Legacy)
    }

    #[must_use]
    pub const fn resolves_to_current_runtime(self) -> Self {
        match self {
            Self::Auto => FUN_RENDERER_CURRENT_AUTO_RESOLUTION,
            Self::Fun => Self::Fun,
            Self::Legacy => Self::Legacy,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunRendererBackendSelectionReason {
    DefaultAuto,
    ExplicitAuto,
    ExplicitFun,
    ExplicitLegacy,
    InvalidValueDefaultedToAuto,
}

impl FunRendererBackendSelectionReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DefaultAuto => "default_auto",
            Self::ExplicitAuto => "explicit_auto",
            Self::ExplicitFun => "explicit_fun",
            Self::ExplicitLegacy => "explicit_legacy",
            Self::InvalidValueDefaultedToAuto => "invalid_value_defaulted_to_auto",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunRendererBackendSelection {
    pub requested: FunRendererRuntimeBackend,
    pub resolved: FunRendererRuntimeBackend,
    pub reason: FunRendererBackendSelectionReason,
    pub loud_diagnostic_required: bool,
    pub future_default_flip_location: &'static str,
}

impl FunRendererBackendSelection {
    #[must_use]
    pub const fn new(
        requested: FunRendererRuntimeBackend,
        reason: FunRendererBackendSelectionReason,
    ) -> Self {
        let resolved = requested.resolves_to_current_runtime();
        Self {
            requested,
            resolved,
            reason,
            loud_diagnostic_required: !matches!(
                reason,
                FunRendererBackendSelectionReason::ExplicitFun
            ),
            future_default_flip_location: FUN_RENDERER_BACKEND_FUTURE_DEFAULT_FLIP_LOCATION,
        }
    }

    #[must_use]
    pub const fn default_auto() -> Self {
        Self::new(
            FunRendererRuntimeBackend::Auto,
            FunRendererBackendSelectionReason::DefaultAuto,
        )
    }

    #[must_use]
    pub const fn explicit_auto() -> Self {
        Self::new(
            FunRendererRuntimeBackend::Auto,
            FunRendererBackendSelectionReason::ExplicitAuto,
        )
    }

    #[must_use]
    pub const fn explicit_fun() -> Self {
        Self::new(
            FunRendererRuntimeBackend::Fun,
            FunRendererBackendSelectionReason::ExplicitFun,
        )
    }

    #[must_use]
    pub const fn explicit_legacy() -> Self {
        Self::new(
            FunRendererRuntimeBackend::Legacy,
            FunRendererBackendSelectionReason::ExplicitLegacy,
        )
    }

    #[must_use]
    pub const fn invalid_defaulted_to_auto() -> Self {
        Self::new(
            FunRendererRuntimeBackend::Auto,
            FunRendererBackendSelectionReason::InvalidValueDefaultedToAuto,
        )
    }

    #[must_use]
    pub const fn uses_legacy_product_path(self) -> bool {
        matches!(self.resolved, FunRendererRuntimeBackend::Legacy)
    }

    #[must_use]
    pub const fn uses_fun_renderer_core(self) -> bool {
        matches!(self.resolved, FunRendererRuntimeBackend::Fun)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunRendererSubsystem {
    RendererCore,
    VirtualGeometry,
    VirtualShadows,
    GpuSceneDatabase,
    FrameGraph,
    PageScheduler,
    CefCompositor,
    UpscalingFrameGeneration,
    BackendAbstraction,
    Lighting,
}

impl FunRendererSubsystem {
    pub const ALL: [Self; 10] = [
        Self::RendererCore,
        Self::VirtualGeometry,
        Self::VirtualShadows,
        Self::GpuSceneDatabase,
        Self::FrameGraph,
        Self::PageScheduler,
        Self::CefCompositor,
        Self::UpscalingFrameGeneration,
        Self::BackendAbstraction,
        Self::Lighting,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RendererCore => "renderer_core",
            Self::VirtualGeometry => "virtual_geometry",
            Self::VirtualShadows => "virtual_shadows",
            Self::GpuSceneDatabase => "gpu_scene_database",
            Self::FrameGraph => "frame_graph",
            Self::PageScheduler => "page_scheduler",
            Self::CefCompositor => "cef_compositor",
            Self::UpscalingFrameGeneration => "upscaling_frame_generation",
            Self::BackendAbstraction => "backend_abstraction",
            Self::Lighting => "lighting",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunRendererOwner {
    FunRenderer,
    Lux,
    FunAi,
    FunRenderBridge,
    BevyLowLevel,
}

impl FunRendererOwner {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FunRenderer => FUN_RENDERER_CRATE_NAME,
            Self::Lux => fun_lux::FUN_LUX_CRATE_NAME,
            Self::FunAi => FUN_RENDERER_AI_OWNER_PACKAGE_NAME,
            Self::FunRenderBridge => FUN_RENDER_BRIDGE_PACKAGE_NAME,
            Self::BevyLowLevel => "bevy_low_level",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunRendererBevyRole {
    None,
    EcsExtractionBridge,
    MinimalPrimitiveSource,
    LowLevelBackendHook,
}

impl FunRendererBevyRole {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::EcsExtractionBridge => "ecs_extraction_bridge",
            Self::MinimalPrimitiveSource => "minimal_primitive_source",
            Self::LowLevelBackendHook => "low_level_backend_hook",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunRendererSubsystemDescriptor {
    pub stable_id: &'static str,
    pub subsystem: FunRendererSubsystem,
    pub owner: FunRendererOwner,
    pub bevy_role: FunRendererBevyRole,
    pub default_renderer_core: bool,
    pub reusable_model_runtime_allowed: bool,
}

pub const FUN_RENDERER_SUBSYSTEM_DESCRIPTORS: [FunRendererSubsystemDescriptor; 10] = [
    FunRendererSubsystemDescriptor {
        stable_id: "fun_renderer.subsystem.renderer_core",
        subsystem: FunRendererSubsystem::RendererCore,
        owner: FunRendererOwner::FunRenderer,
        bevy_role: FunRendererBevyRole::None,
        default_renderer_core: true,
        reusable_model_runtime_allowed: false,
    },
    FunRendererSubsystemDescriptor {
        stable_id: "fun_renderer.subsystem.virtual_geometry",
        subsystem: FunRendererSubsystem::VirtualGeometry,
        owner: FunRendererOwner::FunRenderer,
        bevy_role: FunRendererBevyRole::MinimalPrimitiveSource,
        default_renderer_core: true,
        reusable_model_runtime_allowed: false,
    },
    FunRendererSubsystemDescriptor {
        stable_id: "fun_renderer.subsystem.virtual_shadows",
        subsystem: FunRendererSubsystem::VirtualShadows,
        owner: FunRendererOwner::FunRenderer,
        bevy_role: FunRendererBevyRole::LowLevelBackendHook,
        default_renderer_core: true,
        reusable_model_runtime_allowed: false,
    },
    FunRendererSubsystemDescriptor {
        stable_id: "fun_renderer.subsystem.gpu_scene_database",
        subsystem: FunRendererSubsystem::GpuSceneDatabase,
        owner: FunRendererOwner::FunRenderer,
        bevy_role: FunRendererBevyRole::EcsExtractionBridge,
        default_renderer_core: true,
        reusable_model_runtime_allowed: false,
    },
    FunRendererSubsystemDescriptor {
        stable_id: "fun_renderer.subsystem.frame_graph",
        subsystem: FunRendererSubsystem::FrameGraph,
        owner: FunRendererOwner::FunRenderer,
        bevy_role: FunRendererBevyRole::LowLevelBackendHook,
        default_renderer_core: true,
        reusable_model_runtime_allowed: false,
    },
    FunRendererSubsystemDescriptor {
        stable_id: "fun_renderer.subsystem.page_scheduler",
        subsystem: FunRendererSubsystem::PageScheduler,
        owner: FunRendererOwner::FunRenderer,
        bevy_role: FunRendererBevyRole::None,
        default_renderer_core: true,
        reusable_model_runtime_allowed: false,
    },
    FunRendererSubsystemDescriptor {
        stable_id: "fun_renderer.subsystem.cef_compositor",
        subsystem: FunRendererSubsystem::CefCompositor,
        owner: FunRendererOwner::FunRenderer,
        bevy_role: FunRendererBevyRole::LowLevelBackendHook,
        default_renderer_core: true,
        reusable_model_runtime_allowed: false,
    },
    FunRendererSubsystemDescriptor {
        stable_id: "fun_renderer.subsystem.upscaling_frame_generation",
        subsystem: FunRendererSubsystem::UpscalingFrameGeneration,
        owner: FunRendererOwner::FunRenderer,
        bevy_role: FunRendererBevyRole::LowLevelBackendHook,
        default_renderer_core: true,
        reusable_model_runtime_allowed: false,
    },
    FunRendererSubsystemDescriptor {
        stable_id: "fun_renderer.subsystem.backend_abstraction",
        subsystem: FunRendererSubsystem::BackendAbstraction,
        owner: FunRendererOwner::FunRenderer,
        bevy_role: FunRendererBevyRole::LowLevelBackendHook,
        default_renderer_core: true,
        reusable_model_runtime_allowed: false,
    },
    FunRendererSubsystemDescriptor {
        stable_id: "fun_renderer.subsystem.lighting",
        subsystem: FunRendererSubsystem::Lighting,
        owner: FunRendererOwner::Lux,
        bevy_role: FunRendererBevyRole::LowLevelBackendHook,
        default_renderer_core: true,
        reusable_model_runtime_allowed: false,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunRendererUiRuntimePolicy {
    pub cef_svelte_is_product_ui: bool,
    pub bevy_ui_runtime_product_allowed: bool,
    pub bevy_ui_test_only_allowed: bool,
    pub launcher_ui_owner: &'static str,
    pub editor_ui_owner: &'static str,
    pub game_hud_owner: &'static str,
    pub diagnostics_panel_owner: &'static str,
    pub debug_overlay_owner: &'static str,
}

pub const FUN_RENDERER_UI_RUNTIME_POLICY: FunRendererUiRuntimePolicy = FunRendererUiRuntimePolicy {
    cef_svelte_is_product_ui: true,
    bevy_ui_runtime_product_allowed: false,
    bevy_ui_test_only_allowed: true,
    launcher_ui_owner: "cef_svelte",
    editor_ui_owner: "cef_svelte",
    game_hud_owner: "cef_svelte",
    diagnostics_panel_owner: "cef_svelte",
    debug_overlay_owner: "cef_svelte",
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunRendererCefRuntimePolicy {
    pub gpu_shared_texture_required: bool,
    pub cpu_on_paint_runtime_fallback_allowed: bool,
    pub cpu_golden_fixture_test_only_allowed: bool,
    pub failure_mode: &'static str,
}

pub const FUN_RENDERER_CEF_RUNTIME_POLICY: FunRendererCefRuntimePolicy =
    FunRendererCefRuntimePolicy {
        gpu_shared_texture_required: true,
        cpu_on_paint_runtime_fallback_allowed: false,
        cpu_golden_fixture_test_only_allowed: true,
        failure_mode: "fail_ui_subsystem_with_clear_diagnostic",
    };

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunRendererPresentationFeature {
    DlssSuperResolution,
    FsrSuperResolution,
    FrameGeneration,
}

impl FunRendererPresentationFeature {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DlssSuperResolution => "dlss_super_resolution",
            Self::FsrSuperResolution => "fsr_super_resolution",
            Self::FrameGeneration => "frame_generation",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunRendererPresentationFeatureDescriptor {
    pub stable_id: &'static str,
    pub feature: FunRendererPresentationFeature,
    pub owner: FunRendererOwner,
    pub scene_color_ui_color_separate: bool,
}

pub const FUN_RENDERER_PRESENTATION_FEATURE_DESCRIPTORS:
    [FunRendererPresentationFeatureDescriptor; 3] = [
    FunRendererPresentationFeatureDescriptor {
        stable_id: "fun_renderer.presentation.dlss_super_resolution",
        feature: FunRendererPresentationFeature::DlssSuperResolution,
        owner: FunRendererOwner::FunRenderer,
        scene_color_ui_color_separate: true,
    },
    FunRendererPresentationFeatureDescriptor {
        stable_id: "fun_renderer.presentation.fsr_super_resolution",
        feature: FunRendererPresentationFeature::FsrSuperResolution,
        owner: FunRendererOwner::FunRenderer,
        scene_color_ui_color_separate: true,
    },
    FunRendererPresentationFeatureDescriptor {
        stable_id: "fun_renderer.presentation.frame_generation",
        feature: FunRendererPresentationFeature::FrameGeneration,
        owner: FunRendererOwner::FunRenderer,
        scene_color_ui_color_separate: true,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunRendererFrameGenerationContract {
    pub hud_less_scene_color_required: bool,
    pub ui_color_required: bool,
    pub depth_required: bool,
    pub motion_vectors_required: bool,
    pub present_time_resource_lifetimes_required: bool,
    pub editor_viewports_sr_capable_where_supported: bool,
    pub editor_viewports_fg_capable_where_supported: bool,
    pub cef_text_readability_required: bool,
}

pub const FUN_RENDERER_FRAME_GENERATION_CONTRACT: FunRendererFrameGenerationContract =
    FunRendererFrameGenerationContract {
        hud_less_scene_color_required: true,
        ui_color_required: true,
        depth_required: true,
        motion_vectors_required: true,
        present_time_resource_lifetimes_required: true,
        editor_viewports_sr_capable_where_supported: true,
        editor_viewports_fg_capable_where_supported: true,
        cef_text_readability_required: true,
    };

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunRendererLightingScalePolicy {
    pub artistically_unbounded_budget_managed: bool,
    pub brute_force_lights_times_pixels_allowed: bool,
    pub tiled_clustered_light_bins_required: bool,
    pub reservoir_sampling_required: bool,
    pub temporal_spatial_reuse_required: bool,
    pub emissive_candidate_promotion_required: bool,
    pub virtual_shadow_demand_pages_required: bool,
}

pub const FUN_RENDERER_LIGHTING_SCALE_POLICY: FunRendererLightingScalePolicy =
    FunRendererLightingScalePolicy {
        artistically_unbounded_budget_managed: true,
        brute_force_lights_times_pixels_allowed: false,
        tiled_clustered_light_bins_required: true,
        reservoir_sampling_required: true,
        temporal_spatial_reuse_required: true,
        emissive_candidate_promotion_required: true,
        virtual_shadow_demand_pages_required: true,
    };

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunRendererDynamicSceneTarget {
    pub streaming: bool,
    pub dynamic_scene_mutation: bool,
    pub world_scale: bool,
    pub destruction: bool,
    pub runtime_procedural_geometry: bool,
    pub high_light_counts: bool,
    pub many_moving_occluders: bool,
    pub editor_mode_changes: bool,
    pub p95_p99_stability: bool,
}

pub const FUN_RENDERER_DYNAMIC_SCENE_TARGET: FunRendererDynamicSceneTarget =
    FunRendererDynamicSceneTarget {
        streaming: true,
        dynamic_scene_mutation: true,
        world_scale: true,
        destruction: true,
        runtime_procedural_geometry: true,
        high_light_counts: true,
        many_moving_occluders: true,
        editor_mode_changes: true,
        p95_p99_stability: true,
    };

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunRendererBackend {
    Dx12,
    Vulkan,
}

impl FunRendererBackend {
    pub const ALL: [Self; 2] = [Self::Dx12, Self::Vulkan];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dx12 => "dx12",
            Self::Vulkan => "vulkan",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunRendererBackendDescriptor {
    pub backend: FunRendererBackend,
    pub stable_id: &'static str,
    pub capability_reporting_required: bool,
    pub native_handle_access_owner: FunRendererOwner,
}

pub const FUN_RENDERER_BACKEND_DESCRIPTORS: [FunRendererBackendDescriptor; 2] = [
    FunRendererBackendDescriptor {
        backend: FunRendererBackend::Dx12,
        stable_id: "fun_renderer.backend.dx12",
        capability_reporting_required: true,
        native_handle_access_owner: FunRendererOwner::FunRenderer,
    },
    FunRendererBackendDescriptor {
        backend: FunRendererBackend::Vulkan,
        stable_id: "fun_renderer.backend.vulkan",
        capability_reporting_required: true,
        native_handle_access_owner: FunRendererOwner::FunRenderer,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunRendererFrameGraphStage {
    GpuSceneDatabase,
    VirtualGeometry,
    VirtualShadows,
    Lighting,
    UpscalingFrameGeneration,
    CefComposition,
    Present,
}

impl FunRendererFrameGraphStage {
    pub const ORDER: [Self; 7] = [
        Self::GpuSceneDatabase,
        Self::VirtualGeometry,
        Self::VirtualShadows,
        Self::Lighting,
        Self::UpscalingFrameGeneration,
        Self::CefComposition,
        Self::Present,
    ];

    #[must_use]
    pub const fn order_key(self) -> u16 {
        match self {
            Self::GpuSceneDatabase => 10,
            Self::VirtualGeometry => 20,
            Self::VirtualShadows => 30,
            Self::Lighting => 40,
            Self::UpscalingFrameGeneration => 50,
            Self::CefComposition => 60,
            Self::Present => 70,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GpuSceneDatabase => "gpu_scene_database",
            Self::VirtualGeometry => "virtual_geometry",
            Self::VirtualShadows => "virtual_shadows",
            Self::Lighting => "lighting",
            Self::UpscalingFrameGeneration => "upscaling_frame_generation",
            Self::CefComposition => "cef_composition",
            Self::Present => "present",
        }
    }

    #[must_use]
    pub const fn feeds_temporal_reconstruction(self) -> bool {
        matches!(
            self,
            Self::VirtualGeometry | Self::VirtualShadows | Self::Lighting
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunRendererAiOwnedSurface {
    ModelRegistry,
    ModelManifests,
    InferenceBackendSelection,
    Evals,
    ModelTrustVersioning,
    OfflineTrainingEvaluationHooks,
}

impl FunRendererAiOwnedSurface {
    pub const ALL: [Self; 6] = [
        Self::ModelRegistry,
        Self::ModelManifests,
        Self::InferenceBackendSelection,
        Self::Evals,
        Self::ModelTrustVersioning,
        Self::OfflineTrainingEvaluationHooks,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ModelRegistry => "model_registry",
            Self::ModelManifests => "model_manifests",
            Self::InferenceBackendSelection => "inference_backend_selection",
            Self::Evals => "evals",
            Self::ModelTrustVersioning => "model_trust_versioning",
            Self::OfflineTrainingEvaluationHooks => "offline_training_evaluation_hooks",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunRendererAiInterfaceKind {
    TensorInputSchema,
    TensorOutputSchema,
    GpuResourceHandle,
    FallbackHeuristicPath,
}

impl FunRendererAiInterfaceKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TensorInputSchema => "tensor_input_schema",
            Self::TensorOutputSchema => "tensor_output_schema",
            Self::GpuResourceHandle => "gpu_resource_handle",
            Self::FallbackHeuristicPath => "fallback_heuristic_path",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunRendererAiInterfaceDescriptor {
    pub stable_id: &'static str,
    pub kind: FunRendererAiInterfaceKind,
    pub owner: FunRendererOwner,
    pub reusable_model_runtime_allowed: bool,
}

pub const FUN_RENDERER_AI_INTERFACE_DESCRIPTORS: [FunRendererAiInterfaceDescriptor; 4] = [
    FunRendererAiInterfaceDescriptor {
        stable_id: "fun_renderer.ai_interface.tensor_input_schema",
        kind: FunRendererAiInterfaceKind::TensorInputSchema,
        owner: FunRendererOwner::FunRenderer,
        reusable_model_runtime_allowed: false,
    },
    FunRendererAiInterfaceDescriptor {
        stable_id: "fun_renderer.ai_interface.tensor_output_schema",
        kind: FunRendererAiInterfaceKind::TensorOutputSchema,
        owner: FunRendererOwner::FunRenderer,
        reusable_model_runtime_allowed: false,
    },
    FunRendererAiInterfaceDescriptor {
        stable_id: "fun_renderer.ai_interface.gpu_resource_handle",
        kind: FunRendererAiInterfaceKind::GpuResourceHandle,
        owner: FunRendererOwner::FunRenderer,
        reusable_model_runtime_allowed: false,
    },
    FunRendererAiInterfaceDescriptor {
        stable_id: "fun_renderer.ai_interface.fallback_heuristic_path",
        kind: FunRendererAiInterfaceKind::FallbackHeuristicPath,
        owner: FunRendererOwner::FunRenderer,
        reusable_model_runtime_allowed: false,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunRendererProductTopology {
    pub renderer_package: &'static str,
    pub renderer_crate: &'static str,
    pub scene_package: &'static str,
    pub scene_crate: &'static str,
    pub lighting_package: &'static str,
    pub lighting_crate: &'static str,
    pub bevy_bridge_package: &'static str,
    pub ai_owner_package: &'static str,
}

pub const FUN_RENDERER_PRODUCT_TOPOLOGY: FunRendererProductTopology = FunRendererProductTopology {
    renderer_package: FUN_RENDERER_PACKAGE_NAME,
    renderer_crate: FUN_RENDERER_CRATE_NAME,
    scene_package: fun_scene::FUN_SCENE_PACKAGE_NAME,
    scene_crate: fun_scene::FUN_SCENE_CRATE_NAME,
    lighting_package: fun_lux::FUN_LUX_PACKAGE_NAME,
    lighting_crate: fun_lux::FUN_LUX_CRATE_NAME,
    bevy_bridge_package: FUN_RENDER_BRIDGE_PACKAGE_NAME,
    ai_owner_package: FUN_RENDERER_AI_OWNER_PACKAGE_NAME,
};

pub use api::*;
#[cfg(feature = "bevy_ecs")]
pub use ecs::*;
pub use frame_graph::*;
#[cfg(feature = "bevy_ecs")]
pub use heuristics::*;
pub use pipeline::*;
pub use resource::*;
pub use scene::*;

#[must_use]
pub const fn owner_for_subsystem(subsystem: FunRendererSubsystem) -> FunRendererOwner {
    match subsystem {
        FunRendererSubsystem::Lighting => FunRendererOwner::Lux,
        FunRendererSubsystem::RendererCore
        | FunRendererSubsystem::VirtualGeometry
        | FunRendererSubsystem::VirtualShadows
        | FunRendererSubsystem::GpuSceneDatabase
        | FunRendererSubsystem::FrameGraph
        | FunRendererSubsystem::PageScheduler
        | FunRendererSubsystem::CefCompositor
        | FunRendererSubsystem::UpscalingFrameGeneration
        | FunRendererSubsystem::BackendAbstraction => FunRendererOwner::FunRenderer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topology_names_match_package_decisions() {
        assert_eq!(
            FUN_RENDERER_PRODUCT_TOPOLOGY.renderer_package,
            "fun-renderer"
        );
        assert_eq!(FUN_RENDERER_PRODUCT_TOPOLOGY.renderer_crate, "fun_renderer");
        assert_eq!(FUN_RENDERER_PRODUCT_TOPOLOGY.scene_package, "fun-scene");
        assert_eq!(FUN_RENDERER_PRODUCT_TOPOLOGY.scene_crate, "fun_scene");
        assert_eq!(FUN_RENDERER_PRODUCT_TOPOLOGY.lighting_package, "fun-lux");
        assert_eq!(FUN_RENDERER_PRODUCT_TOPOLOGY.lighting_crate, "fun_lux");
        assert_eq!(
            FUN_RENDERER_PRODUCT_TOPOLOGY.bevy_bridge_package,
            "fun_render"
        );
        assert_eq!(FUN_RENDERER_PRODUCT_TOPOLOGY.ai_owner_package, "fun-ai");
    }

    #[test]
    fn runtime_backend_selection_is_loud_until_fun_default_flip() {
        let default_selection = FunRendererRuntimeBackend::selection_from_env_reader(|_| None);
        assert_eq!(default_selection.requested, FunRendererRuntimeBackend::Auto);
        assert_eq!(
            default_selection.resolved,
            FunRendererRuntimeBackend::Legacy
        );
        assert_eq!(
            default_selection.reason,
            FunRendererBackendSelectionReason::DefaultAuto
        );
        assert!(default_selection.loud_diagnostic_required);
        assert_eq!(
            default_selection.future_default_flip_location,
            FUN_RENDERER_BACKEND_FUTURE_DEFAULT_FLIP_LOCATION
        );

        assert_eq!(
            FunRendererRuntimeBackend::from_env_reader(|_| None),
            FunRendererRuntimeBackend::Legacy
        );
        assert_eq!(
            FunRendererRuntimeBackend::selection_from_env_reader(|name| {
                (name == FUN_RENDERER_RUNTIME_BACKEND_ENV).then_some("legacy")
            })
            .resolved,
            FunRendererRuntimeBackend::Legacy
        );
        let fun_selection = FunRendererRuntimeBackend::selection_from_env_reader(|name| {
            (name == FUN_RENDERER_RUNTIME_BACKEND_ENV).then_some("fun")
        });
        assert_eq!(fun_selection.resolved, FunRendererRuntimeBackend::Fun);
        assert!(!fun_selection.loud_diagnostic_required);
        assert_eq!(FunRendererRuntimeBackend::Auto.as_env_value(), "auto");
        assert!(FunRendererRuntimeBackend::Legacy.is_transition_only());
        assert!(!FunRendererRuntimeBackend::Fun.is_transition_only());
        assert_eq!(FunRendererRuntimeBackend::Fun.as_env_value(), "fun");
    }

    #[test]
    fn product_ui_policy_prohibits_runtime_bevy_ui() {
        let policy = core::hint::black_box(FUN_RENDERER_UI_RUNTIME_POLICY);

        assert!(policy.cef_svelte_is_product_ui);
        assert!(!policy.bevy_ui_runtime_product_allowed);
        assert!(policy.bevy_ui_test_only_allowed);
        assert_eq!(policy.game_hud_owner, "cef_svelte");
    }

    #[test]
    fn cef_runtime_policy_is_gpu_only_for_product_lanes() {
        let policy = core::hint::black_box(FUN_RENDERER_CEF_RUNTIME_POLICY);

        assert!(policy.gpu_shared_texture_required);
        assert!(!policy.cpu_on_paint_runtime_fallback_allowed);
        assert!(policy.cpu_golden_fixture_test_only_allowed);
        assert_eq!(
            policy.failure_mode,
            "fail_ui_subsystem_with_clear_diagnostic"
        );
    }

    #[test]
    fn presentation_features_keep_scene_and_ui_color_separate() {
        for descriptor in FUN_RENDERER_PRESENTATION_FEATURE_DESCRIPTORS {
            assert_eq!(descriptor.owner, FunRendererOwner::FunRenderer);
            assert!(descriptor.scene_color_ui_color_separate);
        }

        let contract = core::hint::black_box(FUN_RENDERER_FRAME_GENERATION_CONTRACT);

        assert!(contract.hud_less_scene_color_required);
        assert!(contract.ui_color_required);
        assert!(contract.depth_required);
        assert!(contract.motion_vectors_required);
        assert!(contract.present_time_resource_lifetimes_required);
        assert!(contract.editor_viewports_sr_capable_where_supported);
        assert!(contract.editor_viewports_fg_capable_where_supported);
        assert!(contract.cef_text_readability_required);
    }

    #[test]
    fn lighting_policy_is_budget_managed_not_brute_force() {
        let policy = core::hint::black_box(FUN_RENDERER_LIGHTING_SCALE_POLICY);

        assert!(policy.artistically_unbounded_budget_managed);
        assert!(!policy.brute_force_lights_times_pixels_allowed);
        assert!(policy.tiled_clustered_light_bins_required);
        assert!(policy.reservoir_sampling_required);
        assert!(policy.temporal_spatial_reuse_required);
        assert!(policy.emissive_candidate_promotion_required);
        assert!(policy.virtual_shadow_demand_pages_required);
    }

    #[test]
    fn dynamic_scene_target_matches_massive_procedural_runtime_goal() {
        let target = core::hint::black_box(FUN_RENDERER_DYNAMIC_SCENE_TARGET);

        assert!(target.streaming);
        assert!(target.dynamic_scene_mutation);
        assert!(target.world_scale);
        assert!(target.destruction);
        assert!(target.runtime_procedural_geometry);
        assert!(target.high_light_counts);
        assert!(target.many_moving_occluders);
        assert!(target.editor_mode_changes);
        assert!(target.p95_p99_stability);
    }

    #[test]
    fn renderer_subsystem_ownership_keeps_lighting_in_fun_lux() {
        assert_eq!(FunRendererSubsystem::ALL.len(), 10);

        for descriptor in FUN_RENDERER_SUBSYSTEM_DESCRIPTORS {
            assert_eq!(descriptor.owner, owner_for_subsystem(descriptor.subsystem));
            assert!(!descriptor.reusable_model_runtime_allowed);
        }

        assert_eq!(
            owner_for_subsystem(FunRendererSubsystem::Lighting),
            FunRendererOwner::Lux
        );
        assert_eq!(
            owner_for_subsystem(FunRendererSubsystem::CefCompositor),
            FunRendererOwner::FunRenderer
        );
    }

    #[test]
    fn backend_contract_is_dx12_and_vulkan_only() {
        assert_eq!(
            FunRendererBackend::ALL,
            [FunRendererBackend::Dx12, FunRendererBackend::Vulkan]
        );

        for descriptor in FUN_RENDERER_BACKEND_DESCRIPTORS {
            assert!(descriptor.capability_reporting_required);
            assert_eq!(
                descriptor.native_handle_access_owner,
                FunRendererOwner::FunRenderer
            );
        }
    }

    #[test]
    fn frame_graph_keeps_cef_after_reconstruction() {
        let mut previous_key = 0;
        for stage in FunRendererFrameGraphStage::ORDER {
            assert!(stage.order_key() > previous_key, "{}", stage.as_str());
            previous_key = stage.order_key();
        }

        assert!(
            FunRendererFrameGraphStage::CefComposition.order_key()
                > FunRendererFrameGraphStage::UpscalingFrameGeneration.order_key()
        );
        assert!(!FunRendererFrameGraphStage::CefComposition.feeds_temporal_reconstruction());
    }

    #[test]
    fn renderer_ai_interfaces_do_not_embed_model_runtime() {
        assert_eq!(FunRendererAiOwnedSurface::ALL.len(), 6);

        for descriptor in FUN_RENDERER_AI_INTERFACE_DESCRIPTORS {
            assert_eq!(descriptor.owner, FunRendererOwner::FunRenderer);
            assert!(!descriptor.reusable_model_runtime_allowed);
        }
    }
}
