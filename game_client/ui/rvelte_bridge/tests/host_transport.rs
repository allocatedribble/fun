//! Pass-73 host transport integration tests.
//!
//! Drives the [`HostBridgeTranslator`] through the eight scenarios
//! the pass-73 spec lists:
//!
//! 1. launcher snapshot;
//! 2. HUD patch;
//! 3. pause command;
//! 4. diagnostics refresh;
//! 5. unknown command rejection;
//! 6. stale patch rejection;
//! 7. oversized payload rejection;
//! 8. host disconnect/reconnect.
//!
//! Plus structural coverage for correlation, freshness, and
//! authorization invariants.

use std::collections::BTreeMap;

use fun_rvelte_bridge::host_transport::{
    CommandLabelMap, FUN_NATIVE_HOST_PAYLOAD_BYTE_CAP, HostBridgeTranslator, HostCommandErrorCode,
    HostCommandStatus, HostInboundEnvelope, HostPatchPayload, HostResponseWire,
    HostSnapshotPayload, HostTransportError, MAX_HOST_COMMAND_PAYLOAD_BYTES,
};
use rvelte_fun_native_codegen::host_bridge::{
    FunNativeHostBridge, FunNativeHostCommandKind, FunNativeHostCommandPayload,
    FunNativeHostStateValue, HostAuthorizationContext,
};

fn launcher_auth() -> HostAuthorizationContext {
    HostAuthorizationContext {
        scope_label: String::from("launcher"),
        session_revision: 1,
        capability_mask: 0xFF,
    }
}

fn make_translator(scope: &str, declared_routes: Vec<u64>) -> HostBridgeTranslator {
    let mut bridge = FunNativeHostBridge::new(1, scope);
    bridge.set_declared_routes(declared_routes);
    let mut translator = HostBridgeTranslator::new(bridge);
    must_ok(translator.subscribe(HostAuthorizationContext {
        scope_label: String::from(scope),
        session_revision: 1,
        capability_mask: 0xFF,
    }));
    translator
}

fn must_ok<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("expected Ok, got {error:?}"),
    }
}

fn must_err<T: std::fmt::Debug, E>(result: Result<T, E>) -> E {
    match result {
        Ok(value) => panic!("expected Err, got Ok({value:?})"),
        Err(error) => error,
    }
}

// --------------------------------------------------------------------
// Constants and label-map metadata.
// --------------------------------------------------------------------

#[test]
fn payload_caps_match_fun_host_constants() {
    // The translator caps must equal fun_host's constants. If
    // fun_host changes the cap, this test breaks loudly.
    assert_eq!(MAX_HOST_COMMAND_PAYLOAD_BYTES, 64 * 1024);
    assert_eq!(FUN_NATIVE_HOST_PAYLOAD_BYTE_CAP, 16 * 1024);
}

#[test]
fn default_label_map_covers_well_known_command_kinds() {
    let map = CommandLabelMap::default_v1();
    assert_eq!(
        map.label_for(FunNativeHostCommandKind::LauncherLaunchProject),
        Some("launcher.show")
    );
    assert_eq!(
        map.label_for(FunNativeHostCommandKind::DiagnosticsRefresh),
        Some("runtime.diagnostics.list")
    );
    assert_eq!(
        map.kind_for("host.commandbar.execute"),
        Some(FunNativeHostCommandKind::SettingsUpdateValue)
    );
}

// --------------------------------------------------------------------
// Spec scenarios.
// --------------------------------------------------------------------

#[test]
fn scenario_launcher_snapshot_lifts_into_bridge() {
    let mut translator = make_translator("launcher", vec![100, 101, 102, 400_000]);
    let mut state = BTreeMap::new();
    state.insert(
        String::from("recent_count"),
        FunNativeHostStateValue::Numeric(7),
    );
    let envelope = HostInboundEnvelope::Snapshot(HostSnapshotPayload {
        component_id: 1,
        revision: 1,
        freshness_token: 1,
        authorization_context: launcher_auth(),
        payload_bytes: 32,
        state: state.clone(),
    });
    must_ok(translator.apply_inbound_envelope(envelope));
    let bridge_state = translator.bridge().state();
    assert_eq!(
        bridge_state.get("recent_count"),
        Some(&FunNativeHostStateValue::Numeric(7))
    );
}

#[test]
fn scenario_hud_patch_advances_bridge_revision() {
    let mut translator = make_translator("hud", vec![100, 400_000]);
    // Snapshot first to establish revision 1.
    let snapshot = HostInboundEnvelope::Snapshot(HostSnapshotPayload {
        component_id: 1,
        revision: 1,
        freshness_token: 1,
        authorization_context: HostAuthorizationContext {
            scope_label: String::from("hud"),
            session_revision: 1,
            capability_mask: 0xFF,
        },
        payload_bytes: 16,
        state: BTreeMap::new(),
    });
    must_ok(translator.apply_inbound_envelope(snapshot));
    // Patch advances revision to 2 and updates ammo state.
    let mut updates = BTreeMap::new();
    updates.insert(String::from("ammo"), FunNativeHostStateValue::Numeric(28));
    let patch = HostInboundEnvelope::Patch(HostPatchPayload {
        component_id: 1,
        route_id: None,
        correlation_id: None,
        response_revision: 2,
        previous_revision: 1,
        freshness_token: 2,
        authorization_context: HostAuthorizationContext {
            scope_label: String::from("hud"),
            session_revision: 1,
            capability_mask: 0xFF,
        },
        payload_bytes: 16,
        state_updates: updates,
    });
    must_ok(translator.apply_inbound_envelope(patch));
    assert_eq!(translator.bridge().last_revision(), 2);
    assert_eq!(
        translator.bridge().state().get("ammo"),
        Some(&FunNativeHostStateValue::Numeric(28))
    );
}

#[test]
fn scenario_pause_command_round_trips_through_correlation_table() {
    let mut translator = make_translator("pause_menu", vec![100, 101, 102]);
    // Submit a typed intent for PauseMenuResume.
    must_ok(translator.bridge_mut().submit_intent(
        100,
        FunNativeHostCommandKind::PauseMenuResume,
        FunNativeHostCommandPayload::Empty,
    ));
    // Drain the pending intents from the bridge directly so the
    // translator can translate each one to a wire command.
    let pending = translator.bridge().pending_intents().to_vec();
    assert_eq!(pending.len(), 1);
    let wire = must_ok(translator.submit_intent_as_wire(&pending[0]));
    assert_eq!(wire.command_id, "runtime.input.set_owner");
    // The host echoes a successful response with the same
    // request_id; the translator removes the pending row.
    let response = HostResponseWire {
        request_id: wire.request_id,
        sequence: 1,
        command_id: String::from("runtime.input.set_owner"),
        status: HostCommandStatus::Ok,
        error_code: None,
        payload_json: b"{}".to_vec(),
    };
    must_ok(translator.apply_command_response(response));
}

#[test]
fn scenario_diagnostics_refresh_carries_typed_payload() {
    let mut translator = make_translator("diagnostics", vec![100, 101, 102, 400_000]);
    must_ok(translator.bridge_mut().submit_intent(
        101,
        FunNativeHostCommandKind::DiagnosticsRefresh,
        FunNativeHostCommandPayload::Empty,
    ));
    let pending = translator.bridge().pending_intents().to_vec();
    let wire = must_ok(translator.submit_intent_as_wire(&pending[0]));
    assert_eq!(wire.command_id, "runtime.diagnostics.list");
    // Authorization is forwarded.
    assert_eq!(wire.authorization_context.scope_label, "diagnostics");
}

#[test]
fn scenario_unknown_command_is_rejected_at_response_label_check() {
    let mut translator = make_translator("launcher", vec![100, 101, 102, 400_000]);
    must_ok(translator.bridge_mut().submit_intent(
        100,
        FunNativeHostCommandKind::LauncherLaunchProject,
        FunNativeHostCommandPayload::Empty,
    ));
    let pending = translator.bridge().pending_intents().to_vec();
    let wire = must_ok(translator.submit_intent_as_wire(&pending[0]));
    // Host echoes a different command_id than the one the
    // translator dispatched. The translator rejects with
    // UnknownCommandLabel.
    let bad_response = HostResponseWire {
        request_id: wire.request_id,
        sequence: 1,
        command_id: String::from("never.declared.here"),
        status: HostCommandStatus::Ok,
        error_code: None,
        payload_json: b"{}".to_vec(),
    };
    let error = must_err(translator.apply_command_response(bad_response));
    assert!(matches!(
        error,
        HostTransportError::UnknownCommandLabel { .. }
    ));
    assert_eq!(
        error.code(),
        "fun.product.rvelte_bridge.host_transport.unknown_command_label"
    );
}

#[test]
fn scenario_stale_patch_is_rejected_by_underlying_bridge() {
    let mut translator = make_translator("launcher", vec![100, 101, 102, 400_000]);
    let snapshot = HostInboundEnvelope::Snapshot(HostSnapshotPayload {
        component_id: 1,
        revision: 1,
        freshness_token: 1,
        authorization_context: launcher_auth(),
        payload_bytes: 16,
        state: BTreeMap::new(),
    });
    must_ok(translator.apply_inbound_envelope(snapshot));
    // Apply a patch claiming previous_revision = 1; advances the
    // bridge to revision 2.
    let first = HostInboundEnvelope::Patch(HostPatchPayload {
        component_id: 1,
        route_id: None,
        correlation_id: None,
        response_revision: 2,
        previous_revision: 1,
        freshness_token: 2,
        authorization_context: launcher_auth(),
        payload_bytes: 16,
        state_updates: BTreeMap::new(),
    });
    must_ok(translator.apply_inbound_envelope(first));
    // Re-apply the same patch (previous_revision=1, but bridge is
    // at revision 2). The bridge surfaces StalePatch; the
    // translator wraps it as BridgeRejected.
    let stale = HostInboundEnvelope::Patch(HostPatchPayload {
        component_id: 1,
        route_id: None,
        correlation_id: None,
        response_revision: 2,
        previous_revision: 1,
        freshness_token: 2,
        authorization_context: launcher_auth(),
        payload_bytes: 16,
        state_updates: BTreeMap::new(),
    });
    let error = must_err(translator.apply_inbound_envelope(stale));
    assert!(matches!(error, HostTransportError::BridgeRejected { .. }));
}

#[test]
fn scenario_oversized_payload_is_rejected_at_translator_gate() {
    let mut translator = make_translator("launcher", vec![100]);
    let oversized_snapshot = HostInboundEnvelope::Snapshot(HostSnapshotPayload {
        component_id: 1,
        revision: 1,
        freshness_token: 1,
        authorization_context: launcher_auth(),
        payload_bytes: FUN_NATIVE_HOST_PAYLOAD_BYTE_CAP + 1,
        state: BTreeMap::new(),
    });
    let error = must_err(translator.apply_inbound_envelope(oversized_snapshot));
    assert!(matches!(
        error,
        HostTransportError::OversizedInboundPayload { .. }
    ));
    assert_eq!(
        error.code(),
        "fun.product.rvelte_bridge.host_transport.oversized_inbound"
    );
}

#[test]
fn scenario_host_disconnect_and_reconnect_round_trip() {
    let mut translator = make_translator("hud", vec![100]);
    assert!(translator.is_connected());

    // Snapshot succeeds while connected.
    let envelope = HostInboundEnvelope::Snapshot(HostSnapshotPayload {
        component_id: 1,
        revision: 1,
        freshness_token: 1,
        authorization_context: HostAuthorizationContext {
            scope_label: String::from("hud"),
            session_revision: 1,
            capability_mask: 0xFF,
        },
        payload_bytes: 16,
        state: BTreeMap::new(),
    });
    must_ok(translator.apply_inbound_envelope(envelope));

    // Disconnect: subsequent inbound payloads reject.
    translator.disconnect();
    assert!(!translator.is_connected());
    let post_disconnect = HostInboundEnvelope::Patch(HostPatchPayload {
        component_id: 1,
        route_id: None,
        correlation_id: None,
        response_revision: 2,
        previous_revision: 1,
        freshness_token: 2,
        authorization_context: HostAuthorizationContext {
            scope_label: String::from("hud"),
            session_revision: 1,
            capability_mask: 0xFF,
        },
        payload_bytes: 16,
        state_updates: BTreeMap::new(),
    });
    let error = must_err(translator.apply_inbound_envelope(post_disconnect.clone()));
    assert!(matches!(error, HostTransportError::Disconnected));

    // Reconnect: the translator accepts payloads again. The
    // bridge state survived the disconnect.
    translator.reconnect();
    assert!(translator.is_connected());
    must_ok(translator.apply_inbound_envelope(post_disconnect));
    assert_eq!(translator.bridge().last_revision(), 2);
}

// --------------------------------------------------------------------
// Cross-cutting invariants.
// --------------------------------------------------------------------

#[test]
fn unknown_correlation_id_in_response_is_rejected() {
    let mut translator = make_translator("launcher", vec![100]);
    // No prior submit_intent_as_wire was called, so request_id 99
    // is not in the pending table.
    let bad_response = HostResponseWire {
        request_id: 99,
        sequence: 1,
        command_id: String::from("launcher.show"),
        status: HostCommandStatus::Ok,
        error_code: None,
        payload_json: b"{}".to_vec(),
    };
    let error = must_err(translator.apply_command_response(bad_response));
    assert!(matches!(
        error,
        HostTransportError::UnknownCorrelation { .. }
    ));
}

#[test]
fn host_error_response_surfaces_typed_error_code() {
    let mut translator = make_translator("launcher", vec![100, 101, 102, 400_000]);
    must_ok(translator.bridge_mut().submit_intent(
        100,
        FunNativeHostCommandKind::LauncherLaunchProject,
        FunNativeHostCommandPayload::Empty,
    ));
    let pending = translator.bridge().pending_intents().to_vec();
    let wire = must_ok(translator.submit_intent_as_wire(&pending[0]));
    let error_response = HostResponseWire {
        request_id: wire.request_id,
        sequence: 1,
        command_id: String::from("launcher.show"),
        status: HostCommandStatus::Error,
        error_code: Some(HostCommandErrorCode::ProjectUnauthorized),
        payload_json: b"{\"error_code\":\"project_unauthorized\"}".to_vec(),
    };
    let error = must_err(translator.apply_command_response(error_response));
    assert!(matches!(
        error,
        HostTransportError::HostError {
            code: HostCommandErrorCode::ProjectUnauthorized
        }
    ));
}

#[test]
fn outbound_payload_cap_rejects_oversized_intent() {
    use rvelte_fun_native_codegen::host_bridge::{
        FUN_NATIVE_HOST_BRIDGE_SCHEMA, FunNativeHostCommandIntent,
    };
    let mut translator = make_translator("launcher", vec![100, 101, 102, 400_000]);
    // The bridge's `submit_intent` would reject anything over 16 KB.
    // Construct a synthetic intent directly to exercise the
    // translator's defense-in-depth 64 KB outbound gate.
    let huge = "x".repeat(MAX_HOST_COMMAND_PAYLOAD_BYTES + 4);
    let synthetic = FunNativeHostCommandIntent {
        schema_version: String::from(FUN_NATIVE_HOST_BRIDGE_SCHEMA),
        component_id: 1,
        route_id: 100,
        command: FunNativeHostCommandKind::LauncherLaunchProject,
        correlation_id: 1,
        request_revision: 1,
        freshness_token: 1,
        authorization_context: launcher_auth(),
        payload_bytes: 0,
        payload: FunNativeHostCommandPayload::String(huge),
    };
    let error = must_err(translator.submit_intent_as_wire(&synthetic));
    assert!(matches!(
        error,
        HostTransportError::OversizedOutboundPayload { .. }
    ));
}

#[test]
fn translator_keeps_subscription_state_isolated() {
    let mut translator = make_translator(
        "settings",
        vec![100, 101, 102, 103, 104, 105, 106, 107, 108],
    );
    must_ok(translator.bridge_mut().submit_intent(
        107,
        FunNativeHostCommandKind::SettingsUpdateValue,
        FunNativeHostCommandPayload::Numeric(42),
    ));
    let pending = translator.bridge().pending_intents().to_vec();
    let wire = must_ok(translator.submit_intent_as_wire(&pending[0]));
    // Request IDs are stable monotonic and start at 1.
    assert_eq!(wire.request_id, 1);
    // A second intent advances the request_id.
    must_ok(translator.bridge_mut().submit_intent(
        108,
        FunNativeHostCommandKind::Custom { code: 1 },
        FunNativeHostCommandPayload::Empty,
    ));
    let pending2 = translator.bridge().pending_intents().to_vec();
    let wire2 = must_ok(translator.submit_intent_as_wire(&pending2[1]));
    assert_eq!(wire2.request_id, 2);
    assert_eq!(wire2.command_id, "custom.1");
}
