#[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
use bevy::{anti_alias::dlss::DlssProjectId, asset::uuid::Uuid};
use bevy::{
    ecs::world::World,
    prelude::*,
    render::{
        error_handler::{ErrorType, RenderError, RenderErrorHandler, RenderErrorPolicy},
        settings::Backends,
    },
    window::{PresentMode, WindowResolution},
    winit::WinitSettings,
};
use tracing::info;

use crate::{
    ClientWindowConfig, FunRenderAppOptions,
    config::{
        NativeDlssConfig, client_render_creation, log_native_dlss_startup_diagnostics,
        render_plugin,
    },
    selected_max_frame_latency, selected_present_mode, selected_render_backend,
};

#[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
const DLSS_PROJECT_ID: &str = "7f2c56d9-bbd1-40e6-aeea-ad1cde733e2e";

#[derive(Debug, Clone, Copy, Resource)]
struct FunWinitRenderStartupDiagnostics {
    runtime_mode: &'static str,
    render_profile: crate::ClientRenderProfile,
    backend: Backends,
    present_mode: PresentMode,
    max_frame_latency: std::num::NonZeroU32,
    maximized: bool,
    requested_width: Option<u32>,
    requested_height: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct FunRenderWinitPresentationPlugin {
    options: FunRenderAppOptions,
}

impl FunRenderWinitPresentationPlugin {
    pub const fn new(options: FunRenderAppOptions) -> Self {
        Self { options }
    }
}

impl Plugin for FunRenderWinitPresentationPlugin {
    fn build(&self, app: &mut App) {
        let render_backend = selected_render_backend();
        let present_mode = selected_present_mode();
        let max_frame_latency = selected_max_frame_latency();
        let window_config = ClientWindowConfig::from_env();

        #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
        app.insert_resource(DlssProjectId(
            Uuid::parse_str(DLSS_PROJECT_ID).expect("DLSS project ID should be a valid UUID"),
        ));

        let title = format!("{} Client", game_shared::GAME_TITLE);
        let default_plugins = DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title,
                present_mode,
                resolution: window_config.resolution(),
                desired_maximum_frame_latency: Some(max_frame_latency),
                ..default()
            }),
            ..default()
        });
        let default_plugins = default_plugins.set(render_plugin(render_backend));

        app.add_plugins(default_plugins)
            .insert_resource(FunWinitRenderStartupDiagnostics {
                runtime_mode: self.options.runtime_mode,
                render_profile: self.options.render_profile,
                backend: render_backend,
                present_mode,
                max_frame_latency,
                maximized: window_config.maximized,
                requested_width: window_config.requested_width(),
                requested_height: window_config.requested_height(),
            })
            .insert_resource(window_config)
            .insert_resource(RenderErrorHandler(recover_render_device))
            .insert_resource(WinitSettings::continuous())
            .add_systems(Startup, log_winit_render_startup_diagnostics);
    }
}

fn log_winit_render_startup_diagnostics(diagnostics: Res<FunWinitRenderStartupDiagnostics>) {
    info!(
        target: "fun::render",
        runtime_mode = diagnostics.runtime_mode,
        render_profile = diagnostics.render_profile.as_env_value(),
        backend = ?diagnostics.backend,
        present_mode = ?diagnostics.present_mode,
        vsync = false,
        max_frame_latency = diagnostics.max_frame_latency.get(),
        maximized = diagnostics.maximized,
        "Fun Winit render backend selected"
    );
    info!(
        target: "fun::render",
        "[fun render] presentation: render.backend={:?} render.adapter=unknown render.driver=unknown render.present_mode={:?} render.desired_maximum_frame_latency={} render.vrr_detected=unknown render.hdr_active=unknown render.swapchain_format=unknown render.window_mode={} render.resolution_width={} render.resolution_height={}",
        diagnostics.backend,
        diagnostics.present_mode,
        diagnostics.max_frame_latency.get(),
        if diagnostics.maximized { "maximized_window" } else { "windowed" },
        diagnostics.requested_width.unwrap_or(0),
        diagnostics.requested_height.unwrap_or(0),
    );
    log_native_dlss_startup_diagnostics(diagnostics.backend, NativeDlssConfig::from_env());
}

fn recover_render_device(
    error: &RenderError,
    _main_world: &mut World,
    _render_world: &mut World,
) -> RenderErrorPolicy {
    info!(
        "[fun render] renderer reported {:?}: {}; recreating renderer with the same profile",
        error.ty, error.description
    );

    match error.ty {
        ErrorType::DeviceLost | ErrorType::OutOfMemory | ErrorType::Internal => {
            RenderErrorPolicy::Recover(client_render_creation(selected_render_backend()))
        }
        ErrorType::Validation => {
            info!(
                "[fun render] validation error may be fallout from a lost GPU device; attempting immediate renderer recovery"
            );
            RenderErrorPolicy::Recover(client_render_creation(selected_render_backend()))
        }
    }
}

#[allow(dead_code, reason = "offscreen presentation owns target sizing")]
pub(crate) fn offscreen_resolution(target_width: u32, target_height: u32) -> WindowResolution {
    WindowResolution::new(target_width.max(1), target_height.max(1))
}
