# fun-ecs Rewrite Plan

status: staged
schema_version: fun_ecs_rewrite_plan_v1
owner: fun

## Strategy

Use a strangler rewrite. Keep the current spatial orchestration code as the
behavioral oracle and add a generic ECS kernel around it.

## Pass 1: Stabilize Spatial Baseline

Done in this frontier:

- document current crate boundary
- preserve doctrine tests
- compile internal command barriers without changing the public 15-set surface
- apply artifact and handoff commands deterministically
- add a first control-plane API for systems, access, resource chunks, revisions,
  and subsystem handoff liveness

## Pass 2: Generic ECS Kernel

Done in this frontier:

- split the internal crate layout into `core`, `runtime`, `spatial`,
  `adapters`, `experiments`, and `diagnostics`
- keep the original public spatial modules visible through `lib.rs`
- add `FunWorld`, `FunWorldId`, `FunWorldRevision`, `FunWorldMode`,
  `FunWorldStorageBackend`, and `FunWorldDiagnostics`
- default `FunWorld` to `Hybrid`
- initialize spatial page table, residency table, dirty ledger, stream
  interests, stream requests, source acquire queue, decoded page queue,
  derived artifact registry, and handoff queues
- add query metadata and command-buffer declarations for the ECS kernel
- lower compiled spatial schedule nodes into a `fun-scheduler` work graph
- record adapter contracts for Bevy, renderer, Lux, Avis, Thunder, and rvelte

Remaining kernel work:

- typed component table declarations beyond the initial metadata layer
- archetype or sparse-set storage decisions by component family
- deterministic replay tests for full world mutation across native storage
- native storage promotion experiments that earn their way out of opt-in gates

## Pass 3: Identity, Revision, And Scheduler Metadata

Done in this frontier:

- added generic ECS identity newtypes and generational entity IDs
- added explicit scheduler bridge traits for entities, chunks, virtual
  resources, and wait tokens
- added world, resource, table, artifact, spatial, external-slab, frame,
  handoff, and schedule revision ledgers
- extended command apply reports with previous/new revisions and touched
  resources, tables, and artifact keys
- added checked artifact, dirty-propagation, and handoff command apply paths
  that reject stale command buffers before mutation
- made scheduler runtime work descriptors carry the observed schedule revision

## Pass 4: Dense Resource Tables

Done in this frontier:

- added `DenseResourceTable<K, Row>`
- added `DenseResourceTableChunk<K, Row>`
- added `DenseResourceTableIndex<K>`
- added `DenseResourceTableRevision`
- added `DenseResourceTableStats`
- added `DenseResourceTableAccess`
- added `DenseResourceTableDigest`
- added selectable `ResourceTableLayout` values: `AoS`, `SoA`, and
  `HybridHotCold`
- added `HotFieldMask`, `ColdPayloadStore`, `ColumnarRowView`, and
  `TableLayoutAdvisor`
- added row metadata adapters for page residency records and derived artifact
  records
- added smoke comparisons against the current dense page/artifact table shapes
- added explicit 1M-row acceptance harnesses for page residency and artifact
  scans

Next table work:

- promote one current spatial table at a time behind compatibility tests
- add real timing/trace capture before claiming performance wins
- split actual SoA columns for hot page/artifact fields only after evidence

## Pass 5: System Descriptors And Access Extraction

Done in this frontier:

- added `FunSystem`, `FunSystemParam`, `FunSystemDescriptor`,
  `FunSystemAccess`, `FunSystemSet`, `FunRunCondition`,
  `FunSystemExecutionContract`, and `FunSystemChunkPolicy`
- added entity query, filter, resource, table, external slab, virtual resource,
  external artifact, command, and event param marker types
- added access extraction for component, component chunk, resource, resource
  table, table chunk, virtual resource, external artifact, external slab,
  command-buffer, and wait-token rows
- added validation for direct world-structure writes, undeclared command
  outputs, unsafe external waits, normal-system waits, and non-send main-thread
  placement
- added conversion from `FunSystemDescriptor` to scheduler-facing
  `EcsSystemDescriptor`
- added descriptors for every current spatial schedule set
- added tests for spatial access rows, command buffer declarations, blocking
  acquire, chunkable decode/artifact builds, and optional idle eviction /
  diagnostics

## Pass 6: Spatial WorkGraph Compiler

Done in this frontier:

- added `EcsSpatialScheduleCompiler`
- added `EcsSpatialScheduleBuildInput` and
  `EcsSpatialScheduleCompileOutput`
- added build report, node spec, access plan, chunk plan, barrier plan, and
  graph digest records
- compile the 15 public spatial sets into `WorkGraph<EcsWork<ProductRegistry>>`
- emit scalar nodes for control stages and publisher stages
- emit chunk nodes for `decode_pages`
- emit one `build_derived_artifacts` chunk per renderer, Lux, Avis, Thunder,
  navigation, audio, telemetry, and editor consumer
- emit request, artifact, dirty-propagation, handoff, and pre-diagnostics
  barrier nodes
- attach command-buffer IDs, scheduler access rows, wait-for edges, and writer
  conflict sets
- prove graph liveness at compile time
- add graph tests for producerless waits, apply cycles, optional present-path
  dependencies, blocking-lane isolation, and table-write conflict sets

Next scheduler work:

- thread cancellation tokens through command-buffer apply and blocking acquire
- split acquire and handoff publication by source or region after trace data
- add executor integration that consumes `EcsSpatialScheduleCompileOutput`

## Pass 7: Subsystem Contracts

Next handoff work:

- renderer artifact manifest contract
- Lux lighting intent contract
- Avis physics cook contract
- Thunder relevance and network row contract
- rvelte UI packet contract
- animation intent contract
- AI intent contract
- Warden evidence contract
- telemetry event contract

Each subsystem keeps its hot data plane. ECS keeps identity, declarations,
resource-table facts, revisions, manifests, handoff queues, and schedule intent.
