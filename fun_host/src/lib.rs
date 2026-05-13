#![forbid(unsafe_code)]
#![allow(dead_code)]

pub mod account;

use fun_ecs::{
    EventReader as MessageReader, EventWriter as MessageWriter, Message, ResMut, Resource,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned, ser::SerializeStruct};

pub const FUN_CLIENT_HOST_PRODUCT: &str = "FunClientHost";
pub const MAX_HOST_COMMAND_PAYLOAD_BYTES: usize = 64 * 1024;

pub const HOST_SNAPSHOT_GET: &str = "host.snapshot.get";
pub const HOST_COMMANDS_LIST: &str = "host.commands.list";
pub const HOST_COMMANDBAR_EXECUTE: &str = "host.commandbar.execute";
pub const LAUNCHER_STATE_GET: &str = "launcher.state.get";
pub const LAUNCHER_SHOW: &str = "launcher.show";
pub const LAUNCHER_HIDE: &str = "launcher.hide";
pub const GAMES_LIST: &str = "games.list";
pub const PROJECTS_AUTHORIZED_LIST: &str = fun_editor_core::PROJECTS_AUTHORIZED_LIST;
pub const PROJECT_EDIT_OPEN: &str = fun_editor_core::PROJECT_EDIT_OPEN;
pub const EDITOR_ACTIVATE: &str = "editor.activate";
pub const EDITOR_DEACTIVATE: &str = "editor.deactivate";
pub const EDITOR_OVERLAY_TOGGLE: &str = "editor.overlay.toggle";
pub const RUNTIME_HOST_STATUS: &str = "runtime.host.status";
pub const RUNTIME_STATUS_GET: &str = "runtime.status.get";
pub const RUNTIME_INPUT_SET_OWNER: &str = "runtime.input.set_owner";
pub const RUNTIME_SERVER_LAUNCH: &str = "runtime.server.launch";
pub const GAMES_JOIN: &str = "games.join";
pub const VIEWPORT_CLIENT_LAUNCH: &str = "viewport.client.launch";
pub const VIEWPORT_CLIENT_STOP: &str = "viewport.client.stop";
pub const VIEWPORT_CLIENT_FOCUS: &str = "viewport.client.focus";
pub const VIEWPORT_CLIENT_RESIZE: &str = "viewport.client.resize";
pub const PREVIEW_VIEWPORT_ENSURE: &str = "preview.viewport.ensure";
pub const PREVIEW_VIEWPORT_STOP: &str = "preview.viewport.stop";
pub const PROJECT_OPEN: &str = fun_editor_core::PROJECT_OPEN;
pub const PREVIEW_RENDERER_ENSURE: &str = fun_editor_core::PREVIEW_RENDERER_ENSURE;
pub const PREVIEW_RENDERER_RESIZE: &str = fun_editor_core::PREVIEW_RENDERER_RESIZE;
pub const PREVIEW_RENDERER_FRAME_GET: &str = fun_editor_core::PREVIEW_RENDERER_FRAME_GET;
pub const PREVIEW_RENDERER_SCENE_SET: &str = fun_editor_core::PREVIEW_RENDERER_SCENE_SET;
pub const PREVIEW_RENDERER_STATUS_GET: &str = fun_editor_core::PREVIEW_RENDERER_STATUS_GET;
pub const MATERIAL_SHADER_LIST: &str = fun_editor_core::MATERIAL_SHADER_LIST;
pub const MATERIAL_SHADER_LOAD: &str = fun_editor_core::MATERIAL_SHADER_LOAD;
pub const MATERIAL_SHADER_SAVE: &str = fun_editor_core::MATERIAL_SHADER_SAVE;
pub const ENTITY_STREAM_OPEN: &str = fun_editor_core::ENTITY_STREAM_OPEN;
pub const RUNTIME_DIAGNOSTICS_LIST: &str = fun_editor_core::RUNTIME_DIAGNOSTICS_LIST;
pub const SCENE_OPERATION_APPLY: &str = fun_editor_core::SCENE_OPERATION_APPLY;
pub const ACCOUNT_LOGIN: &str = "account.login";
pub const ACCOUNT_REGISTER: &str = "account.register";
pub const ACCOUNT_LOGOUT: &str = "account.logout";
pub const AUTH_TICKET_REQUEST: &str = "auth.ticket.request";
pub const BACKEND_AUTH_SESSION_GET: &str = "backend.auth.session.get";
pub const AUTH_ACCOUNT_TICKET_REQUEST: &str = "auth.account.ticket.request";
pub const AUTH_BACKEND_SESSION_GET: &str = "auth.backend.session.get";
pub const AUTH_LOGOUT: &str = "auth.logout";

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunClientHostModule;

impl FunClientHostModule {
    #[must_use]
    pub const fn install_report(self) -> FunClientHostModuleInstallReport {
        FunClientHostModuleInstallReport {
            host_state_resource: true,
            scene_operation_queue_resource: true,
            command_request_stream: true,
            command_response_stream: true,
            command_router_registered: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FunClientHostModuleInstallReport {
    pub host_state_resource: bool,
    pub scene_operation_queue_resource: bool,
    pub command_request_stream: bool,
    pub command_response_stream: bool,
    pub command_router_registered: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FunHostMode {
    Boot,
    #[default]
    Launcher,
    Game,
    Editor,
    EditorOverlay,
    Loading,
    Shutdown,
}

pub type FunClientHostMode = FunHostMode;

impl FunHostMode {
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Boot => "boot",
            Self::Launcher => "launcher",
            Self::Game => "game",
            Self::Editor => "editor",
            Self::EditorOverlay => "editor_overlay",
            Self::Loading => "loading",
            Self::Shutdown => "shutdown",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FunClientHostLifecycle {
    #[default]
    Booting,
    Ready,
    ShuttingDown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunHostState {
    pub mode: FunHostMode,
    pub input_owner: FunInputOwner,
    pub game: GameUiState,
    pub launcher: LauncherState,
    pub editor: EditorState,
    pub project: ProjectState,
    pub runtime: RuntimeHostState,
    pub diagnostics: DiagnosticsState,
    pub account: AccountState,
    pub commandbar: CommandbarState,
}

impl FunHostState {
    #[must_use]
    pub fn with_mode(mode: FunHostMode) -> Self {
        let mut state = Self {
            mode,
            ..Self::default()
        };
        state.apply_mode(mode);
        state
    }

    pub fn apply_mode(&mut self, mode: FunHostMode) {
        self.mode = mode;
        self.input_owner = FunInputOwner::for_mode(mode);
        self.launcher.input_capture = matches!(mode, FunHostMode::Launcher | FunHostMode::Loading);
        self.launcher.visible = matches!(mode, FunHostMode::Launcher);
        self.game.simulation_active =
            matches!(mode, FunHostMode::Game | FunHostMode::EditorOverlay);
        self.game.hud_passive = matches!(mode, FunHostMode::Game);
        self.game.render_visible_behind_ui = matches!(
            mode,
            FunHostMode::Game | FunHostMode::Editor | FunHostMode::EditorOverlay
        );
        self.editor.active = matches!(mode, FunHostMode::Editor | FunHostMode::EditorOverlay);
        self.editor.overlay_active = matches!(mode, FunHostMode::EditorOverlay);
        self.editor.input_capture =
            matches!(mode, FunHostMode::Editor | FunHostMode::EditorOverlay);
        self.editor.services_active =
            matches!(mode, FunHostMode::Editor | FunHostMode::EditorOverlay);
        self.runtime.loading_visible = matches!(mode, FunHostMode::Loading);
    }
}

impl Default for FunHostState {
    fn default() -> Self {
        let mut state = Self {
            mode: FunHostMode::Launcher,
            input_owner: FunInputOwner::LauncherUi,
            game: GameUiState::default(),
            launcher: LauncherState::default(),
            editor: EditorState::default(),
            project: ProjectState::default(),
            runtime: RuntimeHostState::default(),
            diagnostics: DiagnosticsState::default(),
            account: AccountState::default(),
            commandbar: CommandbarState::default(),
        };
        state.apply_mode(FunHostMode::Launcher);
        state
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FunInputOwner {
    #[default]
    Gameplay,
    LauncherUi,
    EditorUi,
    GameMenuUi,
    TextEntry,
    Commandbar,
}

impl FunInputOwner {
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Gameplay => "gameplay",
            Self::LauncherUi => "launcher_ui",
            Self::EditorUi => "editor_ui",
            Self::GameMenuUi => "game_menu_ui",
            Self::TextEntry => "text_entry",
            Self::Commandbar => "commandbar",
        }
    }

    #[must_use]
    pub const fn for_mode(mode: FunHostMode) -> Self {
        match mode {
            FunHostMode::Boot | FunHostMode::Launcher | FunHostMode::Loading => Self::LauncherUi,
            FunHostMode::Game => Self::Gameplay,
            FunHostMode::Editor | FunHostMode::EditorOverlay => Self::EditorUi,
            FunHostMode::Shutdown => Self::Gameplay,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameUiState {
    pub simulation_active: bool,
    pub hud_passive: bool,
    pub render_visible_behind_ui: bool,
    pub active_game_id: Option<String>,
    pub active_session_id: Option<String>,
    pub viewport_layout: FunViewportLayoutState,
}

impl Default for GameUiState {
    fn default() -> Self {
        Self {
            simulation_active: true,
            hud_passive: true,
            render_visible_behind_ui: true,
            active_game_id: None,
            active_session_id: None,
            viewport_layout: FunViewportLayoutState::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FunViewportLayoutState {
    pub revision: u64,
    pub rect: Option<FunViewportRect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunViewportRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor_milli: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LauncherState {
    pub visible: bool,
    pub input_capture: bool,
    pub selected_game_id: Option<String>,
    pub selected_project_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorState {
    pub active: bool,
    pub overlay_active: bool,
    pub input_capture: bool,
    pub services_active: bool,
    pub active_route: FunEditorRoute,
    pub preview_mode: EditorPreviewMode,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            active: false,
            overlay_active: false,
            input_capture: false,
            services_active: false,
            active_route: FunEditorRoute::Overview,
            preview_mode: EditorPreviewMode::CurrentClient,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FunEditorRoute {
    #[default]
    Overview,
    Preview,
    LiveClient,
    Server,
    Graph,
    Diagnostics,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditorPreviewMode {
    #[default]
    CurrentClient,
    PreviewWorld,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectState {
    pub local_project_authorized: bool,
    pub active_project_id: Option<String>,
    pub active_project_root: Option<String>,
}

impl Default for ProjectState {
    fn default() -> Self {
        Self {
            local_project_authorized: true,
            active_project_id: None,
            active_project_root: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeHostState {
    pub return_to_game_available: bool,
    pub active_runtime_id: Option<String>,
    pub services: FunHostRuntimeServices,
    pub loading_visible: bool,
    pub server_addr: Option<String>,
    pub client_session_configured: bool,
    pub server_runtime_running: bool,
}

impl Default for RuntimeHostState {
    fn default() -> Self {
        Self {
            return_to_game_available: true,
            active_runtime_id: None,
            services: FunHostRuntimeServices::default(),
            loading_visible: false,
            server_addr: None,
            client_session_configured: false,
            server_runtime_running: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DiagnosticsState {
    pub unread_count: u32,
    pub last_error_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountState {
    pub backend_session_available: bool,
    pub ticket_redacted: bool,
}

impl Default for AccountState {
    fn default() -> Self {
        Self {
            backend_session_available: false,
            ticket_redacted: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CommandbarState {
    pub active: bool,
    pub pending_command_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunClientHostStartConfig {
    pub mode: FunHostMode,
    pub project_id: Option<String>,
    pub project_path: Option<String>,
    pub server_addr: Option<String>,
    pub game_session_id: Option<String>,
}

impl Default for FunClientHostStartConfig {
    fn default() -> Self {
        Self {
            mode: FunHostMode::Launcher,
            project_id: None,
            project_path: None,
            server_addr: None,
            game_session_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Resource, Serialize, Deserialize)]
pub struct FunClientHostState {
    pub lifecycle: FunClientHostLifecycle,
    pub state: FunHostState,
    pub previous_mode: Option<FunClientHostMode>,
    pub native_ui_command_surface_ready: bool,
    sequence: u64,
}

impl FunClientHostState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn from_start_config(config: FunClientHostStartConfig) -> Self {
        let mut host = Self::default();
        host.apply_start_config(config);
        host
    }

    pub fn apply_start_config(&mut self, config: FunClientHostStartConfig) {
        let FunClientHostStartConfig {
            mode,
            project_id,
            project_path,
            server_addr,
            game_session_id,
        } = config;

        let project_id =
            project_id.or_else(|| project_path.as_deref().and_then(project_id_from_path));
        if let Some(project_id) = project_id {
            self.state.launcher.selected_project_id = Some(project_id.clone());
            self.state.project.active_project_id = Some(project_id);
        }
        if let Some(project_path) = project_path {
            self.state.project.active_project_root = Some(project_path);
        }
        if let Some(server_addr) = server_addr {
            self.state.runtime.server_addr = Some(server_addr);
        }
        if let Some(game_session_id) = game_session_id {
            self.state.launcher.selected_game_id = Some(game_session_id.clone());
            self.state.game.active_session_id = Some(game_session_id);
        }
        if matches!(mode, FunHostMode::Game) {
            self.activate_current_client_runtime();
        }
        self.state.apply_mode(mode);
        self.mark_changed();
    }

    #[must_use]
    pub const fn snapshot(&self) -> FunClientHostSnapshot<'_> {
        FunClientHostSnapshot {
            product_host: FUN_CLIENT_HOST_PRODUCT,
            lifecycle: self.lifecycle,
            state: &self.state,
            previous_mode: self.previous_mode,
            native_ui_command_surface_ready: self.native_ui_command_surface_ready,
        }
    }

    pub fn transition_to(
        &mut self,
        next_mode: FunClientHostMode,
    ) -> Result<(), FunHostCommandErrorCode> {
        if matches!(self.lifecycle, FunClientHostLifecycle::ShuttingDown) {
            return Err(FunHostCommandErrorCode::HostShuttingDown);
        }

        match next_mode {
            FunHostMode::Launcher => {
                self.previous_mode = Some(self.state.mode);
                self.state.apply_mode(FunHostMode::Launcher);
                self.mark_changed();
                Ok(())
            }
            FunHostMode::Game => {
                if !self.state.runtime.return_to_game_available {
                    return Err(FunHostCommandErrorCode::ReturnToGameUnavailable);
                }
                self.previous_mode = Some(self.state.mode);
                self.state.apply_mode(FunHostMode::Game);
                self.mark_changed();
                Ok(())
            }
            FunHostMode::Editor | FunHostMode::EditorOverlay => {
                if !self.state.project.local_project_authorized {
                    return Err(FunHostCommandErrorCode::ProjectUnauthorized);
                }
                self.previous_mode = Some(self.state.mode);
                self.state.apply_mode(next_mode);
                self.mark_changed();
                Ok(())
            }
            FunHostMode::Boot | FunHostMode::Loading | FunHostMode::Shutdown => {
                self.previous_mode = Some(self.state.mode);
                self.state.apply_mode(next_mode);
                self.mark_changed();
                Ok(())
            }
        }
    }

    pub fn set_input_owner(&mut self, owner: FunInputOwner) {
        self.state.input_owner = owner;
        self.state.commandbar.active = matches!(owner, FunInputOwner::Commandbar);
        match owner {
            FunInputOwner::Gameplay => self.state.apply_mode(FunHostMode::Game),
            FunInputOwner::LauncherUi => self.state.apply_mode(FunHostMode::Launcher),
            FunInputOwner::EditorUi => {
                let mode = if self.state.editor.overlay_active {
                    FunHostMode::EditorOverlay
                } else {
                    FunHostMode::Editor
                };
                self.state.apply_mode(mode);
            }
            FunInputOwner::GameMenuUi | FunInputOwner::TextEntry | FunInputOwner::Commandbar => {
                self.state.input_owner = owner;
            }
        }
        self.mark_changed();
    }

    pub fn toggle_editor_overlay(&mut self) -> Result<(), FunHostCommandErrorCode> {
        if matches!(self.state.mode, FunHostMode::EditorOverlay) {
            self.transition_to(FunHostMode::Game)
        } else {
            self.transition_to(FunHostMode::EditorOverlay)
        }
    }

    fn set_editor_route(&mut self, route: FunEditorRoute) {
        self.state.editor.active_route = route;
        self.mark_changed();
    }

    fn update_viewport_layout_from_request(&mut self, request: &FunHostCommandRequest) {
        let Some(rect) = parse_viewport_rect(&request.payload_json) else {
            return;
        };
        self.state.game.viewport_layout.revision =
            self.state.game.viewport_layout.revision.saturating_add(1);
        self.state.game.viewport_layout.rect = Some(rect);
        self.mark_changed();
    }

    fn ensure_current_client_preview(&mut self, request: &FunHostCommandRequest) {
        self.state.editor.preview_mode = EditorPreviewMode::CurrentClient;
        self.update_viewport_layout_from_request(request);
    }

    fn activate_current_client_runtime(&mut self) {
        self.state.runtime.active_runtime_id = Some("current-client".to_owned());
        self.state.runtime.client_session_configured = true;
    }

    fn configure_current_client_session_from_request(&mut self, request: &FunHostCommandRequest) {
        let payload = parse_game_session_payload(&request.payload_json).unwrap_or_default();
        if let Some(game_id) = payload.game_id.or(payload.game_session_id) {
            self.state.launcher.selected_game_id = Some(game_id.clone());
            self.state.game.active_game_id = Some(game_id);
        }
        if let Some(project_id) = payload.project_id {
            self.state.launcher.selected_project_id = Some(project_id.clone());
            self.state.project.active_project_id = Some(project_id);
        }
        if let Some(project_path) = payload.project_path {
            self.state.project.active_project_root = Some(project_path);
        }
        if let Some(server_addr) = payload.server_addr {
            self.state.runtime.server_addr = Some(server_addr);
        }
        if let Some(session_id) = payload.session_id {
            self.state.game.active_session_id = Some(session_id);
        }
        self.update_viewport_layout_from_request(request);
        self.activate_current_client_runtime();
        self.mark_changed();
    }

    fn launch_server_runtime_from_request(&mut self, request: &FunHostCommandRequest) {
        let payload = parse_game_session_payload(&request.payload_json).unwrap_or_default();
        if let Some(project_id) = payload.project_id {
            self.state.launcher.selected_project_id = Some(project_id.clone());
            self.state.project.active_project_id = Some(project_id);
        }
        if let Some(project_path) = payload.project_path {
            self.state.project.active_project_root = Some(project_path);
        }
        if let Some(server_addr) = payload.server_addr {
            self.state.runtime.server_addr = Some(server_addr);
        }
        self.state.runtime.server_runtime_running = true;
        self.state.runtime.services.runtime_lifecycle = FunHostServiceState::Ready;
        self.activate_current_client_runtime();
        let _ = self.transition_to(FunHostMode::Game);
        self.mark_changed();
    }

    fn next_sequence(&mut self) -> u64 {
        self.sequence = self.sequence.saturating_add(1);
        self.sequence
    }

    fn mark_changed(&mut self) {
        self.next_sequence();
    }

    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn snapshot_payload_json(&self) -> Result<Vec<u8>, serde_json::Error> {
        encode_payload(&HostSnapshotPayload {
            snapshot: self.snapshot(),
            command_catalog: FUN_HOST_COMMANDS,
            editor_command_catalog: fun_editor_core::FUN_EDITOR_COMMANDS,
        })
    }

    fn handle_command(&mut self, request: &FunHostCommandRequest) -> FunHostCommandResponse {
        let sequence = self.next_sequence();
        if request.payload_json.len() > MAX_HOST_COMMAND_PAYLOAD_BYTES {
            return FunHostCommandResponse::error(
                request,
                sequence,
                FunHostCommandErrorCode::OversizePayload,
            );
        }
        if !request.payload_json.is_empty()
            && serde_json::from_slice::<serde_json::Value>(&request.payload_json).is_err()
        {
            return FunHostCommandResponse::error(
                request,
                sequence,
                FunHostCommandErrorCode::InvalidPayload,
            );
        }

        let Some(command) = FunHostCommand::from_wire_id(&request.command_id) else {
            return FunHostCommandResponse::error(
                request,
                sequence,
                FunHostCommandErrorCode::UnknownCommand,
            );
        };

        match command {
            FunHostCommand::HostSnapshotGet
            | FunHostCommand::LauncherStateGet
            | FunHostCommand::GamesList
            | FunHostCommand::ProjectsAuthorizedList
            | FunHostCommand::RuntimeHostStatus
            | FunHostCommand::RuntimeStatusGet
            | FunHostCommand::RuntimeDiagnosticsList => self.snapshot_response(request, sequence),
            FunHostCommand::HostCommandsList => {
                self.payload_response(request, sequence, &FUN_HOST_COMMANDS)
            }
            FunHostCommand::HostCommandbarExecute => {
                self.commandbar_execute_response(request, sequence)
            }
            FunHostCommand::LauncherShow => {
                self.transition_response(request, sequence, FunClientHostMode::Launcher)
            }
            FunHostCommand::LauncherHide | FunHostCommand::EditorDeactivate => {
                self.transition_response(request, sequence, FunClientHostMode::Game)
            }
            FunHostCommand::GamesJoin
            | FunHostCommand::ViewportClientLaunch
            | FunHostCommand::ViewportClientFocus => {
                self.configure_current_client_session_from_request(request);
                self.transition_response(request, sequence, FunClientHostMode::Game)
            }
            FunHostCommand::ViewportClientResize => {
                self.update_viewport_layout_from_request(request);
                self.snapshot_response(request, sequence)
            }
            FunHostCommand::ViewportClientStop | FunHostCommand::PreviewViewportStop => {
                self.snapshot_response(request, sequence)
            }
            FunHostCommand::RuntimeInputSetOwner => {
                let owner =
                    parse_input_owner(&request.payload_json).unwrap_or(FunInputOwner::Gameplay);
                self.set_input_owner(owner);
                self.snapshot_response(request, sequence)
            }
            FunHostCommand::EditorActivate
            | FunHostCommand::ProjectEditOpen
            | FunHostCommand::ProjectOpen
            | FunHostCommand::EntityStreamOpen => {
                self.transition_response(request, sequence, FunClientHostMode::Editor)
            }
            FunHostCommand::EditorOverlayToggle => match self.toggle_editor_overlay() {
                Ok(()) => self.snapshot_response(request, sequence),
                Err(error) => FunHostCommandResponse::error(request, sequence, error),
            },
            FunHostCommand::PreviewViewportEnsure
            | FunHostCommand::PreviewRendererEnsure
            | FunHostCommand::PreviewRendererResize
            | FunHostCommand::PreviewRendererFrameGet
            | FunHostCommand::PreviewRendererSceneSet
            | FunHostCommand::PreviewRendererStatusGet => {
                self.ensure_current_client_preview(request);
                self.transition_response(request, sequence, FunClientHostMode::Editor)
            }
            FunHostCommand::MaterialShaderList => {
                self.material_shader_list_response(request, sequence)
            }
            FunHostCommand::MaterialShaderLoad => {
                self.material_shader_load_response(request, sequence)
            }
            FunHostCommand::MaterialShaderSave => {
                self.material_shader_save_response(request, sequence)
            }
            FunHostCommand::SceneOperationApply => {
                self.scene_operation_queue_unavailable_response(request, sequence)
            }
            FunHostCommand::RuntimeServerLaunch => {
                self.launch_server_runtime_from_request(request);
                self.runtime_server_launch_response(request, sequence)
            }
            FunHostCommand::AccountLogin => self.account_command_response(
                request,
                sequence,
                account::request_backend_login_payload(&request.payload_json),
            ),
            FunHostCommand::AccountRegister => self.account_command_response(
                request,
                sequence,
                account::request_backend_register_payload(&request.payload_json),
            ),
            FunHostCommand::AuthAccountTicketRequest => self.account_command_response(
                request,
                sequence,
                account::request_backend_account_ticket_payload(&request.payload_json),
            ),
            FunHostCommand::AuthTicketRequest => self.account_command_response(
                request,
                sequence,
                account::request_backend_auth_ticket_payload(&request.payload_json),
            ),
            FunHostCommand::BackendAuthSessionGet => {
                self.sync_account_state();
                self.payload_response(request, sequence, &account::backend_auth_session())
            }
            FunHostCommand::AccountLogout => {
                self.account_command_response(request, sequence, account::logout_backend_account())
            }
        }
    }

    fn material_shader_list_response(
        &self,
        request: &FunHostCommandRequest,
        sequence: u64,
    ) -> FunHostCommandResponse {
        let payload = parse_request_payload::<MaterialShaderListPayload>(&request.payload_json);
        let result = account::CommandResult {
            ok: true,
            value: Some(material_shader_catalog(&self.state.project, payload)),
            diagnostics: Vec::new(),
        };
        self.payload_response(request, sequence, &result)
    }

    fn material_shader_load_response(
        &self,
        request: &FunHostCommandRequest,
        sequence: u64,
    ) -> FunHostCommandResponse {
        let payload = parse_request_payload::<MaterialShaderPathPayload>(&request.payload_json);
        let summary = material_shader_placeholder_summary(
            &self.state.project,
            payload.project_id,
            payload.relative_path,
        );
        let result = account::CommandResult::<MaterialShaderDocument> {
            ok: false,
            value: None,
            diagnostics: vec![material_shader_not_ready_diagnostic(
                request.command_id.as_str(),
                Some(summary.relative_path),
            )],
        };
        self.payload_response(request, sequence, &result)
    }

    fn material_shader_save_response(
        &self,
        request: &FunHostCommandRequest,
        sequence: u64,
    ) -> FunHostCommandResponse {
        let payload = parse_request_payload::<MaterialShaderSavePayload>(&request.payload_json);
        let source_len = payload
            .source
            .as_ref()
            .and_then(|source| u64::try_from(source.len()).ok())
            .unwrap_or(u64::MAX);
        let mut summary = material_shader_placeholder_summary(
            &self.state.project,
            payload.project_id,
            payload.relative_path,
        );
        summary.byte_len = source_len;
        let result = account::CommandResult::<MaterialShaderDocument> {
            ok: false,
            value: None,
            diagnostics: vec![material_shader_not_ready_diagnostic(
                request.command_id.as_str(),
                Some(summary.relative_path),
            )],
        };
        self.payload_response(request, sequence, &result)
    }

    fn scene_operation_apply_response(
        &mut self,
        request: &FunHostCommandRequest,
        sequence: u64,
        queue: &mut fun_scene::EditorOperationQueue,
    ) -> FunHostCommandResponse {
        let result = match parse_scene_operation_envelope(&request.payload_json) {
            Ok(envelope) => match envelope.validate_product() {
                Ok(()) => {
                    let accepted_operation_count = envelope.operation_count();
                    queue.extend(envelope.into_operations());
                    let _ = self.transition_to(FunHostMode::Editor);
                    self.mark_changed();
                    account::CommandResult {
                        ok: true,
                        value: Some(SceneOperationReceipt {
                            schema_version: fun_scene::EDITOR_OPERATION_SCHEMA_VERSION,
                            accepted_operation_count,
                            queued_for_fun_scene: true,
                            host_applies_through_fun_scene: true,
                            dynamic_rust_expressions_allowed: false,
                        }),
                        diagnostics: Vec::new(),
                    }
                }
                Err(error) => scene_operation_rejection(error.as_str()),
            },
            Err(_) => scene_operation_rejection("invalid_scene_operation_payload"),
        };
        self.payload_response(request, sequence, &result)
    }

    fn scene_operation_queue_unavailable_response(
        &self,
        request: &FunHostCommandRequest,
        sequence: u64,
    ) -> FunHostCommandResponse {
        self.payload_response(
            request,
            sequence,
            &scene_operation_rejection::<SceneOperationReceipt>(
                "scene_operation_queue_unavailable",
            ),
        )
    }

    fn snapshot_response(
        &self,
        request: &FunHostCommandRequest,
        sequence: u64,
    ) -> FunHostCommandResponse {
        match self.snapshot_payload_json() {
            Ok(payload_json) => FunHostCommandResponse::ok(request, sequence, payload_json),
            Err(error) => {
                tracing::warn!(
                    target: "fun::host",
                    ?error,
                    command_id = %request.command_id,
                    "failed to encode host snapshot response"
                );
                FunHostCommandResponse::error(
                    request,
                    sequence,
                    FunHostCommandErrorCode::ResponseEncodingFailed,
                )
            }
        }
    }

    fn payload_response<T: Serialize>(
        &self,
        request: &FunHostCommandRequest,
        sequence: u64,
        payload: &T,
    ) -> FunHostCommandResponse {
        match encode_payload(payload) {
            Ok(payload_json) => FunHostCommandResponse::ok(request, sequence, payload_json),
            Err(error) => {
                tracing::warn!(
                    target: "fun::host",
                    ?error,
                    command_id = %request.command_id,
                    "failed to encode host command payload"
                );
                FunHostCommandResponse::error(
                    request,
                    sequence,
                    FunHostCommandErrorCode::ResponseEncodingFailed,
                )
            }
        }
    }

    fn account_command_response<T: Serialize>(
        &mut self,
        request: &FunHostCommandRequest,
        sequence: u64,
        result: account::CommandResult<T>,
    ) -> FunHostCommandResponse {
        self.sync_account_state();
        self.mark_changed();
        self.payload_response(request, sequence, &result)
    }

    fn runtime_server_launch_response(
        &self,
        request: &FunHostCommandRequest,
        sequence: u64,
    ) -> FunHostCommandResponse {
        self.payload_response(
            request,
            sequence,
            &account::CommandResult {
                ok: true,
                value: Some(RuntimeInstanceSummary::from_host_state(&self.state)),
                diagnostics: Vec::new(),
            },
        )
    }

    fn commandbar_execute_response(
        &mut self,
        request: &FunHostCommandRequest,
        sequence: u64,
    ) -> FunHostCommandResponse {
        let Ok(payload) = serde_json::from_slice::<CommandbarExecutePayload>(&request.payload_json)
        else {
            return self.payload_response(
                request,
                sequence,
                &commandbar_rejection::<CommandbarExecutionPayload<'_>>(
                    "host.commandbar.payload_invalid",
                    "Commandbar execution payload did not match the host schema.",
                ),
            );
        };
        let command_id = payload.command_id.trim();
        if command_id.is_empty() {
            return self.payload_response(
                request,
                sequence,
                &commandbar_rejection::<CommandbarExecutionPayload<'_>>(
                    "host.commandbar.command_empty",
                    "Commandbar command ID cannot be empty.",
                ),
            );
        }
        if commandbar_tool_stub_is_gated(command_id) {
            return self.payload_response(
                request,
                sequence,
                &commandbar_rejection::<CommandbarExecutionPayload<'_>>(
                    "host.commandbar.tool_stub_gated",
                    "This commandbar tool is registered but not connected to runtime execution.",
                ),
            );
        }

        match command_id {
            "editor.set_context.overview" => self.set_editor_route(FunEditorRoute::Overview),
            "editor.set_context.preview" => self.set_editor_route(FunEditorRoute::Preview),
            "editor.set_context.live_client" => self.set_editor_route(FunEditorRoute::LiveClient),
            "editor.set_context.server" => self.set_editor_route(FunEditorRoute::Server),
            "editor.set_context.graph" => self.set_editor_route(FunEditorRoute::Graph),
            "editor.set_context.diagnostics" | "editor.show_log" => {
                self.set_editor_route(FunEditorRoute::Diagnostics);
            }
            "editor.reload_preview" => {
                self.ensure_current_client_preview(request);
                let _ = self.transition_to(FunHostMode::Editor);
            }
            "editor.launch_client" => {
                self.configure_current_client_session_from_request(request);
                let _ = self.transition_to(FunHostMode::Game);
            }
            "editor.launch_server" => self.launch_server_runtime_from_request(request),
            "editor.stop_runtime" | "editor.frame_viewport" | "editor.pick_project" => {
                self.state.commandbar.pending_command_id = Some(command_id.to_owned());
                self.mark_changed();
            }
            "editor.open_default_project" => {
                self.state.commandbar.pending_command_id = Some(command_id.to_owned());
                let _ = self.transition_to(FunHostMode::Editor);
            }
            _ if commandbar_read_tool_is_allowed(command_id) => {
                self.state.commandbar.pending_command_id = Some(command_id.to_owned());
                self.mark_changed();
            }
            _ if FunHostCommand::from_wire_id(command_id)
                .is_some_and(|command| command != FunHostCommand::HostCommandbarExecute) =>
            {
                let nested = FunHostCommandRequest::new(
                    request.request_id,
                    command_id.to_owned(),
                    serde_json::to_vec(
                        payload.payload.as_ref().unwrap_or(&serde_json::Value::Null),
                    )
                    .unwrap_or_else(|_| b"null".to_vec()),
                );
                let nested_response = self.handle_command(&nested);
                if nested_response.status != FunHostCommandStatus::Ok {
                    return self.payload_response(
                        request,
                        sequence,
                        &commandbar_rejection::<CommandbarExecutionPayload<'_>>(
                            "host.commandbar.nested_command_failed",
                            "Rust rejected the commandbar host command.",
                        ),
                    );
                }
            }
            _ => {
                return self.payload_response(
                    request,
                    sequence,
                    &commandbar_rejection::<CommandbarExecutionPayload<'_>>(
                        "host.commandbar.command_unknown",
                        "Commandbar command is not registered with the Rust host.",
                    ),
                );
            }
        }

        self.state.commandbar.pending_command_id = Some(command_id.to_owned());
        self.payload_response(
            request,
            sequence,
            &account::CommandResult {
                ok: true,
                value: Some(CommandbarExecutionPayload {
                    accepted_command_id: command_id,
                    snapshot: self.snapshot(),
                }),
                diagnostics: Vec::new(),
            },
        )
    }

    fn sync_account_state(&mut self) {
        let session = account::backend_auth_session();
        self.state.account.backend_session_available = session.authenticated;
        self.state.account.ticket_redacted = session
            .ticket
            .as_ref()
            .is_none_or(|ticket| ticket.ticket_redacted);
        self.state.runtime.services.account_session = if session.authenticated {
            FunHostServiceState::Ready
        } else {
            FunHostServiceState::NeedsBackendSession
        };
    }

    fn transition_response(
        &mut self,
        request: &FunHostCommandRequest,
        sequence: u64,
        next_mode: FunClientHostMode,
    ) -> FunHostCommandResponse {
        match self.transition_to(next_mode) {
            Ok(()) => self.snapshot_response(request, sequence),
            Err(error) => FunHostCommandResponse::error(request, sequence, error),
        }
    }
}

impl Default for FunClientHostState {
    fn default() -> Self {
        Self {
            lifecycle: FunClientHostLifecycle::Ready,
            state: FunHostState::with_mode(FunHostMode::Launcher),
            previous_mode: None,
            native_ui_command_surface_ready: true,
            sequence: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FunHostRuntimeServices {
    pub launcher_authority: FunHostServiceState,
    pub editor_authority: FunHostServiceState,
    pub runtime_lifecycle: FunHostServiceState,
    pub project_index: FunHostServiceState,
    pub preview_renderer: FunHostServiceState,
    pub account_session: FunHostServiceState,
    pub native_ui_command_router: FunHostServiceState,
}

impl Default for FunHostRuntimeServices {
    fn default() -> Self {
        Self {
            launcher_authority: FunHostServiceState::Ready,
            editor_authority: FunHostServiceState::Ready,
            runtime_lifecycle: FunHostServiceState::Ready,
            project_index: FunHostServiceState::Ready,
            preview_renderer: FunHostServiceState::Ready,
            account_session: FunHostServiceState::NeedsBackendSession,
            native_ui_command_router: FunHostServiceState::Ready,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FunHostServiceState {
    Disabled,
    Ready,
    NeedsBackendSession,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FunClientHostSnapshot<'a> {
    pub product_host: &'static str,
    pub lifecycle: FunClientHostLifecycle,
    pub state: &'a FunHostState,
    pub previous_mode: Option<FunClientHostMode>,
    pub native_ui_command_surface_ready: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeInstanceSummary {
    pub id: String,
    pub project_id: String,
    pub kind: &'static str,
    pub pid: Option<u32>,
    pub process_status: &'static str,
    pub inspector_status: &'static str,
    pub authenticated: bool,
    pub inspector_addr: &'static str,
    pub launch_command: Vec<String>,
    pub stdout_log_path: Option<String>,
    pub stderr_log_path: Option<String>,
    pub diagnostics: Vec<account::Diagnostic>,
}

impl RuntimeInstanceSummary {
    fn from_host_state(state: &FunHostState) -> Self {
        let project_id = state
            .project
            .active_project_id
            .clone()
            .or_else(|| state.launcher.selected_project_id.clone())
            .unwrap_or_else(|| "current-project".to_owned());
        let mut launch_command = vec![
            "fun-client".to_owned(),
            "--start-mode=game".to_owned(),
            format!("--project-id={project_id}"),
        ];
        if let Some(server_addr) = &state.runtime.server_addr {
            launch_command.push(format!("--server-addr={server_addr}"));
        }
        Self {
            id: state
                .runtime
                .active_runtime_id
                .clone()
                .unwrap_or_else(|| "current-client".to_owned()),
            project_id,
            kind: "client",
            pid: None,
            process_status: "running",
            inspector_status: "in_process",
            authenticated: state.account.backend_session_available,
            inspector_addr: "in-process",
            launch_command,
            stdout_log_path: None,
            stderr_log_path: None,
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FunHostCommandCategory {
    Host,
    Launcher,
    Game,
    Editor,
    Project,
    Runtime,
    Preview,
    Material,
    Entity,
    Diagnostics,
    Auth,
    Backend,
}

impl FunHostCommandCategory {
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Launcher => "launcher",
            Self::Game => "game",
            Self::Editor => "editor",
            Self::Project => "project",
            Self::Runtime => "runtime",
            Self::Preview => "preview",
            Self::Material => "material",
            Self::Entity => "entity",
            Self::Diagnostics => "diagnostics",
            Self::Auth => "auth",
            Self::Backend => "backend",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunHostCommand {
    HostSnapshotGet,
    HostCommandsList,
    HostCommandbarExecute,
    LauncherStateGet,
    LauncherShow,
    LauncherHide,
    GamesList,
    ProjectsAuthorizedList,
    ProjectEditOpen,
    EditorActivate,
    EditorDeactivate,
    EditorOverlayToggle,
    RuntimeHostStatus,
    RuntimeStatusGet,
    RuntimeInputSetOwner,
    RuntimeServerLaunch,
    GamesJoin,
    ViewportClientLaunch,
    ViewportClientStop,
    ViewportClientFocus,
    ViewportClientResize,
    PreviewViewportEnsure,
    PreviewViewportStop,
    ProjectOpen,
    PreviewRendererEnsure,
    PreviewRendererResize,
    PreviewRendererFrameGet,
    PreviewRendererSceneSet,
    PreviewRendererStatusGet,
    MaterialShaderList,
    MaterialShaderLoad,
    MaterialShaderSave,
    EntityStreamOpen,
    SceneOperationApply,
    RuntimeDiagnosticsList,
    AccountLogin,
    AccountRegister,
    AccountLogout,
    AuthTicketRequest,
    AuthAccountTicketRequest,
    BackendAuthSessionGet,
}

impl FunHostCommand {
    #[must_use]
    pub const fn wire_id(self) -> &'static str {
        match self {
            Self::HostSnapshotGet => HOST_SNAPSHOT_GET,
            Self::HostCommandsList => HOST_COMMANDS_LIST,
            Self::HostCommandbarExecute => HOST_COMMANDBAR_EXECUTE,
            Self::LauncherStateGet => LAUNCHER_STATE_GET,
            Self::LauncherShow => LAUNCHER_SHOW,
            Self::LauncherHide => LAUNCHER_HIDE,
            Self::GamesList => GAMES_LIST,
            Self::ProjectsAuthorizedList => PROJECTS_AUTHORIZED_LIST,
            Self::ProjectEditOpen => PROJECT_EDIT_OPEN,
            Self::EditorActivate => EDITOR_ACTIVATE,
            Self::EditorDeactivate => EDITOR_DEACTIVATE,
            Self::EditorOverlayToggle => EDITOR_OVERLAY_TOGGLE,
            Self::RuntimeHostStatus => RUNTIME_HOST_STATUS,
            Self::RuntimeStatusGet => RUNTIME_STATUS_GET,
            Self::RuntimeInputSetOwner => RUNTIME_INPUT_SET_OWNER,
            Self::RuntimeServerLaunch => RUNTIME_SERVER_LAUNCH,
            Self::GamesJoin => GAMES_JOIN,
            Self::ViewportClientLaunch => VIEWPORT_CLIENT_LAUNCH,
            Self::ViewportClientStop => VIEWPORT_CLIENT_STOP,
            Self::ViewportClientFocus => VIEWPORT_CLIENT_FOCUS,
            Self::ViewportClientResize => VIEWPORT_CLIENT_RESIZE,
            Self::PreviewViewportEnsure => PREVIEW_VIEWPORT_ENSURE,
            Self::PreviewViewportStop => PREVIEW_VIEWPORT_STOP,
            Self::ProjectOpen => PROJECT_OPEN,
            Self::PreviewRendererEnsure => PREVIEW_RENDERER_ENSURE,
            Self::PreviewRendererResize => PREVIEW_RENDERER_RESIZE,
            Self::PreviewRendererFrameGet => PREVIEW_RENDERER_FRAME_GET,
            Self::PreviewRendererSceneSet => PREVIEW_RENDERER_SCENE_SET,
            Self::PreviewRendererStatusGet => PREVIEW_RENDERER_STATUS_GET,
            Self::MaterialShaderList => MATERIAL_SHADER_LIST,
            Self::MaterialShaderLoad => MATERIAL_SHADER_LOAD,
            Self::MaterialShaderSave => MATERIAL_SHADER_SAVE,
            Self::EntityStreamOpen => ENTITY_STREAM_OPEN,
            Self::SceneOperationApply => SCENE_OPERATION_APPLY,
            Self::RuntimeDiagnosticsList => RUNTIME_DIAGNOSTICS_LIST,
            Self::AccountLogin => ACCOUNT_LOGIN,
            Self::AccountRegister => ACCOUNT_REGISTER,
            Self::AccountLogout => ACCOUNT_LOGOUT,
            Self::AuthTicketRequest => AUTH_TICKET_REQUEST,
            Self::AuthAccountTicketRequest => AUTH_ACCOUNT_TICKET_REQUEST,
            Self::BackendAuthSessionGet => BACKEND_AUTH_SESSION_GET,
        }
    }

    #[must_use]
    pub fn from_wire_id(value: &str) -> Option<Self> {
        match value {
            HOST_SNAPSHOT_GET => Some(Self::HostSnapshotGet),
            HOST_COMMANDS_LIST | "editor.commands.list" => Some(Self::HostCommandsList),
            HOST_COMMANDBAR_EXECUTE => Some(Self::HostCommandbarExecute),
            LAUNCHER_STATE_GET => Some(Self::LauncherStateGet),
            LAUNCHER_SHOW => Some(Self::LauncherShow),
            LAUNCHER_HIDE => Some(Self::LauncherHide),
            GAMES_LIST => Some(Self::GamesList),
            PROJECTS_AUTHORIZED_LIST => Some(Self::ProjectsAuthorizedList),
            PROJECT_EDIT_OPEN => Some(Self::ProjectEditOpen),
            EDITOR_ACTIVATE => Some(Self::EditorActivate),
            EDITOR_DEACTIVATE => Some(Self::EditorDeactivate),
            EDITOR_OVERLAY_TOGGLE => Some(Self::EditorOverlayToggle),
            RUNTIME_HOST_STATUS => Some(Self::RuntimeHostStatus),
            RUNTIME_STATUS_GET => Some(Self::RuntimeStatusGet),
            RUNTIME_INPUT_SET_OWNER => Some(Self::RuntimeInputSetOwner),
            RUNTIME_SERVER_LAUNCH => Some(Self::RuntimeServerLaunch),
            GAMES_JOIN => Some(Self::GamesJoin),
            VIEWPORT_CLIENT_LAUNCH => Some(Self::ViewportClientLaunch),
            VIEWPORT_CLIENT_STOP => Some(Self::ViewportClientStop),
            VIEWPORT_CLIENT_FOCUS => Some(Self::ViewportClientFocus),
            VIEWPORT_CLIENT_RESIZE => Some(Self::ViewportClientResize),
            PREVIEW_VIEWPORT_ENSURE => Some(Self::PreviewViewportEnsure),
            PREVIEW_VIEWPORT_STOP => Some(Self::PreviewViewportStop),
            PROJECT_OPEN => Some(Self::ProjectOpen),
            PREVIEW_RENDERER_ENSURE => Some(Self::PreviewRendererEnsure),
            PREVIEW_RENDERER_RESIZE => Some(Self::PreviewRendererResize),
            PREVIEW_RENDERER_FRAME_GET => Some(Self::PreviewRendererFrameGet),
            PREVIEW_RENDERER_SCENE_SET => Some(Self::PreviewRendererSceneSet),
            PREVIEW_RENDERER_STATUS_GET => Some(Self::PreviewRendererStatusGet),
            MATERIAL_SHADER_LIST => Some(Self::MaterialShaderList),
            MATERIAL_SHADER_LOAD => Some(Self::MaterialShaderLoad),
            MATERIAL_SHADER_SAVE => Some(Self::MaterialShaderSave),
            ENTITY_STREAM_OPEN => Some(Self::EntityStreamOpen),
            SCENE_OPERATION_APPLY => Some(Self::SceneOperationApply),
            RUNTIME_DIAGNOSTICS_LIST => Some(Self::RuntimeDiagnosticsList),
            ACCOUNT_LOGIN => Some(Self::AccountLogin),
            ACCOUNT_REGISTER => Some(Self::AccountRegister),
            ACCOUNT_LOGOUT | AUTH_LOGOUT => Some(Self::AccountLogout),
            AUTH_TICKET_REQUEST => Some(Self::AuthTicketRequest),
            AUTH_ACCOUNT_TICKET_REQUEST => Some(Self::AuthAccountTicketRequest),
            BACKEND_AUTH_SESSION_GET | AUTH_BACKEND_SESSION_GET => {
                Some(Self::BackendAuthSessionGet)
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunHostCommandDescriptor {
    pub id: &'static str,
    pub category: FunHostCommandCategory,
    pub summary: &'static str,
}

impl Serialize for FunHostCommandDescriptor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("FunHostCommandDescriptor", 10)?;
        state.serialize_field("id", self.id)?;
        state.serialize_field("title", command_title(self.id))?;
        state.serialize_field("category", &self.category)?;
        state.serialize_field("summary", self.summary)?;
        state.serialize_field("input_description", command_input_description(self.id))?;
        state.serialize_field("output_description", command_output_description(self.id))?;
        state.serialize_field("payload_schema_id", command_payload_schema_id(self.id))?;
        state.serialize_field("required_capability", command_required_capability(self.id))?;
        state.serialize_field("risk", &command_risk(self.id))?;
        state.serialize_field("rust_owner", "fun_host")?;
        state.end()
    }
}

pub const FUN_HOST_COMMANDS: &[FunHostCommandDescriptor] = &[
    FunHostCommandDescriptor {
        id: HOST_COMMANDS_LIST,
        category: FunHostCommandCategory::Host,
        summary: "Returns Rust-owned command descriptors for commandbar search and execution validation.",
    },
    FunHostCommandDescriptor {
        id: HOST_COMMANDBAR_EXECUTE,
        category: FunHostCommandCategory::Host,
        summary: "Validates a commandbar action through Rust before Svelte applies local visual follow-through.",
    },
    FunHostCommandDescriptor {
        id: HOST_SNAPSHOT_GET,
        category: FunHostCommandCategory::Host,
        summary: "Returns the canonical Rust-owned FunHostState snapshot.",
    },
    FunHostCommandDescriptor {
        id: LAUNCHER_STATE_GET,
        category: FunHostCommandCategory::Launcher,
        summary: "Returns the unified FunClientHost state.",
    },
    FunHostCommandDescriptor {
        id: LAUNCHER_SHOW,
        category: FunHostCommandCategory::Launcher,
        summary: "Switches the native UI surface to launcher mode.",
    },
    FunHostCommandDescriptor {
        id: LAUNCHER_HIDE,
        category: FunHostCommandCategory::Launcher,
        summary: "Returns from launcher mode to the game surface.",
    },
    FunHostCommandDescriptor {
        id: GAMES_LIST,
        category: FunHostCommandCategory::Game,
        summary: "Lists games visible to the Rust-owned launcher service.",
    },
    FunHostCommandDescriptor {
        id: PROJECTS_AUTHORIZED_LIST,
        category: FunHostCommandCategory::Project,
        summary: "Lists projects authorized for local editor access.",
    },
    FunHostCommandDescriptor {
        id: PROJECT_EDIT_OPEN,
        category: FunHostCommandCategory::Project,
        summary: "Opens an authorized project in editor mode.",
    },
    FunHostCommandDescriptor {
        id: EDITOR_ACTIVATE,
        category: FunHostCommandCategory::Editor,
        summary: "Activates the editor surface inside the unified client host.",
    },
    FunHostCommandDescriptor {
        id: EDITOR_DEACTIVATE,
        category: FunHostCommandCategory::Editor,
        summary: "Leaves editor mode and returns to the game surface when allowed.",
    },
    FunHostCommandDescriptor {
        id: EDITOR_OVERLAY_TOGGLE,
        category: FunHostCommandCategory::Editor,
        summary: "Toggles the local in-process editor overlay for the current client.",
    },
    FunHostCommandDescriptor {
        id: RUNTIME_HOST_STATUS,
        category: FunHostCommandCategory::Runtime,
        summary: "Returns runtime lifecycle and service status.",
    },
    FunHostCommandDescriptor {
        id: RUNTIME_STATUS_GET,
        category: FunHostCommandCategory::Runtime,
        summary: "Returns the unified host runtime status without assuming a child client.",
    },
    FunHostCommandDescriptor {
        id: RUNTIME_INPUT_SET_OWNER,
        category: FunHostCommandCategory::Runtime,
        summary: "Sets Rust-owned input ownership for gameplay, launcher, editor, menu, text, or commandbar focus.",
    },
    FunHostCommandDescriptor {
        id: RUNTIME_SERVER_LAUNCH,
        category: FunHostCommandCategory::Runtime,
        summary: "Routes server lifecycle control through the unified host; client process control is not spawned here.",
    },
    FunHostCommandDescriptor {
        id: GAMES_JOIN,
        category: FunHostCommandCategory::Game,
        summary: "Switches the already-running unified client into game mode for the selected game.",
    },
    FunHostCommandDescriptor {
        id: VIEWPORT_CLIENT_LAUNCH,
        category: FunHostCommandCategory::Runtime,
        summary: "Compatibility command that switches the unified host to game mode instead of launching a child client.",
    },
    FunHostCommandDescriptor {
        id: VIEWPORT_CLIENT_FOCUS,
        category: FunHostCommandCategory::Runtime,
        summary: "Compatibility command that releases UI input to gameplay in the unified client.",
    },
    FunHostCommandDescriptor {
        id: VIEWPORT_CLIENT_RESIZE,
        category: FunHostCommandCategory::Runtime,
        summary: "Updates the native UI/game viewport layout state without resizing a child window.",
    },
    FunHostCommandDescriptor {
        id: VIEWPORT_CLIENT_STOP,
        category: FunHostCommandCategory::Runtime,
        summary: "Compatibility no-op because the game client is the current host process.",
    },
    FunHostCommandDescriptor {
        id: PREVIEW_VIEWPORT_ENSURE,
        category: FunHostCommandCategory::Preview,
        summary: "Compatibility command that uses the current client render scene as the editor preview.",
    },
    FunHostCommandDescriptor {
        id: PREVIEW_VIEWPORT_STOP,
        category: FunHostCommandCategory::Preview,
        summary: "Compatibility no-op for the in-host current-client preview.",
    },
    FunHostCommandDescriptor {
        id: PROJECT_OPEN,
        category: FunHostCommandCategory::Project,
        summary: "Indexes and opens a project inside Rust host services.",
    },
    FunHostCommandDescriptor {
        id: PREVIEW_RENDERER_ENSURE,
        category: FunHostCommandCategory::Preview,
        summary: "Binds editor preview controls to the current client render state.",
    },
    FunHostCommandDescriptor {
        id: PREVIEW_RENDERER_RESIZE,
        category: FunHostCommandCategory::Preview,
        summary: "Updates the current-client preview layout; no offscreen child renderer is resized.",
    },
    FunHostCommandDescriptor {
        id: PREVIEW_RENDERER_FRAME_GET,
        category: FunHostCommandCategory::Preview,
        summary: "Returns current-client preview state metadata without rendering pixels in Svelte.",
    },
    FunHostCommandDescriptor {
        id: PREVIEW_RENDERER_SCENE_SET,
        category: FunHostCommandCategory::Preview,
        summary: "Selects the scene for the current client/editor preview state.",
    },
    FunHostCommandDescriptor {
        id: PREVIEW_RENDERER_STATUS_GET,
        category: FunHostCommandCategory::Preview,
        summary: "Returns current-client preview state metadata.",
    },
    FunHostCommandDescriptor {
        id: MATERIAL_SHADER_LIST,
        category: FunHostCommandCategory::Material,
        summary: "Lists WGSL material shader documents visible to the Rust-owned editor service.",
    },
    FunHostCommandDescriptor {
        id: MATERIAL_SHADER_LOAD,
        category: FunHostCommandCategory::Material,
        summary: "Loads a project-owned WGSL material shader document through Rust authority.",
    },
    FunHostCommandDescriptor {
        id: MATERIAL_SHADER_SAVE,
        category: FunHostCommandCategory::Material,
        summary: "Persists a project-owned WGSL material shader document through Rust authority.",
    },
    FunHostCommandDescriptor {
        id: ENTITY_STREAM_OPEN,
        category: FunHostCommandCategory::Editor,
        summary: "Opens a bounded editor entity stream.",
    },
    FunHostCommandDescriptor {
        id: SCENE_OPERATION_APPLY,
        category: FunHostCommandCategory::Editor,
        summary: "Queues validated typed scene operations for fun-scene to apply to RetiredEngine ECS.",
    },
    FunHostCommandDescriptor {
        id: RUNTIME_DIAGNOSTICS_LIST,
        category: FunHostCommandCategory::Runtime,
        summary: "Lists editor-visible runtime diagnostics.",
    },
    FunHostCommandDescriptor {
        id: ACCOUNT_LOGIN,
        category: FunHostCommandCategory::Auth,
        summary: "Submits credentials to the Rust-owned backend account service and stores only redacted UI state.",
    },
    FunHostCommandDescriptor {
        id: ACCOUNT_REGISTER,
        category: FunHostCommandCategory::Auth,
        summary: "Registers an account through Rust-owned backend account authority and keeps raw tokens out of Svelte state.",
    },
    FunHostCommandDescriptor {
        id: ACCOUNT_LOGOUT,
        category: FunHostCommandCategory::Auth,
        summary: "Clears the Rust-owned backend account session and returns redacted session state.",
    },
    FunHostCommandDescriptor {
        id: AUTH_TICKET_REQUEST,
        category: FunHostCommandCategory::Auth,
        summary: "Requests a scoped backend ticket without exposing secrets to UI state.",
    },
    FunHostCommandDescriptor {
        id: BACKEND_AUTH_SESSION_GET,
        category: FunHostCommandCategory::Backend,
        summary: "Returns only redacted backend account session and profile state.",
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FunHostCommandRiskLevel {
    Read,
    UiNavigation,
    RuntimeMutation,
    Filesystem,
    Network,
    Account,
    ToolStub,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FunHostCommandRiskMetadata {
    pub level: FunHostCommandRiskLevel,
    pub mutates_state: bool,
    pub requires_confirmation: bool,
    pub filesystem_access: bool,
    pub network_access: bool,
    pub tool_stub: bool,
}

fn command_title(id: &str) -> &'static str {
    match id {
        HOST_COMMANDS_LIST => "List Host Commands",
        HOST_COMMANDBAR_EXECUTE => "Execute Commandbar Action",
        HOST_SNAPSHOT_GET => "Get Host Snapshot",
        LAUNCHER_STATE_GET => "Get Launcher State",
        LAUNCHER_SHOW => "Show Launcher",
        LAUNCHER_HIDE => "Hide Launcher",
        GAMES_LIST => "List Games",
        GAMES_JOIN => "Join Game",
        PROJECTS_AUTHORIZED_LIST => "List Authorized Projects",
        PROJECT_EDIT_OPEN => "Open Project Editor",
        PROJECT_OPEN => "Open Project",
        EDITOR_ACTIVATE => "Activate Editor",
        EDITOR_DEACTIVATE => "Deactivate Editor",
        EDITOR_OVERLAY_TOGGLE => "Toggle Editor Overlay",
        RUNTIME_HOST_STATUS => "Get Runtime Host Status",
        RUNTIME_STATUS_GET => "Get Runtime Status",
        RUNTIME_INPUT_SET_OWNER => "Set Runtime Input Owner",
        RUNTIME_SERVER_LAUNCH => "Launch Server Runtime",
        VIEWPORT_CLIENT_LAUNCH => "Play Current Client",
        VIEWPORT_CLIENT_STOP => "Clear Client Focus",
        VIEWPORT_CLIENT_FOCUS => "Focus Current Client",
        VIEWPORT_CLIENT_RESIZE => "Resize Client Viewport",
        PREVIEW_VIEWPORT_ENSURE => "Ensure Preview Viewport",
        PREVIEW_VIEWPORT_STOP => "Stop Preview Viewport",
        PREVIEW_RENDERER_ENSURE => "Ensure Current Client Preview",
        PREVIEW_RENDERER_RESIZE => "Resize Current Client Preview",
        PREVIEW_RENDERER_FRAME_GET => "Get Preview Frame",
        PREVIEW_RENDERER_SCENE_SET => "Set Preview Scene",
        PREVIEW_RENDERER_STATUS_GET => "Get Preview Status",
        MATERIAL_SHADER_LIST => "List Material Shaders",
        MATERIAL_SHADER_LOAD => "Load Material Shader",
        MATERIAL_SHADER_SAVE => "Save Material Shader",
        ENTITY_STREAM_OPEN => "Open Entity Stream",
        SCENE_OPERATION_APPLY => "Apply Scene Operation",
        RUNTIME_DIAGNOSTICS_LIST => "List Runtime Diagnostics",
        ACCOUNT_LOGIN => "Account Login",
        ACCOUNT_REGISTER => "Account Register",
        ACCOUNT_LOGOUT => "Account Logout",
        AUTH_TICKET_REQUEST => "Request Backend Ticket",
        BACKEND_AUTH_SESSION_GET => "Get Backend Auth Session",
        _ => "Host Command",
    }
}

fn command_input_description(id: &str) -> &'static str {
    match id {
        HOST_COMMANDS_LIST
        | HOST_SNAPSHOT_GET
        | LAUNCHER_STATE_GET
        | GAMES_LIST
        | PROJECTS_AUTHORIZED_LIST
        | RUNTIME_HOST_STATUS
        | RUNTIME_STATUS_GET
        | PREVIEW_RENDERER_FRAME_GET
        | PREVIEW_RENDERER_STATUS_GET
        | RUNTIME_DIAGNOSTICS_LIST
        | BACKEND_AUTH_SESSION_GET
        | ACCOUNT_LOGOUT => "No input.",
        HOST_COMMANDBAR_EXECUTE => "CommandbarExecutePayload.",
        GAMES_JOIN => "Game/session selection payload.",
        PROJECT_OPEN | PROJECT_EDIT_OPEN => "Project ID or project path payload.",
        RUNTIME_INPUT_SET_OWNER => "RuntimeInputOwnerPayload.",
        RUNTIME_SERVER_LAUNCH => "Project/server runtime launch payload.",
        VIEWPORT_CLIENT_RESIZE | PREVIEW_RENDERER_RESIZE => "Viewport rectangle payload.",
        VIEWPORT_CLIENT_LAUNCH
        | VIEWPORT_CLIENT_FOCUS
        | VIEWPORT_CLIENT_STOP
        | PREVIEW_VIEWPORT_ENSURE
        | PREVIEW_VIEWPORT_STOP
        | PREVIEW_RENDERER_ENSURE
        | PREVIEW_RENDERER_SCENE_SET => "Project, scene, or current-client payload.",
        MATERIAL_SHADER_LIST => "Project ID.",
        MATERIAL_SHADER_LOAD => "MaterialShaderLoadRequest.",
        MATERIAL_SHADER_SAVE => "MaterialShaderSaveRequest.",
        ENTITY_STREAM_OPEN => "EntityStreamOpenRequest.",
        SCENE_OPERATION_APPLY => "fun_scene::EditorOperationEnvelope or one EditorOperation.",
        ACCOUNT_LOGIN | ACCOUNT_REGISTER => "BackendAccountTicketLoginRequest.",
        AUTH_TICKET_REQUEST => "BackendAuthTicketRequest.",
        _ => "JSON payload.",
    }
}

fn command_output_description(id: &str) -> &'static str {
    match id {
        HOST_COMMANDS_LIST => "HostCommandDescriptor[].",
        HOST_COMMANDBAR_EXECUTE => "CommandResult<CommandbarExecutionPayload>.",
        ACCOUNT_LOGIN | ACCOUNT_REGISTER | AUTH_TICKET_REQUEST => {
            "CommandResult<BackendAuthTicket>."
        }
        ACCOUNT_LOGOUT => "CommandResult<BackendAuthSessionState>.",
        BACKEND_AUTH_SESSION_GET => "BackendAuthSessionState.",
        RUNTIME_SERVER_LAUNCH => "CommandResult<RuntimeInstanceSummary>.",
        MATERIAL_SHADER_LIST => "CommandResult<MaterialShaderCatalog>.",
        MATERIAL_SHADER_LOAD | MATERIAL_SHADER_SAVE => "CommandResult<MaterialShaderDocument>.",
        SCENE_OPERATION_APPLY => "CommandResult<SceneOperationReceipt>.",
        _ => "FunClientHostSnapshot.",
    }
}

fn command_payload_schema_id(id: &str) -> &'static str {
    match id {
        HOST_COMMANDS_LIST => "fun.host.commands.list.v1",
        HOST_COMMANDBAR_EXECUTE => "fun.host.commandbar.execute.v1",
        HOST_SNAPSHOT_GET => "fun.host.snapshot.get.v1",
        ACCOUNT_LOGIN => "fun.account.login.v1",
        ACCOUNT_REGISTER => "fun.account.register.v1",
        ACCOUNT_LOGOUT => "fun.account.logout.v1",
        AUTH_TICKET_REQUEST => "fun.auth.ticket.request.v1",
        BACKEND_AUTH_SESSION_GET => "fun.backend.auth.session.get.v1",
        RUNTIME_INPUT_SET_OWNER => "fun.runtime.input.owner.v1",
        RUNTIME_SERVER_LAUNCH => "fun.runtime.server.launch.v1",
        GAMES_JOIN => "fun.games.join.v1",
        VIEWPORT_CLIENT_RESIZE | PREVIEW_RENDERER_RESIZE => "fun.viewport.rect.v1",
        MATERIAL_SHADER_LIST => "fun.material.shader.list.v1",
        MATERIAL_SHADER_LOAD => "fun.material.shader.load.v1",
        MATERIAL_SHADER_SAVE => "fun.material.shader.save.v1",
        SCENE_OPERATION_APPLY => "fun.scene.editor.operation.v1",
        _ => "fun.host.command.payload.v1",
    }
}

fn command_required_capability(id: &str) -> &'static str {
    match id {
        GAMES_JOIN => "JoinGame",
        PROJECT_OPEN
        | PROJECT_EDIT_OPEN
        | EDITOR_ACTIVATE
        | EDITOR_DEACTIVATE
        | EDITOR_OVERLAY_TOGGLE => "EditProject",
        PROJECTS_AUTHORIZED_LIST => "OpenProject",
        ENTITY_STREAM_OPEN => "ReadEntities",
        SCENE_OPERATION_APPLY => "EditProject",
        RUNTIME_DIAGNOSTICS_LIST => "ReadDiagnostics",
        MATERIAL_SHADER_LIST | MATERIAL_SHADER_LOAD => "MaterialShaderRead",
        MATERIAL_SHADER_SAVE => "MaterialShaderWrite",
        ACCOUNT_LOGIN | ACCOUNT_REGISTER | ACCOUNT_LOGOUT | AUTH_TICKET_REQUEST => {
            "RequestBackendTicket"
        }
        RUNTIME_HOST_STATUS
        | RUNTIME_STATUS_GET
        | RUNTIME_INPUT_SET_OWNER
        | RUNTIME_SERVER_LAUNCH
        | VIEWPORT_CLIENT_LAUNCH
        | VIEWPORT_CLIENT_STOP
        | VIEWPORT_CLIENT_FOCUS
        | VIEWPORT_CLIENT_RESIZE
        | PREVIEW_VIEWPORT_ENSURE
        | PREVIEW_VIEWPORT_STOP
        | PREVIEW_RENDERER_ENSURE
        | PREVIEW_RENDERER_RESIZE
        | PREVIEW_RENDERER_FRAME_GET
        | PREVIEW_RENDERER_SCENE_SET
        | PREVIEW_RENDERER_STATUS_GET
        | HOST_COMMANDBAR_EXECUTE => "ControlRuntime",
        _ => "ReadLauncher",
    }
}

fn command_risk(id: &str) -> FunHostCommandRiskMetadata {
    let level = match id {
        ACCOUNT_LOGIN | ACCOUNT_REGISTER | ACCOUNT_LOGOUT | AUTH_TICKET_REQUEST => {
            FunHostCommandRiskLevel::Account
        }
        PROJECT_OPEN | MATERIAL_SHADER_SAVE => FunHostCommandRiskLevel::Filesystem,
        RUNTIME_SERVER_LAUNCH
        | GAMES_JOIN
        | VIEWPORT_CLIENT_LAUNCH
        | VIEWPORT_CLIENT_STOP
        | VIEWPORT_CLIENT_FOCUS
        | VIEWPORT_CLIENT_RESIZE
        | RUNTIME_INPUT_SET_OWNER
        | PREVIEW_RENDERER_ENSURE
        | PREVIEW_RENDERER_RESIZE
        | PREVIEW_RENDERER_SCENE_SET
        | SCENE_OPERATION_APPLY
        | HOST_COMMANDBAR_EXECUTE => FunHostCommandRiskLevel::RuntimeMutation,
        LAUNCHER_SHOW
        | LAUNCHER_HIDE
        | PROJECT_EDIT_OPEN
        | EDITOR_ACTIVATE
        | EDITOR_DEACTIVATE
        | EDITOR_OVERLAY_TOGGLE
        | PREVIEW_VIEWPORT_ENSURE
        | PREVIEW_VIEWPORT_STOP => FunHostCommandRiskLevel::UiNavigation,
        _ => FunHostCommandRiskLevel::Read,
    };
    FunHostCommandRiskMetadata {
        level,
        mutates_state: !matches!(level, FunHostCommandRiskLevel::Read),
        requires_confirmation: matches!(
            level,
            FunHostCommandRiskLevel::Filesystem
                | FunHostCommandRiskLevel::Network
                | FunHostCommandRiskLevel::Account
                | FunHostCommandRiskLevel::ToolStub
        ),
        filesystem_access: matches!(level, FunHostCommandRiskLevel::Filesystem),
        network_access: matches!(
            level,
            FunHostCommandRiskLevel::Network | FunHostCommandRiskLevel::Account
        ),
        tool_stub: matches!(level, FunHostCommandRiskLevel::ToolStub),
    }
}

#[must_use]
pub fn host_command_descriptor(id: &str) -> Option<&'static FunHostCommandDescriptor> {
    FUN_HOST_COMMANDS
        .iter()
        .find(|descriptor| descriptor.id == id)
}

#[derive(Debug, Clone, PartialEq, Eq, Message)]
pub struct FunHostCommandRequest {
    pub request_id: u64,
    pub command_id: String,
    pub payload_json: Vec<u8>,
}

impl FunHostCommandRequest {
    #[must_use]
    pub fn new(request_id: u64, command_id: String, payload_json: Vec<u8>) -> Self {
        Self {
            request_id,
            command_id,
            payload_json,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Message)]
pub struct FunHostCommandResponse {
    pub request_id: u64,
    pub sequence: u64,
    pub command_id: String,
    pub status: FunHostCommandStatus,
    pub error_code: Option<FunHostCommandErrorCode>,
    pub payload_json: Vec<u8>,
}

impl FunHostCommandResponse {
    #[must_use]
    pub fn ok(request: &FunHostCommandRequest, sequence: u64, payload_json: Vec<u8>) -> Self {
        Self {
            request_id: request.request_id,
            sequence,
            command_id: request.command_id.clone(),
            status: FunHostCommandStatus::Ok,
            error_code: None,
            payload_json,
        }
    }

    #[must_use]
    pub fn error(
        request: &FunHostCommandRequest,
        sequence: u64,
        error_code: FunHostCommandErrorCode,
    ) -> Self {
        let payload_json = encode_payload(&HostErrorPayload { error_code })
            .unwrap_or_else(|_| b"{\"error_code\":\"response_encoding_failed\"}".to_vec());
        Self {
            request_id: request.request_id,
            sequence,
            command_id: request.command_id.clone(),
            status: FunHostCommandStatus::Error,
            error_code: Some(error_code),
            payload_json,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FunHostCommandStatus {
    Ok,
    Error,
}

impl FunHostCommandStatus {
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FunHostCommandErrorCode {
    UnknownCommand,
    OversizePayload,
    InvalidPayload,
    ProjectUnauthorized,
    ReturnToGameUnavailable,
    BackendSessionRequired,
    HostShuttingDown,
    ResponseEncodingFailed,
}

impl FunHostCommandErrorCode {
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::UnknownCommand => "unknown_command",
            Self::OversizePayload => "oversize_payload",
            Self::InvalidPayload => "invalid_payload",
            Self::ProjectUnauthorized => "project_unauthorized",
            Self::ReturnToGameUnavailable => "return_to_game_unavailable",
            Self::BackendSessionRequired => "backend_session_required",
            Self::HostShuttingDown => "host_shutting_down",
            Self::ResponseEncodingFailed => "response_encoding_failed",
        }
    }
}

#[derive(Debug, Serialize)]
struct HostSnapshotPayload<'a> {
    snapshot: FunClientHostSnapshot<'a>,
    command_catalog: &'static [FunHostCommandDescriptor],
    editor_command_catalog: &'static [fun_editor_core::FunEditorCommandDescriptor],
}

#[derive(Debug, Serialize)]
struct HostErrorPayload {
    error_code: FunHostCommandErrorCode,
}

#[derive(Debug, Deserialize)]
struct CommandbarExecutePayload {
    #[serde(alias = "commandId")]
    command_id: String,
    #[serde(default)]
    payload: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct CommandbarExecutionPayload<'a> {
    accepted_command_id: &'a str,
    snapshot: FunClientHostSnapshot<'a>,
}

#[derive(Debug, Default, Deserialize)]
struct GameSessionPayload {
    #[serde(default, alias = "gameId")]
    game_id: Option<String>,
    #[serde(default, alias = "gameSessionId")]
    game_session_id: Option<String>,
    #[serde(default, alias = "sessionId")]
    session_id: Option<String>,
    #[serde(default, alias = "projectId")]
    project_id: Option<String>,
    #[serde(default, alias = "projectPath")]
    project_path: Option<String>,
    #[serde(default, alias = "serverAddr")]
    server_addr: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GameSessionEnvelope {
    request: GameSessionPayload,
}

#[derive(Debug, Deserialize)]
struct RuntimeInputOwnerPayload {
    owner: Option<FunInputOwner>,
}

#[derive(Debug, Deserialize)]
struct ViewportLayoutPayload {
    rect: Option<ViewportRectPayload>,
}

#[derive(Debug, Deserialize)]
struct ViewportRectPayload {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    scale_factor: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
struct RequestEnvelope<T> {
    request: T,
}

#[derive(Debug, Default, Deserialize)]
struct MaterialShaderListPayload {
    #[serde(default, alias = "projectId")]
    project_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct MaterialShaderPathPayload {
    #[serde(default, alias = "projectId")]
    project_id: Option<String>,
    #[serde(default, alias = "relativePath")]
    relative_path: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct MaterialShaderSavePayload {
    #[serde(default, alias = "projectId")]
    project_id: Option<String>,
    #[serde(default, alias = "relativePath")]
    relative_path: Option<String>,
    #[serde(default)]
    source: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct MaterialShaderSummary {
    id: String,
    name: String,
    relative_path: String,
    absolute_path: String,
    byte_len: u64,
    modified_unix_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
struct MaterialShaderCatalog {
    project_id: String,
    project_root: String,
    shader_roots: Vec<String>,
    default_relative_directory: &'static str,
    shaders: Vec<MaterialShaderSummary>,
}

#[derive(Debug, Clone, Serialize)]
struct MaterialShaderDocument {
    summary: MaterialShaderSummary,
    source: String,
    diagnostics: Vec<account::Diagnostic>,
}

#[derive(Debug, Clone, Serialize)]
struct SceneOperationReceipt {
    schema_version: u16,
    accepted_operation_count: usize,
    queued_for_fun_scene: bool,
    host_applies_through_fun_scene: bool,
    dynamic_rust_expressions_allowed: bool,
}

fn encode_payload<T: Serialize>(payload: &T) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(payload)
}

fn commandbar_rejection<T>(code: &str, message: &str) -> account::CommandResult<T> {
    account::CommandResult {
        ok: false,
        value: None,
        diagnostics: vec![account::Diagnostic {
            code: code.to_owned(),
            level: account::EventLevel::Warning,
            message: message.to_owned(),
            target_path: None,
            hosted_instance_id: Some(HOST_COMMANDBAR_EXECUTE.to_owned()),
        }],
    }
}

fn scene_operation_rejection<T>(code: &str) -> account::CommandResult<T> {
    account::CommandResult {
        ok: false,
        value: None,
        diagnostics: vec![account::Diagnostic {
            code: code.to_owned(),
            level: account::EventLevel::Warning,
            message: String::from("Scene operation payload did not match the fun-scene contract."),
            target_path: None,
            hosted_instance_id: Some(SCENE_OPERATION_APPLY.to_owned()),
        }],
    }
}

fn commandbar_tool_stub_is_gated(command_id: &str) -> bool {
    command_id.starts_with("backend.")
        || command_id.starts_with("llm.")
        || command_id.starts_with("mcp.")
}

fn commandbar_read_tool_is_allowed(command_id: &str) -> bool {
    matches!(
        command_id,
        "project.current.get"
            | "projects.recent.list"
            | "project.bsn.index"
            | "entity.search"
            | "entity.details.get"
            | "runtime.status.get"
            | "runtime.diagnostics.list"
            | "preview.renderer.status.get"
            | "material.shader.list"
            | "editor.events.list"
    )
}

fn parse_game_session_payload(payload_json: &[u8]) -> Option<GameSessionPayload> {
    if payload_json.is_empty() {
        return None;
    }
    serde_json::from_slice::<GameSessionEnvelope>(payload_json)
        .ok()
        .map(|envelope| envelope.request)
        .or_else(|| {
            serde_json::from_slice::<Option<GameSessionPayload>>(payload_json)
                .ok()
                .flatten()
        })
}

fn project_id_from_path(path: &str) -> Option<String> {
    std::path::Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .map(String::from)
}

fn parse_input_owner(payload_json: &[u8]) -> Option<FunInputOwner> {
    if payload_json.is_empty() {
        return None;
    }
    serde_json::from_slice::<Option<RuntimeInputOwnerPayload>>(payload_json)
        .ok()
        .flatten()
        .and_then(|payload| payload.owner)
}

fn parse_viewport_rect(payload_json: &[u8]) -> Option<FunViewportRect> {
    if payload_json.is_empty() {
        return None;
    }
    let payload = serde_json::from_slice::<ViewportLayoutPayload>(payload_json).ok()?;
    let rect = payload.rect?;
    Some(FunViewportRect {
        x: saturating_i32_from_f64(rect.x),
        y: saturating_i32_from_f64(rect.y),
        width: positive_u32_from_f64(rect.width),
        height: positive_u32_from_f64(rect.height),
        scale_factor_milli: scale_factor_milli(rect.scale_factor.unwrap_or(1.0)),
    })
}

fn parse_request_payload<T>(payload_json: &[u8]) -> T
where
    T: DeserializeOwned + Default,
{
    if payload_json.is_empty() {
        return T::default();
    }
    serde_json::from_slice::<RequestEnvelope<T>>(payload_json)
        .ok()
        .map(|envelope| envelope.request)
        .or_else(|| serde_json::from_slice::<T>(payload_json).ok())
        .unwrap_or_default()
}

fn parse_scene_operation_envelope(
    payload_json: &[u8],
) -> Result<fun_scene::EditorOperationEnvelope, serde_json::Error> {
    serde_json::from_slice::<RequestEnvelope<fun_scene::EditorOperationEnvelope>>(payload_json)
        .map(|envelope| envelope.request)
        .or_else(|_| serde_json::from_slice::<fun_scene::EditorOperationEnvelope>(payload_json))
        .or_else(|_| {
            serde_json::from_slice::<fun_scene::EditorOperation>(payload_json)
                .map(fun_scene::EditorOperationEnvelope::single)
        })
}

fn material_shader_catalog(
    project: &ProjectState,
    payload: MaterialShaderListPayload,
) -> MaterialShaderCatalog {
    let project_id = payload
        .project_id
        .or_else(|| project.active_project_id.clone())
        .unwrap_or_else(|| String::from("current-client"));
    let project_root = project.active_project_root.clone().unwrap_or_default();
    let shader_roots = if project_root.is_empty() {
        Vec::new()
    } else {
        vec![format!("{project_root}/assets/shaders/materials")]
    };
    MaterialShaderCatalog {
        project_id,
        project_root,
        shader_roots,
        default_relative_directory: "assets/shaders/materials",
        shaders: Vec::new(),
    }
}

fn material_shader_placeholder_summary(
    project: &ProjectState,
    project_id: Option<String>,
    relative_path: Option<String>,
) -> MaterialShaderSummary {
    let relative_path = relative_path
        .unwrap_or_else(|| String::from("assets/shaders/materials/material_graph.wgsl"));
    let project_root = project.active_project_root.as_deref().unwrap_or_default();
    let absolute_path = if project_root.is_empty() {
        String::new()
    } else {
        format!("{project_root}/{relative_path}")
    };
    MaterialShaderSummary {
        id: project_id
            .or_else(|| project.active_project_id.clone())
            .map_or_else(
                || relative_path.clone(),
                |id| format!("{id}:{relative_path}"),
            ),
        name: relative_path
            .rsplit('/')
            .next()
            .unwrap_or(relative_path.as_str())
            .to_owned(),
        relative_path,
        absolute_path,
        byte_len: 0,
        modified_unix_ms: None,
    }
}

fn material_shader_not_ready_diagnostic(
    command_id: &str,
    target_path: Option<String>,
) -> account::Diagnostic {
    account::Diagnostic {
        code: format!("{command_id}.not_ready"),
        level: account::EventLevel::Warning,
        message: String::from(
            "Material shader filesystem indexing is not active in this host session yet.",
        ),
        target_path,
        hosted_instance_id: Some(command_id.to_owned()),
    }
}

fn saturating_i32_from_f64(value: f64) -> i32 {
    if !value.is_finite() {
        return 0;
    }
    value
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

fn positive_u32_from_f64(value: f64) -> u32 {
    if !value.is_finite() {
        return 1;
    }
    value.round().clamp(1.0, f64::from(u32::MAX)) as u32
}

fn scale_factor_milli(value: f64) -> u32 {
    if !value.is_finite() {
        return 1_000;
    }
    (value.max(0.001) * 1_000.0)
        .round()
        .clamp(1.0, f64::from(u32::MAX)) as u32
}

fn route_fun_host_commands(
    mut host: ResMut<FunClientHostState>,
    mut scene_operations: ResMut<fun_scene::EditorOperationQueue>,
    mut requests: MessageReader<FunHostCommandRequest>,
    mut responses: MessageWriter<FunHostCommandResponse>,
) {
    for request in requests.read() {
        let response = if FunHostCommand::from_wire_id(&request.command_id)
            == Some(FunHostCommand::SceneOperationApply)
        {
            let sequence = host.next_sequence();
            if request.payload_json.len() > MAX_HOST_COMMAND_PAYLOAD_BYTES {
                FunHostCommandResponse::error(
                    &request,
                    sequence,
                    FunHostCommandErrorCode::OversizePayload,
                )
            } else {
                host.scene_operation_apply_response(&request, sequence, &mut scene_operations)
            }
        } else {
            host.handle_command(&request)
        };
        responses.write(response);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(command_id: &str) -> FunHostCommandRequest {
        FunHostCommandRequest::new(7, String::from(command_id), Vec::new())
    }

    #[test]
    fn host_catalog_keeps_dot_command_ids_as_native_ui_surface_ids() {
        for id in [
            HOST_COMMANDS_LIST,
            HOST_COMMANDBAR_EXECUTE,
            HOST_SNAPSHOT_GET,
            LAUNCHER_STATE_GET,
            LAUNCHER_SHOW,
            GAMES_LIST,
            PROJECTS_AUTHORIZED_LIST,
            PROJECT_EDIT_OPEN,
            EDITOR_ACTIVATE,
            EDITOR_OVERLAY_TOGGLE,
            RUNTIME_INPUT_SET_OWNER,
            RUNTIME_HOST_STATUS,
            RUNTIME_STATUS_GET,
            RUNTIME_SERVER_LAUNCH,
            GAMES_JOIN,
            VIEWPORT_CLIENT_LAUNCH,
            VIEWPORT_CLIENT_STOP,
            VIEWPORT_CLIENT_FOCUS,
            VIEWPORT_CLIENT_RESIZE,
            PROJECT_OPEN,
            PREVIEW_VIEWPORT_ENSURE,
            PREVIEW_VIEWPORT_STOP,
            PREVIEW_RENDERER_ENSURE,
            PREVIEW_RENDERER_RESIZE,
            PREVIEW_RENDERER_FRAME_GET,
            PREVIEW_RENDERER_SCENE_SET,
            PREVIEW_RENDERER_STATUS_GET,
            MATERIAL_SHADER_LIST,
            MATERIAL_SHADER_LOAD,
            MATERIAL_SHADER_SAVE,
            ENTITY_STREAM_OPEN,
            SCENE_OPERATION_APPLY,
            RUNTIME_DIAGNOSTICS_LIST,
            ACCOUNT_LOGIN,
            ACCOUNT_REGISTER,
            ACCOUNT_LOGOUT,
            AUTH_TICKET_REQUEST,
            BACKEND_AUTH_SESSION_GET,
        ] {
            assert!(host_command_descriptor(id).is_some(), "missing `{id}`");
            assert!(
                FunHostCommand::from_wire_id(id).is_some(),
                "unparsed `{id}`"
            );
        }
    }

    #[test]
    fn host_commands_list_returns_rich_commandbar_metadata() {
        let mut host = FunClientHostState::default();
        let response = host.handle_command(&request(HOST_COMMANDS_LIST));

        assert_eq!(response.status, FunHostCommandStatus::Ok);
        let payload: serde_json::Value =
            serde_json::from_slice(&response.payload_json).expect("commands list decodes");
        let commands = payload.as_array().expect("command list");
        let commandbar = commands
            .iter()
            .find(|command| command["id"] == HOST_COMMANDBAR_EXECUTE)
            .expect("commandbar execute descriptor");
        assert_eq!(commandbar["title"], "Execute Commandbar Action");
        assert_eq!(
            commandbar["payload_schema_id"],
            "fun.host.commandbar.execute.v1"
        );
        assert_eq!(commandbar["required_capability"], "ControlRuntime");
        assert_eq!(commandbar["risk"]["mutates_state"], true);
    }

    #[test]
    fn scene_operation_command_queues_validated_fun_scene_operations() {
        let mut host = FunClientHostState::default();
        let mut queue = fun_scene::EditorOperationQueue::default();
        let payload = br#"{
            "operation": {
                "op": "patch_component",
                "entity": { "uri": "scene://arena-blockout/Sun" },
                "component": "LuxLight",
                "field": "intensity_lux",
                "value": 65000.0
            }
        }"#
        .to_vec();
        let request = FunHostCommandRequest::new(9, String::from(SCENE_OPERATION_APPLY), payload);
        let sequence = host.next_sequence();
        let response = host.scene_operation_apply_response(&request, sequence, &mut queue);

        assert_eq!(response.status, FunHostCommandStatus::Ok);
        assert_eq!(queue.len(), 1);
        assert_eq!(host.state.mode, FunHostMode::Editor);
        let payload: serde_json::Value =
            serde_json::from_slice(&response.payload_json).expect("response decodes");
        assert_eq!(payload["ok"], true);
        assert_eq!(payload["value"]["accepted_operation_count"], 1);
        assert_eq!(payload["value"]["queued_for_fun_scene"], true);
    }

    #[test]
    fn material_shader_list_returns_typed_empty_catalog_until_indexer_moves_in() {
        let mut host = FunClientHostState::default();
        let response = host.handle_command(&FunHostCommandRequest::new(
            23,
            String::from(MATERIAL_SHADER_LIST),
            br#"{"projectId":"fun"}"#.to_vec(),
        ));

        assert_eq!(response.status, FunHostCommandStatus::Ok);
        let payload: serde_json::Value =
            serde_json::from_slice(&response.payload_json).expect("material catalog decodes");
        assert_eq!(payload["ok"], true);
        assert_eq!(payload["value"]["project_id"], "fun");
        assert_eq!(
            payload["value"]["default_relative_directory"],
            "assets/shaders/materials"
        );
        assert!(
            payload["value"]["shaders"]
                .as_array()
                .is_some_and(Vec::is_empty)
        );
    }

    #[test]
    fn commandbar_execute_validates_before_svelte_follow_through() {
        let mut host = FunClientHostState::default();
        let response = host.handle_command(&FunHostCommandRequest::new(
            17,
            String::from(HOST_COMMANDBAR_EXECUTE),
            br#"{"command_id":"editor.set_context.graph"}"#.to_vec(),
        ));

        assert_eq!(response.status, FunHostCommandStatus::Ok);
        assert_eq!(host.state.editor.active_route, FunEditorRoute::Graph);
        assert_eq!(
            host.state.commandbar.pending_command_id.as_deref(),
            Some("editor.set_context.graph")
        );
    }

    #[test]
    fn commandbar_execute_keeps_tool_stubs_gated() {
        let mut host = FunClientHostState::default();
        let response = host.handle_command(&FunHostCommandRequest::new(
            18,
            String::from(HOST_COMMANDBAR_EXECUTE),
            br#"{"command_id":"backend.experience.publish"}"#.to_vec(),
        ));

        assert_eq!(response.status, FunHostCommandStatus::Ok);
        let payload: serde_json::Value =
            serde_json::from_slice(&response.payload_json).expect("commandbar result decodes");
        assert_eq!(payload["ok"], false);
        assert_eq!(
            payload["diagnostics"][0]["code"],
            "host.commandbar.tool_stub_gated"
        );
    }

    #[test]
    fn default_host_starts_in_launcher_mode() {
        let host = FunClientHostState::default();

        assert_eq!(host.state.mode, FunHostMode::Launcher);
        assert_eq!(host.state.input_owner, FunInputOwner::LauncherUi);
        assert!(host.state.launcher.visible);
        assert!(!host.state.game.simulation_active);
    }

    #[test]
    fn start_config_can_open_directly_to_game_or_editor() {
        let game = FunClientHostState::from_start_config(FunClientHostStartConfig {
            mode: FunHostMode::Game,
            project_id: Some(String::from("fun")),
            project_path: Some(String::from("C:\\fun")),
            server_addr: Some(String::from("127.0.0.1:5000")),
            game_session_id: Some(String::from("local-dev")),
        });
        assert_eq!(game.state.mode, FunHostMode::Game);
        assert_eq!(game.state.input_owner, FunInputOwner::Gameplay);
        assert_eq!(
            game.state.runtime.server_addr.as_deref(),
            Some("127.0.0.1:5000")
        );
        assert_eq!(
            game.state.runtime.active_runtime_id.as_deref(),
            Some("current-client")
        );
        assert_eq!(
            game.state.launcher.selected_game_id.as_deref(),
            Some("local-dev")
        );

        let editor = FunClientHostState::from_start_config(FunClientHostStartConfig {
            mode: FunHostMode::Editor,
            project_id: Some(String::from("fun")),
            project_path: None,
            server_addr: None,
            game_session_id: None,
        });
        assert_eq!(editor.state.mode, FunHostMode::Editor);
        assert_eq!(editor.state.input_owner, FunInputOwner::EditorUi);
        assert!(editor.state.editor.services_active);
    }

    #[test]
    fn editor_activation_switches_the_single_host_to_editor_mode() {
        let mut host = FunClientHostState::default();
        let response = host.handle_command(&request(EDITOR_ACTIVATE));

        assert_eq!(response.status, FunHostCommandStatus::Ok);
        assert_eq!(host.state.mode, FunHostMode::Editor);
        assert!(host.state.editor.active);
        assert!(!host.state.launcher.visible);
    }

    #[test]
    fn editor_overlay_toggle_is_local_and_returns_to_game() {
        let mut host = FunClientHostState::default();

        let open = host.handle_command(&request(EDITOR_OVERLAY_TOGGLE));
        assert_eq!(open.status, FunHostCommandStatus::Ok);
        assert_eq!(host.state.mode, FunHostMode::EditorOverlay);
        assert_eq!(host.state.input_owner, FunInputOwner::EditorUi);

        let close = host.handle_command(&request(EDITOR_OVERLAY_TOGGLE));
        assert_eq!(close.status, FunHostCommandStatus::Ok);
        assert_eq!(host.state.mode, FunHostMode::Game);
        assert_eq!(host.state.input_owner, FunInputOwner::Gameplay);
    }

    #[test]
    fn legacy_client_viewport_launch_switches_current_host_to_game_mode() {
        let mut host = FunClientHostState::default();
        host.transition_to(FunHostMode::Editor).unwrap();

        let response = host.handle_command(&request(VIEWPORT_CLIENT_LAUNCH));

        assert_eq!(response.status, FunHostCommandStatus::Ok);
        assert_eq!(host.state.mode, FunHostMode::Game);
        assert_eq!(host.state.input_owner, FunInputOwner::Gameplay);
        assert!(host.state.game.simulation_active);
        assert!(host.state.game.hud_passive);
    }

    #[test]
    fn legacy_client_viewport_resize_updates_layout_without_child_window_state() {
        let mut host = FunClientHostState::from_start_config(FunClientHostStartConfig {
            mode: FunHostMode::Game,
            ..Default::default()
        });
        let response = host.handle_command(&FunHostCommandRequest::new(
            11,
            String::from(VIEWPORT_CLIENT_RESIZE),
            br#"{"instanceId":"current","rect":{"x":10.4,"y":20.5,"width":640.2,"height":360.9,"scale_factor":1.25}}"#.to_vec(),
        ));

        assert_eq!(response.status, FunHostCommandStatus::Ok);
        assert_eq!(host.state.mode, FunHostMode::Game);
        assert_eq!(host.state.game.viewport_layout.revision, 1);
        assert_eq!(
            host.state.game.viewport_layout.rect,
            Some(FunViewportRect {
                x: 10,
                y: 21,
                width: 640,
                height: 361,
                scale_factor_milli: 1_250,
            })
        );
    }

    #[test]
    fn games_join_configures_the_current_client_without_child_process_state() {
        let mut host = FunClientHostState::default();
        let response = host.handle_command(&FunHostCommandRequest::new(
            14,
            String::from(GAMES_JOIN),
            br#"{"gameId":"local-fun-dev","projectId":"fun","serverAddr":"127.0.0.1:5000"}"#
                .to_vec(),
        ));

        assert_eq!(response.status, FunHostCommandStatus::Ok);
        assert_eq!(host.state.mode, FunHostMode::Game);
        assert_eq!(
            host.state.runtime.active_runtime_id.as_deref(),
            Some("current-client")
        );
        assert!(host.state.runtime.client_session_configured);
        assert_eq!(
            host.state.launcher.selected_game_id.as_deref(),
            Some("local-fun-dev")
        );
        assert_eq!(host.state.project.active_project_id.as_deref(), Some("fun"));
        assert_eq!(
            host.state.runtime.server_addr.as_deref(),
            Some("127.0.0.1:5000")
        );
    }

    #[test]
    fn runtime_server_launch_marks_in_process_hosting_and_returns_command_result() {
        let mut host = FunClientHostState::default();
        let response = host.handle_command(&FunHostCommandRequest::new(
            15,
            String::from(RUNTIME_SERVER_LAUNCH),
            br#"{"projectId":"fun"}"#.to_vec(),
        ));

        assert_eq!(response.status, FunHostCommandStatus::Ok);
        assert_eq!(host.state.mode, FunHostMode::Game);
        assert!(host.state.runtime.server_runtime_running);
        let payload: serde_json::Value =
            serde_json::from_slice(&response.payload_json).expect("runtime result decodes");
        assert_eq!(payload["ok"], true);
        assert_eq!(payload["value"]["id"], "current-client");
        assert_eq!(payload["value"]["project_id"], "fun");
        assert_eq!(payload["value"]["process_status"], "running");
    }

    #[test]
    fn runtime_input_owner_command_releases_to_gameplay_by_default() {
        let mut host = FunClientHostState::default();
        host.transition_to(FunHostMode::Editor).unwrap();

        let response = host.handle_command(&request(RUNTIME_INPUT_SET_OWNER));

        assert_eq!(response.status, FunHostCommandStatus::Ok);
        assert_eq!(host.state.mode, FunHostMode::Game);
        assert_eq!(host.state.input_owner, FunInputOwner::Gameplay);
    }

    #[test]
    fn runtime_input_owner_command_can_assign_commandbar_without_mode_switch() {
        let mut host = FunClientHostState::default();
        host.transition_to(FunHostMode::Editor).unwrap();

        let response = host.handle_command(&FunHostCommandRequest::new(
            12,
            String::from(RUNTIME_INPUT_SET_OWNER),
            br#"{"owner":"commandbar"}"#.to_vec(),
        ));

        assert_eq!(response.status, FunHostCommandStatus::Ok);
        assert_eq!(host.state.mode, FunHostMode::Editor);
        assert_eq!(host.state.input_owner, FunInputOwner::Commandbar);
        assert!(host.state.commandbar.active);
    }

    #[test]
    fn editor_preview_commands_use_current_client_preview_mode() {
        let mut host = FunClientHostState::default();
        let response = host.handle_command(&FunHostCommandRequest::new(
            13,
            String::from(PREVIEW_RENDERER_ENSURE),
            br#"{"projectId":"fun","sceneId":"default","rect":{"x":0,"y":0,"width":1280,"height":720,"scale_factor":1}}"#.to_vec(),
        ));

        assert_eq!(response.status, FunHostCommandStatus::Ok);
        assert_eq!(host.state.mode, FunHostMode::Editor);
        assert_eq!(
            host.state.editor.preview_mode,
            EditorPreviewMode::CurrentClient
        );
        assert_eq!(host.state.input_owner, FunInputOwner::EditorUi);
        assert_eq!(host.state.game.viewport_layout.revision, 1);
    }

    #[test]
    fn auth_ticket_request_fails_closed_without_serializing_a_ticket() {
        let mut host = FunClientHostState::default();
        let response = host.handle_command(&FunHostCommandRequest::new(
            16,
            String::from(AUTH_TICKET_REQUEST),
            br#"{"audience":"fun_editor","requested_capabilities":["read_entities"]}"#.to_vec(),
        ));

        assert_eq!(response.status, FunHostCommandStatus::Ok);
        assert!(
            !response
                .payload_json
                .windows(b"ticket_id".len())
                .any(|window| window == b"ticket_id")
        );
        assert!(
            !response
                .payload_json
                .windows(b"session_token".len())
                .any(|window| window == b"session_token")
        );
    }

    #[test]
    fn oversized_native_ui_payload_is_rejected_before_command_dispatch() {
        let mut host = FunClientHostState::default();
        let response = host.handle_command(&FunHostCommandRequest::new(
            9,
            String::from(LAUNCHER_STATE_GET),
            vec![0; MAX_HOST_COMMAND_PAYLOAD_BYTES.saturating_add(1)],
        ));

        assert_eq!(response.status, FunHostCommandStatus::Error);
        assert_eq!(
            response.error_code,
            Some(FunHostCommandErrorCode::OversizePayload)
        );
    }

    #[test]
    fn malformed_native_ui_payload_is_rejected_before_command_dispatch() {
        let mut host = FunClientHostState::default();
        let response = host.handle_command(&FunHostCommandRequest::new(
            10,
            String::from(LAUNCHER_STATE_GET),
            b"{not-json".to_vec(),
        ));

        assert_eq!(response.status, FunHostCommandStatus::Error);
        assert_eq!(
            response.error_code,
            Some(FunHostCommandErrorCode::InvalidPayload)
        );
    }
}
