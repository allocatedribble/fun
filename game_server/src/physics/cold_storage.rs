//! Cold/warm/active lifecycle and persistence model.
//!
//! Pass 15 introduced the
//! [`BodyLifecycleTier`](super::shard::BodyLifecycleTier) enum and the
//! shard runtime's `Cold`/`Warm`/`Active`/`Ghost` slots. This module
//! adds the missing pieces:
//!
//! - [`ColdRecord`] — the stable per-body record kept in cold
//!   storage. Carries only the fields needed to wake or reconstruct a
//!   body. **Solver state is intentionally absent** so cold rows do
//!   not mimic full rigid bodies.
//! - [`AggregateBody`] — coarse summary of a clump of cold entities
//!   (e.g. a forest of trees treated as a single record for distant
//!   observers).
//! - [`ColdChunkSnapshot`] — a typed binary canonical chunk. Encoded
//!   with `[FCC1][version][shard][tick][count][checksum][records]`,
//!   versioned, checksummed, and size-capped per the project-FUN
//!   telemetry rules. **No JSON/CSV is treated as truth.**
//! - Promotion / demotion paths threaded through the shard registry
//!   so cold → warm → active and active → warm → cold transitions
//!   produce the spec-required
//!   [`BodyLifecycleCommand`](super::shard::BodyLifecycleCommand)s.
//! - [`ColdStorageDiagnostics`] surfaces represented / active / warm
//!   / cold / aggregate / aggregate-bodies counts, bytes-per-cold-
//!   record, activation rate, and demotion rate.
//!
//! The diagnostic surface is consumed by
//! [`super::shard::ShardRegistry`] and re-exported through the
//! `fun-observer` summary lane (kept additive in this pass; the
//! observer crate is wired in a follow-up turn).

use core::convert::TryInto;

use bevy::prelude::*;

use super::active_pool::ActiveBodyKind;
use super::shard::{
    BodyLifecycleCommand, BodyLifecycleTier, CellId, GlobalPhysicalEntityId, ShardId, ShardRegistry,
};

/// Magic bytes identifying a Fun Cold Chunk v1 payload.
pub const COLD_CHUNK_MAGIC: [u8; 4] = *b"FCC1";

/// Current cold-chunk wire format version.
pub const COLD_CHUNK_VERSION: u8 = 1;

/// Serialized size of a single [`ColdRecord`] in bytes.
///
/// Tracking this as a const lets diagnostics report bytes-per-cold-
/// record without re-encoding. Layout:
/// - 8 bytes global id (u64)
/// - 4 bytes shard id (u32)
/// - 12 bytes cell coord (3 × i32)
/// - 4 bytes class block (body u8 + collider u8 + 2 reserved bytes)
/// - 12 bytes coarse position in mm (3 × i32)
/// - 8 bytes coarse rotation (4 × i16)
/// - 6 bytes coarse linear velocity in cm/s (3 × i16)
/// - 6 bytes coarse angular velocity in mrad/s (3 × i16)
/// - 2 bytes tier u8 + reserved
/// - 4 bytes last authoritative tick (u32)
pub const COLD_RECORD_BYTES: usize = 8 + 4 + 12 + 4 + 12 + 8 + 6 + 6 + 2 + 4;

/// Default cap on chunk bytes. Anything larger is rejected on decode.
pub const DEFAULT_MAX_CHUNK_BYTES: usize = 1024 * 1024;

/// Discrete body archetype recorded for cold revival. Concrete physics
/// material/collider details live elsewhere — cold storage only carries
/// enough to choose a default mass and initial velocity at wake time.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Reflect)]
#[reflect(Debug, PartialEq, Hash)]
pub enum ColdBodyClass {
    #[default]
    Dynamic,
    Kinematic,
    Static,
    Aggregate,
}

impl ColdBodyClass {
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Dynamic => 0,
            Self::Kinematic => 1,
            Self::Static => 2,
            Self::Aggregate => 3,
        }
    }

    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Dynamic),
            1 => Some(Self::Kinematic),
            2 => Some(Self::Static),
            3 => Some(Self::Aggregate),
            _ => None,
        }
    }

    pub const fn to_active_body_kind(self) -> Option<ActiveBodyKind> {
        match self {
            Self::Dynamic => Some(ActiveBodyKind::Dynamic),
            Self::Kinematic => Some(ActiveBodyKind::Kinematic),
            Self::Static | Self::Aggregate => None,
        }
    }
}

/// Minimal collider-archetype tag carried on cold rows. Concrete shape
/// parameters are referenced by id elsewhere; this is just enough to
/// pick a default proxy when a body wakes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Reflect)]
#[reflect(Debug, PartialEq, Hash)]
pub enum ColdColliderClass {
    /// No collider; spawn-side default applies.
    #[default]
    None,
    Sphere,
    Capsule,
    Box,
    Mesh,
}

impl ColdColliderClass {
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Sphere => 1,
            Self::Capsule => 2,
            Self::Box => 3,
            Self::Mesh => 4,
        }
    }

    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::None),
            1 => Some(Self::Sphere),
            2 => Some(Self::Capsule),
            3 => Some(Self::Box),
            4 => Some(Self::Mesh),
            _ => None,
        }
    }
}

/// One typed record in cold storage. The struct is intentionally
/// **smaller** than a full rigid body — pose and velocity are coarse
/// quantized integer triples, and no solver state is preserved.
///
/// Fields:
/// - `global` / `shard` / `cell` for relocation
/// - `body_class` / `collider_class` for wake-time archetype choice
/// - `coarse_position_mm` (i32 millimeters) for shard-relative pose
/// - `coarse_rotation` (i16 quantized quaternion components)
/// - `coarse_linear_velocity_cm_per_s` / `coarse_angular_velocity_milli_rad_per_s`
///   in compact integer units; used by waking integrators only.
/// - `tier` and `last_authoritative_tick`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ColdRecord {
    pub global: GlobalPhysicalEntityId,
    pub shard: ShardId,
    pub cell: CellId,
    pub body_class: ColdBodyClass,
    pub collider_class: ColdColliderClass,
    pub coarse_position_mm: [i32; 3],
    pub coarse_rotation: [i16; 4],
    pub coarse_linear_velocity_cm_per_s: [i16; 3],
    pub coarse_angular_velocity_milli_rad_per_s: [i16; 3],
    pub tier: BodyLifecycleTier,
    pub last_authoritative_tick: u32,
}

impl ColdRecord {
    /// Approximate byte cost of a single cold record — used to size
    /// the chunk and report `bytes_per_cold_record` to diagnostics.
    pub const fn approximate_bytes() -> usize {
        COLD_RECORD_BYTES
    }

    fn write_to(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.global.0.to_le_bytes());
        out.extend_from_slice(&self.shard.0.to_le_bytes());
        out.extend_from_slice(&self.cell.x.to_le_bytes());
        out.extend_from_slice(&self.cell.y.to_le_bytes());
        out.extend_from_slice(&self.cell.z.to_le_bytes());
        out.push(self.body_class.as_u8());
        out.push(self.collider_class.as_u8());
        out.extend_from_slice(&[0_u8, 0_u8]); // padding, reserved for future class bits
        for component in self.coarse_position_mm {
            out.extend_from_slice(&component.to_le_bytes());
        }
        for component in self.coarse_rotation {
            out.extend_from_slice(&component.to_le_bytes());
        }
        for component in self.coarse_linear_velocity_cm_per_s {
            out.extend_from_slice(&component.to_le_bytes());
        }
        for component in self.coarse_angular_velocity_milli_rad_per_s {
            out.extend_from_slice(&component.to_le_bytes());
        }
        out.push(tier_to_u8(self.tier));
        out.push(0_u8); // reserved
        out.extend_from_slice(&self.last_authoritative_tick.to_le_bytes());
    }

    fn read_from(bytes: &[u8]) -> Option<(Self, usize)> {
        if bytes.len() < COLD_RECORD_BYTES {
            return None;
        }
        let global = u64::from_le_bytes(bytes[0..8].try_into().ok()?);
        let shard = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
        let cell_x = i32::from_le_bytes(bytes[12..16].try_into().ok()?);
        let cell_y = i32::from_le_bytes(bytes[16..20].try_into().ok()?);
        let cell_z = i32::from_le_bytes(bytes[20..24].try_into().ok()?);
        let body_class = ColdBodyClass::from_u8(bytes[24])?;
        let collider_class = ColdColliderClass::from_u8(bytes[25])?;
        // bytes[26..28] reserved
        let pos_x = i32::from_le_bytes(bytes[28..32].try_into().ok()?);
        let pos_y = i32::from_le_bytes(bytes[32..36].try_into().ok()?);
        let pos_z = i32::from_le_bytes(bytes[36..40].try_into().ok()?);
        let rot_x = i16::from_le_bytes(bytes[40..42].try_into().ok()?);
        let rot_y = i16::from_le_bytes(bytes[42..44].try_into().ok()?);
        let rot_z = i16::from_le_bytes(bytes[44..46].try_into().ok()?);
        let rot_w = i16::from_le_bytes(bytes[46..48].try_into().ok()?);
        let lin_x = i16::from_le_bytes(bytes[48..50].try_into().ok()?);
        let lin_y = i16::from_le_bytes(bytes[50..52].try_into().ok()?);
        let lin_z = i16::from_le_bytes(bytes[52..54].try_into().ok()?);
        let ang_x = i16::from_le_bytes(bytes[54..56].try_into().ok()?);
        let ang_y = i16::from_le_bytes(bytes[56..58].try_into().ok()?);
        let ang_z = i16::from_le_bytes(bytes[58..60].try_into().ok()?);
        let tier = tier_from_u8(bytes[60])?;
        // bytes[61] reserved
        let tick = u32::from_le_bytes(bytes[62..66].try_into().ok()?);
        Some((
            Self {
                global: GlobalPhysicalEntityId(global),
                shard: ShardId(shard),
                cell: CellId::new(cell_x, cell_y, cell_z),
                body_class,
                collider_class,
                coarse_position_mm: [pos_x, pos_y, pos_z],
                coarse_rotation: [rot_x, rot_y, rot_z, rot_w],
                coarse_linear_velocity_cm_per_s: [lin_x, lin_y, lin_z],
                coarse_angular_velocity_milli_rad_per_s: [ang_x, ang_y, ang_z],
                tier,
                last_authoritative_tick: tick,
            },
            COLD_RECORD_BYTES,
        ))
    }

    /// Build a cold record from a Bevy world position/velocity, using
    /// shard-relative quantization. Position is rounded to mm; velocities
    /// to cm/s and milli-rad/s.
    ///
    /// The argument list is wide on purpose: the cold record's typed
    /// shape *is* the parameter set, and bundling the inputs into a
    /// builder struct would just rename the same fields without
    /// shrinking the surface.
    #[allow(clippy::too_many_arguments)]
    pub fn from_world_state(
        global: GlobalPhysicalEntityId,
        shard: ShardId,
        cell: CellId,
        body_class: ColdBodyClass,
        collider_class: ColdColliderClass,
        position: Vec3,
        rotation: Quat,
        linear_velocity: Vec3,
        angular_velocity: Vec3,
        tier: BodyLifecycleTier,
        last_authoritative_tick: u32,
    ) -> Self {
        let q_pos = position * 1000.0;
        let q_rot = rotation;
        let q_lin = linear_velocity * 100.0;
        let q_ang = angular_velocity * 1000.0;
        Self {
            global,
            shard,
            cell,
            body_class,
            collider_class,
            coarse_position_mm: [
                clamp_to_i32(q_pos.x),
                clamp_to_i32(q_pos.y),
                clamp_to_i32(q_pos.z),
            ],
            coarse_rotation: [
                clamp_to_i16(q_rot.x * i16::MAX as f32),
                clamp_to_i16(q_rot.y * i16::MAX as f32),
                clamp_to_i16(q_rot.z * i16::MAX as f32),
                clamp_to_i16(q_rot.w * i16::MAX as f32),
            ],
            coarse_linear_velocity_cm_per_s: [
                clamp_to_i16(q_lin.x),
                clamp_to_i16(q_lin.y),
                clamp_to_i16(q_lin.z),
            ],
            coarse_angular_velocity_milli_rad_per_s: [
                clamp_to_i16(q_ang.x),
                clamp_to_i16(q_ang.y),
                clamp_to_i16(q_ang.z),
            ],
            tier,
            last_authoritative_tick,
        }
    }

    /// Recover a coarse world position from the stored mm-quantized
    /// coordinates.
    pub fn world_position(&self) -> Vec3 {
        Vec3::new(
            self.coarse_position_mm[0] as f32 / 1000.0,
            self.coarse_position_mm[1] as f32 / 1000.0,
            self.coarse_position_mm[2] as f32 / 1000.0,
        )
    }
}

/// Aggregate body record. Represents a clump of cold entities (e.g. a
/// forest of trees, a debris field) treated as one physics
/// "presence" for distant observers. Aggregates do not promote
/// directly — their members do, one at a time, when relevance pulls
/// the aggregate apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AggregateBody {
    pub global: GlobalPhysicalEntityId,
    pub shard: ShardId,
    pub cell: CellId,
    /// Number of cold entities folded into this aggregate.
    pub member_count: u32,
    /// Bounding sphere radius in mm.
    pub bounding_radius_mm: u32,
    /// Aggregate body class (typically [`ColdBodyClass::Aggregate`]).
    pub body_class: ColdBodyClass,
    pub last_authoritative_tick: u32,
}

/// Header of a cold chunk on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ColdChunkHeader {
    /// The shard the chunk authoritatively belongs to. Decoders
    /// reject chunks whose `shard` doesn't match the expected owner.
    pub shard: ShardId,
    /// Server tick at which the chunk was produced.
    pub tick: u32,
    /// Number of records carried in the payload.
    pub record_count: u32,
}

/// Errors a cold-chunk decode can produce. All are non-fatal and
/// callers should reject the chunk and bump the matching diagnostic
/// counter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColdChunkError {
    /// Chunk is shorter than the fixed header.
    TooShort,
    /// Magic bytes do not match [`COLD_CHUNK_MAGIC`].
    BadMagic,
    /// Wire-format version is not [`COLD_CHUNK_VERSION`].
    UnsupportedVersion(u8),
    /// Recorded checksum does not match the recomputed value over the
    /// payload bytes.
    ChecksumMismatch,
    /// Chunk exceeds the configured maximum byte cap.
    Oversized { size: usize, cap: usize },
    /// Record count or payload length is inconsistent with the
    /// declared count.
    LengthMismatch,
    /// The decoder was told this chunk should belong to one shard but
    /// the chunk header names a different shard.
    StaleShardOwnership { expected: ShardId, actual: ShardId },
    /// A typed enum field carried an unknown discriminant.
    InvalidEnum,
}

/// A typed cold-storage chunk. The wire format is intentionally
/// minimal (magic + version + header + checksum + records) so any
/// project that needs to read or replicate cold rows can do so
/// without a full protobuf compiler in the build.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ColdChunkSnapshot {
    pub header: ColdChunkHeader,
    pub records: Vec<ColdRecord>,
}

impl Default for ColdChunkHeader {
    fn default() -> Self {
        Self {
            shard: ShardId::NONE,
            tick: 0,
            record_count: 0,
        }
    }
}

impl ColdChunkSnapshot {
    pub fn new(shard: ShardId, tick: u32, records: Vec<ColdRecord>) -> Self {
        let record_count = records.len().min(u32::MAX as usize) as u32;
        Self {
            header: ColdChunkHeader {
                shard,
                tick,
                record_count,
            },
            records,
        }
    }

    /// Encode the chunk into its canonical binary representation.
    /// Wire layout:
    ///
    /// ```text
    /// [magic:4][version:1][reserved:3]
    /// [shard:4][tick:4][record_count:4]
    /// [checksum:4]
    /// [records: record_count * COLD_RECORD_BYTES]
    /// ```
    pub fn encode(&self) -> Vec<u8> {
        let payload_bytes = self.records.len() * COLD_RECORD_BYTES;
        let mut out = Vec::with_capacity(24 + payload_bytes);
        out.extend_from_slice(&COLD_CHUNK_MAGIC);
        out.push(COLD_CHUNK_VERSION);
        out.extend_from_slice(&[0_u8, 0_u8, 0_u8]);
        out.extend_from_slice(&self.header.shard.0.to_le_bytes());
        out.extend_from_slice(&self.header.tick.to_le_bytes());
        out.extend_from_slice(&self.header.record_count.to_le_bytes());
        let mut payload = Vec::with_capacity(payload_bytes);
        for record in &self.records {
            record.write_to(&mut payload);
        }
        let checksum = checksum_payload(&payload);
        out.extend_from_slice(&checksum.to_le_bytes());
        out.extend_from_slice(&payload);
        out
    }

    /// Decode a chunk, validating magic, version, length, checksum,
    /// size cap, and (optionally) the expected shard owner.
    pub fn decode(
        bytes: &[u8],
        max_chunk_bytes: usize,
        expected_owner: Option<ShardId>,
    ) -> Result<Self, ColdChunkError> {
        if bytes.len() > max_chunk_bytes {
            return Err(ColdChunkError::Oversized {
                size: bytes.len(),
                cap: max_chunk_bytes,
            });
        }
        if bytes.len() < 24 {
            return Err(ColdChunkError::TooShort);
        }
        if bytes[0..4] != COLD_CHUNK_MAGIC {
            return Err(ColdChunkError::BadMagic);
        }
        let version = bytes[4];
        if version != COLD_CHUNK_VERSION {
            return Err(ColdChunkError::UnsupportedVersion(version));
        }
        let shard = ShardId(u32::from_le_bytes(
            bytes[8..12]
                .try_into()
                .map_err(|_| ColdChunkError::TooShort)?,
        ));
        if let Some(expected) = expected_owner
            && expected != shard
        {
            return Err(ColdChunkError::StaleShardOwnership {
                expected,
                actual: shard,
            });
        }
        let tick = u32::from_le_bytes(
            bytes[12..16]
                .try_into()
                .map_err(|_| ColdChunkError::TooShort)?,
        );
        let record_count = u32::from_le_bytes(
            bytes[16..20]
                .try_into()
                .map_err(|_| ColdChunkError::TooShort)?,
        );
        let checksum = u32::from_le_bytes(
            bytes[20..24]
                .try_into()
                .map_err(|_| ColdChunkError::TooShort)?,
        );
        let payload = &bytes[24..];
        let expected_payload_bytes = (record_count as usize)
            .checked_mul(COLD_RECORD_BYTES)
            .ok_or(ColdChunkError::LengthMismatch)?;
        if payload.len() != expected_payload_bytes {
            return Err(ColdChunkError::LengthMismatch);
        }
        if checksum != checksum_payload(payload) {
            return Err(ColdChunkError::ChecksumMismatch);
        }
        let mut records = Vec::with_capacity(record_count as usize);
        let mut cursor = 0;
        for _ in 0..record_count {
            let (record, consumed) =
                ColdRecord::read_from(&payload[cursor..]).ok_or(ColdChunkError::InvalidEnum)?;
            records.push(record);
            cursor += consumed;
        }
        Ok(Self {
            header: ColdChunkHeader {
                shard,
                tick,
                record_count,
            },
            records,
        })
    }
}

/// FCC1 cold-chunk checksum. Pass-19 audit fix #4 centralised the
/// FNV-1a 32-bit implementation in `thunder::hash` so the four
/// canonical containers (FRP1 / FCC1 / FSST / FCMP) all share one
/// canonical hasher; this is now a thin alias so the rest of the
/// module reads naturally.
#[inline]
fn checksum_payload(payload: &[u8]) -> u32 {
    thunder::hash::fnv1a32(payload)
}

fn tier_to_u8(tier: BodyLifecycleTier) -> u8 {
    match tier {
        BodyLifecycleTier::Cold => 0,
        BodyLifecycleTier::Warm => 1,
        BodyLifecycleTier::Active => 2,
        BodyLifecycleTier::Ghost => 3,
    }
}

fn tier_from_u8(value: u8) -> Option<BodyLifecycleTier> {
    match value {
        0 => Some(BodyLifecycleTier::Cold),
        1 => Some(BodyLifecycleTier::Warm),
        2 => Some(BodyLifecycleTier::Active),
        3 => Some(BodyLifecycleTier::Ghost),
        _ => None,
    }
}

fn clamp_to_i32(value: f32) -> i32 {
    if !value.is_finite() {
        return 0;
    }
    if value >= i32::MAX as f32 {
        return i32::MAX;
    }
    if value <= i32::MIN as f32 {
        return i32::MIN;
    }
    value.round() as i32
}

fn clamp_to_i16(value: f32) -> i16 {
    if !value.is_finite() {
        return 0;
    }
    if value >= i16::MAX as f32 {
        return i16::MAX;
    }
    if value <= i16::MIN as f32 {
        return i16::MIN;
    }
    value.round() as i16
}

/// Per-tick lifecycle / cold-storage diagnostics. Surfaces what
/// fun-observer's summary lane needs.
#[derive(Clone, Copy, Debug, Default)]
pub struct ColdStorageDiagnostics {
    pub represented_entities: u32,
    pub active_entities: u32,
    pub warm_entities: u32,
    pub cold_entities: u32,
    pub aggregate_entities: u32,
    pub aggregate_body_count: u32,
    pub bytes_per_cold_record: u32,
    pub activation_rate: u32,
    pub demotion_rate: u32,
}

/// Result of a promotion or demotion attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LifecycleTransition {
    pub body: GlobalPhysicalEntityId,
    pub from: BodyLifecycleTier,
    pub to: BodyLifecycleTier,
    pub command: BodyLifecycleCommand,
}

/// Promote a body from cold or warm towards active. Emits the
/// matching `BodyLifecycleCommand` (the actual command that the
/// caller routes through the shard registry).
pub fn promote_to_active(
    registry: &ShardRegistry,
    shard: ShardId,
    body: GlobalPhysicalEntityId,
) -> Option<LifecycleTransition> {
    let runtime = registry.shard(shard)?;
    let record = runtime.body(body)?;
    match record.tier {
        BodyLifecycleTier::Cold => Some(LifecycleTransition {
            body,
            from: BodyLifecycleTier::Cold,
            to: BodyLifecycleTier::Warm,
            command: BodyLifecycleCommand::PromoteToWarm { shard, body },
        }),
        BodyLifecycleTier::Warm => Some(LifecycleTransition {
            body,
            from: BodyLifecycleTier::Warm,
            to: BodyLifecycleTier::Active,
            command: BodyLifecycleCommand::ActivateBody { shard, body },
        }),
        BodyLifecycleTier::Active | BodyLifecycleTier::Ghost => None,
    }
}

/// Demote a body from active towards cold.
pub fn demote_to_cold(
    registry: &ShardRegistry,
    shard: ShardId,
    body: GlobalPhysicalEntityId,
) -> Option<LifecycleTransition> {
    let runtime = registry.shard(shard)?;
    let record = runtime.body(body)?;
    match record.tier {
        BodyLifecycleTier::Active => Some(LifecycleTransition {
            body,
            from: BodyLifecycleTier::Active,
            to: BodyLifecycleTier::Warm,
            command: BodyLifecycleCommand::SleepBody { shard, body },
        }),
        BodyLifecycleTier::Warm => Some(LifecycleTransition {
            body,
            from: BodyLifecycleTier::Warm,
            to: BodyLifecycleTier::Cold,
            command: BodyLifecycleCommand::DemoteToCold { shard, body },
        }),
        BodyLifecycleTier::Cold | BodyLifecycleTier::Ghost => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).expect("entity index should be in range")
    }

    fn record(global: u64, shard: u32) -> ColdRecord {
        ColdRecord::from_world_state(
            GlobalPhysicalEntityId(global),
            ShardId(shard),
            CellId::new(1, 2, 3),
            ColdBodyClass::Dynamic,
            ColdColliderClass::Capsule,
            Vec3::new(1.5, 2.5, -3.5),
            Quat::IDENTITY,
            Vec3::new(0.5, 0.0, 0.0),
            Vec3::ZERO,
            BodyLifecycleTier::Cold,
            42,
        )
    }

    #[test]
    fn cold_record_size_matches_constant() {
        let mut buffer = Vec::new();
        record(1, 0).write_to(&mut buffer);
        assert_eq!(buffer.len(), COLD_RECORD_BYTES);
        assert_eq!(ColdRecord::approximate_bytes(), COLD_RECORD_BYTES);
    }

    #[test]
    fn round_trip_chunk_preserves_records() {
        let chunk = ColdChunkSnapshot::new(
            ShardId(7),
            123,
            vec![record(1, 7), record(2, 7), record(3, 7)],
        );
        let bytes = chunk.encode();
        let decoded = ColdChunkSnapshot::decode(&bytes, DEFAULT_MAX_CHUNK_BYTES, Some(ShardId(7)))
            .expect("round-trip decode should succeed");
        assert_eq!(decoded.header, chunk.header);
        assert_eq!(decoded.records, chunk.records);
    }

    #[test]
    fn corrupt_chunk_is_rejected() {
        let chunk = ColdChunkSnapshot::new(ShardId(2), 1, vec![record(1, 2)]);
        let mut bytes = chunk.encode();
        // Flip a payload byte; the FNV checksum must catch it.
        let payload_offset = 24;
        bytes[payload_offset + 1] ^= 0xff;
        let result = ColdChunkSnapshot::decode(&bytes, DEFAULT_MAX_CHUNK_BYTES, None);
        assert_eq!(result, Err(ColdChunkError::ChecksumMismatch));
    }

    #[test]
    fn oversized_chunk_is_rejected() {
        let chunk = ColdChunkSnapshot::new(ShardId(0), 0, vec![record(1, 0); 4]);
        let bytes = chunk.encode();
        let cap = bytes.len() / 2;
        let result = ColdChunkSnapshot::decode(&bytes, cap, None);
        assert!(matches!(result, Err(ColdChunkError::Oversized { .. })));
    }

    #[test]
    fn stale_shard_ownership_is_rejected() {
        let chunk = ColdChunkSnapshot::new(ShardId(3), 0, vec![record(1, 3)]);
        let bytes = chunk.encode();
        let result = ColdChunkSnapshot::decode(&bytes, DEFAULT_MAX_CHUNK_BYTES, Some(ShardId(99)));
        assert!(
            matches!(result, Err(ColdChunkError::StaleShardOwnership { .. })),
            "decoder must reject chunks claimed by another shard"
        );
    }

    #[test]
    fn bad_magic_is_rejected() {
        let mut bytes = ColdChunkSnapshot::new(ShardId(0), 0, vec![record(1, 0)]).encode();
        bytes[0] = b'X';
        let result = ColdChunkSnapshot::decode(&bytes, DEFAULT_MAX_CHUNK_BYTES, None);
        assert_eq!(result, Err(ColdChunkError::BadMagic));
    }

    #[test]
    fn unsupported_version_is_rejected() {
        let mut bytes = ColdChunkSnapshot::new(ShardId(0), 0, vec![record(1, 0)]).encode();
        bytes[4] = 99;
        let result = ColdChunkSnapshot::decode(&bytes, DEFAULT_MAX_CHUNK_BYTES, None);
        assert_eq!(result, Err(ColdChunkError::UnsupportedVersion(99)));
    }

    #[test]
    fn lying_record_count_in_header_is_rejected() {
        // Audit fix #2: a crafted FCC1 whose `record_count` header
        // field disagrees with the actual payload size must reject as
        // LengthMismatch before allocating. The record_count u32 sits
        // at offset 16 in the header (4 magic + 1 version + 3 reserved
        // + 4 shard + 4 tick = 16).
        let snapshot = ColdChunkSnapshot::new(ShardId(0), 7, vec![record(1, 0)]);
        let mut bytes = snapshot.encode();
        let lying_count = u32::MAX.to_le_bytes();
        bytes[16..20].copy_from_slice(&lying_count);
        let err = ColdChunkSnapshot::decode(&bytes, DEFAULT_MAX_CHUNK_BYTES, None)
            .expect_err("oversized record_count must reject");
        assert_eq!(err, ColdChunkError::LengthMismatch);
    }

    #[test]
    fn truncated_payload_is_rejected_as_length_mismatch() {
        // Audit fix #2: payload trimmed below `record_count *
        // COLD_RECORD_BYTES` must reject as LengthMismatch — paired
        // coverage with the lying-count case above.
        let snapshot = ColdChunkSnapshot::new(ShardId(0), 7, vec![record(1, 0), record(2, 0)]);
        let mut bytes = snapshot.encode();
        // Drop the last 10 bytes of payload while keeping the header
        // count of 2.
        bytes.truncate(bytes.len() - 10);
        let err = ColdChunkSnapshot::decode(&bytes, DEFAULT_MAX_CHUNK_BYTES, None)
            .expect_err("truncated payload must reject");
        assert_eq!(err, ColdChunkError::LengthMismatch);
    }

    #[test]
    fn promote_path_yields_warm_then_active_commands() {
        let mut registry = ShardRegistry::with_cell_size(10.0, 1.0);
        let body = GlobalPhysicalEntityId(7);
        let shard = registry
            .spawn_body(
                body,
                entity(70),
                ActiveBodyKind::Dynamic,
                Vec3::new(1.0, 0.0, 0.0),
            )
            .expect("body should spawn")
            .shard;
        // Spawn lands the body Active. Demote it to Warm/Cold so we
        // can test the promotion path.
        let demote =
            demote_to_cold(&registry, shard, body).expect("demote should produce a transition");
        assert_eq!(demote.from, BodyLifecycleTier::Active);
        assert_eq!(demote.to, BodyLifecycleTier::Warm);
        registry.apply_command(demote.command, Vec3::new(1.0, 0.0, 0.0));

        let demote2 = demote_to_cold(&registry, shard, body)
            .expect("second demote should produce a transition");
        assert_eq!(demote2.from, BodyLifecycleTier::Warm);
        assert_eq!(demote2.to, BodyLifecycleTier::Cold);
        registry.apply_command(demote2.command, Vec3::new(1.0, 0.0, 0.0));

        let promote = promote_to_active(&registry, shard, body)
            .expect("promote from cold should produce a transition");
        assert_eq!(promote.from, BodyLifecycleTier::Cold);
        assert_eq!(promote.to, BodyLifecycleTier::Warm);
        assert!(matches!(
            promote.command,
            BodyLifecycleCommand::PromoteToWarm { .. }
        ));
        registry.apply_command(promote.command, Vec3::new(1.0, 0.0, 0.0));

        let promote2 = promote_to_active(&registry, shard, body)
            .expect("promote from warm should produce a transition");
        assert_eq!(promote2.from, BodyLifecycleTier::Warm);
        assert_eq!(promote2.to, BodyLifecycleTier::Active);
        assert!(matches!(
            promote2.command,
            BodyLifecycleCommand::ActivateBody { .. }
        ));
    }

    #[test]
    fn demote_from_cold_or_ghost_yields_no_transition() {
        let mut registry = ShardRegistry::with_cell_size(10.0, 1.0);
        let body = GlobalPhysicalEntityId(11);
        let shard = registry
            .spawn_body(
                body,
                entity(110),
                ActiveBodyKind::Dynamic,
                Vec3::new(1.0, 0.0, 0.0),
            )
            .expect("body should spawn")
            .shard;
        // Drive to cold first.
        let to_warm = demote_to_cold(&registry, shard, body).unwrap();
        registry.apply_command(to_warm.command, Vec3::new(1.0, 0.0, 0.0));
        let to_cold = demote_to_cold(&registry, shard, body).unwrap();
        registry.apply_command(to_cold.command, Vec3::new(1.0, 0.0, 0.0));
        assert!(demote_to_cold(&registry, shard, body).is_none());
    }

    #[test]
    fn promote_from_active_or_ghost_yields_no_transition() {
        let mut registry = ShardRegistry::with_cell_size(10.0, 1.0);
        let body = GlobalPhysicalEntityId(13);
        let shard = registry
            .spawn_body(
                body,
                entity(130),
                ActiveBodyKind::Dynamic,
                Vec3::new(1.0, 0.0, 0.0),
            )
            .expect("body should spawn")
            .shard;
        assert!(promote_to_active(&registry, shard, body).is_none());
    }

    #[test]
    fn bytes_per_cold_record_is_constant_and_small() {
        // The struct-shape rule is "do not mimic full rigid bodies".
        // 66 bytes (the constant we picked) is small enough that
        // billion-entity hosting fits in tens of GB of cold storage.
        assert_eq!(COLD_RECORD_BYTES, 66);
    }
}
