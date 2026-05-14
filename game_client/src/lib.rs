pub mod fun_engine_boot;
#[cfg(feature = "client-telemetry")]
pub mod release_telemetry;
#[cfg(feature = "client-telemetry")]
pub mod telemetry_gate;

pub use fun_engine_boot::{
    ClientBootError, ClientEngineProfile, ClientFunLifecycleOptions, ClientRuntimeModule,
    ClientRuntimeReport, ClientWindowIntent, build_client_fun_engine,
    build_client_fun_engine_with_options, build_client_fun_engine_with_profile,
    client_window_intent, run_client_fun_engine, run_client_fun_engine_with_options,
};
use tracing::warn;

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

    pub const fn should_connect_to_game_server(self) -> bool {
        matches!(self, Self::JoinedGame | Self::EditorHostedClient)
    }

    pub const fn uses_static_preview_stream(self) -> bool {
        matches!(self, Self::EditorPreview)
    }

    pub const fn runs_gameplay_runtime(self) -> bool {
        !self.uses_static_preview_stream()
    }

    pub const fn supports_editor_activation(self) -> bool {
        matches!(self, Self::JoinedGame | Self::EditorHostedClient)
    }

    pub const fn as_env_value(self) -> &'static str {
        match self {
            Self::JoinedGame => "joined_game",
            Self::StandaloneClient => "standalone_client",
            Self::EditorHostedClient => "editor_hosted_client",
            Self::EditorPreview => "editor_preview",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunClientStartMode {
    Launcher,
    Game,
    Editor,
}

impl FunClientStartMode {
    fn from_env_and_args() -> Self {
        if let Some(value) = start_mode_from_args(std::env::args().skip(1)) {
            return value;
        }
        let Some(value) = env_non_empty_string("FUN_START_MODE") else {
            return Self::Launcher;
        };
        Self::parse(&value).unwrap_or_else(|| {
            warn!(
                target: "fun::client::host",
                start_mode = value,
                "unknown FUN_START_MODE; using launcher"
            );
            Self::Launcher
        })
    }

    fn parse(value: &str) -> Option<Self> {
        if value.eq_ignore_ascii_case("launcher") || value.eq_ignore_ascii_case("-launcher") {
            return Some(Self::Launcher);
        }
        if value.eq_ignore_ascii_case("game") || value.eq_ignore_ascii_case("play") {
            return Some(Self::Game);
        }
        if value.eq_ignore_ascii_case("editor") || value.eq_ignore_ascii_case("edit") {
            return Some(Self::Editor);
        }
        None
    }

    pub const fn as_env_value(self) -> &'static str {
        match self {
            Self::Launcher => "launcher",
            Self::Game => "game",
            Self::Editor => "editor",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ClientRenderProfile {
    #[default]
    Default,
    Diagnostics,
}

impl ClientRenderProfile {
    pub fn from_env() -> Self {
        let Some(value) = env_non_empty_string("FUN_CLIENT_RENDER_PROFILE") else {
            return Self::Default;
        };
        Self::parse(&value).unwrap_or_else(|| {
            warn!(
                target: "fun::client::render",
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientAppOptions {
    pub mode: ClientRuntimeMode,
    pub start_mode: FunClientStartMode,
    pub render_profile: ClientRenderProfile,
    pub server_addr: Option<String>,
    pub project_id: Option<String>,
    pub project_path: Option<String>,
    pub game_session_id: Option<String>,
    pub scene_id: Option<String>,
    pub hosted_by_editor: bool,
    pub host_instance_id: Option<String>,
    pub host_parent_pid: Option<u32>,
}

impl ClientAppOptions {
    pub fn from_env() -> Self {
        let mode = ClientRuntimeMode::from_env();
        let start_mode = FunClientStartMode::from_env_and_args();
        let legacy_hosted_by_editor_requested = env_flag("FUN_HOSTED_BY_EDITOR")
            || env_flag("FUN_CLIENT_HOSTED_BY_EDITOR")
            || matches!(
                mode,
                ClientRuntimeMode::EditorHostedClient | ClientRuntimeMode::EditorPreview
            );
        if legacy_hosted_by_editor_requested {
            warn!(
                target: "fun::client::host",
                runtime_mode = mode.as_env_value(),
                "legacy editor-hosted client flags are retired; fun-client is the unified host"
            );
        }
        let legacy_parent_pid = env_host_parent_pid();
        if legacy_parent_pid.is_some() {
            warn!(
                target: "fun::client::host",
                "FUN_HOST_PARENT_PID is ignored because the merged client is not a child process"
            );
        }
        Self {
            mode,
            start_mode,
            render_profile: ClientRenderProfile::from_env(),
            server_addr: option_from_args("server-addr")
                .or_else(|| env_non_empty_string("FUN_SERVER_ADDR")),
            project_id: option_from_args("project-id")
                .or_else(|| env_non_empty_string("FUN_PROJECT_ID")),
            project_path: option_from_args("project-path")
                .or_else(|| env_non_empty_string("FUN_PROJECT_PATH")),
            game_session_id: option_from_args("game-session-id")
                .or_else(|| env_non_empty_string("FUN_GAME_SESSION_ID")),
            scene_id: env_non_empty_string("FUN_SCENE_ID"),
            hosted_by_editor: false,
            host_instance_id: env_non_empty_string("FUN_HOST_INSTANCE_ID"),
            host_parent_pid: None,
        }
    }
}

impl Default for ClientAppOptions {
    fn default() -> Self {
        Self {
            mode: ClientRuntimeMode::JoinedGame,
            start_mode: FunClientStartMode::Launcher,
            render_profile: ClientRenderProfile::Default,
            server_addr: None,
            project_id: None,
            project_path: None,
            game_session_id: None,
            scene_id: None,
            hosted_by_editor: false,
            host_instance_id: None,
            host_parent_pid: None,
        }
    }
}

pub fn run_client_engine() -> Result<ClientRuntimeReport, ClientBootError> {
    run_client_fun_engine()
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

fn start_mode_from_args(args: impl IntoIterator<Item = String>) -> Option<FunClientStartMode> {
    let mut iter = args.into_iter().peekable();
    while let Some(arg) = iter.next() {
        if let Some(value) = arg.strip_prefix("--start-mode=") {
            return FunClientStartMode::parse(value);
        }
        if arg == "--start-mode" {
            return iter
                .next()
                .and_then(|value| FunClientStartMode::parse(&value));
        }
        if matches!(
            arg.as_str(),
            "--launcher" | "-Launcher" | "-launcher" | "/Launcher" | "/launcher"
        ) {
            return Some(FunClientStartMode::Launcher);
        }
        if matches!(
            arg.as_str(),
            "--game" | "-Game" | "-game" | "/Game" | "/game"
        ) {
            return Some(FunClientStartMode::Game);
        }
        if matches!(
            arg.as_str(),
            "--editor" | "-Editor" | "-editor" | "/Editor" | "/editor"
        ) {
            return Some(FunClientStartMode::Editor);
        }
    }
    None
}

fn option_from_args(name: &str) -> Option<String> {
    option_from_args_iter(name, std::env::args().skip(1))
}

fn option_from_args_iter(name: &str, args: impl IntoIterator<Item = String>) -> Option<String> {
    let long_name = format!("--{name}");
    let long_name_with_value = format!("{long_name}=");
    let mut iter = args.into_iter().peekable();
    while let Some(arg) = iter.next() {
        if let Some(value) = arg.strip_prefix(&long_name_with_value)
            && !value.is_empty()
        {
            return Some(value.to_owned());
        }
        if arg == long_name {
            return iter.next().filter(|value| !value.is_empty());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use fun_engine::EngineError;

    #[test]
    fn product_client_engine_boots_on_fun_engine_stack() -> Result<(), EngineError> {
        let options = ClientAppOptions::default();
        let engine = build_client_fun_engine_with_options(&options)?;
        assert!(engine.integrations().renderer().is_some());
        assert_eq!(
            engine
                .integrations()
                .windowing()
                .map(|contract| contract.provider()),
            Some(concat!("fun", "-", "window"))
        );
        Ok(())
    }

    #[test]
    fn client_modes_preserve_runtime_classification() {
        assert!(!ClientRuntimeMode::EditorPreview.runs_gameplay_runtime());
        assert!(!ClientRuntimeMode::EditorPreview.should_connect_to_game_server());
        assert!(ClientRuntimeMode::JoinedGame.runs_gameplay_runtime());
        assert!(ClientRuntimeMode::JoinedGame.should_connect_to_game_server());
    }

    #[test]
    fn start_mode_parses_cli_aliases() {
        assert_eq!(
            FunClientStartMode::parse("launcher"),
            Some(FunClientStartMode::Launcher)
        );
        assert_eq!(
            start_mode_from_args(vec![String::from("--start-mode=game")]),
            Some(FunClientStartMode::Game)
        );
    }

    #[test]
    fn cli_options_parse_project_and_server_values() {
        let args = vec![
            String::from("--server-addr"),
            String::from("127.0.0.1:5000"),
            String::from("--project-path=C:\\fun"),
        ];

        assert_eq!(
            option_from_args_iter("server-addr", args.clone()),
            Some(String::from("127.0.0.1:5000"))
        );
        assert_eq!(
            option_from_args_iter("project-path", args),
            Some(String::from("C:\\fun"))
        );
    }
}
