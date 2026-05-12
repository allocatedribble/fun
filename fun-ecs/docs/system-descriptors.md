# fun-ecs System Descriptors

status: active baseline
schema_version: fun_ecs_system_descriptors_v1
owner: fun

## Boundary

`fun-ecs` owns ECS system declarations and access extraction. `fun-scheduler`
owns admission, lanes, budgets, liveness proofs, and execution.

The bridge is:

- `FunSystemDescriptor`
- `FunSystemAccess`
- `FunSystemParam`
- `FunSystemChunkPolicy`
- `FunRunCondition`
- `FunSystemSet`
- `FunSystemExecutionContract`

Each descriptor converts into scheduler-facing `EcsSystemDescriptor`,
`SystemAccess`, `EcsSystemClass`, `EcsSystemExecutionContract`, `EcsWorkKind`,
chunk keys, virtual resource keys, external artifact keys, command buffers, and
wait-token edges.

## Params

The param declaration layer includes:

- `Query<T, Filter>`
- `EntityRef<T>`
- `EntityMut<T>`
- `Added<T>`
- `Changed<T>`
- `With<T>`
- `Without<T>`
- `Or<T>`
- `And<T>`
- `Res<T>`
- `ResMut<T>`
- `TableRef<T>`
- `TableMut<T>`
- `TableChunkRef<T>`
- `TableChunkMut<T>`
- `ExternalSlabRef<T>`
- `ExternalSlabMut<T>`
- `VirtualResourceRef<T>`
- `VirtualResourceMut<T>`
- `ExternalArtifactRef<T>`
- `ExternalArtifactMut<T>`
- `Commands`
- `SpatialCommands`
- `ArtifactCommands`
- `HandoffCommands`
- `Events<T>`
- `EventWriter<T>`
- `EventReader<T>`

Params extract deterministic rows for component, component chunk, resource,
resource table, table chunk, virtual resource, external artifact, external
slab, command-buffer, and wait-token access.

## Hard Rules

- ordinary systems cannot write world structure directly
- structural changes go through command buffers
- command emitters declare the command buffer they write
- systems that wait declare `EcsLivenessClass::SchedulerWait`
- systems touching external slabs or artifacts cannot wait unless explicitly
  marked safe
- non-send params pin the descriptor to the main thread

## Spatial Baseline

Each current spatial schedule set now has a `FunSystemDescriptor`:

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

The descriptors preserve the old schedule-set policy while exposing scheduler
access rows. `acquire_sources` is blocking-lane work. `decode_pages` and
`build_derived_artifacts` are chunkable. Eviction and diagnostics remain
optional idle work.
