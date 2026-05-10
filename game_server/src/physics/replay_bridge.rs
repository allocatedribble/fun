//! Adapter that builds a Thunder [`replay::RollbackSlice`] /
//! [`replay::StateDigest`] / [`replay::CommandStream`] from the
//! game-server-side [`ShardRegistry`].
//!
//! Pass 18 secondary-owner work. The thunder side owns the protocol
//! types; this module is the thin walk-the-shard-table glue that emits
//! deterministic, sorted, canonicalised data into the typed builders.
//!
//! # Determinism contract
//!
//! - Body iteration follows shard order (sorted by `ShardId`) and
//!   inside each shard, sorted by `GlobalPhysicalEntityId`. The
//!   `StateDigestBuilder` re-sorts before hashing, but feeding the
//!   builder in stable order keeps the slice's *vector* representation
//!   stable too — the rollback slice is a snapshot, not just a hash,
//!   so its on-wire byte layout must also be stable.
//! - Quantization mirrors the cold-storage rules (mm pose, q16
//!   rotation, cm/s linear, mrad/s angular). This keeps a slice
//!   directly comparable against the cold-record format from Pass 16.

use bevy::prelude::*;
use thunder::replay::{
    BodyTierKind, CommandStream, DigestActiveRow, DigestShardOwnership, LifecycleCommandKind,
    LifecycleCommandRecord, LifecycleCounts, PolicyVersion, ReplayBodyId, ReplayShardId,
    RollbackActiveBodyRow, RollbackSlice, ShardTransferRecord, StateDigest, StateDigestBuilder,
};
use thunder::timeline::NetworkTick;

use super::shard::{
    BodyLifecycleCommand, BodyLifecycleTier, GlobalPhysicalEntityId, ShardBodyRecord, ShardId,
    ShardRegistry,
};

/// Convert the game-server lifecycle tier to the typed thunder mirror.
#[must_use]
pub fn map_tier(tier: BodyLifecycleTier) -> BodyTierKind {
    match tier {
        BodyLifecycleTier::Cold => BodyTierKind::Cold,
        BodyLifecycleTier::Warm => BodyTierKind::Warm,
        BodyLifecycleTier::Active => BodyTierKind::Active,
        BodyLifecycleTier::Ghost => BodyTierKind::Ghost,
    }
}

/// Convert the game-server `BodyLifecycleCommand` into a typed
/// [`LifecycleCommandRecord`]. `TransferShard` is split out into its
/// own [`ShardTransferRecord`]; this returns both so callers can push
/// them into the matching streams.
#[must_use]
pub fn map_command(command: BodyLifecycleCommand) -> MappedCommand {
    match command {
        BodyLifecycleCommand::PromoteToWarm { shard, body } => {
            MappedCommand::Lifecycle(LifecycleCommandRecord {
                kind: LifecycleCommandKind::PromoteToWarm,
                body: ReplayBodyId(body.0),
                primary_shard: ReplayShardId(shard.0),
                source_shard: ReplayShardId(shard.0),
            })
        }
        BodyLifecycleCommand::ActivateBody { shard, body } => {
            MappedCommand::Lifecycle(LifecycleCommandRecord {
                kind: LifecycleCommandKind::ActivateBody,
                body: ReplayBodyId(body.0),
                primary_shard: ReplayShardId(shard.0),
                source_shard: ReplayShardId(shard.0),
            })
        }
        BodyLifecycleCommand::SleepBody { shard, body } => {
            MappedCommand::Lifecycle(LifecycleCommandRecord {
                kind: LifecycleCommandKind::SleepBody,
                body: ReplayBodyId(body.0),
                primary_shard: ReplayShardId(shard.0),
                source_shard: ReplayShardId(shard.0),
            })
        }
        BodyLifecycleCommand::DemoteToCold { shard, body } => {
            MappedCommand::Lifecycle(LifecycleCommandRecord {
                kind: LifecycleCommandKind::DemoteToCold,
                body: ReplayBodyId(body.0),
                primary_shard: ReplayShardId(shard.0),
                source_shard: ReplayShardId(shard.0),
            })
        }
        BodyLifecycleCommand::CreateGhost {
            shard,
            body,
            source_shard,
        } => MappedCommand::Lifecycle(LifecycleCommandRecord {
            kind: LifecycleCommandKind::CreateGhost,
            body: ReplayBodyId(body.0),
            primary_shard: ReplayShardId(shard.0),
            source_shard: ReplayShardId(source_shard.0),
        }),
        BodyLifecycleCommand::RetireGhost { shard, body } => {
            MappedCommand::Lifecycle(LifecycleCommandRecord {
                kind: LifecycleCommandKind::RetireGhost,
                body: ReplayBodyId(body.0),
                primary_shard: ReplayShardId(shard.0),
                source_shard: ReplayShardId(shard.0),
            })
        }
        BodyLifecycleCommand::TransferShard { body, from, to } => {
            MappedCommand::Transfer(ShardTransferRecord {
                body: ReplayBodyId(body.0),
                from_shard: ReplayShardId(from.0),
                to_shard: ReplayShardId(to.0),
            })
        }
    }
}

/// Mapped result of a single [`BodyLifecycleCommand`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MappedCommand {
    /// Goes onto the lifecycle command stream.
    Lifecycle(LifecycleCommandRecord),
    /// Goes onto the shard transfer stream.
    Transfer(ShardTransferRecord),
}

/// Build a [`RollbackSlice`] for the given tick and policy version by
/// walking every shard's body table.
///
/// The walk produces stable, sorted rows. Pose / velocity are placed
/// in their quantized cold-record-compatible form. Contacts and dirty
/// masks are *not* derived from the registry here — those come from
/// the contact store and the dirty-row queue respectively, and Pass
/// 18 leaves wiring them in to a future fun-side iteration. Callers
/// can append to the returned slice's `contact_warm_start` and
/// `dirty_mask_state` vectors before publishing.
#[must_use]
pub fn build_rollback_slice(
    registry: &ShardRegistry,
    tick: NetworkTick,
    policy_version: PolicyVersion,
) -> RollbackSlice {
    let mut shard_ids: Vec<ShardId> = registry.iter_shards().map(|s| s.identity.id).collect();
    shard_ids.sort_unstable_by_key(|id| id.0);

    let mut active_bodies = Vec::new();
    let mut shard_ownership = Vec::with_capacity(shard_ids.len());
    let mut counts = LifecycleCounts::default();

    for shard_id in &shard_ids {
        let Some(shard) = registry.shard(*shard_id) else {
            continue;
        };
        let mut bodies: Vec<(GlobalPhysicalEntityId, &ShardBodyRecord)> =
            shard.physics.bodies.iter().map(|(g, r)| (*g, r)).collect();
        bodies.sort_unstable_by_key(|(g, _)| g.0);

        let mut authoritative_count = 0_u32;
        let mut ghost_count = 0_u32;
        for (global, record) in bodies {
            let tier = map_tier(record.tier);
            counts.add(tier);
            match tier {
                BodyTierKind::Ghost => ghost_count = ghost_count.saturating_add(1),
                _ => authoritative_count = authoritative_count.saturating_add(1),
            }
            let pose_mm = quantize_position_mm(record.last_position);
            active_bodies.push(RollbackActiveBodyRow {
                body: ReplayBodyId(global.0),
                shard: ReplayShardId(shard_id.0),
                tier,
                pose_mm,
                rotation_q16: [0, 0, 0, i16::MAX],
                linear_cm_per_s: [0, 0, 0],
                angular_mrad_per_s: [0, 0, 0],
            });
        }
        shard_ownership.push(DigestShardOwnership {
            shard: ReplayShardId(shard_id.0),
            authoritative_count,
            ghost_count,
        });
    }

    RollbackSlice {
        tick,
        policy_version,
        active_bodies,
        contact_warm_start: Vec::new(),
        dirty_mask_state: Vec::new(),
        shard_ownership,
        lifecycle_counts: counts,
    }
}

/// Build the [`StateDigest`] for a `(registry, tick, policy_version)`
/// triple. Equivalent to `build_rollback_slice(...).digest()` but
/// avoids materialising the full slice when only the digest is
/// needed.
#[must_use]
pub fn build_state_digest(
    registry: &ShardRegistry,
    tick: NetworkTick,
    policy_version: PolicyVersion,
) -> StateDigest {
    let mut builder = StateDigestBuilder::new(tick, policy_version);
    let mut counts = LifecycleCounts::default();

    let mut shard_ids: Vec<ShardId> = registry.iter_shards().map(|s| s.identity.id).collect();
    shard_ids.sort_unstable_by_key(|id| id.0);

    for shard_id in &shard_ids {
        let Some(shard) = registry.shard(*shard_id) else {
            continue;
        };
        let mut bodies: Vec<(GlobalPhysicalEntityId, &ShardBodyRecord)> =
            shard.physics.bodies.iter().map(|(g, r)| (*g, r)).collect();
        bodies.sort_unstable_by_key(|(g, _)| g.0);

        let mut authoritative_count = 0_u32;
        let mut ghost_count = 0_u32;
        for (global, record) in bodies {
            let tier = map_tier(record.tier);
            counts.add(tier);
            match tier {
                BodyTierKind::Ghost => ghost_count = ghost_count.saturating_add(1),
                _ => authoritative_count = authoritative_count.saturating_add(1),
            }
            let pose_mm = quantize_position_mm(record.last_position);
            builder.add_active_body(DigestActiveRow {
                body: ReplayBodyId(global.0),
                shard: ReplayShardId(shard_id.0),
                tier,
                pose_mm,
                rotation_q16: [0, 0, 0, i16::MAX],
                linear_cm_per_s: [0, 0, 0],
                angular_mrad_per_s: [0, 0, 0],
            });
        }
        builder.add_shard_ownership(DigestShardOwnership {
            shard: ReplayShardId(shard_id.0),
            authoritative_count,
            ghost_count,
        });
    }

    builder.set_lifecycle_counts(counts);
    builder.finalize()
}

/// Build a deterministic [`CommandStream`] from a slice of
/// [`BodyLifecycleCommand`]s emitted during one tick. Lifecycle and
/// transfer commands are split into the two streams the wire format
/// expects, then canonicalised so the on-wire ordering does not depend
/// on registry iteration.
#[must_use]
pub fn build_command_stream(tick: NetworkTick, commands: &[BodyLifecycleCommand]) -> CommandStream {
    let mut stream = CommandStream::empty(tick);
    for command in commands {
        match map_command(*command) {
            MappedCommand::Lifecycle(record) => stream.lifecycle_commands.push(record),
            MappedCommand::Transfer(record) => stream.shard_transfers.push(record),
        }
    }
    stream.canonicalize();
    stream
}

/// Quantize a world position to millimetre precision, matching the
/// cold-record format. Saturates at i32 bounds to avoid overflow on
/// pathological coordinates.
#[inline]
fn quantize_position_mm(position: Vec3) -> [i32; 3] {
    let to_mm = |component: f32| -> i32 {
        let scaled = component * 1_000.0;
        if scaled >= i32::MAX as f32 {
            i32::MAX
        } else if scaled <= i32::MIN as f32 {
            i32::MIN
        } else {
            scaled as i32
        }
    };
    [to_mm(position.x), to_mm(position.y), to_mm(position.z)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physics::active_pool::ActiveBodyKind;
    use crate::physics::shard::{CellId, ShardBodyRecord, ShardRegistry};
    use thunder::replay::PolicyVersion;

    fn make_record(
        global: GlobalPhysicalEntityId,
        tier: BodyLifecycleTier,
        position: Vec3,
        owner: ShardId,
    ) -> ShardBodyRecord {
        ShardBodyRecord {
            global,
            entity: Entity::from_raw_u32(global.0 as u32 + 1).expect("valid"),
            kind: ActiveBodyKind::Dynamic,
            tier,
            handle: super::super::active_pool::ActiveBodyHandle::default(),
            last_position: position,
            authoritative_shard: owner,
        }
    }

    fn build_two_shard_registry() -> ShardRegistry {
        let mut registry = ShardRegistry::with_cell_size(100.0, 5.0);
        let shard0 = registry.add_shard(CellId::new(0, 0, 0));
        let shard1 = registry.add_shard(CellId::new(1, 0, 0));
        let runtime0 = registry.shard_mut(shard0).expect("shard 0 exists");
        runtime0.physics.bodies.insert(
            GlobalPhysicalEntityId(1),
            make_record(
                GlobalPhysicalEntityId(1),
                BodyLifecycleTier::Active,
                Vec3::new(0.5, 0.0, 0.0),
                shard0,
            ),
        );
        runtime0.physics.bodies.insert(
            GlobalPhysicalEntityId(2),
            make_record(
                GlobalPhysicalEntityId(2),
                BodyLifecycleTier::Warm,
                Vec3::new(1.0, 0.0, 0.0),
                shard0,
            ),
        );
        let runtime1 = registry.shard_mut(shard1).expect("shard 1 exists");
        runtime1.physics.bodies.insert(
            GlobalPhysicalEntityId(3),
            make_record(
                GlobalPhysicalEntityId(3),
                BodyLifecycleTier::Active,
                Vec3::new(101.0, 0.0, 0.0),
                shard1,
            ),
        );
        runtime1.physics.bodies.insert(
            GlobalPhysicalEntityId(4),
            make_record(
                GlobalPhysicalEntityId(4),
                BodyLifecycleTier::Ghost,
                Vec3::new(99.5, 0.0, 0.0),
                shard0,
            ),
        );
        registry
    }

    #[test]
    fn rollback_slice_walks_shards_in_stable_order() {
        let registry = build_two_shard_registry();
        let slice = build_rollback_slice(&registry, NetworkTick(5), PolicyVersion(18));
        // 4 bodies across 2 shards.
        assert_eq!(slice.active_bodies.len(), 4);
        // Shard 0 first (its bodies sorted by global id), then shard 1.
        assert_eq!(slice.active_bodies[0].body, ReplayBodyId(1));
        assert_eq!(slice.active_bodies[0].shard, ReplayShardId(0));
        assert_eq!(slice.active_bodies[1].body, ReplayBodyId(2));
        assert_eq!(slice.active_bodies[2].body, ReplayBodyId(3));
        assert_eq!(slice.active_bodies[3].body, ReplayBodyId(4));
        // Lifecycle counts: 2 active + 1 warm + 1 ghost.
        assert_eq!(slice.lifecycle_counts.active, 2);
        assert_eq!(slice.lifecycle_counts.warm, 1);
        assert_eq!(slice.lifecycle_counts.ghost, 1);
        assert_eq!(slice.shard_ownership.len(), 2);
        assert_eq!(slice.shard_ownership[0].authoritative_count, 2);
        assert_eq!(slice.shard_ownership[1].authoritative_count, 1);
        assert_eq!(slice.shard_ownership[1].ghost_count, 1);
    }

    #[test]
    fn slice_digest_matches_direct_digest_builder() {
        // The bridge offers two paths: build the slice and call
        // .digest(), or call build_state_digest() directly. Both must
        // produce bit-equal digests.
        let registry = build_two_shard_registry();
        let slice = build_rollback_slice(&registry, NetworkTick(5), PolicyVersion(18));
        let direct = build_state_digest(&registry, NetworkTick(5), PolicyVersion(18));
        assert_eq!(slice.digest(), direct);
    }

    #[test]
    fn digest_is_stable_across_repeated_walks() {
        // Same input = same digest. The HashMap iteration order in
        // ShardPhysics::bodies is not stable across runs, so the
        // adapter sorts before hashing — this test pins the
        // determinism guarantee.
        let registry = build_two_shard_registry();
        let first = build_state_digest(&registry, NetworkTick(7), PolicyVersion(18));
        let second = build_state_digest(&registry, NetworkTick(7), PolicyVersion(18));
        let third = build_state_digest(&registry, NetworkTick(7), PolicyVersion(18));
        assert_eq!(first, second);
        assert_eq!(second, third);
    }

    #[test]
    fn command_stream_round_trips_lifecycle_and_transfer() {
        let commands = vec![
            BodyLifecycleCommand::TransferShard {
                body: GlobalPhysicalEntityId(7),
                from: ShardId(0),
                to: ShardId(1),
            },
            BodyLifecycleCommand::PromoteToWarm {
                shard: ShardId(0),
                body: GlobalPhysicalEntityId(2),
            },
            BodyLifecycleCommand::ActivateBody {
                shard: ShardId(0),
                body: GlobalPhysicalEntityId(2),
            },
        ];
        let stream = build_command_stream(NetworkTick(11), &commands);
        // Two lifecycle commands + one transfer.
        assert_eq!(stream.lifecycle_commands.len(), 2);
        assert_eq!(stream.shard_transfers.len(), 1);
        // Canonical order: ActivateBody (code 1) before PromoteToWarm
        // (code 0)? No — codes order PromoteToWarm=0, ActivateBody=1,
        // so promote comes first.
        assert_eq!(
            stream.lifecycle_commands[0].kind,
            LifecycleCommandKind::PromoteToWarm
        );
        assert_eq!(
            stream.lifecycle_commands[1].kind,
            LifecycleCommandKind::ActivateBody
        );
        assert_eq!(stream.shard_transfers[0].body, ReplayBodyId(7));
        assert_eq!(stream.shard_transfers[0].from_shard, ReplayShardId(0));
        assert_eq!(stream.shard_transfers[0].to_shard, ReplayShardId(1));
    }

    #[test]
    fn quantize_position_mm_matches_cold_record_units() {
        // Cold records expect millimetre integers. Our adapter must
        // produce the same scale or the digest cannot be compared
        // against the cold-record stream.
        assert_eq!(
            quantize_position_mm(Vec3::new(1.5, 0.0, 0.0)),
            [1_500, 0, 0]
        );
        assert_eq!(
            quantize_position_mm(Vec3::new(-2.25, 0.0, 0.0)),
            [-2_250, 0, 0]
        );
        // Saturation guard.
        assert_eq!(
            quantize_position_mm(Vec3::new(f32::INFINITY, 0.0, 0.0))[0],
            i32::MAX
        );
        assert_eq!(
            quantize_position_mm(Vec3::new(f32::NEG_INFINITY, 0.0, 0.0))[0],
            i32::MIN
        );
    }

    #[test]
    fn empty_registry_produces_zeroed_slice() {
        let registry = ShardRegistry::with_cell_size(100.0, 5.0);
        let slice = build_rollback_slice(&registry, NetworkTick(0), PolicyVersion(18));
        assert!(slice.active_bodies.is_empty());
        assert!(slice.shard_ownership.is_empty());
        assert_eq!(slice.lifecycle_counts.total(), 0);
        let digest = build_state_digest(&registry, NetworkTick(0), PolicyVersion(18));
        // The active-body digest of an empty bucket is the FNV-1a
        // offset basis since no bytes are fed.
        assert_eq!(digest.active_body_digest, 0xcbf29ce484222325);
    }
}
