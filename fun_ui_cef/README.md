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
