# fun-ecs Scheduler Contract

status: active baseline
schema_version: fun_ecs_scheduler_contract_v1
owner: fun

## Boundary

`fun-ecs` declares schedule intent. `fun-scheduler` admits and executes work.
ECS schedule declarations must be scheduler-native before runtime execution.

## Public Sets

The public spatial schedule keeps 15 high-level sets:

- `sense_sources`
- `build_interest`
- `plan_stream_wave`
- `diff_requests`
- `apply_stream_commands`
- `acquire_sources`
- `decode_pages`
- `build_derived_artifacts`
- `propagate_dirty_regions`
- `publish_renderer_handoffs`
- `publish_lux_handoffs`
- `publish_physics_handoffs`
- `publish_network_handoffs`
- `evict_cold_pages`
- `flush_diagnostics`

## Compiled Graph

`compile_spatial_schedule_graph()` emits scheduler-native nodes. It preserves
the public set surface and inserts internal command barriers:

- `apply_request_commands` after `diff_requests`
- `apply_artifact_commands` after `build_derived_artifacts`
- `apply_dirty_propagation_commands` after `propagate_dirty_regions`
- `apply_handoff_commands` after handoff publishers

Each barrier uses `EcsWorkKind::ApplyCommands` and
`ScheduleLane::EcsCommandBarrier`.

`EcsSpatialScheduleCompiler::compile()` is the first-class spatial compiler. It
lowers the public schedule declarations into
`WorkGraph<EcsWork<ProductRegistry>>` with:

- scalar nodes for small control stages
- chunk nodes for `decode_pages`
- one artifact-build chunk per artifact consumer
- explicit procedural world manifest reads for source acquire and page decode
- explicit apply/finalization barrier nodes
- scheduler access rows for table/resource reads and writes
- command-buffer IDs for request, artifact, dirty-propagation, and handoff
  buffers
- cross-domain wait-for edges from produced wait tokens to consumers
- a shared conflict set for resource/table writers
- a liveness proof, build report, chunk plan, barrier plan, access plan, and
  deterministic graph digest

Default compilation emits two decode chunks, eight artifact-consumer chunks,
five barrier nodes, and preserves the order of the 15 public high-level sets.

`compile_spatial_schedule_work_graph()` lowers those compiled nodes into a
`fun-scheduler` `WorkGraph<FunEcsRuntimeWork>` for the older runtime adapter.
It remains a compatibility bridge; new orchestration code should consume
`EcsSpatialScheduleCompiler::compile()`.

Each `FunEcsRuntimeWork` carries the `ScheduleRevision` observed when the graph
was compiled. Runners must treat that revision as the schedule/access metadata
snapshot used for admission and stale-work validation.

## System Descriptor Bridge

Generic `FunSystemDescriptor` values convert into scheduler-facing
`EcsSystemDescriptor` values. The conversion carries:

- `EcsSystemClass`
- `EcsSystemExecutionContract`
- `EcsWorkKind`
- component/resource access rows
- table chunk access rows
- chunk keys
- virtual resource keys
- external artifact keys
- command-buffer outputs
- awaited wait tokens
- produced wait tokens

Every current spatial schedule set has a descriptor. The descriptors keep
`acquire_sources` on the blocking lane, make `decode_pages` and
`build_derived_artifacts` chunkable, declare command buffers for command
emitters, and keep optional eviction and diagnostics as idle work.
Procedural generation is scheduler-owned decode work: `fun-ecs` declares the
manifest and page recipe inputs, and `fun-scheduler` admits the chunked decode
nodes before any artifact or handoff work can observe the page.
Network validation samples procedural page digests after local decode. Digest
probes are metadata checks only; generated voxel pages, meshes, surfaces, and
GPU resources remain local disposable artifacts.

The spatial compiler validates these descriptor outputs at graph-build time:
blocking source acquire is only emitted on the scheduler blocking lane, decode
and artifact builds are chunked, required reads have wait-token producers, apply
barriers do not form cycles, and renderer handoff does not wait on optional Lux
or physics tokens.

## Determinism Rule

Command buffers are sorted by typed command keys at apply time. Registry and
handoff queue state must not depend on system completion order.

Checked command-buffer apply compares the buffer's observed revision against
the relevant revision ledger category before draining commands. Stale buffers
reject deterministically before mutating registries or handoff queues.

## Liveness Rule

Cross-domain waits use typed wait tokens. Renderer present only waits on
required render artifacts when no valid fallback exists. Physics fixed step can
use conservative fallback. Optional Lux, foliage, and refinement work cannot
gate renderer present.
