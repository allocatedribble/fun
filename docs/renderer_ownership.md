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

## Scene Authority And Streaming

`fun-scene` owns the generic scene authority model. `game_scene` remains a
game-specific catalog for default arenas, benchmark scenes, stress scenes, and
editor starter scenes during the transition.

The shared manifest and stream contract lives in `fun_scene`:

- `SceneManifest`
- `SceneEntityManifest`
- `SceneNetworkManifest`
- `SceneDescriptor`
- `SceneManifestSignature`
- `SceneRendererManifest`
- `SceneLuxManifest`
- `SceneStreamChunk`
- `SceneManifestProvider`
- `NetworkedSceneEntity`
- `SceneStableHistoryKey`
- `SceneStableEntityIndex`
- `chunk_world_specs`
- `chunk_world_specs_with_signature`
- `try_chunk_world_specs`
- `try_chunk_world_specs_with_signature`
- `scene_manifest_signature`
- `world_stream_manifest_signature`
- `qtransform`

The pipeline is:

```text
fun! scene
  -> resolved ECS entities
  -> stable identities
  -> SceneManifest
  -> SceneStreamChunk list
  -> server/client/editor consume same chunks
  -> renderer extracts chunk additions/removals
  -> virtual page service receives chunk priorities
```

`SceneManifest::shared_consumer_contract` is the compact machine-checkable
record for this path. The default game scene now builds its manifest from
`fun_scene::scene_manifest_signature` and carries that full-scene signature on
its streamed chunks with `fun_scene::chunk_world_specs_with_signature`;
`game_server` uses `fun_scene` directly for live stream signatures, chunking,
stable network identities, and quantized transforms.

Network-aware scene data is authored with `NetworkedSceneEntity`, not an opaque
runtime Bevy `Entity`. It carries the replication class, authority mode, scope,
and priority that eventually become Thunder `NetworkIdentity`,
`NetworkAuthority`, `ReplicationScope`, and `ReplicationPriority` components.
The server is the authoritative scene spawner, stable identity assigner, world
stream manifest generator, and gameplay authority. The client consumes streamed
chunks, spawns the renderable subset, and applies prediction where needed. The
editor edits the full scene graph, previews through the same renderer path, and
can generate deterministic test manifests.

`SceneStableIdentity` remains separate from runtime Bevy `Entity`.
`SceneStableHistoryKey` and `SceneStableEntityIndex` are the renderer-facing
bridge for GPU history, motion vectors, and cache invalidation. Scene patches
may change manifest signatures, networking policy, transforms, materials, or
lighting, but they must not silently reassign stable IDs.

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
`FunRendererSet` orders the broad render lane as `Extract`, `PrepareScene`,
`PrepareResources`, `Visibility`, `VirtualGeometry`, `VirtualShadows`, `Lux`,
`Upscale`, `UiComposite`, and `Present`. Section-specific render-world systems
use unprefixed names: `RendererExtractSet`, `RendererPrepareSet`,
`RendererVisibilitySet`, and `RendererFrameSet`. `fun_render` registers the
first renderer/lux resources and message queues in both the app world and
RenderApp when the bridge is installed.

The GPU scene database is the render-world ECS resource `GpuScene`. It owns
compact renderer tables instead of an opaque scene clone:

- `GpuInstanceTable`
- `GpuTransformTable`
- `GpuMaterialTable`
- `GpuGeometryTable`
- `GpuGeometryPageTable`
- `GpuLightTable`
- `GpuShadowPageTable`
- `GpuPageTable`
- `GpuSceneRevisionTable`

ECS components stay ergonomic and typed; GPU storage stays compact and
SoA-oriented. The executable mapping is `COMPONENT_GPU_MAPPINGS`: `Transform`
feeds the transform table, `Renderable` feeds instance/material/geometry-page
tables, `VirtualGeometryAuthoring` feeds geometry-page and shadow-page tables,
`LuxLight` feeds light and shadow-page tables, and `SceneStableIdentity` is the
cross-table history key. Entities carry compact references such as
`GeometryRef`, `MaterialRef`, `LuxLightId`, `SceneStableIdentity`, and
`RendererInstanceId`. Heavy material data, GPU buffers, residency tables,
shadow page state, reservoir state, cache state, and backend objects live in
resources/assets/tables, not on scene entities.

Long-lived renderer history is keyed by stable scene identity, not transient
Bevy `Entity`. `StableHistoryTable` records motion-vector, virtual-geometry
page, shadow-page, GI-cache, and light-reservoir history under
`StableHistoryId`. A respawn with the same stable identity can recover or reset
history according to `HistoryRespawnPolicy`; table compaction for removed
instances must not invalidate unrelated stable histories.

Extraction copies only compact deltas into `ExtractedSceneDeltas`: changed
transforms, visibility flags, geometry refs, material refs, light refs, chunk
load/unload, editor salience, added virtual geometry, removed renderables, CEF
surfaces, viewport policies, and upscale policies. Runtime extraction must use
Bevy change detection such as `Changed<Transform>`, `Changed<Renderable>`,
`Changed<LuxLight>`, `Added<VirtualGeometryAuthoring>`, and
`RemovedComponents<Renderable>`; full scene graph clones are not a product
path.

`FrameGraph` remains a renderer-owned resource, but ECS compiles it from active
components and policies. `CefSurface` adds CEF GPU import and UI composite
nodes, DLSS/FSR `UpscalePolicy` adds the corresponding super-resolution node,
hybrid GI mode adds GI passes, and `VirtualGeometryAuthoring` adds virtual
geometry passes.

`fun-lux` is also ECS-driven. Scene-authored `LuxLight` components become
compact GPU `LuxLight` records, `LuxEmissive` components become emissive
candidate records, `LuxGiParticipant` components drive GI cache participation,
and `VirtualShadowCaster` / `VirtualShadowReceiver` drive virtual shadow page
demand. The renderer-facing lighting state is the `LuxWorld` resource, which
groups `LuxLightDatabase`, `LuxClusterGrid`, `LuxReservoirStorage`,
`LuxShadowRequestQueue`, `LuxGiCache`, and `LuxDiagnostics`.

Lighting work is split into `LuxExtractSet`, `LuxPrepareSet`, and
`LuxRenderSet`. The prepare lane updates the light database, light clusters,
emissive candidates, shadow invalidation, and GI cache requests. The render lane
runs direct lighting, temporal and spatial reservoir reuse, virtual shadow
filtering, GI trace/cache update, reflection trace, and denoising.

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

## Virtual Geometry And Shadows

Virtual geometry is a typed ECS authoring component, not a path convention.
Scenes author it with `fun!` using `Renderable`, `VirtualGeometryAuthoring`,
`RendererBounds`, and related lighting/shadow components. `GeometryRef` and
`MaterialRef` are compact handles; large material data, buffers, bake metadata,
and page residency stay in resources and renderer tables.

Scene-authored virtual geometry uses:

- `VirtualGeometryMode::StaticClusterPages`
- `PagePriorityHint::WorldCritical`
- `DynamicGeometryPolicy::StaticOnly`
- `RenderableFlags::STATIC_WORLD`

`Added<VirtualGeometryAuthoring>` schedules meshlet/cluster bake checks,
requests geometry page metadata, registers bounds, and raises the page priority
record in `GpuScene`. `Changed<Transform>` updates instance transforms, motion
vectors, and shadow invalidation. `RemovedComponents<Renderable>` frees the
instance record, releases page references, and invalidates shadow/GI caches.

Virtual shadows are also typed scene data:

- `VirtualShadowCaster { policy, invalidation }`
- `VirtualShadowReceiver { priority, filter_policy }`
- `ShadowCasterPolicy::VirtualPages`
- `ShadowInvalidationPolicy::OnTransformOrGeometryChange`
- `ShadowReceiverPriority::High`
- `ShadowFilterPolicy::ContactAware`

`fun-lux` consumes those components to update shadow request counts and refresh
priorities. Light changes invalidate light-specific pages; receiver salience can
raise or lower page refresh priority through the existing salience path.

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

## Heuristic Scheduler

The renderer heuristic scheduler is an ECS system graph, not a black-box
renderer loop. It consumes authored and extracted components directly:
`Transform`, `GlobalTransform`, `Visibility`, `ViewVisibility`, `Renderable`,
`VirtualGeometryAuthoring`, `LuxLight`, `LuxEmissive`,
`VirtualShadowReceiver`, `GameplaySalient`, `EditorSelection`, `SceneChunkId`,
`StreamingPriority`, and `TemporalInstability`.

`RenderHeuristicScheduler` is the resource that carries the active
`RenderBudgetMode`, `RenderHeuristicWeights`, and `RenderFrameBudget`.
`RendererViews` supplies viewport context. The scheduler publishes compact
priority components and resources:

- `PagePriority`
- `ShadowPagePriority`
- `LuxLightPriority`
- `GiCacheUpdatePriority`
- `ShadingRatePriority`
- `MlInferencePriority`
- `PagePriorities`
- `ShadowPagePriorities`
- `LuxLightPriorities`
- `GiCacheUpdatePriorities`
- `ShadingRatePriorities`
- `MlInferencePriorities`

The first system graph is `begin_heuristic_frame`,
`score_virtual_geometry_pages`, `score_shadow_receivers`, `score_lux_lights`,
and `score_gi_cache_participants`. These systems are intentionally explicit
about their ECS query tuples because the tuple is the scheduler contract:
screen area, visibility, view visibility, renderability, virtual-geometry page
hints, light importance, emissive importance, receiver risk, gameplay salience,
editor selection, streaming priority, scene chunk membership, and temporal
instability all remain visible to Bevy scheduling and tests.

`HeuristicDebugOverlay` stores `HeuristicPriorityRecord` values with a
`HeuristicCauseSet`. CEF/Svelte diagnostics can render which ECS inputs caused
page, shadow, light, GI cache, shading-rate, or ML fallback priority without
adding a runtime Bevy UI overlay.

## Editor Scene Operations

The editor authoring path is typed and ECS-first. CEF/Svelte edits typed scene
data and sends `scene.operation.apply` to the Rust host. The host validates a
`fun_scene::EditorOperationEnvelope`, queues accepted operations into
`EditorOperationQueue`, and the `fun-scene` operation systems apply supported
operations to Bevy ECS through normal component/resource mutation.

The first operation contract is:

- `spawn_scene_component`
- `patch_component`
- `delete_entity`
- `duplicate_entity`
- `attach_child_scene`
- `adjust_light`
- `adjust_render_policy`
- `mark_selection_salience`

Component names are not product-prefixed. The canonical editor component names
include `LuxLight`, `Renderable`, `ViewportRenderPolicy`, `GameplaySalient`,
`EditorSelection`, `StreamingPriority`, and `TemporalInstability`.

Canonical patch example:

```json
{
  "op": "patch_component",
  "entity": { "uri": "scene://arena-blockout/Floor" },
  "component": "LuxLight",
  "field": "intensity_lux",
  "value": 65000.0
}
```

`.fun` assets are declarative. Asset files may reference typed values and scene
assets, but dynamic Rust expressions remain macro-only. Editor viewport panels
remain CEF/Svelte. Editor overlays are renderer debug primitives or CEF UI
composited late; runtime Bevy UI is not a product/editor panel path.

Observers are for local scene/editor behavior: selection changed, gizmo dragged,
light changed, prefab instantiated, and trigger volume edited. They produce
ordinary ECS component/resource changes consumed by renderer systems. They must
not run renderer hot-path priority, culling, page, lighting, or frame-graph
logic directly.

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
- `fun_renderer::RendererExtractSet`
- `fun_renderer::RendererPrepareSet`
- `fun_renderer::RendererVisibilitySet`
- `fun_renderer::RendererFrameSet`
- `fun_renderer::FUN_RENDERER_ECS_PHASE_DESCRIPTORS`
- `fun_renderer::FUN_RENDERER_DATA_PLACEMENT_POLICY`
- `fun_renderer::FUN_RENDERER_CHANGE_DETECTION_RULES`
- `fun_renderer::GpuScene`
- `fun_renderer::GpuTransformTable`
- `fun_renderer::GpuGeometryPageTable`
- `fun_renderer::GpuShadowPageTable`
- `fun_renderer::RendererInstanceId`
- `fun_renderer::ComponentGpuMapping`
- `fun_renderer::COMPONENT_GPU_MAPPINGS`
- `fun_renderer::GPU_DATA_LAYOUT_POLICY`
- `fun_renderer::StableHistoryTable`
- `fun_renderer::StableHistoryId`
- `fun_renderer::StableHistoryPolicy`
- `fun_renderer::STABLE_HISTORY_POLICY`
- `fun_renderer::ExtractedSceneDeltas`
- `fun_renderer::FrameGraph`
- `fun_renderer::FRAME_GRAPH_COMPONENT_RULES`
- `fun_renderer::FunRendererGpuSceneObject`
- `fun_renderer::FunRendererFrameGraphNode`
- `fun_renderer::FunRendererEcsEvent`
- `fun_lux::LuxExtractSet`
- `fun_lux::LuxPrepareSet`
- `fun_lux::LuxRenderSet`
- `fun_lux::LuxWorld`
- `fun_lux::LuxClusterGrid`
- `fun_lux::LuxReservoirStorage`
- `fun_lux::LuxShadowRequestQueue`
- `fun_lux::LuxGiCache`
- `fun_lux::LuxDiagnostics`
- `fun_scene::SceneStableIdentity`
- `fun_scene::SceneRevision`
- `fun_scene::SceneChunkId`
- `fun_scene::SceneManifest`
- `fun_scene::SceneEntityManifest`
- `fun_scene::SceneNetworkManifest`
- `fun_scene::SceneStreamChunk`
- `fun_scene::SceneRendererManifest`
- `fun_scene::SceneLuxManifest`
- `fun_scene::NetworkedSceneEntity`
- `fun_scene::SceneStableHistoryKey`
- `fun_scene::SceneStableEntityIndex`
- `fun_scene::SceneNetworkingPolicy`
- `fun_scene::SceneNetworkRole`
- `fun_scene::SCENE_NETWORK_ROLE_DESCRIPTORS`
- `fun_scene::scene_manifest_signature`
- `fun_scene::chunk_world_specs_with_signature`
- `fun_scene::try_chunk_world_specs_with_signature`
- `fun_scene::Renderable`
- `fun_scene::VirtualGeometryAuthoring`
- `fun_scene::RendererBounds`
- `fun_scene::LuxLight`
- `fun_scene::LuxEmissive`
- `fun_scene::LuxGiParticipant`
- `fun_scene::VirtualShadowCaster`
- `fun_scene::VirtualShadowReceiver`
- `fun_scene::ShadowCasterPolicy`
- `fun_scene::ShadowInvalidationPolicy`
- `fun_scene::ShadowReceiverPriority`
- `fun_scene::ShadowFilterPolicy`
- `fun_scene::CefSurface`
- `fun_scene::ViewportUiTarget`
- `fun_scene::UpscalePolicy`
- `fun_scene::ViewportRenderPolicy`
- `fun_scene::GameplaySalient`
- `fun_scene::EditorSelection`
- `fun_scene::StreamingPriority`
- `fun_scene::TemporalInstability`
- `fun_scene::EditorIntegrationPolicy`
- `fun_scene::EditorOperationKind`
- `fun_scene::EditorOperation`
- `fun_scene::EditorOperationEnvelope`
- `fun_scene::EditorOperationQueue`
- `fun_scene::EditorSceneEntityIndex`
- `fun_scene::EditorOperationOutcomes`
- `fun_scene::EditorOverlaySurface`
- `fun_scene::EditorObserverTriggerKind`
- `fun_scene::EDITOR_OPERATION_DESCRIPTORS`
- `fun_scene::EDITOR_OBSERVER_DESCRIPTORS`
- `fun_lux::FUN_LUX_ECS_SCHEMA_VERSION`
- `fun_lux::LuxLight`
- `fun_lux::LuxLightDatabase`
- `fun_lux::LuxLightEvent`
- `fun_renderer::RenderHeuristicSet`
- `fun_renderer::RenderHeuristicScheduler`
- `fun_renderer::RenderHeuristicWeights`
- `fun_renderer::RenderFrameBudget`
- `fun_renderer::RendererViews`
- `fun_renderer::PagePriority`
- `fun_renderer::ShadowPagePriority`
- `fun_renderer::LuxLightPriority`
- `fun_renderer::GiCacheUpdatePriority`
- `fun_renderer::ShadingRatePriority`
- `fun_renderer::MlInferencePriority`
- `fun_renderer::PagePriorities`
- `fun_renderer::ShadowPagePriorities`
- `fun_renderer::LuxLightPriorities`
- `fun_renderer::GiCacheUpdatePriorities`
- `fun_renderer::ShadingRatePriorities`
- `fun_renderer::MlInferencePriorities`
- `fun_renderer::HeuristicDebugOverlay`
- `fun_renderer::HeuristicCauseSet`

These tables use stable labels and typed owners so future migration work can be
audited without parsing prose.
