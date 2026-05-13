# fun-ecs Frontier

status: spatial orchestration baseline
schema_version: fun_ecs_frontier_v1
owner: fun

## Current Boundary

`fun-ecs` is the first-party ECS declaration and orchestration crate for spatial
streaming plus the emerging project-FUN ECS control plane. It is not a hot
data-plane owner for renderer, Lux, Avis, Thunder, rvelte, animation, AI,
Warden, or telemetry internals.

The public crate keeps the original spatial module surface and now also exposes
the ECS kernel layer:

- `adapters`
- `artifact`
- `authority`
- `core`
- `control`
- `diagnostics`
- `dirty`
- `experiments`
- `frame_graph`
- `load_animation`
- `page_table`
- `residency`
- `runtime`
- `schedule`
- `source`
- `spatial`
- `storage`
- `streaming`
- `voxel`

The current crate exposes scheduler-facing types from `fun_scheduler_types`,
including:

- `EcsChunkKey`
- `EcsEntityId`
- `EcsSpatialDomainKind`
- `EcsSpatialWaitTokenKind`
- `EcsSystemClass`
- `EcsSystemExecutionContract`
- `EcsVirtualResourceKey`
- `EcsWorkKind`
- `ScheduleDeadline`
- `ScheduleDomain`
- `ScheduleLane`
- `ProductRegistry`
- `WorkGraph`
- `WorkGraphId`
- `LivenessProof`
- `WorkWaitToken`

The spatial schedule now also exposes `EcsSpatialScheduleCompiler` and its
build input, report, node spec, access plan, chunk plan, barrier plan, graph
digest, and `EcsSpatialProductWorkGraph` output aliases.

`fun-ecs` forbids unsafe code with `#![forbid(unsafe_code)]`.

## Product-Scale Caps

The crate currently encodes bounded product-scale tables:

- `ECS_SPATIAL_MAX_PAGE_RECORDS`
- `ECS_SPATIAL_MAX_STREAM_INTERESTS`
- `ECS_SPATIAL_MAX_STREAM_REQUESTS`
- `ECS_SPATIAL_MAX_SOURCE_QUEUE_ROWS`
- `ECS_SPATIAL_MAX_DECODED_PAGE_ROWS`
- `ECS_SPATIAL_MAX_DERIVED_ARTIFACTS`
- `ECS_SPATIAL_MAX_HANDOFF_ROWS`
- `ECS_SPATIAL_MAX_LOAD_ANIMATION_ARTIFACTS`

## Decision

Keep this crate. Treat the current code as the spatial orchestration vertical
slice and grow a first-class ECS control plane around it. Do not replace it with
a blank-slate ECS. Preserve current tests as doctrine gates.

## Internal Layout

The Pass 2 layout separates compatibility surface from ownership boundaries:

- `src/lib.rs` is the public umbrella and stable re-export surface.
- `src/core/` owns world identity, control-plane facts, resource sets, query
  metadata, system descriptors, access extraction, revision ledgers, dense
  resource-table primitives, and command-buffer declarations.
- `src/runtime/` lowers ECS schedule intent into `fun-scheduler` work graphs.
- `src/spatial/` contains the current spatial vertical slice.
- `src/adapters/` records RetiredEngine and subsystem interop contracts.
- `src/experiments/` keeps opt-in storage and execution experiments out of the
  default path.
- `src/diagnostics/` exposes compact debug snapshots for graph and control
  plane state.

See [resource-tables.md](resource-tables.md) for the dense-table contract and
1M-row acceptance harness. See [system-descriptors.md](system-descriptors.md)
for the system descriptor and scheduler bridge contract.
