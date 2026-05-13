# fun-ecs Public API

status: active
schema_version: fun_ecs_public_api_v1

## Stable Facade

The first-class product-facing API is `fun_ecs::api`.

Stable symbols:

- World: `World`, `FunWorld`, `FunWorldBuilder`, `FunWorldRevision`,
  `FunFrameContext`.
- Storage: `ResourceTable<T>`, `ResourceTableKey`, `ResourceTableChunk<T>`,
  `ExternalSlabHandle`, `VirtualResourceHandle`.
- Systems: `IntoFunSystem`, `FunSystemSet`, `FunSchedule`,
  `FunRunCondition`, `FunSystemExecutionContract`.
- Commands: `Commands`, `SpatialCommands`, `CommandJournal`,
  `CommandApplyReport`.
- Scheduler: `compile_schedule_graph`, `compile_spatial_frame_graph`,
  `submit_to_fun_scheduler`, `EcsNodeRunner`, `ProceduralGenerationChunk`,
  `ProceduralGenerationChunkSpec`,
  `ProceduralTerrainSchedulerWorkClass`,
  `ProceduralTerrainCancellationContext`,
  `ProceduralTerrainCancellationDecision`,
  `procedural_generation_deadline`, `procedural_priority_requiredness`,
  `procedural_terrain_work_deadline`,
  `procedural_terrain_work_requiredness`,
  `procedural_terrain_artifact_deadline`,
  `procedural_terrain_artifact_requiredness`,
  `procedural_terrain_cancellation_decision`.
- Artifacts: `ArtifactDag`, `ArtifactManifest`, `ArtifactReadinessToken`,
  `CrossDomainHandoffQueue`, `ProceduralTerrainArtifactFlags`,
  `ProceduralTerrainCoarseProxy`, `ProceduralTerrainSurfacePacket`,
  `ProceduralTerrainMaterialPage`, `ProceduralTerrainLoadAnimationRecord`,
  `ProceduralTerrainLoadStage`, `build_prototype_terrain_artifacts`.
- Terrain: `EcsProceduralWorldManifest`, `EcsBiomeRecipe`,
  `EcsProceduralTerrainSource`, `EcsTerrainGeneratorVersion`,
  `ProceduralTerrainGeneratorDesc`, `ProceduralTerrainShapeDesc`,
  `ProceduralBiomeDesc`, `ProceduralMaterialDesc`,
  `ProceduralFeatureDesc`, `ProceduralGenerationBudget`,
  `ProceduralGenerationScratch`, `ProceduralSurfaceFace`,
  `ProceduralTerrainPageRecipe`, `PageHeightRelation`,
  `ProceduralWorldSyncManifest`, `ProceduralWorldAuthorityPolicy`,
  `ProceduralWorldHandshake`, `ProceduralWorldHandshakeClientExpectation`,
  `ProceduralTerrainDeltaLayerHeader`, `ProceduralGeneratedPageClass`,
  `ProceduralGeneratedPage`, `ProceduralPageDigest`,
  `ProceduralPageDigestProbe`, `ProceduralPageDigestSample`,
  `ProceduralPageDigestMismatchReport`, `hash2`, `hash3`,
  `value_noise2_q16`, `value_noise3_q16`, `fbm2_q16`, `region_seed`,
  `page_seed`, `initial_region_seed_table_digest`, `terrain_height_ft`,
  `estimate_page_height_relation`, `generate_procedural_terrain_page`,
  `generate_procedural_terrain_page_with_scratch`,
  `generate_procedural_generated_page`,
  `generate_procedural_generated_page_with_scratch`,
  `generate_procedural_page_digest`.
- Diagnostics: `ProceduralTerrainPrototypeBenchmarkKind`,
  `ProceduralTerrainPrototypeBenchmarkReport`,
  `PROCEDURAL_TERRAIN_PROTOTYPE_BENCHMARKS`,
  `ECS_PROCEDURAL_TERRAIN_PROTOTYPE_BENCHMARKS`,
  `run_procedural_terrain_prototype_benchmark`.

Existing crate modules remain visible for compatibility while the strangler
rewrite lands. Product-critical server paths should depend on this stable
facade instead of directly binding to storage experiments.

## Experimental Facade

Experimental API is available through `fun_ecs::api::experimental` only when a
matching Cargo feature is enabled:

- `experimental-native-archetypes`
- `experimental-storage-promotion`
- `experimental-materialized-groups`
- `experimental-temporal-snapshots`
- `experimental-speculative-systems`
- `experimental-unsafe-storage`

Experimental APIs are opt-in and are not allowed on product-critical server
paths. The `experimental-unsafe-storage` flag is a named reservation only; the
crate remains `#![forbid(unsafe_code)]`.
