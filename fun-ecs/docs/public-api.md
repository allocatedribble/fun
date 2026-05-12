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
  `submit_to_fun_scheduler`, `EcsNodeRunner`.
- Artifacts: `ArtifactDag`, `ArtifactManifest`, `ArtifactReadinessToken`,
  `CrossDomainHandoffQueue`.

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
