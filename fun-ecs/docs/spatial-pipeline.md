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
10. propagate dirty regions and voxel edits
11. apply dirty propagation commands
12. publish renderer, Lux, physics, and network handoffs
13. apply handoff commands
14. evict cold pages
15. flush diagnostics

## Doctrine

- No page-per-entity design.
- High-level spatial controls may be ECS entities.
- Hot page state lives in dense resources.
- Terrain truth is the procedural world manifest, generator version, page key,
  biome recipe, deterministic fixed-point math, and future deltas.
- Multiplayer terrain synchronization sends the procedural sync manifest and
  page digest probes only. It does not replicate voxel pages, meshes, surface
  packets, GPU resources, or renderer artifacts.
- Server authority covers the world manifest, generator version, seed, spawn
  positions, and future delta layer. Clients own only local streaming priority,
  renderer artifact realization, cache eviction, and debug/profiling display.
- Procedural terrain generation materializes only bounded local page payloads;
  it never implies dense global voxel storage.
- The multiplayer checksum is `ProceduralPageDigest`: same sync manifest,
  generator version, local biome/material tables, and page key must produce the
  same decoded page digest on server and client.
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
