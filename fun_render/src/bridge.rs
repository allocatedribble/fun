use bevy::prelude::{App, Res, ResMut, Resource};
use fun_renderer::{
    BackendCapabilities, ClearColorFrame, DeviceBackend, FrameGraphSubmission, FunRendererBackend,
    FunRendererBackendSelection, FunRendererRuntimeBackend, NoopRendererCore, PresentResult,
    Presentation, RendererCoreBootReport, RendererCoreDiagnostics, RendererCoreSettings,
    RendererCoreShutdownReport, RendererFeatureToggles, RendererFrameDescription,
    RendererFrameGraphDebugArtifact, RendererFrameGraphDiagnostics,
    fun_lux::{LuxBootReport, LuxFrameReport, LuxSettings, LuxShutdownReport, NoopLuxCore},
};
use tracing::{info, warn};

use crate::renderer_settings_ui_model_from_bridge;

pub const FUN_RENDER_BRIDGE_API_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BridgeFeatureToggles {
    pub legacy: bool,
    pub new_core: bool,
    pub dx12: bool,
    pub vulkan: bool,
    pub cef_gpu_only: bool,
    pub upscale: bool,
    pub dlss: bool,
    pub fsr: bool,
    pub frame_generation: bool,
    pub experimental_ml: bool,
    pub lux_many_light: bool,
    pub lux_virtual_shadows: bool,
    pub lux_hybrid_gi: bool,
}

impl BridgeFeatureToggles {
    pub const COMPILED: Self = Self {
        legacy: cfg!(feature = "legacy_renderer") || cfg!(feature = "fun_renderer_legacy"),
        new_core: cfg!(feature = "fun_renderer_core") || cfg!(feature = "fun_renderer_new_core"),
        dx12: cfg!(feature = "dx12_native_interop") || cfg!(feature = "fun_renderer_dx12"),
        vulkan: cfg!(feature = "vulkan_backend") || cfg!(feature = "fun_renderer_vulkan"),
        cef_gpu_only: cfg!(feature = "cef_gpu_only") || cfg!(feature = "fun_renderer_cef_gpu_only"),
        upscale: cfg!(feature = "upscaling") || cfg!(feature = "fun_renderer_upscale"),
        dlss: cfg!(feature = "dlss") || cfg!(feature = "fun_renderer_dlss"),
        fsr: cfg!(feature = "fsr") || cfg!(feature = "fun_renderer_fsr"),
        frame_generation: cfg!(feature = "frame_generation")
            || cfg!(feature = "fun_renderer_frame_generation"),
        experimental_ml: cfg!(feature = "experimental_renderer_ml")
            || cfg!(feature = "fun_renderer_experimental_ml"),
        lux_many_light: cfg!(feature = "many_light") || cfg!(feature = "fun_lux_many_light"),
        lux_virtual_shadows: cfg!(feature = "virtual_shadows")
            || cfg!(feature = "fun_lux_virtual_shadows"),
        lux_hybrid_gi: cfg!(feature = "hybrid_gi") || cfg!(feature = "fun_lux_hybrid_gi"),
    };

    #[must_use]
    pub const fn compiled() -> Self {
        Self::COMPILED
    }

    #[must_use]
    pub const fn renderer_core_toggles(self) -> RendererFeatureToggles {
        RendererFeatureToggles {
            legacy: self.legacy,
            new_core: self.new_core,
            dx12: self.dx12,
            vulkan: self.vulkan,
            cef_gpu_only: self.cef_gpu_only,
            upscale: self.upscale,
            dlss: self.dlss,
            fsr: self.fsr,
            frame_generation: self.frame_generation,
            experimental_ml: self.experimental_ml,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Resource)]
pub struct RendererBridgeSettings {
    pub runtime_backend: FunRendererRuntimeBackend,
    pub backend_selection: FunRendererBackendSelection,
    pub preferred_backend: FunRendererBackend,
    pub renderer_core: RendererCoreSettings,
    pub features: BridgeFeatureToggles,
}

impl RendererBridgeSettings {
    #[must_use]
    pub const fn compiled_default() -> Self {
        let features = BridgeFeatureToggles::COMPILED;
        let backend_selection = FunRendererBackendSelection::default_auto();
        Self {
            runtime_backend: backend_selection.requested,
            backend_selection,
            preferred_backend: FunRendererBackend::Dx12,
            renderer_core: RendererCoreSettings::new(
                backend_selection.resolved,
                FunRendererBackend::Dx12,
                features.renderer_core_toggles(),
            ),
            features,
        }
    }

    #[must_use]
    pub const fn from_runtime_backend(runtime_backend: FunRendererRuntimeBackend) -> Self {
        let mut settings = Self::compiled_default();
        let backend_selection = match runtime_backend {
            FunRendererRuntimeBackend::Auto => FunRendererBackendSelection::explicit_auto(),
            FunRendererRuntimeBackend::Fun => FunRendererBackendSelection::explicit_fun(),
            FunRendererRuntimeBackend::Legacy => FunRendererBackendSelection::explicit_legacy(),
        };
        settings.runtime_backend = runtime_backend;
        settings.backend_selection = backend_selection;
        settings.renderer_core.runtime_backend = backend_selection.resolved;
        settings
    }

    #[must_use]
    pub fn from_env() -> Self {
        let mut settings = Self::compiled_default();
        let backend_selection = FunRendererRuntimeBackend::selection_from_env();
        settings.runtime_backend = backend_selection.requested;
        settings.backend_selection = backend_selection;
        settings.renderer_core.runtime_backend = backend_selection.resolved;
        settings
    }
}

impl Default for RendererBridgeSettings {
    fn default() -> Self {
        Self::compiled_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeHookKind {
    PluginRegistration,
    ExtractionSystem,
    DebugOverlay,
    Benchmark,
}

impl BridgeHookKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PluginRegistration => "plugin_registration",
            Self::ExtractionSystem => "extraction_system",
            Self::DebugOverlay => "debug_overlay",
            Self::Benchmark => "benchmark",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BridgeHookDescriptor {
    pub stable_id: &'static str,
    pub kind: BridgeHookKind,
    pub enabled: bool,
}

impl BridgeHookDescriptor {
    #[must_use]
    pub const fn new(stable_id: &'static str, kind: BridgeHookKind) -> Self {
        Self {
            stable_id,
            kind,
            enabled: true,
        }
    }
}

pub const BRIDGE_HOOKS: [BridgeHookDescriptor; 5] = [
    BridgeHookDescriptor::new(
        "fun_render.bridge.plugin.registration",
        BridgeHookKind::PluginRegistration,
    ),
    BridgeHookDescriptor::new(
        "fun_render.bridge.extraction.scene_deltas",
        BridgeHookKind::ExtractionSystem,
    ),
    BridgeHookDescriptor::new(
        "fun_render.bridge.extraction.lux_deltas",
        BridgeHookKind::ExtractionSystem,
    ),
    BridgeHookDescriptor::new(
        "fun_render.bridge.debug_overlay",
        BridgeHookKind::DebugOverlay,
    ),
    BridgeHookDescriptor::new("fun_render.bridge.benchmark", BridgeHookKind::Benchmark),
];

#[derive(Debug, Clone, PartialEq, Eq, Resource)]
pub struct RendererBridgeHooks {
    pub hooks: Vec<BridgeHookDescriptor>,
    pub extraction_noop_runs: u64,
    pub debug_overlay_noop_runs: u64,
    pub benchmark_noop_runs: u64,
}

impl RendererBridgeHooks {
    #[must_use]
    pub fn compiled() -> Self {
        Self {
            hooks: BRIDGE_HOOKS.to_vec(),
            extraction_noop_runs: 0,
            debug_overlay_noop_runs: 0,
            benchmark_noop_runs: 0,
        }
    }

    #[must_use]
    pub fn contains_kind(&self, kind: BridgeHookKind) -> bool {
        self.hooks
            .iter()
            .any(|descriptor| descriptor.kind == kind && descriptor.enabled)
    }
}

impl Default for RendererBridgeHooks {
    fn default() -> Self {
        Self::compiled()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Resource)]
pub struct RendererBridgeRuntimeState {
    pub selection: FunRendererBackendSelection,
    pub initialized_once: bool,
    pub legacy_product_path_active: bool,
    pub fun_core_initialized: bool,
    pub loud_diagnostic_required: bool,
    pub backend_capabilities: Option<BackendCapabilities>,
    pub core_boot: Option<RendererCoreBootReport>,
    pub core_diagnostics: Option<RendererCoreDiagnostics>,
    pub clear_color_frame: Option<ClearColorFrame>,
    pub present_result: Option<PresentResult>,
    pub core_shutdown: Option<RendererCoreShutdownReport>,
    pub lux_boot: Option<LuxBootReport>,
    pub lux_frame: Option<LuxFrameReport>,
    pub lux_shutdown: Option<LuxShutdownReport>,
}

impl RendererBridgeRuntimeState {
    #[must_use]
    pub const fn from_settings(settings: RendererBridgeSettings) -> Self {
        Self {
            selection: settings.backend_selection,
            initialized_once: false,
            legacy_product_path_active: settings.backend_selection.uses_legacy_product_path(),
            fun_core_initialized: false,
            loud_diagnostic_required: settings.backend_selection.loud_diagnostic_required,
            backend_capabilities: None,
            core_boot: None,
            core_diagnostics: None,
            clear_color_frame: None,
            present_result: None,
            core_shutdown: None,
            lux_boot: None,
            lux_frame: None,
            lux_shutdown: None,
        }
    }
}

impl Default for RendererBridgeRuntimeState {
    fn default() -> Self {
        Self::from_settings(RendererBridgeSettings::compiled_default())
    }
}

#[derive(Debug, Default, Clone, PartialEq, Resource)]
pub struct RendererBridgeFrameGraphReport {
    pub submitted_once: bool,
    pub frame_description: Option<RendererFrameDescription>,
    pub diagnostics: Option<RendererFrameGraphDiagnostics>,
    pub debug_artifact: Option<RendererFrameGraphDebugArtifact>,
}

pub fn install_renderer_bridge_api(app: &mut App, settings: RendererBridgeSettings) {
    app.insert_resource(settings)
        .insert_resource(renderer_settings_ui_model_from_bridge(settings))
        .insert_resource(RendererBridgeRuntimeState::from_settings(settings))
        .init_resource::<RendererBridgeFrameGraphReport>()
        .init_resource::<RendererBridgeHooks>();
}

pub fn renderer_bridge_initialize_runtime(
    settings: Res<RendererBridgeSettings>,
    mut state: ResMut<RendererBridgeRuntimeState>,
    mut frame_graph_report: ResMut<RendererBridgeFrameGraphReport>,
) {
    if state.initialized_once {
        return;
    }

    state.selection = settings.backend_selection;
    state.loud_diagnostic_required = settings.backend_selection.loud_diagnostic_required;
    state.legacy_product_path_active = settings.backend_selection.uses_legacy_product_path();

    if settings.backend_selection.uses_fun_renderer_core() {
        let (mut core, boot_report) = NoopRendererCore::boot(settings.renderer_core);
        let backend_capabilities = core.capabilities();
        let clear_color_frame = core.produce_clear_color_frame();
        let frame_description = renderer_bridge_frame_description_from_settings(
            &settings,
            clear_color_frame.frame_index,
        );
        let frame_graph_diagnostics = core.submit_frame_description(frame_description);
        let frame_graph_debug_artifact = core.frame_graph_debug_artifact();
        let present_result = core.present_clear_color(clear_color_frame);
        let core_diagnostics = core.diagnostics();
        let core_shutdown = core.shutdown();

        let (lux_core, lux_boot) = NoopLuxCore::boot(LuxSettings::default());
        let lux_frame = lux_core.baseline_frame();
        let lux_shutdown = lux_core.shutdown();

        state.fun_core_initialized = true;
        state.backend_capabilities = Some(backend_capabilities);
        state.core_boot = Some(boot_report);
        state.core_diagnostics = Some(core_diagnostics);
        state.clear_color_frame = Some(clear_color_frame);
        state.present_result = Some(present_result);
        state.core_shutdown = Some(core_shutdown);
        state.lux_boot = Some(lux_boot);
        state.lux_frame = Some(lux_frame);
        state.lux_shutdown = Some(lux_shutdown);
        frame_graph_report.submitted_once = true;
        frame_graph_report.frame_description = Some(frame_description);
        frame_graph_report.diagnostics = Some(frame_graph_diagnostics.clone());
        frame_graph_report.debug_artifact = frame_graph_debug_artifact;

        info!(
            target: "fun::render",
            env = fun_renderer::FUN_RENDERER_RUNTIME_BACKEND_ENV,
            requested_backend = settings.backend_selection.requested.as_env_value(),
            resolved_backend = settings.backend_selection.resolved.as_env_value(),
            preferred_backend = settings.preferred_backend.as_str(),
            registered_passes = boot_report.registered_passes,
            frame_graph_passes = frame_graph_diagnostics.pass_count,
            frame_graph_validation_failures = frame_graph_diagnostics.validation_failure_count(),
            clear_color_frame = boot_report.produced_clear_color_frame,
            lux_direct_lighting = lux_boot.direct_lighting.as_str(),
            "fun-renderer core initialized through fun_render bridge frame submission"
        );
    } else {
        warn!(
            target: "fun::render",
            env = fun_renderer::FUN_RENDERER_RUNTIME_BACKEND_ENV,
            requested_backend = settings.backend_selection.requested.as_env_value(),
            resolved_backend = settings.backend_selection.resolved.as_env_value(),
            reason = settings.backend_selection.reason.as_str(),
            future_default_flip_location = settings.backend_selection.future_default_flip_location,
            "FUN_RENDERER_BACKEND routed to legacy Bevy/wgpu presentation for this transition pass"
        );
    }

    state.initialized_once = true;
}

#[must_use]
pub fn renderer_bridge_frame_description_from_settings(
    settings: &RendererBridgeSettings,
    frame_index: u64,
) -> RendererFrameDescription {
    RendererFrameDescription::static_scene_with_ui(frame_index)
        .with_virtual_resources(
            settings.features.lux_virtual_shadows || settings.features.lux_hybrid_gi,
        )
        .with_upscaling(
            settings.features.upscale || settings.features.dlss || settings.features.fsr,
        )
        .with_frame_generation(settings.features.frame_generation)
}

pub fn renderer_bridge_extract_noop(mut hooks: bevy::prelude::ResMut<RendererBridgeHooks>) {
    hooks.extraction_noop_runs = hooks.extraction_noop_runs.saturating_add(1);
}

pub fn renderer_bridge_debug_overlay_noop(mut hooks: bevy::prelude::ResMut<RendererBridgeHooks>) {
    hooks.debug_overlay_noop_runs = hooks.debug_overlay_noop_runs.saturating_add(1);
}

pub fn renderer_bridge_benchmark_noop(mut hooks: bevy::prelude::ResMut<RendererBridgeHooks>) {
    hooks.benchmark_noop_runs = hooks.benchmark_noop_runs.saturating_add(1);
}

#[cfg(test)]
mod tests {
    use bevy::prelude::Schedule;

    use super::*;

    #[test]
    fn bridge_feature_toggles_mirror_compile_flags() {
        let toggles = BridgeFeatureToggles::compiled();

        assert_eq!(
            toggles.new_core,
            cfg!(feature = "fun_renderer_core") || cfg!(feature = "fun_renderer_new_core")
        );
        assert_eq!(
            toggles.dx12,
            cfg!(feature = "dx12_native_interop") || cfg!(feature = "fun_renderer_dx12")
        );
        assert_eq!(
            toggles.cef_gpu_only,
            cfg!(feature = "cef_gpu_only") || cfg!(feature = "fun_renderer_cef_gpu_only")
        );
        assert_eq!(
            toggles.lux_many_light,
            cfg!(feature = "many_light") || cfg!(feature = "fun_lux_many_light")
        );
    }

    #[test]
    fn bridge_api_installs_settings_and_hook_registry() {
        let mut app = App::new();
        install_renderer_bridge_api(&mut app, RendererBridgeSettings::compiled_default());

        let settings = app.world().resource::<RendererBridgeSettings>();
        assert_eq!(settings.runtime_backend, FunRendererRuntimeBackend::Auto);
        assert_eq!(
            settings.backend_selection.resolved,
            FunRendererRuntimeBackend::Legacy
        );
        assert_eq!(settings.preferred_backend, FunRendererBackend::Dx12);

        let state = app.world().resource::<RendererBridgeRuntimeState>();
        assert!(state.legacy_product_path_active);
        assert!(!state.fun_core_initialized);

        let hooks = app.world().resource::<RendererBridgeHooks>();
        assert!(hooks.contains_kind(BridgeHookKind::PluginRegistration));
        assert!(hooks.contains_kind(BridgeHookKind::ExtractionSystem));
        assert!(hooks.contains_kind(BridgeHookKind::DebugOverlay));
        assert!(hooks.contains_kind(BridgeHookKind::Benchmark));
    }

    #[test]
    fn bridge_frame_description_tracks_renderer_feature_slots() {
        let mut settings =
            RendererBridgeSettings::from_runtime_backend(FunRendererRuntimeBackend::Fun);
        settings.features.upscale = true;
        settings.features.frame_generation = true;
        settings.features.lux_virtual_shadows = true;

        let description = renderer_bridge_frame_description_from_settings(&settings, 44);

        assert_eq!(description.frame_index, 44);
        assert!(description.include_static_scene_placeholder);
        assert!(description.include_ui_placeholder);
        assert!(description.include_virtual_resource_slot);
        assert!(description.include_upscaling_slot);
        assert!(description.include_frame_generation_slot);
    }

    #[test]
    fn no_op_bridge_systems_are_schedulable() {
        let mut app = App::new();
        install_renderer_bridge_api(&mut app, RendererBridgeSettings::compiled_default());

        let mut schedule = Schedule::default();
        schedule.add_systems((
            renderer_bridge_extract_noop,
            renderer_bridge_debug_overlay_noop,
            renderer_bridge_benchmark_noop,
        ));
        schedule.run(app.world_mut());

        let hooks = app.world().resource::<RendererBridgeHooks>();
        assert_eq!(hooks.extraction_noop_runs, 1);
        assert_eq!(hooks.debug_overlay_noop_runs, 1);
        assert_eq!(hooks.benchmark_noop_runs, 1);
    }

    #[test]
    fn explicit_fun_backend_initializes_noop_core_and_lux() {
        let mut app = App::new();
        install_renderer_bridge_api(
            &mut app,
            RendererBridgeSettings::from_runtime_backend(FunRendererRuntimeBackend::Fun),
        );

        let mut schedule = Schedule::default();
        schedule.add_systems(renderer_bridge_initialize_runtime);
        schedule.run(app.world_mut());

        let state = app.world().resource::<RendererBridgeRuntimeState>();
        assert!(state.initialized_once);
        assert!(state.fun_core_initialized);
        assert!(!state.legacy_product_path_active);
        assert_eq!(
            state
                .core_boot
                .expect("fun renderer core should boot")
                .runtime_backend,
            FunRendererRuntimeBackend::Fun
        );
        assert_eq!(
            state
                .present_result
                .expect("fun renderer core should present a no-op frame")
                .frame_index,
            1
        );
        assert!(
            state
                .lux_frame
                .expect("fun-lux should produce baseline no-op lighting")
                .baseline_noop
        );
        assert!(
            state
                .core_shutdown
                .expect("fun renderer should shut down cleanly")
                .clean_shutdown
        );

        let frame_graph_report = app.world().resource::<RendererBridgeFrameGraphReport>();
        assert!(frame_graph_report.submitted_once);
        assert!(
            frame_graph_report
                .diagnostics
                .as_ref()
                .expect("frame graph diagnostics")
                .graph_valid()
        );
        assert!(
            frame_graph_report
                .debug_artifact
                .as_ref()
                .expect("frame graph debug artifact")
                .content
                .contains("fun_renderer.pass.compose")
        );
    }

    #[test]
    fn auto_backend_keeps_legacy_product_path_loud() {
        let mut app = App::new();
        install_renderer_bridge_api(
            &mut app,
            RendererBridgeSettings::from_runtime_backend(FunRendererRuntimeBackend::Auto),
        );

        let mut schedule = Schedule::default();
        schedule.add_systems(renderer_bridge_initialize_runtime);
        schedule.run(app.world_mut());

        let state = app.world().resource::<RendererBridgeRuntimeState>();
        assert!(state.initialized_once);
        assert!(state.legacy_product_path_active);
        assert!(state.loud_diagnostic_required);
        assert!(!state.fun_core_initialized);
        assert!(state.core_boot.is_none());
        assert_eq!(state.selection.requested, FunRendererRuntimeBackend::Auto);
        assert_eq!(state.selection.resolved, FunRendererRuntimeBackend::Legacy);
    }
}
