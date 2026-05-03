# fun_ui_cef

`fun_ui_cef` owns the Chromium Embedded Framework browser UI subsystem for the
game client. It is intentionally a sibling of `fun_render`, not a render feature
inside it.

## Contract

- CEF owns browser lifetime, page loading, JavaScript bridge messages,
  windowless paint callbacks, dirty rects, and UI compositor state.
- Bevy ECS owns game state and sends typed UI data through the bridge.
- Bevy render owns game rendering only.
- CEF paint output is not a Bevy RenderGraph node, Bevy image asset, sprite, or
  ECS mesh.
- The main browser page is `fun-ui://main/index.html` with a transparent
  background, so the game remains visible behind transparent pixels.
- Tauri and WRY are not game UI runtime dependencies.

## Rendering model

- CEF runs windowless/offscreen and sends BGRA premultiplied paint buffers to a
  render handler.
- `CefUiCompositor` owns triple-buffered UI pixels and frame metadata: width,
  height, scale factor, generation, dirty rects, alpha mode, and timestamp.
- Dirty rects are coalesced when the list is large. Full-frame upload is
  reserved for first frame, resize, scale change, empty dirty rects, or dirty
  rect explosion.
- The compositor is not a Bevy ECS resource requirement, render graph node,
  Bevy image, sprite, or `fun_render` feature.

## Overlay contract

The active presentation target is a single transparent, windowless CEF browser
whose pixels are composited into the game window. Passive HUD mode lets gameplay
input pass through except registered hit regions. Launcher, editor, pause menu,
text entry, and commandbar modes route capture through Rust-owned host input
ownership and `GameplayInputGate`. Native child windows, browser embedding,
Tauri surfaces, and operating-system overlay windows are not runtime UI paths.

## Main page and bridge

The production page is a single browser instance rooted at
`fun-ui://main/index.html`. It uses internal routes for HUD, pause menu,
launcher, editor, loading, diagnostics, and devtools overlay. The custom scheme
serves only the Vite build output recorded in the generated Rust asset manifest,
including hashed `/assets/...` files, and rejects traversal, absolute path
tricks, hidden path segments, and unknown routes.

`FUN_CEF_UI_DEV_SERVER=http://127.0.0.1:<port>` may redirect development page
loading to a loopback-only HTTP server. Production remains locked to `fun-ui://`
assets with no remote scripts, downloads, or popups.

The JavaScript bridge exposes `window.fun` with request/response, events,
subscriptions, and a single `receiveFromHost(envelope)` entrypoint. Rust uses a
typed `UiEnvelope` with separate control and state lanes; state updates are
patches rather than per-frame full JSON dumps. Host-level launcher/editor
commands travel over a bounded `HostCommand` control payload and are routed by
the Rust `FunClientHost` service, not by Tauri commands or browser-side
authority.

Compatibility commands such as `viewport.client.launch` and
`viewport.client.focus` target the current Fun client host. They do not launch,
embed, focus, or resize a separate client process. Editor preview commands use
the current client render as the background under CEF editor panels; the browser
page reports layout and hit regions instead of rendering preview pixels.

Host commands are typed before dispatch. `HostCommandRequest` carries a
searchable dot-separated command ID, request ID, payload bytes, requested
capability, and size budget. The bridge derives the command target and required
capability, rejects spoofed or missing capability grants, rejects oversize
payloads, and returns either `Ok`, `Rejected`, or `Failed` with bounded
diagnostics. The active host state is exposed as a Rust-owned `FunHostState`
snapshot plus patches; Svelte only mirrors that state.
