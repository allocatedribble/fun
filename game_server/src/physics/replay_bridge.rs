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
            let rotation_q16 = quantize_rotation_q16(record.last_rotation);
            let linear_cm_per_s = quantize_linear_velocity_cm_per_s(record.last_linear_velocity);
            let angular_mrad_per_s =
                quantize_angular_velocity_mrad_per_s(record.last_angular_velocity);
            active_bodies.push(RollbackActiveBodyRow {
                body: ReplayBodyId(global.0),
                shard: ReplayShardId(shard_id.0),
                tier,
                pose_mm,
                rotation_q16,
                linear_cm_per_s,
                angular_mrad_per_s,
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
            let rotation_q16 = quantize_rotation_q16(record.last_rotation);
            let linear_cm_per_s = quantize_linear_velocity_cm_per_s(record.last_linear_velocity);
            let angular_mrad_per_s =
                quantize_angular_velocity_mrad_per_s(record.last_angular_velocity);
            builder.add_active_body(DigestActiveRow {
                body: ReplayBodyId(global.0),
                shard: ReplayShardId(shard_id.0),
                tier,
                pose_mm,
                rotation_q16,
                linear_cm_per_s,
                angular_mrad_per_s,
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

/// Quantize a unit quaternion to a 16-bit-packed [x, y, z, w]. Each
/// component is scaled into [-i16::MAX, i16::MAX] so a full rotation
/// resolves to ~6e-5 radian precision per component — comfortably
/// finer than the cold-record format expects. Saturates on out-of-
/// range inputs (e.g. malformed unit quaternions) so the digest stays
/// well-defined.
#[inline]
fn quantize_rotation_q16(rotation: Quat) -> [i16; 4] {
    let to_q16 = |component: f32| -> i16 {
        let clamped = component.clamp(-1.0, 1.0);
        let scaled = clamped * (i16::MAX as f32);
        if scaled.is_nan() {
            0
        } else if scaled >= i16::MAX as f32 {
            i16::MAX
        } else if scaled <= i16::MIN as f32 {
            i16::MIN
        } else {
            scaled as i16
        }
    };
    [
        to_q16(rotation.x),
        to_q16(rotation.y),
        to_q16(rotation.z),
        to_q16(rotation.w),
    ]
}

/// Quantize a linear velocity (m/s) to centimetres-per-second. i16 covers
/// roughly ±327 m/s, plenty for gameplay; saturates outside that range.
#[inline]
fn quantize_linear_velocity_cm_per_s(velocity: Vec3) -> [i16; 3] {
    let to_i16 = |component: f32| -> i16 {
        let scaled = component * 100.0;
        if scaled.is_nan() {
            0
        } else if scaled >= i16::MAX as f32 {
            i16::MAX
        } else if scaled <= i16::MIN as f32 {
            i16::MIN
        } else {
            scaled as i16
        }
    };
    [to_i16(velocity.x), to_i16(velocity.y), to_i16(velocity.z)]
}

/// Quantize an angular velocity (rad/s) to milliradians-per-second.
/// i16 covers roughly ±32 rad/s, sufficient for any realistic body.
#[inline]
fn quantize_angular_velocity_mrad_per_s(velocity: Vec3) -> [i16; 3] {
    let to_i16 = |component: f32| -> i16 {
        let scaled = component * 1_000.0;
        if scaled.is_nan() {
            0
        } else if scaled >= i16::MAX as f32 {
            i16::MAX
        } else if scaled <= i16::MIN as f32 {
            i16::MIN
        } else {
            scaled as i16
        }
    };
    [to_i16(velocity.x), to_i16(velocity.y), to_i16(velocity.z)]
}

// ============================================================================
// V2-P6: Replay evidence bundle + diff comparison
// ----------------------------------------------------------------------------
// Compact, comparable summary of one tick's replay-relevant state. Used
// by the rollback / desync detector to localise *where* two replays
// diverged: a digest mismatch alone says "they diverged"; an evidence
// bundle diff says "shard 3 has 12 active bodies on side A and 11 on
// side B, and the lifecycle ghost count differs by 1".
//
// The bundle is intentionally narrow:
// - Active body count and digest (whole-active-body bucket only).
// - Contact count and digest (filled by callers; this module does not
//   walk avian's contact store directly).
// - Lifecycle counts (cold/warm/active/ghost/aggregate).
// - Per-shard ownership counts (authoritative + ghost).
//
// Particles + destruction land in follow-up passes per the V2-P6 spec
// task list ("particles later, destruction later").
// ============================================================================

use thunder::hash::fnv1a64;

/// Summary of a tick's contact state. Filled by the caller so the
/// evidence bundle does not have to know about avian's contact store
/// shape. The digest is FNV-1a 64 over the canonicalised contact byte
/// stream; equal `(count, digest)` means the contacts agree.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct ReplayContactEvidence {
    /// Number of active contacts the caller observed this tick.
    pub active_contact_count: u32,
    /// Number of sleeping contacts the caller observed this tick.
    pub sleeping_contact_count: u32,
    /// Caller-supplied digest of the canonical contact byte stream.
    pub contact_digest: u64,
}

/// Tick-scoped evidence bundle. Two bundles produced from the same
/// `(registry, tick, contacts)` triple compare equal; a non-empty
/// [`EvidenceBundleDiff`] from [`compare_evidence_bundles`] localises
/// the divergence.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReplayEvidenceBundle {
    /// Tick the bundle was captured at.
    pub tick: NetworkTick,
    /// Policy-version snapshot the bundle was captured against.
    pub policy_version: PolicyVersion,
    /// Total number of active bodies (across all shards).
    pub active_body_count: u32,
    /// FNV-1a 64 over the canonicalised active-body byte stream.
    pub active_body_digest: u64,
    /// Aggregate lifecycle counts (cold / warm / active / ghost /
    /// aggregate).
    pub lifecycle_counts: LifecycleCounts,
    /// Per-shard ownership rows in shard-id order.
    pub shard_ownership: Vec<DigestShardOwnership>,
    /// Caller-supplied contact evidence (this module does not walk
    /// the contact store directly — see [`ReplayContactEvidence`]).
    pub contact_evidence: ReplayContactEvidence,
}

impl ReplayEvidenceBundle {
    /// Total bodies across every lifecycle tier. Convenience accessor
    /// so callers do not have to round-trip through
    /// [`LifecycleCounts::total`].
    pub fn total_bodies(&self) -> u32 {
        self.lifecycle_counts.total()
    }
}

/// Build a [`ReplayEvidenceBundle`] from a `(registry, tick,
/// policy_version, contacts)` quad. Walks the registry once, building
/// the bundle's lifecycle counts + shard ownership rows + active-body
/// digest in the same canonical order as
/// [`build_state_digest`] so two bundles produced from the same input
/// are byte-for-byte equal.
#[must_use]
pub fn build_evidence_bundle(
    registry: &ShardRegistry,
    tick: NetworkTick,
    policy_version: PolicyVersion,
    contact_evidence: ReplayContactEvidence,
) -> ReplayEvidenceBundle {
    // Reuse the canonical state digest so the active-body digest
    // matches what the rollback slice publishes — there must be one
    // canonical hash per bucket.
    let digest = build_state_digest(registry, tick, policy_version);
    let mut shard_ids: Vec<ShardId> = registry.iter_shards().map(|s| s.identity.id).collect();
    shard_ids.sort_unstable_by_key(|id| id.0);

    let mut shard_ownership = Vec::with_capacity(shard_ids.len());
    let mut counts = LifecycleCounts::default();
    let mut active_body_count = 0_u32;
    for shard_id in &shard_ids {
        let Some(shard) = registry.shard(*shard_id) else {
            continue;
        };
        let mut authoritative_count = 0_u32;
        let mut ghost_count = 0_u32;
        let mut bodies: Vec<(GlobalPhysicalEntityId, &ShardBodyRecord)> =
            shard.physics.bodies.iter().map(|(g, r)| (*g, r)).collect();
        bodies.sort_unstable_by_key(|(g, _)| g.0);
        for (_global, record) in bodies {
            let tier = map_tier(record.tier);
            counts.add(tier);
            active_body_count = active_body_count.saturating_add(1);
            match tier {
                BodyTierKind::Ghost => ghost_count = ghost_count.saturating_add(1),
                _ => authoritative_count = authoritative_count.saturating_add(1),
            }
        }
        shard_ownership.push(DigestShardOwnership {
            shard: ReplayShardId(shard_id.0),
            authoritative_count,
            ghost_count,
        });
    }

    ReplayEvidenceBundle {
        tick,
        policy_version,
        active_body_count,
        active_body_digest: digest.active_body_digest,
        lifecycle_counts: counts,
        shard_ownership,
        contact_evidence,
    }
}

/// Per-bucket diff between two [`ReplayEvidenceBundle`]s. Each field
/// is `Some(_)` when the two sides disagree on that bucket. A bundle
/// where every field is `None` means the two replays are bit-for-bit
/// consistent on the V2-P6 surface.
///
/// The diff is intentionally narrow — it carries the buckets a
/// caller needs to localise the divergence, not the whole replay
/// state. Particles + destruction are reserved for follow-up passes
/// per the V2-P6 spec task list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EvidenceBundleDiff {
    /// Active-body count divergence, `(left, right)`.
    pub active_body_count: Option<(u32, u32)>,
    /// Active-body digest divergence, `(left, right)`.
    pub active_body_digest: Option<(u64, u64)>,
    /// Lifecycle-counts divergence, `(left, right)`.
    pub lifecycle_counts: Option<(LifecycleCounts, LifecycleCounts)>,
    /// Per-shard ownership rows that disagree, in shard-id order.
    /// Each entry is `(shard_id, left, right)` where `left` /
    /// `right` are `Option<DigestShardOwnership>` so the caller can
    /// distinguish "shard exists on left only" from "row contents
    /// differ".
    pub shard_ownership_divergences: Vec<ShardOwnershipDivergence>,
    /// Contact-evidence divergence, `(left, right)`.
    pub contact_evidence: Option<(ReplayContactEvidence, ReplayContactEvidence)>,
    /// Tick divergence, `(left, right)`. Set when the bundles were
    /// captured at different ticks (caller error in most flows).
    pub tick: Option<(NetworkTick, NetworkTick)>,
    /// Policy-version divergence, `(left, right)`. Set when the
    /// bundles were captured against different policy versions.
    pub policy_version: Option<(PolicyVersion, PolicyVersion)>,
}

impl EvidenceBundleDiff {
    /// True if every bucket compared equal — the two replays are
    /// V2-P6-consistent.
    pub fn is_empty(&self) -> bool {
        self.active_body_count.is_none()
            && self.active_body_digest.is_none()
            && self.lifecycle_counts.is_none()
            && self.shard_ownership_divergences.is_empty()
            && self.contact_evidence.is_none()
            && self.tick.is_none()
            && self.policy_version.is_none()
    }
}

/// One per-shard ownership row that differs between two evidence
/// bundles. `left` / `right` are `Option<_>` so a caller can tell
/// "shard present on one side only" apart from "row contents differ".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShardOwnershipDivergence {
    pub shard: ReplayShardId,
    pub left: Option<DigestShardOwnership>,
    pub right: Option<DigestShardOwnership>,
}

/// Pure comparison: walks both bundles, returns the bucket-localised
/// diff. Both bundles must be produced by [`build_evidence_bundle`];
/// the function does not re-canonicalise anything.
#[must_use]
pub fn compare_evidence_bundles(
    left: &ReplayEvidenceBundle,
    right: &ReplayEvidenceBundle,
) -> EvidenceBundleDiff {
    let mut diff = EvidenceBundleDiff::default();
    if left.tick != right.tick {
        diff.tick = Some((left.tick, right.tick));
    }
    if left.policy_version != right.policy_version {
        diff.policy_version = Some((left.policy_version, right.policy_version));
    }
    if left.active_body_count != right.active_body_count {
        diff.active_body_count = Some((left.active_body_count, right.active_body_count));
    }
    if left.active_body_digest != right.active_body_digest {
        diff.active_body_digest = Some((left.active_body_digest, right.active_body_digest));
    }
    if left.lifecycle_counts != right.lifecycle_counts {
        diff.lifecycle_counts = Some((left.lifecycle_counts, right.lifecycle_counts));
    }
    if left.contact_evidence != right.contact_evidence {
        diff.contact_evidence = Some((left.contact_evidence, right.contact_evidence));
    }

    // Shard ownership diff: walk a sorted union of shard ids.
    let mut shards: Vec<ReplayShardId> = left
        .shard_ownership
        .iter()
        .map(|row| row.shard)
        .chain(right.shard_ownership.iter().map(|row| row.shard))
        .collect();
    shards.sort_unstable_by_key(|s| s.0);
    shards.dedup();
    for shard in shards {
        let l = left
            .shard_ownership
            .iter()
            .find(|row| row.shard == shard)
            .copied();
        let r = right
            .shard_ownership
            .iter()
            .find(|row| row.shard == shard)
            .copied();
        if l != r {
            diff.shard_ownership_divergences
                .push(ShardOwnershipDivergence {
                    shard,
                    left: l,
                    right: r,
                });
        }
    }
    diff
}

/// Convenience: hash an arbitrary contact byte stream into a
/// `contact_digest`. Callers serialise their contact state in
/// canonical order then call this so [`ReplayEvidenceBundle`]s
/// generated by different code paths agree on the hashing
/// convention.
#[must_use]
pub fn contact_digest(canonical_bytes: &[u8]) -> u64 {
    fnv1a64(canonical_bytes)
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
            last_rotation: Quat::IDENTITY,
            last_linear_velocity: Vec3::ZERO,
            last_angular_velocity: Vec3::ZERO,
            authoritative_shard: owner,
        }
    }

    fn make_dynamic_record(
        global: GlobalPhysicalEntityId,
        tier: BodyLifecycleTier,
        position: Vec3,
        rotation: Quat,
        linear_velocity: Vec3,
        angular_velocity: Vec3,
        owner: ShardId,
    ) -> ShardBodyRecord {
        ShardBodyRecord {
            global,
            entity: Entity::from_raw_u32(global.0 as u32 + 1).expect("valid"),
            kind: ActiveBodyKind::Dynamic,
            tier,
            handle: super::super::active_pool::ActiveBodyHandle::default(),
            last_position: position,
            last_rotation: rotation,
            last_linear_velocity: linear_velocity,
            last_angular_velocity: angular_velocity,
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
    fn rotation_difference_produces_distinct_digests() {
        // Audit fix #3: before this pass `replay_bridge` hard-coded
        // rotation to [0, 0, 0, i16::MAX] and velocities to zero, so
        // two bodies that differed only in orientation produced
        // identical digests and the "rollback replay stable" invariant
        // passed vacuously. Now that ShardBodyRecord carries rotation,
        // the digest must diverge between two bodies with different
        // orientations.
        let mut registry_a = ShardRegistry::with_cell_size(100.0, 5.0);
        let shard_a = registry_a.add_shard(CellId::new(0, 0, 0));
        registry_a
            .shard_mut(shard_a)
            .expect("shard 0")
            .physics
            .bodies
            .insert(
                GlobalPhysicalEntityId(1),
                make_dynamic_record(
                    GlobalPhysicalEntityId(1),
                    BodyLifecycleTier::Active,
                    Vec3::new(1.0, 0.0, 0.0),
                    Quat::IDENTITY,
                    Vec3::ZERO,
                    Vec3::ZERO,
                    shard_a,
                ),
            );
        let mut registry_b = ShardRegistry::with_cell_size(100.0, 5.0);
        let shard_b = registry_b.add_shard(CellId::new(0, 0, 0));
        registry_b
            .shard_mut(shard_b)
            .expect("shard 0")
            .physics
            .bodies
            .insert(
                GlobalPhysicalEntityId(1),
                make_dynamic_record(
                    GlobalPhysicalEntityId(1),
                    BodyLifecycleTier::Active,
                    Vec3::new(1.0, 0.0, 0.0),
                    Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
                    Vec3::ZERO,
                    Vec3::ZERO,
                    shard_b,
                ),
            );
        let digest_a = build_state_digest(&registry_a, NetworkTick(1), PolicyVersion(20));
        let digest_b = build_state_digest(&registry_b, NetworkTick(1), PolicyVersion(20));
        assert_ne!(
            digest_a.active_body_digest, digest_b.active_body_digest,
            "rotation must contribute to the active-body digest"
        );
    }

    #[test]
    fn linear_velocity_difference_produces_distinct_digests() {
        // Same shape as the rotation test: changing only the linear
        // velocity must change the digest.
        let mut registry_a = ShardRegistry::with_cell_size(100.0, 5.0);
        let shard_a = registry_a.add_shard(CellId::new(0, 0, 0));
        registry_a
            .shard_mut(shard_a)
            .expect("shard 0")
            .physics
            .bodies
            .insert(
                GlobalPhysicalEntityId(1),
                make_dynamic_record(
                    GlobalPhysicalEntityId(1),
                    BodyLifecycleTier::Active,
                    Vec3::ZERO,
                    Quat::IDENTITY,
                    Vec3::new(2.0, 0.0, 0.0),
                    Vec3::ZERO,
                    shard_a,
                ),
            );
        let mut registry_b = ShardRegistry::with_cell_size(100.0, 5.0);
        let shard_b = registry_b.add_shard(CellId::new(0, 0, 0));
        registry_b
            .shard_mut(shard_b)
            .expect("shard 0")
            .physics
            .bodies
            .insert(
                GlobalPhysicalEntityId(1),
                make_dynamic_record(
                    GlobalPhysicalEntityId(1),
                    BodyLifecycleTier::Active,
                    Vec3::ZERO,
                    Quat::IDENTITY,
                    Vec3::new(2.5, 0.0, 0.0),
                    Vec3::ZERO,
                    shard_b,
                ),
            );
        let digest_a = build_state_digest(&registry_a, NetworkTick(1), PolicyVersion(20));
        let digest_b = build_state_digest(&registry_b, NetworkTick(1), PolicyVersion(20));
        assert_ne!(
            digest_a.active_body_digest, digest_b.active_body_digest,
            "linear velocity must contribute to the active-body digest"
        );
    }

    #[test]
    fn quantize_rotation_q16_round_trips_identity() {
        // Identity rotation should land at [0, 0, 0, i16::MAX].
        let q = quantize_rotation_q16(Quat::IDENTITY);
        assert_eq!(q, [0, 0, 0, i16::MAX]);
    }

    #[test]
    fn quantize_velocity_handles_nan_and_saturation() {
        // NaN must produce zero so the digest stays well-defined.
        assert_eq!(
            quantize_linear_velocity_cm_per_s(Vec3::new(f32::NAN, 0.0, 0.0))[0],
            0
        );
        // Out-of-range saturates rather than wrapping.
        assert_eq!(
            quantize_linear_velocity_cm_per_s(Vec3::new(1_000.0, 0.0, 0.0))[0],
            i16::MAX
        );
        assert_eq!(
            quantize_angular_velocity_mrad_per_s(Vec3::new(-1_000.0, 0.0, 0.0))[0],
            i16::MIN
        );
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

    // ---- V2-P6 replay evidence bundle tests --------------------------

    fn empty_contact_evidence() -> ReplayContactEvidence {
        ReplayContactEvidence::default()
    }

    #[test]
    fn evidence_bundle_two_runs_over_same_input_compare_equal() {
        // V2-P6 spec: the bundle's whole point is "two replays
        // produce identical bundles iff the V2-P6 surfaces agree".
        let registry = build_two_shard_registry();
        let a = build_evidence_bundle(
            &registry,
            NetworkTick(7),
            PolicyVersion(20),
            empty_contact_evidence(),
        );
        let b = build_evidence_bundle(
            &registry,
            NetworkTick(7),
            PolicyVersion(20),
            empty_contact_evidence(),
        );
        let diff = compare_evidence_bundles(&a, &b);
        assert!(diff.is_empty(), "identical inputs must compare equal");
        assert_eq!(a, b);
    }

    #[test]
    fn evidence_bundle_active_body_digest_matches_state_digest() {
        // The bundle's `active_body_digest` must equal the canonical
        // state-digest's active-body bucket — there is one canonical
        // hash per bucket, not two.
        let registry = build_two_shard_registry();
        let bundle = build_evidence_bundle(
            &registry,
            NetworkTick(3),
            PolicyVersion(22),
            empty_contact_evidence(),
        );
        let digest = build_state_digest(&registry, NetworkTick(3), PolicyVersion(22));
        assert_eq!(bundle.active_body_digest, digest.active_body_digest);
    }

    #[test]
    fn evidence_bundle_diff_localises_active_body_drift() {
        // Two registries differing only in one body's pose: the
        // bundle's active-body digest must diverge while lifecycle
        // counts + shard ownership agree.
        let mut registry_a = ShardRegistry::with_cell_size(100.0, 5.0);
        let shard_a = registry_a.add_shard(CellId::new(0, 0, 0));
        registry_a
            .shard_mut(shard_a)
            .expect("shard 0")
            .physics
            .bodies
            .insert(
                GlobalPhysicalEntityId(1),
                make_dynamic_record(
                    GlobalPhysicalEntityId(1),
                    BodyLifecycleTier::Active,
                    Vec3::new(1.0, 0.0, 0.0),
                    Quat::IDENTITY,
                    Vec3::ZERO,
                    Vec3::ZERO,
                    shard_a,
                ),
            );
        let mut registry_b = ShardRegistry::with_cell_size(100.0, 5.0);
        let shard_b = registry_b.add_shard(CellId::new(0, 0, 0));
        registry_b
            .shard_mut(shard_b)
            .expect("shard 0")
            .physics
            .bodies
            .insert(
                GlobalPhysicalEntityId(1),
                make_dynamic_record(
                    GlobalPhysicalEntityId(1),
                    BodyLifecycleTier::Active,
                    Vec3::new(1.5, 0.0, 0.0), // different pose
                    Quat::IDENTITY,
                    Vec3::ZERO,
                    Vec3::ZERO,
                    shard_b,
                ),
            );
        let a = build_evidence_bundle(
            &registry_a,
            NetworkTick(5),
            PolicyVersion(20),
            empty_contact_evidence(),
        );
        let b = build_evidence_bundle(
            &registry_b,
            NetworkTick(5),
            PolicyVersion(20),
            empty_contact_evidence(),
        );
        let diff = compare_evidence_bundles(&a, &b);
        assert!(!diff.is_empty(), "different poses must drive a diff");
        assert!(
            diff.active_body_digest.is_some(),
            "pose drift must surface as an active-body digest divergence"
        );
        assert!(
            diff.active_body_count.is_none(),
            "the body count is the same — no count divergence"
        );
        assert!(
            diff.lifecycle_counts.is_none(),
            "the lifecycle ladder is the same — no count divergence"
        );
        assert!(
            diff.shard_ownership_divergences.is_empty(),
            "ownership rows are the same — no ownership divergence"
        );
    }

    #[test]
    fn evidence_bundle_diff_localises_lifecycle_drift() {
        // Same body in both registries but in different tiers:
        // lifecycle counts diverge, body count stays the same.
        let mut registry_a = ShardRegistry::with_cell_size(100.0, 5.0);
        let shard_a = registry_a.add_shard(CellId::new(0, 0, 0));
        registry_a
            .shard_mut(shard_a)
            .expect("shard")
            .physics
            .bodies
            .insert(
                GlobalPhysicalEntityId(1),
                make_record(
                    GlobalPhysicalEntityId(1),
                    BodyLifecycleTier::Active,
                    Vec3::ZERO,
                    shard_a,
                ),
            );
        let mut registry_b = ShardRegistry::with_cell_size(100.0, 5.0);
        let shard_b = registry_b.add_shard(CellId::new(0, 0, 0));
        registry_b
            .shard_mut(shard_b)
            .expect("shard")
            .physics
            .bodies
            .insert(
                GlobalPhysicalEntityId(1),
                make_record(
                    GlobalPhysicalEntityId(1),
                    BodyLifecycleTier::Warm, // different tier
                    Vec3::ZERO,
                    shard_b,
                ),
            );
        let a = build_evidence_bundle(
            &registry_a,
            NetworkTick(5),
            PolicyVersion(20),
            empty_contact_evidence(),
        );
        let b = build_evidence_bundle(
            &registry_b,
            NetworkTick(5),
            PolicyVersion(20),
            empty_contact_evidence(),
        );
        let diff = compare_evidence_bundles(&a, &b);
        assert!(
            diff.lifecycle_counts.is_some(),
            "lifecycle drift must surface"
        );
        // Active-body digest also diverges because the tier byte
        // contributes to the per-body hash.
        assert!(diff.active_body_digest.is_some());
    }

    #[test]
    fn evidence_bundle_diff_localises_shard_ownership_drift() {
        // Two registries identical except registry_b has a body that
        // registry_a doesn't — the shard ownership counts diverge,
        // and the per-shard divergence is reported with both sides.
        let mut registry_a = ShardRegistry::with_cell_size(100.0, 5.0);
        let _shard_a = registry_a.add_shard(CellId::new(0, 0, 0));
        let mut registry_b = ShardRegistry::with_cell_size(100.0, 5.0);
        let shard_b = registry_b.add_shard(CellId::new(0, 0, 0));
        registry_b
            .shard_mut(shard_b)
            .expect("shard")
            .physics
            .bodies
            .insert(
                GlobalPhysicalEntityId(99),
                make_record(
                    GlobalPhysicalEntityId(99),
                    BodyLifecycleTier::Active,
                    Vec3::ZERO,
                    shard_b,
                ),
            );
        let a = build_evidence_bundle(
            &registry_a,
            NetworkTick(1),
            PolicyVersion(20),
            empty_contact_evidence(),
        );
        let b = build_evidence_bundle(
            &registry_b,
            NetworkTick(1),
            PolicyVersion(20),
            empty_contact_evidence(),
        );
        let diff = compare_evidence_bundles(&a, &b);
        assert!(!diff.is_empty());
        assert_eq!(diff.shard_ownership_divergences.len(), 1);
        let row = diff.shard_ownership_divergences[0];
        assert_eq!(row.shard, ReplayShardId(0));
        assert_eq!(row.left.expect("left present").authoritative_count, 0);
        assert_eq!(row.right.expect("right present").authoritative_count, 1);
    }

    #[test]
    fn evidence_bundle_diff_localises_contact_drift() {
        // Same registry, different contact evidence → contacts
        // bucket diverges only.
        let registry = build_two_shard_registry();
        let contacts_a = ReplayContactEvidence {
            active_contact_count: 4,
            sleeping_contact_count: 0,
            contact_digest: contact_digest(b"a"),
        };
        let contacts_b = ReplayContactEvidence {
            active_contact_count: 5, // different
            sleeping_contact_count: 0,
            contact_digest: contact_digest(b"b"),
        };
        let a = build_evidence_bundle(&registry, NetworkTick(2), PolicyVersion(20), contacts_a);
        let b = build_evidence_bundle(&registry, NetworkTick(2), PolicyVersion(20), contacts_b);
        let diff = compare_evidence_bundles(&a, &b);
        assert!(
            diff.contact_evidence.is_some(),
            "contact divergence must surface"
        );
        assert!(
            diff.active_body_digest.is_none(),
            "registry is the same — bodies must agree"
        );
    }

    #[test]
    fn evidence_bundle_total_bodies_matches_lifecycle_total() {
        let registry = build_two_shard_registry();
        let bundle = build_evidence_bundle(
            &registry,
            NetworkTick(0),
            PolicyVersion(20),
            empty_contact_evidence(),
        );
        // Two-shard fixture has 4 bodies (2 active + 1 warm + 1 ghost).
        assert_eq!(bundle.total_bodies(), 4);
        assert_eq!(bundle.active_body_count, 4);
        assert_eq!(bundle.lifecycle_counts.total(), bundle.total_bodies());
    }

    #[test]
    fn contact_digest_uses_thunder_canonical_hash() {
        // V2-P6 dedupe gate: there is one FNV-1a 64 implementation
        // (thunder::hash::fnv1a64). Pin this through the convenience
        // wrapper.
        assert_eq!(contact_digest(b"a"), thunder::hash::fnv1a64(b"a"));
        assert_eq!(contact_digest(b""), thunder::hash::FNV_OFFSET_64);
    }

    #[test]
    fn empty_evidence_bundle_diff_is_empty() {
        // Default-constructed bundles are equal trivially.
        let a = ReplayEvidenceBundle::default();
        let b = ReplayEvidenceBundle::default();
        let diff = compare_evidence_bundles(&a, &b);
        assert!(diff.is_empty());
    }
}
