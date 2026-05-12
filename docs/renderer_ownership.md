# Renderer Ownership

status: active
owner_repo: fun
scope: fun-renderer, fun-lux, fun_render, fun-ai, bevy

## Package Map

| package | crate | folder | owner | purpose |
| --- | --- | --- | --- | --- |
| `fun-ecs` | `fun_ecs` | `fun/fun-ecs` | ECS spatial declarations | generic spatial domains, terrain volume declarations, stream cameras/sources, hot SoA page tables, residency maps, dirty ledgers, derived artifact registries, authoring commands, and cross-domain handoff queues |
| `fun-scene` | `fun_scene` | `fun/fun-scene` | scene authoring | FUN-owned `fun!`/`fun_list!` scene macros, deterministic scene manifests, runtime scene spawning, editor scene authoring, server scene authority, streaming declarations, renderer-facing scene components, lighting/GI authoring components |
| `fun-renderer` | `fun_renderer` | `fun/fun-renderer` | renderer core | default renderer core, virtual geometry, virtual shadows, GPU scene database, frame graph, ECS-derived render artifact realization, renderer-owned NATIVE_UI compositor, upscaling/frame-generation orchestration, DX12/Vulkan/Metal backend abstraction, `bevy_ecs` extraction/scheduling/GPU-scene integration |
| `fun-lux` | `fun_lux` | `fun/fun-lux` | lighting | direct lighting, many-light sampling, virtual shadow policy, GI, reflections, denoising/reconstruction policy, radiance/surface/probe caches |
| `fun_render` | `fun_render` | `fun/fun_render` | Bevy/game bridge | extraction, app/plugin integration, feature flags, legacy compatibility, diagnostics, benchmark integration |

`fun-renderer` depends on `fun-ecs`, `fun-scene`, and `fun-lux`. `fun_render`
depends on and re-exports `fun_renderer`, `fun_scene`, and `fun_lux` for
Bevy/game integration. This makes the ECS spatial declarations, Fun Scene, and
Fun Lux part of the renderer product/API while keeping spatial authority, scene
authoring, and lighting code in maintainable crates.

## ECS-First Scene Flow

`fun-scene` is the authoring and scene-entity source of truth. It starts from
the local Bevy fork's confirmed scene resolver and exposes FUN-owned `fun!` and
`fun_list!` macros. The intended flow is:

```text
Bevy scene resolver
  -> fun-ecs spatial domains, page resources, dirty ledgers, handoff queues
  -> fun-scene / fun!
  -> ECS entities, typed components, observers, stable identities
  -> fun_render extraction and RenderApp scheduling
  -> fun-renderer consumes ECS-derived render artifacts and realizes GPU resources
  -> fun-lux consumes ECS/Lux inputs and emits backend-neutral lighting intents
```

The renderer must not pull opaque scene blobs out of gameplay and mutate secret
render objects. It consumes typed ECS components, archetypes, resources, events,
observers, and change ticks.

## ECS-First Spatial Streaming

`fun-ecs` owns generic spatial streaming declarations. The product default
terrain cell is one foot:

```rust
pub const FUN_DEFAULT_TERRAIN_VOXEL_EDGE_UM: u32 = 304_800;
```

Inch-scale voxels are allowed only as bounded fine-detail overlays. They are not
the global world default.

The ECS entity surface is high level: terrain volumes, stream cameras, stream
sources, authoring tools, debug pins, and high-level world objects. Hot per-page
state lives in ECS resources backed by dense tables:

- `EcsSpatialPageTable`
- `EcsPageResidencyMap`
- `EcsDirtyRegionLedger`
- `EcsDerivedArtifactRegistry`
- `EcsStreamRequestQueue`
- `EcsCrossDomainHandoffQueues`

`fun-renderer` consumes `TerrainSurfacePackets` renderer-artifact rows and
realizes GPU resources. `fun-lux` consumes lighting intent rows. Avis or the
active physics runway consumes `PhysicsCollisionProxy` cook rows. Thunder
consumes `NetworkDeltaRows` rows, not renderer page residency.

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
load/unload, editor salience, added virtual geometry, removed renderables, NATIVE_UI
surfaces, viewport policies, and upscale policies. Runtime extraction must use
Bevy change detection such as `Changed<Transform>`, `Changed<Renderable>`,
`Changed<LuxLight>`, `Added<VirtualGeometryAuthoring>`, and
`RemovedComponents<Renderable>`; full scene graph clones are not a product
path.

`FrameGraph` remains a renderer-owned resource, but ECS compiles it from active
components and policies. `NativeUiSurface` adds NATIVE_UI GPU import and UI composite
nodes, DLSS/FSR `UpscalePolicy` adds the corresponding super-resolution node,
hybrid GI mode adds GI passes, and `VirtualGeometryAuthoring` adds virtual
geometry passes.

Renderer resource ownership is now a typed `fun-renderer` contract, not an
implicit side effect of whichever bridge system touched wgpu last.
`RendererResourceClass` divides allocation ownership into upload, transient,
persistent, imported, and readback/debug classes. `RendererResourceKind`
enumerates the first required kinds: staging buffer pages, ring allocations,
transient upload batches, frame-lifetime textures and buffers, pass-local
scratch, material/mesh tables, page pools, shadow page pools, GI/radiance
caches, texture residency pools, NATIVE_UI shared textures, swapchain resources,
vendor SDK resources, diagnostics readback, screenshots, and benchmark
captures.

Pass 10 promotes that policy into `RendererResourceRegistry`, which owns
generation-checked `RendererResourceId` records above the bridge. Public
renderer resource descriptors are `BufferDesc`, `TextureDesc`, `SamplerDesc`,
`ExternalTextureDesc`, `TransientTextureDesc`, `ReadbackBufferDesc`, and
`UploadBufferDesc`. Registry records track descriptor, lifetime class, usage
flags, current graph state, bridge realization handle, memory size, residency,
debug name, last-used frame, and retirement fence. The lifetime classes are
`Persistent`, `FrameLocal`, `GraphTransient`, `Imported`, `ExternalProducer`,
`Readback`, `Upload`, and `DebugOnly`.

`RENDERER_RESOURCE_OWNERSHIP_POLICY` is the migration rule for this boundary.
`fun-renderer` owns allocator policy, `fun_render` may keep compatibility shims,
major renderer passes may not allocate hidden transient resources outside the
allocator, and generic Bevy prepare-stage upload helpers must not be forced
through `FunUploadArena` until a semantic owner and render-schedule insertion
point are known. `ResourceFrameAllocationDiagnostics` is the canonical per-frame
payload for upload bytes/counts, transient bytes, persistent bytes,
imported-resource count, readback bytes, top allocation sites, and high-water
marks. `ResourceMemoryTelemetry` is the Pass 10 in-memory counter surface for
persistent, transient, upload, readback, imported/external, and debug bytes,
high-water marks, created/destroyed resources, fence retirements, live
resources, and bridge realization count.

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
invalidated, upscaler reset, device lost/restored, and NATIVE_UI GPU frame available.
Change detection must route transform changes to motion data, material changes
to material tables, geometry changes to page residency, light changes to
`fun-lux` candidate tables plus shadow invalidation, and scene patches to
manifest signatures.

Scene authoring component names do not repeat the product prefix. The crate path
already supplies context, so the first-party component taxonomy uses names such
as `SceneStableIdentity`, `SceneRevision`, `SceneChunkId`, `Renderable`,
`VirtualGeometryAuthoring`, `RendererBounds`, `LuxLight`, `LuxEmissive`,
`LuxGiParticipant`, `NativeUiSurface`, `ViewportUiTarget`, `UpscalePolicy`, and
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
`HeuristicCauseSet`. NATIVE_UI/Svelte diagnostics can render which ECS inputs caused
page, shadow, light, GI cache, shading-rate, or ML fallback priority without
adding a runtime Bevy UI overlay.

## Editor Scene Operations

The editor authoring path is typed and ECS-first. NATIVE_UI/Svelte edits typed scene
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
remain NATIVE_UI/Svelte. Editor overlays are renderer debug primitives or NATIVE_UI UI
composited late; runtime Bevy UI is not a product/editor panel path.

Observers are for local scene/editor behavior: selection changed, gizmo dragged,
light changed, prefab instantiated, and trigger volume edited. They produce
ordinary ECS component/resource changes consumed by renderer systems. They must
not run renderer hot-path priority, culling, page, lighting, or frame-graph
logic directly.

## Default Renderer

`fun-renderer` is the product default renderer even while the first visual
output is simple.

- `FUN_RENDERER_BACKEND=fun`: initializes the new no-op renderer core through
  `fun_render` and is the long-term default.
- `FUN_RENDERER_BACKEND=auto`: current transition default; resolves to the
  legacy Bevy/wgpu product path with a loud diagnostic.
- `FUN_RENDERER_BACKEND=legacy`: temporary transition path for one migration
  cycle.
- Long-term legacy path: removed.

The future default flip is owned by
`fun_render::bridge::RendererBridgeSettings::from_env`.

## Pass 1/2 Crate And Feature Skeleton

Pass 1 established buildable ownership seams without adding heavyweight render
behavior. Pass 2 makes the ownership-oriented names canonical and keeps the
older Pass 1 names only as transition aliases.

Renderer bridge flags in `fun_render` forward into `fun-renderer`:

- `legacy_renderer`
- `fun_renderer_core`
- `dx12_native_interop`
- `vulkan_backend`
- `metal_backend`
- `native_ui_gpu_only`
- `upscaling`
- `dlss`
- `fsr`
- `frame_generation`
- `experimental_renderer_ml`

Renderer/lux capability flags forward through `fun-renderer` into `fun-lux`
where applicable:

- `many_light`
- `virtual_geometry`
- `virtual_shadows`
- `hybrid_gi`

Compatibility aliases retained for one transition cycle:

- `fun_renderer_legacy` -> `legacy_renderer`
- `fun_renderer_new_core` -> `fun_renderer_core`
- `fun_renderer_dx12` -> `dx12_native_interop` in `fun-renderer`
- `fun_renderer_vulkan` -> `vulkan_backend`
- `fun_renderer_metal` -> `metal_backend`
- `fun_renderer_upscale` -> `upscaling`
- `fun_renderer_dlss` -> `dlss`
- `fun_renderer_fsr` -> `fsr`
- `fun_renderer_frame_generation` -> `frame_generation`
- `fun_renderer_experimental_ml` -> `experimental_renderer_ml`
- `fun_lux_many_light` -> `many_light`
- `fun_lux_virtual_shadows` -> `virtual_shadows`
- `fun_lux_hybrid_gi` -> `hybrid_gi`

The compile-only API seams are:

- `fun_renderer::RendererCoreSettings`
- `fun_renderer::RendererFeatureToggles`
- `fun_renderer::DeviceBackend`
- `fun_renderer::FrameGraphInterface`
- `fun_renderer::ResourceAllocator`
- `fun_renderer::SceneDatabase`
- `fun_renderer::PassRegistry`
- `fun_renderer::Presentation`
- `fun_renderer::NoopRendererCore`
- `fun_renderer::backend::RendererBackend`
- `fun_renderer::backend::BackendDevice`
- `fun_renderer::backend::BackendSurface`
- `fun_renderer::backend::BackendQueue`
- `fun_renderer::backend::BackendResource`
- `fun_renderer::backend::BackendCommandEncoder`
- `fun_renderer::backend::BackendPipeline`
- `fun_renderer::backend::BackendShader`
- `fun_renderer::backend::BackendTiming`
- `fun_renderer::backend::BackendDiagnostics`
- `fun_renderer::backend::BackendNativeInterop`
- `fun_renderer::backend::BackendCapabilityReport`
- `fun_renderer::backend::DynamicRendererBackend`
- `fun_renderer::backend::BufferId<TUsage>`
- `fun_renderer::backend::TextureId<TUsage>`
- `fun_renderer::backend::PipelineId<TPhase>`
- `fun_renderer::FunRendererBackendSelection`
- `fun_renderer::RENDERER_CORE_INTERFACE_MAP`
- `fun_renderer::RENDERER_UPLOAD_ARENA_SEAM`
- `fun_renderer::RENDERER_NATIVE_UI_COMPOSITOR_INTERFACE`
- `fun_renderer::RENDERER_UPSCALE_FRAME_GENERATION_INTERFACE`
- `fun_lux::LuxSettings`
- `fun_lux::LuxFeatureToggles`
- `fun_lux::LightDatabase`
- `fun_lux::LuxRendererHooks`
- `fun_lux::NoopLuxCore`
- `fun_render::RendererBridgeSettings`
- `fun_render::RendererBridgeRuntimeState`
- `fun_render::BridgeFeatureToggles`
- `fun_render::RendererBridgeHooks`
- `fun_render::install_renderer_bridge_api`

`NoopRendererCore` can boot a clear-color frame and submit it through the
compile-only `Presentation` interface. When `FUN_RENDERER_BACKEND=fun` is
selected, `fun_render` starts that no-op core, records backend capabilities,
produces a clear-color frame, calls `fun_lux::NoopLuxCore`, emits diagnostics,
and shuts the no-op path down cleanly. It is not wired to the product swapchain
yet, so the product legacy Bevy renderer remains the only fully visible runtime
presentation path until a later pass connects backend resources and window
presentation.

The V4 backend contract lives in `fun_renderer::backend`. Production renderer
selection is static through `Renderer<B: RendererBackend>` and
`StaticBackendSelection`; tooling may use `DynamicRendererBackend`, but only at
coarse phase boundaries and never as production performance evidence. Backend
reports distinguish bridge type, actual native backend, wgpu adapter truth,
wgpu-core validation, wgpu-hal native-handle support, Naga shader translation,
native command-encoder availability, and feature gaps with named reasons.
High-level renderer data uses generation-checked typed handles such as
`BufferId<TUsage>`, `TextureId<TUsage>`, `PipelineId<TPhase>`,
`BindTableId<TLayout>`, and `GraphResourceId<TKind>` instead of boxed backend
objects or string keys.

Pass 2 adds the Bevy plugin spine in `fun_renderer::plugin`. The public install
surface is `FunRendererPlugin<B>`, with `DefaultFunRendererPlugin` selecting
DX12 on Windows, Metal on macOS, and Vulkan elsewhere through the static
backend selection policy. Tooling can use `DynamicFunRendererPlugin`, but it
marks diagnostics as dynamic dispatch and is not valid production performance
evidence.

The renderer-owned ECS spine exposes public resources only for high-level
systems: `RendererConfig`, `RendererBackendSelection`, `RendererBridgeState`,
`RendererCapabilities`, `RendererFrameIndex`, `RendererFeatureFlags`,
`RendererQualitySettings`, `RendererDiagnostics`, and `RendererFailureState`.
Backend bridge resources such as `WgpuBridgeDevice`, `WgpuBridgeQueue`,
`WgpuBridgeSurface`, `WgpuCoreState`, `WgpuHalAccess`, `NagaShaderBridge`,
`BackendCommandPools`, and `BackendPipelineCache` are private to the plugin
module.

The first phase labels are Bevy `SystemSet`s: `RendererBackendInit`,
`RendererExtract`, `RendererPrepareAssets`, `RendererPrepareScene`,
`RendererVisibility`, `RendererQueue`, `RendererGraphBuild`,
`RendererGraphCompile`, `RendererRecord`, `RendererSubmit`, `RendererPresent`,
`RendererCleanup`, and `RendererDiagnosticsFlush`. They are no-op/counting
phase drivers until graph packets are recorded, but they establish the stable
ECS schedule contract consumed by `fun_render::FunRenderCorePlugin`.

Pass 3 adds the gameplay-facing component and asset surface in
`fun_renderer::component_api`. This API is intentionally above the bridge:
gameplay and scene systems express renderer intent through renderable, camera,
light, UI/NATIVE_UI, post-process, upscaler, material, texture, sampler, and shader
records without naming `wgpu`, `wgpu-core`, `wgpu-hal`, Naga, or backend-native
handles.

The component API carries a machine-workable hot/cold split guide in
`RENDERER_COMPONENT_HOT_COLD_SPLIT_GUIDE`. Hot components are compact, copyable
records or marker components for query specialization. Cold components hold
debug names, render-target selection, UI transport state, post settings, and
quality controls. Per-frame churn should flow through Bevy change detection,
typed dirty bits, or events; steady-state hot paths should not insert/remove
components only to express transient renderer state.

The asset API is described by `RENDERER_ASSET_API`. Material and texture state
uses stable renderer asset IDs, binding schemas, feature masks, and descriptor
records. Tests assert that the public component descriptors cover the Pass 3
contract and that descriptor names do not leak backend handle terminology. Any
future wgpu upgrade or direct DX12/Vulkan/Metal backend must preserve this
public ECS/asset API and adapt below it.

Pass 4 adds `fun_renderer::extraction`, the renderer-owned bridge from Main
World ECS data into compact Render World tables. It consumes the Pass 3 public
components and assets, allocates generation-checked render IDs, tracks Bevy
entity removals, rejects stale object handles, and writes dense backend-neutral
tables for objects, transforms, bounds, materials, mesh instances, lights,
views, UI surfaces, NATIVE_UI surfaces, and post-process volumes.

Extraction is incremental by default. Systems use Bevy changed-component
filters, removed-component readers, asset-event queues, and explicit full
rebuild control state. No `wgpu`, `wgpu-core`, `wgpu-hal`, Naga, native device,
queue, texture, descriptor heap, or command-list object is needed during
extraction. The `RendererExtract` plugin phase installs and runs these systems
before prepare/queue/graph phases.

`RenderWorldExtractionDiagnostics` is a compact in-memory counter record for
queried entities, changed entities, extracted object/material/light/view/UI/NATIVE_UI
records, removals, full rebuild reason, extraction CPU nanoseconds, stale
reference rejections, and Render World memory growth. If those diagnostics are
persisted or sent across a process/backend boundary later, `fun-data`
compressed protobuf bundles remain the canonical telemetry format.

Pass 5 adds `fun_renderer::ir`, the backend-neutral renderer intermediate
representation between Render World data and bridge execution. It defines
resource, binding, pipeline, command, and graph descriptors without `wgpu`,
wgpu-core, wgpu-hal, Naga, native device, queue, texture, descriptor heap, or
command-list handles. Hot draw, dispatch, and indirect packets are fixed-size
copy records with ID/range fields, not heap-backed per-draw objects.

IR translation is descriptor-granular through `RendererIrBridgeTranslator` and
`IrBridgeTranslationCache`: resource creation, pipeline creation, pass
execution, and graph compilation are the only translation steps. Unchanged
descriptors are fingerprinted and skipped, and entity-level extraction data must
be compacted before it becomes IR. Validation rejects missing resources, invalid
formats and sample counts, binding/layout mismatches, graph dependency errors,
unwritten reads, unconsumed writes, and unsupported backend features. Explicit
shader, pipeline, material binding, and graph schema versions make wgpu/Naga or
direct-backend changes visible instead of silent.

Pass 6 adds the deliberate `fun_renderer::bridge::wgpu` module. It is feature
gated behind `wgpu_bridge`, enabled for default production renderer builds, and
kept out of `--no-default-features` so the high-level backend and IR contracts
remain buildable without a wgpu dependency. The module is split into `device`,
`surface`, `resource`, `pipeline`, `binding`, `command`, `diagnostics`, `hal`,
`core`, and `naga` submodules.

The bridge owns direct `wgpu` descriptor translation: resource descriptors map
to wgpu buffer/texture/sampler descriptors, binding layouts map to wgpu bind
group layout entries, shader/pipeline helpers build wgpu shader and pipeline
descriptors once real handles are supplied, and graph pass descriptors compile
to render/compute/copy pass descriptor records. `WgpuDescriptorCache` keys
translated descriptors by stable IR fingerprints and reports a named
`RuntimeCreationAfterWarmup` failure when measured builds try to create or
recreate cached bridge objects at runtime. `WgpuPassExecutionArrays` compacts
compiled graph plans into dense render, compute, and copy batches so future
encoder recording can operate per pass, not per entity or per draw dispatch
through a backend trait object.

The bridge also owns health truth through `WgpuBridgeHealthReport`: selected
wgpu backend, actual native backend, adapter summary, validation status,
wgpu-core bridge status/compatibility, features/limits summary, HAL access,
native command-encoder availability, Naga translation path, and pipeline-cache
status. This is bridge health only; ECS components and high-level renderer IR
still do not name wgpu handles.

Pass 7 makes `fun_renderer::bridge::wgpu::core` the only wgpu-core boundary.
`WgpuCoreBridge<B>` captures actual adapter backend truth, redacted adapter
identity, adapter/device feature and limit state, validation/device-lost hook
availability, redacted validation and device-lost event summaries, internal
counter summaries, allocator byte/count summaries, and the wgpu stack upgrade
compatibility facts. It uses wgpu's public wgpu-core-backed APIs today and does
not reach into private `wgpu_core::Global`, hub, registry, identity-manager, or
resource-tracker internals. Any future private core access must stay in that
module, be feature-gated, update `WgpuCoreCompatibilityReport`, and follow the
root `docs/renderer/v4_wgpu_upgrade_contract.md` checklist.

Pass 8 makes `fun_renderer::bridge::wgpu::hal` the native interop boundary.
`NativeInteropCapabilities` reports whether native device, queue, command
encoder, DX12 command list, texture handle, external texture import, fence
interop, and native debug marker access are actually available.
`BackendNativeInteropStatus` and `WgpuBridgeHealthReport` carry those bits as
diagnostic truth; public ECS components, extraction tables, and renderer IR
still do not carry native handles.

Native handles are exposed only through scoped callbacks implemented by
`HalInteropBridge` for `Dx12HalInterop`, `VulkanHalInterop`, and
`MetalHalInterop`. The sanctioned entry points are
`with_native_command_encoder` and `with_dx12_command_list`; both require an
active bridge recording scope and return structured `HalInteropFailure` values
instead of falling back silently. The current wgpu DX12 bridge deliberately
reports `NativeCommandListUnavailable` because wgpu-hal 29 does not expose a
sanctioned `ID3D12GraphicsCommandList` path.

Native interop is policy-gated for NATIVE_UI D3D11On12 copies,
DLSS/Streamline/NGX, vendor SDK hooks, PIX/native markers, and external texture
import/export. It is rejected for arbitrary gameplay systems, frame-graph state
bypass, hidden command submission, and graph-resource writes that were not
declared to the graph.

Pass 9 adds the renderer-owned shader schema in `fun_renderer::shader`.
`RendererShaderSchema` is independent of WGSL, HLSL, SPIR-V, MSL, Naga IR, and
native compiler handles: it records stage, entry point, pipeline layout, vertex
inputs, fragment outputs, compute workgroup shape, feature defines, material
schema, source strategy, and debug name as compact renderer data. Reflection
uses fixed-capacity records for binding slots, entry points, root constants,
vertex attributes, render target outputs, depth usage, storage/UAV usage, and
workgroup sizes.

`fun_renderer::bridge::wgpu::naga::NagaShaderBridge` owns WGSL parsing,
validation, reflection, redacted diagnostics, and shader translation cache
decisions below the wgpu bridge. HLSL/DXC for DX12, SPIR-V for Vulkan, Naga IR
passthrough, and MSL for Metal are represented as source strategies behind the
same renderer schema; high-level renderer code does not branch on compiler
source format. Warmup may translate and cache shader metadata, but measured
runtime frames reject cold translation and can only reuse warmed cache entries.

Pass 10 keeps wgpu realization below the bridge. High-level renderer systems
store only `RendererResourceId` and backend-neutral descriptor records. The wgpu
bridge maps those IDs through `WgpuBridgeResourceRealizationMap` to bridge-owned
object slots and handle kinds; actual `wgpu::Buffer`, `wgpu::Texture`, and
`wgpu::Sampler` storage remains private to the bridge implementation. Direct
DX12, Vulkan, and Metal backends can replace that map with native object tables
without changing renderer resource identity.

Pass 11 adds `fun_renderer::binding`, the stable binding and descriptor model
above wgpu bind groups, DX12 descriptor heaps, Vulkan descriptor sets, and Metal
argument buffers. Public binding records are `BindLayout`, `BindTable`,
`BindlessTable`, `ViewBindings`, `SceneBindings`, `MaterialBindings`,
`PassBindings`, and `RootConstants`. They use compact IR IDs,
generation-checked `RendererResourceId` records, stable `MaterialId` values,
stable texture IDs, layout/table/resource hashes, and prepared binding handles;
they do not expose backend descriptor objects to ECS or renderer IR consumers.

`BindingTranslationCache` is the Pass 11 warmup-only creation gate. Measured
frames can reuse unchanged layouts/tables or dirty-update an existing table
when bound renderer resource generations change, but new layout/table/bindless
creation after warmup returns `RuntimeCreationAfterWarmup`. The production
`BindingChurnPolicy` forbids per-entity descriptor allocation and per-draw bind
group creation, and its telemetry records descriptor churn counters as compact
in-memory state. If those counters become retained artifacts, the canonical
format remains a `fun-data` compressed protobuf bundle.

The wgpu bridge consumes the same high-level binding records through
`WgpuBindingBridgeCache`: layouts plan `wgpu::BindGroupLayout`, tables plan
`wgpu::BindGroup`, bindless tables require binding-array capability, and root
constants use push constants when supported or a uniform fallback with an
explicit `RootConstantFallbackCost`. Direct DX12, Vulkan, and Metal mappings are
recorded beside the same binding model rather than hidden in gameplay or
material systems.

Pass 12 adds the renderer-owned pipeline/PSO abstraction. `fun_renderer::pipeline`
keeps `PipelineFamily`, `PipelineVariantKey`, `RenderPipelineDesc`,
`ComputePipelineDesc`, and `PipelineLayoutDesc` as the high-level IR surface and
adds `ShaderEntrySet`, `RenderTargetState`, `DepthState`, `BlendState`,
`RasterState`, `VertexLayout`, `PipelineCacheKey`, and `PreparedPipelineId`.
The cache key includes shader hashes, reflection signature, bind layout hash,
render target/depth/MSAA state, blend/depth/raster state, topology, quality
tier, feature mask, and backend target.

`PipelineCache` permits shader module, pipeline layout, render pipeline, and
compute pipeline creation only during warmup or asset load. Measured frames can
reuse prepared handles but cannot create missing variants; missing prepared
variants resolve to a named debug-material fallback instead of rendering black.
`PipelineWarmupManifest::PASS12` covers depth, opaque, alpha test,
transparent, shadow, sky, UI, post, debug views, and compute before measured
frames.

The wgpu bridge consumes that model through `WgpuPipelineBridgeCache`, which
fingerprints shader modules/layouts and maps render/compute descriptors into
renderer-owned PSO keys. Live wgpu PSO realization is still a future bridge
step, but pipeline churn policy and prepared IDs are now owned above wgpu.

## Ownership Rules

- `fun-renderer` owns renderer-side feature interfaces, tensor input/output
  schemas, GPU resource handles, fallback heuristic paths, renderer-owned NATIVE_UI
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

## NATIVE_UI Boundary

`fun_ui_native_ui` owns browser lifetime, page loading, JavaScript bridge messages,
offscreen paint callbacks, and browser-facing command envelopes.
`fun-renderer` owns the renderer-side compositor contract for NATIVE_UI pixels and GPU
resources. NATIVE_UI UI remains a HUD/UI layer after world rendering, lighting,
upscaling/reconstruction, and post-processing; it must not feed temporal
reconstruction inputs or Ray Reconstruction guide buffers.

Product NATIVE_UI UI is GPU-only. Accelerated shared-texture transport is required.
If GPU transport fails, the UI subsystem fails with a clear diagnostic. Runtime
lanes must not silently fall back to CPU `OnPaint` uploads. Temporary CPU
golden-image fixtures may exist only when clearly test-only and not compiled
into runtime/product lanes.

The Pass 8 runtime seam is `fun_renderer::RendererNativeUiCompositor`. NATIVE_UI callback
handles are copied/imported immediately into the DX12 ring owned by the NATIVE_UI
transport bridge; renderer-facing records contain only stable frame IDs, owned
texture IDs, extents, alpha mode, dirty-rect metadata, copied bytes, and timing
facts. `game_client` converts `Dx12NativeUiReadyFrameToken` into
`RendererNativeUiImportedFrame` and never stores callback-lifetime handles.

The frame graph imports this layer through `FrameGraphPassRole::NativeUiGpuImport`
and `FrameGraphResourceType::UiColorAlpha`, then composes UI after scene
rendering/upscaling. Product UI policy is executable through
`fun-quality check-code-shape`.

## UI Boundary

NATIVE_UI/Svelte is the only product UI surface. Runtime/product lanes must not use
Bevy UI for launcher UI, editor UI, game HUD, diagnostics panels, or debug
overlays. Bevy UI may exist only in temporary test-only code that is clearly not
compiled into runtime/product lanes.

## Presentation Boundary

DLSS, FSR, and frame generation are renderer features. `fun-renderer` owns the
presentation/upscale boundary. Scene color and UI color stay separate. Frame
generation receives HUD-less scene color, UI color, depth, motion vectors, and
valid present-time resource lifetimes. Editor viewports must be SR-capable and
FG-capable where supported; docked/editor text and NATIVE_UI UI must remain readable
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
- `fun_renderer::FUN_RENDERER_NATIVE_UI_RUNTIME_POLICY`
- `fun_renderer::RendererNativeUiCompositor`
- `fun_renderer::RendererNativeUiCompositorDiagnostics`
- `fun_renderer::RendererNativeUiImportedFrame`
- `fun_renderer::RendererNativeUiLayer`
- `fun_renderer::FUN_RENDERER_PRESENTATION_FEATURE_DESCRIPTORS`
- `fun_renderer::FUN_RENDERER_FRAME_GENERATION_CONTRACT`
- `fun_renderer::FUN_RENDERER_LIGHTING_SCALE_POLICY`
- `fun_renderer::FUN_RENDERER_DYNAMIC_SCENE_TARGET`
- `fun_renderer::RendererCoreSettings`
- `fun_renderer::RendererFeatureToggles`
- `fun_renderer::NoopRendererCore`
- `fun_renderer::RendererResourceClass`
- `fun_renderer::RendererResourceKind`
- `fun_renderer::RendererResourceOwner`
- `fun_renderer::ResourceOwnershipPhase`
- `fun_renderer::ResourceMigrationPriority`
- `fun_renderer::RENDERER_RESOURCE_OWNERSHIP_POLICY`
- `fun_renderer::RENDERER_RESOURCE_CLASS_DESCRIPTORS`
- `fun_renderer::ResourceFrameAllocationDiagnostics`
- `fun_renderer::HOT_UPLOAD_KILL_LIST`
- `fun_renderer::RendererFrameGraph`
- `fun_renderer::RendererFrameDescription`
- `fun_renderer::FrameGraphPassType`
- `fun_renderer::FrameGraphResourceType`
- `fun_renderer::FrameGraphDiagnosticCategory`
- `fun_renderer::RendererFrameGraphDiagnostics`
- `fun_renderer::RendererFrameGraphDebugArtifact`
- `fun_renderer::FrameGraphSubmission`
- `fun_renderer::GpuSceneDatabase`
- `fun_renderer::GpuViewRecord`
- `fun_renderer::GpuInstanceRecord`
- `fun_renderer::GpuMeshRecord`
- `fun_renderer::GpuMaterialRecord`
- `fun_renderer::GpuLightRecord`
- `fun_renderer::GpuPageMetadataRecord`
- `fun_renderer::GpuSceneDirtyRange`
- `fun_renderer::GpuSceneDatabaseDiagnostics`
- `fun_render::FunRenderSceneExtractionBridge`
- `fun_render::FunRenderSceneExtractionReport`
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
- `fun_scene::NativeUiSurface`
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
- `fun_lux::LuxSettings`
- `fun_lux::LuxFeatureToggles`
- `fun_lux::NoopLuxCore`
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
