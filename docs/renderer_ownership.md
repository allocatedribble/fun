# Renderer Ownership

status: active
owner_repo: fun
scope: fun-renderer, fun-lux, fun_render, fun-ai, bevy

## Package Map

| package | crate | folder | owner | purpose |
| --- | --- | --- | --- | --- |
| `fun-scene` | `fun_scene` | `fun/fun-scene` | scene authoring | FUN-owned `fun!`/`fun_list!` scene macros, deterministic scene manifests, runtime scene spawning, editor scene authoring, server scene authority, streaming declarations, renderer-facing scene components, lighting/GI authoring components |
| `fun-renderer` | `fun_renderer` | `fun/fun-renderer` | renderer core | default renderer core, virtual geometry, virtual shadows, GPU scene database, frame graph, page scheduler, renderer-owned CEF compositor, upscaling/frame-generation orchestration, DX12/Vulkan backend abstraction, `bevy_ecs` extraction/scheduling/GPU-scene integration |
| `fun-lux` | `fun_lux` | `fun/fun-lux` | lighting | direct lighting, many-light sampling, virtual shadow policy, GI, reflections, denoising/reconstruction policy, radiance/surface/probe caches |
| `fun_render` | `fun_render` | `fun/fun_render` | Bevy/game bridge | extraction, app/plugin integration, feature flags, legacy compatibility, diagnostics, benchmark integration |

`fun-renderer` depends on `fun-scene` and `fun-lux`. `fun_render` depends on and
re-exports `fun_renderer`, `fun_scene`, and `fun_lux` for Bevy/game integration.
This makes Fun Scene and Fun Lux part of the renderer product/API while keeping
scene authoring and lighting code in maintainable crates.

## ECS-First Scene Flow

`fun-scene` is the authoring and scene-entity source of truth. It starts from
the local Bevy fork's confirmed scene resolver and exposes FUN-owned `fun!` and
`fun_list!` macros. The intended flow is:

```text
Bevy scene resolver
  -> fun-scene / fun!
  -> ECS entities, typed components, observers, stable identities
  -> fun_render extraction and RenderApp scheduling
  -> fun-renderer GPU scene DB, frame graph, virtual geometry, virtual shadows
  -> fun-lux light DB, virtual shadow policy, many-light sampling, GI/reflections
```

The renderer must not pull opaque scene blobs out of gameplay and mutate secret
render objects. It consumes typed ECS components, archetypes, resources, events,
observers, and change ticks.

## ECS Renderer Schedule

The renderer path is split into explicit ECS phases:

- Main World: `fun-scene` spawn, hot reload, procedural expansion, gameplay
  mutation, streaming mutation, editor mutation, lighting mutation, and
  renderer component mutation.
- Extract: copy compact renderer-relevant deltas into the Render World.
- Render World: ECS systems update the GPU scene DB, virtual geometry
  residency, virtual shadow demand, `fun-lux` light DB, frame graph, upscale/UI
  composition boundary, and backend execution.

`FunSceneSet` orders scene work as `Resolve`, `Validate`, `Spawn`, and `Patch`.
`FunRendererSet` orders render work as `Extract`, `PrepareScene`,
`PrepareResources`, `Visibility`, `VirtualGeometry`, `VirtualShadows`, `Lux`,
`Upscale`, `UiComposite`, and `Present`. `fun_render` registers the first
renderer/lux resources and message queues in both the app world and RenderApp
when the bridge is installed.

The first renderer ECS lane covers these stable events: scene spawned, scene
patched, chunk loaded/unloaded, geometry changed, material changed, light
changed, transform changed, page fault, shadow invalidated, GI cache
invalidated, upscaler reset, device lost/restored, and CEF GPU frame available.
Change detection must route transform changes to motion data, material changes
to material tables, geometry changes to page residency, light changes to
`fun-lux` candidate tables plus shadow invalidation, and scene patches to
manifest signatures.

Scene authoring component names do not repeat the product prefix. The crate path
already supplies context, so the first-party component taxonomy uses names such
as `SceneStableIdentity`, `SceneRevision`, `SceneChunkId`, `Renderable`,
`VirtualGeometryAuthoring`, `RendererBounds`, `LuxLight`, `LuxEmissive`,
`LuxGiParticipant`, `CefSurface`, `ViewportUiTarget`, `UpscalePolicy`, and
`ViewportRenderPolicy`.

Hot-path scene archetypes are created with these typed components instead of
opaque render objects. Render-world markers and backend resources can still use
renderer-local implementation names, but scene-authored data should stay short
and domain-specific.

Renderer entities carry compact identities and handles. Large material data,
GPU buffers, residency tables, pipeline state, page allocation, light tables,
scene manifests, and viewport state live in resources, assets, or renderer
tables, not on every ECS entity. Sparse components are reserved for editor-only
metadata, debug-only declarations, optional GI/probe participation, and rare
special effects.

## Default Renderer

`fun-renderer` is the product default renderer even while the first visual
output is simple.

- `FUN_RENDERER_BACKEND=fun`: long-term default.
- `FUN_RENDERER_BACKEND=legacy`: temporary transition path for one migration
  cycle.
- Long-term legacy path: removed.

## Ownership Rules

- `fun-renderer` owns renderer-side feature interfaces, tensor input/output
  schemas, GPU resource handles, fallback heuristic paths, renderer-owned CEF
  composition, backend abstraction, frame graph, page scheduling, GPU scene
  data, virtual geometry, and virtual shadows.
- `fun-lux` owns direct lighting, many-light sampling, virtual shadow policy,
  GI, reflections, denoising/reconstruction policy, and radiance/surface/probe
  caches.
- `fun_render` owns Bevy-facing extraction, app/plugin integration, feature
  flags, diagnostics, benchmark integration, and legacy compatibility during the
  migration. It should stop being the renderer brain as code moves into
  `fun-renderer` and `fun-lux`.
- `fun-ai` owns model registry, model manifests, inference backend selection,
  evals, model trust/versioning, and offline training/evaluation hooks. Do not
  embed reusable model runtime code inside `fun-renderer`.
- Bevy remains ECS, app scheduling, extraction framework, and asset/event
  plumbing where useful. `fun-renderer` must work with and take advantage of
  `bevy_ecs` for ECS-shaped state, extraction, scheduling, frame-graph
  coordination, and GPU-scene ownership. Bevy renderer changes are allowed only
  for backend capability reporting, sanctioned native handle or command-list
  access, diagnostics, and unavoidable low-level scheduling primitives.

## CEF Boundary

`fun_ui_cef` owns browser lifetime, page loading, JavaScript bridge messages,
offscreen paint callbacks, and browser-facing command envelopes.
`fun-renderer` owns the renderer-side compositor contract for CEF pixels and GPU
resources. CEF UI remains a HUD/UI layer after world rendering, lighting,
upscaling/reconstruction, and post-processing; it must not feed temporal
reconstruction inputs or Ray Reconstruction guide buffers.

Product CEF UI is GPU-only. Accelerated shared-texture transport is required.
If GPU transport fails, the UI subsystem fails with a clear diagnostic. Runtime
lanes must not silently fall back to CPU `OnPaint` uploads. Temporary CPU
golden-image fixtures may exist only when clearly test-only and not compiled
into runtime/product lanes.

## UI Boundary

CEF/Svelte is the only product UI surface. Runtime/product lanes must not use
Bevy UI for launcher UI, editor UI, game HUD, diagnostics panels, or debug
overlays. Bevy UI may exist only in temporary test-only code that is clearly not
compiled into runtime/product lanes.

## Presentation Boundary

DLSS, FSR, and frame generation are renderer features. `fun-renderer` owns the
presentation/upscale boundary. Scene color and UI color stay separate. Frame
generation receives HUD-less scene color, UI color, depth, motion vectors, and
valid present-time resource lifetimes. Editor viewports must be SR-capable and
FG-capable where supported; docked/editor text and CEF UI must remain readable
and stable.

## Lighting Scale

Unlimited lights means artistically unbounded, budget-managed lighting. Do not
implement `number_of_lights * pixels` brute force. Use tiled/clustered light
bins, reservoir sampling, temporal/spatial reuse, emissive candidate promotion,
and virtual shadow demand pages.

## Dynamic Scene Target

Massive procedural dynamic scenes are the main renderer target. Optimize for
streaming, dynamic scene mutation, world scale, destruction, runtime procedural
geometry, high light counts, many moving occluders, editor mode changes, and
p95/p99 stability.

## Machine Contract

The first executable contract is compile-checked in Rust:

- `fun_lux::FUN_LUX_POLICY_DESCRIPTORS`
- `fun_lux::FUN_LUX_CACHE_DESCRIPTORS`
- `fun_renderer::FUN_RENDERER_SUBSYSTEM_DESCRIPTORS`
- `fun_renderer::FUN_RENDERER_BACKEND_DESCRIPTORS`
- `fun_renderer::FUN_RENDERER_AI_INTERFACE_DESCRIPTORS`
- `fun_renderer::FUN_RENDERER_PRODUCT_TOPOLOGY`
- `fun_renderer::FUN_RENDERER_REQUIRES_BEVY_ECS`
- `fun_scene::FUN_SCENE_PRODUCT_TOPOLOGY`
- `fun_scene::FunSceneAuthoringPolicy`
- `fun_scene::FunSceneRendererDeclaration`
- `fun_scene::FunSceneLightingDeclaration`
- `fun_renderer::FunRendererRuntimeBackend`
- `fun_renderer::FUN_RENDERER_UI_RUNTIME_POLICY`
- `fun_renderer::FUN_RENDERER_CEF_RUNTIME_POLICY`
- `fun_renderer::FUN_RENDERER_PRESENTATION_FEATURE_DESCRIPTORS`
- `fun_renderer::FUN_RENDERER_FRAME_GENERATION_CONTRACT`
- `fun_renderer::FUN_RENDERER_LIGHTING_SCALE_POLICY`
- `fun_renderer::FUN_RENDERER_DYNAMIC_SCENE_TARGET`
- `fun_scene::FunSceneSet`
- `fun_renderer::FunRendererEcsSchedulePolicy`
- `fun_renderer::FunRendererSet`
- `fun_renderer::FUN_RENDERER_ECS_PHASE_DESCRIPTORS`
- `fun_renderer::FUN_RENDERER_DATA_PLACEMENT_POLICY`
- `fun_renderer::FUN_RENDERER_CHANGE_DETECTION_RULES`
- `fun_renderer::FunRendererGpuSceneObject`
- `fun_renderer::FunRendererFrameGraphNode`
- `fun_renderer::FunRendererEcsEvent`
- `fun_scene::SceneStableIdentity`
- `fun_scene::SceneRevision`
- `fun_scene::SceneChunkId`
- `fun_scene::Renderable`
- `fun_scene::VirtualGeometryAuthoring`
- `fun_scene::RendererBounds`
- `fun_scene::LuxLight`
- `fun_scene::LuxEmissive`
- `fun_scene::LuxGiParticipant`
- `fun_scene::CefSurface`
- `fun_scene::ViewportUiTarget`
- `fun_scene::UpscalePolicy`
- `fun_scene::ViewportRenderPolicy`
- `fun_lux::FUN_LUX_ECS_SCHEMA_VERSION`
- `fun_lux::LuxLight`
- `fun_lux::LuxLightDatabase`
- `fun_lux::LuxLightEvent`

These tables use stable labels and typed owners so future migration work can be
audited without parsing prose.
