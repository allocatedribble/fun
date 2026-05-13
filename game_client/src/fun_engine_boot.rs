use core::fmt;

use fun_engine::{
    Engine, EngineError, EngineModule, EngineModuleName, EngineResourceKey, ModuleContext,
    ModuleDependency, ModuleError, ModuleOrder,
    stack::{
        CoreModule, ECS_MODULE_NAME, EcsModule, RENDERER_MODULE_NAME, RendererModule,
        SCHEDULER_MODULE_NAME, SchedulerModule, WINDOW_MODULE_NAME, WindowModule,
    },
};
use fun_window::{
    SurfaceError, WindowError, WindowSize, WindowSpec,
    backend::winit::{WinitSurfaceFrame, run_one_surface_frame},
    surface::wgpu::WgpuSurfaceAdapter,
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
pub struct ClientSurfaceFrameReport {
    window_id: u64,
    width: u32,
    height: u32,
    scale_microunits: u32,
}

impl ClientSurfaceFrameReport {
    pub const fn window_id(self) -> u64 {
        self.window_id
    }

    pub const fn width(self) -> u32 {
        self.width
    }

    pub const fn height(self) -> u32 {
        self.height
    }

    pub const fn scale_microunits(self) -> u32 {
        self.scale_microunits
    }
}

#[derive(Debug)]
pub enum ClientBootError {
    Engine(EngineError),
    Window(WindowError),
    Renderer(ClientRendererError),
}

impl fmt::Display for ClientBootError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Engine(error) => write!(formatter, "client engine boot failed: {error}"),
            Self::Window(error) => write!(formatter, "client window boot failed: {error}"),
            Self::Renderer(error) => write!(formatter, "client renderer surface failed: {error}"),
        }
    }
}

impl std::error::Error for ClientBootError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Engine(error) => Some(error),
            Self::Window(error) => Some(error),
            Self::Renderer(error) => Some(error),
        }
    }
}

impl From<EngineError> for ClientBootError {
    fn from(error: EngineError) -> Self {
        Self::Engine(error)
    }
}

impl From<WindowError> for ClientBootError {
    fn from(error: WindowError) -> Self {
        Self::Window(error)
    }
}

impl From<ClientRendererError> for ClientBootError {
    fn from(error: ClientRendererError) -> Self {
        Self::Renderer(error)
    }
}

#[derive(Debug)]
pub enum ClientRendererError {
    AdapterUnavailable,
    DeviceUnavailable,
    FrameNotRendered,
    NoPresentModes,
    NoSurfaceFormats,
    SurfaceLost,
    SurfaceOccluded,
    SurfaceOutdated,
    SurfaceTimeout,
    SurfaceValidation,
    SurfaceAdapter(SurfaceError),
}

impl fmt::Display for ClientRendererError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::AdapterUnavailable => "graphics adapter unavailable",
            Self::DeviceUnavailable => "graphics device unavailable",
            Self::FrameNotRendered => "surface frame was not rendered",
            Self::NoPresentModes => "surface reports no present modes",
            Self::NoSurfaceFormats => "surface reports no formats",
            Self::SurfaceLost => "surface was lost",
            Self::SurfaceOccluded => "surface is occluded",
            Self::SurfaceOutdated => "surface is outdated",
            Self::SurfaceTimeout => "surface acquisition timed out",
            Self::SurfaceValidation => "surface validation failed",
            Self::SurfaceAdapter(_) => "surface adapter failed",
        })
    }
}

impl std::error::Error for ClientRendererError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SurfaceAdapter(error) => Some(error),
            _ => None,
        }
    }
}

pub fn build_client_fun_engine() -> Result<Engine, EngineError> {
    build_client_fun_engine_with_options(&ClientAppOptions::from_env())
}

pub fn build_client_fun_engine_with_options(
    options: &ClientAppOptions,
) -> Result<Engine, EngineError> {
    build_client_fun_engine_with_window_spec(options, client_primary_window_spec(options))
}

pub fn build_client_fun_engine_with_window_spec(
    options: &ClientAppOptions,
    window_spec: WindowSpec,
) -> Result<Engine, EngineError> {
    Engine::builder()
        .with_module(CoreModule)?
        .with_module(SchedulerModule::default())?
        .with_module(EcsModule)?
        .with_module(WindowModule::primary(window_spec))?
        .with_module(RendererModule::wgpu_default())?
        .with_module(ClientRuntimeModule::new(options))?
        .build()
}

pub fn client_primary_window_spec(options: &ClientAppOptions) -> WindowSpec {
    WindowSpec::default()
        .with_title(client_window_title(options))
        .with_logical_size(WindowSize::DEFAULT)
        .with_visible(true)
}

pub fn run_client_fun_engine() -> Result<fun_engine::EngineExit, ClientBootError> {
    run_client_fun_engine_with_options(&ClientAppOptions::from_env())
}

pub fn run_client_fun_engine_with_options(
    options: &ClientAppOptions,
) -> Result<fun_engine::EngineExit, ClientBootError> {
    let window_spec = client_primary_window_spec(options);
    let mut engine = build_client_fun_engine_with_window_spec(options, window_spec.clone())?;
    let mut surface_result = Err(ClientRendererError::FrameNotRendered);

    run_one_surface_frame(window_spec, |frame| {
        surface_result = pollster::block_on(render_client_surface_frame(frame));
        Ok(())
    })?;

    surface_result?;
    Ok(engine.run()?)
}

async fn render_client_surface_frame(
    frame: WinitSurfaceFrame<'_>,
) -> Result<ClientSurfaceFrameReport, ClientRendererError> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        flags: wgpu::InstanceFlags::from_build_config(),
        memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
        backend_options: wgpu::BackendOptions::default(),
        display: None,
    });
    let surface = WgpuSurfaceAdapter::create_surface(&instance, frame.handles())
        .map_err(ClientRendererError::SurfaceAdapter)?;
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        })
        .await
        .map_err(|_error| ClientRendererError::AdapterUnavailable)?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("fun.game_client.device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        })
        .await
        .map_err(|_error| ClientRendererError::DeviceUnavailable)?;
    let capabilities = surface.get_capabilities(&adapter);
    let format = select_surface_format(&capabilities.formats)?;
    let present_mode = select_present_mode(&capabilities.present_modes)?;
    let alpha_mode = capabilities
        .alpha_modes
        .first()
        .copied()
        .unwrap_or(wgpu::CompositeAlphaMode::Auto);
    let size = frame.physical_size();
    let configuration = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: size.width(),
        height: size.height(),
        present_mode,
        desired_maximum_frame_latency: 2,
        alpha_mode,
        view_formats: vec![format],
    };
    surface.configure(&device, &configuration);
    present_client_surface_frame(&device, &queue, &surface)?;
    let scale = frame.dpi_scale().get();
    let scale_microunits = if scale.is_finite() && scale > 0.0 {
        (scale * 1_000_000.0).round() as u32
    } else {
        1_000_000
    };
    Ok(ClientSurfaceFrameReport {
        window_id: frame.window().get(),
        width: size.width(),
        height: size.height(),
        scale_microunits,
    })
}

fn present_client_surface_frame(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    surface: &wgpu::Surface<'_>,
) -> Result<(), ClientRendererError> {
    let surface_texture = match surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(texture)
        | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
        wgpu::CurrentSurfaceTexture::Timeout => return Err(ClientRendererError::SurfaceTimeout),
        wgpu::CurrentSurfaceTexture::Occluded => return Err(ClientRendererError::SurfaceOccluded),
        wgpu::CurrentSurfaceTexture::Outdated => return Err(ClientRendererError::SurfaceOutdated),
        wgpu::CurrentSurfaceTexture::Lost => return Err(ClientRendererError::SurfaceLost),
        wgpu::CurrentSurfaceTexture::Validation => {
            return Err(ClientRendererError::SurfaceValidation);
        }
    };
    let surface_view = surface_texture
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("fun.game_client.clear.encoder"),
    });
    {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("fun.game_client.clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &surface_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.02,
                        g: 0.04,
                        b: 0.06,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }
    queue.submit([encoder.finish()]);
    surface_texture.present();
    Ok(())
}

fn select_surface_format(
    formats: &[wgpu::TextureFormat],
) -> Result<wgpu::TextureFormat, ClientRendererError> {
    formats
        .iter()
        .copied()
        .find(wgpu::TextureFormat::is_srgb)
        .or_else(|| formats.first().copied())
        .ok_or(ClientRendererError::NoSurfaceFormats)
}

fn select_present_mode(
    present_modes: &[wgpu::PresentMode],
) -> Result<wgpu::PresentMode, ClientRendererError> {
    if present_modes.contains(&wgpu::PresentMode::Fifo) {
        return Ok(wgpu::PresentMode::Fifo);
    }
    present_modes
        .first()
        .copied()
        .ok_or(ClientRendererError::NoPresentModes)
}

fn client_window_title(options: &ClientAppOptions) -> &'static str {
    match options.start_mode {
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
        let exit = engine.run()?;

        assert_eq!(
            exit,
            fun_engine::EngineExit::Completed { frames_executed: 1 }
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
            Some("fun-window")
        );
        assert_eq!(
            engine
                .integrations()
                .renderer()
                .map(|contract| contract.provider()),
            Some("fun-renderer.wgpu")
        );
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
    fn client_window_spec_is_visible_and_titled() {
        let options = ClientAppOptions::default();
        let spec = client_primary_window_spec(&options);

        assert!(spec.visible());
        assert_eq!(spec.title(), "FUN Launcher");
        assert_eq!(spec.logical_size(), WindowSize::DEFAULT);
    }
}
