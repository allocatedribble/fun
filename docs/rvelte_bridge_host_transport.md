# fun-rvelte-bridge host transport (pass 73)

contract_id: fun_product_rvelte_bridge_host_transport_v1
status: pass_73_product_host_bridge_integration
owner: project-fun-ui-platform
schema_label: fun.product.rvelte_bridge.host_transport.v1
related_docs:
  - `rvelte_bridge.md` (pass 72 skeleton)
  - `../../rvelte/docs/project-fun-native-adapter-design.md` (pass 71)
  - `../../rvelte/crates/rvelte-fun-native-codegen/src/host_bridge.rs`
  - `fun_host/src/lib.rs`

## Decision

Pass 73 connects the rvelte FUN-native host bridge to the
product-side `fun_host` wire format. The translator lives in
`fun-rvelte-bridge::host_transport` and mirrors `fun_host`'s
`FunHostCommandRequest` / `FunHostCommandResponse` /
`FunHostCommandErrorCode` shapes **without** pulling Bevy or any
graphics-API crate into the bridge's production tree.

The actual Bevy event plumbing — translating between this module's
wire types and `fun_host`'s `Message`-derived structs — is a
trivial mapping the consumer (a Bevy plugin in `game_client` or
`fun_host` itself) writes once. Pass 73 keeps that translation a
typed pure-data surface so the bridge crate stays free of bevy,
wgpu, dx12, vulkan, metal, native_ui, and swapchain references.

## Wire-Format Mirror

| translator type | mirrors |
|---|---|
| `HostCommandWire` | `fun_host::FunHostCommandRequest` |
| `HostResponseWire` | `fun_host::FunHostCommandResponse` |
| `HostCommandStatus` | `fun_host::FunHostCommandStatus` |
| `HostCommandErrorCode` | `fun_host::FunHostCommandErrorCode` |
| `HostInboundEnvelope` | typed snapshot/patch wrapper |
| `MAX_HOST_COMMAND_PAYLOAD_BYTES` | `fun_host::MAX_HOST_COMMAND_PAYLOAD_BYTES` (64 KB) |
| `FUN_NATIVE_HOST_PAYLOAD_BYTE_CAP` | `rvelte_fun_native_codegen::host_bridge::FUN_NATIVE_HOST_PAYLOAD_BYTE_CAP` (16 KB) |

The two caps are intentionally different: 16 KB is the rvelte
bridge's inbound cap (snapshots and patches arriving from the
host); 64 KB is `fun_host`'s outbound cap (commands the bridge
sends to the host). The translator enforces both at the
appropriate gate before the rvelte bridge or `fun_host` re-applies
its own gate.

## Command-Label Map

`CommandLabelMap::default_v1()` ships the pass-73 mapping between
typed rvelte command kinds and `fun_host` wire labels:

| `FunNativeHostCommandKind` | `fun_host` label |
|---|---|
| `LauncherLaunchProject` | `launcher.show` |
| `LauncherDismissProject` | `launcher.hide` |
| `HudAcknowledgeStatus` | `runtime.host.status` |
| `PauseMenuResume` | `runtime.input.set_owner` |
| `PauseMenuQuit` | `viewport.client.stop` |
| `DiagnosticsReadEntry` | `host.snapshot.get` |
| `DiagnosticsRefresh` | `runtime.diagnostics.list` |
| `SettingsUpdateValue` | `host.commandbar.execute` |
| `Custom { code: 0..=2 }` | `custom.<code>` |
| `Custom { code: _ }` | `custom` |

Future passes can extend the map without breaking existing tests —
the binding is constructed at runtime by `default_v1()` so a
caller can supply a richer map via `CommandLabelMap::new()`.

## Policy Enforcement

`HostBridgeTranslator` enforces all four pass-73 policies:

| policy | enforcement |
|---|---|
| **correlation** | `submit_intent_as_wire` stamps a monotonic `request_id` and stores a `PendingRequest` row keyed by it. `apply_command_response` removes the row by `request_id`; missing rows surface `UnknownCorrelation`. |
| **freshness** | `apply_inbound_envelope` forwards to `FunNativeHostBridge::apply_snapshot` / `apply_patch`, which already enforce monotonic `previous_revision` / `response_revision` semantics; the translator surfaces stale rejections as `BridgeRejected`. |
| **authorization** | every wire command and response carries an `HostAuthorizationContext`; the bridge re-validates `scope_label` and `session_revision != 0` for every payload, and the translator rejects responses whose `command_id` does not match the originating intent's mapped label. |
| **payload caps** | inbound: 16 KB cap before the bridge sees the payload (`OversizedInboundPayload`); outbound: 64 KB cap before any wire command leaves the translator (`OversizedOutboundPayload`). |

## Spec Tests

| spec test | name | what it asserts |
|---|---|---|
| 1. launcher snapshot | `scenario_launcher_snapshot_lifts_into_bridge` | snapshot envelope decodes + applies; bridge state reflects `recent_count` |
| 2. HUD patch | `scenario_hud_patch_advances_bridge_revision` | patch advances revision from 1 to 2 + writes `ammo` state |
| 3. pause command | `scenario_pause_command_round_trips_through_correlation_table` | typed `PauseMenuResume` intent maps to `runtime.input.set_owner`; response with the same `request_id` clears the pending row |
| 4. diagnostics refresh | `scenario_diagnostics_refresh_carries_typed_payload` | `DiagnosticsRefresh` maps to `runtime.diagnostics.list`; authorization scope flows through |
| 5. unknown command rejection | `scenario_unknown_command_is_rejected_at_response_label_check` | response with mismatching `command_id` rejects as `UnknownCommandLabel` |
| 6. stale patch rejection | `scenario_stale_patch_is_rejected_by_underlying_bridge` | re-applying the same `previous_revision` rejects as `BridgeRejected` (wraps `FunNativeHostError::StalePatch`) |
| 7. oversized payload rejection | `scenario_oversized_payload_is_rejected_at_translator_gate` + `outbound_payload_cap_rejects_oversized_intent` | inbound 16 KB cap fires `OversizedInboundPayload`; outbound 64 KB cap fires `OversizedOutboundPayload` |
| 8. host disconnect / reconnect | `scenario_host_disconnect_and_reconnect_round_trip` | `disconnect()` rejects subsequent inbound payloads with `Disconnected`; `reconnect()` re-accepts them; bridge state survives across the disconnect |

Plus four cross-cutting invariant tests: payload caps match
`fun_host` constants; default label map covers well-known kinds;
unknown correlation IDs reject; host error responses surface typed
error codes; subscription state stays isolated.

All 14 host_transport tests pass.

## Validation

```text
cargo fmt -p fun-rvelte-bridge --check          # exit 0
cargo check -p fun-rvelte-bridge                 # exit 0
cargo clippy -p fun-rvelte-bridge --all-targets -- -D warnings   # exit 0
cargo test -p fun-rvelte-bridge                  # 27/27 passing (13 smoke + 14 host transport)
cargo tree -p fun-rvelte-bridge -e normal | grep -ciE "wgpu|vulkan|metal|dx12|d3d12|native_ui|swapchain|fun-renderer|fun_renderer|^bevy"   # 0
```

Plus regression: `cargo test --workspace` from `rvelte/` continues
to report 74 test groups, zero failures.

## Forbidden Surface (Unchanged from Pass 72)

Pass-73 invariant: zero concrete renderer dependency, zero Bevy
dependency, zero browser dependency.

- `Cargo.toml` adds `serde_json` for wire JSON encoding; nothing
  else.
- `cargo tree -p fun-rvelte-bridge -e normal | grep -ciE
  "wgpu|vulkan|metal|dx12|d3d12|native_ui|swapchain|fun-renderer
  |fun_renderer|^bevy"` returns 0.

## Forward Pointers

Per the pass-71 design's migration plan:

- **Pass 74** — wire `fun-renderer`'s `native_ui_adapter` feature
  as the concrete `Renderer2DCommandSink`. The host transport API
  defined here does not change.
- **Pass 75** — wire product input events from the existing
  game-client input pipeline.
- **Pass 76** — add `--rvelte-bridge=<route>` to the game-client
  binary so a single product binary can boot any of the six
  routes through the adapter.

A small Bevy adapter living in `game_client` (or a new
`fun_host_bridge` plugin) will translate between the wire types
defined here and `fun_host`'s `Message`-derived structs. That
plugin is the *only* place Bevy enters the rvelte bridge code
path.

## Exit Criteria

- Native UI uses real host authority: every typed
  rvelte command kind maps to a `fun_host` wire label, every
  inbound snapshot / patch lifts through
  `apply_inbound_envelope`, every outbound intent serializes
  through `submit_intent_as_wire`, and every response round-trips
  through `apply_command_response`.
- Browser bridge remains gone: the production tree contains zero
  references to browser globals, JS bridges, or graphics-API
  crates. The wire format is JSON-encoded typed data, not a JS
  callback.
