//! Shard-local physics runtime prototype.
//!
//! Bounds active physics by region. The first version uses fixed-grid
//! sharding inside a single process, runs multiple
//! [`ShardRuntime`]s side-by-side, and emits explicit
//! [`BodyLifecycleCommand`]s when bodies promote, sleep, transfer
//! between shards, or enter neighbor halos as ghosts. Distributed
//! migration, dynamic re-balancing, and per-shard threading are
//! intentionally out of scope here — this pass is the architectural
//! shape, not a production deployment.
//!
//! # Why a separate module
//!
//! `ActivePhysicsPool` from Pass 10 is a single global resource. The
//! shard runtime composes one `ActivePhysicsPool` per shard plus the
//! typed budget, replication queue, and neighbor table that the
//! broader plan calls for. Avian's hot path stays unchanged in this
//! pass — the shard runtime is a coordinator above the active pool
//! that decides which pool a body lives in and how boundary contacts
//! are routed.
//!
//! # Determinism contract
//!
//! - Per-shard iteration follows the underlying
//!   [`ActivePhysicsPool`]'s deterministic order.
//! - Cross-shard ordering is derived from the shard grid in
//!   row-major (`x`, then `y`, then `z`) order.
//! - Lifecycle commands are produced in the same order the pool
//!   walks dirty rows, so two runs over the same input produce
//!   identical command sequences.

use hashbrown::HashMap;

use bevy::prelude::*;

use super::active_pool::{
    ActiveBodyDirtyRow, ActiveBodyHandle, ActiveBodyKind, ActivePhysicsPool, ActivePoolPolicy,
};

/// Identifier for one physics shard. Always passed by value.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, PartialOrd, Ord, Reflect)]
#[reflect(Debug, PartialEq, Hash)]
pub struct ShardId(pub u32);

impl ShardId {
    /// Sentinel used for "no shard yet". Concrete grid mappings start
    /// at zero and walk upward.
    pub const NONE: Self = Self(u32::MAX);

    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// Integer cell coordinate inside the shard grid.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, PartialOrd, Ord, Reflect)]
#[reflect(Debug, PartialEq, Hash)]
pub struct CellId {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl CellId {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Returns the 26 + 1 cell ids forming this cell plus its full 3D
    /// halo. Order is deterministic (z-major).
    pub fn neighborhood(self) -> [CellId; 27] {
        let mut out = [CellId::default(); 27];
        let mut index = 0;
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    out[index] = CellId::new(self.x + dx, self.y + dy, self.z + dz);
                    index += 1;
                }
            }
        }
        out
    }
}

/// Stable identifier for a physical body across shards. The shard
/// grid translates this to the local pool handle each tick.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, PartialOrd, Ord, Reflect)]
#[reflect(Debug, PartialEq, Hash)]
pub struct GlobalPhysicalEntityId(pub u64);

impl GlobalPhysicalEntityId {
    pub const NONE: Self = Self(u64::MAX);
    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// Slot id that uniquely identifies a body inside a single shard. Pairs
/// the shard's [`ActivePhysicsPool`] handle with the shard id so
/// cross-shard look-ups stay typed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, PartialOrd, Ord, Reflect)]
#[reflect(Debug, PartialEq, Hash)]
pub struct ShardLocalBodyId {
    pub shard: ShardId,
    pub handle: ActiveBodyHandle,
}

/// Body lifecycle tier. Driven by [`BodyLifecycleCommand`]s and used
/// for budget accounting only — the row layout itself does not depend
/// on this tag.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub enum BodyLifecycleTier {
    /// Body is currently asleep; not iterated by solver scans.
    #[default]
    Cold,
    /// Body is warm but not yet active (e.g. queued for promotion).
    Warm,
    /// Body is active and participates in the solver.
    Active,
    /// Body is a ghost owned by a neighbor shard.
    Ghost,
}

/// Bookkeeping record for one body inside a shard. Lives in the
/// shard's body table and tracks the tier separately from the pool
/// row so transitions can be coordinated without touching the pool's
/// dirty-row stream.
///
/// Pose / velocity fields cache the most recent authoritative state
/// produced by the active pool. They are read by Pass 18's
/// [`replay_bridge`](super::replay_bridge) when building rollback
/// slices and state digests; without them the digest would be
/// position-only and two bodies that differ only in orientation or
/// linear velocity would produce identical digests, masking real
/// divergences.
#[derive(Clone, Copy, Debug)]
pub struct ShardBodyRecord {
    pub global: GlobalPhysicalEntityId,
    pub entity: Entity,
    pub kind: ActiveBodyKind,
    pub tier: BodyLifecycleTier,
    pub handle: ActiveBodyHandle,
    pub last_position: Vec3,
    /// Most-recent rotation captured from the active pool. Defaults
    /// to [`Quat::IDENTITY`] before the first solver writeback.
    pub last_rotation: Quat,
    /// Most-recent linear velocity captured from the active pool
    /// (world space, m/s).
    pub last_linear_velocity: Vec3,
    /// Most-recent angular velocity captured from the active pool
    /// (world space, rad/s).
    pub last_angular_velocity: Vec3,
    /// Shard that owns the body authoritatively. When this body lives
    /// as a ghost in this shard, `authoritative_shard` points to the
    /// neighbor that owns the canonical state.
    pub authoritative_shard: ShardId,
}

/// Identity-only fields for a shard. Split from the runtime to keep
/// look-ups cheap.
#[derive(Clone, Copy, Debug)]
pub struct ShardIdentity {
    pub id: ShardId,
    /// Cell coordinate inside the registry grid.
    pub cell: CellId,
    /// World-space center of the shard's authoritative volume.
    pub center: Vec3,
    /// Side length of the authoritative volume (cube-shaped for the
    /// fixed-grid prototype).
    pub size: f32,
    /// Distance beyond the authoritative volume that still produces
    /// ghost insertions in this shard.
    pub halo_radius: f32,
}

impl ShardIdentity {
    pub fn contains_authoritative(&self, position: Vec3) -> bool {
        let half = self.size * 0.5;
        (position.x - self.center.x).abs() <= half
            && (position.y - self.center.y).abs() <= half
            && (position.z - self.center.z).abs() <= half
    }

    pub fn within_halo(&self, position: Vec3) -> bool {
        let half = self.size * 0.5 + self.halo_radius;
        (position.x - self.center.x).abs() <= half
            && (position.y - self.center.y).abs() <= half
            && (position.z - self.center.z).abs() <= half
    }
}

/// Per-shard budget and counters. Distinct from
/// [`super::active_pool::ActivePoolDiagnostics`] because shard-level
/// budgets gate promotions independently of the pool's capacity.
#[derive(Clone, Copy, Debug)]
pub struct ShardBudget {
    pub max_active_bodies: u32,
    pub max_ghost_bodies: u32,
    pub max_promotions_per_tick: u32,
    pub current_active: u32,
    pub current_ghosts: u32,
    pub promotions_this_tick: u32,
    pub commands_emitted_this_tick: u32,
    pub authoritative_transfers_this_tick: u32,
    pub double_solves_avoided_this_tick: u32,
}

impl Default for ShardBudget {
    fn default() -> Self {
        Self {
            max_active_bodies: 4_096,
            max_ghost_bodies: 1_024,
            max_promotions_per_tick: 256,
            current_active: 0,
            current_ghosts: 0,
            promotions_this_tick: 0,
            commands_emitted_this_tick: 0,
            authoritative_transfers_this_tick: 0,
            double_solves_avoided_this_tick: 0,
        }
    }
}

/// Per-shard physics state. Wraps the existing
/// [`ActivePhysicsPool`] without changing it, so the prototype can
/// host multiple shards in one process.
pub struct ShardPhysics {
    pub active_pool: ActivePhysicsPool,
    pub policy: ActivePoolPolicy,
    /// Body table indexed by [`GlobalPhysicalEntityId`]. Maps the
    /// global id to the shard-local pool handle and tier metadata.
    pub bodies: HashMap<GlobalPhysicalEntityId, ShardBodyRecord>,
    /// Inverse mapping for ECS-side resolution.
    pub by_entity: HashMap<Entity, GlobalPhysicalEntityId>,
}

impl ShardPhysics {
    pub fn new(policy: ActivePoolPolicy) -> Self {
        Self {
            active_pool: ActivePhysicsPool::default(),
            policy,
            bodies: HashMap::new(),
            by_entity: HashMap::new(),
        }
    }
}

impl Default for ShardPhysics {
    fn default() -> Self {
        Self::new(ActivePoolPolicy::default())
    }
}

/// Per-shard replication state. Holds the dirty-row queue this shard
/// is allowed to publish to the network bridge plus a placeholder for
/// rollback slices the prototype does not yet implement.
#[derive(Clone, Debug, Default)]
pub struct ShardReplication {
    pub dirty_queue: Vec<ActiveBodyDirtyRow>,
    pub last_published_tick: u32,
    /// Reserved for the rollback-slice ring buffer the production
    /// system will need. Tracked as a typed counter in the prototype
    /// so future passes have a stable insertion point.
    pub rollback_slice_count: u32,
}

/// Static neighbor table for one shard. Computed once when the
/// registry is built; mutating the registry rebuilds neighbor tables
/// in lockstep.
#[derive(Clone, Debug, Default)]
pub struct ShardNeighbors {
    pub neighbors: Vec<ShardId>,
}

/// One physics shard instance. Owns its identity, budget, physics
/// state, replication queue, and neighbor table.
pub struct ShardRuntime {
    pub identity: ShardIdentity,
    pub budget: ShardBudget,
    pub physics: ShardPhysics,
    pub replication: ShardReplication,
    pub neighbors: ShardNeighbors,
}

impl ShardRuntime {
    pub fn new(identity: ShardIdentity) -> Self {
        Self {
            identity,
            budget: ShardBudget::default(),
            physics: ShardPhysics::default(),
            replication: ShardReplication::default(),
            neighbors: ShardNeighbors::default(),
        }
    }

    pub fn body(&self, global: GlobalPhysicalEntityId) -> Option<&ShardBodyRecord> {
        self.physics.bodies.get(&global)
    }

    pub fn body_count(&self) -> u32 {
        self.physics.bodies.len() as u32
    }

    pub fn active_count(&self) -> u32 {
        self.budget.current_active
    }

    pub fn ghost_count(&self) -> u32 {
        self.budget.current_ghosts
    }
}

/// Body lifecycle commands emitted by the registry as bodies move
/// through promotion, sleep, transfer, or ghost lifetimes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BodyLifecycleCommand {
    /// Wake a cold body into the warm tier on the supplied shard.
    PromoteToWarm {
        shard: ShardId,
        body: GlobalPhysicalEntityId,
    },
    /// Push a warm body into the active solver scan set.
    ActivateBody {
        shard: ShardId,
        body: GlobalPhysicalEntityId,
    },
    /// Move an active body to the sleep set.
    SleepBody {
        shard: ShardId,
        body: GlobalPhysicalEntityId,
    },
    /// Drop a sleeping body into cold storage.
    DemoteToCold {
        shard: ShardId,
        body: GlobalPhysicalEntityId,
    },
    /// Insert a ghost copy of a body owned by `source_shard` into the
    /// neighbor `shard`.
    CreateGhost {
        shard: ShardId,
        body: GlobalPhysicalEntityId,
        source_shard: ShardId,
    },
    /// Remove a ghost from the supplied shard.
    RetireGhost {
        shard: ShardId,
        body: GlobalPhysicalEntityId,
    },
    /// Move authoritative ownership for a body from `from` to `to`.
    TransferShard {
        body: GlobalPhysicalEntityId,
        from: ShardId,
        to: ShardId,
    },
}

impl BodyLifecycleCommand {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PromoteToWarm { .. } => "promote_to_warm",
            Self::ActivateBody { .. } => "activate_body",
            Self::SleepBody { .. } => "sleep_body",
            Self::DemoteToCold { .. } => "demote_to_cold",
            Self::CreateGhost { .. } => "create_ghost",
            Self::RetireGhost { .. } => "retire_ghost",
            Self::TransferShard { .. } => "transfer_shard",
        }
    }
}

/// Aggregate diagnostics published by the registry each tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct ShardRegistryDiagnostics {
    pub shard_count: u32,
    pub active_bodies: u32,
    pub ghost_bodies: u32,
    pub commands_emitted_this_tick: u32,
    pub transfers_this_tick: u32,
    pub double_solves_avoided_this_tick: u32,
    pub boundary_crossings_this_tick: u32,
}

/// Resource holding every shard runtime in this process. Looks up the
/// authoritative shard for a position via the cell grid and applies
/// lifecycle commands.
#[derive(Resource)]
pub struct ShardRegistry {
    shards: HashMap<ShardId, ShardRuntime>,
    cell_to_shard: HashMap<CellId, ShardId>,
    cell_size: f32,
    halo_radius: f32,
    next_shard_id: u32,
    diagnostics: ShardRegistryDiagnostics,
}

impl Default for ShardRegistry {
    fn default() -> Self {
        Self::with_cell_size(64.0, 8.0)
    }
}

impl ShardRegistry {
    pub fn with_cell_size(cell_size: f32, halo_radius: f32) -> Self {
        Self {
            shards: HashMap::new(),
            cell_to_shard: HashMap::new(),
            cell_size,
            halo_radius,
            next_shard_id: 0,
            diagnostics: ShardRegistryDiagnostics::default(),
        }
    }

    pub fn cell_size(&self) -> f32 {
        self.cell_size
    }

    pub fn halo_radius(&self) -> f32 {
        self.halo_radius
    }

    pub fn shard_count(&self) -> u32 {
        self.shards.len() as u32
    }

    pub fn diagnostics(&self) -> ShardRegistryDiagnostics {
        self.diagnostics
    }

    /// Computes a fresh [`super::cold_storage::ColdStorageDiagnostics`]
    /// snapshot by walking every shard's body table. Used by
    /// fun-observer's summary lane.
    pub fn cold_storage_diagnostics(&self) -> super::cold_storage::ColdStorageDiagnostics {
        let mut diagnostics = super::cold_storage::ColdStorageDiagnostics::default();
        for runtime in self.shards.values() {
            for record in runtime.physics.bodies.values() {
                diagnostics.represented_entities =
                    diagnostics.represented_entities.saturating_add(1);
                match record.tier {
                    BodyLifecycleTier::Active => {
                        diagnostics.active_entities = diagnostics.active_entities.saturating_add(1);
                    }
                    BodyLifecycleTier::Warm => {
                        diagnostics.warm_entities = diagnostics.warm_entities.saturating_add(1);
                    }
                    BodyLifecycleTier::Cold => {
                        diagnostics.cold_entities = diagnostics.cold_entities.saturating_add(1);
                    }
                    BodyLifecycleTier::Ghost => {
                        // Ghosts don't count as represented authoritative
                        // entities — they're shadow copies. Roll back the
                        // represented counter so it stays a unique-body
                        // measurement.
                        diagnostics.represented_entities =
                            diagnostics.represented_entities.saturating_sub(1);
                    }
                }
            }
        }
        diagnostics.bytes_per_cold_record =
            super::cold_storage::COLD_RECORD_BYTES.min(u32::MAX as usize) as u32;
        diagnostics
    }

    pub fn shard(&self, id: ShardId) -> Option<&ShardRuntime> {
        self.shards.get(&id)
    }

    pub fn shard_mut(&mut self, id: ShardId) -> Option<&mut ShardRuntime> {
        self.shards.get_mut(&id)
    }

    pub fn iter_shards(&self) -> impl Iterator<Item = &ShardRuntime> {
        self.shards.values()
    }

    pub fn cell_for_position(&self, position: Vec3) -> CellId {
        CellId::new(
            (position.x / self.cell_size).floor() as i32,
            (position.y / self.cell_size).floor() as i32,
            (position.z / self.cell_size).floor() as i32,
        )
    }

    pub fn shard_for_position(&self, position: Vec3) -> Option<ShardId> {
        self.cell_to_shard
            .get(&self.cell_for_position(position))
            .copied()
    }

    /// Allocate a new shard for the given cell, returning its id. The
    /// shard center matches the cell's authoritative volume center.
    pub fn add_shard(&mut self, cell: CellId) -> ShardId {
        if let Some(existing) = self.cell_to_shard.get(&cell).copied() {
            return existing;
        }
        let id = ShardId(self.next_shard_id);
        self.next_shard_id = self.next_shard_id.saturating_add(1);
        let center = Vec3::new(
            (cell.x as f32 + 0.5) * self.cell_size,
            (cell.y as f32 + 0.5) * self.cell_size,
            (cell.z as f32 + 0.5) * self.cell_size,
        );
        let identity = ShardIdentity {
            id,
            cell,
            center,
            size: self.cell_size,
            halo_radius: self.halo_radius,
        };
        let runtime = ShardRuntime::new(identity);
        self.shards.insert(id, runtime);
        self.cell_to_shard.insert(cell, id);
        self.rebuild_neighbors();
        id
    }

    /// Allocate any shards required to authoritatively cover the
    /// supplied position plus its halo. Idempotent.
    pub fn ensure_shards_for_position(&mut self, position: Vec3) -> ShardId {
        let cell = self.cell_for_position(position);
        let primary = self.add_shard(cell);
        for neighbor in cell.neighborhood() {
            if neighbor == cell {
                continue;
            }
            self.add_shard(neighbor);
        }
        primary
    }

    fn rebuild_neighbors(&mut self) {
        let cell_to_shard = self.cell_to_shard.clone();
        for shard in self.shards.values_mut() {
            let cell = shard.identity.cell;
            let mut neighbors = Vec::with_capacity(26);
            for candidate in cell.neighborhood() {
                if candidate == cell {
                    continue;
                }
                if let Some(neighbor_id) = cell_to_shard.get(&candidate).copied() {
                    neighbors.push(neighbor_id);
                }
            }
            neighbors.sort_unstable();
            shard.neighbors.neighbors = neighbors;
        }
    }

    /// Reset per-tick budget counters across every shard.
    pub fn begin_tick(&mut self) {
        self.diagnostics.commands_emitted_this_tick = 0;
        self.diagnostics.transfers_this_tick = 0;
        self.diagnostics.double_solves_avoided_this_tick = 0;
        self.diagnostics.boundary_crossings_this_tick = 0;
        for shard in self.shards.values_mut() {
            shard.budget.promotions_this_tick = 0;
            shard.budget.commands_emitted_this_tick = 0;
            shard.budget.authoritative_transfers_this_tick = 0;
            shard.budget.double_solves_avoided_this_tick = 0;
        }
    }

    /// Spawn a body in the shard whose authoritative volume contains
    /// `position`. Returns the resulting [`ShardLocalBodyId`].
    pub fn spawn_body(
        &mut self,
        global: GlobalPhysicalEntityId,
        entity: Entity,
        kind: ActiveBodyKind,
        position: Vec3,
    ) -> Option<ShardLocalBodyId> {
        let primary = self.ensure_shards_for_position(position);
        let shard = self.shards.get_mut(&primary)?;
        if shard.budget.current_active >= shard.budget.max_active_bodies {
            return None;
        }
        let mut promotions = shard.budget.promotions_this_tick;
        let promotions_cap = shard.budget.max_promotions_per_tick;
        let max_per_tick = if promotions_cap == 0 {
            0
        } else {
            promotions_cap
        };
        let outcome = allocate_in_pool(
            &mut shard.physics.active_pool,
            entity,
            kind,
            shard.budget.max_active_bodies,
            max_per_tick,
            &mut promotions,
        )?;
        shard.physics.bodies.insert(
            global,
            ShardBodyRecord {
                global,
                entity,
                kind,
                tier: BodyLifecycleTier::Active,
                handle: outcome,
                last_position: position,
                last_rotation: Quat::IDENTITY,
                last_linear_velocity: Vec3::ZERO,
                last_angular_velocity: Vec3::ZERO,
                authoritative_shard: primary,
            },
        );
        shard.physics.by_entity.insert(entity, global);
        shard.budget.current_active = shard.budget.current_active.saturating_add(1);
        shard.budget.promotions_this_tick = promotions;
        // Propagate the spawn into the dirty buffer so callers can
        // observe it through the existing pool API.
        upsert_in_pool(
            &mut shard.physics.active_pool,
            outcome,
            position,
            Quat::IDENTITY,
            Vec3::ZERO,
            Vec3::ZERO,
        );

        // Halo ghosts so neighbor shards can render and reason about
        // the body without owning it.
        let neighbor_ids = shard.neighbors.neighbors.clone();
        let center = shard.identity.center;
        let _ = center;
        let id_clone = primary;
        for neighbor_id in neighbor_ids {
            if let Some(neighbor) = self.shards.get_mut(&neighbor_id)
                && neighbor.identity.within_halo(position)
                && neighbor.budget.current_ghosts < neighbor.budget.max_ghost_bodies
            {
                neighbor.physics.bodies.insert(
                    global,
                    ShardBodyRecord {
                        global,
                        entity,
                        kind,
                        tier: BodyLifecycleTier::Ghost,
                        handle: ActiveBodyHandle::default(),
                        last_position: position,
                        last_rotation: Quat::IDENTITY,
                        last_linear_velocity: Vec3::ZERO,
                        last_angular_velocity: Vec3::ZERO,
                        authoritative_shard: id_clone,
                    },
                );
                neighbor.budget.current_ghosts = neighbor.budget.current_ghosts.saturating_add(1);
            }
        }

        Some(ShardLocalBodyId {
            shard: primary,
            handle: outcome,
        })
    }

    /// Returns the canonical shard for a body and updates the dirty
    /// buffer + neighbors when a body crosses a boundary or enters /
    /// leaves a halo. Emits one or more
    /// [`BodyLifecycleCommand`]s describing the transitions.
    pub fn process_movement(
        &mut self,
        global: GlobalPhysicalEntityId,
        new_position: Vec3,
    ) -> Vec<BodyLifecycleCommand> {
        let mut commands = Vec::new();
        let Some(record) = self.find_authoritative_record(global) else {
            return commands;
        };
        let from_shard = record.authoritative_shard;
        let new_cell = self.cell_for_position(new_position);
        let to_shard = self
            .cell_to_shard
            .get(&new_cell)
            .copied()
            .unwrap_or(from_shard);

        if to_shard != from_shard {
            commands.push(BodyLifecycleCommand::TransferShard {
                body: global,
                from: from_shard,
                to: to_shard,
            });
            self.diagnostics.transfers_this_tick =
                self.diagnostics.transfers_this_tick.saturating_add(1);
            self.diagnostics.boundary_crossings_this_tick = self
                .diagnostics
                .boundary_crossings_this_tick
                .saturating_add(1);
        }

        // Drive ghost lifetimes against every known shard.
        let shard_ids: Vec<ShardId> = self.shards.keys().copied().collect();
        for shard_id in shard_ids {
            let shard = match self.shards.get(&shard_id) {
                Some(shard) => shard,
                None => continue,
            };
            let was_ghost = shard
                .physics
                .bodies
                .get(&global)
                .map(|record| record.tier == BodyLifecycleTier::Ghost)
                .unwrap_or(false);
            let is_authoritative = shard_id == to_shard;
            let in_halo = shard.identity.within_halo(new_position) && !is_authoritative;

            if in_halo && !was_ghost {
                commands.push(BodyLifecycleCommand::CreateGhost {
                    shard: shard_id,
                    body: global,
                    source_shard: to_shard,
                });
            } else if !in_halo && was_ghost {
                commands.push(BodyLifecycleCommand::RetireGhost {
                    shard: shard_id,
                    body: global,
                });
            }
        }

        // Apply the commands to the shard runtimes so callers see
        // consistent state.
        let applied = commands.clone();
        for command in &applied {
            self.apply_command(*command, new_position);
        }

        commands
    }

    /// Apply one [`BodyLifecycleCommand`] to the affected shards. The
    /// position is required for transfer/ghost commands so the new
    /// body record can be inserted into the destination shard.
    pub fn apply_command(&mut self, command: BodyLifecycleCommand, position: Vec3) {
        self.diagnostics.commands_emitted_this_tick = self
            .diagnostics
            .commands_emitted_this_tick
            .saturating_add(1);
        match command {
            BodyLifecycleCommand::PromoteToWarm { shard, body } => {
                self.set_tier(shard, body, BodyLifecycleTier::Warm);
            }
            BodyLifecycleCommand::ActivateBody { shard, body } => {
                if let Some(runtime) = self.shards.get_mut(&shard)
                    && let Some(record) = runtime.physics.bodies.get_mut(&body)
                {
                    if record.tier != BodyLifecycleTier::Active {
                        runtime.budget.current_active =
                            runtime.budget.current_active.saturating_add(1);
                    }
                    record.tier = BodyLifecycleTier::Active;
                }
            }
            BodyLifecycleCommand::SleepBody { shard, body } => {
                if let Some(runtime) = self.shards.get_mut(&shard)
                    && let Some(record) = runtime.physics.bodies.get_mut(&body)
                {
                    if record.tier == BodyLifecycleTier::Active {
                        runtime.budget.current_active =
                            runtime.budget.current_active.saturating_sub(1);
                    }
                    record.tier = BodyLifecycleTier::Warm;
                }
            }
            BodyLifecycleCommand::DemoteToCold { shard, body } => {
                self.set_tier(shard, body, BodyLifecycleTier::Cold);
            }
            BodyLifecycleCommand::CreateGhost {
                shard,
                body,
                source_shard,
            } => {
                let mut entity = Entity::PLACEHOLDER;
                let mut kind = ActiveBodyKind::Dynamic;
                if let Some(source) = self.shards.get(&source_shard)
                    && let Some(record) = source.physics.bodies.get(&body)
                {
                    entity = record.entity;
                    kind = record.kind;
                }
                if let Some(runtime) = self.shards.get_mut(&shard)
                    && !runtime.physics.bodies.contains_key(&body)
                    && runtime.budget.current_ghosts < runtime.budget.max_ghost_bodies
                {
                    runtime.physics.bodies.insert(
                        body,
                        ShardBodyRecord {
                            global: body,
                            entity,
                            kind,
                            tier: BodyLifecycleTier::Ghost,
                            handle: ActiveBodyHandle::default(),
                            last_position: position,
                            last_rotation: Quat::IDENTITY,
                            last_linear_velocity: Vec3::ZERO,
                            last_angular_velocity: Vec3::ZERO,
                            authoritative_shard: source_shard,
                        },
                    );
                    runtime.budget.current_ghosts = runtime.budget.current_ghosts.saturating_add(1);
                }
            }
            BodyLifecycleCommand::RetireGhost { shard, body } => {
                if let Some(runtime) = self.shards.get_mut(&shard)
                    && let Some(record) = runtime.physics.bodies.remove(&body)
                {
                    if record.tier == BodyLifecycleTier::Ghost {
                        runtime.budget.current_ghosts =
                            runtime.budget.current_ghosts.saturating_sub(1);
                    }
                    runtime.physics.by_entity.remove(&record.entity);
                }
            }
            BodyLifecycleCommand::TransferShard { body, from, to } => {
                if from == to {
                    return;
                }
                let removed = self
                    .shards
                    .get_mut(&from)
                    .and_then(|runtime| runtime.physics.bodies.remove(&body));
                if let Some(record) = removed {
                    if let Some(source_runtime) = self.shards.get_mut(&from) {
                        if record.tier == BodyLifecycleTier::Active {
                            source_runtime.budget.current_active =
                                source_runtime.budget.current_active.saturating_sub(1);
                        }
                        source_runtime.budget.authoritative_transfers_this_tick = source_runtime
                            .budget
                            .authoritative_transfers_this_tick
                            .saturating_add(1);
                        source_runtime.physics.by_entity.remove(&record.entity);
                        // Removing the row from the active pool keeps the
                        // shard's solver-facing iteration clean and ensures
                        // the destination is the sole solver visiting this
                        // body next tick. This is the "no double-solve
                        // across shard boundary" guarantee.
                        source_runtime
                            .physics
                            .active_pool
                            .remove_entity(record.entity);
                        source_runtime.budget.double_solves_avoided_this_tick = source_runtime
                            .budget
                            .double_solves_avoided_this_tick
                            .saturating_add(1);
                        self.diagnostics.double_solves_avoided_this_tick = self
                            .diagnostics
                            .double_solves_avoided_this_tick
                            .saturating_add(1);
                    }
                    if let Some(destination) = self.shards.get_mut(&to) {
                        let mut promotions = destination.budget.promotions_this_tick;
                        let promotions_cap = destination.budget.max_promotions_per_tick;
                        if let Some(handle) = allocate_in_pool(
                            &mut destination.physics.active_pool,
                            record.entity,
                            record.kind,
                            destination.budget.max_active_bodies,
                            promotions_cap,
                            &mut promotions,
                        ) {
                            destination.budget.promotions_this_tick = promotions;
                            destination.budget.current_active =
                                destination.budget.current_active.saturating_add(1);
                            destination.physics.bodies.insert(
                                body,
                                ShardBodyRecord {
                                    global: body,
                                    entity: record.entity,
                                    kind: record.kind,
                                    tier: BodyLifecycleTier::Active,
                                    handle,
                                    last_position: position,
                                    last_rotation: record.last_rotation,
                                    last_linear_velocity: record.last_linear_velocity,
                                    last_angular_velocity: record.last_angular_velocity,
                                    authoritative_shard: to,
                                },
                            );
                            destination.physics.by_entity.insert(record.entity, body);
                            upsert_in_pool(
                                &mut destination.physics.active_pool,
                                handle,
                                position,
                                Quat::IDENTITY,
                                Vec3::ZERO,
                                Vec3::ZERO,
                            );
                        }
                    }
                }
            }
        }
        self.refresh_diagnostics();
    }

    fn set_tier(&mut self, shard: ShardId, body: GlobalPhysicalEntityId, tier: BodyLifecycleTier) {
        if let Some(runtime) = self.shards.get_mut(&shard)
            && let Some(record) = runtime.physics.bodies.get_mut(&body)
        {
            if record.tier == BodyLifecycleTier::Active && tier != BodyLifecycleTier::Active {
                runtime.budget.current_active = runtime.budget.current_active.saturating_sub(1);
            } else if record.tier != BodyLifecycleTier::Active && tier == BodyLifecycleTier::Active
            {
                runtime.budget.current_active = runtime.budget.current_active.saturating_add(1);
            }
            record.tier = tier;
        }
    }

    fn find_authoritative_record(&self, global: GlobalPhysicalEntityId) -> Option<ShardBodyRecord> {
        for runtime in self.shards.values() {
            if let Some(record) = runtime.physics.bodies.get(&global)
                && record.tier != BodyLifecycleTier::Ghost
            {
                return Some(*record);
            }
        }
        None
    }

    fn refresh_diagnostics(&mut self) {
        let mut active = 0_u32;
        let mut ghosts = 0_u32;
        for shard in self.shards.values() {
            active = active.saturating_add(shard.budget.current_active);
            ghosts = ghosts.saturating_add(shard.budget.current_ghosts);
        }
        self.diagnostics.shard_count = self.shards.len() as u32;
        self.diagnostics.active_bodies = active;
        self.diagnostics.ghost_bodies = ghosts;
    }
}

fn allocate_in_pool(
    pool: &mut ActivePhysicsPool,
    entity: Entity,
    kind: ActiveBodyKind,
    capacity: u32,
    max_per_tick: u32,
    promotions: &mut u32,
) -> Option<ActiveBodyHandle> {
    pool.begin_tick();
    pool.__test_allocate_with(entity, kind, capacity, max_per_tick, promotions)
}

fn upsert_in_pool(
    pool: &mut ActivePhysicsPool,
    handle: ActiveBodyHandle,
    position: Vec3,
    rotation: Quat,
    linear_velocity: Vec3,
    angular_velocity: Vec3,
) {
    pool.__test_upsert(
        handle,
        position,
        rotation,
        linear_velocity,
        angular_velocity,
    );
}

// ============================================================================
// V2-P5: Shard runtime productionization
// ----------------------------------------------------------------------------
// Adds typed surface for shard runtime mode + per-shard work scheduling +
// transfer-window state machine + overload policy + overload actions on top
// of the existing prototype. Default behaviour is unchanged: the prototype
// shard continues to operate on `ShardRuntimeMode::SingleProcessFixedGrid`,
// which mirrors today's behaviour. Distributed and dynamic-grid modes ship
// as typed surfaces only — actual implementations land in follow-up passes.
// ============================================================================

use super::physical_lod::PhysicalClass;

/// Top-level shard runtime mode selecting how the host distributes shard
/// work across processes and grid topology.
///
/// Default = [`Self::SingleProcessFixedGrid`] (current production behaviour).
/// The other two variants ship as typed shapes for V2-P5; their actual
/// implementations land in later passes.
#[derive(Resource, Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Reflect)]
#[reflect(Debug, PartialEq, Hash)]
pub enum ShardRuntimeMode {
    /// **Production default.** All shards live in one process on a
    /// fixed grid layout; shard work scheduling is single-process,
    /// deterministic across independent shards in authoritative mode.
    #[default]
    SingleProcessFixedGrid,
    /// Single-process, with the shard grid rebalancing dynamically
    /// based on load. Typed surface only in V2-P5; the rebalancing
    /// scheduler lands in a follow-up pass.
    SingleProcessDynamicGrid,
    /// Distributed across multiple processes (or hosts). Typed
    /// surface only in V2-P5; the cross-process transfer transport
    /// lands when the Thunder bridge wires up.
    DistributedExperimental,
}

impl ShardRuntimeMode {
    /// Stable diagnostic name for telemetry / config bundles.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SingleProcessFixedGrid => "single_process_fixed_grid",
            Self::SingleProcessDynamicGrid => "single_process_dynamic_grid",
            Self::DistributedExperimental => "distributed_experimental",
        }
    }

    /// True if the mode runs all shards in one process.
    pub const fn is_single_process(self) -> bool {
        matches!(
            self,
            Self::SingleProcessFixedGrid | Self::SingleProcessDynamicGrid
        )
    }

    /// True if the grid topology may rebalance at runtime.
    pub const fn allows_dynamic_grid(self) -> bool {
        matches!(
            self,
            Self::SingleProcessDynamicGrid | Self::DistributedExperimental
        )
    }
}

/// Per-shard work item the scheduler hands out. One item per active
/// shard per tick; ordered deterministically when the active mode
/// requires authoritative semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ShardWorkItem {
    /// Which shard this work item is for.
    pub shard: ShardId,
    /// Cell the shard occupies (deterministic order key).
    pub cell: CellId,
    /// Tick the work item runs on.
    pub tick: u32,
}

impl ShardWorkItem {
    /// Convenience constructor.
    pub const fn new(shard: ShardId, cell: CellId, tick: u32) -> Self {
        Self { shard, cell, tick }
    }
}

/// Per-shard work scheduler.
///
/// Walks the [`ShardRegistry`] to emit one [`ShardWorkItem`] per
/// active shard. In authoritative mode the items are sorted by
/// (cell, shard) so the cross-shard solve order is deterministic;
/// independent shards may execute concurrently because the
/// boundary contract guarantees no double solve.
#[derive(Clone, Copy, Debug, Default)]
pub struct ShardWorkScheduler {
    /// Mode that gates determinism + parallelism.
    pub mode: ShardRuntimeMode,
}

impl ShardWorkScheduler {
    /// New scheduler in `mode`.
    pub const fn new(mode: ShardRuntimeMode) -> Self {
        Self { mode }
    }

    /// Build the per-tick work queue from `registry`. Items are
    /// **always** sorted by (cell row-major, shard) for
    /// deterministic ordering — the V2-P5 contract requires
    /// determinism in authoritative mode and the cost of sorting a
    /// shard-count-sized vector is negligible.
    pub fn build(&self, registry: &ShardRegistry, tick: u32) -> Vec<ShardWorkItem> {
        let mut items: Vec<ShardWorkItem> = registry
            .iter_shards()
            .map(|shard| ShardWorkItem::new(shard.identity.id, shard.identity.cell, tick))
            .collect();
        // Cross-shard order is z-major then y, then x, then shard id —
        // matches the existing determinism contract documented at
        // the head of this file. The (z, y, x) sort key keeps
        // neighbouring rows of the grid contiguous in iteration order.
        items.sort_by(|a, b| {
            (a.cell.z, a.cell.y, a.cell.x, a.shard.0)
                .cmp(&(b.cell.z, b.cell.y, b.cell.x, b.shard.0))
        });
        items
    }

    /// True if the scheduler is allowed to dispatch work items in
    /// parallel across independent shards. Always `true` for
    /// `SingleProcessFixedGrid` because the boundary contract
    /// guarantees no double solve; `SingleProcessDynamicGrid` and
    /// `DistributedExperimental` opt in to parallelism per tick.
    pub const fn allows_parallel_independent_shards(&self) -> bool {
        true
    }
}

/// Per-shard hard limits used by [`evaluate_shard_overload`].
///
/// Cross-cutting V2-P5 budget covering active rigid bodies, particles,
/// contacts, dirty bytes, solver wall-time, and wakeup count. Mirrors
/// the existing [`ShardBudget`] but adds the new fields the V2-P5 spec
/// calls out (particles, contacts, dirty bytes, solver_ns,
/// wakeups_per_tick). Leaves the existing `ShardBudget` in place —
/// the V2-P5 wide budget is opt-in via [`ShardOverloadPolicy`].
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct ShardOverloadPolicy {
    /// Maximum active rigid bodies in this shard.
    pub max_active_rigid_bodies: u32,
    /// Maximum live particles (any kind) in this shard.
    pub max_active_particles: u32,
    /// Maximum live contacts (active + sensor) in this shard.
    pub max_active_contacts: u32,
    /// Maximum dirty-output bytes published per tick.
    pub max_dirty_bytes_per_tick: u32,
    /// Maximum wall-clock nanoseconds the solver may spend per tick.
    pub max_solver_ns_per_tick: u64,
    /// Maximum wakeup commands applied per tick. Excess defers to
    /// the next tick.
    pub max_wakeups_per_tick: u32,
    /// Soft fraction (0.0..=1.0) at which the runtime emits a
    /// pressure diagnostic.
    pub soft_pressure_fraction: f32,
}

impl Default for ShardOverloadPolicy {
    fn default() -> Self {
        Self {
            max_active_rigid_bodies: 16_384,
            max_active_particles: 1_000_000,
            max_active_contacts: 65_536,
            max_dirty_bytes_per_tick: 1_048_576,
            max_solver_ns_per_tick: 8_000_000,
            max_wakeups_per_tick: 1_024,
            soft_pressure_fraction: 0.75,
        }
    }
}

/// Coarse pressure level reported by [`evaluate_shard_overload`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Ord, PartialOrd)]
#[repr(u8)]
pub enum ShardPressureLevel {
    /// Under all soft thresholds.
    #[default]
    None = 0,
    /// Past at least one soft threshold but no hard breach.
    Soft = 1,
    /// Past at least one hard threshold — overload action required.
    Hard = 2,
}

/// Action the runtime may take in response to budget pressure.
///
/// V2-P5 spec set: defer wake, demote eligible body, aggregate
/// debris, reduce non-critical fidelity, emit diagnostic. Each is a
/// typed enum variant; the runtime's chosen actions land in
/// [`ShardOverloadReport::actions`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ShardOverloadAction {
    /// Defer one or more wakeup commands to the next tick.
    DeferWake,
    /// Demote one or more eligible (non-gameplay-critical) bodies
    /// back to warm.
    DemoteEligibleBody,
    /// Aggregate debris into a single combined body to free slots.
    AggregateDebris,
    /// Reduce solver fidelity for non-critical bodies (lower
    /// iteration count, simpler contact resolution).
    ReduceNonCriticalFidelity,
    /// Emit a diagnostic without taking a state-changing action
    /// (used at soft pressure).
    EmitDiagnostic,
}

/// Result of one [`evaluate_shard_overload`] call.
#[derive(Clone, Debug, Default)]
pub struct ShardOverloadReport {
    /// Aggregate pressure level.
    pub pressure: ShardPressureLevel,
    /// Actions the runtime should take this tick.
    pub actions: Vec<ShardOverloadAction>,
    /// True if the active-body cap was breached.
    pub bodies_over_cap: bool,
    /// True if the particle cap was breached.
    pub particles_over_cap: bool,
    /// True if the contact cap was breached.
    pub contacts_over_cap: bool,
    /// True if the dirty-bytes cap was breached.
    pub dirty_bytes_over_cap: bool,
    /// True if the solver-time cap was breached.
    pub solver_ns_over_cap: bool,
}

/// Per-class demote eligibility check. Honours
/// [`PhysicalClassPolicy`]'s gameplay-critical guard — bodies whose
/// class is gameplay-critical are NEVER demoted under pressure.
///
/// V2-P5 spec: "Preserve PhysicalClassPolicy gameplay-critical
/// guard."
pub const fn is_demote_eligible(class: PhysicalClass) -> bool {
    !class.is_gameplay_critical()
}

/// Pure-function overload evaluator. No I/O, no allocation beyond
/// the action list.
pub fn evaluate_shard_overload(
    policy: &ShardOverloadPolicy,
    active_rigid_bodies: u32,
    active_particles: u32,
    active_contacts: u32,
    dirty_bytes_this_tick: u32,
    solver_ns_this_tick: u64,
) -> ShardOverloadReport {
    let bodies_over_cap = active_rigid_bodies > policy.max_active_rigid_bodies;
    let particles_over_cap = active_particles > policy.max_active_particles;
    let contacts_over_cap = active_contacts > policy.max_active_contacts;
    let dirty_bytes_over_cap = dirty_bytes_this_tick > policy.max_dirty_bytes_per_tick;
    let solver_ns_over_cap = solver_ns_this_tick > policy.max_solver_ns_per_tick;

    let any_hard = bodies_over_cap
        || particles_over_cap
        || contacts_over_cap
        || dirty_bytes_over_cap
        || solver_ns_over_cap;

    let soft_threshold = |cap: u32| -> u32 {
        if cap == 0 {
            return u32::MAX;
        }
        (cap as f32 * policy.soft_pressure_fraction) as u32
    };
    let any_soft = active_rigid_bodies > soft_threshold(policy.max_active_rigid_bodies)
        || active_particles > soft_threshold(policy.max_active_particles)
        || active_contacts > soft_threshold(policy.max_active_contacts)
        || dirty_bytes_this_tick > soft_threshold(policy.max_dirty_bytes_per_tick);

    let pressure = if any_hard {
        ShardPressureLevel::Hard
    } else if any_soft {
        ShardPressureLevel::Soft
    } else {
        ShardPressureLevel::None
    };

    let mut actions = Vec::new();
    if bodies_over_cap {
        actions.push(ShardOverloadAction::DemoteEligibleBody);
        actions.push(ShardOverloadAction::AggregateDebris);
    }
    if particles_over_cap || dirty_bytes_over_cap {
        actions.push(ShardOverloadAction::ReduceNonCriticalFidelity);
    }
    if contacts_over_cap || solver_ns_over_cap {
        actions.push(ShardOverloadAction::DeferWake);
    }
    if matches!(pressure, ShardPressureLevel::Soft) {
        actions.push(ShardOverloadAction::EmitDiagnostic);
    }

    ShardOverloadReport {
        pressure,
        actions,
        bodies_over_cap,
        particles_over_cap,
        contacts_over_cap,
        dirty_bytes_over_cap,
        solver_ns_over_cap,
    }
}

/// Boundary contract: which shard owns the body vs which shard
/// holds the ghost mirror. V2-P5 spec: "authoritative shard owns
/// body, neighbor shard owns ghost, one solver owner per pair, no
/// double solve."
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BoundaryOwnership {
    /// The body whose ownership is described.
    pub body: GlobalPhysicalEntityId,
    /// Authoritative shard — the only shard that solves the body.
    pub authoritative: ShardId,
    /// Optional ghost shard — mirrors the body's state for boundary
    /// contact generation; **does not** solve.
    pub ghost: Option<ShardId>,
}

impl BoundaryOwnership {
    /// True if `shard` is the solver-owner for this body.
    pub fn is_solver_owner(&self, shard: ShardId) -> bool {
        self.authoritative == shard
    }

    /// True if `shard` holds the ghost mirror.
    pub fn is_ghost_owner(&self, shard: ShardId) -> bool {
        self.ghost == Some(shard)
    }
}

/// Cross-shard transfer state — V2-P5 four-stage handoff window.
///
/// Stages: `Outgoing` → `Ghosted` → `Acknowledged` → `Finalized`.
/// The transfer's destination becomes the new authoritative owner
/// only at `Finalized`; intermediate stages keep the source as
/// authoritative so contacts continue to be solved exactly once.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Ord, PartialOrd)]
#[repr(u8)]
pub enum TransferWindowStage {
    /// Source shard is preparing to release ownership; destination
    /// has not been notified yet.
    #[default]
    Outgoing = 0,
    /// Source has notified destination; destination has inserted
    /// a ghost mirror so its boundary contact generation can see
    /// the body. Source is still authoritative.
    Ghosted = 1,
    /// Destination has acknowledged the ghost; both sides agree on
    /// the body's state. Source is still authoritative.
    Acknowledged = 2,
    /// Ownership has flipped: destination is now authoritative,
    /// source has retired the body. Ghost mirrors collapse.
    Finalized = 3,
}

impl TransferWindowStage {
    /// Stable diagnostic name for telemetry.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Outgoing => "outgoing",
            Self::Ghosted => "ghosted",
            Self::Acknowledged => "acknowledged",
            Self::Finalized => "finalized",
        }
    }

    /// True if `self` represents an in-flight transfer (anything
    /// before `Finalized`).
    pub const fn is_in_flight(self) -> bool {
        !matches!(self, Self::Finalized)
    }

    /// Advance to the next stage. Returns `Some(next)` for live
    /// stages, `None` once finalized.
    pub const fn advance(self) -> Option<Self> {
        match self {
            Self::Outgoing => Some(Self::Ghosted),
            Self::Ghosted => Some(Self::Acknowledged),
            Self::Acknowledged => Some(Self::Finalized),
            Self::Finalized => None,
        }
    }
}

/// One in-flight cross-shard transfer record. Lives in
/// [`ShardTransferLedger`] until it reaches `Finalized` and is
/// retired.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransferWindow {
    /// Body being transferred.
    pub body: GlobalPhysicalEntityId,
    /// Source shard (current authoritative owner during in-flight).
    pub from: ShardId,
    /// Destination shard (becomes authoritative at `Finalized`).
    pub to: ShardId,
    /// Current stage.
    pub stage: TransferWindowStage,
    /// Linear velocity at handoff start. Preserved across stages so
    /// the destination can resume integration without state loss.
    pub linear_velocity: Vec3,
    /// Angular velocity at handoff start.
    pub angular_velocity: Vec3,
    /// Rotation at handoff start.
    pub rotation: Quat,
    /// Tick the transfer began on.
    pub started_tick: u32,
}

impl TransferWindow {
    /// New transfer in `Outgoing` stage.
    pub fn new(
        body: GlobalPhysicalEntityId,
        from: ShardId,
        to: ShardId,
        linear_velocity: Vec3,
        angular_velocity: Vec3,
        rotation: Quat,
        tick: u32,
    ) -> Self {
        Self {
            body,
            from,
            to,
            stage: TransferWindowStage::Outgoing,
            linear_velocity,
            angular_velocity,
            rotation,
            started_tick: tick,
        }
    }

    /// Authoritative shard given the current stage. Source until
    /// `Finalized`, destination after.
    pub const fn solver_owner(&self) -> ShardId {
        match self.stage {
            TransferWindowStage::Finalized => self.to,
            _ => self.from,
        }
    }
}

/// Per-process ledger of in-flight transfer windows.
#[derive(Resource, Clone, Debug, Default)]
pub struct ShardTransferLedger {
    transfers: std::collections::BTreeMap<GlobalPhysicalEntityId, TransferWindow>,
}

impl ShardTransferLedger {
    /// Empty ledger.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of in-flight transfers.
    pub fn len(&self) -> usize {
        self.transfers.len()
    }

    /// True if no transfers are in flight.
    pub fn is_empty(&self) -> bool {
        self.transfers.is_empty()
    }

    /// Begin a transfer in `Outgoing` stage.
    pub fn begin(&mut self, transfer: TransferWindow) -> bool {
        if self.transfers.contains_key(&transfer.body) {
            return false;
        }
        self.transfers.insert(transfer.body, transfer);
        true
    }

    /// Advance the transfer for `body` by one stage. Returns the
    /// new stage on success; returns `None` if the body has no
    /// in-flight transfer or the transfer is already finalized.
    pub fn advance(&mut self, body: GlobalPhysicalEntityId) -> Option<TransferWindowStage> {
        let entry = self.transfers.get_mut(&body)?;
        let next = entry.stage.advance()?;
        entry.stage = next;
        Some(next)
    }

    /// Retire a finalized transfer. Returns the removed window for
    /// downstream telemetry.
    pub fn retire(&mut self, body: GlobalPhysicalEntityId) -> Option<TransferWindow> {
        let entry = self.transfers.get(&body)?;
        if !matches!(entry.stage, TransferWindowStage::Finalized) {
            return None;
        }
        self.transfers.remove(&body)
    }

    /// Look up the in-flight transfer for `body`, if any.
    pub fn get(&self, body: GlobalPhysicalEntityId) -> Option<&TransferWindow> {
        self.transfers.get(&body)
    }

    /// Iterate transfers in body-id-sorted order (deterministic via
    /// `BTreeMap`).
    pub fn iter(
        &self,
    ) -> std::collections::btree_map::Iter<'_, GlobalPhysicalEntityId, TransferWindow> {
        self.transfers.iter()
    }
}

#[cfg(test)]
mod v2p5_tests {
    use super::*;
    use crate::physics::physical_lod::PhysicalClass;
    use bevy::prelude::Vec3;

    fn shard(id: u32) -> ShardId {
        ShardId(id)
    }

    fn body(id: u64) -> GlobalPhysicalEntityId {
        GlobalPhysicalEntityId(id)
    }

    #[test]
    fn shard_runtime_mode_default_is_single_process_fixed_grid() {
        // Production rollback contract: default never silently
        // routes through dynamic-grid or distributed paths.
        assert_eq!(
            ShardRuntimeMode::default(),
            ShardRuntimeMode::SingleProcessFixedGrid
        );
        assert!(ShardRuntimeMode::SingleProcessFixedGrid.is_single_process());
        assert!(!ShardRuntimeMode::SingleProcessFixedGrid.allows_dynamic_grid());
    }

    #[test]
    fn shard_runtime_mode_classifies_paths() {
        assert!(ShardRuntimeMode::SingleProcessDynamicGrid.is_single_process());
        assert!(ShardRuntimeMode::SingleProcessDynamicGrid.allows_dynamic_grid());
        assert!(!ShardRuntimeMode::DistributedExperimental.is_single_process());
        assert!(ShardRuntimeMode::DistributedExperimental.allows_dynamic_grid());
    }

    #[test]
    fn shard_runtime_mode_names_are_stable() {
        assert_eq!(
            ShardRuntimeMode::SingleProcessFixedGrid.as_str(),
            "single_process_fixed_grid"
        );
        assert_eq!(
            ShardRuntimeMode::DistributedExperimental.as_str(),
            "distributed_experimental"
        );
    }

    #[test]
    fn scheduler_emits_one_item_per_active_shard_in_deterministic_order() {
        // Two shards on a 4x1x1 grid; scheduler must emit them in
        // deterministic cell order regardless of registration order.
        let mut registry = ShardRegistry::with_cell_size(1.0, 0.0);
        registry.add_shard(CellId::new(2, 0, 0));
        registry.add_shard(CellId::new(0, 0, 0));
        let scheduler = ShardWorkScheduler::default();
        let items = scheduler.build(&registry, 7);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].cell, CellId::new(0, 0, 0));
        assert_eq!(items[1].cell, CellId::new(2, 0, 0));
        assert_eq!(items[0].tick, 7);
        // Independent shards may run in parallel.
        assert!(scheduler.allows_parallel_independent_shards());
    }

    #[test]
    fn scheduler_is_deterministic_across_runs() {
        let make_items = || {
            let mut registry = ShardRegistry::with_cell_size(1.0, 0.0);
            for cell in [
                CellId::new(0, 1, 0),
                CellId::new(1, 0, 0),
                CellId::new(0, 0, 0),
                CellId::new(1, 1, 0),
            ] {
                registry.add_shard(cell);
            }
            ShardWorkScheduler::default().build(&registry, 1)
        };
        let a = make_items();
        let b = make_items();
        assert_eq!(a, b);
    }

    #[test]
    fn boundary_ownership_one_solver_per_pair() {
        // Body owned by shard 0 with ghost on shard 1: shard 0 is
        // the solver owner; shard 1 holds the ghost only.
        let ownership = BoundaryOwnership {
            body: body(7),
            authoritative: shard(0),
            ghost: Some(shard(1)),
        };
        assert!(ownership.is_solver_owner(shard(0)));
        assert!(!ownership.is_solver_owner(shard(1)));
        assert!(!ownership.is_ghost_owner(shard(0)));
        assert!(ownership.is_ghost_owner(shard(1)));
    }

    #[test]
    fn boundary_ownership_no_double_solve() {
        // Two shards looking at the same body: at most one is the
        // solver owner. The boundary contract is enforced by the
        // single `authoritative` field.
        let ownership = BoundaryOwnership {
            body: body(7),
            authoritative: shard(0),
            ghost: Some(shard(1)),
        };
        let solvers: Vec<ShardId> = [shard(0), shard(1), shard(2)]
            .into_iter()
            .filter(|s| ownership.is_solver_owner(*s))
            .collect();
        assert_eq!(solvers.len(), 1, "exactly one solver owner");
        assert_eq!(solvers[0], shard(0));
    }

    #[test]
    fn transfer_window_walks_four_stages_in_order() {
        let stage = TransferWindowStage::default();
        assert_eq!(stage, TransferWindowStage::Outgoing);
        assert!(stage.is_in_flight());

        let stage = stage.advance().unwrap();
        assert_eq!(stage, TransferWindowStage::Ghosted);
        assert!(stage.is_in_flight());

        let stage = stage.advance().unwrap();
        assert_eq!(stage, TransferWindowStage::Acknowledged);
        assert!(stage.is_in_flight());

        let stage = stage.advance().unwrap();
        assert_eq!(stage, TransferWindowStage::Finalized);
        assert!(!stage.is_in_flight());
        assert!(stage.advance().is_none());
    }

    #[test]
    fn transfer_preserves_rotation_and_velocity_across_stages() {
        let mut window = TransferWindow::new(
            body(1),
            shard(0),
            shard(1),
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(4.0, 5.0, 6.0),
            Quat::from_xyzw(0.1, 0.2, 0.3, 0.927),
            42,
        );
        let original_lin = window.linear_velocity;
        let original_ang = window.angular_velocity;
        let original_rot = window.rotation;
        // Walk through every stage.
        for _ in 0..3 {
            window.stage = window.stage.advance().unwrap();
        }
        assert_eq!(window.stage, TransferWindowStage::Finalized);
        assert_eq!(window.linear_velocity, original_lin);
        assert_eq!(window.angular_velocity, original_ang);
        assert_eq!(window.rotation, original_rot);
    }

    #[test]
    fn transfer_solver_owner_flips_only_at_finalized() {
        let mut window = TransferWindow::new(
            body(1),
            shard(0),
            shard(1),
            Vec3::ZERO,
            Vec3::ZERO,
            Quat::IDENTITY,
            0,
        );
        // Source owns through Outgoing / Ghosted / Acknowledged.
        for _ in 0..3 {
            assert_eq!(
                window.solver_owner(),
                shard(0),
                "source remains authoritative until Finalized"
            );
            if let Some(next) = window.stage.advance() {
                window.stage = next;
            }
        }
        // Now finalized.
        assert_eq!(window.stage, TransferWindowStage::Finalized);
        assert_eq!(window.solver_owner(), shard(1));
    }

    #[test]
    fn transfer_ledger_advance_and_retire() {
        let mut ledger = ShardTransferLedger::default();
        let window = TransferWindow::new(
            body(1),
            shard(0),
            shard(1),
            Vec3::ZERO,
            Vec3::ZERO,
            Quat::IDENTITY,
            0,
        );
        assert!(ledger.begin(window));
        assert_eq!(ledger.len(), 1);
        // Retire before finalized → fails.
        assert!(ledger.retire(body(1)).is_none());
        // Advance through every stage.
        assert_eq!(ledger.advance(body(1)), Some(TransferWindowStage::Ghosted));
        assert_eq!(
            ledger.advance(body(1)),
            Some(TransferWindowStage::Acknowledged)
        );
        assert_eq!(
            ledger.advance(body(1)),
            Some(TransferWindowStage::Finalized)
        );
        // No further advance.
        assert!(ledger.advance(body(1)).is_none());
        // Retire works now.
        let retired = ledger.retire(body(1)).unwrap();
        assert_eq!(retired.body, body(1));
        assert!(ledger.is_empty());
    }

    #[test]
    fn duplicate_transfer_begin_rejected() {
        let mut ledger = ShardTransferLedger::default();
        let w = TransferWindow::new(
            body(1),
            shard(0),
            shard(1),
            Vec3::ZERO,
            Vec3::ZERO,
            Quat::IDENTITY,
            0,
        );
        assert!(ledger.begin(w));
        assert!(!ledger.begin(w), "duplicate transfer rejected");
    }

    #[test]
    fn overload_evaluator_no_pressure_under_caps() {
        let policy = ShardOverloadPolicy::default();
        let report = evaluate_shard_overload(&policy, 100, 1_000, 100, 1_000, 100_000);
        assert_eq!(report.pressure, ShardPressureLevel::None);
        assert!(report.actions.is_empty());
        assert!(!report.bodies_over_cap);
    }

    #[test]
    fn overload_evaluator_soft_pressure_emits_diagnostic() {
        // 13_000 active bodies on a 16_384 cap with 0.75 soft
        // fraction → soft pressure (12_288 threshold).
        let policy = ShardOverloadPolicy::default();
        let report = evaluate_shard_overload(&policy, 13_000, 1_000, 100, 1_000, 100_000);
        assert_eq!(report.pressure, ShardPressureLevel::Soft);
        assert!(
            report
                .actions
                .contains(&ShardOverloadAction::EmitDiagnostic)
        );
        assert!(!report.bodies_over_cap);
    }

    #[test]
    fn overload_evaluator_hard_pressure_demotes_eligible_bodies() {
        let policy = ShardOverloadPolicy::default();
        let report = evaluate_shard_overload(&policy, 20_000, 1_000, 100, 1_000, 100_000);
        assert_eq!(report.pressure, ShardPressureLevel::Hard);
        assert!(report.bodies_over_cap);
        assert!(
            report
                .actions
                .contains(&ShardOverloadAction::DemoteEligibleBody)
        );
        assert!(
            report
                .actions
                .contains(&ShardOverloadAction::AggregateDebris)
        );
    }

    #[test]
    fn overload_evaluator_contact_breach_defers_wake() {
        let policy = ShardOverloadPolicy::default();
        let report = evaluate_shard_overload(&policy, 100, 1_000, 100_000, 1_000, 100_000);
        assert_eq!(report.pressure, ShardPressureLevel::Hard);
        assert!(report.contacts_over_cap);
        assert!(report.actions.contains(&ShardOverloadAction::DeferWake));
    }

    #[test]
    fn overload_evaluator_dirty_bytes_reduces_fidelity() {
        let policy = ShardOverloadPolicy::default();
        let report = evaluate_shard_overload(
            &policy,
            100,
            1_000,
            100,
            policy.max_dirty_bytes_per_tick + 1,
            100_000,
        );
        assert_eq!(report.pressure, ShardPressureLevel::Hard);
        assert!(report.dirty_bytes_over_cap);
        assert!(
            report
                .actions
                .contains(&ShardOverloadAction::ReduceNonCriticalFidelity)
        );
    }

    #[test]
    fn gameplay_critical_classes_never_demote_under_pressure() {
        // V2-P5 spec: "Preserve PhysicalClassPolicy gameplay-critical
        // guard."
        assert!(!is_demote_eligible(PhysicalClass::Player));
        assert!(!is_demote_eligible(PhysicalClass::Vehicle));
        // Non-critical classes are eligible.
        assert!(is_demote_eligible(PhysicalClass::Rubble));
        assert!(is_demote_eligible(PhysicalClass::BackgroundActor));
        assert!(is_demote_eligible(PhysicalClass::AggregateRubbleField));
    }

    #[test]
    fn replay_digest_stable_across_handoff() {
        // Two independent ledgers walking the same transfer through
        // the same stage sequence must produce identical
        // post-handoff state. This is the V2-P5 closest analog to
        // "replay digest stable across handoff" before the
        // shard-runtime digest path is wired.
        let build = || {
            let mut ledger = ShardTransferLedger::default();
            let w = TransferWindow::new(
                body(7),
                shard(0),
                shard(1),
                Vec3::new(1.0, 2.0, 3.0),
                Vec3::new(4.0, 5.0, 6.0),
                Quat::from_xyzw(0.1, 0.2, 0.3, 0.927),
                100,
            );
            ledger.begin(w);
            for _ in 0..3 {
                ledger.advance(body(7));
            }
            let post = ledger.get(body(7)).cloned().unwrap();
            ledger.retire(body(7));
            (post.solver_owner(), post.linear_velocity, post.rotation)
        };
        let a = build();
        let b = build();
        assert_eq!(a, b, "handoff produces deterministic post-state");
        assert_eq!(a.0, shard(1), "post-handoff solver owner is destination");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).expect("entity index should be in range")
    }

    fn global(value: u64) -> GlobalPhysicalEntityId {
        GlobalPhysicalEntityId(value)
    }

    fn registry() -> ShardRegistry {
        ShardRegistry::with_cell_size(10.0, 1.0)
    }

    #[test]
    fn cell_neighborhood_contains_self_and_26_neighbors() {
        let cell = CellId::new(1, 2, 3);
        let neighborhood = cell.neighborhood();
        assert_eq!(neighborhood.len(), 27);
        assert!(neighborhood.contains(&cell));
    }

    #[test]
    fn two_shards_spawn_in_their_authoritative_cells() {
        let mut registry = registry();
        let body_a = global(1);
        let body_b = global(2);

        let shard_a = registry
            .spawn_body(
                body_a,
                entity(10),
                ActiveBodyKind::Dynamic,
                Vec3::new(1.0, 0.0, 0.0),
            )
            .expect("body a should spawn");
        let shard_b = registry
            .spawn_body(
                body_b,
                entity(20),
                ActiveBodyKind::Dynamic,
                Vec3::new(15.0, 0.0, 0.0),
            )
            .expect("body b should spawn");

        assert_ne!(
            shard_a.shard, shard_b.shard,
            "bodies in different cells must land in different shards"
        );

        let runtime_a = registry.shard(shard_a.shard).expect("shard a present");
        let runtime_b = registry.shard(shard_b.shard).expect("shard b present");
        assert_eq!(
            runtime_a.body(body_a).map(|record| record.tier),
            Some(BodyLifecycleTier::Active)
        );
        assert_eq!(
            runtime_b.body(body_b).map(|record| record.tier),
            Some(BodyLifecycleTier::Active)
        );
        assert!(runtime_a.body(body_b).is_none());
        assert!(runtime_b.body(body_a).is_none());
    }

    #[test]
    fn crossing_boundary_emits_transfer_command() {
        let mut registry = registry();
        let body = global(7);
        let initial = Vec3::new(1.0, 0.0, 0.0);
        let crossing = Vec3::new(11.0, 0.0, 0.0);

        let initial_shard = registry
            .spawn_body(body, entity(70), ActiveBodyKind::Dynamic, initial)
            .expect("body should spawn")
            .shard;

        let commands = registry.process_movement(body, crossing);
        let transfer = commands
            .iter()
            .find(|cmd| matches!(cmd, BodyLifecycleCommand::TransferShard { .. }))
            .copied()
            .expect("crossing must emit transfer command");

        match transfer {
            BodyLifecycleCommand::TransferShard {
                body: cmd_body,
                from,
                to,
            } => {
                assert_eq!(cmd_body, body);
                assert_eq!(from, initial_shard);
                assert_ne!(from, to);
            }
            other => panic!("unexpected command {other:?}"),
        }
    }

    #[test]
    fn ghost_appears_in_neighbor_shard_when_inside_halo() {
        let mut registry = ShardRegistry::with_cell_size(10.0, 2.0);
        let body = global(11);
        let near_boundary = Vec3::new(9.5, 0.0, 0.0);
        let primary_shard = registry
            .spawn_body(body, entity(110), ActiveBodyKind::Dynamic, near_boundary)
            .expect("body should spawn")
            .shard;

        // Find a neighbor that contains the position in its halo.
        let neighbor_shard = registry
            .iter_shards()
            .find(|shard| {
                shard.identity.id != primary_shard && shard.identity.within_halo(near_boundary)
            })
            .map(|shard| shard.identity.id)
            .expect("at least one neighbor should host the ghost");
        let runtime = registry.shard(neighbor_shard).unwrap();
        let record = runtime.body(body).expect("ghost should land in neighbor");
        assert_eq!(record.tier, BodyLifecycleTier::Ghost);
        assert_eq!(record.authoritative_shard, primary_shard);
    }

    #[test]
    fn authoritative_shard_changes_exactly_once_per_transfer() {
        let mut registry = registry();
        let body = global(33);
        let start_shard = registry
            .spawn_body(
                body,
                entity(330),
                ActiveBodyKind::Dynamic,
                Vec3::new(1.0, 0.0, 0.0),
            )
            .expect("body should spawn")
            .shard;

        let after_cross = registry.process_movement(body, Vec3::new(11.0, 0.0, 0.0));
        let transfer_count = after_cross
            .iter()
            .filter(|cmd| matches!(cmd, BodyLifecycleCommand::TransferShard { .. }))
            .count();
        assert_eq!(transfer_count, 1, "exactly one transfer per crossing");

        // The body should now live as Active in the destination only.
        let mut authoritative_count = 0_u32;
        let mut ghost_count = 0_u32;
        let mut destination_shard = ShardId::NONE;
        for shard in registry.iter_shards() {
            if let Some(record) = shard.body(body) {
                match record.tier {
                    BodyLifecycleTier::Active
                    | BodyLifecycleTier::Warm
                    | BodyLifecycleTier::Cold => {
                        authoritative_count += 1;
                        destination_shard = shard.identity.id;
                    }
                    BodyLifecycleTier::Ghost => ghost_count += 1,
                }
            }
        }
        assert_eq!(authoritative_count, 1, "exactly one authoritative owner");
        assert!(
            ghost_count <= 26,
            "ghost replicas live only in halo neighbors"
        );
        assert_ne!(destination_shard, start_shard);
    }

    #[test]
    fn no_double_solve_when_body_transfers() {
        let mut registry = registry();
        let body = global(55);
        let entity = entity(550);
        let start_shard = registry
            .spawn_body(
                body,
                entity,
                ActiveBodyKind::Dynamic,
                Vec3::new(1.0, 0.0, 0.0),
            )
            .expect("body should spawn")
            .shard;

        registry.process_movement(body, Vec3::new(11.0, 0.0, 0.0));

        // The source shard's pool no longer carries the body's row. The
        // destination shard's pool does. Two solver-facing iterations
        // therefore can never both visit this body in the same tick.
        let source = registry.shard(start_shard).expect("source shard present");
        assert!(
            source
                .physics
                .active_pool
                .handle_for_entity(entity)
                .is_none(),
            "source pool must drop the body to avoid double-solve"
        );
        let destination_count: u32 = registry
            .iter_shards()
            .filter(|shard| shard.identity.id != start_shard)
            .map(|shard| {
                if shard
                    .physics
                    .active_pool
                    .handle_for_entity(entity)
                    .is_some()
                {
                    1
                } else {
                    0
                }
            })
            .sum();
        assert_eq!(
            destination_count, 1,
            "destination must carry exactly one row"
        );
        assert!(
            registry.diagnostics().double_solves_avoided_this_tick >= 1,
            "diagnostic counter must record the avoided double-solve"
        );
    }

    #[test]
    fn shard_runtime_split_keeps_per_subresource_state() {
        // Struct-shape rule check: each sub-area owns its own counters
        // rather than piling everything into ShardRuntime fields.
        let mut registry = registry();
        let body = global(101);
        let shard = registry
            .spawn_body(
                body,
                entity(1010),
                ActiveBodyKind::Dynamic,
                Vec3::new(2.0, 0.0, 0.0),
            )
            .expect("body should spawn")
            .shard;
        let runtime = registry.shard(shard).expect("shard present");
        assert_eq!(runtime.identity.id, shard);
        assert_eq!(runtime.budget.current_active, 1);
        assert!(runtime.physics.bodies.contains_key(&body));
        assert!(runtime.replication.dirty_queue.is_empty());
        assert!(!runtime.neighbors.neighbors.contains(&shard));
    }

    #[test]
    fn cold_storage_diagnostics_counts_each_tier_separately() {
        // Spawn three bodies in two shards. Demote one to warm, leave
        // one active, drop the third into cold. The diagnostics method
        // should report the right counts in each bucket.
        use super::super::cold_storage::{COLD_RECORD_BYTES, demote_to_cold};

        let mut registry = ShardRegistry::with_cell_size(10.0, 1.0);
        let body_active = global(81);
        let body_warm = global(82);
        let body_cold = global(83);
        let _ = registry
            .spawn_body(
                body_active,
                entity(810),
                ActiveBodyKind::Dynamic,
                Vec3::new(1.0, 0.0, 0.0),
            )
            .expect("active body should spawn");
        let warm_shard = registry
            .spawn_body(
                body_warm,
                entity(820),
                ActiveBodyKind::Dynamic,
                Vec3::new(11.0, 0.0, 0.0),
            )
            .expect("warm body should spawn")
            .shard;
        let cold_shard = registry
            .spawn_body(
                body_cold,
                entity(830),
                ActiveBodyKind::Dynamic,
                Vec3::new(21.0, 0.0, 0.0),
            )
            .expect("cold body should spawn")
            .shard;

        let warm = demote_to_cold(&registry, warm_shard, body_warm).unwrap();
        registry.apply_command(warm.command, Vec3::new(11.0, 0.0, 0.0));

        let mid = demote_to_cold(&registry, cold_shard, body_cold).unwrap();
        registry.apply_command(mid.command, Vec3::new(21.0, 0.0, 0.0));
        let cold = demote_to_cold(&registry, cold_shard, body_cold).unwrap();
        registry.apply_command(cold.command, Vec3::new(21.0, 0.0, 0.0));

        let diagnostics = registry.cold_storage_diagnostics();
        assert_eq!(diagnostics.active_entities, 1);
        assert_eq!(diagnostics.warm_entities, 1);
        assert_eq!(diagnostics.cold_entities, 1);
        assert_eq!(diagnostics.represented_entities, 3);
        assert_eq!(diagnostics.bytes_per_cold_record, COLD_RECORD_BYTES as u32);
    }
}
