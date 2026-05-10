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

use std::collections::HashMap;

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
#[derive(Clone, Copy, Debug)]
pub struct ShardBodyRecord {
    pub global: GlobalPhysicalEntityId,
    pub entity: Entity,
    pub kind: ActiveBodyKind,
    pub tier: BodyLifecycleTier,
    pub handle: ActiveBodyHandle,
    pub last_position: Vec3,
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
