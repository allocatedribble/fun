use bevy::prelude::{App, Resource};
use fun_renderer::{
    FunRendererBackend, FunRendererRuntimeBackend, RendererCoreSettings, RendererFeatureToggles,
};

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
        legacy: cfg!(feature = "fun_renderer_legacy"),
        new_core: cfg!(feature = "fun_renderer_new_core"),
        dx12: cfg!(feature = "fun_renderer_dx12"),
        vulkan: cfg!(feature = "fun_renderer_vulkan"),
        cef_gpu_only: cfg!(feature = "fun_renderer_cef_gpu_only"),
        upscale: cfg!(feature = "fun_renderer_upscale"),
        dlss: cfg!(feature = "fun_renderer_dlss"),
        fsr: cfg!(feature = "fun_renderer_fsr"),
        frame_generation: cfg!(feature = "fun_renderer_frame_generation"),
        experimental_ml: cfg!(feature = "fun_renderer_experimental_ml"),
        lux_many_light: cfg!(feature = "fun_lux_many_light"),
        lux_virtual_shadows: cfg!(feature = "fun_lux_virtual_shadows"),
        lux_hybrid_gi: cfg!(feature = "fun_lux_hybrid_gi"),
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
    pub preferred_backend: FunRendererBackend,
    pub renderer_core: RendererCoreSettings,
    pub features: BridgeFeatureToggles,
}

impl RendererBridgeSettings {
    #[must_use]
    pub const fn compiled_default() -> Self {
        let features = BridgeFeatureToggles::COMPILED;
        Self {
            runtime_backend: FunRendererRuntimeBackend::Fun,
            preferred_backend: FunRendererBackend::Dx12,
            renderer_core: RendererCoreSettings::new(
                FunRendererRuntimeBackend::Fun,
                FunRendererBackend::Dx12,
                features.renderer_core_toggles(),
            ),
            features,
        }
    }

    #[must_use]
    pub const fn from_runtime_backend(runtime_backend: FunRendererRuntimeBackend) -> Self {
        let mut settings = Self::compiled_default();
        settings.runtime_backend = runtime_backend;
        settings.renderer_core.runtime_backend = runtime_backend;
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

pub fn install_renderer_bridge_api(app: &mut App, settings: RendererBridgeSettings) {
    app.insert_resource(settings)
        .init_resource::<RendererBridgeHooks>();
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

        assert_eq!(toggles.new_core, cfg!(feature = "fun_renderer_new_core"));
        assert_eq!(toggles.dx12, cfg!(feature = "fun_renderer_dx12"));
        assert_eq!(
            toggles.cef_gpu_only,
            cfg!(feature = "fun_renderer_cef_gpu_only")
        );
        assert_eq!(toggles.lux_many_light, cfg!(feature = "fun_lux_many_light"));
    }

    #[test]
    fn bridge_api_installs_settings_and_hook_registry() {
        let mut app = App::new();
        install_renderer_bridge_api(&mut app, RendererBridgeSettings::compiled_default());

        let settings = app.world().resource::<RendererBridgeSettings>();
        assert_eq!(settings.runtime_backend, FunRendererRuntimeBackend::Fun);
        assert_eq!(settings.preferred_backend, FunRendererBackend::Dx12);

        let hooks = app.world().resource::<RendererBridgeHooks>();
        assert!(hooks.contains_kind(BridgeHookKind::PluginRegistration));
        assert!(hooks.contains_kind(BridgeHookKind::ExtractionSystem));
        assert!(hooks.contains_kind(BridgeHookKind::DebugOverlay));
        assert!(hooks.contains_kind(BridgeHookKind::Benchmark));
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
}
