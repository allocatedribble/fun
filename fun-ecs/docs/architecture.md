# fun-ecs Architecture

status: active baseline
schema_version: fun_ecs_architecture_v1
owner: fun

## Design Maxim

ECS owns:

- identity
- world structure
- high-level components
- typed resources
- resource-table facts
- system declarations
- command buffers
- revisions
- artifact manifests
- handoff queues
- cross-domain schedule intent

`fun-scheduler` owns:

- execution
- work admission
- lanes
- deadlines
- budgets
- cancellation
- liveness proofs
- blocking isolation
- graph execution

Subsystems own hot data planes:

- renderer owns GPU scene and frame graph internals
- Avis owns dense physics lanes
- Thunder owns packet, snapshot, and delta buffers
- rvelte owns retained UI internals
- Lux owns lighting caches and policy internals

## First-Class Control Plane

`src/core/control_plane.rs` is the strangler seam around the spatial slice. It
is still re-exported publicly as `control` for compatibility. It introduces:

- `FunEcsWorldId`
- `FunEcsRevision`
- `FunEcsSystemId`
- `FunEcsSystemDeclaration`
- `FunEcsSystemAccess`
- `FunEcsResourceKind`
- `FunEcsResourceChunk`
- `FunEcsControlPlane`
- `FunEcsSubsystemHandoffContract`

The control plane declares systems, extracts resource access, chunks resource
tables by scheduler chunk key, and validates that required subsystem handoff
consumers have typed queues.

## Public API

The first-class product-facing API is documented in
[`public-api.md`](public-api.md). New product-critical paths should use
`fun_ecs::api` for the stable world, storage, system, command, scheduler, and
artifact surface.

Storage experiments remain opt-in through named Cargo features and
`fun_ecs::api::experimental`. They are not allowed to become required by
product-critical server paths.

## World Wrapper

`FunWorld` is the first kernel wrapper. Its default mode is `Hybrid`: RetiredEngine hosts
ordinary compatibility entities, while FUN-owned dense resource tables carry
the hot spatial facts and subsystem handoff queues. `RetiredEngineCompatibility` keeps a
RetiredEngine world available for migration and ergonomics. `FunNative` is the future
native backend and does not host RetiredEngine.

In every mode, `fun-scheduler` remains the scheduler authority. RetiredEngine is a world
compatibility substrate, not the owner of global frame orchestration.

## Identity And Revisions

`src/core/identity.rs` defines the generic ECS ID substrate:

- `FunEntity` plus `FunEntityGeneration`
- `FunComponentId`
- `FunResourceId`
- `FunResourceTableId`
- `FunSystemId`
- `FunSystemSetId`
- `FunCommandBufferId`
- `FunArchetypeId`
- `FunStorageChunkId`
- `FunExternalSlabId`
- `FunArtifactId`
- `FunFrameId`

ID `0` remains invalid for invalid-zero IDs. Entities are generational and
bridge to `fun-scheduler` through an explicit packed `EcsEntityId`.

`src/core/revision.rs` defines the revision substrate:

- `WorldRevisionLedger`
- `ResourceRevisionLedger`
- `ResourceTableRevision`
- `ArtifactRevision`
- `SpatialPageRevision`
- `ExternalSlabRevision`
- `FrameRevision`
- `ScheduleRevision`

Command apply reports carry the observed revision, previous revision, new
revision, touched resources, touched tables, and touched artifact keys. Checked
command-buffer application rejects stale observed revisions before mutating
registries or queues.

## Dense Resource Tables

`src/core/table.rs` generalizes the dense resource behavior already proven by
the spatial slice. `DenseResourceTable<K, Row>` provides stable typed row keys,
dense row storage, optional reverse domain indexing, configurable capacity,
deterministic iteration, per-row/table revisions, chunked extraction, scheduler
access descriptors, and deterministic digests.

The table layer supports `AoS`, `SoA`, and `HybridHotCold` layout declarations.
`TableLayoutAdvisor` chooses an initial layout from the use case and table
stats. The current page residency and artifact tables remain in place until a
later promotion pass has benchmark evidence and migration tests.

## System Descriptors

`src/core/system.rs` is the generic ECS system declaration layer. It defines
`FunSystem`, `FunSystemParam`, `FunSystemDescriptor`, `FunSystemAccess`,
`FunSystemSet`, `FunRunCondition`, `FunSystemExecutionContract`, and
`FunSystemChunkPolicy`.

Params extract component, component chunk, resource, resource table, table
chunk, virtual resource, external artifact, external slab, command-buffer, and
wait-token access rows. Descriptors validate the hard rules before converting
to scheduler-facing `EcsSystemDescriptor` values.

See [system-descriptors.md](system-descriptors.md) for the param and bridge
contract.

## Spatial Schedule Compiler

`src/spatial/schedule.rs` now treats `EcsSpatialScheduleSet` as compiler input,
not only metadata. `EcsSpatialScheduleCompiler::compile()` emits
`WorkGraph<EcsWork<ProductRegistry>>` with scheduler access rows, chunk nodes,
apply/finalization barriers, cross-domain wait-for edges, writer conflict sets,
a liveness proof, and a deterministic graph digest.

This keeps `fun-ecs` as the declarative control plane. The graph is executable
by `fun-scheduler`; subsystem hot data remains in renderer, Lux, Avis, Thunder,
rvelte, and the other owning domains.

## Spatial Slice

The existing spatial code remains the baseline implementation:

- stream sensing, interest, wave planning, and request diffs
- bounded source acquire and decode queues
- dense page residency resources
- derived artifact records
- renderer, Lux, physics, and network handoff rows
- voxel edit propagation
- cross-domain wait tokens
- schedule-set contracts

Future generic ECS kernel work should wrap this slice instead of flattening it
into a universal hot-byte store.
