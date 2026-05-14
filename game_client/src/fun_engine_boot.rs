use core::fmt;

use fun_engine::{
    Engine, EngineBuilderDefaultStackExt, EngineError, EngineModule, EngineModuleName,
    EngineResourceKey, EngineRunReport, EngineRuntimeProfile, InteractiveEngineRunner,
    InteractiveWindowProfile, ModuleContext, ModuleDependency, ModuleError, ModuleOrder,
    RendererReport,
    stack::{
        DefaultEngineStack, ECS_MODULE_NAME, RENDERER_MODULE_NAME, SCHEDULER_MODULE_NAME,
        WINDOW_MODULE_NAME,
    },
};

use crate::{ClientAppOptions, ClientRuntimeMode, FunClientStartMode};

const CLIENT_RUNTIME_MODULE: EngineModuleName = match EngineModuleName::new("client_runtime") {
    Some(name) => name,
    None => panic!("client_runtime module name is invalid"),
};

pub const CLIENT_BOOT_OPTIONS_RESOURCE: EngineResourceKey =
    match EngineResourceKey::new("client.boot.options") {
        Some(key) => key,
        None => panic!("client boot options resource key is invalid"),
    };

const CLIENT_RUNTIME_DEPENDENCIES: &[ModuleDependency] = &[
    ModuleDependency::required(ECS_MODULE_NAME),
    ModuleDependency::required(SCHEDULER_MODULE_NAME),
    ModuleDependency::required(WINDOW_MODULE_NAME),
    ModuleDependency::required(RENDERER_MODULE_NAME),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClientFunLifecycleOptions {
    runtime_mode: ClientRuntimeMode,
    start_mode: FunClientStartMode,
    render_profile: &'static str,
    connects_to_server: bool,
    static_preview_stream: bool,
    gameplay_runtime: bool,
}

impl ClientFunLifecycleOptions {
    pub fn from_options(options: &ClientAppOptions) -> Self {
        Self {
            runtime_mode: options.mode,
            start_mode: options.start_mode,
            render_profile: options.render_profile.as_env_value(),
            connects_to_server: options.mode.should_connect_to_game_server(),
            static_preview_stream: options.mode.uses_static_preview_stream(),
            gameplay_runtime: options.mode.runs_gameplay_runtime(),
        }
    }

    pub const fn runtime_mode(self) -> ClientRuntimeMode {
        self.runtime_mode
    }

    pub const fn start_mode(self) -> FunClientStartMode {
        self.start_mode
    }

    pub const fn render_profile(self) -> &'static str {
        self.render_profile
    }

    pub const fn connects_to_server(self) -> bool {
        self.connects_to_server
    }

    pub const fn static_preview_stream(self) -> bool {
        self.static_preview_stream
    }

    pub const fn gameplay_runtime(self) -> bool {
        self.gameplay_runtime
    }
}

impl fun_ecs::Resource for ClientFunLifecycleOptions {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClientWindowIntent {
    mode: FunClientStartMode,
    title_intent: &'static str,
    startup_visible: bool,
    desired_width: u32,
    desired_height: u32,
}

impl ClientWindowIntent {
    pub fn from_options(options: &ClientAppOptions) -> Self {
        Self::for_start_mode(options.start_mode)
    }

    pub const fn for_start_mode(mode: FunClientStartMode) -> Self {
        Self {
            mode,
            title_intent: client_window_title_intent(mode),
            startup_visible: true,
            desired_width: 1280,
            desired_height: 720,
        }
    }

    #[must_use]
    pub const fn with_startup_visibility(mut self, startup_visible: bool) -> Self {
        self.startup_visible = startup_visible;
        self
    }

    #[must_use]
    pub const fn with_desired_size(mut self, width: u32, height: u32) -> Self {
        self.desired_width = width;
        self.desired_height = height;
        self
    }

    pub const fn mode(self) -> FunClientStartMode {
        self.mode
    }

    pub const fn title_intent(self) -> &'static str {
        self.title_intent
    }

    pub const fn startup_visible(self) -> bool {
        self.startup_visible
    }

    pub const fn desired_width(self) -> u32 {
        self.desired_width
    }

    pub const fn desired_height(self) -> u32 {
        self.desired_height
    }

    pub fn interactive_window_profile(self) -> Result<InteractiveWindowProfile, EngineError> {
        InteractiveWindowProfile::new(self.title_intent)
            .with_startup_visibility(self.startup_visible)
            .try_with_desired_size(self.desired_width, self.desired_height)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientEngineProfile {
    runtime_profile: EngineRuntimeProfile,
    window_intent: ClientWindowIntent,
}

impl ClientEngineProfile {
    pub fn from_options(options: &ClientAppOptions) -> Self {
        Self {
            runtime_profile: EngineRuntimeProfile::interactive(),
            window_intent: ClientWindowIntent::from_options(options),
        }
    }

    pub fn interactive(mut self) -> Self {
        self.runtime_profile = EngineRuntimeProfile::interactive();
        self.window_intent = self.window_intent.with_startup_visibility(true);
        self
    }

    pub const fn runtime_profile(&self) -> &EngineRuntimeProfile {
        &self.runtime_profile
    }

    pub const fn window_intent(&self) -> ClientWindowIntent {
        self.window_intent
    }

    pub fn interactive_window_profile(&self) -> Result<InteractiveWindowProfile, EngineError> {
        self.window_intent.interactive_window_profile()
    }
}

pub struct ClientRuntimeModule {
    lifecycle_options: ClientFunLifecycleOptions,
}

impl ClientRuntimeModule {
    pub fn new(options: &ClientAppOptions) -> Self {
        Self {
            lifecycle_options: ClientFunLifecycleOptions::from_options(options),
        }
    }
}

impl EngineModule for ClientRuntimeModule {
    fn name(&self) -> EngineModuleName {
        CLIENT_RUNTIME_MODULE
    }

    fn order(&self) -> ModuleOrder {
        ModuleOrder::Application
    }

    fn dependencies(&self) -> &'static [ModuleDependency] {
        CLIENT_RUNTIME_DEPENDENCIES
    }

    fn configure(&mut self, context: &mut ModuleContext<'_>) -> Result<(), ModuleError> {
        context.register_resource(CLIENT_BOOT_OPTIONS_RESOURCE, self.lifecycle_options)?;
        context.add_setup_step("client.runtime.options")
    }

    fn engine_boot(&mut self, context: &mut ModuleContext<'_>) -> Result<(), ModuleError> {
        let Some(world) = context.resource_mut::<fun_ecs::World>(fun_engine::stack::ECS_WORLD_KEY)
        else {
            return Err(ModuleError::missing_dependency(
                "client runtime requires fun-ecs world",
            ));
        };
        world.insert_resource(self.lifecycle_options);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClientRuntimeReport {
    lifecycle: ClientFunLifecycleOptions,
    engine: EngineRunReport,
    renderer: RendererReport,
}

impl ClientRuntimeReport {
    #[must_use]
    pub const fn new(lifecycle: ClientFunLifecycleOptions, engine: EngineRunReport) -> Self {
        Self {
            lifecycle,
            engine,
            renderer: engine.renderer(),
        }
    }

    #[must_use]
    pub const fn lifecycle(self) -> ClientFunLifecycleOptions {
        self.lifecycle
    }

    #[must_use]
    pub const fn engine(self) -> EngineRunReport {
        self.engine
    }

    #[must_use]
    pub const fn renderer(self) -> RendererReport {
        self.renderer
    }
}

#[derive(Debug)]
pub enum ClientBootError {
    Engine(EngineError),
}

impl fmt::Display for ClientBootError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Engine(error) => write!(formatter, "client engine boot failed: {error}"),
        }
    }
}

impl std::error::Error for ClientBootError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Engine(error) => Some(error),
        }
    }
}

impl From<EngineError> for ClientBootError {
    fn from(error: EngineError) -> Self {
        Self::Engine(error)
    }
}

pub fn build_client_fun_engine() -> Result<Engine, EngineError> {
    build_client_fun_engine_with_options(&ClientAppOptions::from_env())
}

pub fn build_client_fun_engine_with_options(
    options: &ClientAppOptions,
) -> Result<Engine, EngineError> {
    let profile = ClientEngineProfile::from_options(options).interactive();
    build_client_fun_engine_with_profile(options, profile)
}

pub fn build_client_fun_engine_with_profile(
    options: &ClientAppOptions,
    profile: ClientEngineProfile,
) -> Result<Engine, EngineError> {
    Engine::interactive()
        .with_stack(
            DefaultEngineStack::default()
                .with_primary_window_profile(profile.interactive_window_profile()?)
                .with_renderer_profile(profile.runtime_profile().renderer),
        )?
        .with_client_module(ClientRuntimeModule::new(options))?
        .build()
}

pub fn client_window_intent(options: &ClientAppOptions) -> ClientWindowIntent {
    ClientWindowIntent::from_options(options)
}

pub fn run_client_fun_engine() -> Result<ClientRuntimeReport, ClientBootError> {
    run_client_fun_engine_with_options(&ClientAppOptions::from_env())
}

pub fn run_client_fun_engine_with_options(
    options: &ClientAppOptions,
) -> Result<ClientRuntimeReport, ClientBootError> {
    let profile = ClientEngineProfile::from_options(options).interactive();
    let engine = build_client_fun_engine_with_profile(options, profile)?;
    let engine_report = InteractiveEngineRunner::winit(engine)?.run_report()?;
    Ok(ClientRuntimeReport::new(
        ClientFunLifecycleOptions::from_options(options),
        engine_report,
    ))
}

const fn client_window_title_intent(mode: FunClientStartMode) -> &'static str {
    match mode {
        FunClientStartMode::Launcher => "FUN Launcher",
        FunClientStartMode::Game => "FUN Game",
        FunClientStartMode::Editor => "FUN Editor",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fun_engine::stack::{
        ECS_RUNTIME_KEY, RENDERER_RUNTIME_KEY, RendererRuntime, SCHEDULER_RUNTIME_KEY,
        SchedulerRuntime, WINDOW_RUNTIME_KEY, WindowRuntimeState,
    };

    #[test]
    fn fun_engine_boot_contract_registers_default_stack_modules() -> Result<(), EngineError> {
        let options = ClientAppOptions::default();
        let mut engine = build_client_fun_engine_with_options(&options)?;
        engine.run_boot_phases()?;

        assert_eq!(
            *engine.config().runtime_profile(),
            fun_engine::EngineRuntimeProfile::interactive()
        );
        assert_eq!(
            engine
                .integrations()
                .ecs()
                .map(|contract| contract.provider()),
            Some("fun-ecs")
        );
        assert_eq!(
            engine
                .integrations()
                .scheduler()
                .map(|contract| contract.provider()),
            Some("fun-scheduler")
        );
        assert_eq!(
            engine
                .integrations()
                .windowing()
                .map(|contract| contract.provider()),
            Some(concat!("fun", "-", "window"))
        );
        assert!(engine.integrations().renderer().is_some());
        assert!(
            engine
                .resources()
                .get::<SchedulerRuntime>(SCHEDULER_RUNTIME_KEY)
                .is_some()
        );
        assert!(
            engine
                .resources()
                .get::<WindowRuntimeState>(WINDOW_RUNTIME_KEY)
                .is_some()
        );
        assert!(
            engine
                .resources()
                .get::<RendererRuntime>(RENDERER_RUNTIME_KEY)
                .is_some()
        );
        assert!(
            engine
                .resources()
                .get::<fun_engine::stack::EcsRuntime>(ECS_RUNTIME_KEY)
                .is_some()
        );

        let Some(world) = engine
            .resources()
            .get::<fun_ecs::World>(fun_engine::stack::ECS_WORLD_KEY)
        else {
            return Err(EngineError::run_failed("client fun-ecs world missing"));
        };
        assert!(world.contains_resource::<ClientFunLifecycleOptions>());
        Ok(())
    }

    #[test]
    fn client_window_intent_maps_to_interactive_window_profile() {
        let options = ClientAppOptions::default();
        let intent = client_window_intent(&options);
        let profile = intent
            .interactive_window_profile()
            .expect("default client window intent is valid");

        assert_eq!(intent.mode(), FunClientStartMode::Launcher);
        assert_eq!(intent.title_intent(), "FUN Launcher");
        assert!(intent.startup_visible());
        assert_eq!(intent.desired_width(), 1280);
        assert_eq!(intent.desired_height(), 720);
        assert!(profile.startup_visible());
        assert_eq!(profile.title_intent(), "FUN Launcher");
        assert_eq!(profile.desired_size().width(), 1280);
        assert_eq!(profile.desired_size().height(), 720);
    }

    #[test]
    fn client_engine_profile_is_interactive_intent() {
        let options = ClientAppOptions::default();
        let profile = ClientEngineProfile::from_options(&options).interactive();

        assert_eq!(
            *profile.runtime_profile(),
            fun_engine::EngineRuntimeProfile::interactive()
        );
        assert_eq!(profile.window_intent().title_intent(), "FUN Launcher");
        let window_profile = profile
            .interactive_window_profile()
            .expect("default client window intent is valid");
        assert!(window_profile.startup_visible());
        assert_eq!(window_profile.desired_size().width(), 1280);
        assert_eq!(window_profile.desired_size().height(), 720);
    }

    #[test]
    fn client_runtime_report_observes_engine_and_renderer_reports() {
        let options = ClientAppOptions::default();
        let lifecycle = ClientFunLifecycleOptions::from_options(&options);
        let engine = EngineRunReport::new(
            fun_engine::EngineExit::Completed { frames_executed: 1 },
            RendererReport::unavailable(),
        );
        let report = ClientRuntimeReport::new(lifecycle, engine);

        assert_eq!(report.lifecycle(), lifecycle);
        assert_eq!(report.engine().exit(), engine.exit());
        assert_eq!(report.renderer(), engine.renderer());
        assert!(!report.renderer().available());
    }

    #[test]
    fn renderer_boundary_keeps_game_client_out_of_gpu_ownership() {
        let manifest = include_str!("../Cargo.toml");
        for dependency in [
            concat!("fun", "_", "window"),
            concat!("fun", "_", "renderer"),
            concat!("poll", "ster"),
            concat!("w", "gpu"),
        ] {
            assert!(
                !manifest_has_direct_dependency(manifest, dependency),
                "game_client must not directly depend on {dependency}"
            );
        }

        let boot_source = include_str!("fun_engine_boot.rs");
        for forbidden in [
            concat!("w", "gpu", "::"),
            concat!("poll", "ster", "::", "block", "_on"),
            concat!("block", "_on"),
            concat!("Winit", "Surface", "Frame"),
            concat!("run", "_", "one", "_", "surface", "_", "fra", "me"),
            concat!(
                "run", "_", "one", "_", "w", "gpu", "_", "surface", "_", "fra", "me"
            ),
            concat!("W", "gpu", "Surface", "Adapter"),
            concat!("Surface", "Configuration"),
            concat!("Render", "Pass", "Descriptor"),
            concat!("Command", "Encoder", "Descriptor"),
            concat!("fun", "_", "renderer", "::"),
            concat!("Window", "Spec"),
            concat!("fun_engine", "::", "window"),
            concat!("Client", "Surface", "Frame", "Report"),
            concat!("Client", "Renderer", "Error"),
        ] {
            assert!(
                !boot_source.contains(forbidden),
                "game_client boot source must not own renderer token {forbidden}"
            );
        }
    }

    fn manifest_has_direct_dependency(manifest: &str, dependency: &str) -> bool {
        manifest.lines().any(|line| {
            let trimmed = line.trim_start();
            let Some(rest) = trimmed.strip_prefix(dependency) else {
                return false;
            };
            rest.starts_with([' ', '.', '='])
        })
    }
}
