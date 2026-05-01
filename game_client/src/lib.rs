mod compiled_world;
mod editor_hotkey;
pub mod first_person;
mod frame_profile;
mod render_catalog;

pub(crate) use frame_profile::{
    frame_profile_elapsed, frame_profile_ns, frame_profile_scope, frame_profile_start,
};

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
use frame_profile::DetailedFrameProfiler;
#[cfg(all(feature = "render_diagnostics", debug_assertions))]
use frame_profile::install_detailed_frame_profiler;
#[cfg(all(feature = "diagnostics", debug_assertions))]
use render_catalog::catalog_ref_summary;
use render_catalog::{WorldRenderCatalog, prewarm_world_render_catalog, warn_missing_catalog_ref};

use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    num::NonZeroU32,
};
#[cfg(all(feature = "render_diagnostics", debug_assertions))]
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

use avian3d::prelude::{Collider, PhysicsPlugins, RigidBody};
#[cfg(all(feature = "render_diagnostics", debug_assertions))]
use bevy::render::error_handler::RenderRecoveryStatus;
use bevy::render::{
    RenderPlugin,
    error_handler::{ErrorType, RenderError, RenderErrorHandler, RenderErrorPolicy},
    render_resource::TextureUsages,
    settings::{Backends, InstanceFlags, RenderCreation, WgpuSettings},
};
#[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
use bevy::{
    anti_alias::dlss::{
        Dlss, DlssPerfQualityMode, DlssProjectId, DlssRayReconstructionFeature,
        DlssRayReconstructionSupported,
    },
    asset::uuid::Uuid,
};
use bevy::{
    app::AppExit,
    camera::CameraMainTextureUsages,
    ecs::world::World,
    pbr::{
        DefaultOpaqueRendererMethod,
        experimental::meshlet::{
            MESHLET_DEFAULT_VERTEX_POSITION_QUANTIZATION_FACTOR, MeshletMesh, MeshletMesh3d,
            MeshletPlugin,
        },
    },
    prelude::*,
    solari::prelude::{
        RaytracingMesh3d, SolariArchitecture, SolariDebugOverlay, SolariDenoiseMode,
        SolariInternalScale, SolariLighting, SolariPlugins, SolariResetEvent, SolariRuntimeParams,
        SolariSettings, SolariVisualTarget,
    },
    window::{PresentMode, PrimaryWindow, WindowResolution},
    winit::WinitSettings,
};
#[cfg(all(feature = "render_diagnostics", debug_assertions))]
use bevy::{
    dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin},
    diagnostic::{DiagnosticPath, DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    render::diagnostic::RenderDiagnosticsPlugin,
};
use bevy_quinnet::client::{
    ClientConnectionConfiguration, ClientConnectionConfigurationDefaultables, QuinnetClient,
    QuinnetClientPlugin,
    certificate::CertificateVerificationMode,
    connection::{ClientAddrConfiguration, ConnectionEvent},
};
use editor_hotkey::EditorHotkeyPlugin;
use first_person::FirstPersonControllerPlugin;
#[cfg(all(feature = "render_diagnostics", debug_assertions))]
use game_shared::DEFAULT_RENDER_TARGET_RATE_HZ;
use game_shared::{DEFAULT_TICK_RATE_HZ, EditorInputOwner, GAME_SERVER_ADDR, GAME_TITLE};
use thunder::prelude::*;
use tracing::{debug, error, info, warn};

#[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
const DLSS_PROJECT_ID: &str = "7f2c56d9-bbd1-40e6-aeea-ad1cde733e2e";
#[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
const DLSS_RR_MODE: DlssPerfQualityMode = DlssPerfQualityMode::Quality;
const FUN_CLIENT_RENDER_PATH_SIGNATURE_ID: &str = "fun-client-render-path-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientRuntimeMode {
    JoinedGame,
    StandaloneClient,
    EditorHostedClient,
    EditorPreview,
}

impl ClientRuntimeMode {
    fn from_env() -> Self {
        let Some((env_name, value)) = env_non_empty_string("FUN_CLIENT_MODE")
            .map(|value| ("FUN_CLIENT_MODE", value))
            .or_else(|| {
                env_non_empty_string("FUN_CLIENT_RUNTIME_MODE")
                    .map(|value| ("FUN_CLIENT_RUNTIME_MODE", value))
            })
        else {
            return Self::JoinedGame;
        };
        Self::parse(&value).unwrap_or_else(|| {
            warn!(
                target: "fun::client",
                env_name,
                runtime_mode = value,
                "unknown client runtime mode; using joined_game"
            );
            Self::JoinedGame
        })
    }

    fn parse(value: &str) -> Option<Self> {
        if value.eq_ignore_ascii_case("joined_game")
            || value.eq_ignore_ascii_case("joined-game")
            || value.eq_ignore_ascii_case("joined")
        {
            return Some(Self::JoinedGame);
        }
        if value.eq_ignore_ascii_case("standalone_client")
            || value.eq_ignore_ascii_case("standalone-client")
            || value.eq_ignore_ascii_case("standalone")
        {
            return Some(Self::StandaloneClient);
        }
        if value.eq_ignore_ascii_case("editor_hosted_client")
            || value.eq_ignore_ascii_case("editor-hosted-client")
            || value.eq_ignore_ascii_case("editor_hosted")
        {
            return Some(Self::EditorHostedClient);
        }
        if value.eq_ignore_ascii_case("editor_preview")
            || value.eq_ignore_ascii_case("editor-preview")
            || value.eq_ignore_ascii_case("preview")
        {
            return Some(Self::EditorPreview);
        }
        None
    }

    fn should_connect_to_game_server(self) -> bool {
        matches!(self, Self::JoinedGame | Self::EditorHostedClient)
    }

    fn uses_static_preview_stream(self) -> bool {
        matches!(self, Self::EditorPreview)
    }

    fn runs_gameplay_runtime(self) -> bool {
        !self.uses_static_preview_stream()
    }

    pub(crate) const fn supports_editor_activation(self) -> bool {
        matches!(self, Self::JoinedGame | Self::EditorHostedClient)
    }

    pub(crate) fn as_env_value(self) -> &'static str {
        match self {
            Self::JoinedGame => "joined_game",
            Self::StandaloneClient => "standalone_client",
            Self::EditorHostedClient => "editor_hosted_client",
            Self::EditorPreview => "editor_preview",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientRenderProfile {
    Default,
    Diagnostics,
}

impl ClientRenderProfile {
    fn from_env() -> Self {
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

    fn parse(value: &str) -> Option<Self> {
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

    fn as_env_value(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Diagnostics => "diagnostics",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Resource)]
pub struct ClientAppOptions {
    pub mode: ClientRuntimeMode,
    pub render_profile: ClientRenderProfile,
    pub server_addr: Option<String>,
    pub project_id: Option<String>,
    pub game_session_id: Option<String>,
    pub scene_id: Option<String>,
    pub hosted_by_editor: bool,
    pub host_instance_id: Option<String>,
    pub host_parent_pid: Option<u32>,
}

impl ClientAppOptions {
    pub fn from_env() -> Self {
        let mode = ClientRuntimeMode::from_env();
        Self {
            mode,
            render_profile: ClientRenderProfile::from_env(),
            server_addr: env_non_empty_string("FUN_SERVER_ADDR"),
            project_id: env_non_empty_string("FUN_PROJECT_ID"),
            game_session_id: env_non_empty_string("FUN_GAME_SESSION_ID"),
            scene_id: env_non_empty_string("FUN_SCENE_ID"),
            hosted_by_editor: env_flag("FUN_HOSTED_BY_EDITOR")
                || env_flag("FUN_CLIENT_HOSTED_BY_EDITOR")
                || matches!(
                    mode,
                    ClientRuntimeMode::EditorHostedClient | ClientRuntimeMode::EditorPreview
                ),
            host_instance_id: env_non_empty_string("FUN_HOST_INSTANCE_ID"),
            host_parent_pid: env_host_parent_pid(),
        }
    }
}

impl Default for ClientAppOptions {
    fn default() -> Self {
        Self {
            mode: ClientRuntimeMode::JoinedGame,
            render_profile: ClientRenderProfile::Default,
            server_addr: None,
            project_id: None,
            game_session_id: None,
            scene_id: None,
            hosted_by_editor: false,
            host_instance_id: None,
            host_parent_pid: None,
        }
    }
}

fn env_non_empty_string(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn env_flag(name: &str) -> bool {
    match std::env::var(name) {
        Ok(value) => {
            value.eq_ignore_ascii_case("1")
                || value.eq_ignore_ascii_case("true")
                || value.eq_ignore_ascii_case("yes")
                || value.eq_ignore_ascii_case("on")
        }
        Err(_) => false,
    }
}

fn env_host_parent_pid() -> Option<u32> {
    let raw = std::env::var("FUN_HOST_PARENT_PID").ok()?;
    match raw.parse::<u32>() {
        Ok(pid) => Some(pid),
        Err(error) => {
            warn!(
                target: "fun::client::host",
                value = raw,
                %error,
                "ignored invalid FUN_HOST_PARENT_PID"
            );
            None
        }
    }
}

#[derive(Debug, Clone, Copy, Resource)]
pub(crate) struct ClientRenderConfig {
    solari_enabled: bool,
    dlss_rr_enabled: bool,
    meshlets_enabled: bool,
    dlss_rr_disabled_by_denoise_mode: bool,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    render_profile_verbose: bool,
    pub(crate) geometry_policy: RenderGeometryPolicy,
    pub(crate) meshlet_min_triangles: usize,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    fps_overlay_enabled: bool,
}

impl ClientRenderConfig {
    fn from_env() -> Self {
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
    fn from_env() -> Self {
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
pub(crate) enum RenderGeometryClass {
    SimpleRaster,
    MeshletStaticDense,
    MeshletDynamicDense,
    RayProxyOnly,
    Viewmodel,
}

impl RenderGeometryClass {
    pub(crate) fn uses_meshlet(self) -> bool {
        matches!(
            self,
            RenderGeometryClass::MeshletStaticDense | RenderGeometryClass::MeshletDynamicDense
        )
    }

    pub(crate) fn uses_raster_mesh(self) -> bool {
        matches!(
            self,
            RenderGeometryClass::SimpleRaster | RenderGeometryClass::Viewmodel
        )
    }
}

#[derive(Debug, Clone, Copy, Resource)]
struct ClientWindowConfig {
    maximized: bool,
    width: Option<u32>,
    height: Option<u32>,
}

impl ClientWindowConfig {
    fn from_env() -> Self {
        Self {
            maximized: std::env::var_os("FUN_WINDOW_MAXIMIZED").is_some(),
            width: env_u32_opt("FUN_WINDOW_WIDTH"),
            height: env_u32_opt("FUN_WINDOW_HEIGHT"),
        }
    }

    fn resolution(&self) -> WindowResolution {
        match (self.width, self.height) {
            (Some(width), Some(height)) => WindowResolution::new(width, height),
            _ => WindowResolution::default(),
        }
    }
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
const GPU_SAMPLE_STATUS_CODE_PATH: &str = "render/gpu_sample_status_code";
#[cfg(all(feature = "render_diagnostics", debug_assertions))]
const RENDER_FRAME_INDEX_PATH: &str = "render/render_frame_index";
#[cfg(all(feature = "render_diagnostics", debug_assertions))]
const GPU_QUERY_FRAME_INDEX_PATH: &str = "render/gpu_query_frame_index";
#[cfg(all(feature = "render_diagnostics", debug_assertions))]
const SAMPLE_LATENCY_FRAMES_PATH: &str = "render/sample_latency_frames";

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
#[derive(Debug, Default, Resource)]
struct ClientPerfCounters {
    network_receive_cpu_ns: u64,
    world_stream_apply_cpu_ns: u64,
    catalog_lookup_cpu_ns: u64,
    meshlet_path_instance_count: u64,
    raster_path_instance_count: u64,
    ray_proxy_only_count: u64,
    last_gpu_sample_render_frame_index: Option<u64>,
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
impl ClientPerfCounters {
    fn add_network_receive_ns(&mut self, ns: u64) {
        self.network_receive_cpu_ns = self.network_receive_cpu_ns.saturating_add(ns);
    }

    fn add_world_stream_apply_ns(&mut self, ns: u64) {
        self.world_stream_apply_cpu_ns = self.world_stream_apply_cpu_ns.saturating_add(ns);
    }

    fn add_catalog_lookup_ns(&mut self, ns: u64) {
        self.catalog_lookup_cpu_ns = self.catalog_lookup_cpu_ns.saturating_add(ns);
    }

    fn set_render_path_counts(
        &mut self,
        meshlet_path_instance_count: u64,
        raster_path_instance_count: u64,
        ray_proxy_only_count: u64,
    ) {
        self.meshlet_path_instance_count = meshlet_path_instance_count;
        self.raster_path_instance_count = raster_path_instance_count;
        self.ray_proxy_only_count = ray_proxy_only_count;
    }

    fn mark_gpu_sample_frame(&mut self, render_frame_index: Option<u64>) -> bool {
        let Some(render_frame_index) = render_frame_index else {
            return false;
        };
        let is_new = self.last_gpu_sample_render_frame_index != Some(render_frame_index);
        self.last_gpu_sample_render_frame_index = Some(render_frame_index);
        is_new
    }
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
#[derive(Debug, Resource)]
struct ClientEditorControlPlane {
    config: game_shared::EditorControlConfig,
    granted_capabilities: Vec<game_shared::EditorCapability>,
    diagnostic_streams: Vec<game_shared::EditorDiagnosticStream>,
}

#[derive(Debug, Clone, Resource, Default)]
struct ClientEditorInspectorState {
    state: game_shared::EditorInspectorRuntimeState,
}

#[derive(Debug, Clone, Resource)]
pub(crate) struct ClientHostControlState {
    pub(crate) input_owner: EditorInputOwner,
    pub(crate) embedded: bool,
    pub(crate) visual_paused: bool,
    pub(crate) shutdown_requested: bool,
    host_parent_pid: Option<u32>,
    parent_exit_requested: bool,
}

impl ClientHostControlState {
    fn from_options(options: &ClientAppOptions) -> Self {
        Self {
            input_owner: EditorInputOwner::Game,
            embedded: options.hosted_by_editor,
            visual_paused: false,
            shutdown_requested: false,
            host_parent_pid: options.host_parent_pid,
            parent_exit_requested: false,
        }
    }
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
impl Default for ClientEditorControlPlane {
    fn default() -> Self {
        let execute_client_code_enabled =
            std::env::var_os("FUN_EDITOR_ENABLE_CLIENT_EXEC").is_some();

        Self {
            config: game_shared::EditorControlConfig::local_development(
                game_shared::EditorTargetKind::Client,
            ),
            granted_capabilities: game_shared::local_development_capabilities(
                game_shared::EditorTargetKind::Client,
                execute_client_code_enabled,
            ),
            diagnostic_streams: game_shared::default_client_editor_diagnostic_subscriptions(),
        }
    }
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
fn log_client_editor_control_plane(control: Res<ClientEditorControlPlane>) {
    let execute_client_code_enabled = control
        .granted_capabilities
        .contains(&game_shared::EditorCapability::ExecuteClientCode);

    game_shared::fun_diag_info!(
        target: "fun::editor::control",
        protocol_version = game_shared::EDITOR_PROTOCOL_VERSION,
        target_kind = ?control.config.target_kind,
        bind_mode = ?control.config.bind_mode,
        bind_addr = control.config.bind_addr.as_str(),
        enabled = control.config.enabled,
        remote_control_permitted = control.config.permits_remote_editor_control(),
        execute_client_code_enabled = execute_client_code_enabled,
        command_apply_stage = game_shared::EDITOR_COMMAND_APPLY_STAGE,
        granted_capabilities = control.granted_capabilities.len(),
        diagnostic_streams = control.diagnostic_streams.len(),
        "client editor control plane ready"
    );
}

#[derive(bevy::ecs::system::SystemParam)]
struct ClientRuntimeProfiler<'w> {
    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    perf_counters: ResMut<'w, ClientPerfCounters>,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    schedule_profiler: ResMut<'w, ClientScheduleProfiler>,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    frame_profiler: ResMut<'w, DetailedFrameProfiler>,
    log_config: Res<'w, ClientLogConfig>,
}

#[cfg_attr(not(all(feature = "diagnostics", debug_assertions)), allow(dead_code))]
#[derive(Debug, Clone, Copy, Resource)]
pub(crate) struct ClientLogConfig {
    stream_verbose: bool,
    net_verbose: bool,
    render_verbose: bool,
    benchmark_minimal: bool,
}

#[cfg_attr(not(all(feature = "diagnostics", debug_assertions)), allow(dead_code))]
impl ClientLogConfig {
    fn from_env() -> Self {
        Self {
            stream_verbose: std::env::var_os("FUN_LOG_STREAM_VERBOSE").is_some(),
            net_verbose: std::env::var_os("FUN_LOG_NET_VERBOSE").is_some(),
            render_verbose: std::env::var_os("FUN_LOG_RENDER_VERBOSE").is_some(),
            benchmark_minimal: std::env::var_os("FUN_BENCHMARK_LOG_MINIMAL").is_some(),
        }
    }

    fn stream_verbose(self) -> bool {
        self.stream_verbose && !self.benchmark_minimal
    }

    fn net_verbose(self) -> bool {
        self.net_verbose && !self.benchmark_minimal
    }

    fn render_verbose(self) -> bool {
        self.render_verbose && !self.benchmark_minimal
    }

    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    fn diagnostics_verbose(self) -> bool {
        !self.benchmark_minimal
    }
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
const CLIENT_SCHEDULE_SYSTEM_COUNT: usize = 10;
#[cfg(all(feature = "render_diagnostics", debug_assertions))]
const CLIENT_SCHEDULE_SYSTEMS: [ClientScheduleSystem; CLIENT_SCHEDULE_SYSTEM_COUNT] = [
    ClientScheduleSystem::NetworkingReceive,
    ClientScheduleSystem::WorldStreamApply,
    ClientScheduleSystem::MovementInput,
    ClientScheduleSystem::Look,
    ClientScheduleSystem::PhysicsMovement,
    ClientScheduleSystem::DiagnosticsLogging,
    ClientScheduleSystem::RenderConfigWindow,
    ClientScheduleSystem::SolariRuntimeParamsUpdate,
    ClientScheduleSystem::MeshletExtraction,
    ClientScheduleSystem::RenderInterpolation,
];

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClientScheduleSystem {
    NetworkingReceive,
    WorldStreamApply,
    MovementInput,
    Look,
    PhysicsMovement,
    DiagnosticsLogging,
    RenderConfigWindow,
    SolariRuntimeParamsUpdate,
    MeshletExtraction,
    RenderInterpolation,
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
impl ClientScheduleSystem {
    const fn index(self) -> usize {
        match self {
            ClientScheduleSystem::NetworkingReceive => 0,
            ClientScheduleSystem::WorldStreamApply => 1,
            ClientScheduleSystem::MovementInput => 2,
            ClientScheduleSystem::Look => 3,
            ClientScheduleSystem::PhysicsMovement => 4,
            ClientScheduleSystem::DiagnosticsLogging => 5,
            ClientScheduleSystem::RenderConfigWindow => 6,
            ClientScheduleSystem::SolariRuntimeParamsUpdate => 7,
            ClientScheduleSystem::MeshletExtraction => 8,
            ClientScheduleSystem::RenderInterpolation => 9,
        }
    }

    const fn metric_name(self) -> &'static str {
        match self {
            ClientScheduleSystem::NetworkingReceive => "networking_receive",
            ClientScheduleSystem::WorldStreamApply => "world_stream_apply",
            ClientScheduleSystem::MovementInput => "movement_input",
            ClientScheduleSystem::Look => "look",
            ClientScheduleSystem::PhysicsMovement => "physics_movement",
            ClientScheduleSystem::DiagnosticsLogging => "diagnostics_logging",
            ClientScheduleSystem::RenderConfigWindow => "render_config_window",
            ClientScheduleSystem::SolariRuntimeParamsUpdate => "solari_runtime_params_update",
            ClientScheduleSystem::MeshletExtraction => "meshlet_extraction",
            ClientScheduleSystem::RenderInterpolation => "render_interpolation",
        }
    }
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
#[derive(Debug, Clone, Copy, Default)]
struct ClientScheduleSample {
    total_ns: u64,
    max_ns: u64,
    count: u32,
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
#[derive(Debug, Clone, Copy)]
struct ClientScheduleReport {
    system: ClientScheduleSystem,
    total_ns: u64,
    max_ns: u64,
    count: u32,
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
#[derive(Debug, Resource)]
pub(crate) struct ClientScheduleProfiler {
    samples: [ClientScheduleSample; CLIENT_SCHEDULE_SYSTEM_COUNT],
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
impl Default for ClientScheduleProfiler {
    fn default() -> Self {
        Self {
            samples: [ClientScheduleSample::default(); CLIENT_SCHEDULE_SYSTEM_COUNT],
        }
    }
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
impl ClientScheduleProfiler {
    pub(crate) fn record_ns(&mut self, system: ClientScheduleSystem, ns: u64) {
        let sample = &mut self.samples[system.index()];
        sample.total_ns = sample.total_ns.saturating_add(ns);
        sample.max_ns = sample.max_ns.max(ns);
        sample.count = sample.count.saturating_add(1);
    }

    pub(crate) fn record_elapsed(&mut self, system: ClientScheduleSystem, started: Instant) {
        self.record_ns(
            system,
            started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64,
        );
    }

    fn drain_reports(&mut self) -> Vec<ClientScheduleReport> {
        let mut reports = Vec::with_capacity(CLIENT_SCHEDULE_SYSTEM_COUNT);
        for system in CLIENT_SCHEDULE_SYSTEMS {
            let sample = std::mem::take(&mut self.samples[system.index()]);
            if sample.count > 0 || sample.total_ns > 0 {
                reports.push(ClientScheduleReport {
                    system,
                    total_ns: sample.total_ns,
                    max_ns: sample.max_ns,
                    count: sample.count,
                });
            }
        }
        reports
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct RenderPathSignature {
    pub id: &'static str,
    pub render_profile: ClientRenderProfile,
    pub solari_enabled: bool,
    pub dlss_rr_enabled: bool,
    pub meshlets_enabled: bool,
    pub geometry_policy: RenderGeometryPolicy,
    pub meshlet_min_triangles: usize,
    pub opaque_renderer: ClientOpaqueRenderer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientOpaqueRenderer {
    Deferred,
    Forward,
}

impl ClientOpaqueRenderer {
    fn method(self) -> DefaultOpaqueRendererMethod {
        match self {
            Self::Deferred => DefaultOpaqueRendererMethod::deferred(),
            Self::Forward => DefaultOpaqueRendererMethod::forward(),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Deferred => "deferred",
            Self::Forward => "forward",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FunClientRenderPlugin {
    options: ClientAppOptions,
}

impl FunClientRenderPlugin {
    pub fn new(options: ClientAppOptions) -> Self {
        Self { options }
    }
}

impl Plugin for FunClientRenderPlugin {
    fn build(&self, app: &mut App) {
        let render_backend = selected_render_backend();
        let present_mode = selected_present_mode();
        let window_config = ClientWindowConfig::from_env();

        #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
        app.insert_resource(DlssProjectId(
            Uuid::parse_str(DLSS_PROJECT_ID).expect("DLSS project ID should be a valid UUID"),
        ));

        info!(
            target: "fun::render",
            runtime_mode = self.options.mode.as_env_value(),
            render_profile = self.options.render_profile.as_env_value(),
            backend = ?render_backend,
            present_mode = ?present_mode,
            vsync = false,
            max_frame_latency = 3,
            maximized = window_config.maximized,
            "client window/render backend selected"
        );

        let title = match self.options.mode {
            ClientRuntimeMode::EditorPreview => format!("{GAME_TITLE} Preview"),
            _ => format!("{GAME_TITLE} Client"),
        };
        let default_plugins = DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title,
                present_mode,
                resolution: window_config.resolution(),
                desired_maximum_frame_latency: NonZeroU32::new(3),
                ..default()
            }),
            ..default()
        });
        let default_plugins = default_plugins.set(RenderPlugin {
            render_creation: client_render_creation(render_backend),
            ..default()
        });

        app.add_plugins(default_plugins)
            .insert_resource(window_config)
            .insert_resource(RenderErrorHandler(recover_render_device))
            .insert_resource(WinitSettings::continuous());

        install_fun_client_render_path(app, &self.options);
    }
}

pub fn install_fun_client_render_path(app: &mut App, options: &ClientAppOptions) {
    let (render_config, solari_settings, solari_runtime_params) = render_path_config_from_env();
    log_client_render_path(&render_config, &solari_settings, &solari_runtime_params);
    let opaque_renderer = selected_opaque_renderer(&render_config);
    let signature = render_path_signature_for_options(options, &render_config, opaque_renderer);
    emit_render_path_signature(options, &signature);

    let solari_enabled = render_config.solari_enabled;
    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    let fps_overlay_enabled = render_config.fps_overlay_enabled;

    app.insert_resource(opaque_renderer.method())
        .insert_resource(signature)
        .insert_resource(render_config)
        .insert_resource(solari_settings)
        .insert_resource(solari_runtime_params)
        .add_message::<SolariResetEvent>()
        .add_plugins(MeshletPlugin {
            cluster_buffer_slots: 1 << 14,
        })
        .add_systems(
            Startup,
            (setup_lighting, prewarm_world_render_catalog).chain(),
        );

    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    if fps_overlay_enabled {
        app.add_plugins(FpsOverlayPlugin {
            config: FpsOverlayConfig {
                refresh_interval: Duration::from_secs(1),
                text_config: bevy::text::TextFont {
                    font_size: bevy::text::FontSize::Px(12.0),
                    ..default()
                },
                ..default()
            },
        });
    }

    if solari_enabled {
        app.add_plugins(SolariPlugins);
    }

    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    {
        if std::env::var_os("FUN_RENDER_DIAGNOSTICS").is_some() {
            game_shared::fun_diag_info!("[client render] GPU render diagnostics enabled");
            app.add_plugins((
                RenderDiagnosticsPlugin,
                bevy::diagnostic::SystemInformationDiagnosticsPlugin,
            ));
        }
    }
    #[cfg(not(all(feature = "render_diagnostics", debug_assertions)))]
    {
        if std::env::var_os("FUN_RENDER_DIAGNOSTICS").is_some()
            || std::env::var_os("FUN_FRAME_TIME_DIAGNOSTICS").is_some()
            || std::env::var_os("FUN_RENDER_PROFILE_VERBOSE").is_some()
        {
            warn!(
                target: "fun::render",
                "render diagnostics requested but game_client/render_diagnostics is not enabled"
            );
        }
    }
}

fn render_path_config_from_env() -> (ClientRenderConfig, SolariSettings, SolariRuntimeParams) {
    let mut render_config = ClientRenderConfig::from_env();
    let solari_settings = solari_settings_from_env();
    let solari_runtime_params = solari_runtime_params_from_env(&solari_settings);
    if solari_settings.denoise_mode != SolariDenoiseMode::DlssRayReconstruction {
        render_config.dlss_rr_disabled_by_denoise_mode = render_config.dlss_rr_enabled;
        render_config.dlss_rr_enabled = false;
    }
    (render_config, solari_settings, solari_runtime_params)
}

fn log_client_render_path(
    render_config: &ClientRenderConfig,
    solari_settings: &SolariSettings,
    solari_runtime_params: &SolariRuntimeParams,
) {
    if render_config.solari_enabled {
        info!("[client render] Solari lighting will start after the streamed world is ready");
    } else {
        info!("[client render] Solari lighting disabled by FUN_DISABLE_SOLARI");
    }
    if render_config.meshlets_enabled {
        info!("[client render] streamed world will use meshlet meshes");
    } else {
        info!("[client render] streamed world meshlets disabled by FUN_DISABLE_MESHLETS");
    }
    info!(
        "[client render] Solari denoise mode: {:?}",
        solari_settings.denoise_mode
    );
    info!(
        "[client render] Solari internal GI scale: {:?}",
        solari_settings.internal_scale
    );
    info!(
        "[client render] Solari world-cache: {} entries, {} updates/frame soft cap, {} frame slices, camera tiers {}m/{}m/{}m",
        solari_settings.world_cache_size,
        solari_settings.world_cache_cell_updates_soft_cap,
        solari_settings.world_cache_frame_slice_count,
        solari_settings.world_cache_near_camera_distance_meters,
        solari_settings.world_cache_mid_camera_distance_meters,
        solari_settings.world_cache_far_camera_distance_meters
    );
    info!(
        "[client render] Solari architecture: {:?}, visual target: {:?}, target_fps={}, frame_budget_ns={}, gpu_budget_ns={}",
        solari_runtime_params.architecture,
        solari_runtime_params.visual_target,
        solari_runtime_params.target_fps,
        solari_runtime_params.frame_budget_ns,
        solari_runtime_params.gpu_budget_ns
    );
    info!(
        target: "fun::render",
        solari_enabled = render_config.solari_enabled,
        meshlets_enabled = render_config.meshlets_enabled,
        dlss_rr_enabled = render_config.dlss_rr_enabled,
        denoise_mode = ?solari_settings.denoise_mode,
        internal_scale = ?solari_settings.internal_scale,
        world_cache_size = solari_settings.world_cache_size,
        world_cache_updates_soft_cap = solari_settings.world_cache_cell_updates_soft_cap,
        world_cache_frame_slice_count = solari_settings.world_cache_frame_slice_count,
        world_cache_near_meters = solari_settings.world_cache_near_camera_distance_meters,
        world_cache_mid_meters = solari_settings.world_cache_mid_camera_distance_meters,
        world_cache_far_meters = solari_settings.world_cache_far_camera_distance_meters,
        solari_architecture = ?solari_runtime_params.architecture,
        solari_visual_target = ?solari_runtime_params.visual_target,
        solari_target_fps = solari_runtime_params.target_fps,
        solari_frame_budget_ns = solari_runtime_params.frame_budget_ns,
        solari_gpu_budget_ns = solari_runtime_params.gpu_budget_ns,
        solari_quality_level = solari_runtime_params.quality_level,
        solari_cache_update_budget = solari_runtime_params.cache_update_budget,
        solari_specular_refresh_budget = solari_runtime_params.specular_refresh_budget,
        solari_debug_overlay = ?solari_runtime_params.debug_overlay,
        geometry_policy = ?render_config.geometry_policy,
        meshlet_min_triangles = render_config.meshlet_min_triangles,
        "client render configuration"
    );
    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    game_shared::fun_diag_info!(
        target: "fun::render",
        render_profile_verbose = render_config.render_profile_verbose,
        fps_overlay_enabled = render_config.fps_overlay_enabled,
        "client render diagnostic configuration"
    );
}

fn selected_opaque_renderer(render_config: &ClientRenderConfig) -> ClientOpaqueRenderer {
    if render_config.solari_enabled {
        info!("[client render] default opaque renderer: deferred");
        ClientOpaqueRenderer::Deferred
    } else {
        info!("[client render] default opaque renderer: forward");
        ClientOpaqueRenderer::Forward
    }
}

fn render_path_signature_for_options(
    options: &ClientAppOptions,
    render_config: &ClientRenderConfig,
    opaque_renderer: ClientOpaqueRenderer,
) -> RenderPathSignature {
    RenderPathSignature {
        id: FUN_CLIENT_RENDER_PATH_SIGNATURE_ID,
        render_profile: options.render_profile,
        solari_enabled: render_config.solari_enabled,
        dlss_rr_enabled: render_config.dlss_rr_enabled,
        meshlets_enabled: render_config.meshlets_enabled,
        geometry_policy: render_config.geometry_policy,
        meshlet_min_triangles: render_config.meshlet_min_triangles,
        opaque_renderer,
    }
}

fn emit_render_path_signature(options: &ClientAppOptions, signature: &RenderPathSignature) {
    info!(
        target: "fun::render",
        signature_id = signature.id,
        runtime_mode = options.mode.as_env_value(),
        render_profile = signature.render_profile.as_env_value(),
        hosted_by_editor = options.hosted_by_editor,
        solari_enabled = signature.solari_enabled,
        meshlets_enabled = signature.meshlets_enabled,
        dlss_rr_enabled = signature.dlss_rr_enabled,
        geometry_policy = ?signature.geometry_policy,
        meshlet_min_triangles = signature.meshlet_min_triangles,
        opaque_renderer = signature.opaque_renderer.as_str(),
        "RenderPathSignature"
    );
}

#[derive(Debug, Clone)]
pub struct GameClientPlugin {
    options: ClientAppOptions,
}

impl GameClientPlugin {
    pub fn new(options: ClientAppOptions) -> Self {
        Self { options }
    }
}

impl Default for GameClientPlugin {
    fn default() -> Self {
        Self::new(ClientAppOptions::default())
    }
}

impl Plugin for GameClientPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.options.clone())
            .insert_resource(ClientLogConfig::from_env())
            .insert_resource(ClientHostControlState::from_options(&self.options))
            .insert_resource(Time::<Fixed>::from_hz(DEFAULT_TICK_RATE_HZ))
            .init_resource::<LoadedWorldState>()
            .init_resource::<ClientWorldStatus>()
            .init_resource::<ClientEditorInspectorState>();

        if self.options.mode.runs_gameplay_runtime() {
            app.add_plugins((
                PhysicsPlugins::default(),
                QuinnetClientPlugin::default(),
                ThunderPlugin::default(),
                FirstPersonControllerPlugin::gameplay(),
            ));
            if self.options.mode.supports_editor_activation() {
                app.add_plugins(EditorHotkeyPlugin);
            }
        } else {
            app.insert_resource(StaticPreviewWorldStream::new(
                self.options.scene_id.as_deref(),
            ))
            .add_plugins(FirstPersonControllerPlugin::preview_camera());
        }

        #[cfg(all(feature = "diagnostics", debug_assertions))]
        app.init_resource::<ClientEditorControlPlane>()
            .add_systems(Startup, log_client_editor_control_plane);

        #[cfg(all(feature = "render_diagnostics", debug_assertions))]
        app.init_resource::<ClientPerfCounters>()
            .init_resource::<ClientScheduleProfiler>()
            .init_resource::<ClientDiagnostics>();

        app.add_systems(Startup, start_client_editor_inspector);
        app.add_systems(
            Update,
            (watch_host_parent_liveness, apply_editor_runtime_controls),
        );
        if self.options.mode.runs_gameplay_runtime() {
            app.add_systems(Startup, connect_to_game_server)
                .add_systems(
                    Update,
                    (
                        apply_startup_window_config,
                        send_client_hello,
                        receive_server_control,
                        receive_server_snapshots,
                        receive_world_stream,
                        update_client_editor_inspector_snapshot,
                    ),
                );
        } else {
            app.add_systems(
                Update,
                (
                    apply_startup_window_config,
                    apply_static_preview_world_stream,
                    update_client_editor_inspector_snapshot,
                )
                    .chain(),
            );
        }

        #[cfg(all(feature = "render_diagnostics", debug_assertions))]
        {
            install_detailed_frame_profiler(app);
            app.add_systems(Update, log_client_diagnostics);
        }
    }
}

#[derive(Debug, Resource)]
struct StaticPreviewWorldStream {
    scene_id: Option<String>,
    chunks: Vec<WorldStreamChunk>,
    applied: bool,
}

impl StaticPreviewWorldStream {
    fn new(scene_id: Option<&str>) -> Self {
        Self {
            scene_id: scene_id.map(str::to_owned),
            chunks: game_scene::default_scene_world_stream_chunks(WorldRevision(1)),
            applied: false,
        }
    }

    fn load_scene(&mut self, scene_id: String) {
        self.scene_id = Some(scene_id);
        self.chunks = game_scene::default_scene_world_stream_chunks(WorldRevision(1));
        self.applied = false;
    }

    #[cfg(test)]
    fn from_chunks(chunks: Vec<WorldStreamChunk>) -> Self {
        Self {
            scene_id: None,
            chunks,
            applied: false,
        }
    }
}

fn watch_host_parent_liveness(
    mut host_control: ResMut<ClientHostControlState>,
    mut exit: MessageWriter<AppExit>,
) {
    if host_control.parent_exit_requested {
        return;
    }
    let Some(parent_pid) = host_control.host_parent_pid else {
        return;
    };
    if process_is_alive(parent_pid) {
        return;
    }

    host_control.parent_exit_requested = true;
    host_control.shutdown_requested = true;
    warn!(
        target: "fun::client::host",
        parent_pid,
        "editor host parent process exited; shutting down managed client"
    );
    exit.write(AppExit::Success);
}

fn apply_editor_runtime_controls(
    inspector: Res<ClientEditorInspectorState>,
    mut host_control: ResMut<ClientHostControlState>,
    mut preview_stream: Option<ResMut<StaticPreviewWorldStream>>,
    mut exit: MessageWriter<AppExit>,
) {
    for command in inspector.state.drain_runtime_controls(32) {
        let command_id = command.command_id();
        match command {
            game_shared::EditorRuntimeControlCommand::InputSetOwner { owner } => {
                host_control.input_owner = owner;
            }
            game_shared::EditorRuntimeControlCommand::WindowSetEmbedded { embedded } => {
                host_control.embedded = embedded;
            }
            game_shared::EditorRuntimeControlCommand::SimulationPauseVisualOnly => {
                host_control.visual_paused = true;
            }
            game_shared::EditorRuntimeControlCommand::SimulationResume => {
                host_control.visual_paused = false;
            }
            game_shared::EditorRuntimeControlCommand::SceneLoadPreview { scene_id } => {
                if let Some(stream) = preview_stream.as_mut() {
                    stream.load_scene(scene_id);
                }
            }
            game_shared::EditorRuntimeControlCommand::ShutdownRequest => {
                host_control.shutdown_requested = true;
                exit.write(AppExit::Success);
            }
        }
        info!(
            target: "fun::client::host",
            command_id,
            input_owner = ?host_control.input_owner,
            embedded = host_control.embedded,
            visual_paused = host_control.visual_paused,
            "applied authenticated editor runtime control command"
        );
    }
}

#[cfg(windows)]
fn process_is_alive(pid: u32) -> bool {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, WAIT_TIMEOUT},
        System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
            WaitForSingleObject,
        },
    };

    // SAFETY: OpenProcess only requests query/synchronize rights for the PID supplied
    // by the trusted launcher environment; the returned handle is checked before use.
    let handle = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            0,
            pid,
        )
    };
    if handle.is_null() {
        return false;
    }
    // SAFETY: The handle came from a successful OpenProcess call and is closed below.
    let wait_result = unsafe { WaitForSingleObject(handle, 0) };
    // SAFETY: The handle came from OpenProcess and is no longer used after this call.
    unsafe {
        CloseHandle(handle);
    }
    wait_result == WAIT_TIMEOUT
}

#[cfg(not(windows))]
fn process_is_alive(_pid: u32) -> bool {
    true
}

#[allow(
    clippy::too_many_arguments,
    reason = "preview stream application uses the same explicit asset and render stores as network world-stream ingestion"
)]
fn apply_static_preview_world_stream(
    mut commands: Commands,
    render_config: Res<ClientRenderConfig>,
    mut preview_stream: ResMut<StaticPreviewWorldStream>,
    mut loaded_world: ResMut<LoadedWorldState>,
    mut world_status: ResMut<ClientWorldStatus>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut meshlet_meshes: ResMut<Assets<MeshletMesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    catalog: Res<WorldRenderCatalog>,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    mut runtime: ClientRuntimeProfiler,
    #[cfg(not(all(feature = "render_diagnostics", debug_assertions)))]
    runtime: ClientRuntimeProfiler,
    solari_cameras: Query<Entity, (With<Camera3d>, Without<SolariLighting>)>,
    mut solari_lighting: Query<&mut SolariLighting>,
    mut solari_reset_events: MessageWriter<SolariResetEvent>,
    #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))] dlss_rr_supported: Option<
        Res<DlssRayReconstructionSupported>,
    >,
    #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))] dlss_rr_cameras: Query<
        Entity,
        (With<Camera3d>, Without<Dlss<DlssRayReconstructionFeature>>),
    >,
    #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))] mut dlss_rr: Query<
        &mut Dlss<DlssRayReconstructionFeature>,
    >,
) {
    if preview_stream.applied {
        return;
    }

    let scene_id = preview_stream
        .scene_id
        .clone()
        .unwrap_or_else(|| game_shared::DEMO_LEVEL_ID.to_owned());
    info!(
        target: "fun::preview",
        scene_id = scene_id.as_str(),
        chunks = preview_stream.chunks.len(),
        "applying static editor preview stream"
    );
    let mut world_revision_changed = false;
    for chunk in &preview_stream.chunks {
        world_revision_changed |= apply_world_stream_chunk(
            &mut commands,
            &mut loaded_world,
            &mut world_status,
            &mut meshes,
            &mut meshlet_meshes,
            &mut materials,
            &catalog,
            #[cfg(all(feature = "render_diagnostics", debug_assertions))]
            runtime.perf_counters.as_mut(),
            #[cfg(all(feature = "render_diagnostics", debug_assertions))]
            runtime.frame_profiler.as_mut(),
            &render_config,
            &runtime.log_config,
            chunk,
        );
    }
    preview_stream.applied = true;

    if world_revision_changed {
        request_solari_lighting_history_reset(
            "static editor preview stream applied",
            &mut solari_reset_events,
            &mut solari_lighting,
        );
        #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
        reset_dlss_ray_reconstruction_history(&mut dlss_rr);
    }

    if loaded_world.is_complete() {
        world_status.ready = true;
        enable_solari_lighting_for_ready_world(
            &mut commands,
            &render_config,
            &solari_cameras,
            &mut solari_lighting,
            &mut solari_reset_events,
        );
        #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
        enable_dlss_ray_reconstruction_for_ready_world(
            &mut commands,
            &render_config,
            dlss_rr_supported.as_deref(),
            &dlss_rr_cameras,
            &mut dlss_rr,
        );
        info!(
            target: "fun::preview",
            scene_id = scene_id.as_str(),
            spawned_entities = loaded_world.spawned_entities.len(),
            revision = loaded_world.revision.map(|revision| revision.0).unwrap_or_default(),
            "static editor preview stream ready"
        );
    }
}

fn start_client_editor_inspector(inspector: Res<ClientEditorInspectorState>) {
    let execute_client_code_enabled = std::env::var_os("FUN_EDITOR_ENABLE_CLIENT_EXEC").is_some();
    let component_schemas = client_editor_component_schemas();
    inspector.state.replace_entities(
        game_shared::EditorWorldRevision(0),
        game_shared::EditorSchemaRevision(1),
        component_schemas.clone(),
        Vec::new(),
    );
    let mut config = game_shared::EditorInspectorServiceConfig::local_development(
        game_shared::EditorTargetKind::Client,
        "fun",
        game_shared::local_development_capabilities(
            game_shared::EditorTargetKind::Client,
            execute_client_code_enabled,
        ),
    );
    config.tick_rate_hz = DEFAULT_TICK_RATE_HZ.round() as u32;
    config.component_schemas = component_schemas;
    config.runtime_state = inspector.state.clone();

    match game_shared::spawn_editor_inspector_service(config) {
        Ok(summary) => {
            if summary.callback_started || summary.bind_addr.is_some() {
                info!(
                    target: "fun::editor::control",
                    callback_started = summary.callback_started,
                    bind_addr = ?summary.bind_addr,
                    "client editor inspector service started"
                );
            }
        }
        Err(error) => {
            warn!(
                target: "fun::editor::control",
                %error,
                "client editor inspector service did not start"
            );
        }
    }
}

fn client_editor_component_schemas() -> Vec<game_shared::EditorComponentSchema> {
    vec![
        client_component_schema(
            1,
            game_shared::EDITOR_COMPONENT_KIND_TRANSFORM,
            "Transform",
            "Transform",
            "transform3d",
            "primary",
            game_shared::EditorMutability::RuntimeMutable,
            game_shared::EditorReplicationPolicy::ClientLocalOnly,
        ),
        client_component_schema(
            2,
            game_shared::EDITOR_COMPONENT_KIND_NAME,
            "Name",
            "Identity",
            "text",
            "secondary",
            game_shared::EditorMutability::RuntimeMutable,
            game_shared::EditorReplicationPolicy::ClientLocalOnly,
        ),
        client_component_schema(
            3,
            game_shared::EDITOR_COMPONENT_KIND_NETWORK_IDENTITY,
            "Network Identity",
            "Network",
            "read_only_struct",
            "diagnostic",
            game_shared::EditorMutability::ReadOnly,
            game_shared::EditorReplicationPolicy::ClientLocalOnly,
        ),
        client_component_schema(
            4,
            game_shared::EDITOR_COMPONENT_KIND_NETWORK_AUTHORITY,
            "Authority",
            "Network",
            "read_only_struct",
            "diagnostic",
            game_shared::EditorMutability::ReadOnly,
            game_shared::EditorReplicationPolicy::ClientLocalOnly,
        ),
        client_component_schema(
            5,
            game_shared::EDITOR_COMPONENT_KIND_WORLD_CATALOG_REF,
            "Render / Prediction",
            "Render",
            "read_only_struct",
            "advanced",
            game_shared::EditorMutability::ReadOnly,
            game_shared::EditorReplicationPolicy::ClientLocalOnly,
        ),
    ]
}

#[allow(
    clippy::too_many_arguments,
    reason = "schema registration rows stay compact here until the editor schema macro owns this table"
)]
fn client_component_schema(
    stable_type_id: u64,
    component_kind: ComponentKind,
    display_label: &str,
    ui_group: &str,
    ui_widget: &str,
    importance: &str,
    mutability: game_shared::EditorMutability,
    replication_policy: game_shared::EditorReplicationPolicy,
) -> game_shared::EditorComponentSchema {
    game_shared::EditorComponentSchema {
        stable_type_id: game_shared::EditorStableTypeId(stable_type_id),
        component_kind,
        display_label: display_label.to_owned(),
        mutability,
        serialization_policy: game_shared::EditorSerializationPolicy::Compactly,
        replication_policy,
        ui_group: ui_group.to_owned(),
        ui_widget: ui_widget.to_owned(),
        importance: importance.to_owned(),
        diagnostic_label: format!("fun::editor::client_schema:{display_label}"),
    }
}

#[allow(
    clippy::type_complexity,
    reason = "Bevy query tuple documents the exact client mirror components exposed to the editor snapshot"
)]
fn update_client_editor_inspector_snapshot(
    inspector: Res<ClientEditorInspectorState>,
    loaded_world: Res<LoadedWorldState>,
    query: Query<(
        &NetworkIdentity,
        &NetworkAuthority,
        Option<&Name>,
        Option<&Transform>,
        Option<&RenderGeometryClass>,
    )>,
    mut last_diagnostic_revision: Local<u64>,
) {
    let schema_revision = game_shared::EditorSchemaRevision(1);
    let world_revision = game_shared::EditorWorldRevision(
        loaded_world
            .revision
            .map(|revision| revision.0)
            .unwrap_or_default(),
    );
    let schemas = client_editor_component_schemas();
    let mut rows = Vec::with_capacity(query.iter().count());

    for (identity, authority, name, transform, render_geometry) in &query {
        if !identity.entity.is_valid() {
            continue;
        }

        let mut components = Vec::with_capacity(5);
        if let Some(transform) = transform {
            components.push(client_editor_component_value(
                game_shared::EDITOR_COMPONENT_KIND_TRANSFORM,
                schema_revision,
                client_transform_preview(transform),
            ));
        }
        if let Some(name) = name {
            components.push(client_editor_component_value(
                game_shared::EDITOR_COMPONENT_KIND_NAME,
                schema_revision,
                name.as_str().to_owned(),
            ));
        }
        components.push(client_editor_component_value(
            game_shared::EDITOR_COMPONENT_KIND_NETWORK_IDENTITY,
            schema_revision,
            format!(
                "mirror net={} shard={} local={} class={:?}",
                identity.entity.0,
                identity.entity.shard(),
                identity.entity.local(),
                identity.class
            ),
        ));
        components.push(client_editor_component_value(
            game_shared::EDITOR_COMPONENT_KIND_NETWORK_AUTHORITY,
            schema_revision,
            format!("{:?}", authority.mode),
        ));
        components.push(client_editor_component_value(
            game_shared::EDITOR_COMPONENT_KIND_WORLD_CATALOG_REF,
            schema_revision,
            format!(
                "render_geometry={}",
                render_geometry
                    .map(|geometry| format!("{geometry:?}"))
                    .unwrap_or_else(|| "none".to_owned())
            ),
        ));

        rows.push(game_shared::EditorEntityRow {
            entity: identity.entity,
            target: game_shared::EditorTargetKind::Client,
            display_label: name
                .map(|name| name.as_str().to_owned())
                .unwrap_or_else(|| format!("ClientMirror-{}", identity.entity.0)),
            component_count: components.len().min(u16::MAX as usize) as u16,
            schema_revision,
            components,
        });
    }

    inspector
        .state
        .replace_entities(world_revision, schema_revision, schemas, rows);

    if inspector.state.editor_attached() && *last_diagnostic_revision != world_revision.0 {
        *last_diagnostic_revision = world_revision.0;
        inspector
            .state
            .emit_diagnostic(game_shared::DiagnosticPacket::Counter {
                counter: game_shared::DiagnosticCounter {
                    target: "client".to_owned(),
                    name: "world_stream_revision".to_owned(),
                    value: game_shared::DiagnosticValue::U64 {
                        value: world_revision.0,
                    },
                    unit: game_shared::DiagnosticUnit::Count,
                    window: game_shared::DiagnosticWindow {
                        sample_count: 1,
                        duration_ns: 0,
                    },
                },
            });
    }
}

fn client_editor_component_value(
    component_kind: ComponentKind,
    schema_revision: game_shared::EditorSchemaRevision,
    value_preview: String,
) -> game_shared::EditorEntityComponentValue {
    game_shared::EditorEntityComponentValue {
        component_kind,
        schema_revision,
        value_preview,
        raw_payload: Vec::new(),
    }
}

fn client_transform_preview(transform: &Transform) -> String {
    format!(
        "translation=({:.2},{:.2},{:.2}) rotation=({:.3},{:.3},{:.3},{:.3}) scale=({:.2},{:.2},{:.2})",
        transform.translation.x,
        transform.translation.y,
        transform.translation.z,
        transform.rotation.x,
        transform.rotation.y,
        transform.rotation.z,
        transform.rotation.w,
        transform.scale.x,
        transform.scale.y,
        transform.scale.z
    )
}

fn solari_settings_from_env() -> SolariSettings {
    let mut settings = SolariSettings {
        denoise_mode: solari_denoise_mode_from_env(),
        internal_scale: solari_internal_scale_from_env(),
        debug_direct_visibility: std::env::var_os("FUN_SOLARI_DEBUG_DIRECT_VISIBILITY").is_some(),
        ..default()
    };

    apply_u32_env(
        "FUN_SOLARI_WORLD_CACHE_SIZE",
        &mut settings.world_cache_size,
    );
    apply_u32_env(
        "FUN_SOLARI_WORLD_CACHE_UPDATES",
        &mut settings.world_cache_cell_updates_soft_cap,
    );
    apply_u32_env(
        "FUN_SOLARI_WORLD_CACHE_LIGHT_SAMPLES",
        &mut settings.world_cache_direct_light_sample_count,
    );
    apply_u32_env(
        "FUN_SOLARI_WORLD_CACHE_FRAME_SLICES",
        &mut settings.world_cache_frame_slice_count,
    );
    apply_u32_env(
        "FUN_SOLARI_WORLD_CACHE_NEAR_METERS",
        &mut settings.world_cache_near_camera_distance_meters,
    );
    apply_u32_env(
        "FUN_SOLARI_WORLD_CACHE_MID_METERS",
        &mut settings.world_cache_mid_camera_distance_meters,
    );
    apply_u32_env(
        "FUN_SOLARI_WORLD_CACHE_FAR_METERS",
        &mut settings.world_cache_far_camera_distance_meters,
    );
    apply_u32_env(
        "FUN_SOLARI_LIGHT_TILE_BLOCKS",
        &mut settings.light_tile_blocks,
    );
    apply_u32_env(
        "FUN_SOLARI_LIGHT_TILE_SAMPLES",
        &mut settings.light_tile_samples_per_block,
    );
    apply_u32_env(
        "FUN_SOLARI_BLAS_COMPACTION_VERTICES",
        &mut settings.max_blas_compaction_budget_vertices,
    );

    settings
}

fn solari_runtime_params_from_env(settings: &SolariSettings) -> SolariRuntimeParams {
    let architecture = solari_architecture_from_env();
    let visual_target = solari_visual_target_from_env();
    let target_fps = env_u32("FUN_SOLARI_TARGET_FPS").unwrap_or(144);
    let frame_budget_ns = env_u32("FUN_SOLARI_FRAME_BUDGET_NS")
        .unwrap_or_else(|| 1_000_000_000u32.saturating_div(target_fps.max(1)));
    let gpu_budget_ns = env_u32("FUN_SOLARI_GPU_BUDGET_NS").unwrap_or(3_000_000);

    let mut params = match architecture {
        SolariArchitecture::Legacy => SolariRuntimeParams::legacy_from_settings(settings),
        SolariArchitecture::Budgeted => SolariRuntimeParams::budgeted(
            settings,
            visual_target,
            target_fps,
            frame_budget_ns,
            gpu_budget_ns,
        ),
    };

    apply_runtime_f32_env(
        "FUN_SOLARI_RECONSTRUCTION_STRENGTH",
        &mut params.reconstruction_strength,
    );
    apply_runtime_u32_env(
        "FUN_SOLARI_CACHE_UPDATE_BUDGET",
        &mut params.cache_update_budget,
    );
    apply_runtime_u32_env(
        "FUN_SOLARI_SPECULAR_REFRESH_BUDGET",
        &mut params.specular_refresh_budget,
    );
    apply_runtime_f32_env(
        "FUN_SOLARI_DI_REUSE_RADIUS",
        &mut params.di_spatial_reuse_radius_pixels,
    );
    apply_runtime_f32_env(
        "FUN_SOLARI_GI_REUSE_RADIUS",
        &mut params.gi_spatial_reuse_radius_pixels,
    );
    apply_runtime_f32_env(
        "FUN_SOLARI_DI_CONFIDENCE_CAP",
        &mut params.di_temporal_confidence_cap,
    );
    apply_runtime_f32_env(
        "FUN_SOLARI_GI_CONFIDENCE_CAP",
        &mut params.gi_temporal_confidence_cap,
    );
    params.debug_overlay = solari_debug_overlay_from_env();

    params.validated()
}

fn solari_architecture_from_env() -> SolariArchitecture {
    match std::env::var("FUN_SOLARI_ARCH")
        .ok()
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("budgeted") | Some("budget") | Some("v3") => SolariArchitecture::Budgeted,
        Some("legacy") | None => SolariArchitecture::Legacy,
        Some(unknown) => {
            warn!(
                target: "fun::render",
                value = unknown,
                "unknown FUN_SOLARI_ARCH; using legacy Solari architecture"
            );
            SolariArchitecture::Legacy
        }
    }
}

fn solari_visual_target_from_env() -> SolariVisualTarget {
    match std::env::var("FUN_SOLARI_VISUAL_TARGET")
        .ok()
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("competitive") | Some("comp") | Some("fps") => SolariVisualTarget::Competitive,
        Some("cinematic") | Some("quality") => SolariVisualTarget::Cinematic,
        Some("balanced") | None => SolariVisualTarget::Balanced,
        Some(unknown) => {
            warn!(
                target: "fun::render",
                value = unknown,
                "unknown FUN_SOLARI_VISUAL_TARGET; using balanced Solari visual target"
            );
            SolariVisualTarget::Balanced
        }
    }
}

fn solari_debug_overlay_from_env() -> SolariDebugOverlay {
    match std::env::var("FUN_SOLARI_DEBUG_OVERLAY")
        .ok()
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("surface")
        | Some("surface-classification")
        | Some("surface_classification")
        | Some("classification") => SolariDebugOverlay::SurfaceClassification,
        Some("queue") | Some("queues") | Some("work-queue") | Some("work_queue")
        | Some("work-queues") | Some("work_queues") => SolariDebugOverlay::WorkQueues,
        Some("none") | Some("off") | None => SolariDebugOverlay::None,
        Some(unknown) => {
            warn!(
                target: "fun::render",
                value = unknown,
                "unknown FUN_SOLARI_DEBUG_OVERLAY; debug overlay disabled"
            );
            SolariDebugOverlay::None
        }
    }
}

fn apply_runtime_u32_env(name: &'static str, value: &mut u32) {
    if let Some(parsed) = env_u32(name) {
        *value = parsed;
        info!(target: "fun::render", setting = name, value = parsed, "applied Solari runtime integer setting");
    }
}

fn apply_runtime_f32_env(name: &'static str, value: &mut f32) {
    let Some(raw) = std::env::var_os(name) else {
        return;
    };
    let raw = raw.to_string_lossy();
    match raw.parse::<f32>() {
        Ok(parsed) => {
            *value = parsed;
            info!(target: "fun::render", setting = name, value = parsed, "applied Solari runtime float setting");
        }
        Err(error) => {
            warn!(
                target: "fun::render",
                setting = name,
                value = %raw,
                %error,
                "ignored invalid Solari runtime float setting"
            );
        }
    }
}

fn env_u32(name: &'static str) -> Option<u32> {
    let raw = std::env::var_os(name)?;
    let raw = raw.to_string_lossy();
    match raw.parse::<u32>() {
        Ok(parsed) => Some(parsed),
        Err(error) => {
            warn!(
                target: "fun::render",
                setting = name,
                value = %raw,
                %error,
                "ignored invalid Solari integer setting"
            );
            None
        }
    }
}

fn solari_internal_scale_from_env() -> SolariInternalScale {
    let Some(raw) = std::env::var("FUN_SOLARI_INTERNAL_SCALE")
        .ok()
        .map(|value| value.to_ascii_lowercase())
    else {
        return SolariInternalScale::Full;
    };

    match raw.as_str() {
        "1" | "1.0" | "full" | "native" => SolariInternalScale::Full,
        "0.75" | ".75" | "75" | "3/4" | "three-quarter" | "three_quarter" => {
            SolariInternalScale::ThreeQuarter
        }
        "0.66" | "0.666" | "0.67" | ".66" | ".666" | ".67" | "66" | "2/3" | "two-thirds"
        | "two_thirds" => SolariInternalScale::TwoThirds,
        "0.5" | ".5" | "50" | "1/2" | "half" => SolariInternalScale::Half,
        unknown => {
            warn!(
                "Unknown FUN_SOLARI_INTERNAL_SCALE={unknown}; using full-resolution Solari GI reservoirs"
            );
            SolariInternalScale::Full
        }
    }
}

fn apply_u32_env(name: &'static str, value: &mut u32) {
    let Some(raw) = std::env::var_os(name) else {
        return;
    };
    let raw = raw.to_string_lossy();
    match raw.parse::<u32>() {
        Ok(parsed) => {
            *value = parsed;
            info!(target: "fun::render", setting = name, value = parsed, "applied Solari numeric setting");
        }
        Err(error) => {
            warn!(
                target: "fun::render",
                setting = name,
                value = %raw,
                %error,
                "ignored invalid Solari numeric setting"
            );
        }
    }
}

fn solari_denoise_mode_from_env() -> SolariDenoiseMode {
    let mode = std::env::var("FUN_SOLARI_DENOISE_MODE")
        .ok()
        .map(|mode| mode.to_ascii_lowercase());

    parse_solari_denoise_mode(
        mode.as_deref(),
        std::env::var_os("FUN_DISABLE_DLSS_RR").is_some(),
    )
}

fn parse_solari_denoise_mode(
    mode: Option<&str>,
    _dlss_ray_reconstruction_disabled: bool,
) -> SolariDenoiseMode {
    let Some(mode) = mode else {
        return SolariDenoiseMode::BalancedFast;
    };

    match mode {
        "off" | "raw" => SolariDenoiseMode::Off,
        "cheap" | "cheap-temporal" | "cheap_temporal" => SolariDenoiseMode::CheapTemporal,
        "fast" | "balanced-fast" | "balanced_fast" | "balancedfast" => {
            SolariDenoiseMode::BalancedFast
        }
        "balanced" | "svgf" | "svgf-lite" | "svgf_lite" => SolariDenoiseMode::Balanced,
        "quality" | "svgf-quality" | "svgf_quality" => SolariDenoiseMode::Quality,
        "rr" | "dlss" | "dlss-rr" | "dlss_rr" | "ray-reconstruction" => {
            SolariDenoiseMode::DlssRayReconstruction
        }
        unknown => {
            warn!(
                "Unknown FUN_SOLARI_DENOISE_MODE={unknown}; falling back to balanced-fast Solari denoising"
            );
            SolariDenoiseMode::BalancedFast
        }
    }
}

#[cfg(any(test, feature = "benchmarks"))]
pub fn benchmark_parse_solari_denoise_mode(
    mode: Option<&str>,
    dlss_ray_reconstruction_disabled: bool,
) -> SolariDenoiseMode {
    parse_solari_denoise_mode(mode, dlss_ray_reconstruction_disabled)
}

pub fn build_client_app() -> App {
    build_client_app_with_options(ClientAppOptions::from_env())
}

pub fn build_client_app_with_options(options: ClientAppOptions) -> App {
    let mut app = App::new();
    info!(
        target: "fun::client",
        runtime_mode = options.mode.as_env_value(),
        render_profile = options.render_profile.as_env_value(),
        hosted_by_editor = options.hosted_by_editor,
        has_host_instance_id = options.host_instance_id.is_some(),
        has_host_parent_pid = options.host_parent_pid.is_some(),
        has_project_id = options.project_id.is_some(),
        has_scene_id = options.scene_id.is_some(),
        has_server_addr = options.server_addr.is_some(),
        "building Fun client app"
    );
    app.add_plugins((
        FunClientRenderPlugin::new(options.clone()),
        GameClientPlugin::new(options),
    ));

    app
}

fn client_render_creation(render_backend: Backends) -> RenderCreation {
    RenderCreation::Automatic(Box::new(WgpuSettings {
        backends: Some(render_backend),
        instance_flags: InstanceFlags::empty().with_env(),
        ..default()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joined_game_and_editor_preview_share_render_path_signature() {
        let joined_options = ClientAppOptions {
            mode: ClientRuntimeMode::JoinedGame,
            ..Default::default()
        };
        let preview_options = ClientAppOptions {
            mode: ClientRuntimeMode::EditorPreview,
            hosted_by_editor: true,
            ..Default::default()
        };

        let render_config = ClientRenderConfig::from_env();
        let joined_signature = render_path_signature_for_options(
            &joined_options,
            &render_config,
            ClientOpaqueRenderer::Deferred,
        );
        let preview_signature = render_path_signature_for_options(
            &preview_options,
            &render_config,
            ClientOpaqueRenderer::Deferred,
        );

        assert_ne!(joined_options.mode, preview_options.mode);
        assert_eq!(joined_signature, preview_signature);
    }

    #[test]
    fn editor_preview_mode_disables_gameplay_runtime() {
        assert!(!ClientRuntimeMode::EditorPreview.runs_gameplay_runtime());
        assert!(!ClientRuntimeMode::EditorPreview.should_connect_to_game_server());
        assert!(ClientRuntimeMode::JoinedGame.runs_gameplay_runtime());
        assert!(ClientRuntimeMode::JoinedGame.should_connect_to_game_server());
    }

    #[test]
    fn editor_preview_static_stream_keeps_transforms_stable_across_frames() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<SolariResetEvent>()
            .insert_resource(ClientRenderConfig::from_env())
            .insert_resource(ClientLogConfig {
                stream_verbose: false,
                net_verbose: false,
                render_verbose: false,
                benchmark_minimal: true,
            })
            .insert_resource(WorldRenderCatalog::default())
            .insert_resource(Assets::<Mesh>::default())
            .insert_resource(Assets::<MeshletMesh>::default())
            .insert_resource(Assets::<StandardMaterial>::default())
            .insert_resource(StaticPreviewWorldStream::from_chunks(vec![
                preview_test_chunk(),
            ]))
            .init_resource::<LoadedWorldState>()
            .init_resource::<ClientWorldStatus>()
            .add_systems(Update, apply_static_preview_world_stream);

        app.update();
        let before = preview_streamed_transforms(&mut app);
        for _ in 0..5 {
            app.update();
        }
        let after = preview_streamed_transforms(&mut app);

        assert_eq!(before, after);
        assert_eq!(before.len(), 1);
        assert!(app.world().resource::<ClientWorldStatus>().ready);
    }

    fn preview_test_chunk() -> WorldStreamChunk {
        WorldStreamChunk {
            level_id: WorldLevelId("preview-test".to_owned()),
            revision: WorldRevision(1),
            chunk_index: 0,
            chunk_count: 1,
            entities: vec![WorldEntitySpec {
                entity: NetEntity(10_001),
                name: "PreviewStaticProbe".to_owned(),
                class: ReplicationClass::World,
                authority: AuthorityMode::StaticServer,
                transform: game_scene::qtransform(&Transform::from_xyz(1.0, 2.0, 3.0)),
                catalog: None,
                render: None,
                collider: None,
                color: None,
            }],
        }
    }

    fn preview_streamed_transforms(app: &mut App) -> Vec<(u64, [f32; 3])> {
        let mut query = app.world_mut().query::<(&NetworkIdentity, &Transform)>();
        let mut transforms = query
            .iter(app.world())
            .map(|(identity, transform)| (identity.entity.0, transform.translation.to_array()))
            .collect::<Vec<_>>();
        transforms.sort_by_key(|(entity, _)| *entity);
        transforms
    }
}

fn recover_render_device(
    error: &RenderError,
    _main_world: &mut World,
    _render_world: &mut World,
) -> RenderErrorPolicy {
    info!(
        "[client render] renderer reported {:?}: {}; recreating renderer with the same profile",
        error.ty, error.description
    );

    match error.ty {
        ErrorType::DeviceLost | ErrorType::OutOfMemory | ErrorType::Internal => {
            RenderErrorPolicy::Recover(client_render_creation(selected_render_backend()))
        }
        ErrorType::Validation => {
            info!(
                "[client render] validation error may be fallout from a lost GPU device; attempting immediate renderer recovery"
            );
            RenderErrorPolicy::Recover(client_render_creation(selected_render_backend()))
        }
    }
}

fn selected_render_backend() -> Backends {
    match std::env::var("FUN_RENDER_BACKEND")
        .as_deref()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Ok("dx12") | Ok("d3d12") | Ok("directx12") => Backends::DX12,
        Ok("auto") => Backends::VULKAN | Backends::DX12,
        Ok("vulkan") | Ok("vk") | Ok("") | Err(_) => Backends::VULKAN,
        Ok(other) => {
            info!("[client render] unknown FUN_RENDER_BACKEND={other}; using Vulkan");
            Backends::VULKAN
        }
    }
}

fn selected_present_mode() -> PresentMode {
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
            info!("[client render] unknown FUN_PRESENT_MODE={other}; using Immediate");
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

fn setup_lighting(mut commands: Commands) {
    info!("[client render] spawning directional light");
    commands.spawn((
        DirectionalLight {
            illuminance: 15_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(8.0, 16.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn apply_startup_window_config(
    mut applied: Local<bool>,
    window_config: Res<ClientWindowConfig>,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))] mut schedule_profiler: ResMut<
        ClientScheduleProfiler,
    >,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))] mut frame_profiler: ResMut<
        DetailedFrameProfiler,
    >,
    mut primary_window: Query<&mut Window, With<PrimaryWindow>>,
) {
    crate::frame_profile_start!(started);
    if *applied {
        #[cfg(all(feature = "render_diagnostics", debug_assertions))]
        schedule_profiler.record_elapsed(ClientScheduleSystem::RenderConfigWindow, started);
        crate::frame_profile_elapsed!(
            frame_profiler,
            started,
            "Update",
            "apply_startup_window_config",
        );
        return;
    }

    *applied = true;
    if !window_config.maximized {
        #[cfg(all(feature = "render_diagnostics", debug_assertions))]
        schedule_profiler.record_elapsed(ClientScheduleSystem::RenderConfigWindow, started);
        crate::frame_profile_elapsed!(
            frame_profiler,
            started,
            "Update",
            "apply_startup_window_config",
        );
        return;
    }

    let Ok(mut window) = primary_window.single_mut() else {
        warn!(target: "fun::render", "could not maximize client window because no primary window was available");
        #[cfg(all(feature = "render_diagnostics", debug_assertions))]
        schedule_profiler.record_elapsed(ClientScheduleSystem::RenderConfigWindow, started);
        crate::frame_profile_elapsed!(
            frame_profiler,
            started,
            "Update",
            "apply_startup_window_config",
        );
        return;
    };

    window.set_maximized(true);
    info!(target: "fun::render", "requested maximized primary window");
    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    schedule_profiler.record_elapsed(ClientScheduleSystem::RenderConfigWindow, started);
    crate::frame_profile_elapsed!(
        frame_profiler,
        started,
        "Update",
        "apply_startup_window_config",
    );
}

fn connect_to_game_server(options: Res<ClientAppOptions>, mut client: ResMut<QuinnetClient>) {
    if !options.mode.should_connect_to_game_server() {
        info!(
            target: "fun::net",
            runtime_mode = options.mode.as_env_value(),
            "client runtime mode does not open a game-server connection"
        );
        return;
    }

    if !client.is_disconnected() {
        info!("[client net] Quinnet already has an active connection");
        debug!(target: "fun::net", "quinnet already has an active connection");
        return;
    }

    let server_addr = options.server_addr.as_deref().unwrap_or(GAME_SERVER_ADDR);
    info!("[client net] opening connection to {server_addr}");
    info!(target: "fun::net", server_addr, "opening game server connection");
    let limits = ChannelLimits::default();
    let config = ClientConnectionConfiguration {
        addr_config: ClientAddrConfiguration::from_strings(server_addr, "0.0.0.0:0")
            .expect("game server address should be valid"),
        cert_mode: CertificateVerificationMode::SkipVerification,
        defaultables: ClientConnectionConfigurationDefaultables {
            send_channels_cfg: ClientChannel::channels_configuration(limits),
            ..Default::default()
        },
    };

    match client.open_connection(config) {
        Ok(connection_id) => {
            info!("[client net] connecting to {server_addr} with local connection {connection_id}");
            info!(
                target: "fun::net",
                server_addr,
                connection_id,
                "connecting to game server"
            );
        }
        Err(error) => {
            info!("[client net] failed to start game server connection: {error}");
            error!(target: "fun::net", server_addr, %error, "failed to start game server connection");
        }
    }
}

fn send_client_hello(
    mut events: MessageReader<ConnectionEvent>,
    mut client: ResMut<QuinnetClient>,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))] mut frame_profiler: ResMut<
        DetailedFrameProfiler,
    >,
) {
    crate::frame_profile_scope!(_scope, frame_profiler, "Update", "send_client_hello");
    for event in events.read() {
        info!("[client net] connection event id={}", event.id);
        info!(target: "fun::net", connection_id = event.id, "connection event");
        let Some(connection) = client.get_connection_mut_by_id(event.id) else {
            info!(
                "[client net] connection event id={} had no matching connection",
                event.id
            );
            warn!(
                target: "fun::net",
                connection_id = event.id,
                "connection event had no matching Quinnet connection"
            );
            continue;
        };

        let hello = ClientPacket::Hello {
            hello: ClientHello {
                protocol_version: 1,
                session_token: Vec::new(),
                feature_bits: 0,
                oldest_input_sequence: PacketSequence(0),
            },
        };

        match encode_client_packet(&hello) {
            Ok(bytes) => {
                let byte_len = bytes.len();
                connection.try_send_payload_on(ClientChannel::Control, bytes);
                info!(
                    "[client net] sent hello on control channel for connection {} ({} bytes)",
                    event.id, byte_len
                );
                info!(
                    target: "fun::net",
                    connection_id = event.id,
                    bytes = byte_len,
                    channel = "control",
                    "sent client hello"
                );
            }
            Err(error) => {
                info!("[client net] failed to encode client hello: {error}");
                error!(target: "fun::net", %error, "failed to encode client hello");
            }
        }
    }
}

fn receive_server_control(
    mut client: ResMut<QuinnetClient>,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))] mut perf_counters: ResMut<
        ClientPerfCounters,
    >,
    _log_config: Res<ClientLogConfig>,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))] mut schedule_profiler: ResMut<
        ClientScheduleProfiler,
    >,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))] mut frame_profiler: ResMut<
        DetailedFrameProfiler,
    >,
) {
    crate::frame_profile_start!(system_started);
    let Some(connection) = client.get_connection_mut() else {
        #[cfg(all(feature = "render_diagnostics", debug_assertions))]
        schedule_profiler.record_elapsed(ClientScheduleSystem::NetworkingReceive, system_started);
        crate::frame_profile_elapsed!(
            frame_profiler,
            system_started,
            "Update",
            "receive_server_control",
        );
        return;
    };

    while let Some(payload) = connection.try_receive_payload(ServerChannel::Control) {
        crate::frame_profile_start!(receive_started);
        let payload_len = payload.as_ref().len();
        game_shared::fun_diag_info_if!(
            _log_config.net_verbose(),
            target: "fun::net",
            bytes = payload_len,
            channel = "control",
            "received server control payload"
        );
        crate::frame_profile_elapsed!(
            frame_profiler,
            receive_started,
            "Update",
            "receive_server_control",
            "try_receive_control_payload",
        );
        crate::frame_profile_start!(decode_started);
        match decode_server_packet(payload.as_ref()) {
            Ok(ServerPacket::Welcome { welcome: _welcome }) => {
                crate::frame_profile_elapsed!(
                    frame_profiler,
                    decode_started,
                    "Update",
                    "receive_server_control",
                    "decode_server_packet",
                );
                game_shared::fun_diag_info_if!(
                    _log_config.net_verbose(),
                    target: "fun::net",
                    client_id = _welcome.client_id.0,
                    server_tick = _welcome.server_tick.0,
                    baseline_tick = _welcome.baseline_tick.0,
                    feature_bits = _welcome.feature_bits,
                    "received server welcome"
                );
            }
            Ok(ServerPacket::Disconnect { reason }) => {
                crate::frame_profile_elapsed!(
                    frame_profiler,
                    decode_started,
                    "Update",
                    "receive_server_control",
                    "decode_server_packet",
                );
                warn!(target: "fun::net", %reason, "server disconnected client");
            }
            Ok(_packet) => {
                crate::frame_profile_elapsed!(
                    frame_profiler,
                    decode_started,
                    "Update",
                    "receive_server_control",
                    "decode_server_packet",
                );
                game_shared::fun_diag_debug_if!(
                    _log_config.net_verbose(),
                    target: "fun::net",
                    packet = ?_packet,
                    "ignoring control packet on client"
                );
            }
            Err(error) => {
                crate::frame_profile_elapsed!(
                    frame_profiler,
                    decode_started,
                    "Update",
                    "receive_server_control",
                    "decode_server_packet",
                );
                error!(target: "fun::net", %error, bytes = payload_len, "failed to decode server control packet");
            }
        }
        #[cfg(all(feature = "render_diagnostics", debug_assertions))]
        perf_counters.add_network_receive_ns(
            receive_started
                .elapsed()
                .as_nanos()
                .min(u128::from(u64::MAX)) as u64,
        );
    }
    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    schedule_profiler.record_elapsed(ClientScheduleSystem::NetworkingReceive, system_started);
    crate::frame_profile_elapsed!(
        frame_profiler,
        system_started,
        "Update",
        "receive_server_control",
    );
}

fn receive_server_snapshots(
    mut client: ResMut<QuinnetClient>,
    loaded_world: Res<LoadedWorldState>,
    mut transforms: Query<&mut Transform>,
    _log_config: Res<ClientLogConfig>,
) {
    let Some(connection) = client.get_connection_mut() else {
        return;
    };

    while let Some(payload) = connection.try_receive_payload(ServerChannel::Snapshot) {
        let payload_len = payload.as_ref().len();
        let snapshot = match decode_server_packet(payload.as_ref()) {
            Ok(ServerPacket::Snapshot { snapshot }) => snapshot,
            Ok(_packet) => {
                game_shared::fun_diag_debug_if!(
                    _log_config.net_verbose(),
                    target: "fun::net",
                    packet = ?_packet,
                    "ignoring non-snapshot packet on snapshot channel"
                );
                continue;
            }
            Err(error) => {
                error!(
                    target: "fun::net",
                    %error,
                    bytes = payload_len,
                    "failed to decode server snapshot packet"
                );
                continue;
            }
        };

        let mut applied = 0usize;
        for delta in &snapshot.entities {
            let Some(transform_delta) = delta.transform else {
                continue;
            };
            let Some(entity) = loaded_world.spawned_entities.get(&delta.entity).copied() else {
                continue;
            };
            let Ok(mut transform) = transforms.get_mut(entity) else {
                continue;
            };
            *transform = transform_from_quantized(transform_delta);
            applied = applied.saturating_add(1);
        }

        game_shared::fun_diag_info_if!(
            _log_config.net_verbose(),
            target: "fun::net",
            sequence = snapshot.sequence.0,
            server_tick = snapshot.server_tick.0,
            entities = snapshot.entities.len(),
            applied,
            bytes = payload_len,
            "applied authoritative snapshot"
        );
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Bevy system parameters expose each runtime resource and query directly to the scheduler"
)]
fn receive_world_stream(
    mut commands: Commands,
    mut client: ResMut<QuinnetClient>,
    render_config: Res<ClientRenderConfig>,
    mut loaded_world: ResMut<LoadedWorldState>,
    mut world_status: ResMut<ClientWorldStatus>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut meshlet_meshes: ResMut<Assets<MeshletMesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    catalog: Res<WorldRenderCatalog>,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    mut runtime: ClientRuntimeProfiler,
    #[cfg(not(all(feature = "render_diagnostics", debug_assertions)))]
    runtime: ClientRuntimeProfiler,
    solari_cameras: Query<Entity, (With<Camera3d>, Without<SolariLighting>)>,
    mut solari_lighting: Query<&mut SolariLighting>,
    mut solari_reset_events: MessageWriter<SolariResetEvent>,
    #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))] dlss_rr_supported: Option<
        Res<DlssRayReconstructionSupported>,
    >,
    #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))] dlss_rr_cameras: Query<
        Entity,
        (With<Camera3d>, Without<Dlss<DlssRayReconstructionFeature>>),
    >,
    #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))] mut dlss_rr: Query<
        &mut Dlss<DlssRayReconstructionFeature>,
    >,
) {
    crate::frame_profile_start!(system_started);
    let Some(connection) = client.get_connection_mut() else {
        #[cfg(all(feature = "render_diagnostics", debug_assertions))]
        runtime
            .schedule_profiler
            .record_elapsed(ClientScheduleSystem::NetworkingReceive, system_started);
        crate::frame_profile_elapsed!(
            runtime.frame_profiler,
            system_started,
            "Update",
            "receive_world_stream",
        );
        return;
    };

    while let Some(payload) = connection.try_receive_payload(ServerChannel::Stream) {
        crate::frame_profile_start!(receive_started);
        let payload_len = payload.as_ref().len();
        game_shared::fun_diag_debug_if!(
            runtime.log_config.stream_verbose(),
            target: "fun::stream",
            bytes = payload_len,
            channel = "stream",
            "received stream payload"
        );
        let packet = match decode_server_packet(payload.as_ref()) {
            Ok(ServerPacket::WorldStream { chunk }) => chunk,
            Ok(_packet) => {
                game_shared::fun_diag_debug_if!(
                    runtime.log_config.stream_verbose(),
                    target: "fun::stream",
                    packet = ?_packet,
                    "ignoring non-world packet on world stream channel"
                );
                #[cfg(all(feature = "render_diagnostics", debug_assertions))]
                runtime.perf_counters.add_network_receive_ns(
                    receive_started
                        .elapsed()
                        .as_nanos()
                        .min(u128::from(u64::MAX)) as u64,
                );
                continue;
            }
            Err(error) => {
                error!(target: "fun::stream", %error, bytes = payload_len, "failed to decode world stream packet");
                #[cfg(all(feature = "render_diagnostics", debug_assertions))]
                runtime.perf_counters.add_network_receive_ns(
                    receive_started
                        .elapsed()
                        .as_nanos()
                        .min(u128::from(u64::MAX)) as u64,
                );
                continue;
            }
        };
        crate::frame_profile_elapsed!(
            runtime.frame_profiler,
            receive_started,
            "Update",
            "receive_world_stream",
            "decode_server_packet",
        );
        #[cfg(all(feature = "render_diagnostics", debug_assertions))]
        runtime.perf_counters.add_network_receive_ns(
            receive_started
                .elapsed()
                .as_nanos()
                .min(u128::from(u64::MAX)) as u64,
        );

        game_shared::fun_diag_info_if!(
            runtime.log_config.stream_verbose(),
            target: "fun::stream",
            level = %packet.level_id.0,
            revision = packet.revision.0,
            chunk_index = packet.chunk_index,
            chunk_number = packet.chunk_index + 1,
            chunk_count = packet.chunk_count,
            entity_count = packet.entities.len(),
            bytes = payload_len,
            "applying streamed world chunk"
        );
        #[cfg(all(feature = "render_diagnostics", debug_assertions))]
        game_shared::fun_diag_info!(
            target: "fun::client::profiler",
            server_tick = tracing::field::Empty,
            client_id = tracing::field::Empty,
            packet_type = "world_stream",
            world_revision = packet.revision.0,
            chunk_index = packet.chunk_index.saturating_add(1),
            chunk_count = packet.chunk_count,
            bytes = payload_len,
            stage = "receive_world_stream",
            duration_ns = receive_started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64,
            "client profiler event"
        );

        #[cfg(all(feature = "diagnostics", debug_assertions))]
        let apply_diagnostic_started = std::time::Instant::now();
        crate::frame_profile_start!(apply_started);
        let world_revision_changed = apply_world_stream_chunk(
            &mut commands,
            &mut loaded_world,
            &mut world_status,
            &mut meshes,
            &mut meshlet_meshes,
            &mut materials,
            &catalog,
            #[cfg(all(feature = "render_diagnostics", debug_assertions))]
            runtime.perf_counters.as_mut(),
            #[cfg(all(feature = "render_diagnostics", debug_assertions))]
            runtime.frame_profiler.as_mut(),
            &render_config,
            &runtime.log_config,
            &packet,
        );
        #[cfg(all(feature = "render_diagnostics", debug_assertions))]
        let apply_ns = apply_started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        #[cfg(all(feature = "render_diagnostics", debug_assertions))]
        runtime.perf_counters.add_world_stream_apply_ns(apply_ns);
        #[cfg(all(feature = "render_diagnostics", debug_assertions))]
        runtime
            .schedule_profiler
            .record_ns(ClientScheduleSystem::WorldStreamApply, apply_ns);
        #[cfg(all(feature = "diagnostics", debug_assertions))]
        let apply_diagnostic_ns = apply_diagnostic_started
            .elapsed()
            .as_nanos()
            .min(u128::from(u64::MAX)) as u64;
        game_shared::fun_diag_info!(
            target: "fun::client::profiler",
            server_tick = tracing::field::Empty,
            client_id = tracing::field::Empty,
            packet_type = "world_stream",
            world_revision = packet.revision.0,
            chunk_index = packet.chunk_index.saturating_add(1),
            chunk_count = packet.chunk_count,
            bytes = payload_len,
            stage = "apply_world_stream",
            duration_ns = apply_diagnostic_ns,
            "client profiler event"
        );
        crate::frame_profile_ns!(
            runtime.frame_profiler,
            apply_ns,
            "Update",
            "receive_world_stream",
            "apply_world_stream_chunk",
        );
        if world_revision_changed {
            crate::frame_profile_start!(reset_started);
            request_solari_lighting_history_reset(
                "streamed world revision changed",
                &mut solari_reset_events,
                &mut solari_lighting,
            );
            #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
            reset_dlss_ray_reconstruction_history(&mut dlss_rr);
            crate::frame_profile_elapsed!(
                runtime.frame_profiler,
                reset_started,
                "Update",
                "receive_world_stream",
                "reset_temporal_history",
            );
        }

        if loaded_world.is_complete() {
            let became_ready = !world_status.ready;
            if became_ready {
                info!(
                    target: "fun::stream",
                    level = ?loaded_world.level_id,
                    revision = ?loaded_world.revision.map(|revision| revision.0),
                    spawned_entities = loaded_world.spawned_entities.len(),
                    chunks_received = loaded_world.received_chunks.len(),
                    "streamed world is ready"
                );
            }
            world_status.ready = true;
            if became_ready {
                crate::frame_profile_start!(ready_started);
                enable_solari_lighting_for_ready_world(
                    &mut commands,
                    &render_config,
                    &solari_cameras,
                    &mut solari_lighting,
                    &mut solari_reset_events,
                );
                #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
                enable_dlss_ray_reconstruction_for_ready_world(
                    &mut commands,
                    &render_config,
                    dlss_rr_supported.as_deref(),
                    &dlss_rr_cameras,
                    &mut dlss_rr,
                );
                #[cfg(any(not(feature = "dlss"), feature = "force_disable_dlss"))]
                info!("[client render] DLSS feature disabled at compile time");
                crate::frame_profile_elapsed!(
                    runtime.frame_profiler,
                    ready_started,
                    "Update",
                    "receive_world_stream",
                    "enable_ready_world_rendering",
                );
            }
        }

        if loaded_world.is_complete() && !loaded_world.ack_sent {
            crate::frame_profile_start!(ack_started);
            let ack = ClientPacket::WorldReady {
                ack: WorldStreamAck {
                    level_id: WorldLevelId(loaded_world.level_id.clone().unwrap_or_default()),
                    revision: loaded_world.revision.unwrap_or_default(),
                },
            };

            match encode_client_packet(&ack) {
                Ok(bytes) => {
                    let _byte_len = bytes.len();
                    connection.try_send_payload_on(ClientChannel::Control, bytes);
                    loaded_world.ack_sent = true;
                    game_shared::fun_diag_info_if!(
                        runtime.log_config.stream_verbose(),
                        target: "fun::stream",
                        level = %loaded_world.level_id.as_deref().unwrap_or_default(),
                        revision = loaded_world.revision.unwrap_or_default().0,
                        bytes = _byte_len,
                        "sent world-ready ack"
                    );
                }
                Err(error) => {
                    error!(target: "fun::stream", %error, "failed to encode world ready acknowledgement");
                }
            }
            crate::frame_profile_elapsed!(
                runtime.frame_profiler,
                ack_started,
                "Update",
                "receive_world_stream",
                "send_world_ready_ack",
            );
        }
    }
    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    runtime
        .schedule_profiler
        .record_elapsed(ClientScheduleSystem::NetworkingReceive, system_started);
    crate::frame_profile_elapsed!(
        runtime.frame_profiler,
        system_started,
        "Update",
        "receive_world_stream",
    );
}

fn enable_solari_lighting_for_ready_world(
    commands: &mut Commands,
    render_config: &ClientRenderConfig,
    solari_cameras: &Query<Entity, (With<Camera3d>, Without<SolariLighting>)>,
    solari_lighting: &mut Query<&mut SolariLighting>,
    solari_reset_events: &mut MessageWriter<SolariResetEvent>,
) {
    if !render_config.solari_enabled {
        info!("[client render] streamed world ready; Solari remains disabled");
        info!(target: "fun::solari", "streamed world ready; Solari remains disabled");
        return;
    }

    let mut enabled_count = 0usize;
    for camera_entity in solari_cameras.iter() {
        commands.entity(camera_entity).insert((
            CameraMainTextureUsages::default().with(TextureUsages::STORAGE_BINDING),
            SolariLighting::default(),
        ));
        enabled_count += 1;
    }

    if enabled_count > 0 {
        info!("[client render] enabled Solari lighting for {enabled_count} ready camera view(s)");
        info!(
            target: "fun::solari",
            enabled_views = enabled_count,
            "enabled Solari lighting for ready world"
        );
    }

    request_solari_lighting_history_reset(
        "streamed world became ready",
        solari_reset_events,
        solari_lighting,
    );
}

fn request_solari_lighting_history_reset(
    reason: &str,
    solari_reset_events: &mut MessageWriter<SolariResetEvent>,
    solari_lighting: &mut Query<&mut SolariLighting>,
) {
    solari_reset_events.write(SolariResetEvent {
        reason: Cow::Owned(reason.to_owned()),
    });
    info!("[client render] requested Solari temporal history reset: {reason}");
    let reset_count = reset_solari_lighting_history(solari_lighting);
    info!(
        target: "fun::solari",
        render_solari_reset_requested = true,
        render_solari_reset_reason = reason,
        render_solari_reset_view_count = reset_count,
        render_solari_reset_resource_generation_before = tracing::field::Empty,
        render_solari_reset_resource_generation_after = tracing::field::Empty,
        "requested Solari temporal history reset"
    );
}

fn reset_solari_lighting_history(solari_lighting: &mut Query<&mut SolariLighting>) -> usize {
    let mut reset_count = 0usize;
    for mut lighting in solari_lighting.iter_mut() {
        lighting.reset = true;
        reset_count += 1;
    }

    if reset_count > 0 {
        info!("[client render] reset Solari temporal history for {reset_count} view(s)");
        info!(
            target: "fun::solari",
            render_solari_reset_applied = true,
            render_solari_reset_reason = "direct_component_reset",
            render_solari_reset_view_count = reset_count,
            render_solari_reset_resource_generation_before = tracing::field::Empty,
            render_solari_reset_resource_generation_after = tracing::field::Empty,
            reset_views = reset_count,
            "reset Solari temporal history"
        );
    }
    reset_count
}

#[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
fn enable_dlss_ray_reconstruction_for_ready_world(
    commands: &mut Commands,
    render_config: &ClientRenderConfig,
    dlss_rr_supported: Option<&DlssRayReconstructionSupported>,
    dlss_rr_cameras: &Query<Entity, (With<Camera3d>, Without<Dlss<DlssRayReconstructionFeature>>)>,
    dlss_rr: &mut Query<&mut Dlss<DlssRayReconstructionFeature>>,
) {
    if !render_config.solari_enabled {
        return;
    }

    if !render_config.dlss_rr_enabled {
        if render_config.dlss_rr_disabled_by_denoise_mode {
            info!(
                "[client render] DLSS Ray Reconstruction disabled because the active Solari denoiser is not the RR preset"
            );
            info!(
                target: "fun::rr",
                "DLSS Ray Reconstruction disabled by active Solari denoiser"
            );
        } else {
            info!("[client render] DLSS Ray Reconstruction disabled by FUN_DISABLE_DLSS_RR");
            info!(target: "fun::rr", "DLSS Ray Reconstruction disabled by FUN_DISABLE_DLSS_RR");
        }
        return;
    }

    if dlss_rr_supported.is_none() {
        info!(
            "[client render] DLSS Ray Reconstruction unavailable; Solari lighting will use its non-DLSS path"
        );
        info!(target: "fun::rr", "DLSS Ray Reconstruction unavailable; Solari lighting will use its non-DLSS path");
        return;
    }

    let mut enabled_count = 0usize;
    for camera_entity in dlss_rr_cameras.iter() {
        commands
            .entity(camera_entity)
            .insert(Dlss::<DlssRayReconstructionFeature> {
                perf_quality_mode: DLSS_RR_MODE,
                reset: true,
                _phantom_data: Default::default(),
            });
        enabled_count += 1;
    }

    if enabled_count > 0 {
        info!(
            "[client render] enabled DLSS Ray Reconstruction for {enabled_count} ready camera view(s)"
        );
        info!(
            target: "fun::rr",
            enabled_views = enabled_count,
            mode = ?DLSS_RR_MODE,
            "DLSS Ray Reconstruction enabled for Solari lighting"
        );
    }

    reset_dlss_ray_reconstruction_history(dlss_rr);
}

#[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
fn reset_dlss_ray_reconstruction_history(
    dlss_rr: &mut Query<&mut Dlss<DlssRayReconstructionFeature>>,
) {
    let mut reset_count = 0usize;
    for mut dlss in dlss_rr.iter_mut() {
        dlss.reset = true;
        reset_count += 1;
    }

    if reset_count > 0 {
        info!("[client render] reset DLSS Ray Reconstruction history for {reset_count} view(s)");
        info!(
            target: "fun::rr",
            reset_views = reset_count,
            "reset DLSS Ray Reconstruction history"
        );
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "world-stream application threads explicit mutable stores through one deterministic baseline update path"
)]
fn apply_world_stream_chunk(
    commands: &mut Commands,
    loaded_world: &mut LoadedWorldState,
    world_status: &mut ClientWorldStatus,
    meshes: &mut Assets<Mesh>,
    meshlet_meshes: &mut Assets<MeshletMesh>,
    materials: &mut Assets<StandardMaterial>,
    catalog: &WorldRenderCatalog,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    perf_counters: &mut ClientPerfCounters,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    frame_profiler: &mut DetailedFrameProfiler,
    render_config: &ClientRenderConfig,
    _log_config: &ClientLogConfig,
    chunk: &WorldStreamChunk,
) -> bool {
    let mut world_revision_changed = false;
    if loaded_world.revision != Some(chunk.revision)
        || loaded_world.level_id.as_deref() != Some(chunk.level_id.0.as_str())
    {
        crate::frame_profile_start!(reset_started);
        world_revision_changed = true;
        game_shared::fun_diag_info_if!(
            _log_config.stream_verbose(),
            target: "fun::stream",
            old_level = ?loaded_world.level_id,
            old_revision = ?loaded_world.revision.map(|revision| revision.0),
            old_spawned_entities = loaded_world.spawned_entities.len(),
            new_level = %chunk.level_id.0,
            new_revision = chunk.revision.0,
            expected_chunks = chunk.chunk_count,
            "resetting streamed world"
        );
        for entity in loaded_world.spawned_entities.values().copied() {
            commands.entity(entity).despawn();
        }

        loaded_world.level_id = Some(chunk.level_id.0.clone());
        loaded_world.revision = Some(chunk.revision);
        loaded_world.expected_chunks = chunk.chunk_count;
        loaded_world.received_chunks.clear();
        loaded_world.spawned_entities.clear();
        loaded_world.ack_sent = false;
        world_status.ready = false;
        game_shared::fun_diag_info_if!(
            _log_config.stream_verbose(),
            target: "fun::stream",
            level = %chunk.level_id.0,
            revision = chunk.revision.0,
            chunk_count = chunk.chunk_count,
            "receiving streamed world"
        );
        crate::frame_profile_elapsed!(
            frame_profiler,
            reset_started,
            "Update",
            "receive_world_stream",
            "apply_world_stream_chunk",
            "reset_streamed_world",
        );
    }

    for spec in &chunk.entities {
        if loaded_world.spawned_entities.contains_key(&spec.entity) {
            game_shared::fun_diag_warn!(
                target: "fun::stream",
                net_entity = spec.entity.0,
                name = %spec.name,
                "skipping duplicate streamed entity"
            );
            continue;
        }

        crate::frame_profile_start!(spawn_started);
        let entity = spawn_streamed_entity(
            commands,
            meshes,
            meshlet_meshes,
            materials,
            catalog,
            #[cfg(all(feature = "render_diagnostics", debug_assertions))]
            perf_counters,
            #[cfg(all(feature = "render_diagnostics", debug_assertions))]
            frame_profiler,
            render_config,
            _log_config,
            spec,
        );
        crate::frame_profile_elapsed!(
            frame_profiler,
            spawn_started,
            "Update",
            "receive_world_stream",
            "apply_world_stream_chunk",
            "spawn_streamed_entity",
        );
        game_shared::fun_diag_debug_if!(
            _log_config.stream_verbose(),
            target: "fun::stream",
            ecs_entity = ?entity,
            net_entity = spec.entity.0,
            name = %spec.name,
            "spawned streamed entity"
        );
        loaded_world.spawned_entities.insert(spec.entity, entity);
    }

    loaded_world.received_chunks.insert(chunk.chunk_index);
    game_shared::fun_diag_info_if!(
        _log_config.stream_verbose(),
        target: "fun::stream",
        received_chunks = loaded_world.received_chunks.len(),
        expected_chunks = loaded_world.expected_chunks,
        spawned_entities = loaded_world.spawned_entities.len(),
        "streamed world chunk complete"
    );

    world_revision_changed
}

#[allow(
    clippy::too_many_arguments,
    reason = "spawn wiring keeps Bevy asset stores explicit while catalog-driven construction is still local"
)]
fn spawn_streamed_entity(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    meshlet_meshes: &mut Assets<MeshletMesh>,
    materials: &mut Assets<StandardMaterial>,
    catalog: &WorldRenderCatalog,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    perf_counters: &mut ClientPerfCounters,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    frame_profiler: &mut DetailedFrameProfiler,
    render_config: &ClientRenderConfig,
    _log_config: &ClientLogConfig,
    spec: &WorldEntitySpec,
) -> Entity {
    let _translation = vec3_from_quantized(spec.transform.translation);
    let mut entity_commands = commands.spawn((
        Name::new(spec.name.clone()),
        NetworkIdentity {
            entity: spec.entity,
            class: spec.class,
        },
        NetworkAuthority {
            mode: spec.authority,
        },
        transform_from_quantized(spec.transform),
    ));

    game_shared::fun_diag_debug_if!(
        _log_config.stream_verbose(),
        target: "fun::stream::entity",
        net_entity = spec.entity.0,
        name = %spec.name,
        class = ?spec.class,
        authority = ?spec.authority,
        translation_x = _translation.x,
        translation_y = _translation.y,
        translation_z = _translation.z,
        catalog = catalog_ref_summary(spec.catalog),
        render = ?spec.render,
        collider = ?spec.collider,
        color = ?spec.color,
        "streamed entity spawn spec"
    );

    if let Some(catalog_ref) = spec.catalog {
        crate::frame_profile_start!(lookup_started);
        let compiled = catalog.lookup(catalog_ref);
        #[cfg(all(feature = "render_diagnostics", debug_assertions))]
        perf_counters.add_catalog_lookup_ns(
            lookup_started
                .elapsed()
                .as_nanos()
                .min(u128::from(u64::MAX)) as u64,
        );
        crate::frame_profile_elapsed!(
            frame_profiler,
            lookup_started,
            "Update",
            "receive_world_stream",
            "apply_world_stream_chunk",
            "spawn_streamed_entity",
            "catalog_lookup",
        );

        if let Some(compiled) = compiled {
            crate::frame_profile_start!(render_insert_started);
            entity_commands.insert(compiled.geometry_class);
            if compiled.geometry_class.uses_meshlet() {
                if let (Some(meshlet_mesh), Some(material)) =
                    (compiled.meshlet_mesh.as_ref(), compiled.material.as_ref())
                {
                    entity_commands.insert((
                        MeshletMesh3d(meshlet_mesh.clone()),
                        MeshMaterial3d::<StandardMaterial>(material.clone()),
                    ));
                }
            } else if compiled.geometry_class.uses_raster_mesh()
                && let (Some(mesh), Some(material)) =
                    (compiled.raster_mesh.as_ref(), compiled.material.as_ref())
            {
                entity_commands.insert((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d::<StandardMaterial>(material.clone()),
                ));
            }

            if render_config.solari_enabled
                && let Some(ray_proxy) = compiled.ray_proxy.as_ref()
            {
                entity_commands.insert(RaytracingMesh3d(ray_proxy.clone()));
            }

            if let Some(collider) = compiled.collider.as_ref() {
                entity_commands.insert((RigidBody::Static, collider.clone()));
            }
            crate::frame_profile_elapsed!(
                frame_profiler,
                render_insert_started,
                "Update",
                "receive_world_stream",
                "apply_world_stream_chunk",
                "spawn_streamed_entity",
                "insert_catalog_render_components",
            );

            game_shared::fun_diag_info_if!(
                _log_config.render_verbose(),
                target: "fun::render_catalog",
                name = %spec.name,
                asset_id = catalog_ref.asset_id,
                material_id = catalog_ref.material_id,
                collider_id = catalog_ref.collider_id,
                catalog_asset = compiled.entry.name,
                geometry_class = ?compiled.geometry_class,
                triangles = compiled.triangle_count,
                meshlet = compiled.geometry_class.uses_meshlet(),
                raster = compiled.geometry_class.uses_raster_mesh(),
                raytracing = render_config.solari_enabled && compiled.ray_proxy.is_some(),
                "inserted catalog-backed streamed render components"
            );
        } else {
            warn_missing_catalog_ref(catalog_ref, &spec.name);
        }
    } else if let Some(primitive) = spec.render {
        crate::frame_profile_start!(primitive_started);
        let mesh = mesh_from_primitive(primitive);
        let (raytracing_mesh, meshlet_mesh) = add_scene_mesh_assets(
            meshes,
            meshlet_meshes,
            mesh,
            &spec.name,
            render_config,
            _log_config,
        );
        let material = materials.add(color_from_packed(spec.color));
        crate::frame_profile_elapsed!(
            frame_profiler,
            primitive_started,
            "Update",
            "receive_world_stream",
            "apply_world_stream_chunk",
            "spawn_streamed_entity",
            "build_primitive_mesh_assets",
        );

        crate::frame_profile_start!(render_insert_started);
        if render_config.meshlets_enabled {
            entity_commands.insert((
                MeshletMesh3d(meshlet_mesh.expect("meshlet handle should exist when enabled")),
                MeshMaterial3d::<StandardMaterial>(material),
            ));
        } else if let Some(mesh) = raytracing_mesh.as_ref() {
            entity_commands.insert((
                Mesh3d(mesh.clone()),
                MeshMaterial3d::<StandardMaterial>(material),
            ));
        }

        if render_config.solari_enabled
            && let Some(raytracing_mesh) = raytracing_mesh
        {
            entity_commands.insert(RaytracingMesh3d(raytracing_mesh));
        }
        crate::frame_profile_elapsed!(
            frame_profiler,
            render_insert_started,
            "Update",
            "receive_world_stream",
            "apply_world_stream_chunk",
            "spawn_streamed_entity",
            "insert_primitive_render_components",
        );

        game_shared::fun_diag_debug_if!(
            _log_config.render_verbose(),
            target: "fun::render::entity",
            name = %spec.name,
            meshlet = render_config.meshlets_enabled,
            raytracing = render_config.solari_enabled,
            "inserted streamed render components"
        );
    }

    if spec.catalog.is_none()
        && let Some(collider) = spec.collider
    {
        crate::frame_profile_start!(collider_started);
        entity_commands.insert((RigidBody::Static, collider_from_stream(collider)));
        crate::frame_profile_elapsed!(
            frame_profiler,
            collider_started,
            "Update",
            "receive_world_stream",
            "apply_world_stream_chunk",
            "spawn_streamed_entity",
            "insert_collider",
        );
    }

    entity_commands.id()
}

fn mesh_from_primitive(primitive: WorldPrimitive) -> Mesh {
    match primitive {
        WorldPrimitive::Plane { size } => {
            let size = vec3_from_quantized(size);
            Plane3d::default().mesh().size(size.x, size.z).build()
        }
        WorldPrimitive::Cuboid { size } => {
            let size = vec3_from_quantized(size);
            Cuboid::new(size.x, size.y, size.z).mesh().build()
        }
    }
}

fn collider_from_stream(collider: WorldCollider) -> Collider {
    match collider {
        WorldCollider::Cuboid { size } => {
            let size = vec3_from_quantized(size);
            Collider::cuboid(size.x, size.y, size.z)
        }
    }
}

fn add_scene_mesh_assets(
    meshes: &mut Assets<Mesh>,
    meshlet_meshes: &mut Assets<MeshletMesh>,
    mesh: Mesh,
    name: &str,
    render_config: &ClientRenderConfig,
    _log_config: &ClientLogConfig,
) -> (Option<Handle<Mesh>>, Option<Handle<MeshletMesh>>) {
    let _vertex_count = mesh.count_vertices();
    let meshlet_handle = if render_config.meshlets_enabled {
        let meshlet_mesh =
            MeshletMesh::from_mesh(&mesh, MESHLET_DEFAULT_VERTEX_POSITION_QUANTIZATION_FACTOR)
                .unwrap_or_else(|error| panic!("failed to build {name} meshlet mesh: {error}"));
        Some(meshlet_meshes.add(meshlet_mesh))
    } else {
        None
    };
    let raytracing_handle = if render_config.solari_enabled || !render_config.meshlets_enabled {
        let raytracing_mesh = mesh.with_generated_tangents().unwrap_or_else(|error| {
            panic!("failed to generate {name} raytracing tangents: {error}")
        });
        Some(meshes.add(raytracing_mesh))
    } else {
        None
    };

    game_shared::fun_diag_debug_if!(
        _log_config.render_verbose(),
        target: "fun::render::mesh",
        name,
        vertices = _vertex_count,
        meshlets_enabled = render_config.meshlets_enabled,
        solari_enabled = render_config.solari_enabled,
        raytracing_handle = ?raytracing_handle,
        meshlet_handle = ?meshlet_handle,
        "built streamed mesh assets"
    );

    (raytracing_handle, meshlet_handle)
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "diagnostics collection intentionally reads many Bevy resources and query shapes without hiding scheduler access"
)]
fn log_client_diagnostics(
    time: Res<Time>,
    mut diagnostics: ResMut<ClientDiagnostics>,
    render_diagnostics: Res<DiagnosticsStore>,
    render_config: Res<ClientRenderConfig>,
    mut solari_runtime_params: ResMut<SolariRuntimeParams>,
    mut perf_counters: ResMut<ClientPerfCounters>,
    mut schedule_profiler: ResMut<ClientScheduleProfiler>,
    mut frame_profiler: ResMut<DetailedFrameProfiler>,
    log_config: Res<ClientLogConfig>,
    catalog: Option<Res<WorldRenderCatalog>>,
    render_recovery: Option<Res<RenderRecoveryStatus>>,
    loaded_world: Res<LoadedWorldState>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<
        (
            Entity,
            Option<&Name>,
            &GlobalTransform,
            Option<&Camera>,
            Option<&Projection>,
            Option<&SolariLighting>,
        ),
        With<Camera3d>,
    >,
    renderables: Query<
        (
            Entity,
            Option<&Name>,
            Option<&GlobalTransform>,
            Option<&Transform>,
            Option<&MeshletMesh3d>,
            Option<&RaytracingMesh3d>,
            Option<&Mesh3d>,
            Option<&MeshMaterial3d<StandardMaterial>>,
            Option<&Collider>,
            Option<&RenderGeometryClass>,
        ),
        Or<(
            With<MeshletMesh3d>,
            With<RaytracingMesh3d>,
            With<Mesh3d>,
            With<Collider>,
            With<RenderGeometryClass>,
        )>,
    >,
) {
    crate::frame_profile_start!(system_started);
    if !diagnostics.timer.tick(time.delta()).just_finished() {
        let log_client_ns = system_started
            .elapsed()
            .as_nanos()
            .min(u128::from(u64::MAX)) as u64;
        frame_profiler.record_diagnostics_ns(&["log_client_diagnostics_ns"], log_client_ns);
        crate::frame_profile_elapsed!(
            frame_profiler,
            system_started,
            "Update",
            "log_client_diagnostics",
            "timer_tick",
        );
        return;
    }
    crate::frame_profile_start!(diagnostics_started);

    let mut renderable_count = 0usize;
    let mut meshlet_count = 0usize;
    let mut raytracing_count = 0usize;
    let mut mesh3d_count = 0usize;
    let mut material_count = 0usize;
    let mut collider_count = 0usize;
    let mut simple_raster_count = 0usize;
    let mut meshlet_static_dense_count = 0usize;
    let mut meshlet_dynamic_dense_count = 0usize;
    let mut ray_proxy_only_count = 0usize;
    let mut viewmodel_count = 0usize;
    let mut samples = Vec::new();

    for (
        entity,
        name,
        global_transform,
        transform,
        meshlet,
        raytracing,
        mesh3d,
        material,
        collider,
        geometry_class,
    ) in &renderables
    {
        renderable_count += 1;
        meshlet_count += usize::from(meshlet.is_some());
        raytracing_count += usize::from(raytracing.is_some());
        mesh3d_count += usize::from(mesh3d.is_some());
        material_count += usize::from(material.is_some());
        collider_count += usize::from(collider.is_some());
        match geometry_class {
            Some(RenderGeometryClass::SimpleRaster) => simple_raster_count += 1,
            Some(RenderGeometryClass::MeshletStaticDense) => meshlet_static_dense_count += 1,
            Some(RenderGeometryClass::MeshletDynamicDense) => meshlet_dynamic_dense_count += 1,
            Some(RenderGeometryClass::RayProxyOnly) => ray_proxy_only_count += 1,
            Some(RenderGeometryClass::Viewmodel) => viewmodel_count += 1,
            None => {}
        }

        if samples.len() < 6 {
            let translation = global_transform
                .map(|transform| transform.translation())
                .or_else(|| transform.map(|transform| transform.translation))
                .unwrap_or(Vec3::NAN);
            samples.push(format!(
                "{:?}/{} pos=({:.2},{:.2},{:.2}) meshlet={} ray={} mesh3d={} mat={} collider={} class={:?}",
                entity,
                name.map(|name| name.as_str()).unwrap_or("<unnamed>"),
                translation.x,
                translation.y,
                translation.z,
                meshlet.is_some(),
                raytracing.is_some(),
                mesh3d.is_some(),
                material.is_some(),
                collider.is_some(),
                geometry_class,
            ));
        }
    }

    let camera_count = cameras.iter().count();
    let active_camera_count = cameras
        .iter()
        .filter(|(_, _, _, camera, _, _)| camera.is_none_or(|camera| camera.is_active))
        .count();
    if camera_count != 1 || active_camera_count != 1 {
        game_shared::fun_diag_warn!(
            target: "fun::camera",
            cameras = camera_count,
            active_cameras = active_camera_count,
            "expected exactly one active 3D camera"
        );
    }

    if log_config.diagnostics_verbose() {
        game_shared::fun_diag_info!(
            target: "fun::diag",
            level = ?loaded_world.level_id,
            revision = ?loaded_world.revision.map(|revision| revision.0),
            chunks_received = loaded_world.received_chunks.len(),
            chunks_expected = loaded_world.expected_chunks,
            spawned_entities = loaded_world.spawned_entities.len(),
            ack_sent = loaded_world.ack_sent,
            cameras = camera_count,
            active_cameras = active_camera_count,
            renderables = renderable_count,
            meshlets = meshlet_count,
            raytracing = raytracing_count,
            mesh3d = mesh3d_count,
            materials = material_count,
            colliders = collider_count,
            catalog_assets = catalog.as_ref().map(|catalog| catalog.len()).unwrap_or_default(),
            simple_raster = simple_raster_count,
            meshlet_static_dense = meshlet_static_dense_count,
            meshlet_dynamic_dense = meshlet_dynamic_dense_count,
            ray_proxy_only = ray_proxy_only_count,
            viewmodel = viewmodel_count,
            "client world diagnostic snapshot"
        );
        game_shared::fun_diag_info!(
            target: "fun::render_catalog",
            catalog_assets = catalog.as_ref().map(|catalog| catalog.len()).unwrap_or_default(),
            simple_raster = simple_raster_count,
            meshlet_static_dense = meshlet_static_dense_count,
            meshlet_dynamic_dense = meshlet_dynamic_dense_count,
            ray_proxy_only = ray_proxy_only_count,
            viewmodel = viewmodel_count,
            "client render catalog usage snapshot"
        );
    }
    perf_counters.set_render_path_counts(
        (meshlet_static_dense_count + meshlet_dynamic_dense_count) as u64,
        (simple_raster_count + viewmodel_count) as u64,
        ray_proxy_only_count as u64,
    );

    if log_config.diagnostics_verbose() {
        for (entity, name, transform, camera, projection, solari) in &cameras {
            let translation = transform.translation();
            game_shared::fun_diag_info!(
                target: "fun::diag::camera",
                entity = ?entity,
                name = name.map(|name| name.as_str()).unwrap_or("<unnamed>"),
                active = camera.is_none_or(|camera| camera.is_active),
                projection = projection.map(projection_summary).unwrap_or("<none>"),
                solari = solari.is_some(),
                position_x = translation.x,
                position_y = translation.y,
                position_z = translation.z,
                forward_x = transform.forward().x,
                forward_y = transform.forward().y,
                forward_z = transform.forward().z,
                "client camera diagnostic sample"
            );
        }

        for sample in samples {
            game_shared::fun_diag_debug!(target: "fun::diag::renderable", %sample, "renderable diagnostic sample");
        }
    }

    schedule_profiler.record_elapsed(
        ClientScheduleSystem::DiagnosticsLogging,
        diagnostics_started,
    );
    crate::frame_profile_start!(render_perf_started);
    log_render_performance(
        &render_diagnostics,
        render_recovery.as_deref(),
        solari_runtime_params.as_mut(),
        perf_counters.as_mut(),
        schedule_profiler.as_mut(),
        render_config.render_profile_verbose,
        primary_window
            .single()
            .ok()
            .map(ClientWindowProfile::from_window),
    );
    let render_perf_ns = render_perf_started
        .elapsed()
        .as_nanos()
        .min(u128::from(u64::MAX)) as u64;
    frame_profiler.record_diagnostics_ns(&["log_render_performance_ns"], render_perf_ns);
    crate::frame_profile_ns!(
        frame_profiler,
        render_perf_ns,
        "Update",
        "log_client_diagnostics",
        "log_render_performance",
    );
    let log_client_ns = system_started
        .elapsed()
        .as_nanos()
        .min(u128::from(u64::MAX)) as u64;
    frame_profiler.record_diagnostics_ns(&["log_client_diagnostics_ns"], log_client_ns);
    crate::frame_profile_ns!(
        frame_profiler,
        log_client_ns,
        "Update",
        "log_client_diagnostics",
    );
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
fn projection_summary(projection: &Projection) -> &'static str {
    match projection {
        Projection::Perspective(_) => "perspective",
        Projection::Orthographic(_) => "orthographic",
        Projection::Custom(_) => "custom",
    }
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
#[derive(Debug, Clone, Copy)]
struct ClientWindowProfile {
    logical_width: f64,
    logical_height: f64,
    physical_width: u32,
    physical_height: u32,
    scale_factor: f64,
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
impl ClientWindowProfile {
    fn from_window(window: &Window) -> Self {
        Self {
            logical_width: f64::from(window.resolution.width()),
            logical_height: f64::from(window.resolution.height()),
            physical_width: window.resolution.physical_width(),
            physical_height: window.resolution.physical_height(),
            scale_factor: f64::from(window.resolution.scale_factor()),
        }
    }

    fn physical_pixels(self) -> u64 {
        u64::from(self.physical_width) * u64::from(self.physical_height)
    }
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
#[derive(Debug, Clone)]
struct RenderProfileMetric {
    path: String,
    group: String,
    kind: &'static str,
    current: f64,
    average: Option<f64>,
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
fn log_render_performance(
    diagnostics: &DiagnosticsStore,
    render_recovery: Option<&RenderRecoveryStatus>,
    solari_runtime_params: &mut SolariRuntimeParams,
    perf_counters: &mut ClientPerfCounters,
    schedule_profiler: &mut ClientScheduleProfiler,
    verbose_profile: bool,
    window: Option<ClientWindowProfile>,
) {
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|diagnostic| diagnostic.value());
    let frame_ms = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|diagnostic| diagnostic.value());
    let app_frame_index =
        diagnostic_value_path(diagnostics, &FrameTimeDiagnosticsPlugin::FRAME_COUNT)
            .map(|value| value as u64);
    let render_frame_index =
        diagnostic_value(diagnostics, RENDER_FRAME_INDEX_PATH).map(|value| value as u64);
    let gpu_query_frame_index =
        diagnostic_value(diagnostics, GPU_QUERY_FRAME_INDEX_PATH).map(|value| value as u64);
    let sample_latency_frames =
        diagnostic_value(diagnostics, SAMPLE_LATENCY_FRAMES_PATH).map(|value| value as u64);
    let raw_gpu_sample_status = diagnostic_value(diagnostics, GPU_SAMPLE_STATUS_CODE_PATH)
        .and_then(gpu_sample_status_from_code);
    let gpu_sample_status = classify_gpu_sample_status(
        raw_gpu_sample_status,
        app_frame_index,
        render_frame_index,
        render_recovery,
        perf_counters,
    );

    let solari_presample = diagnostic_average(
        diagnostics,
        "render/solari_lighting/presample_light_tiles/elapsed_gpu",
    );
    let solari_surface_classify = diagnostic_average(
        diagnostics,
        "render/solari_lighting/surface_classify/elapsed_gpu",
    );
    let solari_work_queue =
        diagnostic_average(diagnostics, "render/solari_lighting/work_queue/elapsed_gpu");
    let solari_world_cache = diagnostic_average(
        diagnostics,
        "render/solari_lighting/world_cache/elapsed_gpu",
    );
    let solari_world_cache_active_cells = diagnostic_average(
        diagnostics,
        "render/solari_lighting/world_cache_active_cells_count",
    );
    let solari_direct = diagnostic_average(
        diagnostics,
        "render/solari_lighting/direct_lighting/elapsed_gpu",
    );
    let solari_diffuse_combined = diagnostic_average(
        diagnostics,
        "render/solari_lighting/diffuse_indirect_lighting/elapsed_gpu",
    );
    let solari_diffuse_initial = diagnostic_average(
        diagnostics,
        "render/solari_lighting/diffuse_indirect_lighting_initial/elapsed_gpu",
    );
    let solari_diffuse_spatial = diagnostic_average(
        diagnostics,
        "render/solari_lighting/diffuse_indirect_lighting_spatial/elapsed_gpu",
    );
    let solari_diffuse =
        solari_diffuse_combined.or_else(|| Some(solari_diffuse_initial? + solari_diffuse_spatial?));
    let solari_dlss_rr_guide_resolve = diagnostic_average(
        diagnostics,
        "render/solari_lighting/dlss_rr_guide_resolve/elapsed_gpu",
    );
    let solari_specular_regular = diagnostic_average(
        diagnostics,
        "render/solari_lighting/specular_indirect_lighting_regular/elapsed_gpu",
    );
    let solari_specular_queued = diagnostic_average(
        diagnostics,
        "render/solari_lighting/specular_indirect_lighting_queued/elapsed_gpu",
    );
    let solari_specular_psr = diagnostic_average(
        diagnostics,
        "render/solari_lighting/specular_indirect_lighting_psr/elapsed_gpu",
    );
    let solari_denoise_cheap = diagnostic_average(
        diagnostics,
        "render/solari_lighting/denoise_cheap_temporal/elapsed_gpu",
    );
    let solari_denoise_composite = diagnostic_average(
        diagnostics,
        "render/solari_lighting/denoise_composite/elapsed_gpu",
    );
    let solari_denoise_atrous_1 = diagnostic_average(
        diagnostics,
        "render/solari_lighting/denoise_atrous_step_1/elapsed_gpu",
    );
    let solari_denoise_atrous_2 = diagnostic_average(
        diagnostics,
        "render/solari_lighting/denoise_atrous_step_2/elapsed_gpu",
    );
    let solari_denoise_atrous_3 = diagnostic_average(
        diagnostics,
        "render/solari_lighting/denoise_atrous_step_3/elapsed_gpu",
    );
    let solari_specular = solari_specular_regular
        .or(solari_specular_queued)
        .or(solari_specular_psr);
    let solari_passes = [
        solari_presample,
        solari_surface_classify,
        solari_work_queue,
        solari_world_cache,
        solari_direct,
        solari_diffuse,
        solari_dlss_rr_guide_resolve,
        solari_specular,
        solari_denoise_cheap,
        solari_denoise_atrous_1,
        solari_denoise_atrous_2,
        solari_denoise_atrous_3,
        solari_denoise_composite,
    ];
    let solari_total = solari_passes
        .into_iter()
        .flatten()
        .fold(None, |total: Option<f64>, value| {
            Some(total.unwrap_or_default() + value)
        });
    let solari_denoise_total = sum_optional_ms([
        solari_denoise_cheap,
        solari_denoise_atrous_1,
        solari_denoise_atrous_2,
        solari_denoise_atrous_3,
        solari_denoise_composite,
    ]);
    let solari_runtime_update_started = Instant::now();
    solari_runtime_params.previous_solari_ns = ms_to_u32_ns(solari_total);
    solari_runtime_params.previous_direct_ns = ms_to_u32_ns(solari_direct);
    solari_runtime_params.previous_diffuse_gi_ns = ms_to_u32_ns(solari_diffuse);
    solari_runtime_params.previous_specular_ns = ms_to_u32_ns(solari_specular);
    solari_runtime_params.previous_radiance_cache_ns = ms_to_u32_ns(solari_world_cache);
    solari_runtime_params.previous_denoise_ns = ms_to_u32_ns(solari_denoise_total);
    schedule_profiler.record_elapsed(
        ClientScheduleSystem::SolariRuntimeParamsUpdate,
        solari_runtime_update_started,
    );
    let meshlet_visibility = diagnostic_average(
        diagnostics,
        "render/meshlet_visibility_buffer_raster/elapsed_gpu",
    );
    let meshlet_visibility_first_pass = diagnostic_average(
        diagnostics,
        "render/meshlet_visibility_buffer_raster/first_pass/elapsed_gpu",
    );
    let meshlet_visibility_depth_pyramid_1 = diagnostic_average(
        diagnostics,
        "render/meshlet_visibility_buffer_raster/depth_pyramid_first/elapsed_gpu",
    );
    let meshlet_visibility_second_pass = diagnostic_average(
        diagnostics,
        "render/meshlet_visibility_buffer_raster/second_pass/elapsed_gpu",
    );
    let meshlet_visibility_depth_resolve = diagnostic_average(
        diagnostics,
        "render/meshlet_visibility_buffer_raster/depth_resolve/elapsed_gpu",
    );
    let meshlet_visibility_material_depth = diagnostic_average(
        diagnostics,
        "render/meshlet_visibility_buffer_raster/material_depth_resolve/elapsed_gpu",
    );
    let meshlet_visibility_depth_pyramid_2 = diagnostic_average(
        diagnostics,
        "render/meshlet_visibility_buffer_raster/depth_pyramid_second/elapsed_gpu",
    );
    let meshlet_path_instance_count =
        diagnostic_average(diagnostics, "meshlet_path_instance_count")
            .map(|value| value as u64)
            .unwrap_or(perf_counters.meshlet_path_instance_count);
    let meshlet_extract_cpu_ns =
        diagnostic_average(diagnostics, "meshlet_extract_cpu_ns").map(ms_to_ns_from_value);
    if let Some(meshlet_extract_cpu_ns) = meshlet_extract_cpu_ns {
        schedule_profiler.record_ns(
            ClientScheduleSystem::MeshletExtraction,
            meshlet_extract_cpu_ns,
        );
    }
    let meshlet_prepare_cpu_ns =
        diagnostic_average(diagnostics, "meshlet_prepare_cpu_ns").map(ms_to_ns_from_value);
    let meshlet_bind_group_prepare_cpu_ns =
        diagnostic_average(diagnostics, "meshlet_bind_group_prepare_cpu_ns")
            .map(ms_to_ns_from_value);
    let meshlet_material_queue_cpu_ns =
        diagnostic_average(diagnostics, "meshlet_material_queue_cpu_ns").map(ms_to_ns_from_value);
    let meshlet_material_queue_dirty_instance_count =
        diagnostic_average(diagnostics, "meshlet_material_queue_dirty_instance_count")
            .map(|value| value as u64);
    let meshlet_instance_full_buffer_writes =
        diagnostic_average(diagnostics, "meshlet_instance_full_buffer_writes")
            .map(|value| value as u64);
    let meshlet_instance_range_buffer_writes =
        diagnostic_average(diagnostics, "meshlet_instance_range_buffer_writes")
            .map(|value| value as u64);
    let meshlet_material_full_buffer_writes =
        diagnostic_average(diagnostics, "meshlet_material_full_buffer_writes")
            .map(|value| value as u64);
    let meshlet_material_range_buffer_writes =
        diagnostic_average(diagnostics, "meshlet_material_range_buffer_writes")
            .map(|value| value as u64);
    let meshlet_view_visibility_buffer_writes =
        diagnostic_average(diagnostics, "meshlet_view_visibility_buffer_writes")
            .map(|value| value as u64);
    let meshlet_view_reset_cpu_queue_writes =
        diagnostic_average(diagnostics, "meshlet_view_reset_cpu_queue_writes")
            .map(|value| value as u64);
    let meshlet_view_reset_cpu_queue_writes_per_view =
        diagnostic_average(diagnostics, "meshlet_view_reset_cpu_queue_writes_per_view");
    let meshlet_view_count =
        diagnostic_average(diagnostics, "meshlet_view_count").map(|value| value as u64);
    let material_bind_group_reused_count =
        diagnostic_average(diagnostics, "render/cpu/material_bind_group_reused_count")
            .map(|value| value as u64);
    let material_bind_group_recreated_count = diagnostic_average(
        diagnostics,
        "render/cpu/material_bind_group_recreated_count",
    )
    .map(|value| value as u64);
    let material_buffer_only_update_count =
        diagnostic_average(diagnostics, "render/cpu/material_buffer_only_update_count")
            .map(|value| value as u64);
    let material_bind_group_recreate_ns =
        diagnostic_average(diagnostics, "render/cpu/material_bind_group_recreate_ns")
            .map(|value| value as u64);
    let material_buffer_update_ns =
        diagnostic_average(diagnostics, "render/cpu/material_buffer_update_ns")
            .map(|value| value as u64);
    let dlss_rr = diagnostic_average(diagnostics, "render/dlss_ray_reconstruction/elapsed_gpu");
    let standard_raster = diagnostic_average(diagnostics, "render/main_opaque_pass_3d/elapsed_gpu");
    let post_process = sum_optional_ms([
        diagnostic_average(diagnostics, "render/tonemapping/elapsed_gpu"),
        diagnostic_average(diagnostics, "render/upscaling/elapsed_gpu"),
        diagnostic_average(diagnostics, "render/bloom/elapsed_gpu"),
        diagnostic_average(diagnostics, "render/anti_aliasing/elapsed_gpu"),
    ]);
    let ui_overlay_cpu_ns =
        diagnostic_average(diagnostics, "render/ui/elapsed_cpu").map(ms_to_ns_from_value);
    let present_wait_ns =
        diagnostic_average(diagnostics, "render/present/elapsed_cpu").map(ms_to_ns_from_value);
    let physics_fixed_update_cpu_ns =
        diagnostic_average(diagnostics, "physics/fixed_update/elapsed_cpu")
            .map(ms_to_ns_from_value);
    let network_receive_cpu_ns = take_counter(&mut perf_counters.network_receive_cpu_ns);
    let world_stream_apply_cpu_ns = take_counter(&mut perf_counters.world_stream_apply_cpu_ns);
    let catalog_lookup_cpu_ns = take_counter(&mut perf_counters.catalog_lookup_cpu_ns);
    let raster_path_instance_count = perf_counters.raster_path_instance_count;
    let ray_proxy_only_count = perf_counters.ray_proxy_only_count;
    let pixel_count = window.map(ClientWindowProfile::physical_pixels);
    let mpixels = pixel_count.map(|pixels| pixels as f64 / 1_000_000.0);
    let configured_budget_pressure =
        diagnostic_average(diagnostics, "render/solari_lighting/solari_budget_pressure");
    let measured_budget_pressure = solari_total
        .map(|total_ms| (total_ms * 1_000_000.0) / solari_runtime_params.gpu_budget_ns as f64);
    if let Some(pressure) = measured_budget_pressure {
        solari_runtime_params.budget_pressure = pressure.max(0.0) as f32;
        let debt = (pressure - 1.0).max(0.0) as f32;
        solari_runtime_params.visual_debt_mean = debt;
        solari_runtime_params.visual_debt_p95 = debt;
    }
    let solari_budget_pressure = measured_budget_pressure.or(configured_budget_pressure);
    let solari_quality_level =
        diagnostic_average(diagnostics, "render/solari_lighting/solari_quality_level")
            .unwrap_or(solari_runtime_params.quality_level as f64);
    let solari_direct_active_pixels = diagnostic_average(
        diagnostics,
        "render/solari_lighting/solari_direct_active_pixels",
    )
    .or_else(|| pixel_count.map(|pixels| pixels as f64));
    let solari_gi_active_tiles =
        diagnostic_average(diagnostics, "render/solari_lighting/solari_gi_active_tiles");
    let solari_specular_active_pixels = diagnostic_average(
        diagnostics,
        "render/solari_lighting/solari_specular_active_pixels",
    )
    .or_else(|| pixel_count.map(|pixels| pixels as f64));
    let solari_cache_requests =
        diagnostic_average(diagnostics, "render/solari_lighting/solari_cache_requests")
            .unwrap_or(solari_runtime_params.cache_update_budget as f64);
    let solari_cache_hit_rate =
        diagnostic_average(diagnostics, "render/solari_lighting/solari_cache_hit_rate");
    let solari_visual_debt_mean = measured_budget_pressure
        .map(|pressure| (pressure - 1.0).max(0.0))
        .or_else(|| {
            diagnostic_average(
                diagnostics,
                "render/solari_lighting/solari_visual_debt_mean",
            )
        });
    let solari_visual_debt_p95 = solari_visual_debt_mean.or_else(|| {
        diagnostic_average(diagnostics, "render/solari_lighting/solari_visual_debt_p95")
    });
    let solari_reconstruction_pixels = diagnostic_average(
        diagnostics,
        "render/solari_lighting/solari_reconstruction_pixels",
    );
    let solari_work_queue_direct = diagnostic_value(
        diagnostics,
        "render/solari_lighting/work_queue_direct_critical_pixels",
    );
    let solari_work_queue_gi_tiles =
        diagnostic_value(diagnostics, "render/solari_lighting/work_queue_gi_tiles");
    let solari_work_queue_gi_repair = diagnostic_value(
        diagnostics,
        "render/solari_lighting/work_queue_gi_repair_pixels",
    );
    let solari_work_queue_specular = diagnostic_value(
        diagnostics,
        "render/solari_lighting/work_queue_specular_pixels",
    );
    let solari_work_queue_cache = diagnostic_value(
        diagnostics,
        "render/solari_lighting/work_queue_cache_requests",
    );
    let solari_work_queue_denoise = diagnostic_value(
        diagnostics,
        "render/solari_lighting/work_queue_denoise_repair_tiles",
    );
    let solari_work_queue_overflow =
        diagnostic_value(diagnostics, "render/solari_lighting/work_queue_overflow");
    let solari_work_queue_active_tiles = diagnostic_value(
        diagnostics,
        "render/solari_lighting/work_queue_active_tiles",
    );
    let radiance_cache_requests = diagnostic_value(
        diagnostics,
        "render/solari_lighting/radiance_cache_requests",
    );
    let radiance_cache_hits =
        diagnostic_value(diagnostics, "render/solari_lighting/radiance_cache_hits");
    let radiance_cache_misses =
        diagnostic_value(diagnostics, "render/solari_lighting/radiance_cache_misses");
    let radiance_cache_overflow = diagnostic_value(
        diagnostics,
        "render/solari_lighting/radiance_cache_overflow",
    );
    let radiance_cache_hit_rate = match (radiance_cache_hits, radiance_cache_misses) {
        (Some(hits), Some(misses)) if hits + misses > 0.0 => Some(hits / (hits + misses)),
        _ => None,
    };

    game_shared::fun_diag_info!(
        target: "fun::perf",
        fps = ?fps,
        frame_ms = ?frame_ms,
        frame_ns = ?ms_to_ns(frame_ms),
        solari_gpu_ms = ?solari_total,
        solari_gpu_ns = ?ms_to_ns(solari_total),
        meshlet_visibility_gpu_ms = ?meshlet_visibility,
        meshlet_visibility_gpu_ns = ?ms_to_ns(meshlet_visibility),
        dlss_rr_gpu_ms = ?dlss_rr,
        dlss_rr_gpu_ns = ?ms_to_ns(dlss_rr),
        gpu_sample_status,
        app_frame_index = ?app_frame_index,
        render_frame_index = ?render_frame_index,
        gpu_query_frame_index = ?gpu_query_frame_index,
        sample_latency_frames = ?sample_latency_frames,
        render_pixels = ?pixel_count,
        render_mpix = ?mpixels,
        "client render performance sample"
    );
    game_shared::fun_diag_info!(
        target: "fun::perf::non_solari",
        meshlet_visibility_gpu_ms = ?meshlet_visibility,
        meshlet_visibility_gpu_ns = ?ms_to_ns(meshlet_visibility),
        meshlet_first_pass_gpu_ns = ?ms_to_ns(meshlet_visibility_first_pass),
        meshlet_depth_pyramid_first_gpu_ns = ?ms_to_ns(meshlet_visibility_depth_pyramid_1),
        meshlet_second_pass_gpu_ns = ?ms_to_ns(meshlet_visibility_second_pass),
        meshlet_depth_resolve_gpu_ns = ?ms_to_ns(meshlet_visibility_depth_resolve),
        meshlet_material_depth_gpu_ns = ?ms_to_ns(meshlet_visibility_material_depth),
        meshlet_depth_pyramid_second_gpu_ns = ?ms_to_ns(meshlet_visibility_depth_pyramid_2),
        post_process_gpu_ms = ?post_process,
        post_process_gpu_ns = ?ms_to_ns(post_process),
        meshlet_extract_cpu_ns = ?meshlet_extract_cpu_ns,
        meshlet_prepare_cpu_ns = ?meshlet_prepare_cpu_ns,
        meshlet_bind_group_prepare_cpu_ns = ?meshlet_bind_group_prepare_cpu_ns,
        meshlet_material_queue_cpu_ns = ?meshlet_material_queue_cpu_ns,
        meshlet_material_queue_dirty_instance_count = ?meshlet_material_queue_dirty_instance_count,
        meshlet_instance_full_buffer_writes = ?meshlet_instance_full_buffer_writes,
        meshlet_instance_range_buffer_writes = ?meshlet_instance_range_buffer_writes,
        meshlet_material_full_buffer_writes = ?meshlet_material_full_buffer_writes,
        meshlet_material_range_buffer_writes = ?meshlet_material_range_buffer_writes,
        meshlet_view_visibility_buffer_writes = ?meshlet_view_visibility_buffer_writes,
        meshlet_view_reset_cpu_queue_writes = ?meshlet_view_reset_cpu_queue_writes,
        meshlet_view_reset_cpu_queue_writes_per_view = ?meshlet_view_reset_cpu_queue_writes_per_view,
        meshlet_view_count = ?meshlet_view_count,
        render_cpu_material_bind_group_reused_count = ?material_bind_group_reused_count,
        render_cpu_material_bind_group_recreated_count = ?material_bind_group_recreated_count,
        render_cpu_material_buffer_only_update_count = ?material_buffer_only_update_count,
        render_cpu_material_bind_group_recreate_ns = ?material_bind_group_recreate_ns,
        render_cpu_material_buffer_update_ns = ?material_buffer_update_ns,
        meshlet_path_instance_count,
        raster_path_instance_count,
        ray_proxy_only_count,
        standard_raster_gpu_ms = ?standard_raster,
        standard_raster_gpu_ns = ?ms_to_ns(standard_raster),
        physics_fixed_update_cpu_ns = ?physics_fixed_update_cpu_ns,
        network_receive_cpu_ns,
        world_stream_apply_cpu_ns,
        catalog_lookup_cpu_ns,
        ui_overlay_cpu_ns = ?ui_overlay_cpu_ns,
        present_wait_ns = ?present_wait_ns,
        "non-Solari performance budget sample"
    );
    game_shared::fun_diag_info!(
        "[client perf] fps={} frame_ms={} solari_gpu_ms={} meshlet_visibility_gpu_ms={} dlss_rr_gpu_ms={}",
        format_optional_number(fps),
        format_optional_number(frame_ms),
        format_optional_number(solari_total),
        format_optional_number(meshlet_visibility),
        format_optional_number(dlss_rr),
    );
    game_shared::fun_diag_info!(
        "[client perf] gpu_sample_status={} app_frame_index={} render_frame_index={} gpu_query_frame_index={} sample_latency_frames={}",
        gpu_sample_status,
        format_optional_u64(app_frame_index),
        format_optional_u64(render_frame_index),
        format_optional_u64(gpu_query_frame_index),
        format_optional_u64(sample_latency_frames),
    );
    game_shared::fun_diag_info!(
        "[client perf] non_solari gpu_ms: meshlet_visibility_gpu_ms={} meshlet_first_pass_gpu_ms={} meshlet_depth_pyramid_first_gpu_ms={} meshlet_second_pass_gpu_ms={} meshlet_depth_resolve_gpu_ms={} meshlet_material_depth_gpu_ms={} meshlet_depth_pyramid_second_gpu_ms={} standard_raster_gpu_ms={} post_process_gpu_ms={}",
        format_optional_number(meshlet_visibility),
        format_optional_number(meshlet_visibility_first_pass),
        format_optional_number(meshlet_visibility_depth_pyramid_1),
        format_optional_number(meshlet_visibility_second_pass),
        format_optional_number(meshlet_visibility_depth_resolve),
        format_optional_number(meshlet_visibility_material_depth),
        format_optional_number(meshlet_visibility_depth_pyramid_2),
        format_optional_number(standard_raster),
        format_optional_number(post_process),
    );
    game_shared::fun_diag_info!(
        "[client perf] render paths: meshlet_path_instance_count={} raster_path_instance_count={} ray_proxy_only_count={}",
        meshlet_path_instance_count,
        raster_path_instance_count,
        ray_proxy_only_count,
    );
    game_shared::fun_diag_info!(
        "[client perf] non_solari cpu_ns: meshlet_extract_cpu_ns={} meshlet_prepare_cpu_ns={} meshlet_bind_group_prepare_cpu_ns={} meshlet_material_queue_cpu_ns={} meshlet_material_queue_dirty_instance_count={} physics_fixed_update_cpu_ns={} network_receive_cpu_ns={} world_stream_apply_cpu_ns={} catalog_lookup_cpu_ns={} ui_overlay_cpu_ns={} present_wait_ns={}",
        format_optional_u64(meshlet_extract_cpu_ns),
        format_optional_u64(meshlet_prepare_cpu_ns),
        format_optional_u64(meshlet_bind_group_prepare_cpu_ns),
        format_optional_u64(meshlet_material_queue_cpu_ns),
        format_optional_u64(meshlet_material_queue_dirty_instance_count),
        format_optional_u64(physics_fixed_update_cpu_ns),
        network_receive_cpu_ns,
        world_stream_apply_cpu_ns,
        catalog_lookup_cpu_ns,
        format_optional_u64(ui_overlay_cpu_ns),
        format_optional_u64(present_wait_ns),
    );
    game_shared::fun_diag_info!(
        "[client perf] meshlet buffers: instance_full_buffer_writes={} instance_range_buffer_writes={} material_full_buffer_writes={} material_range_buffer_writes={} view_visibility_buffer_writes={} view_reset_cpu_queue_writes={} view_reset_cpu_queue_writes_per_view={} view_count={}",
        format_optional_u64(meshlet_instance_full_buffer_writes),
        format_optional_u64(meshlet_instance_range_buffer_writes),
        format_optional_u64(meshlet_material_full_buffer_writes),
        format_optional_u64(meshlet_material_range_buffer_writes),
        format_optional_u64(meshlet_view_visibility_buffer_writes),
        format_optional_u64(meshlet_view_reset_cpu_queue_writes),
        format_optional_number(meshlet_view_reset_cpu_queue_writes_per_view),
        format_optional_u64(meshlet_view_count),
    );
    game_shared::fun_diag_info!(
        target: "fun::perf::material",
        render_cpu_material_bind_group_reused_count = ?material_bind_group_reused_count,
        render_cpu_material_bind_group_recreated_count = ?material_bind_group_recreated_count,
        render_cpu_material_buffer_only_update_count = ?material_buffer_only_update_count,
        render_cpu_material_bind_group_recreate_ns = ?material_bind_group_recreate_ns,
        render_cpu_material_buffer_update_ns = ?material_buffer_update_ns,
        "material bind group performance sample"
    );
    game_shared::fun_diag_info!(
        "[client perf] material bind groups: reused={} recreated={} buffer_only_updates={} recreate_ns={} buffer_update_ns={}",
        format_optional_u64(material_bind_group_reused_count),
        format_optional_u64(material_bind_group_recreated_count),
        format_optional_u64(material_buffer_only_update_count),
        format_optional_u64(material_bind_group_recreate_ns),
        format_optional_u64(material_buffer_update_ns),
    );
    log_schedule_heatmap(schedule_profiler);

    if let Some(window) = window {
        game_shared::fun_diag_info!(
            target: "fun::perf::window",
            logical_width = window.logical_width,
            logical_height = window.logical_height,
            physical_width = window.physical_width,
            physical_height = window.physical_height,
            scale_factor = window.scale_factor,
            pixels = window.physical_pixels(),
            target_render_ms = target_frame_ms(DEFAULT_RENDER_TARGET_RATE_HZ),
            target_solari_ms = target_frame_ms(solari_runtime_params.target_fps as f64),
            "client window render target"
        );
    }

    let process_cpu = diagnostic_average_path(
        diagnostics,
        &bevy::diagnostic::SystemInformationDiagnosticsPlugin::PROCESS_CPU_USAGE,
    );
    let process_mem = diagnostic_average_path(
        diagnostics,
        &bevy::diagnostic::SystemInformationDiagnosticsPlugin::PROCESS_MEM_USAGE,
    );
    let system_cpu = diagnostic_average_path(
        diagnostics,
        &bevy::diagnostic::SystemInformationDiagnosticsPlugin::SYSTEM_CPU_USAGE,
    );
    let system_mem = diagnostic_average_path(
        diagnostics,
        &bevy::diagnostic::SystemInformationDiagnosticsPlugin::SYSTEM_MEM_USAGE,
    );
    game_shared::fun_diag_info!(
        "[client perf] process_cpu_pct={} process_mem_gib={} system_cpu_pct={} system_mem_pct={}",
        format_optional_number(process_cpu),
        format_optional_number(process_mem),
        format_optional_number(system_cpu),
        format_optional_number(system_mem),
    );
    game_shared::fun_diag_info!(
        target: "fun::perf::system",
        process_cpu_pct = ?process_cpu,
        process_mem_gib = ?process_mem,
        system_cpu_pct = ?system_cpu,
        system_mem_pct = ?system_mem,
        "client system performance sample"
    );

    game_shared::fun_diag_info!(
        target: "fun::perf::solari",
        presample_ns = ?ms_to_ns(solari_presample),
        surface_classify_ns = ?ms_to_ns(solari_surface_classify),
        work_queue_ns = ?ms_to_ns(solari_work_queue),
        world_cache_ns = ?ms_to_ns(solari_world_cache),
        world_cache_active_cells = ?solari_world_cache_active_cells,
        direct_ns = ?ms_to_ns(solari_direct),
        diffuse_ns = ?ms_to_ns(solari_diffuse),
        diffuse_initial_ns = ?ms_to_ns(solari_diffuse_initial),
        diffuse_spatial_ns = ?ms_to_ns(solari_diffuse_spatial),
        dlss_rr_guide_resolve_ns = ?ms_to_ns(solari_dlss_rr_guide_resolve),
        specular_regular_ns = ?ms_to_ns(solari_specular_regular),
        specular_queued_ns = ?ms_to_ns(solari_specular_queued),
        specular_psr_ns = ?ms_to_ns(solari_specular_psr),
        denoise_cheap_ns = ?ms_to_ns(solari_denoise_cheap),
        denoise_atrous_1_ns = ?ms_to_ns(solari_denoise_atrous_1),
        denoise_atrous_2_ns = ?ms_to_ns(solari_denoise_atrous_2),
        denoise_atrous_3_ns = ?ms_to_ns(solari_denoise_atrous_3),
        denoise_composite_ns = ?ms_to_ns(solari_denoise_composite),
        "Solari GPU pass timing sample"
    );
    game_shared::fun_diag_info!(
        "[client perf] solari passes gpu_ms: presample={} surface_classify={} work_queue={} world_cache={} direct={} diffuse={} diffuse_initial={} diffuse_spatial={} dlss_rr_guide_resolve={} specular_regular={} specular_queued={} specular_psr={} denoise_cheap={} denoise_atrous_1={} denoise_atrous_2={} denoise_atrous_3={} denoise_composite={}",
        format_optional_number(solari_presample),
        format_optional_number(solari_surface_classify),
        format_optional_number(solari_work_queue),
        format_optional_number(solari_world_cache),
        format_optional_number(solari_direct),
        format_optional_number(solari_diffuse),
        format_optional_number(solari_diffuse_initial),
        format_optional_number(solari_diffuse_spatial),
        format_optional_number(solari_dlss_rr_guide_resolve),
        format_optional_number(solari_specular_regular),
        format_optional_number(solari_specular_queued),
        format_optional_number(solari_specular_psr),
        format_optional_number(solari_denoise_cheap),
        format_optional_number(solari_denoise_atrous_1),
        format_optional_number(solari_denoise_atrous_2),
        format_optional_number(solari_denoise_atrous_3),
        format_optional_number(solari_denoise_composite),
    );
    game_shared::fun_diag_info!(
        "[client perf] solari budget: architecture={:?} visual_target={:?} target_fps={} frame_budget_ns={} gpu_budget_ns={} budget_pressure={} quality_level={} direct_active_pixels={} gi_active_tiles={} specular_active_pixels={} cache_requests={} cache_hit_rate={} visual_debt_mean={} visual_debt_p95={} reconstruction_pixels={}",
        solari_runtime_params.architecture,
        solari_runtime_params.visual_target,
        solari_runtime_params.target_fps,
        solari_runtime_params.frame_budget_ns,
        solari_runtime_params.gpu_budget_ns,
        format_optional_number(solari_budget_pressure),
        format_number(solari_quality_level),
        format_optional_number(solari_direct_active_pixels),
        format_optional_number(solari_gi_active_tiles),
        format_optional_number(solari_specular_active_pixels),
        format_number(solari_cache_requests),
        format_optional_number(solari_cache_hit_rate),
        format_optional_number(solari_visual_debt_mean),
        format_optional_number(solari_visual_debt_p95),
        format_optional_number(solari_reconstruction_pixels),
    );
    game_shared::fun_diag_info!(
        target: "fun::perf::solari_budget",
        architecture = ?solari_runtime_params.architecture,
        visual_target = ?solari_runtime_params.visual_target,
        target_fps = solari_runtime_params.target_fps,
        frame_budget_ns = solari_runtime_params.frame_budget_ns,
        gpu_budget_ns = solari_runtime_params.gpu_budget_ns,
        budget_pressure = ?solari_budget_pressure,
        quality_level = solari_quality_level,
        direct_active_pixels = ?solari_direct_active_pixels,
        gi_active_tiles = ?solari_gi_active_tiles,
        specular_active_pixels = ?solari_specular_active_pixels,
        cache_requests = solari_cache_requests,
        cache_hit_rate = ?solari_cache_hit_rate,
        visual_debt_mean = ?solari_visual_debt_mean,
        visual_debt_p95 = ?solari_visual_debt_p95,
        reconstruction_pixels = ?solari_reconstruction_pixels,
        "Solari budget diagnostic sample"
    );
    game_shared::fun_diag_info!(
        target: "fun::perf::solari_queues",
        direct_critical_pixels = ?solari_work_queue_direct,
        gi_tiles = ?solari_work_queue_gi_tiles,
        gi_repair_pixels = ?solari_work_queue_gi_repair,
        specular_pixels = ?solari_work_queue_specular,
        cache_requests = ?solari_work_queue_cache,
        denoise_repair_tiles = ?solari_work_queue_denoise,
        overflow = ?solari_work_queue_overflow,
        active_tiles = ?solari_work_queue_active_tiles,
        "Solari GPU work queue diagnostic sample"
    );
    game_shared::fun_diag_info!(
        "[client perf] solari queues: direct={} gi_tiles={} gi_repair={} specular={} cache={} denoise={} overflow={} active_tiles={}",
        format_optional_number(solari_work_queue_direct),
        format_optional_number(solari_work_queue_gi_tiles),
        format_optional_number(solari_work_queue_gi_repair),
        format_optional_number(solari_work_queue_specular),
        format_optional_number(solari_work_queue_cache),
        format_optional_number(solari_work_queue_denoise),
        format_optional_number(solari_work_queue_overflow),
        format_optional_number(solari_work_queue_active_tiles),
    );
    game_shared::fun_diag_info!(
        target: "fun::perf::radiance_cache",
        requests = ?radiance_cache_requests,
        hits = ?radiance_cache_hits,
        misses = ?radiance_cache_misses,
        hit_rate = ?radiance_cache_hit_rate,
        overflow = ?radiance_cache_overflow,
        "Solari radiance cache diagnostic sample"
    );
    game_shared::fun_diag_info!(
        "[client perf] radiance cache: requests={} hits={} misses={} hit_rate={} overflow={}",
        format_optional_number(radiance_cache_requests),
        format_optional_number(radiance_cache_hits),
        format_optional_number(radiance_cache_misses),
        format_optional_number(radiance_cache_hit_rate),
        format_optional_number(radiance_cache_overflow),
    );

    let mut render_timings = diagnostics
        .iter()
        .filter_map(|diagnostic| {
            let path = diagnostic.path().as_str();
            if path.starts_with("render/")
                && (path.ends_with("/elapsed_gpu") || path.ends_with("/elapsed_cpu"))
            {
                diagnostic.average().map(|value| (path.to_owned(), value))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    render_timings.sort_by(|(_, left), (_, right)| {
        right.partial_cmp(left).unwrap_or(std::cmp::Ordering::Equal)
    });

    if !render_timings.is_empty() {
        let top_timings = render_timings
            .into_iter()
            .take(8)
            .map(|(path, value)| format!("{path}={}", format_number(value)))
            .collect::<Vec<_>>()
            .join(", ");
        game_shared::fun_diag_info!("[client perf] top render timings {top_timings}");
        game_shared::fun_diag_info!(target: "fun::perf::top_render", %top_timings, "top render timings");
    }

    if verbose_profile {
        log_verbose_render_profile(diagnostics, window, frame_ms, solari_total);
    }

    if let Some(status) = render_recovery
        && (status.errors_seen > 0
            || status.surface_losses > 0
            || status.surface_validation_errors > 0
            || status.surface_timeouts > 0
            || status.recovery_attempts > 0
            || status.frames_without_rendering > 0)
    {
        game_shared::fun_diag_warn!(
            target: "fun::render::recovery",
            errors = status.errors_seen,
            surface_losses = status.surface_losses,
            surface_validation_errors = status.surface_validation_errors,
            surface_timeouts = status.surface_timeouts,
            attempts = status.recovery_attempts,
            successes = status.recovery_successes,
            failures = status.recovery_failures,
            frames_without_rendering = status.frames_without_rendering,
            last_type = ?status.last_error_type,
            last_reason = ?status.last_device_lost_reason,
            last_policy = ?status.last_policy,
            last_description = %status.last_error_description,
            "renderer recovery status"
        );
    }
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
fn log_schedule_heatmap(schedule_profiler: &mut ClientScheduleProfiler) {
    let mut reports = schedule_profiler.drain_reports();
    let total_ns = reports
        .iter()
        .map(|report| report.total_ns)
        .fold(0u64, u64::saturating_add);
    if reports.is_empty() {
        return;
    }

    let schedule_payload = reports
        .iter()
        .map(|report| format!("{}_ns={}", report.system.metric_name(), report.total_ns))
        .collect::<Vec<_>>()
        .join(" ");
    game_shared::fun_diag_info!("[client perf] schedule cpu_ns: {schedule_payload}");

    let schedule_max_payload = reports
        .iter()
        .map(|report| {
            format!(
                "{}_max_ns={} {}_count={}",
                report.system.metric_name(),
                report.max_ns,
                report.system.metric_name(),
                report.count
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    game_shared::fun_diag_info!("[client perf] schedule detail: {schedule_max_payload}");

    reports.sort_by_key(|report| std::cmp::Reverse(report.total_ns));
    let heatmap = reports
        .iter()
        .map(|report| {
            let pct = if total_ns > 0 {
                (report.total_ns as f64 / total_ns as f64) * 100.0
            } else {
                0.0
            };
            format!(
                "{}={}ns/{:.1}%",
                report.system.metric_name(),
                report.total_ns,
                pct
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    game_shared::fun_diag_info!("[client perf] schedule heatmap: total_ns={total_ns} {heatmap}");
    game_shared::fun_diag_info!(
        target: "fun::perf::schedule_heatmap",
        total_ns,
        heatmap = %heatmap,
        "client schedule heatmap"
    );
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
fn log_verbose_render_profile(
    diagnostics: &DiagnosticsStore,
    window: Option<ClientWindowProfile>,
    frame_ms: Option<f64>,
    solari_total: Option<f64>,
) {
    let mut metrics = collect_render_profile_metrics(diagnostics);
    metrics.sort_by(|left, right| {
        right
            .current
            .partial_cmp(&left.current)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut gpu_metrics = metrics
        .iter()
        .filter(|metric| metric.kind == "elapsed_gpu")
        .cloned()
        .collect::<Vec<_>>();
    gpu_metrics.sort_by(|left, right| {
        right
            .current
            .partial_cmp(&left.current)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut cpu_metrics = metrics
        .iter()
        .filter(|metric| metric.kind == "elapsed_cpu")
        .cloned()
        .collect::<Vec<_>>();
    cpu_metrics.sort_by(|left, right| {
        right
            .current
            .partial_cmp(&left.current)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut statistic_metrics = metrics
        .iter()
        .filter(|metric| metric.kind != "elapsed_gpu" && metric.kind != "elapsed_cpu")
        .cloned()
        .collect::<Vec<_>>();
    statistic_metrics.sort_by(|left, right| {
        right
            .current
            .partial_cmp(&left.current)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let gpu_total = gpu_metrics.iter().map(|metric| metric.current).sum::<f64>();
    let cpu_total = cpu_metrics.iter().map(|metric| metric.current).sum::<f64>();
    let pixel_count = window.map(ClientWindowProfile::physical_pixels);
    let target_120hz = target_frame_ms(120.0);
    let frame_budget_pct = frame_ms.map(|frame_ms| (frame_ms / target_120hz) * 100.0);
    let recorded_gpu_budget_pct = if gpu_total > 0.0 {
        Some((gpu_total / target_120hz) * 100.0)
    } else {
        None
    };
    let unattributed_frame_ms = match (frame_ms, Some(gpu_total).filter(|value| *value > 0.0)) {
        (Some(frame_ms), Some(gpu_total)) => Some((frame_ms - gpu_total).max(0.0)),
        _ => None,
    };

    game_shared::fun_diag_info!(
        target: "fun::perf::render_profile",
        gpu_metric_count = gpu_metrics.len(),
        cpu_metric_count = cpu_metrics.len(),
        stat_metric_count = statistic_metrics.len(),
        recorded_gpu_ms = gpu_total,
        recorded_cpu_ms = cpu_total,
        solari_sum_ms = ?solari_total,
        frame_ms = ?frame_ms,
        target_120hz_ms = target_120hz,
        frame_budget_pct = ?frame_budget_pct,
        recorded_gpu_budget_pct = ?recorded_gpu_budget_pct,
        unattributed_frame_ms = ?unattributed_frame_ms,
        pixels = ?pixel_count,
        "verbose render profile summary"
    );

    let mut group_totals = BTreeMap::<String, f64>::new();
    for metric in &gpu_metrics {
        *group_totals.entry(metric.group.clone()).or_default() += metric.current;
    }
    let mut group_totals = group_totals.into_iter().collect::<Vec<_>>();
    group_totals.sort_by(|(_, left), (_, right)| {
        right.partial_cmp(left).unwrap_or(std::cmp::Ordering::Equal)
    });

    for (rank, (group, value)) in group_totals.iter().enumerate() {
        let pct = percentage(*value, gpu_total);
        game_shared::fun_diag_info!(
            target: "fun::perf::render_profile::gpu_group",
            rank = rank + 1,
            group = %group,
            current_ms = *value,
            current_ns = ms_to_ns(Some(*value)),
            pct_recorded_gpu = ?pct,
            budget_120hz_pct = (*value / target_120hz) * 100.0,
            ns_per_pixel = ?ns_per_pixel(*value, pixel_count),
            "verbose render GPU group"
        );
    }

    for (rank, metric) in gpu_metrics.iter().enumerate() {
        let pct = percentage(metric.current, gpu_total);
        game_shared::fun_diag_info!(
            target: "fun::perf::render_profile::gpu_metric",
            rank = rank + 1,
            group = %metric.group,
            path = %metric.path,
            current_ms = metric.current,
            average_ms = ?metric.average,
            current_ns = ms_to_ns(Some(metric.current)),
            pct_recorded_gpu = ?pct,
            budget_120hz_pct = (metric.current / target_120hz) * 100.0,
            ns_per_pixel = ?ns_per_pixel(metric.current, pixel_count),
            recommendation = render_profile_recommendation(&metric.path),
            "verbose render GPU metric"
        );
    }

    for (rank, metric) in cpu_metrics.iter().enumerate() {
        game_shared::fun_diag_info!(
            target: "fun::perf::render_profile::cpu_metric",
            rank = rank + 1,
            group = %metric.group,
            path = %metric.path,
            current_ms = metric.current,
            average_ms = ?metric.average,
            current_ns = ms_to_ns(Some(metric.current)),
            "verbose render CPU metric"
        );
    }

    for (rank, metric) in statistic_metrics.iter().take(80).enumerate() {
        game_shared::fun_diag_info!(
            target: "fun::perf::render_profile::stat_metric",
            rank = rank + 1,
            group = %metric.group,
            kind = metric.kind,
            path = %metric.path,
            current = metric.current,
            average = ?metric.average,
            per_pixel = ?invocations_per_pixel(metric.current, pixel_count),
            "verbose render statistic metric"
        );
    }
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
fn collect_render_profile_metrics(diagnostics: &DiagnosticsStore) -> Vec<RenderProfileMetric> {
    diagnostics
        .iter()
        .filter_map(|diagnostic| {
            let path = diagnostic.path().as_str();
            if !path.starts_with("render/") {
                return None;
            }

            let current = diagnostic.value().or_else(|| diagnostic.average())?;
            let average = diagnostic.average();
            let kind = render_metric_kind(path);
            Some(RenderProfileMetric {
                path: path.to_owned(),
                group: render_metric_group(path),
                kind,
                current,
                average,
            })
        })
        .collect()
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
fn render_metric_group(path: &str) -> String {
    path.strip_prefix("render/")
        .unwrap_or(path)
        .split('/')
        .next()
        .unwrap_or("unknown")
        .to_owned()
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
fn render_metric_kind(path: &str) -> &'static str {
    match path.rsplit('/').next().unwrap_or_default() {
        "elapsed_gpu" => "elapsed_gpu",
        "elapsed_cpu" => "elapsed_cpu",
        "compute_shader_invocations" => "compute_shader_invocations",
        "fragment_shader_invocations" => "fragment_shader_invocations",
        "vertex_shader_invocations" => "vertex_shader_invocations",
        "clipper_invocations" => "clipper_invocations",
        "clipper_primitives_out" => "clipper_primitives_out",
        _ => "value",
    }
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
fn render_profile_recommendation(path: &str) -> &'static str {
    if path.contains("solari_lighting/direct_lighting") {
        "largest full-resolution Solari bucket: reduce direct-light ReSTIR work, specialize directional-light visibility, or make direct shadows adaptive before lowering resolution"
    } else if path.contains("meshlet_visibility_buffer_raster") {
        "visibility-buffer raster is resolution-bound: inspect meshlet pass count, material filtering, overdraw, and per-pixel visibility work"
    } else if path.contains("diffuse_indirect_lighting") {
        "GI bucket: tune half-resolution/checkerboard GI, reservoir reuse radius, and near-field fallback so fewer pixels trace while preserving corners"
    } else if path.contains("specular_indirect_lighting") {
        "specular bucket: use roughness buckets, lower glossy bounce count, and skip expensive filtering for mirrors"
    } else if path.contains("denoise") {
        "denoiser bucket: use confidence alpha to skip taps, keep fused output, and reduce spatial radius where history is stable"
    } else if path.contains("world_cache") {
        "world cache bucket: raise frame slices or lower soft-cap only if active-cell pressure spikes; keep near-camera priority stable"
    } else if path.contains("tonemapping") || path.contains("upscaling") {
        "post-process bucket: keep this small; optimize only after Solari and meshlet costs are under budget"
    } else if path.contains("prepass") || path.contains("deferred") {
        "G-buffer/prepass bucket: verify meshlet deferred prepass does not duplicate work or write unnecessary attachments"
    } else {
        "inspect shader invocations and resolution scaling for this bucket before changing quality"
    }
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
fn target_frame_ms(hz: f64) -> f64 {
    1000.0 / hz
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
fn percentage(value: f64, total: f64) -> Option<f64> {
    (total > 0.0).then_some((value / total) * 100.0)
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
fn ns_per_pixel(value_ms: f64, pixel_count: Option<u64>) -> Option<f64> {
    let pixels = pixel_count?;
    (pixels > 0).then_some((value_ms * 1_000_000.0) / pixels as f64)
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
fn invocations_per_pixel(value: f64, pixel_count: Option<u64>) -> Option<f64> {
    let pixels = pixel_count?;
    (pixels > 0).then_some(value / pixels as f64)
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
fn diagnostic_average(diagnostics: &DiagnosticsStore, path: &'static str) -> Option<f64> {
    diagnostics
        .get(&DiagnosticPath::new(path))
        .and_then(|diagnostic| diagnostic.average())
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
fn diagnostic_value(diagnostics: &DiagnosticsStore, path: &'static str) -> Option<f64> {
    diagnostics
        .get(&DiagnosticPath::new(path))
        .and_then(|diagnostic| diagnostic.value().or_else(|| diagnostic.average()))
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
fn diagnostic_value_path(diagnostics: &DiagnosticsStore, path: &DiagnosticPath) -> Option<f64> {
    diagnostics
        .get(path)
        .and_then(|diagnostic| diagnostic.value().or_else(|| diagnostic.average()))
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
fn diagnostic_average_path(diagnostics: &DiagnosticsStore, path: &DiagnosticPath) -> Option<f64> {
    diagnostics
        .get(path)
        .and_then(|diagnostic| diagnostic.average())
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
fn gpu_sample_status_from_code(code: f64) -> Option<&'static str> {
    match code.round() as u64 {
        1 => Some("ready"),
        2 => Some("query_pending"),
        3 => Some("warming_up"),
        4 => Some("not_rendered"),
        5 => Some("unsupported"),
        6 => Some("device_lost"),
        7 => Some("correlation_missing"),
        _ => None,
    }
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
fn classify_gpu_sample_status(
    raw_status: Option<&'static str>,
    app_frame_index: Option<u64>,
    render_frame_index: Option<u64>,
    render_recovery: Option<&RenderRecoveryStatus>,
    perf_counters: &mut ClientPerfCounters,
) -> &'static str {
    if render_recovery.is_some_and(|status| status.last_error_type == Some(ErrorType::DeviceLost)) {
        return "device_lost";
    }
    if render_recovery.is_some_and(|status| status.frames_without_rendering > 0) {
        return "not_rendered";
    }

    match raw_status {
        Some("ready") if perf_counters.mark_gpu_sample_frame(render_frame_index) => "ready",
        Some("ready") => "query_pending",
        Some(status) => status,
        None if app_frame_index.is_some_and(|frame| frame < 3) => "warming_up",
        None => "query_pending",
    }
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
fn format_optional_number(value: Option<f64>) -> String {
    value
        .map(format_number)
        .unwrap_or_else(|| "pending".to_owned())
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
fn ms_to_ns(value: Option<f64>) -> Option<u64> {
    value.map(ms_to_ns_from_value)
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
fn ms_to_ns_from_value(value: f64) -> u64 {
    (value * 1_000_000.0).round().max(0.0) as u64
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
fn ms_to_u32_ns(value: Option<f64>) -> u32 {
    ms_to_ns(value).unwrap_or_default().min(u64::from(u32::MAX)) as u32
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
fn sum_optional_ms(values: impl IntoIterator<Item = Option<f64>>) -> Option<f64> {
    values
        .into_iter()
        .flatten()
        .fold(None, |total: Option<f64>, value| {
            Some(total.unwrap_or_default() + value)
        })
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
fn format_number(value: f64) -> String {
    format!("{value:.2}")
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
fn format_optional_u64(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "pending".to_owned())
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
fn take_counter(counter: &mut u64) -> u64 {
    let value = *counter;
    *counter = 0;
    value
}

fn color_from_packed(color: Option<PackedColorRgba8>) -> StandardMaterial {
    let color = color.unwrap_or_else(|| PackedColorRgba8::srgb(180, 180, 180));
    Color::srgba_u8(color.r, color.g, color.b, color.a).into()
}

fn transform_from_quantized(transform: QuantizedTransform3) -> Transform {
    let rotation = transform.rotation.to_f32();
    Transform::from_translation(vec3_from_quantized(transform.translation)).with_rotation(
        Quat::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3]),
    )
}

fn vec3_from_quantized(value: QuantizedVec3) -> Vec3 {
    Vec3::from_array(value.to_f32(Quantization::MILLIMETERS))
}

#[derive(Debug, Default, Resource)]
struct LoadedWorldState {
    level_id: Option<String>,
    revision: Option<WorldRevision>,
    expected_chunks: u16,
    received_chunks: HashSet<u16>,
    spawned_entities: HashMap<NetEntity, Entity>,
    ack_sent: bool,
}

#[derive(Debug, Default, Resource)]
pub(crate) struct ClientWorldStatus {
    pub ready: bool,
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
#[derive(Debug, Resource)]
struct ClientDiagnostics {
    timer: Timer,
}

#[cfg(all(feature = "render_diagnostics", debug_assertions))]
impl Default for ClientDiagnostics {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(2.0, TimerMode::Repeating),
        }
    }
}

impl LoadedWorldState {
    fn is_complete(&self) -> bool {
        self.expected_chunks > 0 && self.received_chunks.len() >= self.expected_chunks as usize
    }
}
