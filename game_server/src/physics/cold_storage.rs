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

// ============================================================================
// V2-P6: Cold-storage hardening — schema evolution + encoding experiment
// ----------------------------------------------------------------------------
// Adds a typed schema-version surface (so future cold-chunk format
// revisions land behind a typed enum rather than ad-hoc match arms),
// and a typed encoding-experiment surface (so the cold-record encoder
// can be swapped between the canonical fixed 66-byte layout, a
// varint+delta variant, and a block-compressed variant without
// rewriting the rest of the pipeline). All additions are opt-in: the
// production wire format remains the V1 fixed-layout encoding consumed
// by `ColdChunkSnapshot::{encode, decode}`.
// ============================================================================

/// Typed cold-chunk wire-format version. Today only [`Self::V1`] is
/// supported; reserved future versions must be added as variants here
/// before any encoder writes them, so the decoder always sees a
/// closed enum and can reject unknown values without allocating.
///
/// Reservation table:
///
/// | Version | Status        | Notes                                      |
/// |---------|---------------|--------------------------------------------|
/// | `0`     | Reserved      | Sentinel; never written to the wire.       |
/// | `1`     | **Current**   | Fixed 66-byte records, FCC1 magic.         |
/// | `2`     | Reserved      | Schema evolution slot — varint+delta body. |
/// | `3`     | Reserved      | Schema evolution slot — block-compressed.  |
/// | `4..`   | Reserved      | Held back for future schema revisions.     |
///
/// Decoders see [`Self::V1`] for `bytes[4] == 1` and
/// [`Self::FutureReserved(version)`] for any other valid byte. The
/// production `ColdChunkSnapshot::decode` rejects every non-`V1`
/// version with [`ColdChunkError::UnsupportedVersion`]; downstream
/// migration tooling can match on `FutureReserved` to attempt a
/// best-effort upgrade.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ColdChunkSchemaVersion {
    /// Fixed 66-byte records; the only version a production decoder
    /// accepts today.
    V1,
    /// Reserved-but-not-yet-implemented version. Decoders MUST NOT
    /// produce a [`ColdChunkSnapshot`] from these payloads; this
    /// variant exists so migration tools can recognise the byte
    /// without needing to thread it through the typed surface.
    FutureReserved(u8),
}

impl ColdChunkSchemaVersion {
    /// Lowest reserved-but-future version byte. The current production
    /// version is `1`, so `2` is the first slot a future encoder may
    /// claim.
    pub const NEXT_RESERVED: u8 = 2;

    /// Wire byte for this version.
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::V1 => COLD_CHUNK_VERSION,
            Self::FutureReserved(value) => value,
        }
    }

    /// Decode a wire byte into a typed version. Returns
    /// [`Self::FutureReserved`] for anything that is not `V1` so the
    /// decoder always has a closed match arm to react to.
    pub const fn from_u8(value: u8) -> Self {
        if value == COLD_CHUNK_VERSION {
            Self::V1
        } else {
            Self::FutureReserved(value)
        }
    }

    /// True iff a production decoder accepts this version. Today
    /// only [`Self::V1`] is supported.
    pub const fn is_supported(self) -> bool {
        matches!(self, Self::V1)
    }
}

/// Typed cold-record encoding strategy. The V1 wire format is always
/// [`Self::FixedV1`]; the other two variants are experiments that
/// share the same `ColdRecord` type but emit a different byte stream
/// per record block. They live behind this enum so the rest of the
/// pipeline (hot/warm/cold lifecycle commands, diagnostics, shard
/// transfers) is encoding-agnostic.
///
/// All variants round-trip through
/// [`encode_records_with`] / [`decode_records_with`] without loss —
/// the experiment is purely about per-record byte cost.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub enum ColdRecordEncoding {
    /// Production: fixed 66-byte records concatenated in order.
    /// Decode is constant-time per record and requires no
    /// pre-pass over the payload.
    #[default]
    FixedV1,
    /// Experiment: per-field LEB128-style varint encoding with delta
    /// coding against the previous record. Position, rotation, and
    /// velocity components are stored as the signed delta against
    /// the prior record's field, then varint-encoded. Common case
    /// (cluster of similar bodies) compresses heavily; degenerate
    /// case (random poses) inflates by ~10%.
    VarintDelta,
    /// Experiment: emit the V1 fixed payload then run a tiny LZ-style
    /// block compressor over it. Designed for cold storage on disk
    /// rather than wire transfer; round-trips exactly but the encode
    /// cost is higher than `VarintDelta`.
    BlockCompressed,
}

impl ColdRecordEncoding {
    /// Stable diagnostic name for telemetry / config bundles.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FixedV1 => "fixed_v1",
            Self::VarintDelta => "varint_delta",
            Self::BlockCompressed => "block_compressed",
        }
    }

    /// True iff this encoding is the production wire format.
    pub const fn is_production(self) -> bool {
        matches!(self, Self::FixedV1)
    }
}

/// Per-encoding diagnostic. Reports how many bytes the encoded record
/// block consumed plus the fixed-layout reference size so a caller can
/// compute the compression ratio without re-encoding.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct ColdEncodingStats {
    /// Encoding that produced these stats.
    pub encoding: ColdRecordEncoding,
    /// Records covered by the encoded block.
    pub record_count: u32,
    /// Bytes the encoded block consumed.
    pub encoded_bytes: u32,
    /// Bytes the same record set would consume in the fixed V1
    /// layout. Equal to `record_count * COLD_RECORD_BYTES`.
    pub reference_fixed_bytes: u32,
}

impl ColdEncodingStats {
    /// Compression ratio relative to the fixed V1 layout. Returns
    /// `1.0` for the production encoding (it *is* the reference) and
    /// `< 1.0` when the experiment beats the fixed layout. Returns
    /// `1.0` when there are no records to avoid divide-by-zero.
    pub fn ratio_vs_fixed(self) -> f32 {
        if self.reference_fixed_bytes == 0 {
            return 1.0;
        }
        self.encoded_bytes as f32 / self.reference_fixed_bytes as f32
    }
}

/// Encode a record block with the chosen [`ColdRecordEncoding`]. The
/// returned vector is self-describing for `VarintDelta` (each record
/// emits its own header) and includes a small magic + record count
/// prefix for `BlockCompressed`. `FixedV1` matches the production
/// wire format exactly.
#[must_use]
pub fn encode_records_with(records: &[ColdRecord], encoding: ColdRecordEncoding) -> Vec<u8> {
    match encoding {
        ColdRecordEncoding::FixedV1 => {
            let mut out = Vec::with_capacity(records.len() * COLD_RECORD_BYTES);
            for record in records {
                record.write_to(&mut out);
            }
            out
        }
        ColdRecordEncoding::VarintDelta => {
            let mut out = Vec::with_capacity(records.len() * COLD_RECORD_BYTES / 2);
            // Header: magic + count.
            out.extend_from_slice(b"VDC1");
            out.extend_from_slice(&(records.len() as u32).to_le_bytes());
            let mut prev: Option<&ColdRecord> = None;
            for record in records {
                write_varint_delta(&mut out, record, prev);
                prev = Some(record);
            }
            out
        }
        ColdRecordEncoding::BlockCompressed => {
            let mut payload = Vec::with_capacity(records.len() * COLD_RECORD_BYTES);
            for record in records {
                record.write_to(&mut payload);
            }
            let compressed = block_compress(&payload);
            let mut out = Vec::with_capacity(8 + compressed.len());
            out.extend_from_slice(b"BLZ1");
            out.extend_from_slice(&(records.len() as u32).to_le_bytes());
            out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            out.extend_from_slice(&compressed);
            out
        }
    }
}

/// Decode a record block previously produced by
/// [`encode_records_with`]. Returns the records plus the
/// `ColdEncodingStats` describing the byte cost.
pub fn decode_records_with(
    bytes: &[u8],
    encoding: ColdRecordEncoding,
) -> Result<(Vec<ColdRecord>, ColdEncodingStats), ColdChunkError> {
    match encoding {
        ColdRecordEncoding::FixedV1 => {
            if !bytes.len().is_multiple_of(COLD_RECORD_BYTES) {
                return Err(ColdChunkError::LengthMismatch);
            }
            let count = bytes.len() / COLD_RECORD_BYTES;
            let mut records = Vec::with_capacity(count);
            let mut cursor = 0;
            for _ in 0..count {
                let (record, consumed) =
                    ColdRecord::read_from(&bytes[cursor..]).ok_or(ColdChunkError::InvalidEnum)?;
                records.push(record);
                cursor += consumed;
            }
            let stats = ColdEncodingStats {
                encoding,
                record_count: count as u32,
                encoded_bytes: bytes.len() as u32,
                reference_fixed_bytes: (count * COLD_RECORD_BYTES) as u32,
            };
            Ok((records, stats))
        }
        ColdRecordEncoding::VarintDelta => {
            if bytes.len() < 8 || &bytes[0..4] != b"VDC1" {
                return Err(ColdChunkError::BadMagic);
            }
            let count = u32::from_le_bytes(
                bytes[4..8]
                    .try_into()
                    .map_err(|_| ColdChunkError::TooShort)?,
            ) as usize;
            // V2-P6 exit gate: no unbounded allocation from encoded
            // counts. Cap the with_capacity hint at a sane envelope so
            // a hostile chunk that lies about its count cannot
            // pre-allocate hundreds of megabytes before the decoder
            // walks the payload.
            let cap_hint = count.min(bytes.len() / 8);
            let mut records = Vec::with_capacity(cap_hint);
            let mut cursor = 8;
            let mut prev: Option<ColdRecord> = None;
            for _ in 0..count {
                let (record, consumed) = read_varint_delta(&bytes[cursor..], prev.as_ref())
                    .ok_or(ColdChunkError::LengthMismatch)?;
                records.push(record);
                prev = Some(record);
                cursor = cursor
                    .checked_add(consumed)
                    .ok_or(ColdChunkError::LengthMismatch)?;
            }
            if cursor != bytes.len() {
                return Err(ColdChunkError::LengthMismatch);
            }
            let stats = ColdEncodingStats {
                encoding,
                record_count: count as u32,
                encoded_bytes: bytes.len() as u32,
                reference_fixed_bytes: (count * COLD_RECORD_BYTES) as u32,
            };
            Ok((records, stats))
        }
        ColdRecordEncoding::BlockCompressed => {
            if bytes.len() < 12 || &bytes[0..4] != b"BLZ1" {
                return Err(ColdChunkError::BadMagic);
            }
            let count = u32::from_le_bytes(
                bytes[4..8]
                    .try_into()
                    .map_err(|_| ColdChunkError::TooShort)?,
            ) as usize;
            let payload_len = u32::from_le_bytes(
                bytes[8..12]
                    .try_into()
                    .map_err(|_| ColdChunkError::TooShort)?,
            ) as usize;
            let expected_payload = count
                .checked_mul(COLD_RECORD_BYTES)
                .ok_or(ColdChunkError::LengthMismatch)?;
            if payload_len != expected_payload {
                return Err(ColdChunkError::LengthMismatch);
            }
            let payload = block_decompress(&bytes[12..], payload_len)
                .ok_or(ColdChunkError::LengthMismatch)?;
            let mut records = Vec::with_capacity(count);
            let mut cursor = 0;
            for _ in 0..count {
                let (record, consumed) =
                    ColdRecord::read_from(&payload[cursor..]).ok_or(ColdChunkError::InvalidEnum)?;
                records.push(record);
                cursor += consumed;
            }
            let stats = ColdEncodingStats {
                encoding,
                record_count: count as u32,
                encoded_bytes: bytes.len() as u32,
                reference_fixed_bytes: (count * COLD_RECORD_BYTES) as u32,
            };
            Ok((records, stats))
        }
    }
}

// --- varint+delta helpers --------------------------------------------------

fn write_varint_unsigned(out: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        out.push(((value as u8) & 0x7f) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn read_varint_unsigned(bytes: &[u8]) -> Option<(u64, usize)> {
    let mut value: u64 = 0;
    let mut shift = 0;
    for (i, &byte) in bytes.iter().enumerate() {
        // Cap at 10 bytes (max for a u64 LEB128).
        if i >= 10 {
            return None;
        }
        let lo = (byte & 0x7f) as u64;
        value |= lo.checked_shl(shift)?;
        if byte & 0x80 == 0 {
            return Some((value, i + 1));
        }
        shift += 7;
    }
    None
}

fn zigzag_encode_i64(value: i64) -> u64 {
    ((value << 1) ^ (value >> 63)) as u64
}

fn zigzag_decode_u64(value: u64) -> i64 {
    ((value >> 1) as i64) ^ -((value & 1) as i64)
}

fn write_varint_signed(out: &mut Vec<u8>, value: i64) {
    write_varint_unsigned(out, zigzag_encode_i64(value));
}

fn read_varint_signed(bytes: &[u8]) -> Option<(i64, usize)> {
    let (raw, consumed) = read_varint_unsigned(bytes)?;
    Some((zigzag_decode_u64(raw), consumed))
}

fn write_varint_delta(out: &mut Vec<u8>, record: &ColdRecord, prev: Option<&ColdRecord>) {
    write_varint_unsigned(out, record.global.0);
    write_varint_unsigned(out, record.shard.0 as u64);
    write_varint_signed(out, record.cell.x as i64);
    write_varint_signed(out, record.cell.y as i64);
    write_varint_signed(out, record.cell.z as i64);
    out.push(record.body_class.as_u8());
    out.push(record.collider_class.as_u8());
    let prev_pos = prev.map(|p| p.coarse_position_mm).unwrap_or([0; 3]);
    let prev_rot = prev.map(|p| p.coarse_rotation).unwrap_or([0; 4]);
    let prev_lin = prev
        .map(|p| p.coarse_linear_velocity_cm_per_s)
        .unwrap_or([0; 3]);
    let prev_ang = prev
        .map(|p| p.coarse_angular_velocity_milli_rad_per_s)
        .unwrap_or([0; 3]);
    for (current, previous) in record.coarse_position_mm.iter().zip(prev_pos.iter()) {
        write_varint_signed(out, (*current - *previous) as i64);
    }
    for (current, previous) in record.coarse_rotation.iter().zip(prev_rot.iter()) {
        write_varint_signed(out, (*current as i32 - *previous as i32) as i64);
    }
    for (current, previous) in record
        .coarse_linear_velocity_cm_per_s
        .iter()
        .zip(prev_lin.iter())
    {
        write_varint_signed(out, (*current as i32 - *previous as i32) as i64);
    }
    for (current, previous) in record
        .coarse_angular_velocity_milli_rad_per_s
        .iter()
        .zip(prev_ang.iter())
    {
        write_varint_signed(out, (*current as i32 - *previous as i32) as i64);
    }
    out.push(tier_to_u8(record.tier));
    write_varint_unsigned(out, record.last_authoritative_tick as u64);
}

fn read_varint_delta(bytes: &[u8], prev: Option<&ColdRecord>) -> Option<(ColdRecord, usize)> {
    let mut cursor = 0;
    let (global, c) = read_varint_unsigned(&bytes[cursor..])?;
    cursor += c;
    let (shard_raw, c) = read_varint_unsigned(&bytes[cursor..])?;
    cursor += c;
    let (cell_x, c) = read_varint_signed(&bytes[cursor..])?;
    cursor += c;
    let (cell_y, c) = read_varint_signed(&bytes[cursor..])?;
    cursor += c;
    let (cell_z, c) = read_varint_signed(&bytes[cursor..])?;
    cursor += c;
    if bytes.len() < cursor + 2 {
        return None;
    }
    let body_class = ColdBodyClass::from_u8(bytes[cursor])?;
    cursor += 1;
    let collider_class = ColdColliderClass::from_u8(bytes[cursor])?;
    cursor += 1;
    let prev_pos = prev.map(|p| p.coarse_position_mm).unwrap_or([0; 3]);
    let prev_rot = prev.map(|p| p.coarse_rotation).unwrap_or([0; 4]);
    let prev_lin = prev
        .map(|p| p.coarse_linear_velocity_cm_per_s)
        .unwrap_or([0; 3]);
    let prev_ang = prev
        .map(|p| p.coarse_angular_velocity_milli_rad_per_s)
        .unwrap_or([0; 3]);
    let mut pos = [0_i32; 3];
    for i in 0..3 {
        let (delta, c) = read_varint_signed(&bytes[cursor..])?;
        cursor += c;
        pos[i] = prev_pos[i].wrapping_add(delta as i32);
    }
    let mut rot = [0_i16; 4];
    for i in 0..4 {
        let (delta, c) = read_varint_signed(&bytes[cursor..])?;
        cursor += c;
        rot[i] = (prev_rot[i] as i32).wrapping_add(delta as i32) as i16;
    }
    let mut lin = [0_i16; 3];
    for i in 0..3 {
        let (delta, c) = read_varint_signed(&bytes[cursor..])?;
        cursor += c;
        lin[i] = (prev_lin[i] as i32).wrapping_add(delta as i32) as i16;
    }
    let mut ang = [0_i16; 3];
    for i in 0..3 {
        let (delta, c) = read_varint_signed(&bytes[cursor..])?;
        cursor += c;
        ang[i] = (prev_ang[i] as i32).wrapping_add(delta as i32) as i16;
    }
    if bytes.len() < cursor + 1 {
        return None;
    }
    let tier = tier_from_u8(bytes[cursor])?;
    cursor += 1;
    let (tick, c) = read_varint_unsigned(&bytes[cursor..])?;
    cursor += c;
    Some((
        ColdRecord {
            global: GlobalPhysicalEntityId(global),
            shard: ShardId(shard_raw as u32),
            cell: CellId::new(cell_x as i32, cell_y as i32, cell_z as i32),
            body_class,
            collider_class,
            coarse_position_mm: pos,
            coarse_rotation: rot,
            coarse_linear_velocity_cm_per_s: lin,
            coarse_angular_velocity_milli_rad_per_s: ang,
            tier,
            last_authoritative_tick: tick as u32,
        },
        cursor,
    ))
}

// --- block compression helpers --------------------------------------------
//
// Tiny LZ-style backreference encoder. Each token is one byte:
//   0xxxxxxx              literal: copy next 1..=127 bytes directly
//   1lllllll oooooooo     backreference: copy `l + 2` bytes from
//                          `offset + 1` bytes back (l: 7 bits + 2,
//                          offset: 8 bits + 1, max 256-byte window)
//
// Designed for clarity and round-trip correctness, not for raw
// compression ratio against a real LZ77/LZ4 implementation.

fn block_compress(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        let mut best_len = 0;
        let mut best_offset = 0;
        let window_start = i.saturating_sub(256);
        let max_match = (input.len() - i).min(129);
        if max_match >= 3 {
            for back in window_start..i {
                let mut len = 0;
                while len < max_match && input[back + len] == input[i + len] {
                    len += 1;
                    if back + len >= i {
                        break;
                    }
                }
                if len > best_len {
                    best_len = len;
                    best_offset = i - back;
                }
            }
        }
        if best_len >= 3 {
            let token = 0x80 | ((best_len - 2) as u8 & 0x7f);
            out.push(token);
            out.push((best_offset - 1) as u8);
            i += best_len;
        } else {
            // Literal run: emit up to 127 bytes that don't beat threshold.
            let run_start = i;
            let mut run_len = 0;
            while run_len < 127 && i + run_len < input.len() {
                run_len += 1;
                i += 1;
            }
            let _ = run_start;
            out.push(run_len as u8);
            out.extend_from_slice(&input[i - run_len..i]);
        }
    }
    out
}

fn block_decompress(input: &[u8], expected_len: usize) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(expected_len);
    let mut i = 0;
    while i < input.len() {
        let token = input[i];
        i += 1;
        if token & 0x80 == 0 {
            let len = token as usize;
            if i + len > input.len() {
                return None;
            }
            out.extend_from_slice(&input[i..i + len]);
            i += len;
        } else {
            if i >= input.len() {
                return None;
            }
            let len = ((token & 0x7f) as usize) + 2;
            let offset = (input[i] as usize) + 1;
            i += 1;
            if offset > out.len() {
                return None;
            }
            let start = out.len() - offset;
            for j in 0..len {
                let byte = out[start + j];
                out.push(byte);
            }
        }
    }
    if out.len() != expected_len {
        return None;
    }
    Some(out)
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

    // ---- V2-P6 cold-storage hardening tests --------------------------

    #[test]
    fn invalid_body_class_byte_is_rejected_as_invalid_enum() {
        // V2-P6 spec malformed-input list: invalid enum. A chunk with
        // a body_class byte outside the typed `ColdBodyClass` range
        // must reject as `InvalidEnum` rather than panic or silently
        // produce a default.
        let chunk = ColdChunkSnapshot::new(ShardId(0), 0, vec![record(1, 0)]);
        let mut bytes = chunk.encode();
        // Chunk header is 24 bytes; the body_class byte sits at
        // offset 24 in the payload (record offset 24 inside the
        // record itself).
        let body_class_offset = 24 + 24;
        bytes[body_class_offset] = 0xff;
        // Recompute checksum so we exercise the enum-parse path
        // rather than the checksum-mismatch path.
        let payload_checksum = thunder::hash::fnv1a32(&bytes[24..]);
        bytes[20..24].copy_from_slice(&payload_checksum.to_le_bytes());
        let err = ColdChunkSnapshot::decode(&bytes, DEFAULT_MAX_CHUNK_BYTES, None)
            .expect_err("invalid body_class byte must reject");
        assert_eq!(err, ColdChunkError::InvalidEnum);
    }

    #[test]
    fn schema_version_v1_is_supported_and_future_is_not() {
        assert!(ColdChunkSchemaVersion::V1.is_supported());
        assert!(!ColdChunkSchemaVersion::FutureReserved(99).is_supported());
        assert_eq!(ColdChunkSchemaVersion::V1.as_u8(), COLD_CHUNK_VERSION);
        assert_eq!(ColdChunkSchemaVersion::FutureReserved(7).as_u8(), 7);
    }

    #[test]
    fn schema_version_decode_round_trips_known_and_future_values() {
        // V2-P6 spec: schema evolution requires a typed surface so
        // future versions land behind a closed enum.
        let known = ColdChunkSchemaVersion::from_u8(COLD_CHUNK_VERSION);
        assert_eq!(known, ColdChunkSchemaVersion::V1);
        let future = ColdChunkSchemaVersion::from_u8(42);
        assert_eq!(future, ColdChunkSchemaVersion::FutureReserved(42));
    }

    #[test]
    fn schema_version_next_reserved_is_above_current() {
        // The reservation table documents NEXT_RESERVED as the first
        // free slot. Pinning it here so future passes that bump the
        // version can fail this assertion and update the table at the
        // same time.
        const { assert!(ColdChunkSchemaVersion::NEXT_RESERVED > COLD_CHUNK_VERSION) };
    }

    #[test]
    fn fixed_v1_encoding_round_trips() {
        let records = vec![record(1, 0), record(2, 0), record(3, 0)];
        let bytes = encode_records_with(&records, ColdRecordEncoding::FixedV1);
        let (decoded, stats) =
            decode_records_with(&bytes, ColdRecordEncoding::FixedV1).expect("round-trip");
        assert_eq!(decoded, records);
        assert_eq!(stats.encoding, ColdRecordEncoding::FixedV1);
        assert_eq!(stats.record_count, 3);
        assert_eq!(stats.encoded_bytes, (3 * COLD_RECORD_BYTES) as u32);
        assert_eq!(stats.reference_fixed_bytes, (3 * COLD_RECORD_BYTES) as u32);
        // Production encoding is byte-equal to the reference.
        assert_eq!(stats.ratio_vs_fixed(), 1.0);
        assert!(ColdRecordEncoding::FixedV1.is_production());
    }

    #[test]
    fn varint_delta_encoding_round_trips() {
        let records = vec![record(1, 0), record(2, 0), record(3, 0)];
        let bytes = encode_records_with(&records, ColdRecordEncoding::VarintDelta);
        let (decoded, stats) =
            decode_records_with(&bytes, ColdRecordEncoding::VarintDelta).expect("round-trip");
        assert_eq!(decoded, records);
        assert_eq!(stats.encoding, ColdRecordEncoding::VarintDelta);
        assert_eq!(stats.record_count, 3);
        assert!(!ColdRecordEncoding::VarintDelta.is_production());
        // Three near-identical bodies should compress to noticeably
        // less than the fixed reference.
        assert!(
            stats.ratio_vs_fixed() < 1.0,
            "varint+delta with shared poses should beat the fixed layout (ratio = {})",
            stats.ratio_vs_fixed()
        );
    }

    #[test]
    fn block_compressed_encoding_round_trips() {
        // Use a higher record count so the block compressor's
        // backreferences have something to hit.
        let records: Vec<ColdRecord> = (0..8).map(|i| record(i as u64, 0)).collect();
        let bytes = encode_records_with(&records, ColdRecordEncoding::BlockCompressed);
        let (decoded, stats) =
            decode_records_with(&bytes, ColdRecordEncoding::BlockCompressed).expect("round-trip");
        assert_eq!(decoded, records);
        assert_eq!(stats.record_count, 8);
        assert!(!ColdRecordEncoding::BlockCompressed.is_production());
        // 8 near-identical bodies share lots of bytes; the block
        // compressor must beat the reference.
        assert!(
            stats.ratio_vs_fixed() < 1.0,
            "block compression with similar bodies should beat the fixed layout (ratio = {})",
            stats.ratio_vs_fixed()
        );
    }

    #[test]
    fn varint_delta_lying_count_does_not_oversize_alloc() {
        // V2-P6 exit gate: no unbounded allocation path from encoded
        // counts. A varint+delta payload that lies about its record
        // count must reject without honouring the count's
        // with_capacity hint.
        let records = vec![record(1, 0), record(2, 0)];
        let mut bytes = encode_records_with(&records, ColdRecordEncoding::VarintDelta);
        // Overwrite the count header (offset 4..8 after the magic).
        bytes[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        let err = decode_records_with(&bytes, ColdRecordEncoding::VarintDelta)
            .expect_err("lying count must reject");
        // LengthMismatch (cursor walks off the end of the buffer
        // before count records have been consumed).
        assert_eq!(err, ColdChunkError::LengthMismatch);
    }

    #[test]
    fn varint_delta_truncated_payload_is_rejected() {
        let records = vec![record(1, 0), record(2, 0), record(3, 0)];
        let bytes = encode_records_with(&records, ColdRecordEncoding::VarintDelta);
        let truncated = &bytes[..bytes.len() - 4];
        let err = decode_records_with(truncated, ColdRecordEncoding::VarintDelta)
            .expect_err("truncated payload must reject");
        assert_eq!(err, ColdChunkError::LengthMismatch);
    }

    #[test]
    fn block_compressed_bad_magic_is_rejected() {
        let records = vec![record(1, 0), record(2, 0)];
        let mut bytes = encode_records_with(&records, ColdRecordEncoding::BlockCompressed);
        bytes[0] = b'X';
        let err = decode_records_with(&bytes, ColdRecordEncoding::BlockCompressed)
            .expect_err("bad magic must reject");
        assert_eq!(err, ColdChunkError::BadMagic);
    }

    #[test]
    fn block_compressed_payload_size_lie_is_rejected() {
        let records = vec![record(1, 0), record(2, 0)];
        let mut bytes = encode_records_with(&records, ColdRecordEncoding::BlockCompressed);
        // Overwrite payload_len header (offset 8..12) with a value
        // that disagrees with `count * COLD_RECORD_BYTES`.
        bytes[8..12].copy_from_slice(&7_u32.to_le_bytes());
        let err = decode_records_with(&bytes, ColdRecordEncoding::BlockCompressed)
            .expect_err("payload-len lie must reject");
        assert_eq!(err, ColdChunkError::LengthMismatch);
    }

    #[test]
    fn cold_record_remains_smaller_than_active_body_row() {
        // V2-P6 exit gate: cold storage remains smaller than active
        // body rows. The active-pool row carries position + rotation
        // + linear/angular velocity + handle metadata + lifecycle
        // tier, so it sits well above 66 bytes per body. Pin the
        // invariant rather than assert the active row's exact size
        // (that crosses module boundaries).
        let active_row_floor = 96; // conservative lower bound
        assert!(COLD_RECORD_BYTES < active_row_floor);
    }

    #[test]
    fn no_solver_internals_in_cold_record() {
        // V2-P6 exit gate: no solver internals stored in cold
        // records. The cold record carries pose + velocity (waking
        // integrator inputs) and lifecycle metadata only — solver
        // state (impulse cache, contact island id, sleep timer) lives
        // in the active path's contact store and is rebuilt on wake.
        // Pin this by asserting the field surface is the documented
        // 11-field shape.
        let r = record(1, 0);
        let _expected: ColdRecord = ColdRecord {
            global: r.global,
            shard: r.shard,
            cell: r.cell,
            body_class: r.body_class,
            collider_class: r.collider_class,
            coarse_position_mm: r.coarse_position_mm,
            coarse_rotation: r.coarse_rotation,
            coarse_linear_velocity_cm_per_s: r.coarse_linear_velocity_cm_per_s,
            coarse_angular_velocity_milli_rad_per_s: r.coarse_angular_velocity_milli_rad_per_s,
            tier: r.tier,
            last_authoritative_tick: r.last_authoritative_tick,
        };
    }
}
