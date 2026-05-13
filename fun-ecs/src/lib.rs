#![forbid(unsafe_code)]

pub mod adapters;
pub mod api;
#[path = "spatial/artifact.rs"]
pub mod artifact;
#[path = "spatial/authority.rs"]
pub mod authority;
#[path = "core/control_plane.rs"]
pub mod control;
pub mod core;
pub mod diagnostics;
#[path = "spatial/dirty.rs"]
pub mod dirty;
pub mod experiments;
#[path = "spatial/frame_graph.rs"]
pub mod frame_graph;
#[path = "spatial/load_animation.rs"]
pub mod load_animation;
#[path = "spatial/page_table.rs"]
pub mod page_table;
#[path = "spatial/procedural.rs"]
pub mod procedural;
#[path = "spatial/residency.rs"]
pub mod residency;
pub mod runtime;
#[path = "spatial/schedule.rs"]
pub mod schedule;
#[path = "spatial/source.rs"]
pub mod source;
#[path = "spatial/types.rs"]
pub mod spatial;
#[path = "core/storage.rs"]
pub mod storage;
#[path = "spatial/streaming.rs"]
pub mod streaming;
#[path = "spatial/terrain.rs"]
pub mod terrain;
#[path = "spatial/voxel.rs"]
pub mod voxel;

pub use adapters::{
    AiEcsBridge, AiProposalPlan, AnimationEcsBridge, AnimationPoseArtifactManifest, AvisEcsBridge,
    ConservativePhysicsFallback, FUN_ECS_ADAPTER_CONTRACTS, FunEcsAdapterContract,
    FunEcsAdapterKind, FunEcsAdapterRole, LuxDirtyRow, LuxDirtyRowTable, LuxEcsBridge,
    LuxFallbackPolicy, LuxHandoffImportPlan, NetworkDeltaManifest, NetworkRelevanceRow,
    NetworkRelevanceTable, NetworkRollbackApplyPlan, PerPeerEcsBudgetBridge, PhysicsCookImportPlan,
    PhysicsProxyManifest, PhysicsWritebackApplyPlan, RenderableDeclaration,
    RendererArtifactImportPlan, RendererArtifactRetirePlan, RendererEcsBridge,
    RendererFrameSnapshot, RendererPresentDependencyValidator, RvelteEcsBridge,
    RvelteGenerationSnapshot, RvelteInputEnvelope, RveltePaintPacketManifest,
    RvelteRendererHandoffQueue, RvelteStateResource, SubsystemContractError, TelemetryEcsBridge,
    TelemetryReport, TelemetryReportKind, ThunderEcsBridge, WardenEcsBridge,
};
pub use api::{
    ArtifactManifest, CommandApplyReport, CommandJournal, CrossDomainHandoffQueue,
    ExternalSlabHandle, FUN_ECS_STABLE_API_SYMBOLS, FunExperimentalApiFeature,
    FunExperimentalApiGate, FunPublicApiContractReport, FunSchedule, FunStableApiArea,
    FunStableApiSymbol, IntoFunSystem, ResourceTable, ResourceTableChunk, ResourceTableKey,
    VirtualResourceHandle, World, compile_schedule_graph, compile_spatial_frame_graph,
    experimental_api_gates, public_api_contract_report, stable_api_symbols,
    submit_to_fun_scheduler,
};
pub use artifact::{
    ArtifactBuildPlan, ArtifactConsumer, ArtifactConsumerRead, ArtifactConsumerReadDecision,
    ArtifactDag, ArtifactDagError, ArtifactDependency, ArtifactFallbackPolicy, ArtifactInput,
    ArtifactNode, ArtifactNodeId, ArtifactOutput, ArtifactProducer, ArtifactReadinessToken,
    ArtifactRetirePlan, CollisionCookMode, ECS_DERIVED_ARTIFACT_BUILD_SYSTEM_COUNT,
    ECS_DERIVED_ARTIFACT_BUILD_SYSTEMS, ECS_PROCEDURAL_TERRAIN_OPTIONAL_ARTIFACT_KINDS,
    ECS_PROCEDURAL_TERRAIN_REQUIRED_ARTIFACT_KINDS, ECS_PROCEDURAL_TERRAIN_REQUIRED_BUILD_SYSTEMS,
    EcsArtifactBuildReport, EcsArtifactConsumer, EcsArtifactState, EcsCrossDomainHandoff,
    EcsCrossDomainHandoffKind, EcsCrossDomainHandoffQueues, EcsDerivedArtifactBuildSystem,
    EcsDerivedArtifactKind, EcsDerivedArtifactRecord, EcsDerivedArtifactRegistry,
    EcsLuxHandoffPublishReport, EcsLuxHandoffQueue, EcsPhysicsCookPublishReport,
    EcsPhysicsCookQueue, EcsPhysicsSourceTruth, EcsRendererHandoffPublishReport,
    EcsRendererHandoffQueue, EcsTerrainPhysicsRules, FoliageArtifactLodBand, LuxArtifactHandoff,
    LuxArtifactHandoffKind, PackedAabb, PackedRange, ProceduralTerrainArtifactFlags,
    ProceduralTerrainCoarseProxy, ProceduralTerrainLoadAnimationRecord, ProceduralTerrainLoadStage,
    ProceduralTerrainMaterialPage, ProceduralTerrainSurfacePacket, RendererArtifactHandoff,
    RendererArtifactHandoffKind, RendererVisibilityHint, VoxelPhysicsCookRequest,
    VoxelSurfacePacketArtifact, artifact_virtual_resource_key, build_derived_artifacts,
    build_prototype_terrain_artifacts, derived_artifact_source_digest, lux_handoff_from_artifact,
    physics_cook_from_artifact, procedural_terrain_coarse_proxy_from_decoded_page,
    procedural_terrain_material_page_from_decoded_page,
    procedural_terrain_surface_packet_from_decoded_page, publish_lux_handoffs,
    publish_physics_cooks, publish_renderer_handoffs, renderer_handoff_from_artifact,
    rvelte_artifact_virtual_resource_key,
};
pub use authority::{
    EcsAssetRef, EcsAuthoringCommandKind, EcsAuthoringEditCommand, EcsAuthoringTool, EcsDebugPin,
    EcsSpatialCommand, EcsSpatialCommandApplyDigest, EcsSpatialCommandApplyReport,
    EcsSpatialCommandBuffer, EcsSpatialCommandSortKey, EcsVoxelEditLog, EcsVoxelEditLogEntry,
    PackedTransform, VoxelEditOp, apply_artifact_commands, apply_artifact_commands_with_revisions,
    apply_dirty_propagation_commands, apply_dirty_propagation_commands_with_revisions,
    apply_handoff_commands, apply_handoff_commands_with_revisions,
};
pub use control::{
    FUN_ECS_MAX_RESOURCE_CHUNKS, FUN_ECS_MAX_SYSTEM_DECLARATIONS,
    FUN_ECS_REQUIRED_HANDOFF_CONSUMER_COUNT, FUN_ECS_REQUIRED_HANDOFF_CONSUMERS,
    FUN_ECS_SUBSYSTEM_HANDOFF_CONTRACTS, FunEcsAccessMode, FunEcsComponentKind, FunEcsControlPlane,
    FunEcsLivenessReport, FunEcsResourceChunk, FunEcsResourceKind, FunEcsRevision, FunEcsSubsystem,
    FunEcsSubsystemHandoffContract, FunEcsSystemAccess, FunEcsSystemDeclaration, FunEcsSystemId,
    FunEcsValidationError, FunEcsWorldId, validate_subsystem_liveness,
};
pub use core::{
    Added, AgentAccessScope, AgentCommandEnvelope, AgentManifest, AgentMutationProposal,
    AgentProposalCommandBuffer, AgentProposalDiagnostic, AgentProposalDiagnosticCode,
    AgentProposalValidationReport, AgentProposalWorldState, And, ArtifactCommandBuffer,
    ArtifactCommands, ArtifactRevision, AuthoringCommandBuffer, Changed, ColdPayloadStore,
    ColumnarRowView, Commands, DENSE_RESOURCE_TABLE_BENCHMARK_ROW_COUNT, DenseResourcePriorityBand,
    DenseResourceTable, DenseResourceTableAccess, DenseResourceTableAccessKind,
    DenseResourceTableChunk, DenseResourceTableChunkRow, DenseResourceTableConfig,
    DenseResourceTableDigest, DenseResourceTableIndex, DenseResourceTableKey,
    DenseResourceTableRevision, DenseResourceTableRow, DenseResourceTableStats, EntityMut,
    EntityRef, EventReader, EventWriter, Events, ExternalArtifactMut, ExternalArtifactRef,
    ExternalSlabMut, ExternalSlabRef, ExternalSlabRevision, FUN_COMMAND_BUFFER_ARTIFACTS,
    FUN_COMMAND_BUFFER_DEFAULT_CAPACITY, FUN_COMMAND_BUFFER_DIRTY_PROPAGATION,
    FUN_COMMAND_BUFFER_HANDOFFS, FUN_COMMAND_BUFFER_SPATIAL_REQUESTS,
    FUN_COMMAND_BUFFER_WORLD_STRUCTURE, FUN_COMMAND_JOURNAL_MAX_ROWS,
    FUN_ECS_MAX_RESOURCE_REVISION_RECORDS, FUN_ECS_MAX_REVISION_TOUCH_ROWS,
    FUN_WORLD_INITIALIZED_SPATIAL_RESOURCE_COUNT, FUN_WORLD_SPATIAL_RESOURCE_KINDS, FrameRevision,
    FunArchetypeId, FunArtifactId, FunCommandApplyReport, FunCommandApplyRevisionReport,
    FunCommandBuffer, FunCommandBufferClass, FunCommandBufferDeclaration, FunCommandBufferId,
    FunCommandBufferOutput, FunCommandDeterministicKey, FunCommandDigest, FunCommandDrainPolicy,
    FunCommandEnvelope, FunCommandJournal, FunCommandKind, FunCommandMergePolicy,
    FunCommandPayload, FunCommandPhase, FunCommandValidationError, FunComponentId,
    FunComponentParam, FunEntity, FunEntityGeneration, FunEventParam, FunExternalArtifactParam,
    FunExternalSlabId, FunExternalSlabParam, FunExternalWaitSafety, FunFrameId, FunQueryAccess,
    FunQueryFilterAccess, FunQueryId, FunQueryMetadata, FunQueryValidationError, FunResourceAccess,
    FunResourceId, FunResourceParam, FunResourceTableId, FunRevision, FunRevisionLedgerError,
    FunRunCondition, FunRunConditionId, FunSchedulerEcsRegistry, FunSchedulerVirtualResourceKey,
    FunSchedulerWaitToken, FunSchedulerWaitTokenId, FunSpatialPageVirtualResourceKey,
    FunStorageChunkId, FunSubsystemCommand, FunSystem, FunSystemAccess, FunSystemAccessMode,
    FunSystemAccessRow, FunSystemAccessTarget, FunSystemChunkPolicy, FunSystemClass,
    FunSystemDescriptor, FunSystemExecutionContract, FunSystemId, FunSystemParam,
    FunSystemParamAccess, FunSystemSet, FunSystemSetId, FunSystemValidationError, FunTableParam,
    FunVirtualResourceParam, FunWaitTokenDirection, FunWorld, FunWorldBuilder, FunWorldDiagnostics,
    FunWorldId, FunWorldMode, FunWorldResourceSet, FunWorldRevision, FunWorldSchedulerAuthority,
    FunWorldStorageBackend, HandoffCommandBuffer, HandoffCommands, HandoffRevision, HotFieldMask,
    IntoSchedulerChunkKey, IntoSchedulerEntityId, IntoSchedulerVirtualResourceKey,
    IntoSchedulerWaitToken, NetworkCommandBuffer, Or, PhysicsCommandBuffer, Query,
    RendererCommandBuffer, Res, ResMut, ResourceRevisionLedger, ResourceRevisionRecord,
    ResourceTableLayout, ResourceTableRevision, ResourceTableUseCase, RevisionCategory,
    ScheduleRevision, SpatialCommandBuffer, SpatialCommandJournalContext, SpatialCommands,
    SpatialPageRevision, TableChunkMut, TableChunkRef, TableLayoutAdvice, TableLayoutAdvisor,
    TableMut, TableRef, UiCommandBuffer, VirtualResourceMut, VirtualResourceRef, With, Without,
    WorldRevisionLedger, external_slab_virtual_resource_key, scheduler_component_id,
    scheduler_resource_id, scheduler_system_id, scheduler_table_resource_id,
    spatial_command_journal_from_commands, stage_agent_proposal, validate_agent_proposal,
};
pub use diagnostics::{
    ECS_BENCHMARK_TOTAL_WORKLOADS, ECS_DIAGNOSTICS_UI_VIEW_COUNT, ECS_DIAGNOSTICS_UI_VIEWS,
    ECS_ENTITY_KERNEL_BENCHMARKS, ECS_PROCEDURAL_TERRAIN_PROTOTYPE_BENCHMARKS,
    ECS_RESOURCE_TABLE_KERNEL_BENCHMARKS, ECS_SCHEDULER_KERNEL_BENCHMARKS,
    ECS_SPATIAL_BASELINE_BENCHMARKS, EcsArtifactReport, EcsBenchmarkConsumer,
    EcsBenchmarkSuiteKind, EcsBenchmarkWorkload, EcsBenchmarkWorkloadKind, EcsCommandReport,
    EcsCrossDomainReport, EcsDiagnosticsUiPacket, EcsDiagnosticsUiPacketSet, EcsDiagnosticsUiView,
    EcsFrameReport, EcsHandoffReport, EcsQueryReport, EcsReplacementGateCategory,
    EcsReplacementGateInput, EcsReplacementGateRejectReason, EcsReplacementGateReport,
    EcsScheduleReport, EcsStorageReport, EcsTableReport, FunEcsGraphDebugSnapshot,
    PROCEDURAL_TERRAIN_BENCHMARK_SEED, PROCEDURAL_TERRAIN_COLD_SPAWN_CAMERA_FT,
    PROCEDURAL_TERRAIN_COLD_SPAWN_DESIRED_SHELLS, PROCEDURAL_TERRAIN_COLD_SPAWN_REQUIRED_SHELLS,
    PROCEDURAL_TERRAIN_NEGATIVE_CAMERA_FT, PROCEDURAL_TERRAIN_PROTOTYPE_BENCHMARKS,
    PROCEDURAL_TERRAIN_TELEPORT_DESTINATION_FT, PROCEDURAL_TERRAIN_TREADMILL_DESIRED_SHELLS,
    PROCEDURAL_TERRAIN_TREADMILL_FRAMES, PROCEDURAL_TERRAIN_TREADMILL_REQUIRED_SHELLS,
    PROCEDURAL_TERRAIN_TREADMILL_STEP_FT, ProceduralTerrainPrototypeBenchmarkKind,
    ProceduralTerrainPrototypeBenchmarkReport, ecs_benchmark_workloads,
    run_procedural_terrain_prototype_benchmark,
};
pub use dirty::{
    EcsDirtyRegion, EcsDirtyRegionLedger, EcsVoxelEditImpactMask, EcsVoxelEditPropagationOptions,
    EcsVoxelEditPropagationReport, propagate_voxel_edit,
};
pub use experiments::{
    Archetype, ArchetypeChunk, ArchetypeTable, ChunkDirtyMask, ChunkRevision, ComponentColumn,
    EntityLocation, FUN_ECS_ARCHETYPE_DEFAULT_CHUNK_CAPACITY, FUN_ECS_SPARSE_DEFAULT_MAX_PAGES,
    FUN_ECS_SPARSE_DEFAULT_PAGE_SIZE, FUN_ECS_STORAGE_EXPERIMENTS_REQUIRE_SAFE_RUST,
    FunEcsExperimentGate, FunEcsExperimentKind, MaterializedGroup, MaterializedGroupComponentMask,
    MaterializedGroupDigest, MaterializedGroupKind, MaterializedGroupMaintainer,
    MaterializedGroupRow, MaterializedGroupSourceRow, ResourceTableSnapshot, SnapshotConsumer,
    SnapshotGeneration, SnapshotLease, SnapshotRetireQueue, SparseChunkKey, SparseDenseIndex,
    SparsePage, SparsePagedPool, SparsePagedPoolConfig, StorageExperimentError, TableAdded,
    TableChanged, TableChunkQuery, TableFilter, TableQuery, TableQueryRow, WorldSnapshot,
};
pub use frame_graph::{
    ECS_CROSS_DOMAIN_FRAME_EDGE_COUNT, ECS_CROSS_DOMAIN_FRAME_EDGES, ECS_CROSS_DOMAIN_FRAME_FLOW,
    ECS_CROSS_DOMAIN_FRAME_STAGE_COUNT, EcsCrossDomainFallbackKind, EcsCrossDomainFrameEdge,
    EcsCrossDomainFrameOwner, EcsCrossDomainFrameStage, EcsCrossDomainWaitClass,
    EcsCrossDomainWaitDecision, EcsCrossDomainWaitPolicy, EcsCrossDomainWaitRejectReason,
    EcsCrossDomainWaitToken, EcsCrossDomainWaitTokenKind,
};
pub use fun_scheduler_types::{
    EcsAccess, EcsAccessTarget, EcsAgentManifestId, EcsChunkKey, EcsCommandBufferId,
    EcsComponentId, EcsEntityId, EcsExternalArtifactKey, EcsLivenessClass, EcsResourceId,
    EcsRunConditionId, EcsScheduleBuilder, EcsScheduleError, EcsSchedulePhase,
    EcsSpatialDomainKind, EcsSpatialWaitTokenKind, EcsSystemClass, EcsSystemDescriptor,
    EcsSystemExecutionContract, EcsSystemId, EcsSystemSetId, EcsVirtualResourceKey, EcsWork,
    EcsWorkKind, EcsWorldRevision, GraphInvariantError, LivenessProof, MainThreadRequirement,
    ProductRegistry, ScheduleDeadline, ScheduleDomain, ScheduleLane, SystemAccess, WaitForEdge,
    WaitForEdgeKind, WorkBlockingClass, WorkEdge, WorkGraph, WorkGraphId, WorkNode, WorkNodeId,
    WorkRequiredness, WorkSplitHint, WorkWaitToken,
};
pub use load_animation::{
    EcsLoadAnimationArtifactQueue, EcsLoadWave, EcsLoadWaveLedger, LoadAnimationArtifact,
    LoadAnimationArtifactInput, LoadAnimationBuildOptions, LoadAnimationBuildReport,
    LoadAnimationState, LoadAnimationStyle, build_load_animation_records,
    load_animation_artifact_record, load_animation_handoff_queue_from_commands,
    publish_load_animation_handoffs,
};
pub use page_table::{EcsPageResidencyTable, EcsSpatialPageTable};
pub use procedural::{
    EcsProceduralRecipeRef, PROCEDURAL_TERRAIN_DESC_SCHEMA_VERSION,
    PROCEDURAL_TERRAIN_GENERATOR_VERSION_V1, PageHeightRelation, ProceduralBiomeDesc,
    ProceduralFeatureDesc, ProceduralGenerationBudget, ProceduralMaterialDesc,
    ProceduralTerrainGeneratorDesc, ProceduralTerrainPageRecipe, ProceduralTerrainShapeDesc,
    fbm2_q16, hash2, hash3, value_noise2_q16, value_noise3_q16,
};
pub use residency::{
    EcsPageFailureCode, EcsPageResidencyMap, EcsPageResidencyRecord, EcsPageResidencyState,
};
pub use runtime::{
    EcsChunkExecutionContext, EcsCommandApplyContext, EcsNodeContext, EcsNodeError,
    EcsNodeExecutionMode, EcsNodeMetrics, EcsNodeOutcome, EcsNodeRunner, EcsSpatialFrameRunReport,
    EcsSpatialWorldDigest, FUN_FRAME_STAGE_COUNT, FUN_FRAME_STAGES, FunEcsRuntimeWork,
    FunEcsScheduleWorkGraph, FunEcsSchedulerBridgeReport, FunFrameBudgetPressure,
    FunFrameCompileError, FunFrameCompiler, FunFrameContext, FunFrameDigest, FunFrameExecutionMode,
    FunFrameFallbackAvailability, FunFrameFallbackKind, FunFrameFallbackPlan, FunFrameGraph,
    FunFrameImportedGraph, FunFrameIntent, FunFrameReport, FunFrameResourceReadPlan,
    FunFrameResourceSource, FunFrameSchedule, FunFrameStage, FunFrameStageDeclaration,
    FunFrameStageNode, FunFrameSubsystemReadiness, FunFrameWaitPlan, FunFrameWaitTokenKind,
    compile_spatial_schedule_work_graph, compile_spatial_schedule_work_graph_at_revision,
    scheduler_bridge_report,
};
pub use schedule::{
    ECS_PROCEDURAL_GENERATION_ESTIMATED_VOXELS_PER_PAGE, ECS_SPATIAL_ARTIFACT_BUILD_CONSUMER_COUNT,
    ECS_SPATIAL_ARTIFACT_BUILD_CONSUMERS, ECS_SPATIAL_COMPILED_SCHEDULE_FRAME_ORDER,
    ECS_SPATIAL_COMPILED_SCHEDULE_NODE_COUNT, ECS_SPATIAL_DEFAULT_DECODE_CHUNKS,
    ECS_SPATIAL_MAX_COMPILE_CHUNKS, ECS_SPATIAL_MAX_COMPILED_CHUNKS,
    ECS_SPATIAL_SCHEDULE_FRAME_ORDER, ECS_SPATIAL_SCHEDULE_SET_COUNT, EcsSpatialAccessPlan,
    EcsSpatialBarrierPlan, EcsSpatialChunkPlan, EcsSpatialCommandBarrierKind,
    EcsSpatialCompiledScheduleNode, EcsSpatialCompiledScheduleNodeKind,
    EcsSpatialCompilerBarrierKind, EcsSpatialGraphDigest, EcsSpatialProductWorkGraph,
    EcsSpatialScheduleBuildInput, EcsSpatialScheduleBuildReport, EcsSpatialScheduleCompileError,
    EcsSpatialScheduleCompileOutput, EcsSpatialScheduleCompiler, EcsSpatialScheduleSet,
    EcsSpatialWorkNodeKind, EcsSpatialWorkNodeSpec, ProceduralGenerationChunk,
    ProceduralGenerationChunkSpec, ProceduralTerrainCancellationContext,
    ProceduralTerrainCancellationDecision, ProceduralTerrainSchedulerWorkClass,
    compile_spatial_schedule_graph, procedural_generation_deadline,
    procedural_generation_default_chunk_key, procedural_priority_requiredness,
    procedural_terrain_artifact_deadline, procedural_terrain_artifact_requiredness,
    procedural_terrain_cancellation_decision, procedural_terrain_work_deadline,
    procedural_terrain_work_requiredness, spatial_set_access, spatial_system_descriptor,
    spatial_system_id, spatial_token,
};
pub use source::{
    EcsCompressedPagePayload, EcsDecodeOverlay, EcsDecodeOverlayKind, EcsDecodeReport,
    EcsDecodeTelemetry, EcsDecodedPagePayloadKind, EcsDecodedPageQueue, EcsDecodedPageRecord,
    EcsSourceAcquireQueue, EcsSourceAcquireRecord, EcsSourceAcquireReport, EcsSourceChecksum,
    EcsSourceChecksumAlgorithm, EcsSourceFailure, EcsSourcePayload, EcsSourcePayloadCodec,
    EcsSourceRequest, EcsSpatialRegionManifest, EcsSpatialSource, EcsSpatialSourceKind,
    acquire_sources, decode_pages, decode_pages_with_procedural_manifest,
    decode_pages_with_procedural_manifest_and_budget,
    decode_pages_with_procedural_manifest_and_scratch,
};
pub use spatial::{
    DebugPinReason, EcsAabbF32, EcsAabbI64, EcsBoundsUm, EcsClipLevelDesc, EcsFineOverlayDecl,
    EcsPageChannel, EcsPageChannelMask, EcsSpatialEntityKind, EcsSpatialGridDesc,
    EcsSpatialGridRegistry, EcsSpatialPageCoord, EcsSpatialPageKey, EcsSpatialRegionKey,
    EcsSpatialScalePreset, EcsSpatialValidationError, EcsSpatialVolume, EcsTerrainVoxelModel,
    FineDetailOverlayVolume, FoliageField, FoliagePlacementPolicy, FoliageRenderPolicy,
    SpatialDebugPin, SpatialStreamCamera, StormClipmapPolicy, StormVolumeField,
    StormVolumeStreamingRules, StreamCameraRole, VoxelCollisionPolicy, VoxelFoliagePolicy,
    VoxelRenderPolicy, VoxelTerrainVolume,
};
pub use storage::{DenseSlotKey, DenseSlotMap, RingBuffer};
pub use streaming::{
    EcsActiveStreamCameraSnapshot, EcsCameraTransformSample, EcsStreamCamera, EcsStreamInterest,
    EcsStreamInterestKind, EcsStreamInterestTable, EcsStreamPriority, EcsStreamRequest,
    EcsStreamRequestDiffResult, EcsStreamRequestQueue, EcsStreamSource, EcsStreamSourceDescriptor,
    EcsStreamWaveLedger, EcsStreamWaveRecord, EcsStreamingHole, EcsStreamingSourceSnapshot,
    EcsViewFrustum, IVec3, StreamWaveReason, build_interest, chebyshev_shell_offsets,
    diff_requests, offset_page, plan_stream_wave, sense_sources, terrain_page_for_world_ft,
};
pub use terrain::{
    DeterministicMathMode, ECS_PROCEDURAL_TERRAIN_DEFAULT_REGION_EDGE_PAGES,
    ECS_PROCEDURAL_TERRAIN_GENERATOR_VERSION_V1, ECS_PROCEDURAL_TERRAIN_SCHEMA_VERSION,
    EcsBiomeRecipe, EcsProceduralTerrainSource, EcsProceduralWorldManifest,
    EcsTerrainGeneratorVersion, ProceduralFeature, ProceduralFeatureMask, ProceduralGeneratedPage,
    ProceduralGenerationScratch, ProceduralPageDigest, ProceduralPageDigestMismatchReport,
    ProceduralPageDigestProbe, ProceduralPageDigestSample, ProceduralSurfaceFace,
    ProceduralTerrainDeltaLayerHeader, ProceduralWorldAuthorityPolicy, ProceduralWorldHandshake,
    ProceduralWorldHandshakeClientExpectation, ProceduralWorldSyncManifest, WorldOriginPolicy,
    estimate_page_height_relation, generate_procedural_generated_page,
    generate_procedural_generated_page_with_scratch, generate_procedural_page_digest,
    generate_procedural_terrain_page, generate_procedural_terrain_page_with_scratch,
    initial_region_seed_table_digest, page_seed, region_seed, terrain_height_ft,
};
pub use voxel::{
    EcsFineOverlayLink, EcsFineOverlayOverride, EcsFineOverlayOverrideMask, MaterialPalettePolicy,
    ProceduralGeneratedPageClass, VOXEL_BRICK_EDGE_CELLS, VOXEL_BRICK_FOOT_CELL_COUNT,
    VOXEL_BRICK_OCCUPANCY_WORDS, VOXEL_CELL_EDGE_UM, VOXEL_CLUSTER_EDGE_CELLS,
    VOXEL_CLUSTER_SUMMARIES_PER_BRICK, VOXEL_CLUSTERS_PER_BRICK_AXIS,
    VOXEL_MATERIAL_PALETTE_CAPACITY, VOXEL_MEGAREGION_EDGE_CELLS, VOXEL_MEGAREGION_EDGE_REGIONS,
    VOXEL_REGION_EDGE_BRICKS, VOXEL_REGION_EDGE_CELLS, VoxelBrickPayload, VoxelClusterSummary,
    VoxelEditPolicy, VoxelGridDesc, VoxelMaterialPalette, VoxelOccupancyStorage,
    VoxelOccupancyStorageKind, VoxelPagePayloadKind,
};

pub const FUN_ECS_SCHEMA_VERSION: u16 = 1;
pub const FUN_ECS_PACKAGE_NAME: &str = "fun-ecs";
pub const FUN_ECS_CRATE_NAME: &str = "fun_ecs";

pub const FUN_DEFAULT_TERRAIN_VOXEL_EDGE_UM: u32 = 304_800;
pub const FUN_INCH_VOXEL_EDGE_UM: u32 = 25_400;
pub const ECS_SPATIAL_MAX_GRIDS: usize = 1_024;
pub const ECS_SPATIAL_MAX_PAGE_RECORDS: usize = 1_048_576;
pub const ECS_SPATIAL_MAX_DIRTY_REGIONS: usize = 16_384;
pub const ECS_SPATIAL_MAX_STREAM_REQUESTS: usize = 32_768;
pub const ECS_SPATIAL_MAX_STREAM_INTERESTS: usize = 65_536;
pub const ECS_SPATIAL_MAX_STREAM_WAVES: usize = 256;
pub const ECS_SPATIAL_MAX_SOURCE_QUEUE_ROWS: usize = 32_768;
pub const ECS_SPATIAL_MAX_SOURCE_PAYLOAD_BYTES: usize = 1_048_576;
pub const ECS_SPATIAL_MAX_DECODED_PAGE_ROWS: usize = 32_768;
pub const ECS_SPATIAL_MAX_DECODE_OVERLAYS: usize = 16_384;
pub const ECS_SPATIAL_MAX_DERIVED_ARTIFACTS: usize = 1_048_576;
pub const ECS_SPATIAL_MAX_HANDOFF_ROWS: usize = 65_536;
pub const ECS_SPATIAL_MAX_LOAD_ANIMATION_ARTIFACTS: usize = 65_536;

macro_rules! ecs_id {
    ($name:ident, $raw:ty) => {
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub $raw);

        impl $name {
            pub const INVALID: Self = Self(0);

            #[must_use]
            pub const fn new(value: $raw) -> Self {
                Self(value)
            }

            #[must_use]
            pub const fn get(self) -> $raw {
                self.0
            }

            #[must_use]
            pub const fn is_valid(self) -> bool {
                self.0 != 0
            }
        }

        impl crate::DenseSlotKey for $name {
            fn is_valid(self) -> bool {
                self.is_valid()
            }
        }
    };
}

ecs_id!(EcsSpatialGridId, u64);
ecs_id!(EcsSpatialPageId, u64);
ecs_id!(EcsSpatialVolumeId, u64);
ecs_id!(EcsSpatialSourceId, u64);
ecs_id!(EcsSourceRequestId, u64);
ecs_id!(EcsStreamCameraId, u32);
ecs_id!(EcsStreamSourceId, u32);
ecs_id!(EcsStreamWaveId, u64);
ecs_id!(EcsStreamRequestId, u64);
ecs_id!(EcsDerivedArtifactId, u64);
ecs_id!(EcsAuthoringCommandId, u64);
ecs_id!(EcsHandoffQueueId, u32);
ecs_id!(EcsAssetId, u64);
ecs_id!(FixedStepId, u64);
ecs_id!(BiomeSourceId, u64);
ecs_id!(ProceduralTerrainProfileId, u64);
ecs_id!(FoliageSpeciesPaletteId, u64);
ecs_id!(VoxelTerrainId, u64);
ecs_id!(TerrainMaterialId, u32);
ecs_id!(TerrainMaterialPaletteId, u32);
ecs_id!(WeatherProfileId, u32);
ecs_id!(NetworkPlayerId, u64);

#[cfg(test)]
mod tests {
    use bevy_ecs::world::World;

    use super::*;

    fn bounds() -> EcsBoundsUm {
        EcsBoundsUm::new([0, 0, 0], [304_800 * 16, 304_800 * 16, 304_800 * 4])
    }

    fn terrain_page(channel: EcsPageChannel) -> EcsSpatialPageKey {
        EcsSpatialPageKey::new(
            EcsSpatialDomainKind::Terrain,
            EcsSpatialGridId::new(1),
            0,
            4,
            1,
            8,
            channel,
        )
    }

    // Doctrine gates, not unit trivia: no page-per-entity design; high-level
    // spatial controls may be entities; hot page state lives in dense resources;
    // mutation is command-buffered; handoffs are typed; optional Lux, foliage,
    // and refinement work cannot gate renderer present; physics fixed step can
    // use conservative fallback; cross-domain tokens map to scheduler wait
    // tokens.
    #[test]
    fn product_default_terrain_voxel_is_one_foot() {
        let model = EcsTerrainVoxelModel::default();
        assert_eq!(
            model.primary_cell_edge_um(),
            FUN_DEFAULT_TERRAIN_VOXEL_EDGE_UM
        );
        assert_eq!(model.validate_global_default(), Ok(()));
        assert_eq!(
            EcsTerrainVoxelModel {
                primary_scale: EcsSpatialScalePreset::InchHeroPatch,
                fine_overlay_page_cap: 1024,
            }
            .validate_global_default(),
            Err(EcsSpatialValidationError::FineScaleCannotBeGlobal)
        );
    }

    #[test]
    fn foot_scale_grid_desc_uses_generic_page_channels() {
        let grid = EcsSpatialGridDesc::terrain_foot_default();
        assert_eq!(grid.domain, EcsSpatialDomainKind::Terrain);
        assert_eq!(grid.unit_edge_um, FUN_DEFAULT_TERRAIN_VOXEL_EDGE_UM);
        assert_eq!(grid.cell_edge_units, 1);
        assert_eq!(grid.cluster_edge_cells, 8);
        assert_eq!(grid.page_edge_cells, 32);
        assert_eq!(grid.region_edge_pages, 8);
        assert_eq!(grid.validate(), Ok(()));
        assert!(grid.channel_mask.contains(EcsPageChannel::Occupancy));
        assert!(grid.channel_mask.contains(EcsPageChannel::Surface));
        assert!(!grid.channel_mask.contains(EcsPageChannel::FineOverlay));
        assert_eq!(grid.clip_levels[0].level, 0);
    }

    #[test]
    fn page_key_is_domain_grid_level_xyz_and_channel_specific() {
        let occupancy = terrain_page(EcsPageChannel::Occupancy);
        let surface = terrain_page(EcsPageChannel::Surface);
        assert_ne!(occupancy, surface);
        assert_ne!(occupancy.chunk_key(), surface.chunk_key());
        assert_eq!(occupancy.domain, EcsSpatialDomainKind::Terrain);
        assert_eq!(occupancy.grid_id, EcsSpatialGridId::new(1));
    }

    #[test]
    fn inch_scale_overlay_must_be_bounded() {
        let overlay = EcsFineOverlayDecl::inch_hero_patch(bounds(), 128);
        assert_eq!(overlay.validate(), Ok(()));

        let unbounded = EcsFineOverlayDecl::inch_hero_patch(EcsBoundsUm::default(), 128);
        assert_eq!(
            unbounded.validate(),
            Err(EcsSpatialValidationError::FineOverlayMustBeBounded)
        );

        let not_fine = EcsFineOverlayDecl {
            scale: EcsSpatialScalePreset::GameplayFoot,
            bounds: bounds(),
            max_pages: 128,
        };
        assert_eq!(
            not_fine.validate(),
            Err(EcsSpatialValidationError::OverlayScaleNotFine)
        );
    }

    #[test]
    fn hot_pages_live_in_resource_soa_not_page_entities() {
        let mut world = World::new();
        world.init_resource::<EcsSpatialPageTable>();
        let mut record = EcsPageResidencyRecord::new(
            terrain_page(EcsPageChannel::Occupancy),
            EcsStreamPriority::new(EcsStreamInterestKind::VisibleNear, 1, 0),
            1,
        );
        record.state = EcsPageResidencyState::Requested;
        record.last_requested_frame = 42;
        world
            .resource_mut::<EcsSpatialPageTable>()
            .push(record)
            .expect("page table admits first page");

        let table = world.resource::<EcsSpatialPageTable>();
        assert_eq!(table.len(), 1);
        assert!(table.is_consistent());
        assert_eq!(table.get_by_key(record.key), Some(&record));
        assert_eq!(table.record(0), Some(record));

        for kind in EcsSpatialEntityKind::all() {
            assert_ne!(kind.label(), "page");
            assert_ne!(kind.label(), "brick");
            assert_ne!(kind.label(), "foliage_instance");
        }
    }

    #[test]
    fn residency_lifecycle_uses_external_not_gpu() {
        let states = [
            EcsPageResidencyState::Cold,
            EcsPageResidencyState::Requested,
            EcsPageResidencyState::SourceQueued,
            EcsPageResidencyState::SourceLoading,
            EcsPageResidencyState::SourceReady,
            EcsPageResidencyState::Decoding,
            EcsPageResidencyState::CpuDecoded,
            EcsPageResidencyState::DerivedBuilding,
            EcsPageResidencyState::HandoffQueued,
            EcsPageResidencyState::ExternalPublishing,
            EcsPageResidencyState::ExternalResidentCoarse,
            EcsPageResidencyState::ExternalResidentFine,
            EcsPageResidencyState::FullyReady,
        ];
        assert!(states.contains(&EcsPageResidencyState::ExternalResidentFine));
        assert!(EcsPageResidencyState::ExternalResidentCoarse.is_external_resident());
        for state in states {
            assert!(!state.label().contains("gpu"));
        }
    }

    #[test]
    fn high_level_spatial_controls_are_ecs_entities() {
        let mut world = World::new();
        let volume = EcsSpatialVolume::terrain(EcsSpatialVolumeId::new(5), bounds());
        let camera = EcsStreamCamera {
            id: EcsStreamCameraId::new(2),
            chunk_key: terrain_page(EcsPageChannel::Occupancy).chunk_key(),
            radius_pages: 16,
            max_requests_per_frame: 64,
        };

        let entity = world.spawn((volume, camera)).id();
        assert_eq!(world.get::<EcsSpatialVolume>(entity), Some(&volume));
        assert_eq!(world.get::<EcsStreamCamera>(entity), Some(&camera));
    }

    #[test]
    fn derived_artifacts_feed_consumers_without_render_pages() {
        let source_page = terrain_page(EcsPageChannel::Surface);
        let mut artifacts = EcsDerivedArtifactRegistry::default();
        let rows = [
            (
                EcsDerivedArtifactId::new(1),
                EcsDerivedArtifactKind::TerrainSurfacePackets,
                EcsArtifactConsumer::Renderer,
                EcsCrossDomainHandoffKind::RenderArtifact,
                WorkRequiredness::Required,
            ),
            (
                EcsDerivedArtifactId::new(2),
                EcsDerivedArtifactKind::VirtualShadowInvalidation,
                EcsArtifactConsumer::Lux,
                EcsCrossDomainHandoffKind::LuxLightingIntent,
                WorkRequiredness::Optional,
            ),
            (
                EcsDerivedArtifactId::new(3),
                EcsDerivedArtifactKind::PhysicsCollisionProxy,
                EcsArtifactConsumer::AvisPhysics,
                EcsCrossDomainHandoffKind::PhysicsCookRequest,
                WorkRequiredness::Required,
            ),
            (
                EcsDerivedArtifactId::new(4),
                EcsDerivedArtifactKind::NetworkDeltaRows,
                EcsArtifactConsumer::ThunderNetwork,
                EcsCrossDomainHandoffKind::NetworkRelevanceRow,
                WorkRequiredness::Required,
            ),
        ];
        let mut handoffs = EcsCrossDomainHandoffQueues::default();

        for (artifact_id, artifact_kind, consumer, handoff_kind, requiredness) in rows {
            artifacts
                .push(EcsDerivedArtifactRecord {
                    artifact_id,
                    source_page,
                    kind: artifact_kind,
                    source_epoch: 9,
                    source_digest: derived_artifact_source_digest(source_page, 9, 10),
                    artifact_epoch: 10,
                    state: EcsArtifactState::Ready,
                    requiredness,
                    consumer,
                })
                .expect("artifact registry admits typed row");
            handoffs
                .push(EcsCrossDomainHandoff::new(
                    EcsHandoffQueueId::new(1),
                    handoff_kind,
                    consumer,
                    artifact_id,
                    source_page,
                    10,
                ))
                .expect("handoff queue admits typed row");
        }

        assert!(artifacts.is_consistent());
        assert_eq!(artifacts.len(), 4);
        assert_eq!(handoffs.rows.len(), 4);
        assert_eq!(
            artifacts
                .get(EcsDerivedArtifactId::new(1))
                .map(|record| record.consumer),
            Some(EcsArtifactConsumer::Renderer)
        );
        assert_eq!(
            artifacts
                .get(EcsDerivedArtifactId::new(4))
                .map(|record| record.consumer),
            Some(EcsArtifactConsumer::ThunderNetwork)
        );
        assert_eq!(
            artifacts
                .get(EcsDerivedArtifactId::new(2))
                .map(|record| record.requiredness),
            Some(WorkRequiredness::Optional)
        );
        assert_eq!(
            artifacts
                .get(EcsDerivedArtifactId::new(4))
                .map(|record| record.kind),
            Some(EcsDerivedArtifactKind::NetworkDeltaRows)
        );
    }

    #[test]
    fn grid_registry_validates_bounds_and_channels() {
        let mut registry = EcsSpatialGridRegistry::default();
        registry
            .push(
                EcsSpatialGridId::new(1),
                EcsSpatialGridDesc::terrain_foot_default(),
            )
            .expect("default terrain grid is valid");
        assert_eq!(registry.len(), 1);

        let mut invalid = EcsSpatialGridDesc::terrain_foot_default();
        invalid.channel_mask = EcsPageChannelMask::EMPTY;
        let err = registry
            .push(EcsSpatialGridId::new(2), invalid)
            .expect_err("empty channel mask rejects");
        assert_eq!(err, EcsSpatialValidationError::GridHasNoChannels);
    }

    #[test]
    fn foliage_pages_use_generic_domain_channels_not_leaf_entities() {
        let grid = EcsSpatialGridDesc::foliage_default();
        assert_eq!(grid.domain, EcsSpatialDomainKind::Foliage);
        assert_eq!(grid.validate(), Ok(()));
        assert!(grid.channel_mask.contains(EcsPageChannel::FoliageSeed));
        assert!(grid.channel_mask.contains(EcsPageChannel::FoliageGeometry));
        assert!(grid.channel_mask.contains(EcsPageChannel::CanopyOpacity));
        assert!(grid.channel_mask.contains(EcsPageChannel::VirtualShadow));
        assert!(!grid.channel_mask.contains(EcsPageChannel::Occupancy));

        let field = FoliageField {
            source_grid: EcsSpatialGridId::new(1),
            biome_source: BiomeSourceId::new(2),
            species_palette: FoliageSpeciesPaletteId::new(3),
            placement_policy: FoliagePlacementPolicy::TerrainMaterialDriven,
            render_policy: FoliageRenderPolicy::NearGeometryMidCardsFarCanopy,
        };
        assert_eq!(field.source_grid, EcsSpatialGridId::new(1));
        for kind in EcsSpatialEntityKind::all() {
            assert_ne!(kind.label(), "leaf");
            assert_ne!(kind.label(), "foliage_leaf");
            assert_ne!(kind.label(), "foliage_instance");
        }
    }

    #[test]
    fn storm_volume_pages_are_weather_domain_and_independent_from_terrain() {
        let grid = EcsSpatialGridDesc::storm_volume_default();
        assert_eq!(grid.domain, EcsSpatialDomainKind::WeatherVolume);
        assert_eq!(grid.validate(), Ok(()));
        assert!(
            grid.channel_mask
                .contains(EcsPageChannel::WeatherExtinction)
        );
        assert!(grid.channel_mask.contains(EcsPageChannel::Radiance));
        assert!(grid.channel_mask.contains(EcsPageChannel::Debug));
        assert!(!grid.channel_mask.contains(EcsPageChannel::Occupancy));

        let storm = StormVolumeField {
            profile: WeatherProfileId::new(9),
            clipmap_levels: 4,
            near_resolution: [64, 32, 64],
            max_distance_ft: 4_096,
            temporal_reprojection: true,
        };
        assert!(storm.is_streaming_independent_from_terrain());
        let rules = StormVolumeStreamingRules::PRODUCT_DEFAULT;
        assert!(rules.contract_holds());
        assert!(!rules.required_for_terrain_correctness);
        assert!(!rules.evicted_with_terrain_pages);
    }

    #[test]
    fn requested_high_level_spatial_entities_are_components() {
        let mut world = World::new();
        let camera = SpatialStreamCamera {
            role: StreamCameraRole::MainView,
            enabled: true,
            priority: 255,
            required_shells: 3,
            desired_shells: 9,
            velocity_lookahead_s: 0.25,
        };
        let terrain = VoxelTerrainVolume {
            grid: EcsSpatialGridId::new(1),
            terrain_id: VoxelTerrainId::new(7),
            default_material: TerrainMaterialId::new(2),
            source: EcsSpatialSourceId::new(11),
            collision_policy: VoxelCollisionPolicy::NarrowBandSdf,
            render_policy: VoxelRenderPolicy::SurfacePackets,
            foliage_policy: VoxelFoliagePolicy::ClustersAndCanopy,
        };
        let overlay = FineDetailOverlayVolume {
            parent_grid: EcsSpatialGridId::new(1),
            scale: EcsSpatialScalePreset::InchHeroPatch,
            bounds: bounds(),
            channel_mask: EcsPageChannelMask::from_channel(EcsPageChannel::FineOverlay),
        };
        let foliage = FoliageField {
            source_grid: EcsSpatialGridId::new(1),
            biome_source: BiomeSourceId::new(4),
            species_palette: FoliageSpeciesPaletteId::new(5),
            placement_policy: FoliagePlacementPolicy::BiomeDensity,
            render_policy: FoliageRenderPolicy::NearGeometryMidCardsFarCanopy,
        };
        let storm = StormVolumeField {
            profile: WeatherProfileId::new(3),
            clipmap_levels: 4,
            near_resolution: [64, 32, 64],
            max_distance_ft: 2_048,
            temporal_reprojection: true,
        };
        let pin = SpatialDebugPin {
            key: terrain_page(EcsPageChannel::Surface),
            reason: DebugPinReason::ResidencyDebug,
        };

        let entity = world
            .spawn((camera, terrain, overlay, foliage, storm, pin))
            .id();
        assert_eq!(world.get::<SpatialStreamCamera>(entity), Some(&camera));
        assert_eq!(world.get::<VoxelTerrainVolume>(entity), Some(&terrain));
        assert_eq!(world.get::<FineDetailOverlayVolume>(entity), Some(&overlay));
        assert_eq!(world.get::<FoliageField>(entity), Some(&foliage));
        assert_eq!(world.get::<StormVolumeField>(entity), Some(&storm));
        assert_eq!(world.get::<SpatialDebugPin>(entity), Some(&pin));
    }

    #[test]
    fn dense_resources_match_spatial_hot_state_contract() {
        let mut grids = EcsSpatialGridRegistry::default();
        grids
            .push(
                EcsSpatialGridId::new(1),
                EcsSpatialGridDesc::terrain_foot_default(),
            )
            .expect("grid insert");
        assert!(grids.get(EcsSpatialGridId::new(1)).is_some());

        let mut pages = EcsPageResidencyTable::default();
        let page_id = EcsSpatialPageId::new(1);
        let record = EcsPageResidencyRecord::new(
            terrain_page(EcsPageChannel::Occupancy),
            EcsStreamPriority::new(EcsStreamInterestKind::CameraContainingPage, 0, 0),
            2,
        );
        pages.insert(page_id, record).expect("page insert");
        assert_eq!(pages.key_to_id.get(&record.key), Some(&page_id));
        assert_eq!(pages.get(page_id), Some(&record));
        assert_eq!(
            pages.insert(EcsSpatialPageId::new(2), record),
            Err(EcsSpatialValidationError::PageTableDuplicateKey)
        );
        assert!(pages.is_consistent());

        let mut waves = EcsStreamWaveLedger::default();
        waves.push(EcsStreamWaveRecord {
            wave_id: EcsStreamWaveId::new(1),
            origin_page: record.key,
            origin_world_ft: IVec3::new(128, 32, 256),
            camera_entity: EcsEntityId::new(9),
            created_frame: 44,
            max_shell_planned: 3,
            reason: StreamWaveReason::ColdStart,
        });
        assert_eq!(waves.active_wave, EcsStreamWaveId::new(1));
        assert_eq!(waves.waves.len(), 1);

        let mut interests = EcsStreamInterestTable::default();
        interests
            .push(EcsStreamInterest {
                camera: EcsStreamCameraId::new(1),
                camera_entity: EcsEntityId::new(9),
                page: record.key,
                kind: EcsStreamInterestKind::CameraContainingPage,
                priority: EcsStreamPriority::new(EcsStreamInterestKind::CameraContainingPage, 0, 0),
                shell: 0,
                required: true,
            })
            .expect("interest insert");
        assert_eq!(interests.interests.len(), 1);
    }

    #[test]
    fn spatial_commands_capture_structure_changes_for_apply_barrier() {
        let source_page = terrain_page(EcsPageChannel::Surface);
        let artifact = EcsDerivedArtifactRecord {
            artifact_id: EcsDerivedArtifactId::new(9),
            source_page,
            kind: EcsDerivedArtifactKind::TerrainSurfacePackets,
            source_epoch: 1,
            source_digest: derived_artifact_source_digest(source_page, 1, 2),
            artifact_epoch: 2,
            state: EcsArtifactState::Ready,
            requiredness: WorkRequiredness::Required,
            consumer: EcsArtifactConsumer::Renderer,
        };
        let renderer_handoff = RendererArtifactHandoff {
            artifact_id: artifact.artifact_id,
            source_page,
            kind: RendererArtifactHandoffKind::TerrainSurfacePackets,
            source_epoch: artifact.source_epoch,
            source_digest: artifact.source_digest,
            artifact_epoch: artifact.artifact_epoch,
            requiredness: artifact.requiredness,
            visibility_hint: RendererVisibilityHint::VisibleNear,
        };
        let lux_handoff = LuxArtifactHandoff {
            source_page,
            kind: LuxArtifactHandoffKind::RadianceClipmapDirtyRows,
            dirty_epoch: 3,
            bounds_world: EcsAabbF32::new([0.0, 0.0, 0.0], [32.0, 32.0, 32.0]),
            requiredness: WorkRequiredness::Optional,
        };
        let physics = VoxelPhysicsCookRequest {
            source_page,
            dirty_epoch: 4,
            mode: CollisionCookMode::NarrowBandSdf,
            requiredness: WorkRequiredness::Required,
            fixed_step_deadline: Some(FixedStepId::new(2)),
        };
        let mut commands = EcsSpatialCommandBuffer::default();
        commands
            .push(EcsSpatialCommand::RequestPage(source_page))
            .expect("request command");
        commands
            .push(EcsSpatialCommand::PublishArtifact(artifact))
            .expect("artifact command");
        commands
            .push(EcsSpatialCommand::PublishRendererHandoff(renderer_handoff))
            .expect("renderer command");
        commands
            .push(EcsSpatialCommand::PublishLuxHandoff(lux_handoff))
            .expect("lux command");
        commands
            .push(EcsSpatialCommand::PublishPhysicsCook(physics))
            .expect("physics command");

        assert_eq!(commands.commands.len(), 5);
        assert!(matches!(
            commands.commands[1],
            EcsSpatialCommand::PublishArtifact(_)
        ));
    }

    #[test]
    fn renderer_handoffs_use_renderer_specific_kind_visibility_and_epochs() {
        let source_page = terrain_page(EcsPageChannel::Surface);
        let rows = [
            EcsDerivedArtifactRecord {
                artifact_id: EcsDerivedArtifactId::new(31),
                source_page,
                kind: EcsDerivedArtifactKind::TerrainSurfacePackets,
                source_epoch: 8,
                source_digest: derived_artifact_source_digest(source_page, 8, 9),
                artifact_epoch: 9,
                state: EcsArtifactState::Ready,
                requiredness: WorkRequiredness::Required,
                consumer: EcsArtifactConsumer::Renderer,
            },
            EcsDerivedArtifactRecord {
                artifact_id: EcsDerivedArtifactId::new(32),
                source_page,
                kind: EcsDerivedArtifactKind::RadianceUpdateRows,
                source_epoch: 8,
                source_digest: derived_artifact_source_digest(source_page, 8, 9),
                artifact_epoch: 9,
                state: EcsArtifactState::Ready,
                requiredness: WorkRequiredness::Optional,
                consumer: EcsArtifactConsumer::Lux,
            },
        ];
        let mut commands = EcsSpatialCommandBuffer::default();
        let report =
            publish_renderer_handoffs(&rows, RendererVisibilityHint::CoarseFallback, &mut commands)
                .expect("publish renderer handoffs");

        assert_eq!(report.inspected, 2);
        assert_eq!(report.published, 1);
        assert_eq!(report.skipped_non_renderer, 1);
        let handoff = match commands.commands[0] {
            EcsSpatialCommand::PublishRendererHandoff(handoff) => handoff,
            _ => panic!("expected renderer handoff command"),
        };
        assert_eq!(
            handoff.kind,
            RendererArtifactHandoffKind::TerrainSurfacePackets
        );
        assert_eq!(handoff.source_epoch, 8);
        assert_eq!(handoff.artifact_epoch, 9);
        assert_eq!(
            handoff.visibility_hint,
            RendererVisibilityHint::CoarseFallback
        );
    }

    #[test]
    fn lux_handoffs_are_backend_neutral_dirty_rows_with_bounds() {
        let source_page = terrain_page(EcsPageChannel::Radiance);
        let rows = [
            EcsDerivedArtifactRecord {
                artifact_id: EcsDerivedArtifactId::new(41),
                source_page,
                kind: EcsDerivedArtifactKind::RadianceUpdateRows,
                source_epoch: 8,
                source_digest: derived_artifact_source_digest(source_page, 8, 13),
                artifact_epoch: 13,
                state: EcsArtifactState::Ready,
                requiredness: WorkRequiredness::Optional,
                consumer: EcsArtifactConsumer::Lux,
            },
            EcsDerivedArtifactRecord {
                artifact_id: EcsDerivedArtifactId::new(42),
                source_page,
                kind: EcsDerivedArtifactKind::TerrainSurfacePackets,
                source_epoch: 8,
                source_digest: derived_artifact_source_digest(source_page, 8, 13),
                artifact_epoch: 13,
                state: EcsArtifactState::Ready,
                requiredness: WorkRequiredness::Required,
                consumer: EcsArtifactConsumer::Renderer,
            },
        ];
        let bounds = EcsAabbF32::new([0.0, 0.0, 0.0], [32.0, 32.0, 32.0]);
        let mut commands = EcsSpatialCommandBuffer::default();
        let report = publish_lux_handoffs(&rows, bounds, &mut commands).expect("lux handoffs");

        assert_eq!(report.inspected, 2);
        assert_eq!(report.published, 1);
        assert_eq!(report.skipped_non_lux, 1);
        let handoff = match commands.commands[0] {
            EcsSpatialCommand::PublishLuxHandoff(handoff) => handoff,
            _ => panic!("expected lux handoff command"),
        };
        assert_eq!(handoff.source_page, source_page);
        assert_eq!(
            handoff.kind,
            LuxArtifactHandoffKind::RadianceClipmapDirtyRows
        );
        assert_eq!(handoff.dirty_epoch, 13);
        assert_eq!(handoff.bounds_world, bounds);
        assert_eq!(handoff.requiredness, WorkRequiredness::Optional);
    }

    #[test]
    fn voxel_edit_log_covers_required_authoring_ops() {
        let edit_bounds = EcsAabbI64::new([0, 0, 0], [32, 32, 8]);
        let material = TerrainMaterialId::new(9);
        let stamp_transform = PackedTransform::new(
            IVec3::new(8, 8, 2),
            [0, 0, 0, i16::MAX],
            [u16::MAX, u16::MAX, u16::MAX],
            EcsAabbI64::new([4, 4, 0], [12, 12, 4]),
        );
        let edits = [
            VoxelEditOp::FillAabb {
                bounds: edit_bounds,
                material,
            },
            VoxelEditOp::CarveSphere {
                center: IVec3::new(8, 8, 8),
                radius_voxels: 4,
            },
            VoxelEditOp::PaintMaterial {
                bounds: edit_bounds,
                material,
            },
            VoxelEditOp::StampSdf {
                transform: stamp_transform,
                sdf_asset: EcsAssetRef::new(EcsAssetId::new(17), 3),
                material,
            },
            VoxelEditOp::ApplyExplosion {
                center: IVec3::new(16, 16, 4),
                radius_voxels: 6,
                impulse_q: 128,
            },
            VoxelEditOp::RestoreFromSource {
                bounds: edit_bounds,
            },
        ];
        let mut log = EcsVoxelEditLog::default();
        let mut commands = EcsSpatialCommandBuffer::default();

        for (index, edit) in edits.into_iter().enumerate() {
            edit.validate().expect("valid edit op");
            log.push(EcsVoxelEditLogEntry {
                command_id: EcsAuthoringCommandId::new(index as u64 + 1),
                op: edit,
                dirty_epoch: 10 + index as u32,
            })
            .expect("edit log entry");
            commands
                .push(EcsSpatialCommand::ApplyVoxelEdit(edit))
                .expect("edit command");
        }

        assert_eq!(log.entries.len(), 6);
        assert_eq!(commands.commands.len(), 6);
        assert_eq!(
            VoxelEditOp::CarveSphere {
                center: IVec3::new(8, 8, 8),
                radius_voxels: 4,
            }
            .dirty_bounds(),
            EcsAabbI64::new([4, 4, 4], [13, 13, 13])
        );
        assert_eq!(
            VoxelEditOp::FillAabb {
                bounds: EcsAabbI64::default(),
                material,
            }
            .validate(),
            Err(EcsSpatialValidationError::InvalidEditBounds)
        );
        assert_eq!(
            VoxelEditOp::CarveSphere {
                center: IVec3::zero(),
                radius_voxels: 0,
            }
            .validate(),
            Err(EcsSpatialValidationError::InvalidEditRadius)
        );
    }

    #[test]
    fn voxel_edit_propagates_dirty_epochs_artifacts_and_physics_cooks() {
        let source_page = terrain_page(EcsPageChannel::Occupancy);
        let mut page_table = EcsPageResidencyTable::default();
        page_table
            .push(EcsPageResidencyRecord::new(
                source_page,
                EcsStreamPriority::new(EcsStreamInterestKind::CollisionCriticalNear, 0, 0),
                1,
            ))
            .expect("page table insert");
        let mut dirty_ledger = EcsDirtyRegionLedger::default();
        let mut next_artifact_id = 100;
        let mut commands = EcsSpatialCommandBuffer::default();
        let mut options = EcsVoxelEditPropagationOptions::collision_critical(
            source_page,
            22,
            FixedStepId::new(7),
        );
        options.navigation_enabled = true;
        options.audio_enabled = true;
        options.network_enabled = true;

        let report = propagate_voxel_edit(
            VoxelEditOp::FillAabb {
                bounds: EcsAabbI64::new([0, 0, 0], [32, 32, 32]),
                material: TerrainMaterialId::new(3),
            },
            options,
            &mut page_table,
            &mut dirty_ledger,
            &mut next_artifact_id,
            &mut commands,
        )
        .expect("propagate edit");

        assert_eq!(report.dirty_regions_marked, 1);
        assert_eq!(report.page_epochs_advanced, 1);
        assert_eq!(report.physics_cooks_published, 1);
        assert!(report.artifacts_marked_dirty >= 10);
        assert_eq!(dirty_ledger.epoch, 22);
        assert_eq!(
            page_table
                .get_by_key(source_page)
                .map(|record| record.dirty_epoch),
            Some(22)
        );

        let artifacts: Vec<EcsDerivedArtifactRecord> = commands
            .commands
            .iter()
            .filter_map(|command| match command {
                EcsSpatialCommand::PublishArtifact(artifact) => Some(*artifact),
                _ => None,
            })
            .collect();
        assert!(artifacts.iter().any(|artifact| {
            artifact.kind == EcsDerivedArtifactKind::TerrainSurfacePackets
                && artifact.consumer == EcsArtifactConsumer::Renderer
        }));
        assert!(artifacts.iter().any(|artifact| {
            artifact.kind == EcsDerivedArtifactKind::TerrainMaterialPage
                && artifact.consumer == EcsArtifactConsumer::Renderer
        }));
        assert!(artifacts.iter().any(|artifact| {
            artifact.kind == EcsDerivedArtifactKind::TerrainSdf
                && artifact.consumer == EcsArtifactConsumer::Lux
        }));
        assert!(artifacts.iter().any(|artifact| {
            artifact.kind == EcsDerivedArtifactKind::ShadowInvalidationRows
                && artifact.consumer == EcsArtifactConsumer::Lux
        }));
        assert!(artifacts.iter().any(|artifact| {
            artifact.kind == EcsDerivedArtifactKind::FoliageClusters
                && artifact.requiredness == WorkRequiredness::Optional
        }));
        assert!(artifacts.iter().any(|artifact| {
            artifact.kind == EcsDerivedArtifactKind::NavTile
                && artifact.consumer == EcsArtifactConsumer::Navigation
        }));
        assert!(artifacts.iter().any(|artifact| {
            artifact.kind == EcsDerivedArtifactKind::AudioOcclusionTile
                && artifact.consumer == EcsArtifactConsumer::Audio
        }));
        assert!(artifacts.iter().any(|artifact| {
            artifact.kind == EcsDerivedArtifactKind::NetworkRelevanceRows
                && artifact.consumer == EcsArtifactConsumer::ThunderNetwork
        }));

        let physics = commands.commands.iter().find_map(|command| match command {
            EcsSpatialCommand::PublishPhysicsCook(request) => Some(*request),
            _ => None,
        });
        assert_eq!(
            physics,
            Some(VoxelPhysicsCookRequest {
                source_page,
                dirty_epoch: 22,
                mode: CollisionCookMode::NarrowBandSdf,
                requiredness: WorkRequiredness::Required,
                fixed_step_deadline: Some(FixedStepId::new(7)),
            })
        );
    }

    #[test]
    fn physics_cook_requests_and_rules_stay_ecs_authoritative() {
        let source_page = terrain_page(EcsPageChannel::NarrowBandSdf);
        let artifact = EcsDerivedArtifactRecord {
            artifact_id: EcsDerivedArtifactId::new(71),
            source_page,
            kind: EcsDerivedArtifactKind::CollisionSdfProxy,
            source_epoch: 5,
            source_digest: derived_artifact_source_digest(source_page, 5, 23),
            artifact_epoch: 23,
            state: EcsArtifactState::Ready,
            requiredness: WorkRequiredness::Required,
            consumer: EcsArtifactConsumer::AvisPhysics,
        };
        let request = physics_cook_from_artifact(artifact, CollisionCookMode::NarrowBandSdf, None)
            .expect("physics request");

        assert_eq!(request.source_page, source_page);
        assert_eq!(request.dirty_epoch, 23);
        assert_eq!(request.mode, CollisionCookMode::NarrowBandSdf);
        assert_eq!(
            request.mode.conservative_proxy_before_full_cook(),
            CollisionCookMode::ConvexClusterProxy
        );
        assert!(EcsTerrainPhysicsRules::PRODUCT_DEFAULT.contract_holds());
        assert_ne!(
            EcsTerrainPhysicsRules::PRODUCT_DEFAULT.physics_source,
            EcsPhysicsSourceTruth::RendererGpuBuffers
        );
        assert!(
            EcsStreamInterestKind::CollisionCriticalNear.priority_rank()
                < EcsStreamInterestKind::FoliageNear.priority_rank()
        );
        assert_eq!(
            EcsTerrainPhysicsRules::PRODUCT_DEFAULT.far_collision_requiredness(),
            WorkRequiredness::Optional
        );
        assert_eq!(
            EcsTerrainPhysicsRules {
                gameplay_pins_far_collision: true,
                ..EcsTerrainPhysicsRules::PRODUCT_DEFAULT
            }
            .far_collision_requiredness(),
            WorkRequiredness::Required
        );
    }

    #[test]
    fn foliage_derived_artifacts_cover_near_mid_far_outputs() {
        assert_eq!(
            EcsDerivedArtifactKind::FoliageTrunkBranchVirtualGeometry.foliage_lod_band(),
            Some(FoliageArtifactLodBand::Near)
        );
        assert_eq!(
            EcsDerivedArtifactKind::FoliageGrassBrushInstanceClusters.foliage_lod_band(),
            Some(FoliageArtifactLodBand::Near)
        );
        assert_eq!(
            EcsDerivedArtifactKind::FoliageCollisionLargeObjectProxy.foliage_lod_band(),
            Some(FoliageArtifactLodBand::Near)
        );
        assert_eq!(
            EcsDerivedArtifactKind::FoliageCardsImpostors.foliage_lod_band(),
            Some(FoliageArtifactLodBand::Mid)
        );
        assert_eq!(
            EcsDerivedArtifactKind::FoliageClusteredAnimation.foliage_lod_band(),
            Some(FoliageArtifactLodBand::Mid)
        );
        assert_eq!(
            EcsDerivedArtifactKind::CanopyOpacity.foliage_lod_band(),
            Some(FoliageArtifactLodBand::Mid)
        );
        assert_eq!(
            EcsDerivedArtifactKind::FoliageCanopyTransmittance.foliage_lod_band(),
            Some(FoliageArtifactLodBand::Far)
        );
        assert_eq!(
            EcsDerivedArtifactKind::FoliageBiomeTint.foliage_lod_band(),
            Some(FoliageArtifactLodBand::Far)
        );
        assert_eq!(
            EcsDerivedArtifactKind::FoliageHorizonImpostorField.foliage_lod_band(),
            Some(FoliageArtifactLodBand::Far)
        );
        assert_eq!(
            EcsDerivedArtifactKind::FoliageSdfOpacityShadows.foliage_lod_band(),
            Some(FoliageArtifactLodBand::Far)
        );

        assert_eq!(
            RendererArtifactHandoffKind::from_derived(
                EcsDerivedArtifactKind::FoliageTrunkBranchVirtualGeometry,
            ),
            Some(RendererArtifactHandoffKind::FoliageVirtualGeometry)
        );
        assert_eq!(
            LuxArtifactHandoffKind::from_derived(
                EcsDerivedArtifactKind::FoliageCanopyTransmittance
            ),
            Some(LuxArtifactHandoffKind::CanopyOpacityForTransmittance)
        );
        assert_eq!(
            LuxArtifactHandoffKind::from_derived(EcsDerivedArtifactKind::FoliageSdfOpacityShadows),
            Some(LuxArtifactHandoffKind::VoxelShadowInvalidationRows)
        );
        let foliage_collision = EcsDerivedArtifactRecord {
            artifact_id: EcsDerivedArtifactId::new(81),
            source_page: EcsSpatialPageKey::new(
                EcsSpatialDomainKind::Foliage,
                EcsSpatialGridId::new(1),
                0,
                0,
                0,
                0,
                EcsPageChannel::FoliageGeometry,
            ),
            kind: EcsDerivedArtifactKind::FoliageCollisionLargeObjectProxy,
            source_epoch: 4,
            source_digest: derived_artifact_source_digest(
                EcsSpatialPageKey::new(
                    EcsSpatialDomainKind::Foliage,
                    EcsSpatialGridId::new(1),
                    0,
                    0,
                    0,
                    0,
                    EcsPageChannel::FoliageGeometry,
                ),
                4,
                12,
            ),
            artifact_epoch: 12,
            state: EcsArtifactState::Ready,
            requiredness: WorkRequiredness::Optional,
            consumer: EcsArtifactConsumer::AvisPhysics,
        };
        assert_eq!(
            physics_cook_from_artifact(
                foliage_collision,
                CollisionCookMode::ConvexClusterProxy,
                None
            )
            .map(|request| request.mode),
            Some(CollisionCookMode::ConvexClusterProxy)
        );
    }

    #[test]
    fn spatial_schedule_sets_have_requested_frame_order() {
        let order = EcsSpatialScheduleSet::all();
        assert_eq!(order.len(), ECS_SPATIAL_SCHEDULE_SET_COUNT);
        assert_eq!(
            order,
            &[
                EcsSpatialScheduleSet::SenseSources,
                EcsSpatialScheduleSet::BuildInterest,
                EcsSpatialScheduleSet::PlanStreamWave,
                EcsSpatialScheduleSet::DiffRequests,
                EcsSpatialScheduleSet::ApplyStreamCommands,
                EcsSpatialScheduleSet::AcquireSources,
                EcsSpatialScheduleSet::DecodePages,
                EcsSpatialScheduleSet::BuildDerivedArtifacts,
                EcsSpatialScheduleSet::ApplyArtifactCommands,
                EcsSpatialScheduleSet::PublishRendererHandoffs,
                EcsSpatialScheduleSet::PublishLuxHandoffs,
                EcsSpatialScheduleSet::PublishPhysicsHandoffs,
                EcsSpatialScheduleSet::ApplyHandoffCommands,
                EcsSpatialScheduleSet::EvictColdPages,
                EcsSpatialScheduleSet::FlushDiagnostics,
            ]
        );
        for pair in order.windows(2) {
            assert_eq!(pair[0].next(), Some(pair[1]));
            assert!(pair[0].ordinal() < pair[1].ordinal());
        }
        assert_eq!(EcsSpatialScheduleSet::FlushDiagnostics.next(), None);
    }

    #[test]
    fn spatial_schedule_sets_map_to_scheduler_domains() {
        let sense = EcsSpatialScheduleSet::SenseSources.execution_contract();
        assert_eq!(sense.domain, ScheduleDomain::FunEcs);
        assert_eq!(sense.deadline, ScheduleDeadline::Frame);

        let plan = EcsSpatialScheduleSet::PlanStreamWave.execution_contract();
        assert_eq!(plan.domain, ScheduleDomain::RendererPageScheduler);
        assert_eq!(plan.deadline, ScheduleDeadline::Frame);

        let diff = EcsSpatialScheduleSet::DiffRequests.execution_contract();
        assert_eq!(diff.domain, ScheduleDomain::RendererPageScheduler);
        assert_eq!(diff.deadline, ScheduleDeadline::Frame);

        let apply = EcsSpatialScheduleSet::ApplyStreamCommands.execution_contract();
        assert_eq!(apply.lane, ScheduleLane::EcsCommandBarrier);
        assert_eq!(apply.deadline, ScheduleDeadline::Frame);
        assert_eq!(
            EcsSpatialScheduleSet::ApplyStreamCommands.work_kind(),
            EcsWorkKind::ApplyCommands
        );

        let acquire = EcsSpatialScheduleSet::AcquireSources.execution_contract();
        assert_eq!(acquire.domain, ScheduleDomain::Blocking);
        assert_eq!(acquire.lane, ScheduleLane::Blocking);
        assert_eq!(acquire.deadline, ScheduleDeadline::Stream);
        assert_eq!(acquire.blocking_class, WorkBlockingClass::BlockingPool);
        assert_eq!(acquire.main_thread, MainThreadRequirement::NotMainThread);

        let decode = EcsSpatialScheduleSet::DecodePages.execution_contract();
        assert_eq!(decode.domain, ScheduleDomain::RendererPageScheduler);
        assert_eq!(decode.deadline, ScheduleDeadline::Stream);
        assert_eq!(decode.split, WorkSplitHint::Splittable);
        assert!(EcsSpatialScheduleSet::DecodePages.is_chunk_parallel());

        let evict = EcsSpatialScheduleSet::EvictColdPages.execution_contract();
        assert_eq!(evict.deadline, ScheduleDeadline::IdleWindow);
        assert_eq!(evict.requiredness, WorkRequiredness::Optional);

        let diagnostics = EcsSpatialScheduleSet::FlushDiagnostics.execution_contract();
        assert_eq!(diagnostics.domain, ScheduleDomain::Telemetry);
        assert_eq!(diagnostics.deadline, ScheduleDeadline::IdleWindow);
        assert_eq!(diagnostics.requiredness, WorkRequiredness::Optional);
    }

    #[test]
    fn spatial_schedule_artifact_and_handoff_mappings_are_domain_specific() {
        let renderer =
            EcsSpatialScheduleSet::build_derived_artifact_execution(EcsArtifactConsumer::Renderer);
        assert_eq!(renderer.domain, ScheduleDomain::Renderer);
        assert_eq!(renderer.split, WorkSplitHint::Splittable);

        let lux = EcsSpatialScheduleSet::build_derived_artifact_execution(EcsArtifactConsumer::Lux);
        assert_eq!(lux.domain, ScheduleDomain::RendererLux);

        let physics = EcsSpatialScheduleSet::build_derived_artifact_execution(
            EcsArtifactConsumer::AvisPhysics,
        );
        assert_eq!(physics.domain, ScheduleDomain::AvisPhysics);

        let network = EcsSpatialScheduleSet::build_derived_artifact_execution(
            EcsArtifactConsumer::ThunderNetwork,
        );
        assert_eq!(network.domain, ScheduleDomain::ThunderNetwork);

        let background_renderer = EcsSpatialScheduleSet::renderer_handoff_execution(false);
        assert_eq!(background_renderer.domain, ScheduleDomain::Renderer);
        assert_eq!(background_renderer.deadline, ScheduleDeadline::Stream);

        let visible_renderer = EcsSpatialScheduleSet::renderer_handoff_execution(true);
        assert_eq!(visible_renderer.deadline, ScheduleDeadline::Frame);

        let noncritical_lux = EcsSpatialScheduleSet::lux_handoff_execution(false);
        assert_eq!(noncritical_lux.domain, ScheduleDomain::RendererLux);
        assert_eq!(noncritical_lux.deadline, ScheduleDeadline::Stream);

        let fixed_physics = EcsSpatialScheduleSet::physics_handoff_execution(true);
        assert_eq!(fixed_physics.domain, ScheduleDomain::AvisPhysics);
        assert_eq!(fixed_physics.deadline, ScheduleDeadline::FixedStep);

        let stream_physics = EcsSpatialScheduleSet::physics_handoff_execution(false);
        assert_eq!(stream_physics.deadline, ScheduleDeadline::Stream);
    }

    #[test]
    fn spatial_schedule_graph_compiles_internal_apply_barriers_without_public_set_churn() {
        let public_order = EcsSpatialScheduleSet::all();
        let compiled = compile_spatial_schedule_graph();

        assert_eq!(public_order.len(), ECS_SPATIAL_SCHEDULE_SET_COUNT);
        assert_eq!(compiled.len(), ECS_SPATIAL_COMPILED_SCHEDULE_NODE_COUNT);
        assert_eq!(
            compiled[4].kind,
            EcsSpatialCompiledScheduleNodeKind::Barrier(
                EcsSpatialCommandBarrierKind::ApplyRequestCommands
            )
        );
        assert_eq!(
            compiled[8].kind,
            EcsSpatialCompiledScheduleNodeKind::Barrier(
                EcsSpatialCommandBarrierKind::ApplyArtifactCommands
            )
        );
        assert_eq!(
            compiled[12].kind,
            EcsSpatialCompiledScheduleNodeKind::Barrier(
                EcsSpatialCommandBarrierKind::ApplyHandoffCommands
            )
        );
        for node in compiled {
            if matches!(node.kind, EcsSpatialCompiledScheduleNodeKind::Barrier(_)) {
                assert_eq!(node.work_kind(), EcsWorkKind::ApplyCommands);
                assert_eq!(
                    node.execution_contract().lane,
                    ScheduleLane::EcsCommandBarrier
                );
            }
        }
    }

    #[test]
    fn voxel_grid_specialization_pins_foot_hierarchy() {
        let grid = VoxelGridDesc::terrain_default();
        assert_eq!(grid.validate(), Ok(()));
        assert_eq!(grid.cell_edge_um(), FUN_DEFAULT_TERRAIN_VOXEL_EDGE_UM);
        assert_eq!(grid.cluster_edge_voxels, VOXEL_CLUSTER_EDGE_CELLS);
        assert_eq!(grid.brick_edge_voxels, VOXEL_BRICK_EDGE_CELLS);
        assert_eq!(grid.region_edge_voxels, VOXEL_REGION_EDGE_CELLS);
        assert_eq!(grid.mega_region_edge_voxels(), VOXEL_MEGAREGION_EDGE_CELLS);
        assert_eq!(grid.spatial_grid.cluster_edge_cells, 8);
        assert_eq!(grid.spatial_grid.page_edge_cells, 32);
        assert_eq!(grid.spatial_grid.region_edge_pages, 8);
        assert_eq!(VOXEL_CLUSTER_SUMMARIES_PER_BRICK, 64);
        assert_eq!(VOXEL_BRICK_FOOT_CELL_COUNT, 32 * 32 * 32);
        assert_eq!(VOXEL_BRICK_OCCUPANCY_WORDS, 512);

        let mut invalid = grid;
        invalid.spatial_grid.unit_edge_um = FUN_INCH_VOXEL_EDGE_UM;
        assert_eq!(
            invalid.validate(),
            Err(EcsSpatialValidationError::GridInvalidDimension)
        );
    }

    #[test]
    fn voxel_brick_payload_carries_payload_kind_and_cluster_summaries() {
        let key = terrain_page(EcsPageChannel::Occupancy);
        let empty = VoxelBrickPayload::empty(key, 12);
        assert_eq!(empty.key, key);
        assert_eq!(empty.kind, VoxelPagePayloadKind::Empty);
        assert_eq!(
            empty.generated_class,
            ProceduralGeneratedPageClass::EmptyAir
        );
        assert_eq!(empty.occupancy.kind, VoxelOccupancyStorageKind::Empty);
        assert_eq!(empty.clusters.len(), VOXEL_CLUSTER_SUMMARIES_PER_BRICK);
        assert_eq!(empty.edit_epoch, 12);

        let solid = VoxelBrickPayload::uniform_solid(key, 7, 13);
        assert_eq!(solid.kind, VoxelPagePayloadKind::UniformSolid);
        assert_eq!(
            solid.generated_class,
            ProceduralGeneratedPageClass::UniformSolid
        );
        assert_eq!(
            solid.occupancy.kind,
            VoxelOccupancyStorageKind::UniformSolid
        );
        assert_eq!(solid.material_palette.len, 1);
        assert_eq!(solid.material_palette.materials[0], 7);
        assert!(
            solid
                .clusters
                .iter()
                .all(|summary| summary.occupancy_popcount == 512 && summary.dominant_material == 7)
        );
    }

    #[test]
    fn fine_overlay_links_remain_optional_bounded_generic_pages() {
        let parent = terrain_page(EcsPageChannel::Surface);
        let overlay = EcsSpatialPageKey::new(
            EcsSpatialDomainKind::Terrain,
            EcsSpatialGridId::new(1),
            0,
            4,
            1,
            8,
            EcsPageChannel::FineOverlay,
        );
        let link = EcsFineOverlayLink::new(
            parent,
            overlay,
            EcsSpatialScalePreset::InchHeroPatch,
            bounds(),
        );
        assert_eq!(link.validate(), Ok(()));
        assert!(EcsFineOverlayLink::DEFAULT_OVERRIDES.contains(EcsFineOverlayOverride::Material));
        assert!(EcsFineOverlayLink::DEFAULT_OVERRIDES.contains(EcsFineOverlayOverride::MicroSdf));
        assert!(
            EcsFineOverlayLink::DEFAULT_OVERRIDES.contains(EcsFineOverlayOverride::Displacement)
        );
        assert!(EcsFineOverlayLink::DEFAULT_OVERRIDES.contains(EcsFineOverlayOverride::Decals));
        assert!(EcsFineOverlayLink::DEFAULT_OVERRIDES.contains(EcsFineOverlayOverride::Erosion));
        assert!(
            EcsFineOverlayLink::DEFAULT_OVERRIDES
                .contains(EcsFineOverlayOverride::DestructionScars)
        );
        assert!(EcsFineOverlayLink::DEFAULT_OVERRIDES.contains(EcsFineOverlayOverride::TireTracks));
        assert!(EcsFineOverlayLink::DEFAULT_OVERRIDES.contains(EcsFineOverlayOverride::Footprints));

        let global_scale = EcsFineOverlayLink::new(
            parent,
            overlay,
            EcsSpatialScalePreset::GameplayFoot,
            bounds(),
        );
        assert_eq!(
            global_scale.validate(),
            Err(EcsSpatialValidationError::OverlayScaleNotFine)
        );

        let wrong_channel = EcsFineOverlayLink::new(
            parent,
            terrain_page(EcsPageChannel::Material),
            EcsSpatialScalePreset::InchHeroPatch,
            bounds(),
        );
        assert_eq!(
            wrong_channel.validate(),
            Err(EcsSpatialValidationError::FineOverlayPageInvalid)
        );

        let overlay_as_parent = EcsFineOverlayLink::new(
            overlay,
            overlay,
            EcsSpatialScalePreset::InchHeroPatch,
            bounds(),
        );
        assert_eq!(
            overlay_as_parent.validate(),
            Err(EcsSpatialValidationError::FineOverlayParentInvalid)
        );
    }

    fn stream_source() -> EcsStreamSourceDescriptor {
        EcsStreamSourceDescriptor {
            source: EcsStreamSourceId::new(1),
            domain: EcsSpatialDomainKind::Terrain,
            grid_id: EcsSpatialGridId::new(1),
            priority: 255,
        }
    }

    fn stream_camera(required_shells: u16, desired_shells: u16) -> SpatialStreamCamera {
        SpatialStreamCamera {
            role: StreamCameraRole::MainView,
            enabled: true,
            priority: 255,
            required_shells,
            desired_shells,
            velocity_lookahead_s: 0.5,
        }
    }

    fn camera_sample(
        camera_entity: u64,
        origin_world_ft: IVec3,
        velocity_world_ft_s: IVec3,
        cut_or_teleport: bool,
    ) -> EcsCameraTransformSample {
        EcsCameraTransformSample {
            camera_entity: EcsEntityId::new(camera_entity),
            camera_id: EcsStreamCameraId::new(camera_entity as u32),
            origin_world_ft,
            velocity_world_ft_s,
            view_frustum: EcsViewFrustum::default(),
            cut_or_teleport,
        }
    }

    fn streaming_snapshot(
        camera: SpatialStreamCamera,
        sample: EcsCameraTransformSample,
    ) -> EcsStreamingSourceSnapshot {
        sense_sources(100, &[(camera, sample)], &[stream_source()])
    }

    fn page_xyz(x: i32, y: i32, z: i32) -> EcsSpatialPageKey {
        EcsSpatialPageKey::new(
            EcsSpatialDomainKind::Terrain,
            EcsSpatialGridId::new(1),
            0,
            x,
            y,
            z,
            EcsPageChannel::Occupancy,
        )
    }

    #[test]
    fn camera_cold_start_creates_shell_zero_requests_first() {
        let snapshot = streaming_snapshot(
            stream_camera(0, 1),
            camera_sample(1, IVec3::new(4, 4, 4), IVec3::zero(), false),
        );
        let interests = build_interest(
            &snapshot,
            &EcsSpatialGridRegistry::default(),
            &EcsPageResidencyTable::default(),
        )
        .expect("build interests");
        assert_eq!(interests.interests[0].shell, 0);
        assert_eq!(
            interests.interests[0].kind,
            EcsStreamInterestKind::CameraContainingPage
        );

        let mut commands = EcsSpatialCommandBuffer::default();
        diff_requests(
            &interests,
            &EcsPageResidencyTable::default(),
            &[],
            &[],
            &mut commands,
        )
        .expect("diff requests");
        assert_eq!(
            commands.commands.first(),
            Some(&EcsSpatialCommand::RequestPage(page_xyz(0, 0, 0)))
        );
    }

    #[test]
    fn fast_movement_biases_lookahead_without_starving_current_shell() {
        let snapshot = streaming_snapshot(
            stream_camera(0, 2),
            camera_sample(1, IVec3::new(4, 4, 4), IVec3::new(128, 0, 0), false),
        );
        let interests = build_interest(
            &snapshot,
            &EcsSpatialGridRegistry::default(),
            &EcsPageResidencyTable::default(),
        )
        .expect("build interests");
        assert_eq!(interests.interests[0].shell, 0);
        assert_eq!(
            interests.interests[0].kind,
            EcsStreamInterestKind::CameraContainingPage
        );
        let lookahead_index = interests
            .interests
            .iter()
            .position(|interest| interest.kind == EcsStreamInterestKind::VelocityLookahead)
            .expect("velocity lookahead interest");
        let first_visible_near_index = interests
            .interests
            .iter()
            .position(|interest| interest.kind == EcsStreamInterestKind::VisibleNear)
            .expect("visible near interest");
        assert!(lookahead_index < first_visible_near_index);
    }

    #[test]
    fn teleport_cancels_stale_outer_shell_and_starts_new_wave() {
        let snapshot = streaming_snapshot(
            stream_camera(0, 1),
            camera_sample(7, IVec3::new(2_048, 0, 0), IVec3::zero(), true),
        );
        let mut ledger = EcsStreamWaveLedger::default();
        let wave = plan_stream_wave(&snapshot, &mut ledger, 0, 1, StreamWaveReason::CameraMoved)
            .expect("teleport wave");
        assert_eq!(wave.reason, StreamWaveReason::TeleportOrCut);
        assert_eq!(wave.wave_id, EcsStreamWaveId::new(1));
        assert_eq!(wave.origin_page, page_xyz(64, 0, 0));

        let interests = build_interest(
            &snapshot,
            &EcsSpatialGridRegistry::default(),
            &EcsPageResidencyTable::default(),
        )
        .expect("build interests");
        let stale_outer_page = page_xyz(0, 3, 0);
        let mut commands = EcsSpatialCommandBuffer::default();
        let result = diff_requests(
            &interests,
            &EcsPageResidencyTable::default(),
            &[stale_outer_page],
            &[],
            &mut commands,
        )
        .expect("diff requests");
        assert_eq!(result.cancelled, 1);
        assert!(
            commands
                .commands
                .contains(&EcsSpatialCommand::CancelPage(stale_outer_page))
        );
    }

    #[test]
    fn deterministic_camera_path_produces_deterministic_request_order() {
        let snapshot = streaming_snapshot(
            stream_camera(1, 2),
            camera_sample(2, IVec3::new(96, 64, 0), IVec3::new(64, 0, 0), false),
        );
        let interests_a = build_interest(
            &snapshot,
            &EcsSpatialGridRegistry::default(),
            &EcsPageResidencyTable::default(),
        )
        .expect("interests a");
        let interests_b = build_interest(
            &snapshot,
            &EcsSpatialGridRegistry::default(),
            &EcsPageResidencyTable::default(),
        )
        .expect("interests b");
        assert_eq!(interests_a, interests_b);

        let mut commands_a = EcsSpatialCommandBuffer::default();
        let mut commands_b = EcsSpatialCommandBuffer::default();
        diff_requests(
            &interests_a,
            &EcsPageResidencyTable::default(),
            &[],
            &[],
            &mut commands_a,
        )
        .expect("diff a");
        diff_requests(
            &interests_b,
            &EcsPageResidencyTable::default(),
            &[],
            &[],
            &mut commands_b,
        )
        .expect("diff b");
        assert_eq!(commands_a.commands, commands_b.commands);
    }

    #[test]
    fn request_diff_uses_command_buffer_and_marks_holes_without_world_mutation() {
        let failed_page = page_xyz(0, 0, 0);
        let fallback_page = page_xyz(0, 0, -1);
        let mut residency = EcsPageResidencyTable::default();
        let mut failed_record = EcsPageResidencyRecord::new(
            failed_page,
            EcsStreamPriority::new(EcsStreamInterestKind::CameraContainingPage, 0, 0),
            1,
        );
        failed_record.state = EcsPageResidencyState::Failed;
        failed_record.fallback = Some(fallback_page);
        residency
            .insert(EcsSpatialPageId::new(99), failed_record)
            .expect("failed page insert");
        let before = residency.clone();

        let snapshot = streaming_snapshot(
            stream_camera(0, 0),
            camera_sample(3, IVec3::new(4, 4, 4), IVec3::zero(), false),
        );
        let interests = build_interest(&snapshot, &EcsSpatialGridRegistry::default(), &residency)
            .expect("build interests");
        let mut commands = EcsSpatialCommandBuffer::default();
        let result =
            diff_requests(&interests, &residency, &[], &[], &mut commands).expect("diff requests");

        assert_eq!(residency, before);
        assert_eq!(result.holes.len(), 1);
        assert_eq!(result.holes[0].page, failed_page);
        assert_eq!(result.holes[0].fallback, Some(fallback_page));
        assert!(
            commands
                .commands
                .contains(&EcsSpatialCommand::RequestPage(fallback_page))
        );
    }

    #[derive(Debug, Clone)]
    struct TestSpatialSource {
        source: EcsSpatialSourceId,
        source_kind: EcsSpatialSourceKind,
        region_edge_pages: u16,
        payloads: Vec<(EcsSpatialPageKey, EcsSourcePayload)>,
    }

    impl EcsSpatialSource for TestSpatialSource {
        fn source_id(&self) -> EcsSpatialSourceId {
            self.source
        }

        fn manifest_for_region(&self, key: EcsSpatialRegionKey) -> EcsSpatialRegionManifest {
            EcsSpatialRegionManifest {
                region: key,
                source: self.source,
                source_kind: self.source_kind,
                manifest_epoch: 4,
                page_count: u32::from(self.region_edge_pages).pow(3),
                channel_mask: EcsPageChannelMask::terrain_primary(),
                checksum: EcsSourceChecksum {
                    algorithm: EcsSourceChecksumAlgorithm::Fnv1a64,
                    value: key.chunk_key().get(),
                },
            }
        }

        fn request_page(&self, key: EcsSpatialPageKey) -> EcsSourceRequest {
            let region = EcsSpatialRegionKey::from_page(key, self.region_edge_pages);
            let payload = self
                .payloads
                .iter()
                .find(|(candidate, _payload)| *candidate == key)
                .map(|(_candidate, payload)| payload.clone())
                .unwrap_or_else(|| {
                    EcsSourcePayload::Failure(EcsSourceFailure {
                        key,
                        code: EcsPageFailureCode::SourceUnavailable,
                        checksum: EcsSourceChecksum::NONE,
                        retry_after_frame: 0,
                    })
                });
            EcsSourceRequest {
                request_id: EcsSourceRequestId::new(key.chunk_key().get()),
                source: self.source,
                source_kind: self.source_kind,
                key,
                region,
                manifest_epoch: 4,
                source_epoch: 5,
                priority: EcsStreamPriority::new(EcsStreamInterestKind::VisibleNear, 1, key.level),
                payload,
            }
        }
    }

    fn source_with_payloads(
        payloads: Vec<(EcsSpatialPageKey, EcsSourcePayload)>,
    ) -> TestSpatialSource {
        TestSpatialSource {
            source: EcsSpatialSourceId::new(17),
            source_kind: EcsSpatialSourceKind::CookedPackage,
            region_edge_pages: 8,
            payloads,
        }
    }

    fn compressed_payload(key: EcsSpatialPageKey, bytes: Vec<u8>) -> EcsSourcePayload {
        let checksum = EcsSourceChecksum::fnv1a64(&bytes);
        EcsSourcePayload::CompressedPage(
            EcsCompressedPagePayload::try_new(key, EcsSourcePayloadCodec::Zstd, bytes, checksum)
                .expect("valid compressed payload"),
        )
    }

    fn procedural_surface_decoded_page() -> EcsDecodedPageRecord {
        let manifest = EcsProceduralWorldManifest::default();
        let key = page_xyz(0, 1, 0);
        generate_procedural_terrain_page(
            manifest,
            manifest.recipe_ref_for_page(key),
            EcsSpatialSourceId::new(9),
            manifest.source_epoch(),
            8,
            0,
        )
        .expect("procedural surface decoded page")
    }

    fn decoded_page_with_class(
        key: EcsSpatialPageKey,
        generated_class: ProceduralGeneratedPageClass,
    ) -> EcsDecodedPageRecord {
        let mut brick = if generated_class == ProceduralGeneratedPageClass::UniformSolid {
            VoxelBrickPayload::uniform_solid(key, 3, 5)
        } else {
            VoxelBrickPayload::empty(key, 5)
        };
        brick.generated_class = generated_class;
        if generated_class.builds_surface_artifacts() {
            brick.kind = VoxelPagePayloadKind::DenseFootCells;
        }
        let clusters = brick.clusters;
        EcsDecodedPageRecord {
            key,
            source: EcsSpatialSourceId::new(17),
            source_epoch: 5,
            payload_kind: if generated_class == ProceduralGeneratedPageClass::EmptyAir {
                EcsDecodedPagePayloadKind::Empty
            } else {
                EcsDecodedPagePayloadKind::VoxelBrick
            },
            voxel_brick: Some(brick),
            cluster_summaries: clusters,
            telemetry: EcsDecodeTelemetry {
                key,
                source_epoch: 5,
                decode_epoch: 8,
                source_bytes: 0,
                overlay_count: 0,
                cluster_summary_count: VOXEL_CLUSTER_SUMMARIES_PER_BRICK as u16,
                checksum: EcsSourceChecksum::NONE,
                failure: None,
            },
        }
    }

    #[test]
    fn generic_spatial_source_acquire_outputs_payload_recipe_failure_and_checksum() {
        let compressed_page = page_xyz(0, 0, 0);
        let recipe_page = page_xyz(1, 0, 0);
        let missing_page = page_xyz(2, 0, 0);
        let source = source_with_payloads(vec![
            (
                compressed_page,
                compressed_payload(compressed_page, vec![7, 3]),
            ),
            (
                recipe_page,
                EcsSourcePayload::ProceduralTerrainRecipe(
                    EcsProceduralWorldManifest::default().recipe_ref_for_page(recipe_page),
                ),
            ),
        ]);

        let mut queue = EcsSourceAcquireQueue::default();
        let report = acquire_sources(
            &source,
            &[compressed_page, recipe_page, missing_page],
            8,
            &mut queue,
        )
        .expect("acquire sources");

        assert_eq!(report.requested, 3);
        assert_eq!(report.compressed_payloads, 1);
        assert_eq!(report.procedural_recipes, 1);
        assert_eq!(report.failures, 1);
        assert_eq!(queue.rows.len(), 3);
        assert!(matches!(
            queue.rows[0].request.payload,
            EcsSourcePayload::CompressedPage(_)
        ));
        assert!(matches!(
            queue.rows[1].request.payload,
            EcsSourcePayload::ProceduralTerrainRecipe(_)
        ));
        assert!(matches!(
            queue.rows[2].request.payload,
            EcsSourcePayload::Failure(_)
        ));
        assert_eq!(
            queue.rows[0].manifest.region,
            EcsSpatialRegionKey::from_page(compressed_page, 8)
        );

        let acquire_contract = EcsSpatialScheduleSet::AcquireSources.execution_contract();
        assert_eq!(acquire_contract.domain, ScheduleDomain::Blocking);
        assert_eq!(acquire_contract.lane, ScheduleLane::Blocking);
        assert_eq!(
            acquire_contract.blocking_class,
            WorkBlockingClass::BlockingPool
        );
        assert_eq!(
            acquire_contract.main_thread,
            MainThreadRequirement::NotMainThread
        );
    }

    #[test]
    fn decode_pages_reads_payload_manifest_and_overlays_into_cluster_telemetry() {
        let page = page_xyz(0, 0, 0);
        let source = source_with_payloads(vec![(page, compressed_payload(page, vec![7]))]);
        let mut acquired = EcsSourceAcquireQueue::default();
        acquire_sources(&source, &[page], 8, &mut acquired).expect("acquire page");

        let overlays = [
            EcsDecodeOverlay {
                key: page,
                kind: EcsDecodeOverlayKind::SaveGameDelta,
                epoch: 11,
                checksum: EcsSourceChecksum::NONE,
            },
            EcsDecodeOverlay {
                key: page,
                kind: EcsDecodeOverlayKind::EditorMemory,
                epoch: 12,
                checksum: EcsSourceChecksum::NONE,
            },
        ];
        let mut decoded = EcsDecodedPageQueue::default();
        let report = decode_pages(&acquired, &overlays, 6, &mut decoded).expect("decode pages");

        assert_eq!(report.decoded, 1);
        assert_eq!(report.overlays_read, 2);
        assert_eq!(decoded.rows.len(), 1);
        let row = &decoded.rows[0];
        assert_eq!(row.key, page);
        assert_eq!(row.payload_kind, EcsDecodedPagePayloadKind::VoxelBrick);
        assert_eq!(
            row.cluster_summaries.len(),
            VOXEL_CLUSTER_SUMMARIES_PER_BRICK
        );
        assert_eq!(row.cluster_summaries[0].dominant_material, 7);
        assert_eq!(row.telemetry.source_bytes, 1);
        assert_eq!(row.telemetry.overlay_count, 2);
        assert_eq!(
            row.telemetry.cluster_summary_count,
            VOXEL_CLUSTER_SUMMARIES_PER_BRICK as u16
        );
    }

    #[test]
    fn decode_pages_limits_procedural_generation_by_frame_budget() {
        let manifest = EcsProceduralWorldManifest::default();
        let source = EcsProceduralTerrainSource::new(EcsSpatialSourceId::new(9), manifest);
        let first_page = page_xyz(0, 1, 0);
        let second_page = page_xyz(1, 1, 0);
        let mut acquired = EcsSourceAcquireQueue::default();
        acquire_sources(&source, &[first_page, second_page], 8, &mut acquired)
            .expect("acquire procedural source rows");

        let mut budget = manifest.generator_desc().budgets;
        budget.max_pages_generated_per_frame = 1;
        let mut decoded = EcsDecodedPageQueue::default();
        let report = decode_pages_with_procedural_manifest_and_budget(
            &acquired,
            &[],
            6,
            manifest,
            budget,
            &mut decoded,
        )
        .expect("decode with budget");

        assert_eq!(report.generated_pages, 1);
        assert_eq!(report.budget_deferred, 1);
        assert_eq!(report.decoded, 1);
        assert_eq!(report.procedural, 0);
        assert_eq!(decoded.rows.len(), 1);
        assert_eq!(decoded.rows[0].key, first_page);
    }

    #[test]
    fn decode_pages_rejects_malformed_source_payload_length() {
        let page = page_xyz(0, 0, 0);
        let malformed = EcsCompressedPagePayload {
            key: page,
            codec: EcsSourcePayloadCodec::Zstd,
            byte_len: 9,
            bytes: vec![1],
            checksum: EcsSourceChecksum::NONE,
        };
        assert_eq!(
            malformed.validate(),
            Err(EcsSpatialValidationError::SourcePayloadLengthMismatch)
        );

        let source =
            source_with_payloads(vec![(page, EcsSourcePayload::CompressedPage(malformed))]);
        let mut acquired = EcsSourceAcquireQueue::default();
        assert_eq!(
            acquire_sources(&source, &[page], 8, &mut acquired),
            Err(EcsSpatialValidationError::SourcePayloadLengthMismatch)
        );
    }

    #[test]
    fn build_derived_artifacts_declares_all_requested_specialized_systems() {
        let labels: Vec<&'static str> = EcsDerivedArtifactBuildSystem::all()
            .iter()
            .map(|system| system.label())
            .collect();
        assert_eq!(labels.len(), ECS_DERIVED_ARTIFACT_BUILD_SYSTEM_COUNT);
        assert!(labels.contains(&"build_terrain_surface_packets"));
        assert!(labels.contains(&"build_terrain_coarse_proxy"));
        assert!(labels.contains(&"build_terrain_sdf"));
        assert!(labels.contains(&"build_terrain_material_page"));
        assert!(labels.contains(&"build_load_animation_record"));
        assert!(labels.contains(&"build_foliage_seeds"));
        assert!(labels.contains(&"build_foliage_clusters"));
        assert!(labels.contains(&"build_canopy_opacity"));
        assert!(labels.contains(&"build_shadow_invalidation_rows"));
        assert!(labels.contains(&"build_radiance_update_rows"));
        assert!(labels.contains(&"build_physics_cook_requests"));
        assert!(labels.contains(&"build_collision_sdf_proxy"));
        assert!(labels.contains(&"build_voxel_edit_delta_rows"));
        assert!(labels.contains(&"build_network_relevance_rows"));
        assert!(labels.contains(&"build_foliage_trunk_branch_virtual_geometry"));
        assert!(labels.contains(&"build_foliage_grass_brush_instance_clusters"));
        assert!(labels.contains(&"build_foliage_collision_large_object_proxy"));
        assert!(labels.contains(&"build_foliage_cards_impostors"));
        assert!(labels.contains(&"build_foliage_clustered_animation"));
        assert!(labels.contains(&"build_foliage_canopy_transmittance"));
        assert!(labels.contains(&"build_foliage_biome_tint"));
        assert!(labels.contains(&"build_foliage_horizon_impostor_field"));
        assert!(labels.contains(&"build_foliage_sdf_opacity_shadows"));
        assert!(labels.contains(&"build_storm_extinction_dirty_rows"));
    }

    #[test]
    fn prototype_terrain_profile_builds_first_visual_artifacts_only() {
        let decoded_page = procedural_surface_decoded_page();
        let mut next_artifact_id = 0;
        let mut commands = EcsSpatialCommandBuffer::default();
        let report = build_prototype_terrain_artifacts(
            &[decoded_page],
            None,
            ProceduralTerrainArtifactFlags::FIRST_VISUAL,
            &mut next_artifact_id,
            &mut commands,
        )
        .expect("build first visual artifacts");
        let artifacts: Vec<EcsDerivedArtifactRecord> = commands
            .commands
            .iter()
            .filter_map(|command| match command {
                EcsSpatialCommand::PublishArtifact(artifact) => Some(*artifact),
                _ => None,
            })
            .collect();
        let kinds: Vec<EcsDerivedArtifactKind> =
            artifacts.iter().map(|artifact| artifact.kind).collect();

        assert_eq!(report.artifacts_published, 4);
        assert_eq!(next_artifact_id, 4);
        for kind in ECS_PROCEDURAL_TERRAIN_REQUIRED_ARTIFACT_KINDS {
            assert!(kinds.contains(&kind), "missing required artifact {kind:?}");
        }
        for kind in ECS_PROCEDURAL_TERRAIN_OPTIONAL_ARTIFACT_KINDS {
            assert!(
                !kinds.contains(&kind),
                "optional artifact {kind:?} must be flag gated"
            );
        }
    }

    #[test]
    fn optional_prototype_artifact_flags_enable_refinement_without_requiredness() {
        let decoded_page = procedural_surface_decoded_page();
        let mut next_artifact_id = 0;
        let mut commands = EcsSpatialCommandBuffer::default();
        build_prototype_terrain_artifacts(
            &[decoded_page],
            None,
            ProceduralTerrainArtifactFlags::ALL_OPTIONAL,
            &mut next_artifact_id,
            &mut commands,
        )
        .expect("build optional prototype artifacts");
        let artifacts: Vec<EcsDerivedArtifactRecord> = commands
            .commands
            .iter()
            .filter_map(|command| match command {
                EcsSpatialCommand::PublishArtifact(artifact) => Some(*artifact),
                _ => None,
            })
            .collect();

        for kind in ECS_PROCEDURAL_TERRAIN_OPTIONAL_ARTIFACT_KINDS {
            let artifact = artifacts
                .iter()
                .find(|artifact| artifact.kind == kind)
                .unwrap_or_else(|| panic!("missing optional artifact {kind:?}"));
            assert_eq!(artifact.requiredness, WorkRequiredness::Optional);
        }
        assert!(artifacts.iter().any(|artifact| {
            artifact.kind == EcsDerivedArtifactKind::VirtualShadowInvalidation
                && artifact.consumer == EcsArtifactConsumer::Lux
        }));
        assert!(
            !artifacts
                .iter()
                .any(|artifact| artifact.kind == EcsDerivedArtifactKind::ShadowInvalidationRows)
        );
    }

    #[test]
    fn procedural_terrain_payload_builders_extract_proxy_surface_and_material_data() {
        let decoded_page = procedural_surface_decoded_page();
        let coarse =
            procedural_terrain_coarse_proxy_from_decoded_page(&decoded_page).expect("coarse proxy");
        let surface = procedural_terrain_surface_packet_from_decoded_page(&decoded_page)
            .expect("surface packet");
        let material = procedural_terrain_material_page_from_decoded_page(&decoded_page)
            .expect("material page");

        assert_eq!(coarse.source_page, decoded_page.key);
        assert!(coarse.min_height_ft <= coarse.max_height_ft);
        assert_ne!(coarse.occupied_cluster_mask, 0);
        assert!(coarse.dominant_material.is_valid());
        assert_eq!(surface.source_page, decoded_page.key);
        assert_ne!(surface.exposed_face_count, 0);
        assert_eq!(surface.packet_range.count, surface.exposed_face_count);
        assert!(surface.material_palette_id.is_valid());
        assert_eq!(material.source_page, decoded_page.key);
        assert_ne!(material.palette.len, 0);
        assert!(
            material
                .dominant_material_per_cluster
                .iter()
                .any(|material| material.is_valid())
        );
    }

    #[test]
    fn procedural_load_animation_progress_matches_stream_pipeline_stages() {
        let page = page_xyz(0, 1, 0);
        let stages = [
            (ProceduralTerrainLoadStage::PageRequested, 0.10),
            (ProceduralTerrainLoadStage::RecipeAcquired, 0.25),
            (ProceduralTerrainLoadStage::PageDecoded, 0.45),
            (ProceduralTerrainLoadStage::ArtifactReady, 0.75),
            (ProceduralTerrainLoadStage::RendererPublished, 1.00),
        ];

        for (stage, progress) in stages {
            let record = ProceduralTerrainLoadAnimationRecord::new(page, stage, 7);
            assert_eq!(record.progress, progress);
            assert_eq!(record.stage.label(), stage.label());
        }
    }

    #[test]
    fn uniform_pages_publish_only_aggregate_physics_artifacts() {
        let page = page_xyz(0, 0, 0);
        let source = source_with_payloads(vec![(page, compressed_payload(page, vec![3]))]);
        let mut acquired = EcsSourceAcquireQueue::default();
        acquire_sources(&source, &[page], 8, &mut acquired).expect("acquire page");
        let mut decoded = EcsDecodedPageQueue::default();
        decode_pages(&acquired, &[], 8, &mut decoded).expect("decode page");

        let registry = EcsDerivedArtifactRegistry::default();
        let mut next_artifact_id = 0;
        let mut commands = EcsSpatialCommandBuffer::default();
        let report = build_derived_artifacts(
            &decoded.rows,
            EcsDerivedArtifactBuildSystem::all(),
            &mut next_artifact_id,
            &mut commands,
        )
        .expect("build artifacts");

        assert_eq!(registry.len(), 0);
        assert_eq!(report.decoded_pages, 1);
        assert_eq!(report.artifacts_published, 2);
        assert_eq!(
            report.skipped_by_page_class as usize,
            ECS_DERIVED_ARTIFACT_BUILD_SYSTEM_COUNT - 2
        );
        let artifacts: Vec<EcsDerivedArtifactRecord> = commands
            .commands
            .iter()
            .filter_map(|command| match command {
                EcsSpatialCommand::PublishArtifact(artifact) => Some(*artifact),
                _ => None,
            })
            .collect();
        assert_eq!(artifacts.len(), 2);
        assert!(artifacts.iter().any(|artifact| {
            artifact.kind == EcsDerivedArtifactKind::PhysicsCookRequests
                && artifact.consumer == EcsArtifactConsumer::AvisPhysics
        }));
        assert!(artifacts.iter().any(|artifact| {
            artifact.kind == EcsDerivedArtifactKind::CollisionSdfProxy
                && artifact.consumer == EcsArtifactConsumer::AvisPhysics
        }));
        assert_eq!(next_artifact_id, 2);
    }

    #[test]
    fn artifact_publishers_need_deterministic_apply_before_renderer_handoffs() {
        let page = page_xyz(0, 0, 0);
        let mut decoded = EcsDecodedPageQueue::default();
        decoded
            .push(decoded_page_with_class(
                page,
                ProceduralGeneratedPageClass::SurfaceMixed,
            ))
            .expect("push surface page");

        let mut next_artifact_id = 0;
        let mut commands = EcsSpatialCommandBuffer::default();
        build_derived_artifacts(
            &decoded.rows,
            &[
                EcsDerivedArtifactBuildSystem::BuildTerrainSurfacePackets,
                EcsDerivedArtifactBuildSystem::BuildTerrainMaterialPage,
            ],
            &mut next_artifact_id,
            &mut commands,
        )
        .expect("build artifacts");

        let empty_registry = EcsDerivedArtifactRegistry::default();
        let mut pre_apply_handoff_commands = EcsSpatialCommandBuffer::default();
        let pre_apply_report = publish_renderer_handoffs(
            empty_registry.artifacts.values(),
            RendererVisibilityHint::VisibleNear,
            &mut pre_apply_handoff_commands,
        )
        .expect("pre-apply publish");
        assert_eq!(pre_apply_report.published, 0);
        assert!(pre_apply_handoff_commands.commands.is_empty());

        let mut commands_a = commands.clone();
        let mut commands_b = commands.clone();
        commands_b.commands.reverse();
        let mut registry_a = EcsDerivedArtifactRegistry::default();
        let mut registry_b = EcsDerivedArtifactRegistry::default();
        let apply_a =
            apply_artifact_commands(&mut commands_a, &mut registry_a).expect("apply artifacts a");
        let apply_b =
            apply_artifact_commands(&mut commands_b, &mut registry_b).expect("apply artifacts b");

        assert_eq!(apply_a.applied, 2);
        assert_eq!(apply_b.applied, 2);
        assert_eq!(apply_a.digest, apply_b.digest);
        assert_eq!(registry_a.artifacts.values(), registry_b.artifacts.values());

        let mut handoff_commands_a = EcsSpatialCommandBuffer::default();
        let mut handoff_commands_b = EcsSpatialCommandBuffer::default();
        publish_renderer_handoffs(
            registry_a.artifacts.values(),
            RendererVisibilityHint::VisibleNear,
            &mut handoff_commands_a,
        )
        .expect("publish handoffs a");
        publish_renderer_handoffs(
            registry_b.artifacts.values(),
            RendererVisibilityHint::VisibleNear,
            &mut handoff_commands_b,
        )
        .expect("publish handoffs b");
        handoff_commands_b.commands.reverse();

        let mut renderer_queue_a = EcsRendererHandoffQueue::default();
        let mut renderer_queue_b = EcsRendererHandoffQueue::default();
        let mut lux_queue_a = EcsLuxHandoffQueue::default();
        let mut lux_queue_b = EcsLuxHandoffQueue::default();
        let mut physics_queue_a = EcsPhysicsCookQueue::default();
        let mut physics_queue_b = EcsPhysicsCookQueue::default();
        let handoff_apply_a = apply_handoff_commands(
            &mut handoff_commands_a,
            &mut renderer_queue_a,
            &mut lux_queue_a,
            &mut physics_queue_a,
        )
        .expect("apply handoffs a");
        let handoff_apply_b = apply_handoff_commands(
            &mut handoff_commands_b,
            &mut renderer_queue_b,
            &mut lux_queue_b,
            &mut physics_queue_b,
        )
        .expect("apply handoffs b");

        assert_eq!(handoff_apply_a.applied, 2);
        assert_eq!(handoff_apply_a.digest, handoff_apply_b.digest);
        assert_eq!(renderer_queue_a, renderer_queue_b);
        assert_eq!(renderer_queue_a.items.len(), 2);
        assert!(lux_queue_a.items.is_empty());
        assert!(physics_queue_a.items.is_empty());
    }

    #[test]
    fn build_derived_artifacts_skips_failed_source_pages() {
        let page = page_xyz(0, 0, 0);
        let source = source_with_payloads(Vec::new());
        let mut acquired = EcsSourceAcquireQueue::default();
        acquire_sources(&source, &[page], 8, &mut acquired).expect("acquire failure");
        let mut decoded = EcsDecodedPageQueue::default();
        decode_pages(&acquired, &[], 8, &mut decoded).expect("decode failure");

        let mut next_artifact_id = 0;
        let mut commands = EcsSpatialCommandBuffer::default();
        let report = build_derived_artifacts(
            &decoded.rows,
            EcsDerivedArtifactBuildSystem::all(),
            &mut next_artifact_id,
            &mut commands,
        )
        .expect("build failed page");

        assert_eq!(report.skipped_failed_pages, 1);
        assert_eq!(report.artifacts_published, 0);
        assert!(commands.commands.is_empty());
    }

    #[test]
    fn empty_pages_emit_no_surface_physics_or_lux_artifacts() {
        let page = page_xyz(0, 0, 4);
        let mut decoded = EcsDecodedPageQueue::default();
        decoded
            .push(decoded_page_with_class(
                page,
                ProceduralGeneratedPageClass::EmptyAir,
            ))
            .expect("push empty page");

        let mut next_artifact_id = 0;
        let mut commands = EcsSpatialCommandBuffer::default();
        let report = build_derived_artifacts(
            &decoded.rows,
            EcsDerivedArtifactBuildSystem::all(),
            &mut next_artifact_id,
            &mut commands,
        )
        .expect("build empty page");

        assert_eq!(report.decoded_pages, 1);
        assert_eq!(report.artifacts_published, 0);
        assert_eq!(
            report.skipped_by_page_class as usize,
            ECS_DERIVED_ARTIFACT_BUILD_SYSTEM_COUNT
        );
        assert_eq!(next_artifact_id, 0);
        assert!(commands.commands.is_empty());
    }
}
