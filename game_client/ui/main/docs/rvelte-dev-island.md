# rvelte Dev Island

adapter_id: fun.game_client.ui.main.rvelte_dev_island.v1
status: pass_30_typed_host_bridge_dev_panel
owner: fun_product_ui
scope: diagnostics_route_only
feature_flag: `VITE_FUN_RVELTE_DEV=1`
default_enabled: false
component_id: diagnostics_log_panel
manifest_id: rvelte.manifest.diagnostics_log_panel_component.v1
panel_source: src/lib/components/LogPanel.svelte

## Boundary

- Svelte remains the shell for `fun/game_client/ui/main`.
- rvelte assets are loaded only from `/rvelte-dev/` after the feature flag is
  enabled.
- Generated rvelte assets are not committed; build the rvelte browser dev-panel
  fixture from `rvelte/`, then run `npm run build:rvelte-dev` to populate
  `public/rvelte-dev/`.
- The island mounts a rvelte diagnostics log panel derived from the real Svelte
  `LogPanel.svelte` diagnostics route panel.
- The original Svelte `LogPanel` remains rendered by `DiagnosticsShell` as the
  fallback and visual comparison surface.
- Browser JavaScript owns only DOM handles, Wasm loading, patch commit, and
  event forwarding for this fixture.
- Host payloads pass through `fun.rvelte.dev_bridge.v1` validation before they
  can seed rvelte state.
- rvelte refresh events are forwarded to `window.fun` as
  `rvelte.dev_panel.refresh_requested` with a typed payload.
- rvelte host requests are also forwarded to `window.fun` as
  `rvelte.host_bridge.request` using `rvelte.host_bridge.v1`. The browser
  request only asks for `diagnostics.refresh`; Rust host validation remains the
  authority for command authorization, freshness, and response payloads.
- Snapshot rows are bounded, string-capped, and redacted before crossing from
  Svelte state into the rvelte Wasm fixture.

## Failure Diagnostics

| code | condition | behavior |
|---|---|---|
| `rvelte.wasm_missing` | `/rvelte-dev/dev_panel.wasm` cannot be fetched or instantiated | island fails closed |
| `rvelte.manifest_mismatch` | generated module manifest ID differs from `rvelte.manifest.diagnostics_log_panel_component.v1` | island fails closed |
| `rvelte.facade_incompatible` | generated module is missing or facade ABI differs from `1` | island fails closed |
| `rvelte.host_bridge_unavailable` | `window.funHost` is unavailable | island mounts locally and reports host forwarding disabled |
| `rvelte.host_payload_invalid` | Svelte-hosted mount payload fails schema/range validation | island fails closed |

## Migration Scan

```text
cargo run -p rvelte-cli -- migrate-svelte scan ../fun/game_client/ui/main/src/lib/components/LogPanel.svelte --format machine-fixture
```

The scan classifies `LogPanel.svelte` as unsupported for automatic conversion
because its script block is TypeScript. It still detects one prop and keyed
template rows. Pass 29 therefore hand-authors the `.rvt` source and manifest
instead of silently dropping behavior.

## Commands

```text
node examples/browser-dev-panel/build.mjs
node examples/browser-dev-panel/smoke.mjs
node examples/browser-dev-panel/host-bridge-smoke.mjs
node examples/browser-dev-panel/benchmark.mjs
npm run build:rvelte-dev
Set VITE_FUN_RVELTE_DEV=1 in the environment, then run npm run check
Set VITE_FUN_RVELTE_DEV=1 in the environment, then run npm run build
```

Run the first command from `rvelte/`; run the remaining commands from
`fun/game_client/ui/main`.

The benchmark report is a rendered local measurement view:

```text
rvelte/target/rvelte/bench/pass29-project-fun-dev-panel/dev-panel-benchmark.report.md
```

It is not canonical telemetry and must not be used for product performance
claims without the compressed protobuf benchmark path.

## Product-Lane Rule

The production default path does not load rvelte runtime assets. If
`VITE_FUN_RVELTE_DEV` is unset or any value other than `1`, `DiagnosticsShell`
does not render the island and no `/rvelte-dev/dev-panel.js` dynamic import is
issued. `vite.config.ts` also disables `publicDir` when the flag is off so a
default build does not copy the dev island fixture assets into `dist/`.

## Rollback

Unset `VITE_FUN_RVELTE_DEV` or set it to any value other than `1`. The route
returns to the Svelte-only diagnostics path and the rvelte assets are neither
imported nor copied into a default production build.
