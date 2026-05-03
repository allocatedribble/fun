use bevy::{
    input::ButtonInput,
    prelude::{KeyCode, Plugin, Res, ResMut, Update},
};
use fun_host::FunClientHostState;
use tracing::{debug, warn};

use crate::ClientAppOptions;

pub(crate) struct EditorHotkeyPlugin;

impl Plugin for EditorHotkeyPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_systems(Update, activate_editor_overlay_on_f1);
    }
}

fn activate_editor_overlay_on_f1(
    keys: Res<ButtonInput<KeyCode>>,
    options: Res<ClientAppOptions>,
    mut host: ResMut<FunClientHostState>,
) {
    if !keys.just_pressed(KeyCode::F1) || !options.mode.supports_editor_activation() {
        return;
    }

    match host.toggle_editor_overlay() {
        Ok(()) => {
            debug!(
                target: "fun::editor::activation",
                runtime_mode = options.mode.as_env_value(),
                host_mode = host.state.mode.as_wire_str(),
                "toggled in-process editor overlay"
            );
        }
        Err(error) => {
            warn!(
                target: "fun::editor::activation",
                runtime_mode = options.mode.as_env_value(),
                error = error.as_wire_str(),
                "F1 editor overlay activation rejected by FunClientHost"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ClientRuntimeMode;

    #[test]
    fn editor_hotkey_is_in_process_for_gameplay_client_modes() {
        assert!(ClientRuntimeMode::JoinedGame.supports_editor_activation());
        assert!(ClientRuntimeMode::EditorHostedClient.supports_editor_activation());
        assert!(!ClientRuntimeMode::StandaloneClient.supports_editor_activation());
        assert!(!ClientRuntimeMode::EditorPreview.supports_editor_activation());
    }
}
