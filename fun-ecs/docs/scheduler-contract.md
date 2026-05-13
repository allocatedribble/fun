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
- `apply_artifact_commands`
- `publish_renderer_handoffs`
- `publish_lux_handoffs`
- `publish_physics_handoffs`
- `apply_handoff_commands`
- `evict_cold_pages`
- `flush_diagnostics`

## Compiled Graph

`compile_spatial_schedule_graph()` emits scheduler-native nodes. It preserves
the public set surface and inserts internal command barriers:

- `apply_request_commands` after `diff_requests`
- `apply_artifact_commands` after `build_derived_artifacts`
- `apply_handoff_commands` after physics handoff publication

Each barrier uses `EcsWorkKind::ApplyCommands` and
`ScheduleLane::EcsCommandBarrier`.

`EcsSpatialScheduleCompiler::compile()` is the first-class spatial compiler. It
lowers the public schedule declarations into
`WorkGraph<EcsWork<ProductRegistry>>` with:

- scalar nodes for small control stages
- page-keyed chunk nodes for `decode_pages`
- page-keyed chunk nodes for `build_derived_artifacts`
- explicit procedural world manifest reads for source acquire and page decode
- explicit request, artifact, and handoff apply barrier nodes
- scheduler access rows for table/resource reads and writes
- command-buffer IDs for request, artifact, and handoff buffers
- cross-domain wait-for edges from produced wait tokens to consumers
- a shared conflict set for resource/table writers
- a liveness proof, build report, chunk plan, barrier plan, access plan, and
  deterministic graph digest

Default compilation emits two procedural generation chunks, two decode chunks,
two artifact page chunks, three barrier nodes, and preserves the order of the
15 public high-level sets.

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
manifest and `ProceduralTerrainPageRecipe` inputs, and `fun-scheduler` admits
the chunked decode nodes before any artifact or handoff work can observe the
page.
`ProceduralGenerationChunk` records the first stream request, request count,
estimated voxel cost, and priority floor for each chunk. Its chunk key is the
source page key's scheduler chunk key; synthetic decode and artifact chunk keys
are not used for procedural terrain work.
Camera-containing and near visible page generation uses a frame deadline when
the priority shell is zero. Outer-shell generation uses a stream deadline,
collision-critical physics proxy work uses fixed-step deadlines, Lux/shadow
work uses stream deadlines, and diagnostics overlays use idle-window
deadlines.
Required prototype terrain work is the camera-containing coarse proxy, nearby
visible surface packets, visible material page, and renderer handoff for
visible/coarse fallback. Load animation, Lux invalidation, terrain SDF,
physics outside the near collision shell, far refinement, and diagnostics
overlays are optional.
Cancellation policy is explicit: far pages after teleport, stale source
recipes, outer-shell pressure work, and optional SDF/shadow/physics work can be
cancelled or deferred. Camera-containing pages, the active near collision
shell, in-progress command barriers, and currently visible pages with no
fallback are never cancelled.
The decode path spends `ProceduralGenerationBudget::max_pages_generated_per_frame`
while converting `EcsSourcePayload::ProceduralTerrainRecipe` rows into
`ProceduralGeneratedPage` records; excess rows remain deferred for a later
decode frame instead of running unbounded generation.
The generated page output includes `ProceduralGeneratedPageClass`, and derived
artifact builders use that class as an admission filter before allocating
surface, Lux, or physics outputs for a page.
The prototype artifact profile is first-visual biased: renderer-visible coarse
proxy, surface packets, material page, and load animation records are admitted
by default. Terrain SDF, physics cook requests, and virtual shadow invalidation
are optional `ProceduralTerrainArtifactFlags`; their work is optional and must
not gate renderer present.
Decode must run the cheap `PageHeightRelation` path before dense occupancy so
empty-air and uniform-solid pages do not spend scheduler budget on all 32^3
voxels.
`EcsNodeRunner` owns a `ProceduralGenerationScratch` generation context and
passes it into procedural decode. That keeps occupancy words, temporary material
indices, cluster summaries, and surface-face hints reusable across page rows
within the scheduler worker instead of allocating a fresh dense buffer for each
page.
Mixed pages are generated in 8 x 8 x 8 cluster order. Cluster height bounds are
sampled before dense work, so empty and solid clusters update compact summaries
directly and only mixed clusters spend per-voxel budget.
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
