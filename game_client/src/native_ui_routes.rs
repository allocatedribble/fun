use std::collections::BTreeMap;

use bevy::{
    input::{
        ButtonInput, ButtonState,
        keyboard::KeyboardInput,
        mouse::{MouseButtonInput, MouseWheel},
    },
    prelude::*,
    window::{CursorMoved, PrimaryWindow},
};
use fun_host::{FunClientHostState, FunEditorRoute, FunHostMode, FunInputOwner};
use fun_rvelte_bridge::{
    FUN_NATIVE_HOST_BRIDGE_SCHEMA, FrameOutput, FunNativeHostSnapshot, FunNativeHostStateValue,
    FunUiHitRegionKind, FunUiHitRegionPacket, HostAuthorizationContext, ProductInputEvent,
    ProductKey, ProductPointerButton, ProductRouteKind, ProductRouteRegistry, ProductRvelteAdapter,
    ProductRvelteDiagnostic, fun_renderer_backend::FunRendererPacketConsumer,
};
#[cfg(test)]
use fun_rvelte_bridge::{
    FunUiHitRegionId, FunUiHitRegionPolicy, FunUiInputRouteId, FunUiLayerId, FunUiNodeId, FunUiRect,
};
use tracing::{debug, warn};

pub const NATIVE_UI_ROUTES_SCHEMA: &str = "fun.game_client.native_ui_routes.v1";
const NATIVE_UI_POINTER_ID: u64 = 1;
const CAP_LAUNCHER: u32 = 1 << 0;
const CAP_HUD: u32 = 1 << 1;
const CAP_PAUSE: u32 = 1 << 2;
const CAP_DIAGNOSTICS: u32 = 1 << 3;
const CAP_COMMANDBAR: u32 = 1 << 4;
const CAP_SETTINGS: u32 = 1 << 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeUiGameplayInputBlockReason {
    None,
    UiPointerRegion,
    UiModal,
    TextEntry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct NativeUiGameplayInputGate {
    pub block_movement: bool,
    pub block_fire: bool,
    pub block_actions: bool,
    pub block_look: bool,
    pub keyboard_owned_by_ui: bool,
    pub pointer_owned_by_ui: bool,
    pub reason: NativeUiGameplayInputBlockReason,
}

impl NativeUiGameplayInputGate {
    #[must_use]
    pub const fn blocks_movement(self) -> bool {
        self.block_movement
    }

    #[must_use]
    pub const fn blocks_pointer_actions(self) -> bool {
        self.block_fire || self.block_actions || self.pointer_owned_by_ui
    }

    #[must_use]
    pub const fn blocks_keyboard_actions(self) -> bool {
        self.keyboard_owned_by_ui
    }

    #[must_use]
    pub const fn blocks_look(self) -> bool {
        self.block_look
    }
}

impl Default for NativeUiGameplayInputGate {
    fn default() -> Self {
        Self {
            block_movement: false,
            block_fire: false,
            block_actions: false,
            block_look: false,
            keyboard_owned_by_ui: false,
            pointer_owned_by_ui: false,
            reason: NativeUiGameplayInputBlockReason::None,
        }
    }
}

struct NativeUiRoutesState {
    adapter: ProductRvelteAdapter<FunRendererPacketConsumer>,
    mounted: bool,
    active_route: Option<ProductRouteKind>,
    last_host_sequence: u64,
    last_frame: Option<NativeUiFrameSummary>,
    last_error_code: Option<&'static str>,
    hit_regions: Vec<FunUiHitRegionPacket>,
    pointer_position: NativeUiPointerPosition,
}

impl NativeUiRoutesState {
    fn new() -> Self {
        let renderer = FunRendererPacketConsumer::new().with_auto_materialize_resources(true);
        Self {
            adapter: ProductRvelteAdapter::new(ProductRouteRegistry::with_well_known(), renderer),
            mounted: false,
            active_route: None,
            last_host_sequence: u64::MAX,
            last_frame: None,
            last_error_code: None,
            hit_regions: Vec::new(),
            pointer_position: NativeUiPointerPosition::default(),
        }
    }

    fn mount_route(&mut self, route: ProductRouteKind) -> Result<(), ProductRvelteDiagnostic> {
        self.adapter.mount_route(route)?;
        self.mounted = true;
        Ok(())
    }

    fn ensure_active_route(
        &mut self,
        route: ProductRouteKind,
        host: &FunClientHostState,
    ) -> Result<(), ProductRvelteDiagnostic> {
        if !self.mounted {
            self.mount_route(route)?;
        }
        if self.active_route != Some(route) {
            self.adapter.set_active_route(route)?;
            self.mounted = true;
            self.active_route = Some(route);
            self.ingest_host_snapshot(route, host)?;
            self.last_host_sequence = host.sequence();
            return Ok(());
        }
        if self.last_host_sequence != host.sequence() {
            self.ingest_host_snapshot(route, host)?;
            self.last_host_sequence = host.sequence();
        }
        Ok(())
    }

    fn ingest_host_snapshot(
        &mut self,
        route: ProductRouteKind,
        host: &FunClientHostState,
    ) -> Result<(), ProductRvelteDiagnostic> {
        self.adapter
            .ingest_host_snapshot(route, snapshot_for_host_route(route, host))
    }

    fn record_frame(&mut self, frame: FrameOutput) -> NativeUiFrameSummary {
        self.hit_regions.clear();
        self.hit_regions
            .extend(frame.frame.hit_regions.iter().copied());
        let summary = NativeUiFrameSummary {
            route: frame.route,
            frame_id: frame.frame_id,
            hit_region_count: self.hit_regions.len(),
            command_intent_count: frame.command_intents.len(),
            diagnostic_count: frame.diagnostics.len(),
            layers_submitted: frame.submit.layers_submitted,
            draws_submitted: frame.submit.draws_submitted,
        };
        self.last_frame = Some(summary);
        self.last_error_code = None;
        summary
    }

    fn record_error(&mut self, diagnostic: ProductRvelteDiagnostic) {
        let code = diagnostic.code();
        if self.last_error_code != Some(code) {
            warn!(
                target: "fun::ui",
                schema = NATIVE_UI_ROUTES_SCHEMA,
                diagnostic = code,
                "native rvelte route tick failed"
            );
        }
        self.last_error_code = Some(code);
    }

    fn hit_region_at(&self, x: i32, y: i32) -> Option<FunUiHitRegionPacket> {
        self.hit_regions
            .iter()
            .filter(|region| hit_region_accepts_pointer(region, x, y))
            .max_by_key(|region| (region.z_order, region.priority))
            .copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NativeUiFrameSummary {
    route: ProductRouteKind,
    frame_id: u64,
    hit_region_count: usize,
    command_intent_count: usize,
    diagnostic_count: usize,
    layers_submitted: u32,
    draws_submitted: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct NativeUiRouteReport {
    pub schema: &'static str,
    pub mounted: bool,
    pub active_route: Option<ProductRouteKind>,
    pub frame_id: u64,
    pub submitted_layer_count: u32,
    pub submitted_draw_count: u32,
    pub diagnostic_count: u32,
    pub last_error_code: Option<&'static str>,
}

impl Default for NativeUiRouteReport {
    fn default() -> Self {
        Self {
            schema: NATIVE_UI_ROUTES_SCHEMA,
            mounted: false,
            active_route: None,
            frame_id: 0,
            submitted_layer_count: 0,
            submitted_draw_count: 0,
            diagnostic_count: 0,
            last_error_code: None,
        }
    }
}

impl NativeUiRouteReport {
    fn record_frame(&mut self, mounted: bool, summary: NativeUiFrameSummary) {
        self.mounted = mounted;
        self.active_route = Some(summary.route);
        self.frame_id = summary.frame_id;
        self.submitted_layer_count = summary.layers_submitted;
        self.submitted_draw_count = summary.draws_submitted;
        self.diagnostic_count = u32::try_from(summary.diagnostic_count).unwrap_or(u32::MAX);
        self.last_error_code = None;
    }

    fn record_error(
        &mut self,
        mounted: bool,
        active_route: Option<ProductRouteKind>,
        code: &'static str,
    ) {
        self.mounted = mounted;
        self.active_route = active_route;
        self.last_error_code = Some(code);
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GameNativeUiRoutesPlugin;

impl Plugin for GameNativeUiRoutesPlugin {
    fn build(&self, app: &mut App) {
        app.insert_non_send(NativeUiRoutesState::new())
            .init_resource::<NativeUiGameplayInputGate>()
            .init_resource::<NativeUiRouteReport>()
            .add_systems(Startup, mount_native_ui_routes)
            .add_systems(
                Update,
                (
                    sync_native_ui_route_from_host,
                    forward_bevy_input_to_native_ui,
                    tick_native_ui_routes,
                    update_native_ui_gameplay_input_gate,
                )
                    .chain(),
            );
    }
}

fn mount_native_ui_routes(
    mut native_ui: NonSendMut<NativeUiRoutesState>,
    host: Res<FunClientHostState>,
    mut report: ResMut<NativeUiRouteReport>,
) {
    let route = product_route_for_host(&host);
    match native_ui.ensure_active_route(route, &host) {
        Ok(()) => {
            report.mounted = native_ui.mounted;
            report.active_route = native_ui.active_route;
        }
        Err(diagnostic) => {
            let code = diagnostic.code();
            report.record_error(native_ui.mounted, native_ui.active_route, code);
            warn!(
                target: "fun::ui",
                schema = NATIVE_UI_ROUTES_SCHEMA,
                diagnostic = code,
                "native rvelte route mount failed"
            );
        }
    }
}

fn sync_native_ui_route_from_host(
    mut native_ui: NonSendMut<NativeUiRoutesState>,
    host: Res<FunClientHostState>,
    mut report: ResMut<NativeUiRouteReport>,
) {
    let route = product_route_for_host(&host);
    match native_ui.ensure_active_route(route, &host) {
        Ok(()) => {
            report.mounted = native_ui.mounted;
            report.active_route = native_ui.active_route;
        }
        Err(diagnostic) => {
            let code = diagnostic.code();
            native_ui.record_error(diagnostic);
            report.record_error(native_ui.mounted, native_ui.active_route, code);
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "native UI input forwarding consumes pointer, keyboard and focus state together"
)]
fn forward_bevy_input_to_native_ui(
    mut native_ui: NonSendMut<NativeUiRoutesState>,
    host: Res<FunClientHostState>,
    primary_window: Query<(Entity, &Window), With<PrimaryWindow>>,
    keys: Res<ButtonInput<KeyCode>>,
    gate: Res<NativeUiGameplayInputGate>,
    mut cursor_moved: MessageReader<CursorMoved>,
    mut mouse_buttons: MessageReader<MouseButtonInput>,
    mut mouse_wheel: MessageReader<MouseWheel>,
    mut keyboard_inputs: MessageReader<KeyboardInput>,
) {
    let Ok((primary_window_entity, window)) = primary_window.single() else {
        return;
    };
    let forward_pointer = gate.pointer_owned_by_ui || route_wants_pointer(&host);
    let forward_keyboard = gate.keyboard_owned_by_ui || route_wants_keyboard(&host);

    if forward_pointer {
        for event in cursor_moved.read() {
            if event.window != primary_window_entity {
                continue;
            }
            native_ui.pointer_position = NativeUiPointerPosition {
                x: saturating_i32_from_f32(event.position.x),
                y: saturating_i32_from_f32(event.position.y),
            };
            let pointer_position = native_ui.pointer_position;
            submit_native_input(
                &mut native_ui,
                ProductInputEvent::PointerMove {
                    pointer_id: NATIVE_UI_POINTER_ID,
                    x: pointer_position.x,
                    y: pointer_position.y,
                },
            );
        }

        let pointer_position = native_pointer_position(&mut native_ui, window);
        for event in mouse_buttons.read() {
            if event.window != primary_window_entity {
                continue;
            }
            let Some(button) = product_pointer_button(event.button) else {
                continue;
            };
            let input = match event.state {
                ButtonState::Pressed => ProductInputEvent::PointerDown {
                    pointer_id: NATIVE_UI_POINTER_ID,
                    x: pointer_position.x,
                    y: pointer_position.y,
                    button,
                },
                ButtonState::Released => ProductInputEvent::PointerUp {
                    pointer_id: NATIVE_UI_POINTER_ID,
                    x: pointer_position.x,
                    y: pointer_position.y,
                    button,
                },
            };
            submit_native_input(&mut native_ui, input);
        }

        for event in mouse_wheel.read() {
            if event.window == primary_window_entity && event.y < 0.0 {
                submit_native_input(&mut native_ui, ProductInputEvent::FocusNext);
            } else if event.window == primary_window_entity && event.y > 0.0 {
                submit_native_input(&mut native_ui, ProductInputEvent::FocusPrevious);
            }
        }
    } else {
        cursor_moved.clear();
        mouse_buttons.clear();
        mouse_wheel.clear();
    }

    if forward_keyboard {
        for event in keyboard_inputs.read() {
            if event.window != primary_window_entity || event.state != ButtonState::Pressed {
                continue;
            }
            if let Some(key) = product_key(event.key_code) {
                submit_native_input(
                    &mut native_ui,
                    ProductInputEvent::KeyDown {
                        key,
                        shift: keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]),
                    },
                );
            }
            if let Some(text) = event.text.as_deref() {
                submit_text_input(&mut native_ui, text);
            }
        }
    } else {
        keyboard_inputs.clear();
    }
}

fn tick_native_ui_routes(
    mut native_ui: NonSendMut<NativeUiRoutesState>,
    mut report: ResMut<NativeUiRouteReport>,
) {
    match native_ui.adapter.tick() {
        Ok(frame) => {
            if frame.diagnostics.is_empty() {
                debug!(
                    target: "fun::ui",
                    schema = NATIVE_UI_ROUTES_SCHEMA,
                    route = frame.route.as_str(),
                    frame_id = frame.frame_id,
                    layers = frame.submit.layers_submitted,
                    draws = frame.submit.draws_submitted,
                    diagnostics = frame.diagnostics.len(),
                    "native rvelte route frame submitted"
                );
            }
            let summary = native_ui.record_frame(frame);
            report.record_frame(native_ui.mounted, summary);
        }
        Err(diagnostic) => {
            let code = diagnostic.code();
            native_ui.record_error(diagnostic);
            report.record_error(native_ui.mounted, native_ui.active_route, code);
        }
    }
}

fn update_native_ui_gameplay_input_gate(
    native_ui: NonSend<NativeUiRoutesState>,
    host: Res<FunClientHostState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut gate: ResMut<NativeUiGameplayInputGate>,
) {
    let pointer_region = windows
        .single()
        .ok()
        .and_then(Window::cursor_position)
        .and_then(|position| {
            native_ui.hit_region_at(
                saturating_i32_from_f32(position.x),
                saturating_i32_from_f32(position.y),
            )
        });

    *gate = gameplay_gate_for_host(&host, pointer_region);
}

fn submit_native_input(native_ui: &mut NativeUiRoutesState, input: ProductInputEvent) {
    if let Err(diagnostic) = native_ui.adapter.submit_product_input(input) {
        native_ui.record_error(diagnostic);
    }
}

fn submit_text_input(native_ui: &mut NativeUiRoutesState, text: &str) {
    let mut buffer = String::new();
    for ch in text.chars() {
        if ch.is_ascii() && !ch.is_control() {
            buffer.push(ch);
        }
        if buffer.len() >= 64 {
            break;
        }
    }
    if !buffer.is_empty() {
        submit_native_input(native_ui, ProductInputEvent::TextInput { text: buffer });
    }
}

fn product_route_for_host(host: &FunClientHostState) -> ProductRouteKind {
    if host.state.commandbar.active || host.state.input_owner == FunInputOwner::Commandbar {
        return ProductRouteKind::CommandBar;
    }
    if host.state.input_owner == FunInputOwner::GameMenuUi {
        return ProductRouteKind::PauseMenu;
    }
    if host.state.editor.active_route == FunEditorRoute::Diagnostics {
        return ProductRouteKind::DiagnosticsList;
    }
    match host.state.mode {
        FunHostMode::Boot | FunHostMode::Launcher | FunHostMode::Loading => {
            ProductRouteKind::LauncherShell
        }
        FunHostMode::Game => ProductRouteKind::HudOverlay,
        FunHostMode::Editor | FunHostMode::EditorOverlay => ProductRouteKind::SettingsShell,
        FunHostMode::Shutdown => ProductRouteKind::PauseMenu,
    }
}

fn route_wants_pointer(host: &FunClientHostState) -> bool {
    !matches!(host.state.input_owner, FunInputOwner::Gameplay)
}

fn route_wants_keyboard(host: &FunClientHostState) -> bool {
    matches!(
        host.state.input_owner,
        FunInputOwner::LauncherUi
            | FunInputOwner::EditorUi
            | FunInputOwner::GameMenuUi
            | FunInputOwner::TextEntry
            | FunInputOwner::Commandbar
    )
}

fn gameplay_gate_for_host(
    host: &FunClientHostState,
    pointer_region: Option<FunUiHitRegionPacket>,
) -> NativeUiGameplayInputGate {
    match host.state.input_owner {
        FunInputOwner::Gameplay => {
            if pointer_region.is_some_and(pointer_region_blocks_gameplay) {
                NativeUiGameplayInputGate {
                    block_movement: false,
                    block_fire: true,
                    block_actions: true,
                    block_look: true,
                    keyboard_owned_by_ui: false,
                    pointer_owned_by_ui: true,
                    reason: NativeUiGameplayInputBlockReason::UiPointerRegion,
                }
            } else {
                NativeUiGameplayInputGate::default()
            }
        }
        FunInputOwner::TextEntry | FunInputOwner::Commandbar => NativeUiGameplayInputGate {
            block_movement: true,
            block_fire: true,
            block_actions: true,
            block_look: true,
            keyboard_owned_by_ui: true,
            pointer_owned_by_ui: pointer_region.is_some(),
            reason: NativeUiGameplayInputBlockReason::TextEntry,
        },
        FunInputOwner::LauncherUi | FunInputOwner::EditorUi | FunInputOwner::GameMenuUi => {
            NativeUiGameplayInputGate {
                block_movement: true,
                block_fire: true,
                block_actions: true,
                block_look: true,
                keyboard_owned_by_ui: true,
                pointer_owned_by_ui: true,
                reason: NativeUiGameplayInputBlockReason::UiModal,
            }
        }
    }
}

fn snapshot_for_host_route(
    route: ProductRouteKind,
    host: &FunClientHostState,
) -> FunNativeHostSnapshot {
    let mut state = BTreeMap::new();
    state.insert(
        String::from("mode_code"),
        FunNativeHostStateValue::Numeric(host_mode_code(host.state.mode)),
    );
    state.insert(
        String::from("input_owner_code"),
        FunNativeHostStateValue::Numeric(input_owner_code(host.state.input_owner)),
    );
    state.insert(
        String::from("launcher_visible"),
        FunNativeHostStateValue::Boolean(host.state.launcher.visible),
    );
    state.insert(
        String::from("hud_passive"),
        FunNativeHostStateValue::Boolean(host.state.game.hud_passive),
    );
    state.insert(
        String::from("simulation_active"),
        FunNativeHostStateValue::Boolean(host.state.game.simulation_active),
    );
    state.insert(
        String::from("editor_active"),
        FunNativeHostStateValue::Boolean(host.state.editor.active),
    );
    state.insert(
        String::from("diagnostics_unread"),
        FunNativeHostStateValue::Numeric(host.state.diagnostics.unread_count),
    );
    state.insert(
        String::from("account_ready"),
        FunNativeHostStateValue::Boolean(host.state.account.backend_session_available),
    );
    let payload_bytes = native_snapshot_payload_bytes(&state);
    FunNativeHostSnapshot {
        schema_version: String::from(FUN_NATIVE_HOST_BRIDGE_SCHEMA),
        component_id: u64::from(route.code()),
        revision: host.sequence(),
        freshness_token: host.sequence(),
        authorization_context: HostAuthorizationContext {
            scope_label: String::from(route.as_str()),
            session_revision: host.sequence(),
            capability_mask: route_capability_mask(route),
        },
        payload_bytes,
        state,
    }
}

fn native_snapshot_payload_bytes(state: &BTreeMap<String, FunNativeHostStateValue>) -> u32 {
    let mut bytes = 0usize;
    for key in state.keys() {
        bytes = bytes.saturating_add(key.len()).saturating_add(8);
    }
    u32::try_from(bytes).unwrap_or(u32::MAX).min(16 * 1024)
}

const fn route_capability_mask(route: ProductRouteKind) -> u32 {
    match route {
        ProductRouteKind::LauncherShell => CAP_LAUNCHER,
        ProductRouteKind::HudOverlay => CAP_HUD,
        ProductRouteKind::PauseMenu => CAP_PAUSE,
        ProductRouteKind::DiagnosticsList => CAP_DIAGNOSTICS,
        ProductRouteKind::CommandBar => CAP_COMMANDBAR,
        ProductRouteKind::SettingsShell => CAP_SETTINGS,
        ProductRouteKind::ProductReserved { .. } => 0,
    }
}

const fn host_mode_code(mode: FunHostMode) -> u32 {
    match mode {
        FunHostMode::Boot => 0,
        FunHostMode::Launcher => 1,
        FunHostMode::Game => 2,
        FunHostMode::Editor => 3,
        FunHostMode::EditorOverlay => 4,
        FunHostMode::Loading => 5,
        FunHostMode::Shutdown => 6,
    }
}

const fn input_owner_code(owner: FunInputOwner) -> u32 {
    match owner {
        FunInputOwner::Gameplay => 0,
        FunInputOwner::LauncherUi => 1,
        FunInputOwner::EditorUi => 2,
        FunInputOwner::GameMenuUi => 3,
        FunInputOwner::TextEntry => 4,
        FunInputOwner::Commandbar => 5,
    }
}

fn product_pointer_button(button: MouseButton) -> Option<ProductPointerButton> {
    match button {
        MouseButton::Left => Some(ProductPointerButton::Primary),
        MouseButton::Right => Some(ProductPointerButton::Secondary),
        MouseButton::Middle => Some(ProductPointerButton::Middle),
        MouseButton::Back | MouseButton::Forward | MouseButton::Other(_) => None,
    }
}

fn product_key(key_code: KeyCode) -> Option<ProductKey> {
    match key_code {
        KeyCode::Enter | KeyCode::NumpadEnter => Some(ProductKey::Enter),
        KeyCode::Space => Some(ProductKey::Space),
        KeyCode::Escape => Some(ProductKey::Escape),
        KeyCode::Tab => Some(ProductKey::Tab),
        KeyCode::Backspace => Some(ProductKey::Backspace),
        KeyCode::ArrowUp => Some(ProductKey::ArrowUp),
        KeyCode::ArrowDown => Some(ProductKey::ArrowDown),
        KeyCode::ArrowLeft => Some(ProductKey::ArrowLeft),
        KeyCode::ArrowRight => Some(ProductKey::ArrowRight),
        _ => None,
    }
}

fn native_pointer_position(
    native_ui: &mut NativeUiRoutesState,
    window: &Window,
) -> NativeUiPointerPosition {
    if let Some(position) = window.cursor_position() {
        native_ui.pointer_position = NativeUiPointerPosition {
            x: saturating_i32_from_f32(position.x),
            y: saturating_i32_from_f32(position.y),
        };
    }
    native_ui.pointer_position
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct NativeUiPointerPosition {
    x: i32,
    y: i32,
}

fn saturating_i32_from_f32(value: f32) -> i32 {
    if !value.is_finite() {
        return 0;
    }
    value.round().clamp(i32::MIN as f32, i32::MAX as f32) as i32
}

fn hit_region_accepts_pointer(region: &FunUiHitRegionPacket, x: i32, y: i32) -> bool {
    if !region.pointer_enabled || region.policy.pass_through {
        return false;
    }
    let left = region.bounds.origin.x;
    let top = region.bounds.origin.y;
    let right = left.saturating_add(i32::try_from(region.bounds.size.width).unwrap_or(i32::MAX));
    let bottom = top.saturating_add(i32::try_from(region.bounds.size.height).unwrap_or(i32::MAX));
    x >= left && y >= top && x < right && y < bottom
}

fn pointer_region_blocks_gameplay(region: FunUiHitRegionPacket) -> bool {
    region.policy.captures_pointer
        || region.policy.modal_capture
        || matches!(
            region.kind,
            FunUiHitRegionKind::Capture
                | FunUiHitRegionKind::Modal
                | FunUiHitRegionKind::Drag
                | FunUiHitRegionKind::TextInput
                | FunUiHitRegionKind::NativeSlot
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host_with_mode(mode: FunHostMode) -> FunClientHostState {
        FunClientHostState::from_start_config(fun_host::FunClientHostStartConfig {
            mode,
            ..Default::default()
        })
    }

    #[test]
    fn route_selection_covers_product_runtime_surfaces() {
        assert_eq!(
            product_route_for_host(&host_with_mode(FunHostMode::Launcher)),
            ProductRouteKind::LauncherShell
        );
        assert_eq!(
            product_route_for_host(&host_with_mode(FunHostMode::Game)),
            ProductRouteKind::HudOverlay
        );

        let mut pause = host_with_mode(FunHostMode::Game);
        pause.state.input_owner = FunInputOwner::GameMenuUi;
        assert_eq!(product_route_for_host(&pause), ProductRouteKind::PauseMenu);

        let mut diagnostics = host_with_mode(FunHostMode::Editor);
        diagnostics.state.editor.active_route = FunEditorRoute::Diagnostics;
        assert_eq!(
            product_route_for_host(&diagnostics),
            ProductRouteKind::DiagnosticsList
        );
    }

    #[test]
    fn host_snapshot_stays_inside_bridge_payload_cap() {
        let host = host_with_mode(FunHostMode::Launcher);
        let snapshot = snapshot_for_host_route(ProductRouteKind::LauncherShell, &host);

        assert_eq!(snapshot.schema_version, FUN_NATIVE_HOST_BRIDGE_SCHEMA);
        assert!(snapshot.payload_bytes <= 16 * 1024);
        assert_eq!(snapshot.component_id, 1);
        assert_eq!(snapshot.authorization_context.scope_label, "launcher_shell");
    }

    #[test]
    fn gameplay_gate_blocks_only_pointer_when_hud_region_is_hit() {
        let host = host_with_mode(FunHostMode::Game);
        let region = FunUiHitRegionPacket {
            hit_region_id: FunUiHitRegionId::new(1),
            kind: FunUiHitRegionKind::Capture,
            node_id: FunUiNodeId::new(1),
            layer_id: FunUiLayerId::new(1),
            input_route_id: FunUiInputRouteId::new(1),
            bounds: FunUiRect::new(0, 0, 100, 100),
            policy: FunUiHitRegionPolicy::for_kind(FunUiHitRegionKind::Capture),
            clip_id: None,
            z_order: 0,
            priority: 0,
            revision: 1,
            pointer_enabled: true,
            focusable: true,
        };

        let gate = gameplay_gate_for_host(&host, Some(region));

        assert!(!gate.blocks_movement());
        assert!(gate.blocks_pointer_actions());
        assert!(gate.blocks_look());
        assert_eq!(
            gate.reason,
            NativeUiGameplayInputBlockReason::UiPointerRegion
        );
    }
}
