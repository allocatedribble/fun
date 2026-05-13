use core::marker::PhantomData;

use fun_ecs::{
    prelude::{ResMut, Resource},
    schedule::{IntoScheduleConfigs, SystemSet},
};
use retired_engine_app::{App, Plugin, PostUpdate, Update};

use crate::{
    RendererFeatureToggles,
    backend::{
        BackendBridgeType, BackendCapabilityReport, BackendDiagnostics, BackendDispatchMode,
        BackendNativeInterop, BackendNativeInteropStatus, BackendResource, BackendShader,
        BackendTiming, DefaultProductionBackendSelection, DynamicRendererBackend, NativeBackend,
        RendererBackend, StaticBackendSelection,
    },
    extraction::{
        begin_render_world_extraction_frame, extract_renderer_asset_events,
        extract_renderer_lights, extract_renderer_native_ui_surfaces,
        extract_renderer_post_process_volumes, extract_renderer_renderables,
        extract_renderer_ui_surfaces, extract_renderer_views,
        install_render_world_extraction_resources,
    },
    settings::{GraphicsBackendSetting, RendererQualityTier},
};

#[cfg(feature = "wgpu_bridge")]
use crate::{
    backend::{WgpuDx12Bridge, WgpuMetalBridge, WgpuVulkanBridge},
    bridge::wgpu::{
        WgpuBridgeRuntimeFailureReason, WgpuRendererBackend, build_health_artifact_for_state,
        build_health_artifact_for_unknown_backend, initialize_wgpu_bridge_runtime,
        snapshot_core_bridge, snapshot_hal_status,
    },
};

pub const FUN_RENDERER_PLUGIN_SCHEMA_VERSION: u16 = 1;
pub const RENDERER_PHASE_COUNT: usize = 13;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RendererTransformSystems;

impl RendererTransformSystems {
    const PROPAGATE: Self = Self;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererPhase {
    BackendInit,
    Extract,
    PrepareAssets,
    PrepareScene,
    Visibility,
    Queue,
    GraphBuild,
    GraphCompile,
    Record,
    Submit,
    Present,
    Cleanup,
    DiagnosticsFlush,
}

impl RendererPhase {
    pub const ORDER: [Self; RENDERER_PHASE_COUNT] = [
        Self::BackendInit,
        Self::Extract,
        Self::PrepareAssets,
        Self::PrepareScene,
        Self::Visibility,
        Self::Queue,
        Self::GraphBuild,
        Self::GraphCompile,
        Self::Record,
        Self::Submit,
        Self::Present,
        Self::Cleanup,
        Self::DiagnosticsFlush,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::BackendInit => 0,
            Self::Extract => 1,
            Self::PrepareAssets => 2,
            Self::PrepareScene => 3,
            Self::Visibility => 4,
            Self::Queue => 5,
            Self::GraphBuild => 6,
            Self::GraphCompile => 7,
            Self::Record => 8,
            Self::Submit => 9,
            Self::Present => 10,
            Self::Cleanup => 11,
            Self::DiagnosticsFlush => 12,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BackendInit => "renderer_backend_init",
            Self::Extract => "renderer_extract",
            Self::PrepareAssets => "renderer_prepare_assets",
            Self::PrepareScene => "renderer_prepare_scene",
            Self::Visibility => "renderer_visibility",
            Self::Queue => "renderer_queue",
            Self::GraphBuild => "renderer_graph_build",
            Self::GraphCompile => "renderer_graph_compile",
            Self::Record => "renderer_record",
            Self::Submit => "renderer_submit",
            Self::Present => "renderer_present",
            Self::Cleanup => "renderer_cleanup",
            Self::DiagnosticsFlush => "renderer_diagnostics_flush",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererPhaseDescriptor {
    pub phase: RendererPhase,
    pub order_key: u16,
    pub public_ecs_boundary: bool,
    pub backend_internal_boundary: bool,
}

pub const RENDERER_PHASE_DESCRIPTORS: [RendererPhaseDescriptor; RENDERER_PHASE_COUNT] = [
    RendererPhaseDescriptor {
        phase: RendererPhase::BackendInit,
        order_key: 10,
        public_ecs_boundary: true,
        backend_internal_boundary: true,
    },
    RendererPhaseDescriptor {
        phase: RendererPhase::Extract,
        order_key: 20,
        public_ecs_boundary: true,
        backend_internal_boundary: false,
    },
    RendererPhaseDescriptor {
        phase: RendererPhase::PrepareAssets,
        order_key: 30,
        public_ecs_boundary: true,
        backend_internal_boundary: false,
    },
    RendererPhaseDescriptor {
        phase: RendererPhase::PrepareScene,
        order_key: 40,
        public_ecs_boundary: true,
        backend_internal_boundary: false,
    },
    RendererPhaseDescriptor {
        phase: RendererPhase::Visibility,
        order_key: 50,
        public_ecs_boundary: true,
        backend_internal_boundary: false,
    },
    RendererPhaseDescriptor {
        phase: RendererPhase::Queue,
        order_key: 60,
        public_ecs_boundary: true,
        backend_internal_boundary: false,
    },
    RendererPhaseDescriptor {
        phase: RendererPhase::GraphBuild,
        order_key: 70,
        public_ecs_boundary: true,
        backend_internal_boundary: false,
    },
    RendererPhaseDescriptor {
        phase: RendererPhase::GraphCompile,
        order_key: 80,
        public_ecs_boundary: true,
        backend_internal_boundary: true,
    },
    RendererPhaseDescriptor {
        phase: RendererPhase::Record,
        order_key: 90,
        public_ecs_boundary: true,
        backend_internal_boundary: true,
    },
    RendererPhaseDescriptor {
        phase: RendererPhase::Submit,
        order_key: 100,
        public_ecs_boundary: true,
        backend_internal_boundary: true,
    },
    RendererPhaseDescriptor {
        phase: RendererPhase::Present,
        order_key: 110,
        public_ecs_boundary: true,
        backend_internal_boundary: true,
    },
    RendererPhaseDescriptor {
        phase: RendererPhase::Cleanup,
        order_key: 120,
        public_ecs_boundary: true,
        backend_internal_boundary: true,
    },
    RendererPhaseDescriptor {
        phase: RendererPhase::DiagnosticsFlush,
        order_key: 130,
        public_ecs_boundary: true,
        backend_internal_boundary: false,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct RendererBackendInit;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct RendererExtract;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct RendererPrepareAssets;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct RendererPrepareScene;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct RendererVisibility;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct RendererQueue;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct RendererGraphBuild;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct RendererGraphCompile;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct RendererRecord;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct RendererSubmit;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct RendererPresent;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct RendererCleanup;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct RendererDiagnosticsFlush;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct RendererConfig {
    pub schema_version: u16,
    pub ecs_owned_orchestration: bool,
    pub backend_internals_private: bool,
    pub production_static_dispatch: bool,
}

impl RendererConfig {
    pub const DEFAULT: Self = Self {
        schema_version: FUN_RENDERER_PLUGIN_SCHEMA_VERSION,
        ecs_owned_orchestration: true,
        backend_internals_private: true,
        production_static_dispatch: true,
    };
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub enum RendererBackendSelection {
    Static {
        backend_name: &'static str,
        bridge_type: BackendBridgeType,
        native_backend: NativeBackend,
    },
    DynamicTooling {
        backend: DynamicRendererBackend,
        bridge_type: BackendBridgeType,
        native_backend: NativeBackend,
    },
}

impl RendererBackendSelection {
    #[must_use]
    pub const fn static_for<B: RendererBackend>() -> Self {
        Self::Static {
            backend_name: B::NAME,
            bridge_type: B::CAPABILITY_REPORT.bridge_type,
            native_backend: B::CAPABILITY_REPORT.actual_native_backend,
        }
    }

    #[must_use]
    pub const fn dynamic(backend: DynamicRendererBackend) -> Self {
        let report = backend.capability_report();
        Self::DynamicTooling {
            backend,
            bridge_type: report.bridge_type,
            native_backend: report.actual_native_backend,
        }
    }

    #[must_use]
    pub const fn production_perf_evidence_allowed(self) -> bool {
        matches!(self, Self::Static { .. })
    }
}

impl Default for RendererBackendSelection {
    fn default() -> Self {
        Self::static_for::<<DefaultProductionBackendSelection as StaticBackendSelection>::Backend>()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct RendererBridgeState {
    pub backend_resources_installed: bool,
    pub initialized: bool,
    pub dispatch_mode: BackendDispatchMode,
    pub bridge_type: BackendBridgeType,
    pub actual_native_backend: NativeBackend,
    pub native_interop: BackendNativeInteropStatus,
}

impl RendererBridgeState {
    #[must_use]
    pub const fn from_report(report: BackendCapabilityReport) -> Self {
        Self {
            backend_resources_installed: false,
            initialized: false,
            dispatch_mode: report.dispatch_mode,
            bridge_type: report.bridge_type,
            actual_native_backend: report.actual_native_backend,
            native_interop: BackendNativeInteropStatus::from_report(report),
        }
    }
}

impl Default for RendererBridgeState {
    fn default() -> Self {
        Self::from_report(BackendCapabilityReport::NULL)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct RendererCapabilities {
    pub report: BackendCapabilityReport,
}

impl RendererCapabilities {
    #[must_use]
    pub const fn from_report(report: BackendCapabilityReport) -> Self {
        Self { report }
    }
}

impl Default for RendererCapabilities {
    fn default() -> Self {
        Self::from_report(BackendCapabilityReport::NULL)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Resource)]
pub struct RendererFrameIndex(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct RendererFeatureFlags {
    pub legacy: bool,
    pub new_core: bool,
    pub dx12: bool,
    pub vulkan: bool,
    pub metal: bool,
    pub native_ui_gpu_only: bool,
    pub upscaling: bool,
    pub dlss: bool,
    pub fsr: bool,
    pub frame_generation: bool,
    pub experimental_ml: bool,
}

impl RendererFeatureFlags {
    #[must_use]
    pub const fn from_toggles(toggles: RendererFeatureToggles) -> Self {
        Self {
            legacy: toggles.legacy,
            new_core: toggles.new_core,
            dx12: toggles.dx12,
            vulkan: toggles.vulkan,
            metal: toggles.metal,
            native_ui_gpu_only: toggles.native_ui_gpu_only,
            upscaling: toggles.upscale,
            dlss: toggles.dlss,
            fsr: toggles.fsr,
            frame_generation: toggles.frame_generation,
            experimental_ml: toggles.experimental_ml,
        }
    }

    #[must_use]
    pub const fn compiled() -> Self {
        Self::from_toggles(RendererFeatureToggles::COMPILED)
    }
}

impl Default for RendererFeatureFlags {
    fn default() -> Self {
        Self::compiled()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct RendererQualitySettings {
    pub quality_tier: RendererQualityTier,
    pub graphics_backend: GraphicsBackendSetting,
}

impl RendererQualitySettings {
    pub const BASELINE: Self = Self {
        quality_tier: RendererQualityTier::Baseline,
        graphics_backend: GraphicsBackendSetting::Auto,
    };
}

impl Default for RendererQualitySettings {
    fn default() -> Self {
        Self::BASELINE
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct RendererDiagnostics {
    pub phase_runs: [u64; RENDERER_PHASE_COUNT],
    pub last_completed_phase: Option<RendererPhase>,
    pub dynamic_dispatch_used: bool,
    pub diagnostics_flush_count: u64,
}

impl RendererDiagnostics {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            phase_runs: [0; RENDERER_PHASE_COUNT],
            last_completed_phase: None,
            dynamic_dispatch_used: false,
            diagnostics_flush_count: 0,
        }
    }

    pub fn record_phase(&mut self, phase: RendererPhase) {
        let index = phase.index();
        self.phase_runs[index] = self.phase_runs[index].saturating_add(1);
        self.last_completed_phase = Some(phase);
    }
}

impl Default for RendererDiagnostics {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererFailureReason {
    BackendResourceInstallFailed,
    DynamicBackendUsedForProductionEvidence,
    CapabilityMismatch,
    BridgeRuntimeAdapterSelectionFailed,
    BridgeRuntimeDeviceCreationFailed,
    BridgeRuntimeBackendTruthMismatch,
    BridgeRuntimeInstanceCreationFailed,
    BridgeRuntimeSurfaceCreationFailed,
    BridgeRuntimeSurfaceConfigurationFailed,
}

impl RendererFailureReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BackendResourceInstallFailed => "backend_resource_install_failed",
            Self::DynamicBackendUsedForProductionEvidence => {
                "dynamic_backend_used_for_production_evidence"
            }
            Self::CapabilityMismatch => "capability_mismatch",
            Self::BridgeRuntimeAdapterSelectionFailed => "bridge_runtime_adapter_selection_failed",
            Self::BridgeRuntimeDeviceCreationFailed => "bridge_runtime_device_creation_failed",
            Self::BridgeRuntimeBackendTruthMismatch => "bridge_runtime_backend_truth_mismatch",
            Self::BridgeRuntimeInstanceCreationFailed => "bridge_runtime_instance_creation_failed",
            Self::BridgeRuntimeSurfaceCreationFailed => "bridge_runtime_surface_creation_failed",
            Self::BridgeRuntimeSurfaceConfigurationFailed => {
                "bridge_runtime_surface_configuration_failed"
            }
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub struct RendererFailureState {
    pub failed: bool,
    pub reason: Option<RendererFailureReason>,
}

pub type DefaultFunRendererPlugin =
    FunRendererPlugin<<DefaultProductionBackendSelection as StaticBackendSelection>::Backend>;

pub struct FunRendererPlugin<B: RendererBackend> {
    _backend: PhantomData<fn() -> B>,
}

impl<B: RendererBackend> FunRendererPlugin<B> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            _backend: PhantomData,
        }
    }
}

impl<B: RendererBackend> Default for FunRendererPlugin<B> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "wgpu_bridge")]
impl<B> Plugin for FunRendererPlugin<B>
where
    B: WgpuRendererBackend
        + BackendDiagnostics
        + BackendNativeInterop
        + BackendResource
        + BackendShader
        + BackendTiming,
{
    fn build(&self, app: &mut App) {
        install_public_renderer_resources::<B>(app);
        app.add_plugins(FunWgpuBridgePlugin::<B>::default());
        install_renderer_phase_systems::<B>(app);
        install_wgpu_bridge_runtime_systems::<B>(app);
    }
}

#[cfg(not(feature = "wgpu_bridge"))]
impl<B> Plugin for FunRendererPlugin<B>
where
    B: RendererBackend
        + BackendDiagnostics
        + BackendNativeInterop
        + BackendResource
        + BackendShader
        + BackendTiming,
{
    fn build(&self, app: &mut App) {
        install_public_renderer_resources::<B>(app);
        install_renderer_phase_systems::<B>(app);
    }
}

pub struct DynamicFunRendererPlugin {
    backend: DynamicRendererBackend,
}

impl DynamicFunRendererPlugin {
    #[must_use]
    pub const fn new(backend: DynamicRendererBackend) -> Self {
        Self { backend }
    }
}

impl Default for DynamicFunRendererPlugin {
    fn default() -> Self {
        Self::new(DynamicRendererBackend::Null)
    }
}

impl Plugin for DynamicFunRendererPlugin {
    fn build(&self, app: &mut App) {
        let report = self.backend.capability_report();
        app.insert_resource(RendererConfig {
            production_static_dispatch: false,
            ..RendererConfig::DEFAULT
        })
        .insert_resource(RendererBackendSelection::dynamic(self.backend))
        .insert_resource(RendererBridgeState::from_report(report))
        .insert_resource(RendererCapabilities::from_report(report))
        .init_resource::<RendererFrameIndex>()
        .init_resource::<RendererFeatureFlags>()
        .init_resource::<RendererQualitySettings>()
        .insert_resource(RendererDiagnostics {
            dynamic_dispatch_used: true,
            ..RendererDiagnostics::new()
        })
        .init_resource::<RendererFailureState>();
        app.insert_resource(DynamicBackendToolingState {
            backend: self.backend,
            dispatch_mode: BackendDispatchMode::DynamicTooling,
        });
        install_render_world_extraction_resources(app);
        install_renderer_phase_systems_dynamic(app);
    }
}

#[cfg(feature = "wgpu_bridge")]
pub struct FunWgpuBridgePlugin<B: WgpuRendererBackend> {
    _backend: PhantomData<fn() -> B>,
}

#[cfg(feature = "wgpu_bridge")]
impl<B: WgpuRendererBackend> FunWgpuBridgePlugin<B> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            _backend: PhantomData,
        }
    }
}

#[cfg(feature = "wgpu_bridge")]
impl<B: WgpuRendererBackend> Default for FunWgpuBridgePlugin<B> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "wgpu_bridge")]
impl<B> Plugin for FunWgpuBridgePlugin<B>
where
    B: WgpuRendererBackend
        + BackendDiagnostics
        + BackendNativeInterop
        + BackendShader
        + BackendTiming,
{
    fn build(&self, app: &mut App) {
        app.init_resource::<WgpuBridgeDevice<B::Native>>()
            .init_resource::<WgpuBridgeQueue<B::Native>>()
            .init_resource::<WgpuBridgeAdapter<B::Native>>()
            .init_resource::<WgpuBridgeSurface<B::Native>>()
            .init_resource::<WgpuCoreState>()
            .init_resource::<WgpuHalAccess>()
            .init_resource::<NagaShaderBridge>()
            .init_resource::<BackendCommandPools<B::Native>>()
            .init_resource::<WgpuPipelineBridgeCacheResource<B::Native>>()
            .init_resource::<WgpuBindingBridgeCacheResource<B::Native>>()
            .init_resource::<WgpuDescriptorCacheResource<B::Native>>()
            .init_resource::<WgpuBridgeResourceRealizationMapResource<B::Native>>()
            .init_resource::<BridgeRuntimeOptions<B::Native>>()
            .init_resource::<WgpuBridgeHealthArtifactResource>()
            .init_resource::<WgpuBridgeRuntimeInitialized>();
    }
}

#[cfg(feature = "wgpu_bridge")]
pub type FunDx12BridgePlugin = FunWgpuBridgePlugin<WgpuDx12Bridge>;
#[cfg(feature = "wgpu_bridge")]
pub type FunVulkanBridgePlugin = FunWgpuBridgePlugin<WgpuVulkanBridge>;
#[cfg(feature = "wgpu_bridge")]
pub type FunMetalBridgePlugin = FunWgpuBridgePlugin<WgpuMetalBridge>;

pub fn install_public_renderer_resources<B>(app: &mut App)
where
    B: RendererBackend + BackendNativeInterop,
{
    let report = B::CAPABILITY_REPORT;
    app.insert_resource(RendererConfig::DEFAULT)
        .insert_resource(RendererBackendSelection::static_for::<B>())
        .insert_resource(RendererBridgeState::from_report(report))
        .insert_resource(RendererCapabilities::from_report(report))
        .init_resource::<RendererFrameIndex>()
        .init_resource::<RendererFeatureFlags>()
        .insert_resource(RendererQualitySettings {
            quality_tier: RendererQualityTier::Baseline,
            graphics_backend: graphics_backend_for_native(report.actual_native_backend),
        })
        .init_resource::<RendererDiagnostics>()
        .init_resource::<RendererFailureState>();
    install_render_world_extraction_resources(app);
}

pub fn install_renderer_phase_systems<B>(app: &mut App)
where
    B: RendererBackend + BackendDiagnostics + BackendNativeInterop,
{
    app.configure_sets(
        PostUpdate,
        (
            RendererBackendInit,
            RendererExtract,
            RendererPrepareAssets,
            RendererPrepareScene,
            RendererVisibility,
            RendererQueue,
            RendererGraphBuild,
            RendererGraphCompile,
            RendererRecord,
            RendererSubmit,
            RendererPresent,
            RendererCleanup,
            RendererDiagnosticsFlush,
        )
            .chain()
            .after(RendererTransformSystems::PROPAGATE),
    )
    .add_systems(
        PostUpdate,
        (
            renderer_backend_init::<B>.in_set(RendererBackendInit),
            (
                renderer_extract_phase,
                begin_render_world_extraction_frame,
                extract_renderer_asset_events,
                extract_renderer_renderables,
                extract_renderer_views,
                extract_renderer_lights,
                extract_renderer_ui_surfaces,
                extract_renderer_native_ui_surfaces,
                extract_renderer_post_process_volumes,
            )
                .chain()
                .in_set(RendererExtract),
            renderer_prepare_assets_phase.in_set(RendererPrepareAssets),
            renderer_prepare_scene_phase.in_set(RendererPrepareScene),
            renderer_visibility_phase.in_set(RendererVisibility),
            renderer_queue_phase.in_set(RendererQueue),
            renderer_graph_build_phase.in_set(RendererGraphBuild),
            renderer_graph_compile_phase.in_set(RendererGraphCompile),
            renderer_record_phase.in_set(RendererRecord),
            renderer_submit_phase.in_set(RendererSubmit),
            renderer_present_phase.in_set(RendererPresent),
            renderer_cleanup_phase.in_set(RendererCleanup),
            renderer_diagnostics_flush_phase.in_set(RendererDiagnosticsFlush),
        ),
    );
}

fn install_renderer_phase_systems_dynamic(app: &mut App) {
    app.configure_sets(
        PostUpdate,
        (
            RendererBackendInit,
            RendererExtract,
            RendererPrepareAssets,
            RendererPrepareScene,
            RendererVisibility,
            RendererQueue,
            RendererGraphBuild,
            RendererGraphCompile,
            RendererRecord,
            RendererSubmit,
            RendererPresent,
            RendererCleanup,
            RendererDiagnosticsFlush,
        )
            .chain()
            .after(RendererTransformSystems::PROPAGATE),
    )
    .add_systems(
        PostUpdate,
        (
            dynamic_renderer_backend_init.in_set(RendererBackendInit),
            (
                renderer_extract_phase,
                begin_render_world_extraction_frame,
                extract_renderer_asset_events,
                extract_renderer_renderables,
                extract_renderer_views,
                extract_renderer_lights,
                extract_renderer_ui_surfaces,
                extract_renderer_native_ui_surfaces,
                extract_renderer_post_process_volumes,
            )
                .chain()
                .in_set(RendererExtract),
            renderer_prepare_assets_phase.in_set(RendererPrepareAssets),
            renderer_prepare_scene_phase.in_set(RendererPrepareScene),
            renderer_visibility_phase.in_set(RendererVisibility),
            renderer_queue_phase.in_set(RendererQueue),
            renderer_graph_build_phase.in_set(RendererGraphBuild),
            renderer_graph_compile_phase.in_set(RendererGraphCompile),
            renderer_record_phase.in_set(RendererRecord),
            renderer_submit_phase.in_set(RendererSubmit),
            renderer_present_phase.in_set(RendererPresent),
            renderer_cleanup_phase.in_set(RendererCleanup),
            renderer_diagnostics_flush_phase.in_set(RendererDiagnosticsFlush),
        ),
    );
}

fn renderer_backend_init<B>(
    mut bridge_state: ResMut<RendererBridgeState>,
    mut capabilities: ResMut<RendererCapabilities>,
    mut diagnostics: ResMut<RendererDiagnostics>,
) where
    B: RendererBackend + BackendNativeInterop,
{
    bridge_state.backend_resources_installed = true;
    bridge_state.initialized = true;
    bridge_state.dispatch_mode = BackendDispatchMode::StaticProduction;
    bridge_state.bridge_type = B::CAPABILITY_REPORT.bridge_type;
    bridge_state.actual_native_backend = B::CAPABILITY_REPORT.actual_native_backend;
    bridge_state.native_interop = B::native_interop_status();
    capabilities.report = B::CAPABILITY_REPORT;
    diagnostics.record_phase(RendererPhase::BackendInit);
}

fn dynamic_renderer_backend_init(
    tooling: ResMut<DynamicBackendToolingState>,
    mut bridge_state: ResMut<RendererBridgeState>,
    mut capabilities: ResMut<RendererCapabilities>,
    mut diagnostics: ResMut<RendererDiagnostics>,
) {
    let report = tooling.backend.capability_report();
    bridge_state.backend_resources_installed = true;
    bridge_state.initialized = true;
    bridge_state.dispatch_mode = tooling.dispatch_mode;
    bridge_state.bridge_type = report.bridge_type;
    bridge_state.actual_native_backend = report.actual_native_backend;
    bridge_state.native_interop = BackendNativeInteropStatus::from_report(report);
    capabilities.report = report;
    diagnostics.dynamic_dispatch_used = true;
    diagnostics.record_phase(RendererPhase::BackendInit);
}

fn renderer_extract_phase(mut diagnostics: ResMut<RendererDiagnostics>) {
    diagnostics.record_phase(RendererPhase::Extract);
}

fn renderer_prepare_assets_phase(mut diagnostics: ResMut<RendererDiagnostics>) {
    diagnostics.record_phase(RendererPhase::PrepareAssets);
}

fn renderer_prepare_scene_phase(mut diagnostics: ResMut<RendererDiagnostics>) {
    diagnostics.record_phase(RendererPhase::PrepareScene);
}

fn renderer_visibility_phase(mut diagnostics: ResMut<RendererDiagnostics>) {
    diagnostics.record_phase(RendererPhase::Visibility);
}

fn renderer_queue_phase(mut diagnostics: ResMut<RendererDiagnostics>) {
    diagnostics.record_phase(RendererPhase::Queue);
}

fn renderer_graph_build_phase(mut diagnostics: ResMut<RendererDiagnostics>) {
    diagnostics.record_phase(RendererPhase::GraphBuild);
}

fn renderer_graph_compile_phase(mut diagnostics: ResMut<RendererDiagnostics>) {
    diagnostics.record_phase(RendererPhase::GraphCompile);
}

fn renderer_record_phase(mut diagnostics: ResMut<RendererDiagnostics>) {
    diagnostics.record_phase(RendererPhase::Record);
}

fn renderer_submit_phase(mut diagnostics: ResMut<RendererDiagnostics>) {
    diagnostics.record_phase(RendererPhase::Submit);
}

fn renderer_present_phase(
    mut frame_index: ResMut<RendererFrameIndex>,
    mut diagnostics: ResMut<RendererDiagnostics>,
) {
    diagnostics.record_phase(RendererPhase::Present);
    frame_index.0 = frame_index.0.saturating_add(1);
}

fn renderer_cleanup_phase(mut diagnostics: ResMut<RendererDiagnostics>) {
    diagnostics.record_phase(RendererPhase::Cleanup);
}

fn renderer_diagnostics_flush_phase(mut diagnostics: ResMut<RendererDiagnostics>) {
    diagnostics.record_phase(RendererPhase::DiagnosticsFlush);
    diagnostics.diagnostics_flush_count = diagnostics.diagnostics_flush_count.saturating_add(1);
}

const fn graphics_backend_for_native(native_backend: NativeBackend) -> GraphicsBackendSetting {
    match native_backend {
        NativeBackend::Dx12 => GraphicsBackendSetting::Dx12,
        NativeBackend::Vulkan => GraphicsBackendSetting::Vulkan,
        NativeBackend::Metal => GraphicsBackendSetting::Metal,
        NativeBackend::Unknown => GraphicsBackendSetting::Auto,
    }
}

#[cfg(feature = "wgpu_bridge")]
const fn renderer_failure_reason_for_runtime(
    reason: WgpuBridgeRuntimeFailureReason,
) -> RendererFailureReason {
    match reason {
        WgpuBridgeRuntimeFailureReason::AdapterSelectionFailed => {
            RendererFailureReason::BridgeRuntimeAdapterSelectionFailed
        }
        WgpuBridgeRuntimeFailureReason::DeviceCreationFailed => {
            RendererFailureReason::BridgeRuntimeDeviceCreationFailed
        }
        WgpuBridgeRuntimeFailureReason::BackendTruthMismatch => {
            RendererFailureReason::BridgeRuntimeBackendTruthMismatch
        }
        WgpuBridgeRuntimeFailureReason::InstanceCreationFailed => {
            RendererFailureReason::BridgeRuntimeInstanceCreationFailed
        }
        WgpuBridgeRuntimeFailureReason::SurfaceCreationFailed => {
            RendererFailureReason::BridgeRuntimeSurfaceCreationFailed
        }
        WgpuBridgeRuntimeFailureReason::SurfaceConfigurationFailed => {
            RendererFailureReason::BridgeRuntimeSurfaceConfigurationFailed
        }
    }
}

#[cfg(feature = "wgpu_bridge")]
fn install_wgpu_bridge_runtime_systems<B>(app: &mut App)
where
    B: WgpuRendererBackend
        + BackendDiagnostics
        + BackendNativeInterop
        + BackendShader
        + BackendTiming,
{
    app.add_systems(
        Update,
        renderer_bridge_runtime_init::<B>
            .in_set(RendererBackendInit)
            .after(renderer_backend_init::<B>),
    );
}

#[cfg(feature = "wgpu_bridge")]
#[allow(clippy::too_many_arguments)]
fn renderer_bridge_runtime_init<B>(
    options: ResMut<BridgeRuntimeOptions<B::Native>>,
    mut initialized: ResMut<WgpuBridgeRuntimeInitialized>,
    mut device_resource: ResMut<WgpuBridgeDevice<B::Native>>,
    mut queue_resource: ResMut<WgpuBridgeQueue<B::Native>>,
    mut adapter_resource: ResMut<WgpuBridgeAdapter<B::Native>>,
    mut command_pools: ResMut<BackendCommandPools<B::Native>>,
    mut core_state: ResMut<WgpuCoreState>,
    mut hal_access: ResMut<WgpuHalAccess>,
    descriptor_cache: ResMut<WgpuDescriptorCacheResource<B::Native>>,
    mut artifact: ResMut<WgpuBridgeHealthArtifactResource>,
    mut bridge_state: ResMut<RendererBridgeState>,
    mut failure_state: ResMut<RendererFailureState>,
) where
    B: WgpuRendererBackend,
{
    if initialized.attempted {
        return;
    }
    initialized.attempted = true;

    match initialize_wgpu_bridge_runtime::<B::Native>(&options.options) {
        Ok(state) => {
            adapter_resource.adapter = Some(::std::sync::Arc::clone(&state.adapter));
            queue_resource.queue = Some(::std::sync::Arc::clone(&state.queue));
            command_pools.device = Some(::std::sync::Arc::clone(&state.device));
            core_state.snapshot = Some(snapshot_core_bridge::<B::Native>(&state));
            hal_access.status = Some(snapshot_hal_status::<B::Native>());
            artifact.artifact = Some(build_health_artifact_for_state::<B::Native>(
                &state,
                &descriptor_cache.cache,
            ));
            bridge_state.actual_native_backend = state.actual_native_backend;
            device_resource.state = Some(state);
            initialized.succeeded = true;
        }
        Err(failure) => {
            failure_state.failed = true;
            failure_state.reason = Some(renderer_failure_reason_for_runtime(failure.reason));
            bridge_state.actual_native_backend = failure.actual_backend;
            artifact.artifact = Some(build_health_artifact_for_unknown_backend::<B::Native>(
                failure,
                &descriptor_cache.cache,
            ));
            hal_access.status = Some(snapshot_hal_status::<B::Native>());
            initialized.succeeded = false;
        }
    }
}

#[derive(Resource)]
struct DynamicBackendToolingState {
    backend: DynamicRendererBackend,
    dispatch_mode: BackendDispatchMode,
}

#[cfg(feature = "wgpu_bridge")]
#[allow(dead_code)]
mod wgpu_runtime_resources {
    use std::sync::Arc;

    use fun_ecs::Resource;

    use crate::bridge::wgpu::{
        WgpuBindingBridgeCache, WgpuBridgeDeviceState, WgpuBridgeHealthArtifact,
        WgpuBridgeResourceRealizationMap, WgpuBridgeRuntimeOptions, WgpuBridgeSurfaceState,
        WgpuDescriptorCache, WgpuNativeBackend, WgpuPipelineBridgeCache,
    };

    #[derive(Resource)]
    pub(crate) struct WgpuBridgeDevice<B: WgpuNativeBackend> {
        pub(crate) state: Option<WgpuBridgeDeviceState<B>>,
    }

    impl<B: WgpuNativeBackend> Default for WgpuBridgeDevice<B> {
        fn default() -> Self {
            Self { state: None }
        }
    }

    #[derive(Resource)]
    pub(crate) struct WgpuBridgeQueue<B: WgpuNativeBackend> {
        pub(crate) queue: Option<Arc<::wgpu::Queue>>,
        pub(crate) _backend: ::core::marker::PhantomData<fn() -> B>,
    }

    impl<B: WgpuNativeBackend> Default for WgpuBridgeQueue<B> {
        fn default() -> Self {
            Self {
                queue: None,
                _backend: ::core::marker::PhantomData,
            }
        }
    }

    #[derive(Resource)]
    pub(crate) struct WgpuBridgeAdapter<B: WgpuNativeBackend> {
        pub(crate) adapter: Option<Arc<::wgpu::Adapter>>,
        pub(crate) _backend: ::core::marker::PhantomData<fn() -> B>,
    }

    impl<B: WgpuNativeBackend> Default for WgpuBridgeAdapter<B> {
        fn default() -> Self {
            Self {
                adapter: None,
                _backend: ::core::marker::PhantomData,
            }
        }
    }

    #[derive(Resource)]
    pub(crate) struct WgpuBridgeSurface<B: WgpuNativeBackend> {
        pub(crate) state: Option<WgpuBridgeSurfaceState>,
        pub(crate) _backend: ::core::marker::PhantomData<fn() -> B>,
    }

    impl<B: WgpuNativeBackend> Default for WgpuBridgeSurface<B> {
        fn default() -> Self {
            Self {
                state: None,
                _backend: ::core::marker::PhantomData,
            }
        }
    }

    #[derive(Default, Resource)]
    pub(crate) struct WgpuCoreState {
        pub(crate) snapshot: Option<crate::bridge::wgpu::WgpuCoreBridgeSnapshot>,
    }

    #[derive(Default, Resource)]
    pub(crate) struct WgpuHalAccess {
        pub(crate) status: Option<crate::bridge::wgpu::WgpuHalBridgeStatus>,
    }

    #[derive(Default, Resource)]
    pub(crate) struct NagaShaderBridge {
        pub(crate) bridge: crate::bridge::wgpu::NagaShaderBridge,
    }

    #[derive(Resource)]
    pub(crate) struct BackendCommandPools<B: WgpuNativeBackend> {
        pub(crate) device: Option<Arc<::wgpu::Device>>,
        pub(crate) _backend: ::core::marker::PhantomData<fn() -> B>,
    }

    impl<B: WgpuNativeBackend> Default for BackendCommandPools<B> {
        fn default() -> Self {
            Self {
                device: None,
                _backend: ::core::marker::PhantomData,
            }
        }
    }

    #[derive(Resource)]
    pub(crate) struct WgpuPipelineBridgeCacheResource<B: WgpuNativeBackend> {
        pub(crate) cache: WgpuPipelineBridgeCache,
        pub(crate) _backend: ::core::marker::PhantomData<fn() -> B>,
    }

    impl<B: WgpuNativeBackend> Default for WgpuPipelineBridgeCacheResource<B> {
        fn default() -> Self {
            Self {
                cache: WgpuPipelineBridgeCache::default(),
                _backend: ::core::marker::PhantomData,
            }
        }
    }

    #[derive(Resource)]
    pub(crate) struct WgpuBindingBridgeCacheResource<B: WgpuNativeBackend> {
        pub(crate) cache: WgpuBindingBridgeCache,
        pub(crate) _backend: ::core::marker::PhantomData<fn() -> B>,
    }

    impl<B: WgpuNativeBackend> Default for WgpuBindingBridgeCacheResource<B> {
        fn default() -> Self {
            Self {
                cache: WgpuBindingBridgeCache::default(),
                _backend: ::core::marker::PhantomData,
            }
        }
    }

    #[derive(Resource)]
    pub(crate) struct WgpuDescriptorCacheResource<B: WgpuNativeBackend> {
        pub(crate) cache: WgpuDescriptorCache,
        pub(crate) _backend: ::core::marker::PhantomData<fn() -> B>,
    }

    impl<B: WgpuNativeBackend> Default for WgpuDescriptorCacheResource<B> {
        fn default() -> Self {
            Self {
                cache: WgpuDescriptorCache::default(),
                _backend: ::core::marker::PhantomData,
            }
        }
    }

    #[derive(Resource)]
    pub(crate) struct WgpuBridgeResourceRealizationMapResource<B: WgpuNativeBackend> {
        pub(crate) map: WgpuBridgeResourceRealizationMap,
        pub(crate) _backend: ::core::marker::PhantomData<fn() -> B>,
    }

    impl<B: WgpuNativeBackend> Default for WgpuBridgeResourceRealizationMapResource<B> {
        fn default() -> Self {
            Self {
                map: WgpuBridgeResourceRealizationMap::new(),
                _backend: ::core::marker::PhantomData,
            }
        }
    }

    #[derive(Resource)]
    pub(crate) struct BridgeRuntimeOptions<B: WgpuNativeBackend> {
        pub(crate) options: WgpuBridgeRuntimeOptions,
        pub(crate) _backend: ::core::marker::PhantomData<fn() -> B>,
    }

    impl<B: WgpuNativeBackend> Default for BridgeRuntimeOptions<B> {
        fn default() -> Self {
            Self {
                options: WgpuBridgeRuntimeOptions::production_default(),
                _backend: ::core::marker::PhantomData,
            }
        }
    }

    #[derive(Default, Resource)]
    pub(crate) struct WgpuBridgeHealthArtifactResource {
        pub(crate) artifact: Option<WgpuBridgeHealthArtifact>,
    }

    #[derive(Default, Clone, Copy, PartialEq, Eq, Hash, Resource)]
    pub(crate) struct WgpuBridgeRuntimeInitialized {
        pub(crate) attempted: bool,
        pub(crate) succeeded: bool,
    }
}

#[cfg(feature = "wgpu_bridge")]
use wgpu_runtime_resources::{
    BackendCommandPools, BridgeRuntimeOptions, NagaShaderBridge, WgpuBindingBridgeCacheResource,
    WgpuBridgeAdapter, WgpuBridgeDevice, WgpuBridgeHealthArtifactResource, WgpuBridgeQueue,
    WgpuBridgeResourceRealizationMapResource, WgpuBridgeRuntimeInitialized, WgpuBridgeSurface,
    WgpuCoreState, WgpuDescriptorCacheResource, WgpuHalAccess, WgpuPipelineBridgeCacheResource,
};

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "wgpu_bridge")]
    use crate::bridge::wgpu::{Dx12Native, MetalNative, VulkanNative};
    use crate::{
        backend::{WgpuDx12Backend, WgpuVulkanBackend},
        extraction::{
            RenderStableIdAllocator, RenderWorldExtractionDiagnostics, RenderWorldTables,
        },
    };

    #[test]
    fn fun_renderer_plugin_installs_static_dx12_spine_resources() {
        let mut app = App::new();
        app.add_plugins(FunRendererPlugin::<WgpuDx12Backend>::default());
        // Use fallback adapter for tests so DX12 init does not require a real GPU.
        if let Some(mut options) = app
            .world_mut()
            .get_resource_mut::<BridgeRuntimeOptions<Dx12Native>>()
        {
            options.options = crate::bridge::wgpu::WgpuBridgeRuntimeOptions::fallback_for_tests();
        }

        assert!(app.world().contains_resource::<RendererConfig>());
        assert!(app.world().contains_resource::<RendererBackendSelection>());
        assert!(app.world().contains_resource::<RendererBridgeState>());
        assert!(app.world().contains_resource::<RendererCapabilities>());
        assert!(app.world().contains_resource::<RendererFrameIndex>());
        assert!(app.world().contains_resource::<RendererFeatureFlags>());
        assert!(app.world().contains_resource::<RendererQualitySettings>());
        assert!(app.world().contains_resource::<RendererDiagnostics>());
        assert!(app.world().contains_resource::<RendererFailureState>());
        assert!(app.world().contains_resource::<RenderStableIdAllocator>());
        assert!(app.world().contains_resource::<RenderWorldTables>());
        assert!(
            app.world()
                .contains_resource::<RenderWorldExtractionDiagnostics>()
        );
        assert!(
            app.world()
                .contains_resource::<WgpuBridgeDevice<Dx12Native>>()
        );
        assert!(
            app.world()
                .contains_resource::<WgpuPipelineBridgeCacheResource<Dx12Native>>()
        );
        assert!(
            app.world()
                .contains_resource::<WgpuBridgeAdapter<Dx12Native>>()
        );
        assert!(
            app.world()
                .contains_resource::<WgpuBridgeHealthArtifactResource>()
        );
        assert!(
            app.world()
                .contains_resource::<WgpuBridgeRuntimeInitialized>()
        );

        let selection = app.world().resource::<RendererBackendSelection>();
        assert_eq!(
            *selection,
            RendererBackendSelection::Static {
                backend_name: "wgpu_dx12",
                bridge_type: BackendBridgeType::Wgpu,
                native_backend: NativeBackend::Dx12,
            }
        );

        app.update();

        let frame = app.world().resource::<RendererFrameIndex>();
        assert_eq!(frame.0, 1);
        let diagnostics = app.world().resource::<RendererDiagnostics>();
        assert_eq!(
            diagnostics.last_completed_phase,
            Some(RendererPhase::DiagnosticsFlush)
        );
        for phase in RendererPhase::ORDER {
            assert_eq!(
                diagnostics.phase_runs[phase.index()],
                1,
                "{}",
                phase.as_str()
            );
        }

        let initialized = app.world().resource::<WgpuBridgeRuntimeInitialized>();
        assert!(initialized.attempted);
        let artifact = app.world().resource::<WgpuBridgeHealthArtifactResource>();
        let artifact = artifact
            .artifact
            .expect("bridge runtime init must publish a canonical health artifact");
        assert_eq!(
            artifact.canonical_path,
            crate::bridge::wgpu::bridge_health_canonical_artifact_path()
        );
        let bridge_state = app.world().resource::<RendererBridgeState>();
        if initialized.succeeded {
            assert_eq!(bridge_state.actual_native_backend, NativeBackend::Dx12);
            assert!(
                app.world()
                    .resource::<WgpuBridgeAdapter<Dx12Native>>()
                    .adapter
                    .is_some()
            );
            assert!(
                app.world()
                    .resource::<WgpuBridgeQueue<Dx12Native>>()
                    .queue
                    .is_some()
            );
            // Pass 16: when the DX12 production gate succeeds, the bridge
            // health artifact must carry a DX12-native-backend health snapshot
            // (driver report, command-list bridge probe, diagnostic hook
            // status), and the truth gate must say the actual backend is DX12.
            let dx12_health = artifact
                .dx12_native_backend_health
                .expect("dx12 native backend health must be present on dx12 success path");
            assert_eq!(dx12_health.requested_backend, NativeBackend::Dx12);
            assert_eq!(dx12_health.actual_backend, NativeBackend::Dx12);
            assert!(dx12_health.product_dx12_truth_holds());
            assert_eq!(
                dx12_health.canonical_path,
                crate::bridge::wgpu::DX12_BRIDGE_DIAGNOSTICS_CANONICAL_ARTIFACT_PATH
            );
            // The public wgpu-hal bridge does not expose ID3D12GraphicsCommandList,
            // so the probe must report NativeCommandListUnavailable rather than
            // claim availability that the bridge cannot back.
            assert_eq!(
                dx12_health.command_list_status(),
                crate::bridge::wgpu::Dx12CommandListBridgeStatus::NativeCommandListUnavailable
            );
        } else {
            // On hosts without DX12, the truth gate must mark a failure with the
            // BridgeRuntime* reason. The failure must be captured in
            // RendererFailureState; the renderer phases must still complete the
            // spine so diagnostics remain coherent.
            let failure = app.world().resource::<RendererFailureState>();
            assert!(failure.failed);
            assert!(matches!(
                failure.reason,
                Some(
                    RendererFailureReason::BridgeRuntimeAdapterSelectionFailed
                        | RendererFailureReason::BridgeRuntimeDeviceCreationFailed
                        | RendererFailureReason::BridgeRuntimeBackendTruthMismatch
                        | RendererFailureReason::BridgeRuntimeInstanceCreationFailed
                )
            ));
        }
    }

    #[test]
    fn backend_bridge_plugins_install_internal_resources_only() {
        let mut app = App::new();
        app.add_plugins(FunDx12BridgePlugin::default());

        assert!(
            app.world()
                .contains_resource::<WgpuBridgeDevice<Dx12Native>>()
        );
        assert!(
            app.world()
                .contains_resource::<WgpuBridgeQueue<Dx12Native>>()
        );
        assert!(
            app.world()
                .contains_resource::<WgpuBridgeAdapter<Dx12Native>>()
        );
        assert!(
            app.world()
                .contains_resource::<WgpuBridgeSurface<Dx12Native>>()
        );
        assert!(app.world().contains_resource::<WgpuCoreState>());
        assert!(app.world().contains_resource::<WgpuHalAccess>());
        assert!(app.world().contains_resource::<NagaShaderBridge>());
        assert!(
            app.world()
                .contains_resource::<BackendCommandPools<Dx12Native>>()
        );
        assert!(
            app.world()
                .contains_resource::<WgpuPipelineBridgeCacheResource<Dx12Native>>()
        );
        assert!(
            app.world()
                .contains_resource::<WgpuBindingBridgeCacheResource<Dx12Native>>()
        );
        assert!(
            app.world()
                .contains_resource::<WgpuDescriptorCacheResource<Dx12Native>>()
        );
        assert!(
            app.world()
                .contains_resource::<WgpuBridgeResourceRealizationMapResource<Dx12Native>>()
        );
        assert!(!app.world().contains_resource::<RendererConfig>());
    }

    #[test]
    fn dynamic_renderer_plugin_marks_tooling_dispatch() {
        let mut app = App::new();
        app.add_plugins(DynamicFunRendererPlugin::new(
            DynamicRendererBackend::WgpuVulkan,
        ));

        let selection = app.world().resource::<RendererBackendSelection>();
        assert!(!selection.production_perf_evidence_allowed());
        assert_eq!(
            app.world()
                .resource::<RendererConfig>()
                .production_static_dispatch,
            false
        );
        assert!(app.world().contains_resource::<RenderWorldTables>());

        app.update();

        let diagnostics = app.world().resource::<RendererDiagnostics>();
        assert!(diagnostics.dynamic_dispatch_used);
        assert_eq!(
            app.world().resource::<RendererBridgeState>().dispatch_mode,
            BackendDispatchMode::DynamicTooling
        );
    }

    #[test]
    fn renderer_phase_descriptors_cover_required_spine_order() {
        assert_eq!(RENDERER_PHASE_DESCRIPTORS.len(), RENDERER_PHASE_COUNT);
        let mut previous = 0;
        for descriptor in RENDERER_PHASE_DESCRIPTORS {
            assert!(descriptor.order_key > previous);
            previous = descriptor.order_key;
            assert!(descriptor.public_ecs_boundary);
        }
        assert_eq!(RendererPhase::ORDER[0], RendererPhase::BackendInit);
        assert_eq!(
            RendererPhase::ORDER[RENDERER_PHASE_COUNT - 1],
            RendererPhase::DiagnosticsFlush
        );
    }

    #[test]
    fn vulkan_and_metal_plugin_aliases_are_static_wgpu_bridges() {
        let mut vulkan = App::new();
        vulkan.add_plugins(FunRendererPlugin::<WgpuVulkanBackend>::default());
        if let Some(mut options) = vulkan
            .world_mut()
            .get_resource_mut::<BridgeRuntimeOptions<VulkanNative>>()
        {
            options.options = crate::bridge::wgpu::WgpuBridgeRuntimeOptions::fallback_for_tests();
        }
        assert_eq!(
            vulkan
                .world()
                .resource::<RendererCapabilities>()
                .report
                .actual_native_backend,
            NativeBackend::Vulkan
        );

        let mut metal = App::new();
        metal.add_plugins(FunMetalBridgePlugin::default());
        assert!(
            metal
                .world()
                .contains_resource::<WgpuBridgeDevice<MetalNative>>()
        );
    }

    #[test]
    fn dx12_production_truth_gate_rejects_non_dx12_actual_backend() {
        // When the runtime cannot produce a DX12 adapter, the truth gate must mark
        // a clear failure on the renderer failure state and continue running the
        // ECS spine without exposing wgpu handles to gameplay-facing ECS.
        let failure_reason = renderer_failure_reason_for_runtime(
            WgpuBridgeRuntimeFailureReason::BackendTruthMismatch,
        );
        assert_eq!(
            failure_reason,
            RendererFailureReason::BridgeRuntimeBackendTruthMismatch
        );

        let adapter_failure = renderer_failure_reason_for_runtime(
            WgpuBridgeRuntimeFailureReason::AdapterSelectionFailed,
        );
        assert_eq!(
            adapter_failure,
            RendererFailureReason::BridgeRuntimeAdapterSelectionFailed
        );

        let device_failure = renderer_failure_reason_for_runtime(
            WgpuBridgeRuntimeFailureReason::DeviceCreationFailed,
        );
        assert_eq!(
            device_failure,
            RendererFailureReason::BridgeRuntimeDeviceCreationFailed
        );
    }

    #[test]
    fn dx12_production_gate_rejects_non_dx12_adapter_with_actual_backend_mismatch() {
        // Pass 16: the DX12 production gate is a typed Dx12ProductionGateError
        // that distinguishes "wrong bridge type" from "actual adapter is not
        // DX12". Vulkan/Metal cannot pose as DX12, and a non-DX12 bridge
        // cannot satisfy the DX12 gate.
        use crate::bridge::wgpu::{Dx12ProductionGateError, dx12_production_gate_for};

        let mismatch = dx12_production_gate_for::<Dx12Native>(NativeBackend::Vulkan)
            .expect_err("Vulkan adapter must not pose as DX12");
        assert_eq!(
            mismatch,
            Dx12ProductionGateError::ActualBackendMismatch {
                requested: NativeBackend::Dx12,
                actual: NativeBackend::Vulkan,
            }
        );

        let wrong_bridge = dx12_production_gate_for::<VulkanNative>(NativeBackend::Vulkan)
            .expect_err("Vulkan bridge cannot satisfy the DX12 production gate");
        assert!(matches!(
            wrong_bridge,
            Dx12ProductionGateError::NotDx12Bridge { .. }
        ));

        // The successful path must accept actual DX12 unchanged.
        assert_eq!(
            dx12_production_gate_for::<Dx12Native>(NativeBackend::Dx12),
            Ok(())
        );
    }
}
