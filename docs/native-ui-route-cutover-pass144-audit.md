# Tier 7 / Pass 144 — NATIVE_UI/Browser Product-Runtime Audit

audit_id: fun_native_ui_removal_pass144_audit_v1
status: pass_144_complete_pass_145_blocked_on_route_wiring
owner: project-fun-ui-platform
scope: product_runtime_native_ui_and_browser_symbols
supersedes: extends [`rvelte/docs/native_ui-removal-inventory.md`](../../rvelte/docs/native_ui-removal-inventory.md) with file-and-line precision and current native-route status

## Decision Recap

NATIVE_UI is removed from the target architecture per Pass 38 inventory. Pass 144
audits the remaining product-runtime surface so Pass 145 can delete confidently
once the precondition (native launcher / HUD / pause / diagnostics route
mounts) is satisfied.

The replacement target is the native rvelte/FUN UI adapter:

- compiler output ⇒ `fun_ui_render_packet_v1`;
- packet consumer ⇒ `fun-renderer/src/ui/native_adapter.rs`;
- product-side bridge ⇒ `fun/game_client/ui/rvelte_bridge` (`ProductRvelteAdapter`).

## Status Labels

| status | meaning |
|---|---|
| `delete_now` | Remove on the next modifying turn; not blocked by other work. |
| `delete_after_route_cutover` | Active product code path; remove only after `game_client` mounts the native rvelte route for launcher / HUD / pause / diagnostics and the equivalent UI ships behind a feature flag. |
| `keep_test_reference_only` | Browser/Svelte material retained as a comparison fixture, not a product path. |
| `keep_archived_documentation_only` | Historical doc; preserve in place with a superseded-by note. |
| `keep_active_product` | Confirmed not NATIVE_UI — the native replacement (or a renamed alias). Keep. |

## 1. Inventory

### 1a. NATIVE_UI crate and game-client integration (`delete_after_route_cutover`)

| Path | Symbol family | LOC | Notes |
|---|---|---|---|
| [fun/fun_ui_native_ui/src/](../fun_ui_native_ui/src/) | NATIVE_UI crate root | ~2.5 KLOC | All 12 files are NATIVE_UI-only: `bootstrap.rs`, `bridge.rs`, `browser.rs`, `compositor.rs`, `diagnostics.rs`, `input.rs`, `lib.rs`, `model.rs`, `render_handler.rs`, `runtime.rs`, `scheme.rs`, `security.rs`. |
| [fun/fun_ui_native_ui/build.rs](../fun_ui_native_ui/build.rs) | NATIVE_UI build script | ~50 | Embeds `fun/game_client/ui/main/dist/**` for the `fun-ui://main/**` scheme handler. |
| [fun/fun_ui_native_ui/Cargo.toml](../fun_ui_native_ui/Cargo.toml) | crate manifest | 18 | Declares `native_ui = "147.1.0"` (line 14). |
| [fun/game_client/src/native_ui.rs](../game_client/src/native_ui.rs) | game-client NATIVE_UI integration entry | ~2 KLOC | `GameNativeUiPlugin` instantiated unconditionally inside `#[cfg(feature = "native_ui")]` at [fun/game_client/src/lib.rs:887-888](../game_client/src/lib.rs). Module gate at [fun/game_client/src/lib.rs:4-5](../game_client/src/lib.rs). |
| [fun/game_client/src/native_ui_dx12/](../game_client/src/native_ui_dx12/) | NATIVE_UI accelerated-paint transport | ~55 KB across 5 files | `mod.rs`, `bridge.rs` (~38 KB — D3D11On12 ring + dirty rect coordination), `diagnostics.rs`, `handles.rs`, `ring.rs`. Windows-only compile gate at [fun/game_client/src/lib.rs:2-3](../game_client/src/lib.rs). |
| [fun/fun_render/src/dx12_native/native_ui.rs](../fun_render/src/dx12_native/native_ui.rs) | renderer-side NATIVE_UI DX12 transport policy | ~100 | `Dx12NativeUiTransportPath` and `Dx12NativeUiTransportPolicy`. Re-exported from [fun/fun_render/src/dx12_native/mod.rs:17-20](../fun_render/src/dx12_native/mod.rs). |

### 1b. Build / runtime configuration (`delete_after_route_cutover`)

| Path | Surface | Notes |
|---|---|---|
| [fun/Cargo.toml](../Cargo.toml) | workspace member `fun_ui_native_ui` | line 11. |
| [fun/game_client/Cargo.toml](../game_client/Cargo.toml) | features `native_ui`, `native_ui_dx12_accelerated_paint` | feature rows 39–45; optional dep `fun_ui_native_ui` row ~75; DX12 deps `wgpu`, `windows` rows ~89–96. |
| [fun/scripts/stack/profiles/native_ui.cpu.json](../scripts/stack/profiles/native_ui.cpu.json) | stack runner profile | `paint_transport: cpu`, `enabled: true`. |
| [fun/scripts/stack/profiles/native_ui.d3d11on12.strict.json](../scripts/stack/profiles/native_ui.d3d11on12.strict.json) | stack runner profile | `paint_transport: d3d11on12`, `accelerated_strict: true`. |
| [fun/scripts/stack/stack.schema.json](../scripts/stack/stack.schema.json) | NATIVE_UI config block + env bindings | object `native_ui` (~line 69+); env vars `FUN_NATIVE_UI_TRANSPORT_STATUS_PATH`, `FUN_NATIVE_UI_PAINT_TRANSPORT`, `FUN_NATIVE_UI_ACCELERATED_PAINT` (~lines 155–157). |
| [fun/fun-renderer/Cargo.toml](../fun-renderer/Cargo.toml) | `native_ui_gpu_only` feature + alias | line 16 (empty deps), line 49 alias `fun_renderer_native_ui_gpu_only`. **Already a no-op**; can be deleted now if desired but harmless. Reclassify as `delete_now` (see §3). |

### 1c. Old browser UI packaging (`keep_test_reference_only`)

| Path | Surface | Notes |
|---|---|---|
| [fun/game_client/ui/main/](../game_client/ui/main/) | Vite + Svelte build, `dist/`, `package.json`, `vite.config.ts`, `svelte.config.js` | Builds the legacy browser UI. Not product after route cutover; retained per Pass 38 inventory as comparison fixture. |
| [fun/game_client/ui/main/src/lib/host/bridge.ts](../game_client/ui/main/src/lib/host/bridge.ts) | JS host bridge (~391 lines) | Uses `window.funHost.postMessage` first, falls back to `window.native_uiQuery` (lines ~95–111). Post-deletion, `native_uiQuery` fallback branch becomes dead and should be pruned even while the file stays. |
| [fun/game_client/ui/main/src/lib/host/commands.ts](../game_client/ui/main/src/lib/host/commands.ts) | bridge availability check | line 17 references `window.native_uiQuery`; same fallback pruning. |
| [fun/game_client/ui/main/dist/assets/index-ccr8AYVz.js](../game_client/ui/main/dist/assets/index-ccr8AYVz.js) | bundled output | one NATIVE_UI reference (compiled from bridge.ts fallback). Regenerated on next `npm run build`. |
| [fun/game_client/ui/main/docs/rvelte-dev-island.md](../game_client/ui/main/docs/rvelte-dev-island.md) | dev-island docs | Pass 20 dev-only diagnostics island under `VITE_FUN_RVELTE_DEV=1`. |

### 1d. Documentation (`keep_archived_documentation_only` or `delete_now`)

| Path | Surface | Action | Notes |
|---|---|---|---|
| [fun/docs/dx12_native_ui_accelerated_paint.md](dx12_native_ui_accelerated_paint.md) | architecture doc, ~23 KB | `keep_archived_documentation_only` | Add a superseded-by header pointing at this audit and the native adapter. |
| [fun/docs/dx12_dlss_current_state.md](dx12_dlss_current_state.md) | DLSS state doc | `keep_archived_documentation_only` | Mentions NATIVE_UI in capability-selector context. |
| [fun/docs/dx12_native_interop_governance.md](dx12_native_interop_governance.md) | governance doc | `keep_archived_documentation_only` | Mentions NATIVE_UI GPU transport. |
| [fun/docs/rendering/fun-render-migration.md](rendering/fun-render-migration.md) | renderer migration | `keep_archived_documentation_only` | Pass-era NATIVE_UI references. |
| [fun/docs/renderer_ownership.md](renderer_ownership.md) | ownership doc | `keep_archived_documentation_only` | NATIVE_UI mentioned as legacy product UI. |
| [fun/docs/rvelte_bridge.md](rvelte_bridge.md) | rvelte bridge product doc | `keep_active_product` | Names the native bridge. No NATIVE_UI imports. |
| [fun/docs/rvelte_bridge_fun_renderer_backend.md](rvelte_bridge_fun_renderer_backend.md) | renderer-backend doc | `keep_active_product` | Names the native renderer-backend. No NATIVE_UI imports. |
| [fun/README.md](../README.md) | fun overview | `delete_now` (rephrase, not file delete) | Lines ~52, 114, 169, 242, 260, 287, 294, 302 still describe NATIVE_UI as the active product UI. Per Pass 38 inventory `delete_now` for "target docs presenting NATIVE_UI as default." Replace with native rvelte phrasing (or qualify with "legacy until route cutover"). |
| [README.md](../../README.md) (root) | umbrella README | `delete_now` (rephrase) | Lines 33–35, 41–43 say "active NATIVE_UI/Svelte lane" and "fun_host + NATIVE_UI/Svelte path." Replace with native rvelte adapter phrasing. |
| [docs/data-platform/project-wide-telemetry-ledger.md](../../docs/data-platform/project-wide-telemetry-ledger.md) | telemetry ledger | `keep_archived_documentation_only` | Lists `NativeUiRenderRoleStatus`, `NativeUiComposition` as legacy telemetry families. |
| [fun-data/src/rules/dx12_doctrine.rs](../../fun-data/src/rules/dx12_doctrine.rs) | code-quality rule strings | `keep_active_product` (rule strings reference NATIVE_UI telemetry tokens; out of scope for this pass — rules naturally retire when the data they govern stops being produced) |

### 1e. Active product code that must NOT be deleted

| Path | Surface | Why kept |
|---|---|---|
| [fun/fun-renderer/src/ui/native_adapter.rs](../fun-renderer/src/ui/native_adapter.rs) | native UI adapter (~46 KB) | The replacement. Consumes `FunUiFramePacket`. Default-on via `native_ui_adapter` feature ([fun/fun-renderer/Cargo.toml:13](../fun-renderer/Cargo.toml)). |
| [fun/fun-renderer/src/ui/mod.rs](../fun-renderer/src/ui/mod.rs) | UI adapter module root | gates `native_adapter` submodule. |
| [fun/fun-renderer/src/passg_native_ui_product_route.rs](../fun-renderer/src/passg_native_ui_product_route.rs) | native UI product route | Active consumer of `fun_ui_render_packet_v1`. |
| [fun/fun-renderer/src/tier6_native_ui_rendering.rs](../fun-renderer/src/tier6_native_ui_rendering.rs) | native UI render tier | Active. |
| [fun/game_client/ui/rvelte_bridge/](../game_client/ui/rvelte_bridge/) | product rvelte adapter (Pass 72) | `ProductRvelteAdapter`, `ProductRouteRegistry`, `FunNativeUiRuntime`, `FunRenderUiAdapter` consumer wiring. **Not yet mounted in `game_client::lib`** (see §2). |

## 2. Native-route coverage status (Pass 145 precondition)

**Headline:** the native rvelte product adapter exists, compiles, and tests
green. It is **not yet mounted in `game_client` or `fun_host`** — the
`fun_rvelte_bridge` crate has zero call sites outside its own crate and tests.

| Surface | Native route declared? | Native route mounted in product? | Today's product backend |
|---|---|---|---|
| Launcher shell | yes ([rvelte_bridge/src/route_registry.rs:21](../game_client/ui/rvelte_bridge/src/route_registry.rs)) | **no** | NATIVE_UI (when `native_ui` feature on) / nothing (when off) |
| HUD overlay | yes (route_registry.rs:23) | **no** | NATIVE_UI |
| Pause menu | yes (route_registry.rs:25) | **no** | NATIVE_UI |
| Diagnostics list | yes (route_registry.rs:27) | **no** | NATIVE_UI; dev-only island under `VITE_FUN_RVELTE_DEV=1` |
| Command bar | yes (route_registry.rs:29) | **no** | NATIVE_UI |
| Settings shell | yes (route_registry.rs:31) | **no** | NATIVE_UI |

**Evidence of un-mounted state.** Searched
`fun/game_client/src/**/*.rs` and `fun/fun_host/src/**/*.rs` for
`rvelte_bridge`, `RvelteBridge`, `FunNativeUiRuntime`, `ProductRvelteAdapter`
— **zero matches** outside the crate itself and its `tests/`. NATIVE_UI is still
the only product UI plugin added to the Bevy `App`
([fun/game_client/src/lib.rs:887-888](../game_client/src/lib.rs)).

## 3. Pass 145 deletion list (with gating)

### 3a. Safe to delete on Pass 145 turn (no further precondition)

These are doc/feature-string changes whose semantic meaning is already gone:

- [fun/fun-renderer/Cargo.toml](../fun-renderer/Cargo.toml) lines 16, 49: empty `native_ui_gpu_only` feature and its `fun_renderer_native_ui_gpu_only` alias. (No callers; verifiable with a workspace grep.)
- Root [README.md](../../README.md) lines 33–35, 41–43, and the third-bullet "Product UI is NATIVE_UI/Svelte only" claim near line 169 of [fun/README.md](../README.md): rephrase to name native rvelte as the canonical product UI path. (Pass 38 marked these `delete_now`.)

### 3b. Blocked until launcher / HUD / pause / diagnostics native routes mount

Order matters; do them as one atomic Pass 145 turn after the route-mount pass:

1. **Game-client gate flip:**
   - [fun/game_client/src/lib.rs](../game_client/src/lib.rs) lines 2–7 (native_ui module gates) and lines 887–888 (`GameNativeUiPlugin` registration).
   - Replace with `ProductRvelteAdapter`/`FunNativeUiRuntime` mount, fed by `fun_host`'s host snapshots.
2. **Crate deletion:**
   - [fun/fun_ui_native_ui/](../fun_ui_native_ui/) (entire crate).
   - [fun/game_client/src/native_ui.rs](../game_client/src/native_ui.rs).
   - [fun/game_client/src/native_ui_dx12/](../game_client/src/native_ui_dx12/) (entire dir).
   - [fun/fun_render/src/dx12_native/native_ui.rs](../fun_render/src/dx12_native/native_ui.rs) and its re-export at [mod.rs:17-20](../fun_render/src/dx12_native/mod.rs).
3. **Manifest cleanup:**
   - [fun/Cargo.toml](../Cargo.toml) line 11 (`fun_ui_native_ui` workspace member).
   - [fun/game_client/Cargo.toml](../game_client/Cargo.toml) features `native_ui`, `native_ui_dx12_accelerated_paint` and the corresponding optional deps.
4. **Build profile / schema:**
   - [fun/scripts/stack/profiles/native_ui.cpu.json](../scripts/stack/profiles/native_ui.cpu.json).
   - [fun/scripts/stack/profiles/native_ui.d3d11on12.strict.json](../scripts/stack/profiles/native_ui.d3d11on12.strict.json).
   - [fun/scripts/stack/stack.schema.json](../scripts/stack/stack.schema.json) `native_ui` config block + the three `FUN_NATIVE_UI_*` env-var bindings.
5. **Browser-bridge fallback pruning** (optional same-pass cleanup, not file deletion):
   - [fun/game_client/ui/main/src/lib/host/bridge.ts](../game_client/ui/main/src/lib/host/bridge.ts) lines ~95–111: drop the `window.native_uiQuery` fallback branch.
   - [fun/game_client/ui/main/src/lib/host/commands.ts](../game_client/ui/main/src/lib/host/commands.ts) line 17: drop the `native_uiQuery` availability check.
6. **Doc supersession headers** for the architecture docs in §1d marked `keep_archived_documentation_only`.

### 3c. Out of scope for Pass 145 (not deleted)

- [fun/game_client/ui/main/](../game_client/ui/main/) browser tree — Pass 38 keeps it as test reference fixture. A later "Tier 8: minimize browser fixture" pass can decide its end state.
- [fun_editor/](../../fun_editor/) — archived workspace.
- Telemetry name strings for `NativeUiRenderRoleStatus` / `NativeUiComposition` in `fun-data` rules — they retire naturally when no producer emits them.

## 4. Route-cutover work the next pass must do (Pass 145 precondition)

Stated as a forward checklist for whoever runs the route-mount pass before
Pass 145:

1. In `fun/game_client/Cargo.toml`, add `fun_rvelte_bridge` (path
   `ui/rvelte_bridge`) as a non-optional dependency and add a feature
   `native_ui_routes` (default off) that gates the new mount until parity is
   measured.
2. In `fun/game_client/src/lib.rs`, behind `#[cfg(feature = "native_ui_routes")]`,
   construct a `FunNativeUiRuntime` (using the default fixture resolver), wire
   `ProductRouteRegistry::with_well_known()`, and feed its `FrameOutput` to the
   `fun-renderer` native adapter via the existing `FunRendererPacketConsumer`.
3. Wire `fun_host` host snapshots / patches into `FunNativeUiRuntime` via
   `host_transport`.
4. Translate Bevy/winit input into `ProductInputEvent` via
   `fun_rvelte_bridge::input_translator::translate_product_input` and route it
   through the runtime.
5. Verify launcher / HUD / pause / diagnostics render correctly under
   `--features native_ui_routes` for one full session each.
6. Flip the default feature set: enable `native_ui_routes`, disable `native_ui`.
7. Land Pass 145 (this audit's deletion list 3a + 3b) on top.

## 5. Risks and surprises

- **No TODO/FIXME blockers** were found in `fun/game_client/src/**` for the
  cutover (`TODO.*native_ui`, `FIXME.*native_ui`). The cutover is a design task, not a
  finish-incomplete-code task.
- **`fun_renderer_native_ui_gpu_only` is already a no-op** (feature deps list is
  empty) — listed in §3a for cleanup.
- **Bridge.ts still ships NATIVE_UI fallback** even though the file lives in the
  test-reference tree. Worth pruning at Pass 145 to remove confusion when
  someone reads the file expecting a clean native bridge.
- **No NATIVE_UI code outside the predicted scope.** All discovered NATIVE_UI references
  fall inside the Pass 38 inventory's surface set; nothing snuck into
  `fun_render/`, `fun-renderer/` core, `fun-scene`, `fun-lux`, `fun_host`, or
  `thunder` outside the documented seam.
- **`fun-backend/` and `frontend-observer/` stay untouched.** Both contain
  Vite/Svelte but are documented as account/observer surfaces, not product
  game UI; out of scope for Pass 144/145 by AGENTS.md boundary rules.

## Acceptance Criteria Check

- [x] **A deletion list exists.** §3a (immediate) and §3b (post-route-mount).
- [x] **No product symbol is ambiguous.** Every NATIVE_UI/browser symbol in the
      audited scope is classified with a path and a status; the active
      product replacement is identified per surface.

## Pass 145 Status

**BLOCKED on its stated precondition.** The user's Pass 145 goal explicitly
reads "Remove NATIVE_UI from product runtime *once launcher/HUD/pause/diagnostics
are covered*." Native routes are declared and tested in
`fun/game_client/ui/rvelte_bridge` but not mounted in `fun/game_client/src` or
`fun/fun_host/src`. Run the §4 route-cutover pass first; then Pass 145 becomes
a mechanical deletion of §3b's items.
