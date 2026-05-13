# fun-ecs Spatial Pipeline

status: active baseline
schema_version: fun_ecs_spatial_pipeline_v1
owner: fun

## Pipeline

The current spatial pipeline is:

1. sense active cameras and stream sources
2. build stream interests
3. plan stream waves
4. diff requested and pinned pages
5. apply request commands
6. acquire source rows from the procedural manifest or package/delta sources
7. decode pages, including deterministic procedural terrain generation
8. build derived artifacts
9. apply artifact commands
10. publish renderer handoffs
11. publish Lux handoffs
12. publish physics handoffs
13. apply handoff commands
14. evict cold pages
15. flush diagnostics

## Doctrine

- No page-per-entity design.
- High-level spatial controls may be ECS entities.
- Hot page state lives in dense resources.
- Terrain truth is the procedural world manifest, generator version, page key,
  biome recipe, deterministic fixed-point math, and future deltas.
- `spatial/procedural.rs` owns the typed generator descriptor and
  `ProceduralTerrainPageRecipe`; stream requests carry that recipe into decode
  work without copying terrain pages, meshes, or renderer artifacts.
- `acquire_sources` emits `EcsSourcePayload::ProceduralTerrainRecipe` for
  valid terrain occupancy/material/surface page requests. It performs manifest
  validation and recipe construction only; procedural terrain acquisition does
  not perform disk IO or blocking generation work.
- The first terrain profile is `BEDROCK_QUARRY`: rolling hills, deterministic
  ridge fields, cliff-band terracing, and strata material bands. It has no full
  caves, no foliage generation, and no water simulation.
- The base gameplay/storage grid stays foot-scale: one voxel is 304,800
  micrometers, and one page is 32 x 32 x 32 voxels. Inch-scale detail is a
  future bounded overlay, not the base generator.
- Multiplayer terrain synchronization sends the procedural sync manifest and
  page digest probes only. It does not replicate voxel pages, meshes, surface
  packets, GPU resources, or renderer artifacts.
- Startup synchronization is `ProceduralWorldHandshake`: server sends the sync
  manifest, deterministic initial region seed-table digest, spawn position in
  feet, and server tick. Clients validate supported schema/version, local biome
  and material table digests, and the expected foot-scale world mode before
  accepting the world.
- Region and page sub-seeds come from `region_seed(world_seed, region)` and
  `page_seed(world_seed, page)`. They depend only on the world seed and spatial
  key, never generation order, so stream order cannot affect terrain content.
- Runtime multiplayer validation is metadata-only. Servers send
  `ProceduralPageDigestSample` rows near players; clients report
  `ProceduralPageDigestMismatchReport` only when their local digest differs.
  Page contents, generated meshes, surface packets, and GPU resources are not
  network payloads.
- `ProceduralTerrainDeltaLayerHeader` is reserved now with an empty
  manifest-bound delta layer. Future edits and destruction append deltas without
  changing the base procedural manifest contract.
- Server authority covers the world manifest, generator version, seed, spawn
  positions, and future delta layer. Clients own only local streaming priority,
  renderer artifact realization, cache eviction, and debug/profiling display.
- Procedural terrain generation materializes only bounded local page payloads;
  it never implies dense global voxel storage.
- Decode estimates each page's `PageHeightRelation` before voxel evaluation by
  sampling the page footprint corners and center, then applying a deterministic
  fixed-point height margin. Pages entirely above terrain become `EmptyAir`,
  pages entirely below terrain become `UniformSolid`, and only
  surface-intersecting pages enter mixed-page generation.
- Mixed-page generation is cluster-major over 8 x 8 x 8 voxel clusters. Each
  cluster is sampled and classified as empty, solid, or mixed before any dense
  cell loop. Empty and solid clusters update summaries and occupancy directly;
  only mixed clusters evaluate individual voxels.
- `ProceduralGenerationScratch` is the reusable generation-context buffer for
  occupancy words, temporary material indices, cluster summaries, and
  `ProceduralSurfaceFace` hints. Scheduler decode reuses it across procedural
  page rows so empty/uniform fast paths do not allocate dense page buffers and
  surface pages avoid allocation storms.
- The material hot path uses deterministic array slots rather than map
  iteration: 0 air, 1 grass, 2 dirt, 3 rock, 4 snow, and 5 sand. Surface pages
  with this tiny page palette are emitted as `PaletteRle` payloads instead of
  requiring a dense material array.
- Occupancy popcount, dominant material, min/max local height, SDF bounds,
  exposed-face hints, material palette membership, source epoch, and digest
  inputs are produced while generation walks the page; there is no
  generate-then-summarize pass over dense voxels.
- Procedural decode materializes `ProceduralGeneratedPage` first, then adapts
  it into the decoded page queue. That generated page owns the page class,
  payload kind, occupancy storage, material palette, cluster summaries,
  source epoch, and deterministic `ProceduralPageDigest`.
- `DecodePages` and `BuildDerivedArtifacts` are scheduler-visible chunked
  terrain work. Their chunk key is the source page key's scheduler chunk key,
  and `ProceduralGenerationChunk` carries request range, estimated voxel cost,
  and priority floor for admission and cancellation decisions.
- Camera-containing and near visible generation is required frame-deadline
  work when shell zero is involved. Outer shell work uses stream deadlines,
  collision-critical physics uses fixed-step deadlines, Lux/shadow refinement
  uses stream deadlines, and diagnostics overlays use idle-window deadlines.
- Far pages after teleport, stale source recipes, outer-shell pressure work,
  and optional SDF/shadow/physics work can be cancelled or deferred. The
  camera-containing page, active near collision shell, in-progress command
  barrier, and visible page without fallback cannot be cancelled.
- Generated pages carry `ProceduralGeneratedPageClass`. Empty air pages emit no
  surface, physics, or Lux artifacts; uniform solid pages emit aggregate
  physics-only artifacts when requested; surface and feature pages emit surface,
  material, coarse-proxy, Lux, and optional physics artifacts.
- The first visual terrain build profile emits only terrain coarse proxy,
  terrain surface packets, terrain material page, and load animation record.
  Terrain SDF, physics cook requests, and virtual shadow invalidation are
  optional refinement artifacts behind explicit `ProceduralTerrainArtifactFlags`
  and cannot block first visual terrain.
- Prototype terrain artifact payloads are typed ECS products:
  `ProceduralTerrainCoarseProxy` summarizes height/material/occupied clusters,
  `ProceduralTerrainSurfacePacket` carries blocky exposed-face packet bounds,
  `ProceduralTerrainMaterialPage` carries the page palette and cluster dominant
  materials, and `ProceduralTerrainLoadAnimationRecord` maps stream progress
  from request through renderer publication.
- The multiplayer checksum is `ProceduralPageDigest`: same sync manifest,
  generator version, local biome/material tables, and page key must produce the
  same decoded page digest on server and client.
- Prototype terrain performance gates are executable diagnostics, not prose-only
  targets. `run_procedural_terrain_prototype_benchmark` covers cold spawn,
  streaming treadmill, teleport, multiplayer digest, and negative-coordinate
  worlds using the same stream, acquire, decode, artifact, and renderer-handoff
  path as runtime terrain. Reports carry time-to-first coarse/surface terrain,
  generated and fast-path-skipped page counts, renderer handoffs, frame p95,
  treadmill queue/eviction/upload rates, teleport stale cancellation and shell-0
  latency, multiplayer digest matches, and negative-coordinate floor-division
  checks.
- Mutation crosses deterministic command-buffer barriers.
- Handoffs are typed records.
- Derived artifacts feed consumers without creating render pages.
- Optional Lux, foliage, and refinement work cannot block renderer present.
- Physics fixed step can use conservative fallback.

## Current Apply Semantics

Artifact and handoff publishers emit `EcsSpatialCommand` values. The compiled
schedule applies artifact commands before handoff publishers read registries and
applies handoff commands before subsystem consumers read queues.

`propagate_voxel_edit` still updates page dirty epochs and the dirty ledger
inline while emitting replayable dirty commands and artifact or physics commands.
That preserves the current behavioral oracle while the generic kernel grows.
