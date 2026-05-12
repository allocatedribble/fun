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

`compile_spatial_schedule_work_graph()` lowers those compiled nodes into a
`fun-scheduler` `WorkGraph<FunEcsRuntimeWork>`. The graph uses deterministic
descriptor ordering, frame deadline, and explicit dependency edges in the
compiled spatial order.

Each `FunEcsRuntimeWork` carries the `ScheduleRevision` observed when the graph
was compiled. Runners must treat that revision as the schedule/access metadata
snapshot used for admission and stale-work validation.

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
