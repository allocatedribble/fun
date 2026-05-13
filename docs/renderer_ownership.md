# Renderer Ownership

status: active
owner_repo: fun
scope: fun-renderer, fun-lux, fun-scene, fun-window integration

## Package Map

| package | crate | owner | purpose |
| --- | --- | --- | --- |
| `fun-ecs` | `fun_ecs` | ECS spatial declarations | control-plane resources, entities, events, page tables, dirty ledgers, and handoff queues |
| `fun-scene` | `fun_scene` | scene authoring | scene macros, transforms, hierarchy, visibility, manifests, streaming declarations, and renderer-facing scene data |
| `fun-lux` | `fun_lux` | lighting policy | direct lighting, many-light sampling, virtual shadow policy, GI, reflections, denoising, and radiance/probe caches |
| `fun-renderer` | `fun_renderer` | renderer core | surface integration, frame graph, GPU scene database, resources, pipelines, backend bridges, submit, and present |
| `fun-window` | `fun_window` | window layer | window specs, events, command proxy, raw handle views, redraw requests, and lifecycle facts |

## Product Flow

The product renderer path is FUN-native:

```text
fun-window window + handle views
  -> fun-renderer surface creation
  -> fun-scheduler frame phases
  -> fun-ecs extraction
  -> fun-renderer resources and frame graph
  -> backend bridge records commands
  -> submit and present
```

`fun-window` never owns the graphics device, queue, swapchain, frame graph, or
render passes. `fun-renderer` never owns OS event-loop policy. `fun-ecs` owns
world state; the renderer consumes compact extraction rows and renderer-owned
tables.

## ECS Scene Flow

`fun-scene` is the authoring and scene-entity source of truth:

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

The renderer consumes typed ECS components, resources, events, change ticks, and
manifest rows. It must not mutate opaque gameplay blobs or stash backend
objects on scene entities.

## Render Phases

The frame is split into scheduler-visible phases:

- collect window events
- apply window commands
- dispatch input
- run world systems
- extract render world
- prepare render assets
- build render graph
- record render commands
- submit render commands
- present surface
- flush diagnostics

Every queue on this path is bounded or justified. Resize and redraw churn is
coalesced before renderer reconfiguration work is admitted.

## Resource Ownership

`fun-renderer` owns renderer IDs, allocation policy, resource descriptors,
pipeline schema, GPU-scene tables, transient resources, persistent resources,
upload paths, readback paths, and diagnostics counters.

High-level systems use `GeometryRef`, `MaterialRef`, `LuxLightId`,
`SceneStableIdentity`, renderer asset handles, and descriptor records. They do
not store backend handles or command objects.

## Lighting

`fun-lux` emits backend-neutral `LuxFramePlan`, `LuxPassRequest`,
`LuxResourceIntent`, and quality policy records. `fun-renderer` translates those
records into backend work. This keeps lighting policy testable without a GPU
while still giving the renderer one authoritative source of lighting intent.

## Validation

Run from `fun`:

```text
cargo check -p fun-renderer --features fun_ecs
cargo test -p fun-renderer --features fun_ecs renderer_integration
cargo check -p fun-lux
cargo check -p fun-scene
```

Run from `fun-engine` for the window half:

```text
cargo check -p fun-window --features backend-winit,surface-wgpu
cargo test -p fun-window --features backend-winit
```
