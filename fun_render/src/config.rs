use bevy::{
    pbr::DefaultOpaqueRendererMethod,
    prelude::*,
    render::{
        RenderPlugin,
        settings::{Backends, InstanceFlags, RenderCreation, WgpuSettings},
    },
    window::{PresentMode, WindowResolution},
};
use tracing::{info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientRenderProfile {
    Default,
    Diagnostics,
}

impl ClientRenderProfile {
    pub fn from_env() -> Self {
        let Ok(value) = std::env::var("FUN_CLIENT_RENDER_PROFILE") else {
            return Self::Default;
        };
        Self::parse(&value).unwrap_or_else(|| {
            warn!(
                target: "fun::render",
                render_profile = value,
                "unknown FUN_CLIENT_RENDER_PROFILE; using default"
            );
            Self::Default
        })
    }

    pub fn parse(value: &str) -> Option<Self> {
        if value.eq_ignore_ascii_case("default") || value.eq_ignore_ascii_case("debug") {
            return Some(Self::Default);
        }
        if value.eq_ignore_ascii_case("diagnostics")
            || value.eq_ignore_ascii_case("debug_diagnostics")
            || value.eq_ignore_ascii_case("debug+diagnostics")
        {
            return Some(Self::Diagnostics);
        }
        None
    }

    pub const fn as_env_value(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Diagnostics => "diagnostics",
        }
    }
}

#[derive(Debug, Clone, Copy, Resource)]
pub struct ClientRenderConfig {
    pub solari_enabled: bool,
    pub dlss_rr_enabled: bool,
    pub meshlets_enabled: bool,
    pub dlss_rr_disabled_by_denoise_mode: bool,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    pub render_profile_verbose: bool,
    pub geometry_policy: RenderGeometryPolicy,
    pub meshlet_min_triangles: usize,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    pub fps_overlay_enabled: bool,
}

impl ClientRenderConfig {
    pub fn from_env() -> Self {
        Self {
            solari_enabled: std::env::var_os("FUN_DISABLE_SOLARI").is_none(),
            dlss_rr_enabled: std::env::var_os("FUN_DISABLE_DLSS_RR").is_none(),
            meshlets_enabled: std::env::var_os("FUN_DISABLE_MESHLETS").is_none(),
            dlss_rr_disabled_by_denoise_mode: false,
            #[cfg(all(feature = "render_diagnostics", debug_assertions))]
            render_profile_verbose: std::env::var_os("FUN_RENDER_PROFILE_VERBOSE").is_some(),
            geometry_policy: RenderGeometryPolicy::from_env(),
            meshlet_min_triangles: env_usize("FUN_MESHLET_MIN_TRIANGLES", 512),
            #[cfg(all(feature = "render_diagnostics", debug_assertions))]
            fps_overlay_enabled: std::env::var_os("FUN_DISABLE_FPS_OVERLAY").is_none(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderGeometryPolicy {
    Hybrid,
    AllMeshlet,
    AllRaster,
}

impl RenderGeometryPolicy {
    pub fn from_env() -> Self {
        match std::env::var("FUN_RENDER_GEOMETRY_POLICY")
            .as_deref()
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Ok("all_meshlet") | Ok("all-meshlet") | Ok("meshlet") => Self::AllMeshlet,
            Ok("all_raster") | Ok("all-raster") | Ok("raster") => Self::AllRaster,
            Ok("hybrid") | Ok("") | Err(_) => Self::Hybrid,
            Ok(other) => {
                warn!(
                    target: "fun::render",
                    policy = other,
                    "unknown FUN_RENDER_GEOMETRY_POLICY; using hybrid"
                );
                Self::Hybrid
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub enum RenderGeometryClass {
    SimpleRaster,
    MeshletStaticDense,
    MeshletDynamicDense,
    RayProxyOnly,
    Viewmodel,
}

impl RenderGeometryClass {
    pub const fn uses_meshlet(self) -> bool {
        matches!(
            self,
            RenderGeometryClass::MeshletStaticDense | RenderGeometryClass::MeshletDynamicDense
        )
    }

    pub const fn uses_raster_mesh(self) -> bool {
        matches!(
            self,
            RenderGeometryClass::SimpleRaster | RenderGeometryClass::Viewmodel
        )
    }
}

#[derive(Debug, Clone, Copy, Resource)]
pub struct ClientWindowConfig {
    pub maximized: bool,
    width: Option<u32>,
    height: Option<u32>,
}

impl ClientWindowConfig {
    pub fn from_env() -> Self {
        Self {
            maximized: std::env::var_os("FUN_WINDOW_MAXIMIZED").is_some(),
            width: env_u32_opt("FUN_WINDOW_WIDTH"),
            height: env_u32_opt("FUN_WINDOW_HEIGHT"),
        }
    }

    pub fn resolution(&self) -> WindowResolution {
        match (self.width, self.height) {
            (Some(width), Some(height)) => WindowResolution::new(width, height),
            _ => WindowResolution::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientOpaqueRenderer {
    Deferred,
    Forward,
}

impl ClientOpaqueRenderer {
    pub(crate) fn method(self) -> DefaultOpaqueRendererMethod {
        match self {
            Self::Deferred => DefaultOpaqueRendererMethod::deferred(),
            Self::Forward => DefaultOpaqueRendererMethod::forward(),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Deferred => "deferred",
            Self::Forward => "forward",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunRenderPresentation {
    WinitWindow,
    EditorOffscreen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunRenderAppOptions {
    pub runtime_mode: &'static str,
    pub render_profile: ClientRenderProfile,
    pub hosted_by_editor: bool,
    pub presentation: FunRenderPresentation,
}

impl FunRenderAppOptions {
    pub const fn winit_client(
        runtime_mode: &'static str,
        render_profile: ClientRenderProfile,
        hosted_by_editor: bool,
    ) -> Self {
        Self {
            runtime_mode,
            render_profile,
            hosted_by_editor,
            presentation: FunRenderPresentation::WinitWindow,
        }
    }

    pub const fn editor_offscreen(render_profile: ClientRenderProfile) -> Self {
        Self {
            runtime_mode: "editor_static_preview",
            render_profile,
            hosted_by_editor: true,
            presentation: FunRenderPresentation::EditorOffscreen,
        }
    }

    pub fn is_editor_preview(self) -> bool {
        matches!(self.presentation, FunRenderPresentation::EditorOffscreen)
            || self.runtime_mode == "editor_preview"
    }
}

#[cfg_attr(
    not(any(feature = "offscreen", feature = "winit_presentation")),
    allow(dead_code)
)]
pub(crate) fn client_render_creation(render_backend: Backends) -> RenderCreation {
    RenderCreation::Automatic(Box::new(WgpuSettings {
        backends: Some(render_backend),
        instance_flags: InstanceFlags::empty().with_env(),
        ..default()
    }))
}

#[cfg_attr(
    not(any(feature = "offscreen", feature = "winit_presentation")),
    allow(dead_code)
)]
pub(crate) fn render_plugin(render_backend: Backends) -> RenderPlugin {
    RenderPlugin {
        render_creation: client_render_creation(render_backend),
        ..default()
    }
}

pub fn selected_render_backend() -> Backends {
    match std::env::var("FUN_RENDER_BACKEND")
        .as_deref()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Ok("dx12") | Ok("d3d12") | Ok("directx12") => Backends::DX12,
        Ok("auto") => Backends::VULKAN | Backends::DX12,
        Ok("vulkan") | Ok("vk") | Ok("") | Err(_) => Backends::VULKAN,
        Ok(other) => {
            info!("[fun render] unknown FUN_RENDER_BACKEND={other}; using Vulkan");
            Backends::VULKAN
        }
    }
}

pub fn selected_present_mode() -> PresentMode {
    match std::env::var("FUN_PRESENT_MODE")
        .as_deref()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Ok("auto_no_vsync") | Ok("autonovsync") | Ok("auto-no-vsync") => PresentMode::AutoNoVsync,
        Ok("auto_vsync") | Ok("autovsync") | Ok("auto-vsync") => PresentMode::AutoVsync,
        Ok("fifo") | Ok("vsync") => PresentMode::Fifo,
        Ok("fifo_relaxed") | Ok("fifo-relaxed") => PresentMode::FifoRelaxed,
        Ok("mailbox") => PresentMode::Mailbox,
        Ok("immediate") | Ok("") | Err(_) => PresentMode::Immediate,
        Ok(other) => {
            info!("[fun render] unknown FUN_PRESENT_MODE={other}; using Immediate");
            PresentMode::Immediate
        }
    }
}

fn env_u32_opt(name: &str) -> Option<u32> {
    let value = std::env::var(name).ok()?;
    match value.parse::<u32>() {
        Ok(parsed) if parsed > 0 => Some(parsed),
        _ => {
            warn!(
                target: "fun::render",
                setting = name,
                value,
                "ignored invalid positive integer setting"
            );
            None
        }
    }
}

fn env_usize(name: &str, default_value: usize) -> usize {
    let Ok(value) = std::env::var(name) else {
        return default_value;
    };
    match value.parse::<usize>() {
        Ok(parsed) if parsed > 0 => parsed,
        _ => {
            warn!(
                target: "fun::render",
                setting = name,
                value,
                default_value,
                "ignored invalid positive integer setting"
            );
            default_value
        }
    }
}
