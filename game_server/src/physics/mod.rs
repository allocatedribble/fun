//! Server-side physics adapters.
//!
//! Currently houses the [`active_pool`] prototype. This is the first
//! "shard-local dense active rows" surface; it lives in `game_server`
//! deliberately because the first iteration is allowed to make
//! project-specific assumptions (sharding, replication dirty bits,
//! lifecycle policy) that would not be appropriate inside Avian yet.
//!
//! The plugin is not registered in `main.rs` by default so behavior is
//! unchanged. Wire `ActivePhysicsPoolPlugin` into the app explicitly to
//! exercise the pool path.

#[allow(dead_code)]
pub mod acceleration_bridge;
#[allow(dead_code)]
pub mod active_pool;
#[allow(dead_code)]
pub mod avis_rigid_mvp;
#[allow(dead_code)]
pub mod cold_storage;
#[allow(dead_code)]
pub mod physical_lod;
#[allow(dead_code)]
pub mod replay_bridge;
#[allow(dead_code)]
pub mod shard;
#[allow(dead_code)]
pub mod thunder_bridge;

#[allow(unused_imports)]
pub use acceleration_bridge::{
    AccelerationDecision, AccelerationDecisionPolicy, AccelerationOpComplexity,
    ShardAccelerationDiagnostics, build_shard_acceleration_policy, decide_acceleration,
    resolve_acceleration_policy,
};
#[allow(unused_imports)]
pub use active_pool::{
    ActiveBodyDirtyRow, ActiveBodyHandle, ActiveBodyId, ActiveBodyKind, ActiveBodyRow,
    ActivePhysicsHandle, ActivePhysicsPool, ActivePhysicsPoolPlugin, ActivePoolDiagnostics,
    ActivePoolDirtyMask, ActivePoolPolicy, ActivePoolWritebackMode,
};
#[allow(unused_imports)]
pub use avis_rigid_mvp::{
    AvisRigidBroadProxyLanes, AvisRigidDiagnostics, AvisRigidDirtyRow, AvisRigidExperimental,
    AvisRigidMode, avis_dirty_outputs_to_ecs_writeback, avis_dirty_outputs_to_thunder_bridge,
    ecs_extraction_to_avis_lane,
};
#[allow(unused_imports)]
pub use cold_storage::{
    AggregateBody, COLD_CHUNK_MAGIC, COLD_CHUNK_VERSION, COLD_RECORD_BYTES, ColdBodyClass,
    ColdChunkError, ColdChunkHeader, ColdChunkSchemaVersion, ColdChunkSnapshot, ColdColliderClass,
    ColdEncodingStats, ColdRecord, ColdRecordEncoding, ColdStorageDiagnostics,
    DEFAULT_MAX_CHUNK_BYTES, LifecycleTransition, decode_records_with, demote_to_cold,
    encode_records_with, promote_to_active,
};
#[allow(unused_imports)]
pub use physical_lod::{
    AggregateContext, AggregateKind, MergeReason, PhysicalClass, PhysicalClassPolicy,
    PhysicalLodDiagnostics, PhysicalLodRecord, PhysicalLodRegistry, ReplicationClass, SplitReason,
    should_merge_into_aggregate, should_split_aggregate,
};
#[allow(unused_imports)]
pub use replay_bridge::{
    EvidenceBundleDiff, MappedCommand, ReplayContactEvidence, ReplayEvidenceBundle,
    ShardOwnershipDivergence, build_command_stream, build_evidence_bundle, build_rollback_slice,
    build_state_digest, compare_evidence_bundles, contact_digest, map_command, map_tier,
};
#[allow(unused_imports)]
pub use shard::{
    BodyLifecycleCommand, BodyLifecycleTier, BoundaryOwnership, CellId, GlobalPhysicalEntityId,
    ShardBodyRecord, ShardBudget, ShardId, ShardIdentity, ShardLocalBodyId, ShardNeighbors,
    ShardOverloadAction, ShardOverloadPolicy, ShardOverloadReport, ShardPhysics,
    ShardPressureLevel, ShardRegistry, ShardRegistryDiagnostics, ShardReplication, ShardRuntime,
    ShardRuntimeMode, ShardTransferLedger, ShardWorkItem, ShardWorkScheduler, TransferWindow,
    TransferWindowStage, evaluate_shard_overload, is_demote_eligible,
};
#[allow(unused_imports)]
pub use thunder_bridge::ThunderPhysicsDeltaSnapshot;
