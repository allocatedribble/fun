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

The first presentation target is Windows: a same-process transparent top-level
overlay surface tracks the game window content rect, DPI scale, focus/minimize
visibility, and click-through mode. Passive HUD mode is click-through; modal or
active regions switch the overlay to interactive. macOS transparent panels,
Linux X11 transparent windows, and Wayland-specific handling remain follow-up
platform work.

## Main page and bridge

The production page is a single browser instance rooted at
`fun-ui://main/index.html`. It uses internal routes for HUD, pause menu,
loadout, scoreboard, chat, loading, diagnostics, and devtools overlay. The
custom scheme resolves only the known HTML/CSS/JS assets and rejects traversal,
absolute path tricks, hidden path segments, and unknown routes.

`FUN_CEF_UI_DEV_SERVER=http://127.0.0.1:<port>` may redirect development page
loading to a loopback-only HTTP server. Production remains locked to `fun-ui://`
assets with no remote scripts, downloads, or popups.

The JavaScript bridge exposes `window.fun` with request/response, events,
subscriptions, and a single `receiveFromHost(envelope)` entrypoint. Rust uses a
typed `UiEnvelope` with separate control and state lanes; state updates are
patches rather than per-frame full JSON dumps.
