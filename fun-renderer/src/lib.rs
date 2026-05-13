#![forbid(unsafe_code)]

pub mod api;
#[cfg(any())]
pub mod asset_prep;
pub mod backend;
pub mod benchmark;
pub mod binding;
// Pass C0 / C1 — typed cloud renderer ownership.  The
// typed product cloud renderer is owned by `fun-renderer`;
// the typed `fun_render::sky` module stays as a typed
// bridge / migration donor that extracts settings,
// weather, and signals from the typed RetiredEngine app world.
#[cfg(feature = "wgpu_bridge")]
pub mod bridge;
#[cfg(any())]
pub mod cloud_diagnostics;
#[cfg(any())]
pub mod cloud_executor;
#[cfg(any())]
pub mod cloud_gpu_resource_set;
#[cfg(any())]
pub mod cloud_passes;
#[cfg(any())]
pub mod cloud_receive_lux_lighting;
#[cfg(any())]
pub mod cloud_resources;
#[cfg(any())]
pub mod cloud_shaders;
#[cfg(any())]
pub mod cloud_shadow;
#[cfg(any())]
pub mod cloud_shadow_director;
#[cfg(any())]
pub mod cloud_shadow_dispatch_feedback;
#[cfg(any())]
pub mod cloud_shadow_golden_scenes;
#[cfg(any())]
pub mod cloud_shadow_live_executor;
#[cfg(any())]
pub mod cloud_shadow_look_tuning;
#[cfg(any())]
pub mod cloud_shadow_passes;
#[cfg(any())]
pub mod cloud_shadow_pipelines;
#[cfg(any())]
pub mod cloud_shadow_runtime_diagnostics;
#[cfg(any())]
pub mod cloud_shadow_runtime_probe;
#[cfg(any())]
pub mod clouds;
#[cfg(feature = "fun_ecs")]
pub mod component_api;
#[cfg(feature = "scene_contract")]
pub mod default_flip;
#[cfg(any())]
pub mod dx12_production;
#[cfg(feature = "scene_contract")]
pub mod dynamic_geometry;
#[cfg(any())]
pub mod ecs;
pub mod ecs_handoff;
#[cfg(any())]
pub mod exposure_pass;
#[cfg(any())]
pub mod extraction;
pub mod frame_generation;
pub mod frame_graph;
#[cfg(any())]
pub mod fun_render_cloud_retirement;
#[cfg(any())]
pub mod fun_render_route_audit;
pub mod gpu_driven;
#[cfg(any())]
pub mod gpu_driven_runtime;
#[cfg(any())]
pub mod hdr_pipeline;
#[cfg(any())]
pub mod heuristics;
pub mod ir;
#[cfg(any())]
pub mod lighting_stack;
#[cfg(any())]
pub mod live_proof_frame_executor;
#[cfg(any())]
pub mod lux_diagnostics;
#[cfg(any())]
pub mod lux_direct_lighting_cloud_layer;
#[cfg(any())]
pub mod lux_direct_lighting_cloud_shader;
#[cfg(any())]
pub mod lux_graph;
#[cfg(any())]
pub mod lux_live_lighting;
#[cfg(any())]
pub mod lux_material_cloud_layer;
#[cfg(any())]
pub mod lux_material_pbr_cloud_shader;
#[cfg(any())]
pub mod lux_passes;
#[cfg(any())]
pub mod lux_resources;
#[cfg(any())]
pub mod lux_shadow_aux_layer;
#[cfg(any())]
pub mod lux_volumetric_cloud_layer;
#[cfg(any())]
pub mod lux_volumetric_executor;
#[cfg(any())]
pub mod lux_volumetric_light_inject_cloud_shader;
#[cfg(feature = "experimental_renderer_ml")]
pub mod ml;
pub mod page;
#[cfg(feature = "schedule_contract")]
pub mod page_scheduler;
pub mod parity;
#[cfg(any())]
pub mod passb_proof_frame_runtime;
#[cfg(any())]
pub mod passc_runtime_cache_burndown;
#[cfg(any())]
pub mod passd_gpu_scene_indirect_draw;
#[cfg(any())]
pub mod passe_clustered_lighting_virtual_shadow_mvp;
#[cfg(any())]
pub mod passf_temporal_stack;
#[cfg(any())]
pub mod passg_native_ui_product_route;
#[cfg(any())]
pub mod passh_native_command_list_fail_closed;
#[cfg(any())]
pub mod passi_gpu_driven_compute_indirect;
#[cfg(any())]
pub mod passj_clustered_lighting_live;
#[cfg(any())]
pub mod passk_temporal_reconstruction_live;
#[cfg(any())]
pub mod passl_native_ui_live;
#[cfg(any())]
pub mod passm_windowed_surface_present;
pub mod pipeline;
#[cfg(any())]
pub mod plugin;
#[cfg(any())]
pub mod post_process;
#[cfg(any())]
pub mod presentation;
#[cfg(any())]
pub mod presentation_stack;
#[cfg(any())]
pub mod proof_scene;
#[cfg(any())]
pub mod quality_audit_contract;
pub mod queue_scheduler;
#[cfg(feature = "schedule_graph")]
pub mod render_work_graph;
#[cfg(feature = "fun_ecs")]
pub mod renderer_integration;
#[cfg(feature = "scene_contract")]
pub mod research;
pub mod resource;
#[cfg(feature = "scene_contract")]
pub mod scene;
#[cfg(any())]
pub mod scene_streaming;
#[cfg(feature = "schedule_contract")]
pub mod schedule_contract;
pub mod scheduler;
pub mod settings;
pub mod shader;
#[cfg(any())]
pub mod taa;
#[cfg(any())]
pub mod tier0_proof_frame_gate;
#[cfg(any())]
pub mod tier1_cache_backed_optimization;
#[cfg(any())]
pub mod tier2_gpu_driven_proof;
#[cfg(any())]
pub mod tier3_lighting_shadows_at_scale;
#[cfg(any())]
pub mod tier4_transient_memory_and_barriers;
#[cfg(any())]
pub mod tier5_temporal_reconstruction;
#[cfg(any())]
pub mod tier6_native_ui_rendering;
#[cfg(any())]
pub mod tier7_vendor_sdks_and_frame_generation;
#[cfg(any())]
pub mod tier8_direct_backend_experiments;
#[cfg(any())]
pub mod tonemap_pass;
pub mod ui;
pub mod upscaling;
#[cfg(any())]
pub mod vendor_sdk_bridge;
pub mod virtual_geometry;
pub mod virtual_shadow;

pub use ecs_handoff::*;
pub use fun_ecs;
#[cfg(feature = "scene_contract")]
pub use fun_lux;
#[cfg(feature = "scene_contract")]
pub use fun_scene;
pub use parity::*;

pub const FUN_RENDERER_SCHEMA_VERSION: u16 = 1;
pub const FUN_RENDERER_PACKAGE_NAME: &str = "fun-renderer";
pub const FUN_RENDERER_CRATE_NAME: &str = "fun_renderer";
pub const FUN_RENDER_DONOR_PACKAGE_NAME: &str = "fun_render";
pub const FUN_RENDERER_AI_OWNER_PACKAGE_NAME: &str = "fun-ai";
pub const FUN_RENDERER_SCENE_OWNER_PACKAGE_NAME: &str = "fun-scene";
pub const FUN_RENDERER_SCENE_OWNER_CRATE_NAME: &str = "fun_scene";
pub const FUN_RENDERER_LIGHTING_OWNER_PACKAGE_NAME: &str = "fun-lux";
pub const FUN_RENDERER_LIGHTING_OWNER_CRATE_NAME: &str = "fun_lux";
pub const FUN_RENDERER_REQUIRES_RETIRED_ENGINE_ECS: bool = false;
pub const FUN_RENDERER_RUNTIME_BACKEND_ENV: &str = "FUN_RENDERER_BACKEND";
pub const FUN_RENDERER_BACKEND_FUTURE_DEFAULT_FLIP_LOCATION: &str = "fun-engine::RendererModule";
pub const FUN_RENDERER_CURRENT_AUTO_RESOLUTION: FunRendererRuntimeBackend =
    FunRendererRuntimeBackend::Fun;

const _: () = {
    assert!(!FUN_RENDERER_REQUIRES_RETIRED_ENGINE_ECS);
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
    pub fn from_env_reader<'a>(read: impl FnMut(&'static str) -> Option<&'a str>) -> Self {
        Self::selection_from_env_reader(read).resolved
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
            loud_diagnostic_required: matches!(
                reason,
                FunRendererBackendSelectionReason::ExplicitLegacy
                    | FunRendererBackendSelectionReason::InvalidValueDefaultedToAuto
            ) || matches!(resolved, FunRendererRuntimeBackend::Legacy),
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
    RenderArtifactRealization,
    /// Pass 20+ native rvelte/FUN UI composition is the product UI
    /// composition path. Legacy NATIVE_UI composition is demoted to
    /// `NativeUiRenderRoleStatus::DemotedToLegacyDiagnostic` and rendered
    /// via the same subsystem identity for legacy/diagnostic lanes
    /// only.
    UiComposition,
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
        Self::RenderArtifactRealization,
        Self::UiComposition,
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
            Self::RenderArtifactRealization => "render_artifact_realization",
            Self::UiComposition => "ui_composition",
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
    RetiredEngineLowLevel,
}

impl FunRendererOwner {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FunRenderer => FUN_RENDERER_CRATE_NAME,
            Self::Lux => FUN_RENDERER_LIGHTING_OWNER_CRATE_NAME,
            Self::FunAi => FUN_RENDERER_AI_OWNER_PACKAGE_NAME,
            Self::FunRenderBridge => FUN_RENDER_DONOR_PACKAGE_NAME,
            Self::RetiredEngineLowLevel => "retired_engine_low_level",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunRendererRetiredEngineRole {
    None,
    EcsExtractionBridge,
    MinimalPrimitiveSource,
    LowLevelBackendHook,
}

impl FunRendererRetiredEngineRole {
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
    pub retired_engine_role: FunRendererRetiredEngineRole,
    pub default_renderer_core: bool,
    pub reusable_model_runtime_allowed: bool,
}

pub const FUN_RENDERER_SUBSYSTEM_DESCRIPTORS: [FunRendererSubsystemDescriptor; 10] = [
    FunRendererSubsystemDescriptor {
        stable_id: "fun_renderer.subsystem.renderer_core",
        subsystem: FunRendererSubsystem::RendererCore,
        owner: FunRendererOwner::FunRenderer,
        retired_engine_role: FunRendererRetiredEngineRole::None,
        default_renderer_core: true,
        reusable_model_runtime_allowed: false,
    },
    FunRendererSubsystemDescriptor {
        stable_id: "fun_renderer.subsystem.virtual_geometry",
        subsystem: FunRendererSubsystem::VirtualGeometry,
        owner: FunRendererOwner::FunRenderer,
        retired_engine_role: FunRendererRetiredEngineRole::MinimalPrimitiveSource,
        default_renderer_core: true,
        reusable_model_runtime_allowed: false,
    },
    FunRendererSubsystemDescriptor {
        stable_id: "fun_renderer.subsystem.virtual_shadows",
        subsystem: FunRendererSubsystem::VirtualShadows,
        owner: FunRendererOwner::FunRenderer,
        retired_engine_role: FunRendererRetiredEngineRole::LowLevelBackendHook,
        default_renderer_core: true,
        reusable_model_runtime_allowed: false,
    },
    FunRendererSubsystemDescriptor {
        stable_id: "fun_renderer.subsystem.gpu_scene_database",
        subsystem: FunRendererSubsystem::GpuSceneDatabase,
        owner: FunRendererOwner::FunRenderer,
        retired_engine_role: FunRendererRetiredEngineRole::EcsExtractionBridge,
        default_renderer_core: true,
        reusable_model_runtime_allowed: false,
    },
    FunRendererSubsystemDescriptor {
        stable_id: "fun_renderer.subsystem.frame_graph",
        subsystem: FunRendererSubsystem::FrameGraph,
        owner: FunRendererOwner::FunRenderer,
        retired_engine_role: FunRendererRetiredEngineRole::LowLevelBackendHook,
        default_renderer_core: true,
        reusable_model_runtime_allowed: false,
    },
    FunRendererSubsystemDescriptor {
        stable_id: "fun_renderer.subsystem.render_artifact_realization",
        subsystem: FunRendererSubsystem::RenderArtifactRealization,
        owner: FunRendererOwner::FunRenderer,
        retired_engine_role: FunRendererRetiredEngineRole::None,
        default_renderer_core: true,
        reusable_model_runtime_allowed: false,
    },
    FunRendererSubsystemDescriptor {
        stable_id: "fun_renderer.subsystem.ui_composition",
        subsystem: FunRendererSubsystem::UiComposition,
        owner: FunRendererOwner::FunRenderer,
        retired_engine_role: FunRendererRetiredEngineRole::LowLevelBackendHook,
        default_renderer_core: true,
        reusable_model_runtime_allowed: false,
    },
    FunRendererSubsystemDescriptor {
        stable_id: "fun_renderer.subsystem.upscaling_frame_generation",
        subsystem: FunRendererSubsystem::UpscalingFrameGeneration,
        owner: FunRendererOwner::FunRenderer,
        retired_engine_role: FunRendererRetiredEngineRole::LowLevelBackendHook,
        default_renderer_core: true,
        reusable_model_runtime_allowed: false,
    },
    FunRendererSubsystemDescriptor {
        stable_id: "fun_renderer.subsystem.backend_abstraction",
        subsystem: FunRendererSubsystem::BackendAbstraction,
        owner: FunRendererOwner::FunRenderer,
        retired_engine_role: FunRendererRetiredEngineRole::LowLevelBackendHook,
        default_renderer_core: true,
        reusable_model_runtime_allowed: false,
    },
    FunRendererSubsystemDescriptor {
        stable_id: "fun_renderer.subsystem.lighting",
        subsystem: FunRendererSubsystem::Lighting,
        owner: FunRendererOwner::Lux,
        retired_engine_role: FunRendererRetiredEngineRole::LowLevelBackendHook,
        default_renderer_core: true,
        reusable_model_runtime_allowed: false,
    },
];

/// Lib-layer mirror of [`crate::ui::native_adapter::NativeUiRenderRoleStatus`]
/// `DemotedToLegacyDiagnostic`. The native UI adapter owns the
/// authoritative typed enum; the lib policy carries the same stable
/// string so the source-of-truth UI policy test can verify the two
/// layers agree without crossing a feature-gate boundary at compile
/// time.
pub const FUN_RENDERER_LIB_LAYER_NATIVE_UI_ROLE_STATUS: &str = "demoted_to_legacy_diagnostic";

/// Lib-layer mirror of [`crate::ui::native_adapter::NATIVE_UI_ADAPTER_PRODUCT_DEFAULT`].
/// Pass 20 made `fun_ui_render_packet_v1` the canonical product UI
/// ingest packet schema; the lib layer carries the same string so
/// the source-of-truth UI policy test can verify the policies agree
/// without coupling lib.rs to the `native_ui_adapter` feature gate.
pub const FUN_RENDERER_LIB_LAYER_NATIVE_RVELTE_PACKET_SCHEMA: &str = "fun_ui_render_packet_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunRendererUiRuntimePolicy {
    /// Pass 20 demoted NATIVE_UI/Svelte from the product UI surface. This
    /// field is `true` to record the demotion explicitly. The product
    /// UI surface is the native rvelte/FUN UI path; see
    /// `native_rvelte_is_product_ui` below and
    /// [`crate::ui::native_adapter::NativeUiProductPolicy::PRODUCT_DEFAULT`]
    /// for the authoritative typed contract.
    pub native_ui_svelte_legacy_only: bool,
    /// Pass 20+ native rvelte/FUN UI is the product UI surface.
    pub native_rvelte_is_product_ui: bool,
    /// Lib-layer mirror of
    /// [`crate::ui::native_adapter::NativeUiRenderRoleStatus`] string form.
    /// Always equals
    /// [`FUN_RENDERER_LIB_LAYER_NATIVE_UI_ROLE_STATUS`].
    pub native_ui_role_status: &'static str,
    /// Lib-layer mirror of
    /// [`crate::ui::native_adapter::NATIVE_UI_ADAPTER_PRODUCT_DEFAULT`].
    /// Always equals
    /// [`FUN_RENDERER_LIB_LAYER_NATIVE_RVELTE_PACKET_SCHEMA`].
    pub native_ui_product_packet_schema: &'static str,
    pub retired_engine_ui_runtime_product_allowed: bool,
    pub retired_engine_ui_test_only_allowed: bool,
    pub launcher_ui_owner: &'static str,
    pub editor_ui_owner: &'static str,
    pub game_hud_owner: &'static str,
    pub diagnostics_panel_owner: &'static str,
    pub debug_overlay_owner: &'static str,
}

pub const FUN_RENDERER_UI_RUNTIME_POLICY: FunRendererUiRuntimePolicy = FunRendererUiRuntimePolicy {
    native_ui_svelte_legacy_only: true,
    native_rvelte_is_product_ui: true,
    native_ui_role_status: FUN_RENDERER_LIB_LAYER_NATIVE_UI_ROLE_STATUS,
    native_ui_product_packet_schema: FUN_RENDERER_LIB_LAYER_NATIVE_RVELTE_PACKET_SCHEMA,
    retired_engine_ui_runtime_product_allowed: false,
    retired_engine_ui_test_only_allowed: true,
    launcher_ui_owner: "native_rvelte_fun_ui",
    editor_ui_owner: "native_rvelte_fun_ui",
    game_hud_owner: "native_rvelte_fun_ui",
    diagnostics_panel_owner: "native_rvelte_fun_ui",
    debug_overlay_owner: "native_rvelte_fun_ui",
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunRendererNativeUiRuntimePolicy {
    pub gpu_shared_texture_required: bool,
    pub cpu_on_paint_runtime_fallback_allowed: bool,
    pub cpu_golden_fixture_test_only_allowed: bool,
    pub failure_mode: &'static str,
}

pub const FUN_RENDERER_NATIVE_UI_RUNTIME_POLICY: FunRendererNativeUiRuntimePolicy =
    FunRendererNativeUiRuntimePolicy {
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
    pub native_ui_text_readability_required: bool,
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
        native_ui_text_readability_required: true,
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
    Metal,
}

impl FunRendererBackend {
    pub const ALL: [Self; 3] = [Self::Dx12, Self::Vulkan, Self::Metal];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dx12 => "dx12",
            Self::Vulkan => "vulkan",
            Self::Metal => "metal",
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

pub const FUN_RENDERER_BACKEND_DESCRIPTORS: [FunRendererBackendDescriptor; 3] = [
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
    FunRendererBackendDescriptor {
        backend: FunRendererBackend::Metal,
        stable_id: "fun_renderer.backend.metal",
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
    NativeUiComposition,
    Present,
}

impl FunRendererFrameGraphStage {
    pub const ORDER: [Self; 7] = [
        Self::GpuSceneDatabase,
        Self::VirtualGeometry,
        Self::VirtualShadows,
        Self::Lighting,
        Self::UpscalingFrameGeneration,
        Self::NativeUiComposition,
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
            Self::NativeUiComposition => 60,
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
            Self::NativeUiComposition => "native_ui_composition",
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
    pub ecs_package: &'static str,
    pub ecs_crate: &'static str,
    pub scene_package: &'static str,
    pub scene_crate: &'static str,
    pub lighting_package: &'static str,
    pub lighting_crate: &'static str,
    pub retired_donor_package: &'static str,
    pub ai_owner_package: &'static str,
}

pub const FUN_RENDERER_PRODUCT_TOPOLOGY: FunRendererProductTopology = FunRendererProductTopology {
    renderer_package: FUN_RENDERER_PACKAGE_NAME,
    renderer_crate: FUN_RENDERER_CRATE_NAME,
    ecs_package: fun_ecs::FUN_ECS_PACKAGE_NAME,
    ecs_crate: fun_ecs::FUN_ECS_CRATE_NAME,
    scene_package: FUN_RENDERER_SCENE_OWNER_PACKAGE_NAME,
    scene_crate: FUN_RENDERER_SCENE_OWNER_CRATE_NAME,
    lighting_package: FUN_RENDERER_LIGHTING_OWNER_PACKAGE_NAME,
    lighting_crate: FUN_RENDERER_LIGHTING_OWNER_CRATE_NAME,
    retired_donor_package: FUN_RENDER_DONOR_PACKAGE_NAME,
    ai_owner_package: FUN_RENDERER_AI_OWNER_PACKAGE_NAME,
};

pub use api::*;
#[cfg(any())]
pub use asset_prep::*;
pub use backend::*;
pub use benchmark::*;
// Pass C0 / C1 typed cloud renderer re-exports.
#[cfg(any())]
pub use cloud_diagnostics::*;
#[cfg(any())]
pub use cloud_executor::*;
#[cfg(any())]
pub use cloud_gpu_resource_set::*;
#[cfg(any())]
pub use cloud_passes::*;
#[cfg(any())]
pub use cloud_receive_lux_lighting::*;
#[cfg(any())]
pub use cloud_resources::*;
#[cfg(any())]
pub use cloud_shaders::*;
#[cfg(any())]
pub use cloud_shadow::*;
#[cfg(any())]
pub use cloud_shadow_director::*;
#[cfg(any())]
pub use cloud_shadow_dispatch_feedback::*;
#[cfg(any())]
pub use cloud_shadow_golden_scenes::*;
#[cfg(any())]
pub use cloud_shadow_live_executor::*;
#[cfg(any())]
pub use cloud_shadow_look_tuning::*;
#[cfg(any())]
pub use cloud_shadow_passes::*;
#[cfg(any())]
pub use cloud_shadow_pipelines::*;
#[cfg(any())]
pub use cloud_shadow_runtime_diagnostics::*;
#[cfg(any())]
pub use cloud_shadow_runtime_probe::*;
#[cfg(any())]
pub use clouds::*;
#[cfg(feature = "fun_ecs")]
pub use component_api::*;
#[cfg(any())]
pub use dx12_production::*;
#[cfg(feature = "scene_contract")]
pub use dynamic_geometry::*;
#[cfg(any())]
pub use ecs::*;
#[cfg(any())]
pub use extraction::*;
pub use frame_graph::*;
#[cfg(any())]
pub use fun_render_cloud_retirement::*;
#[cfg(any())]
pub use fun_render_route_audit::*;
pub use gpu_driven::*;
#[cfg(any())]
pub use gpu_driven_runtime::*;
#[cfg(any())]
pub use heuristics::*;
pub use ir::*;
#[cfg(any())]
pub use lighting_stack::*;
#[cfg(any())]
pub use live_proof_frame_executor::*;
#[cfg(any())]
pub use lux_direct_lighting_cloud_layer::*;
#[cfg(any())]
pub use lux_direct_lighting_cloud_shader::*;
#[cfg(any())]
pub use lux_material_cloud_layer::*;
#[cfg(any())]
pub use lux_material_pbr_cloud_shader::*;
#[cfg(any())]
pub use lux_shadow_aux_layer::*;
#[cfg(any())]
pub use lux_volumetric_cloud_layer::*;
#[cfg(any())]
pub use lux_volumetric_light_inject_cloud_shader::*;
#[cfg(feature = "fun_ecs")]
pub use renderer_integration::*;
// Pass V2.5 typed renderer-side HDR-post modules.
#[cfg(any())]
pub use exposure_pass::*;
#[cfg(any())]
pub use hdr_pipeline::*;
#[cfg(any())]
pub use lux_diagnostics::*;
#[cfg(any())]
pub use lux_graph::*;
#[cfg(any())]
pub use lux_live_lighting::*;
#[cfg(any())]
pub use lux_passes::*;
#[cfg(any())]
pub use lux_resources::*;
#[cfg(any())]
pub use lux_volumetric_executor::*;
#[cfg(feature = "experimental_renderer_ml")]
pub use ml::*;
pub use page::*;
#[cfg(any())]
pub use passb_proof_frame_runtime::*;
#[cfg(any())]
pub use passc_runtime_cache_burndown::*;
#[cfg(any())]
pub use passd_gpu_scene_indirect_draw::*;
#[cfg(any())]
pub use passe_clustered_lighting_virtual_shadow_mvp::*;
#[cfg(any())]
pub use passf_temporal_stack::*;
#[cfg(any())]
pub use passg_native_ui_product_route::*;
#[cfg(any())]
pub use passh_native_command_list_fail_closed::*;
#[cfg(any())]
pub use passi_gpu_driven_compute_indirect::*;
#[cfg(any())]
pub use passj_clustered_lighting_live::*;
#[cfg(any())]
pub use passk_temporal_reconstruction_live::*;
#[cfg(any())]
pub use passl_native_ui_live::*;
#[cfg(any())]
pub use passm_windowed_surface_present::*;
pub use pipeline::*;
#[cfg(any())]
pub use plugin::*;
#[cfg(any())]
pub use post_process::*;
#[cfg(any())]
pub use presentation::*;
#[cfg(any())]
pub use presentation_stack::*;
#[cfg(any())]
pub use proof_scene::*;
#[cfg(any())]
pub use quality_audit_contract::*;
pub use queue_scheduler::*;
#[cfg(feature = "scene_contract")]
pub use research::*;
pub use resource::*;
#[cfg(feature = "scene_contract")]
pub use scene::*;
#[cfg(any())]
pub use scene_streaming::*;
pub use scheduler::*;
pub use settings::*;
#[cfg(any())]
pub use taa::*;
#[cfg(any())]
pub use tier0_proof_frame_gate::*;
#[cfg(any())]
pub use tier1_cache_backed_optimization::*;
#[cfg(any())]
pub use tier2_gpu_driven_proof::*;
#[cfg(any())]
pub use tier3_lighting_shadows_at_scale::*;
#[cfg(any())]
pub use tier4_transient_memory_and_barriers::*;
#[cfg(any())]
pub use tier5_temporal_reconstruction::*;
#[cfg(any())]
pub use tier6_native_ui_rendering::*;
#[cfg(any())]
pub use tier7_vendor_sdks_and_frame_generation::*;
#[cfg(any())]
pub use tier8_direct_backend_experiments::*;
#[cfg(any())]
pub use tonemap_pass::*;
#[cfg(any(feature = "scene_contract", feature = "native_ui_adapter"))]
pub use ui::*;
#[cfg(any())]
pub use vendor_sdk_bridge::*;
pub use virtual_geometry::*;
pub use virtual_shadow::*;

#[must_use]
pub const fn owner_for_subsystem(subsystem: FunRendererSubsystem) -> FunRendererOwner {
    match subsystem {
        FunRendererSubsystem::Lighting => FunRendererOwner::Lux,
        FunRendererSubsystem::RendererCore
        | FunRendererSubsystem::VirtualGeometry
        | FunRendererSubsystem::VirtualShadows
        | FunRendererSubsystem::GpuSceneDatabase
        | FunRendererSubsystem::FrameGraph
        | FunRendererSubsystem::RenderArtifactRealization
        | FunRendererSubsystem::UiComposition
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
        assert_eq!(FUN_RENDERER_PRODUCT_TOPOLOGY.ecs_package, "fun-ecs");
        assert_eq!(FUN_RENDERER_PRODUCT_TOPOLOGY.ecs_crate, "fun_ecs");
        assert_eq!(FUN_RENDERER_PRODUCT_TOPOLOGY.scene_package, "fun-scene");
        assert_eq!(FUN_RENDERER_PRODUCT_TOPOLOGY.scene_crate, "fun_scene");
        assert_eq!(FUN_RENDERER_PRODUCT_TOPOLOGY.lighting_package, "fun-lux");
        assert_eq!(FUN_RENDERER_PRODUCT_TOPOLOGY.lighting_crate, "fun_lux");
        assert_eq!(
            FUN_RENDERER_PRODUCT_TOPOLOGY.retired_donor_package,
            "fun_render"
        );
        assert_eq!(FUN_RENDERER_PRODUCT_TOPOLOGY.ai_owner_package, "fun-ai");
    }

    #[test]
    fn runtime_backend_selection_defaults_to_fun_after_default_flip() {
        let default_selection = FunRendererRuntimeBackend::selection_from_env_reader(|_| None);
        assert_eq!(default_selection.requested, FunRendererRuntimeBackend::Auto);
        assert_eq!(default_selection.resolved, FunRendererRuntimeBackend::Fun);
        assert_eq!(
            default_selection.reason,
            FunRendererBackendSelectionReason::DefaultAuto
        );
        assert!(!default_selection.loud_diagnostic_required);
        assert_eq!(
            default_selection.future_default_flip_location,
            FUN_RENDERER_BACKEND_FUTURE_DEFAULT_FLIP_LOCATION
        );

        assert_eq!(
            FunRendererRuntimeBackend::from_env_reader(|_| None),
            FunRendererRuntimeBackend::Fun
        );
        let legacy_selection = FunRendererRuntimeBackend::selection_from_env_reader(|name| {
            (name == FUN_RENDERER_RUNTIME_BACKEND_ENV).then_some("legacy")
        });
        assert_eq!(legacy_selection.resolved, FunRendererRuntimeBackend::Legacy);
        assert_eq!(
            legacy_selection.reason,
            FunRendererBackendSelectionReason::ExplicitLegacy
        );
        assert!(legacy_selection.loud_diagnostic_required);
        let fun_selection = FunRendererRuntimeBackend::selection_from_env_reader(|name| {
            (name == FUN_RENDERER_RUNTIME_BACKEND_ENV).then_some("fun")
        });
        assert_eq!(fun_selection.resolved, FunRendererRuntimeBackend::Fun);
        assert!(!fun_selection.loud_diagnostic_required);
        let invalid_selection = FunRendererRuntimeBackend::selection_from_env_reader(|name| {
            (name == FUN_RENDERER_RUNTIME_BACKEND_ENV).then_some("typo")
        });
        assert_eq!(invalid_selection.resolved, FunRendererRuntimeBackend::Fun);
        assert_eq!(
            invalid_selection.reason,
            FunRendererBackendSelectionReason::InvalidValueDefaultedToAuto
        );
        assert!(invalid_selection.loud_diagnostic_required);
        assert_eq!(FunRendererRuntimeBackend::Auto.as_env_value(), "auto");
        assert!(FunRendererRuntimeBackend::Legacy.is_transition_only());
        assert!(!FunRendererRuntimeBackend::Fun.is_transition_only());
        assert_eq!(FunRendererRuntimeBackend::Fun.as_env_value(), "fun");
    }

    #[test]
    fn product_ui_policy_prohibits_runtime_retired_engine_ui() {
        let policy = core::hint::black_box(FUN_RENDERER_UI_RUNTIME_POLICY);

        // Pass 20 demoted NATIVE_UI/Svelte from the product UI surface; the
        // policy must record both the demotion *and* the native
        // rvelte/FUN UI replacement.
        assert!(policy.native_ui_svelte_legacy_only);
        assert!(policy.native_rvelte_is_product_ui);
        assert_eq!(
            policy.native_ui_role_status,
            FUN_RENDERER_LIB_LAYER_NATIVE_UI_ROLE_STATUS
        );
        assert!(!policy.retired_engine_ui_runtime_product_allowed);
        assert!(policy.retired_engine_ui_test_only_allowed);
        assert_eq!(policy.game_hud_owner, "native_rvelte_fun_ui");
        assert_eq!(policy.launcher_ui_owner, "native_rvelte_fun_ui");
        assert_eq!(policy.editor_ui_owner, "native_rvelte_fun_ui");
        assert_eq!(policy.diagnostics_panel_owner, "native_rvelte_fun_ui");
        assert_eq!(policy.debug_overlay_owner, "native_rvelte_fun_ui");
    }

    /// Pass A source-of-truth UI policy test. Pulls in
    /// (1) the lib-layer `FUN_RENDERER_UI_RUNTIME_POLICY` and its
    ///     `FUN_RENDERER_LIB_LAYER_NATIVE_UI_ROLE_STATUS` /
    ///     `FUN_RENDERER_LIB_LAYER_NATIVE_RVELTE_PACKET_SCHEMA`
    ///     mirrors,
    /// (2) the workspace-map / subsystem taxonomy
    ///     (`FunRendererSubsystem::UiComposition`), which replaces
    ///     the older `NativeUiCompositor` variant,
    /// (3) the native UI adapter
    ///     (`NativeUiProductPolicy::PRODUCT_DEFAULT`), which is the
    ///     authoritative typed contract, and
    /// (4) the NATIVE_UI role status (`NativeUiRenderRoleStatus`), which is
    ///     the typed legacy-status taxonomy,
    /// and asserts that every source of truth agrees NATIVE_UI/Svelte is
    /// demoted to legacy/diagnostic and the product UI surface is
    /// native rvelte/FUN UI.
    #[cfg(feature = "native_ui_adapter")]
    #[test]
    fn source_of_truth_ui_policy_aligns_across_lib_subsystem_adapter_and_rvelte_bridge() {
        use crate::ui::native_adapter::{
            NATIVE_UI_ADAPTER_PRODUCT_DEFAULT, NativeUiProductPolicy, NativeUiRenderRoleStatus,
        };

        // (1) Lib-layer policy demotes NATIVE_UI/Svelte and elevates native
        // rvelte.
        let lib_policy = FUN_RENDERER_UI_RUNTIME_POLICY;
        assert!(lib_policy.native_ui_svelte_legacy_only);
        assert!(lib_policy.native_rvelte_is_product_ui);

        // (2) Subsystem taxonomy uses `UiComposition`. There is no
        // `NativeUiCompositor` variant; this `assert!` walks `ALL` so a
        // re-introduction of a NATIVE_UI-specific subsystem here would fail
        // the test.
        let ui_composition_present = FunRendererSubsystem::ALL
            .iter()
            .any(|s| matches!(s, FunRendererSubsystem::UiComposition));
        assert!(ui_composition_present);
        let ui_composition_descriptor = FUN_RENDERER_SUBSYSTEM_DESCRIPTORS
            .iter()
            .find(|d| matches!(d.subsystem, FunRendererSubsystem::UiComposition))
            .expect("UiComposition subsystem must be present in descriptors");
        assert_eq!(
            ui_composition_descriptor.stable_id,
            "fun_renderer.subsystem.ui_composition"
        );
        assert_eq!(
            ui_composition_descriptor.owner,
            FunRendererOwner::FunRenderer
        );

        // (3) Native UI adapter is the authoritative typed contract.
        let adapter_policy = NativeUiProductPolicy::PRODUCT_DEFAULT;
        assert!(adapter_policy.product_ui_path_is_native_rvelte);
        assert_eq!(
            adapter_policy.native_ui_status,
            NativeUiRenderRoleStatus::DemotedToLegacyDiagnostic
        );
        assert!(adapter_policy.fun_render_adapter_owns_packet_ingest);
        assert!(adapter_policy.renderer_packet_validation_required);

        // (4) Lib-layer mirrors must match the typed adapter values
        // exactly, so the two sources of truth cannot drift.
        assert_eq!(
            lib_policy.native_ui_role_status,
            adapter_policy.native_ui_status.as_str()
        );
        assert_eq!(
            lib_policy.native_ui_role_status,
            NativeUiRenderRoleStatus::DemotedToLegacyDiagnostic.as_str()
        );
        assert_eq!(
            lib_policy.native_ui_product_packet_schema,
            adapter_policy.product_packet_schema
        );
        assert_eq!(
            lib_policy.native_ui_product_packet_schema,
            NATIVE_UI_ADAPTER_PRODUCT_DEFAULT
        );
        assert_eq!(
            FUN_RENDERER_LIB_LAYER_NATIVE_RVELTE_PACKET_SCHEMA,
            NATIVE_UI_ADAPTER_PRODUCT_DEFAULT
        );
        assert_eq!(
            FUN_RENDERER_LIB_LAYER_NATIVE_UI_ROLE_STATUS,
            NativeUiRenderRoleStatus::DemotedToLegacyDiagnostic.as_str()
        );

        // NATIVE_UI role status enum: every variant the native adapter
        // exposes must report `is_product_ui_surface() == false`. If
        // a future variant ever reintroduces NATIVE_UI as a product UI
        // surface, this test fails immediately.
        for status in [
            NativeUiRenderRoleStatus::DemotedToLegacyDiagnostic,
            NativeUiRenderRoleStatus::ArchivedReferenceOnly,
            NativeUiRenderRoleStatus::StagedRemoval,
            NativeUiRenderRoleStatus::LegacyComparisonOnly,
            NativeUiRenderRoleStatus::TemporaryMigrationBridge,
        ] {
            assert!(!status.is_product_ui_surface(), "{}", status.as_str());
        }
    }

    #[test]
    fn native_ui_runtime_policy_is_gpu_only_for_product_lanes() {
        let policy = core::hint::black_box(FUN_RENDERER_NATIVE_UI_RUNTIME_POLICY);

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
        assert!(contract.native_ui_text_readability_required);
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
            owner_for_subsystem(FunRendererSubsystem::UiComposition),
            FunRendererOwner::FunRenderer
        );
    }

    #[test]
    fn backend_contract_covers_dx12_vulkan_and_metal() {
        assert_eq!(
            FunRendererBackend::ALL,
            [
                FunRendererBackend::Dx12,
                FunRendererBackend::Vulkan,
                FunRendererBackend::Metal
            ]
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
    fn frame_graph_keeps_native_ui_after_reconstruction() {
        let mut previous_key = 0;
        for stage in FunRendererFrameGraphStage::ORDER {
            assert!(stage.order_key() > previous_key, "{}", stage.as_str());
            previous_key = stage.order_key();
        }

        assert!(
            FunRendererFrameGraphStage::NativeUiComposition.order_key()
                > FunRendererFrameGraphStage::UpscalingFrameGeneration.order_key()
        );
        assert!(!FunRendererFrameGraphStage::NativeUiComposition.feeds_temporal_reconstruction());
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
