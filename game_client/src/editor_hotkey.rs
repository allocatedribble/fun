use std::{
    io::Write,
    net::{TcpStream, ToSocketAddrs},
    time::{SystemTime, UNIX_EPOCH},
};

use bevy::{
    input::ButtonInput,
    prelude::{FromWorld, KeyCode, Plugin, Query, Res, Resource, Update, Window, With},
    window::PrimaryWindow,
};
use game_shared::{
    EditorActivationMode, EditorActivationPacket, EditorActivationRequested, EditorPacketHeader,
    EditorProtocolPacket, EditorRequestId, EditorSizeBudget, EditorTargetKind, PacketSequence,
    encode_editor_packet,
};
use tracing::{debug, warn};

use crate::ClientAppOptions;

const EDITOR_CONTROL_ADDR_ENV: &str = "FUN_EDITOR_CONTROL_ADDR";
const EDITOR_ACTIVATION_ADDR_ENV: &str = "FUN_EDITOR_ACTIVATION_ADDR";
const DEFAULT_GAME_SESSION_ID: &str = "unregistered";

pub(crate) struct EditorHotkeyPlugin;

impl Plugin for EditorHotkeyPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.init_resource::<EditorActivationEndpoints>()
            .add_systems(Update, request_editor_activation_on_f1);
    }
}

#[derive(Debug, Clone, Resource)]
struct EditorActivationEndpoints {
    control_addr: Option<String>,
    activation_addr: Option<String>,
}

impl EditorActivationEndpoints {
    fn from_env() -> Self {
        Self {
            control_addr: env_non_empty_string(EDITOR_CONTROL_ADDR_ENV),
            activation_addr: env_non_empty_string(EDITOR_ACTIVATION_ADDR_ENV),
        }
    }
}

impl FromWorld for EditorActivationEndpoints {
    fn from_world(_world: &mut bevy::prelude::World) -> Self {
        Self::from_env()
    }
}

fn request_editor_activation_on_f1(
    keys: Res<ButtonInput<KeyCode>>,
    options: Res<ClientAppOptions>,
    endpoints: Res<EditorActivationEndpoints>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
) {
    if !keys.just_pressed(KeyCode::F1) || !options.mode.supports_editor_activation() {
        return;
    }

    let focused_window = primary_window
        .single()
        .map(|window| window.focused)
        .unwrap_or(false);
    let Some(project_id) = options.project_id.clone() else {
        warn!(
            target: "fun::editor::activation",
            runtime_mode = options.mode.as_env_value(),
            "F1 editor activation ignored because FUN_PROJECT_ID is not set"
        );
        return;
    };
    let Some(addr) = select_activation_addr(
        endpoints.control_addr.as_deref(),
        endpoints.activation_addr.as_deref(),
    ) else {
        warn!(
            target: "fun::editor::activation",
            "F1 editor activation ignored because no editor control or loopback activation endpoint is configured"
        );
        return;
    };

    let request = EditorActivationRequested {
        project_id,
        game_session_id: options
            .game_session_id
            .clone()
            .unwrap_or_else(|| DEFAULT_GAME_SESSION_ID.to_owned()),
        client_pid: std::process::id(),
        focused_window,
        requested_mode: EditorActivationMode::EditorShell,
    };

    match send_editor_activation_request(addr, request) {
        Ok(()) => {
            debug!(
                target: "fun::editor::activation",
                addr,
                focused_window,
                "sent F1 editor activation request"
            );
        }
        Err(error) => {
            warn!(
                target: "fun::editor::activation",
                addr,
                %error,
                "failed to send F1 editor activation request"
            );
        }
    }
}

fn send_editor_activation_request(
    addr: &str,
    request: EditorActivationRequested,
) -> std::io::Result<()> {
    let packet = EditorProtocolPacket::Activation {
        packet: EditorActivationPacket {
            header: EditorPacketHeader::new(
                EditorRequestId(unix_ms()),
                EditorTargetKind::Client,
                PacketSequence(1),
                None,
                EditorSizeBudget { max_bytes: 4096 },
            ),
            request,
        },
    };
    let bytes = encode_editor_packet(&packet).map_err(|error| {
        std::io::Error::other(format!("activation packet encode failed: {error:?}"))
    })?;
    let mut stream = TcpStream::connect(addr)?;
    stream.write_all(&bytes)?;
    stream.flush()
}

fn select_activation_addr<'a>(
    control_addr: Option<&'a str>,
    fallback_addr: Option<&'a str>,
) -> Option<&'a str> {
    control_addr
        .filter(|addr| !addr.is_empty())
        .or_else(|| fallback_addr.filter(|addr| is_loopback_addr(addr)))
}

fn is_loopback_addr(addr: &str) -> bool {
    addr.to_socket_addrs()
        .map(|mut addrs| addrs.any(|addr| addr.ip().is_loopback()))
        .unwrap_or(false)
}

fn env_non_empty_string(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ClientRuntimeMode;

    #[test]
    fn editor_hotkey_is_limited_to_joined_client_modes() {
        assert!(ClientRuntimeMode::JoinedGame.supports_editor_activation());
        assert!(ClientRuntimeMode::EditorHostedClient.supports_editor_activation());
        assert!(!ClientRuntimeMode::StandaloneClient.supports_editor_activation());
        assert!(!ClientRuntimeMode::EditorPreview.supports_editor_activation());
    }

    #[test]
    fn activation_endpoint_prefers_existing_control_channel() {
        assert_eq!(
            select_activation_addr(Some("10.1.2.3:4444"), Some("127.0.0.1:5555")),
            Some("10.1.2.3:4444")
        );
    }

    #[test]
    fn activation_fallback_requires_loopback() {
        assert_eq!(
            select_activation_addr(None, Some("127.0.0.1:5555")),
            Some("127.0.0.1:5555")
        );
        assert_eq!(select_activation_addr(None, Some("192.0.2.10:5555")), None);
    }
}
