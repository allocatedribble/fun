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

Next scheduler work:

- attach budgets, deadlines, lanes, split hints, and blocking policy
- validate graph liveness before execution
- prove cancellation and barrier ordering with tests

## Pass 5: Subsystem Contracts

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
