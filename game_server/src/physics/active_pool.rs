//! Shard-local dense active physics rows for the Fun game server.
//!
//! ECS remains the authoritative composition, identity, and lifecycle
//! surface; this module owns the dense data plane that the solver and
//! extraction pipelines need: per-entity active body rows kept in
//! cache-friendly `Vec` storage, a generational handle map, and dirty-row
//! tracking for downstream replication or writeback.
//!
//! # Scope of this prototype
//!
//! - Only the pool data structure, lifecycle bridge, extraction system,
//!   writeback system, and diagnostics. No Avian public API churn.
//! - No persistence metadata, networking reliability mode, gameplay tags,
//!   material/render data, editor labels, or long debug names land in
//!   [`ActiveBodyRow`]. Those stay in ECS.
//! - Generic primitives (handle layout, dirty masks) intentionally live here
//!   first so the shape can stabilize before any of it migrates into Avian.

use core::sync::atomic::{AtomicU32, Ordering};
use std::collections::HashMap;

use avian3d::prelude::{
    ActiveBodySource as AvianActiveBodySource, ActiveContactSource as AvianActiveContactSource,
    ActiveDynamicBody, ActiveKinematicBody, ActivePoolHandle as AvianActivePoolHandle,
    AngularVelocity, DenseActivePhysicsPlugin, LinearVelocity, PhysicsActiveSet, PhysicsGhost,
    PhysicsSchedule, PhysicsStepSystems, Position, Rotation, ShardTransferPending, SleepingBody,
};
use bevy::ecs::system::{ParamSet, SystemParam};
use bevy::prelude::*;

/// Stable per-entity slot id inside the [`ActivePhysicsPool`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, PartialOrd, Ord, Reflect)]
#[reflect(Debug, PartialEq, Hash)]
pub struct ActiveBodyId(pub u32);

impl ActiveBodyId {
    /// Numeric slot index used by dense-pool storage.
    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// Generation counter that disambiguates reused [`ActiveBodyId`] slots.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, PartialOrd, Ord, Reflect)]
#[reflect(Debug, PartialEq, Hash)]
pub struct ActiveBodyGeneration(pub u32);

impl ActiveBodyGeneration {
    pub const ZERO: Self = Self(0);
    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// Generational handle for an active body row. Always passed by value.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, PartialOrd, Ord, Reflect)]
#[reflect(Debug, PartialEq, Hash)]
pub struct ActiveBodyHandle {
    pub id: ActiveBodyId,
    pub generation: ActiveBodyGeneration,
}

/// ECS-side bridge component that lets gameplay/replication code resolve an
/// entity to its dense pool row.
#[derive(Component, Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Reflect)]
#[reflect(Component, Debug, PartialEq, Hash)]
#[component(storage = "Table")]
pub struct ActivePhysicsHandle(pub ActiveBodyHandle);

impl ActivePhysicsHandle {
    pub const fn handle(self) -> ActiveBodyHandle {
        self.0
    }
}

/// What kind of active body the pool row backs. Mirrors the Avian markers
/// without coupling the row layout to the ECS marker components.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub enum ActiveBodyKind {
    /// Active dynamic rigid body (default).
    #[default]
    Dynamic,
    /// Active kinematic rigid body driven by gameplay.
    Kinematic,
}

/// Bitmask describing which sub-state of an active body row was changed in
/// the current tick. Used to drive sparse writeback.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct ActivePoolDirtyMask(u8);

impl ActivePoolDirtyMask {
    pub const NONE: Self = Self(0);
    pub const POSITION: Self = Self(1 << 0);
    pub const ROTATION: Self = Self(1 << 1);
    pub const LINEAR_VELOCITY: Self = Self(1 << 2);
    pub const ANGULAR_VELOCITY: Self = Self(1 << 3);
    pub const TRANSFORM: Self = Self(0b0000_0011);
    pub const VELOCITY: Self = Self(0b0000_1100);
    pub const ALL: Self = Self(0b0000_1111);

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn is_clean(self) -> bool {
        self.0 == 0
    }

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn difference(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
}

/// Dense per-tick active body record. Layout intentionally avoids long debug
/// strings, persistence metadata, and replication policy — those live in ECS.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActiveBodyRow {
    /// The owning ECS entity. Used to resolve writeback targets without
    /// re-querying the world.
    pub entity: Entity,
    pub position: Vec3,
    pub rotation: Quat,
    pub linear_velocity: Vec3,
    pub angular_velocity: Vec3,
    pub body_kind: ActiveBodyKind,
}

impl ActiveBodyRow {
    fn empty(entity: Entity) -> Self {
        Self {
            entity,
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            linear_velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            body_kind: ActiveBodyKind::Dynamic,
        }
    }
}

/// Dense per-tick dirty-row record consumed by writeback or replication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveBodyDirtyRow {
    pub handle: ActiveBodyHandle,
    pub dirty_mask: ActivePoolDirtyMask,
    pub tick: u32,
}

/// What writeback strategy the pool applies when publishing dirty rows back
/// to ECS components each tick.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub enum ActivePoolWritebackMode {
    /// Default: write any dirty component back (tracks dirty-mask precisely).
    #[default]
    DirtyOnly,
    /// Always write Position/Rotation/Linear/Angular velocity for all rows;
    /// useful for legacy comparisons against the pre-pool ECS pipeline.
    Unconditional,
}

/// Bounds and ordering policy applied to the pool. Inserted as a resource
/// before [`ActivePhysicsPoolPlugin`].
#[derive(Resource, Clone, Copy, Debug)]
pub struct ActivePoolPolicy {
    /// Maximum number of active rows the pool can hold simultaneously.
    pub capacity: u32,
    /// If true, the pool walks rows in deterministic slot order during
    /// extraction/writeback. If false, no deterministic guarantee is made
    /// (still safe — Bevy's parallel iteration policy applies).
    pub deterministic_order: bool,
    /// Writeback strategy used by [`writeback_active_bodies_to_ecs`].
    pub writeback_mode: ActivePoolWritebackMode,
    /// Maximum number of new rows allocated per tick. Excess
    /// promotions wait for the next tick. `0` means unlimited.
    pub max_promotions_per_tick: u32,
}

impl Default for ActivePoolPolicy {
    fn default() -> Self {
        Self {
            capacity: 16_384,
            deterministic_order: true,
            writeback_mode: ActivePoolWritebackMode::DirtyOnly,
            max_promotions_per_tick: 0,
        }
    }
}

/// Per-tick counters for the active pool. Cheap to read from a benchmark or
/// diagnostics overlay.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct ActivePoolDiagnostics {
    pub capacity: u32,
    pub active_rows: u32,
    pub free_slots: u32,
    pub allocations_this_tick: u32,
    pub stale_handle_rejections: u32,
    pub extraction_count: u32,
    pub writeback_count: u32,
    pub promotions_deferred: u32,
}

/// Internal slot record. Holds the row plus metadata used by the generational
/// handle map.
#[derive(Clone, Copy, Debug)]
struct PoolSlot {
    row: ActiveBodyRow,
    generation: ActiveBodyGeneration,
    occupied: bool,
}

impl PoolSlot {
    fn empty(generation: ActiveBodyGeneration) -> Self {
        Self {
            row: ActiveBodyRow::empty(Entity::PLACEHOLDER),
            generation,
            occupied: false,
        }
    }
}

/// Shard-local dense active body pool.
///
/// The pool is *not* the source of truth; ECS is. The pool is a frame-local
/// dense data plane that mirrors ECS active rows for cache-friendly solver
/// and replication scans. Lifecycle changes propagate from ECS to the pool
/// through the [`extract_active_bodies_from_ecs`] system; downstream writes
/// flow back through [`writeback_active_bodies_to_ecs`].
#[derive(Resource, Debug, Default)]
pub struct ActivePhysicsPool {
    slots: Vec<PoolSlot>,
    free_slots: Vec<u32>,
    handles_by_entity: HashMap<Entity, ActiveBodyHandle>,
    dirty_rows: Vec<ActiveBodyDirtyRow>,
    tick: u32,
    stale_rejections: u32,
}

impl ActivePhysicsPool {
    /// Number of currently occupied rows.
    pub fn active_row_count(&self) -> u32 {
        (self.slots.len() - self.free_slots.len()) as u32
    }

    /// Number of free slots available without growing the dense storage.
    pub fn free_slot_count(&self) -> u32 {
        self.free_slots.len() as u32
    }

    /// Total slot count (used + free); reflects the dense `Vec` length.
    pub fn slot_count(&self) -> u32 {
        self.slots.len() as u32
    }

    /// Current pool tick, advanced by [`begin_tick`](Self::begin_tick).
    pub fn current_tick(&self) -> u32 {
        self.tick
    }

    /// Drains the dirty row buffer, returning the rows accumulated since the
    /// last drain. Intended for replication or writeback consumers.
    pub fn drain_dirty_rows(&mut self) -> Vec<ActiveBodyDirtyRow> {
        std::mem::take(&mut self.dirty_rows)
    }

    /// Read a row by handle, rejecting stale or dangling handles.
    pub fn row(&self, handle: ActiveBodyHandle) -> Option<&ActiveBodyRow> {
        let slot = self.slots.get(handle.id.raw() as usize)?;
        if !slot.occupied || slot.generation != handle.generation {
            return None;
        }
        Some(&slot.row)
    }

    /// Look up an existing handle for an entity, if any.
    pub fn handle_for_entity(&self, entity: Entity) -> Option<ActiveBodyHandle> {
        self.handles_by_entity.get(&entity).copied()
    }

    /// Advance the per-tick state. Should be called once per simulation tick
    /// before extraction.
    pub fn begin_tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        self.dirty_rows.clear();
        self.stale_rejections = 0;
    }

    /// Allocates (or reuses) a slot for an entity.
    ///
    /// Honors `max_promotions_per_tick`: if the cap was already met,
    /// allocation is deferred until the next tick (returns
    /// [`AllocationOutcome::Deferred`]).
    fn allocate_or_lookup(
        &mut self,
        entity: Entity,
        body_kind: ActiveBodyKind,
        capacity: u32,
        max_promotions_per_tick: u32,
        promotions_this_tick: &mut u32,
    ) -> AllocationOutcome {
        if let Some(handle) = self.handles_by_entity.get(&entity).copied() {
            return AllocationOutcome::Existing(handle);
        }
        if max_promotions_per_tick != 0 && *promotions_this_tick >= max_promotions_per_tick {
            return AllocationOutcome::Deferred;
        }
        if self.active_row_count() >= capacity {
            return AllocationOutcome::CapacityReached;
        }
        let raw_id = if let Some(id) = self.free_slots.pop() {
            let slot = &mut self.slots[id as usize];
            slot.row = ActiveBodyRow::empty(entity);
            slot.row.body_kind = body_kind;
            slot.occupied = true;
            id
        } else {
            let id = self.slots.len() as u32;
            self.slots.push(PoolSlot {
                row: {
                    let mut row = ActiveBodyRow::empty(entity);
                    row.body_kind = body_kind;
                    row
                },
                generation: ActiveBodyGeneration::ZERO,
                occupied: true,
            });
            id
        };
        let slot = &self.slots[raw_id as usize];
        let handle = ActiveBodyHandle {
            id: ActiveBodyId(raw_id),
            generation: slot.generation,
        };
        self.handles_by_entity.insert(entity, handle);
        *promotions_this_tick = promotions_this_tick.saturating_add(1);
        AllocationOutcome::Allocated(handle)
    }

    /// Removes the row for an entity if one exists. Returns whether anything
    /// was removed. Increments the slot's generation so any cached handle
    /// referring to it is rejected by [`row`](Self::row).
    pub fn remove_entity(&mut self, entity: Entity) -> bool {
        let Some(handle) = self.handles_by_entity.remove(&entity) else {
            return false;
        };
        let slot = match self.slots.get_mut(handle.id.raw() as usize) {
            Some(slot) => slot,
            None => return false,
        };
        if !slot.occupied || slot.generation != handle.generation {
            return false;
        }
        slot.occupied = false;
        slot.row = ActiveBodyRow::empty(Entity::PLACEHOLDER);
        slot.generation = ActiveBodyGeneration(slot.generation.raw().wrapping_add(1));
        self.free_slots.push(handle.id.raw());
        true
    }

    /// Updates a row from extracted ECS state, recording a dirty mask if
    /// anything changed. Returns the dirty mask actually recorded.
    fn upsert_from_ecs(
        &mut self,
        handle: ActiveBodyHandle,
        position: Vec3,
        rotation: Quat,
        linear_velocity: Vec3,
        angular_velocity: Vec3,
    ) -> Option<ActivePoolDirtyMask> {
        let tick = self.tick;
        let Some(slot) = self.slots.get_mut(handle.id.raw() as usize) else {
            self.stale_rejections = self.stale_rejections.saturating_add(1);
            return None;
        };
        if !slot.occupied || slot.generation != handle.generation {
            self.stale_rejections = self.stale_rejections.saturating_add(1);
            return None;
        }
        let mut mask = ActivePoolDirtyMask::NONE;
        if slot.row.position != position {
            slot.row.position = position;
            mask = mask.union(ActivePoolDirtyMask::POSITION);
        }
        if slot.row.rotation != rotation {
            slot.row.rotation = rotation;
            mask = mask.union(ActivePoolDirtyMask::ROTATION);
        }
        if slot.row.linear_velocity != linear_velocity {
            slot.row.linear_velocity = linear_velocity;
            mask = mask.union(ActivePoolDirtyMask::LINEAR_VELOCITY);
        }
        if slot.row.angular_velocity != angular_velocity {
            slot.row.angular_velocity = angular_velocity;
            mask = mask.union(ActivePoolDirtyMask::ANGULAR_VELOCITY);
        }
        if !mask.is_clean() {
            self.dirty_rows.push(ActiveBodyDirtyRow {
                handle,
                dirty_mask: mask,
                tick,
            });
        }
        Some(mask)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AllocationOutcome {
    Allocated(ActiveBodyHandle),
    Existing(ActiveBodyHandle),
    Deferred,
    CapacityReached,
}

impl ActivePhysicsPool {
    /// Allocation helper used by the shard runtime and tests. Wraps the
    /// private allocator and converts the typed outcome into a simple
    /// `Option<ActiveBodyHandle>`. Always uses [`ActiveBodyKind::Dynamic`].
    #[doc(hidden)]
    pub fn __test_allocate(
        &mut self,
        entity: Entity,
        promotions: &mut u32,
    ) -> Option<ActiveBodyHandle> {
        match self.allocate_or_lookup(entity, ActiveBodyKind::Dynamic, 1024, 0, promotions) {
            AllocationOutcome::Allocated(handle) | AllocationOutcome::Existing(handle) => {
                Some(handle)
            }
            AllocationOutcome::Deferred | AllocationOutcome::CapacityReached => None,
        }
    }

    /// Allocation helper that exposes the full set of policy params.
    /// Used by the shard runtime to drive multi-shard pool storage.
    #[doc(hidden)]
    pub fn __test_allocate_with(
        &mut self,
        entity: Entity,
        kind: ActiveBodyKind,
        capacity: u32,
        max_per_tick: u32,
        promotions: &mut u32,
    ) -> Option<ActiveBodyHandle> {
        match self.allocate_or_lookup(entity, kind, capacity, max_per_tick, promotions) {
            AllocationOutcome::Allocated(handle) | AllocationOutcome::Existing(handle) => {
                Some(handle)
            }
            AllocationOutcome::Deferred | AllocationOutcome::CapacityReached => None,
        }
    }

    /// Upsert helper that wraps the private upsert routine.
    #[doc(hidden)]
    pub fn __test_upsert(
        &mut self,
        handle: ActiveBodyHandle,
        position: Vec3,
        rotation: Quat,
        linear_velocity: Vec3,
        angular_velocity: Vec3,
    ) -> Option<ActivePoolDirtyMask> {
        self.upsert_from_ecs(
            handle,
            position,
            rotation,
            linear_velocity,
            angular_velocity,
        )
    }
}

impl ActiveBodyHandle {
    /// Convert this handle into the engine-facing
    /// [`avian3d::prelude::ActivePoolHandle`].
    pub const fn to_avian(self) -> AvianActivePoolHandle {
        AvianActivePoolHandle::new(self.id.raw(), self.generation.raw())
    }

    /// Convert from the engine-facing
    /// [`avian3d::prelude::ActivePoolHandle`].
    pub const fn from_avian(handle: AvianActivePoolHandle) -> Self {
        Self {
            id: ActiveBodyId(handle.id),
            generation: ActiveBodyGeneration(handle.generation),
        }
    }
}

impl PhysicsActiveSet for ActivePhysicsPool {
    fn active_count(&self) -> u32 {
        ActivePhysicsPool::active_row_count(self)
    }

    fn for_each_handle(&self, f: &mut dyn FnMut(AvianActivePoolHandle)) {
        for (slot_index, slot) in self.slots.iter().enumerate() {
            if !slot.occupied {
                continue;
            }
            f(AvianActivePoolHandle::new(
                slot_index as u32,
                slot.generation.raw(),
            ));
        }
    }

    fn entity_for(&self, handle: AvianActivePoolHandle) -> Option<Entity> {
        self.row(ActiveBodyHandle::from_avian(handle))
            .map(|row| row.entity)
    }
}

impl AvianActiveBodySource for ActivePhysicsPool {
    fn position(&self, handle: AvianActivePoolHandle) -> Option<Vec3> {
        self.row(ActiveBodyHandle::from_avian(handle))
            .map(|row| row.position)
    }

    fn rotation(&self, handle: AvianActivePoolHandle) -> Option<Quat> {
        self.row(ActiveBodyHandle::from_avian(handle))
            .map(|row| row.rotation)
    }

    fn linear_velocity(&self, handle: AvianActivePoolHandle) -> Option<Vec3> {
        self.row(ActiveBodyHandle::from_avian(handle))
            .map(|row| row.linear_velocity)
    }

    fn angular_velocity(&self, handle: AvianActivePoolHandle) -> Option<Vec3> {
        self.row(ActiveBodyHandle::from_avian(handle))
            .map(|row| row.angular_velocity)
    }
}

impl AvianActiveContactSource for ActivePhysicsPool {
    fn active_contact_count(&self) -> u32 {
        // Contact rows are not yet stored on the pool; Pass 12 will route
        // narrow-phase contacts through the Avian contact source trait.
        0
    }
}

/// System set the active pool plugin uses for ordering.
#[derive(SystemSet, Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ActivePoolSystems {
    /// Bridges ECS lifecycle changes (added/removed active markers, sleeping
    /// transitions) into pool row allocation/removal.
    Lifecycle,
    /// Reads ECS state into dense pool rows.
    Extract,
    /// Publishes pool writes back to ECS components.
    Writeback,
}

/// Plugin wiring up the active pool resource and its systems.
///
/// Adding this plugin also enrolls the simulation in
/// [`PhysicsDataPlaneMode::DenseActivePool`] via Avian's
/// [`DenseActivePhysicsPlugin`], so downstream Avian-side passes that
/// branch on the data plane policy automatically pick up the dense path.
#[derive(Default)]
pub struct ActivePhysicsPoolPlugin;

impl Plugin for ActivePhysicsPoolPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<DenseActivePhysicsPlugin>() {
            app.add_plugins(DenseActivePhysicsPlugin);
        }

        app.init_resource::<ActivePhysicsPool>()
            .init_resource::<ActivePoolPolicy>()
            .init_resource::<ActivePoolDiagnostics>()
            .register_type::<ActivePhysicsHandle>();

        app.configure_sets(
            PhysicsSchedule,
            (
                ActivePoolSystems::Lifecycle,
                ActivePoolSystems::Extract,
                ActivePoolSystems::Writeback,
            )
                .chain()
                .in_set(PhysicsStepSystems::First),
        );

        app.add_systems(
            PhysicsSchedule,
            (
                lifecycle_remove_pool_rows.in_set(ActivePoolSystems::Lifecycle),
                extract_active_bodies_from_ecs.in_set(ActivePoolSystems::Extract),
                writeback_active_bodies_to_ecs.in_set(ActivePoolSystems::Writeback),
            ),
        );
    }
}

/// Param bundle for the lifecycle bridge so it stays under Bevy's
/// `system_param` width budget.
#[derive(SystemParam)]
pub struct LifecycleRemovals<'w, 's> {
    pub removed_dynamic: RemovedComponents<'w, 's, ActiveDynamicBody>,
    pub removed_kinematic: RemovedComponents<'w, 's, ActiveKinematicBody>,
    pub sleeping: Query<'w, 's, Entity, With<SleepingBody>>,
    pub ghosts: Query<'w, 's, Entity, With<PhysicsGhost>>,
    pub transfer_pending: Query<'w, 's, Entity, With<ShardTransferPending>>,
}

/// Removes pool rows for entities that just left the active set: components
/// dropped (`ActiveDynamicBody`/`ActiveKinematicBody`) or transitioned into
/// `Sleeping`/`PhysicsGhost`/`ShardTransferPending`.
pub fn lifecycle_remove_pool_rows(
    mut pool: ResMut<ActivePhysicsPool>,
    mut commands: Commands,
    mut removals: LifecycleRemovals,
) {
    let LifecycleRemovals {
        removed_dynamic,
        removed_kinematic,
        sleeping,
        ghosts,
        transfer_pending,
    } = &mut removals;

    let drop = |entity: Entity, pool: &mut ActivePhysicsPool, commands: &mut Commands| {
        if pool.remove_entity(entity) {
            commands.entity(entity).try_remove::<ActivePhysicsHandle>();
        }
    };

    for entity in removed_dynamic.read() {
        drop(entity, &mut pool, &mut commands);
    }
    for entity in removed_kinematic.read() {
        drop(entity, &mut pool, &mut commands);
    }
    for entity in sleeping
        .iter()
        .chain(ghosts.iter())
        .chain(transfer_pending.iter())
    {
        drop(entity, &mut pool, &mut commands);
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn extract_active_bodies_from_ecs(
    policy: Res<ActivePoolPolicy>,
    mut pool: ResMut<ActivePhysicsPool>,
    mut diagnostics: ResMut<ActivePoolDiagnostics>,
    mut commands: Commands,
    dynamic: Query<
        (
            Entity,
            &Position,
            &Rotation,
            Option<&LinearVelocity>,
            Option<&AngularVelocity>,
        ),
        (
            With<ActiveDynamicBody>,
            Without<SleepingBody>,
            Without<PhysicsGhost>,
            Without<ShardTransferPending>,
        ),
    >,
    kinematic: Query<
        (
            Entity,
            &Position,
            &Rotation,
            Option<&LinearVelocity>,
            Option<&AngularVelocity>,
        ),
        (
            With<ActiveKinematicBody>,
            Without<SleepingBody>,
            Without<PhysicsGhost>,
            Without<ShardTransferPending>,
        ),
    >,
) {
    pool.begin_tick();
    let mut allocations = 0_u32;
    let mut deferred = 0_u32;
    let mut extraction_count = 0_u32;
    let mut promotions_this_tick = 0_u32;

    let capacity = policy.capacity;
    let max_per_tick = policy.max_promotions_per_tick;

    let process_iter = |entity: Entity,
                        position: &Position,
                        rotation: &Rotation,
                        linear: Option<&LinearVelocity>,
                        angular: Option<&AngularVelocity>,
                        kind: ActiveBodyKind,
                        commands: &mut Commands,
                        pool: &mut ActivePhysicsPool,
                        allocations: &mut u32,
                        deferred: &mut u32,
                        extraction_count: &mut u32,
                        promotions_this_tick: &mut u32| {
        let outcome =
            pool.allocate_or_lookup(entity, kind, capacity, max_per_tick, promotions_this_tick);
        let handle = match outcome {
            AllocationOutcome::Allocated(handle) => {
                *allocations = allocations.saturating_add(1);
                commands.entity(entity).insert(ActivePhysicsHandle(handle));
                handle
            }
            AllocationOutcome::Existing(handle) => handle,
            AllocationOutcome::Deferred | AllocationOutcome::CapacityReached => {
                *deferred = deferred.saturating_add(1);
                return;
            }
        };
        let lin = linear.map(|v| v.0).unwrap_or(Vec3::ZERO);
        let ang = angular.map(|v| v.0).unwrap_or(Vec3::ZERO);
        if pool
            .upsert_from_ecs(handle, position.0, rotation.0, lin, ang)
            .is_some()
        {
            *extraction_count = extraction_count.saturating_add(1);
        }
    };

    let dynamic_iter: Vec<_> = if policy.deterministic_order {
        let mut entries: Vec<_> = dynamic.iter().collect();
        entries.sort_by_key(|(entity, _, _, _, _)| *entity);
        entries
    } else {
        dynamic.iter().collect()
    };
    for (entity, position, rotation, linear, angular) in dynamic_iter {
        process_iter(
            entity,
            position,
            rotation,
            linear,
            angular,
            ActiveBodyKind::Dynamic,
            &mut commands,
            &mut pool,
            &mut allocations,
            &mut deferred,
            &mut extraction_count,
            &mut promotions_this_tick,
        );
    }

    let kinematic_iter: Vec<_> = if policy.deterministic_order {
        let mut entries: Vec<_> = kinematic.iter().collect();
        entries.sort_by_key(|(entity, _, _, _, _)| *entity);
        entries
    } else {
        kinematic.iter().collect()
    };
    for (entity, position, rotation, linear, angular) in kinematic_iter {
        process_iter(
            entity,
            position,
            rotation,
            linear,
            angular,
            ActiveBodyKind::Kinematic,
            &mut commands,
            &mut pool,
            &mut allocations,
            &mut deferred,
            &mut extraction_count,
            &mut promotions_this_tick,
        );
    }

    diagnostics.capacity = capacity;
    diagnostics.active_rows = pool.active_row_count();
    diagnostics.free_slots = pool.free_slot_count();
    diagnostics.allocations_this_tick = allocations;
    diagnostics.promotions_deferred = deferred;
    diagnostics.extraction_count = extraction_count;
    diagnostics.stale_handle_rejections = pool.stale_rejections;
}

/// Publishes dirty pool rows back to ECS Position/Rotation/velocity
/// components.
#[allow(clippy::type_complexity)]
pub fn writeback_active_bodies_to_ecs(
    mut pool: ResMut<ActivePhysicsPool>,
    policy: Res<ActivePoolPolicy>,
    mut diagnostics: ResMut<ActivePoolDiagnostics>,
    mut writers: ParamSet<(
        Query<&mut Position>,
        Query<&mut Rotation>,
        Query<&mut LinearVelocity>,
        Query<&mut AngularVelocity>,
    )>,
) {
    let dirty = pool.drain_dirty_rows();
    let mut writeback = 0_u32;
    let mode = policy.writeback_mode;
    let counter = AtomicU32::new(0);

    for record in &dirty {
        let Some(row) = pool.row(record.handle) else {
            continue;
        };
        let entity = row.entity;
        let mask = match mode {
            ActivePoolWritebackMode::DirtyOnly => record.dirty_mask,
            ActivePoolWritebackMode::Unconditional => ActivePoolDirtyMask::ALL,
        };

        if mask.contains(ActivePoolDirtyMask::POSITION)
            && let Ok(mut position) = writers.p0().get_mut(entity)
        {
            position.0 = row.position;
            counter.fetch_add(1, Ordering::Relaxed);
        }
        if mask.contains(ActivePoolDirtyMask::ROTATION)
            && let Ok(mut rotation) = writers.p1().get_mut(entity)
        {
            rotation.0 = row.rotation;
            counter.fetch_add(1, Ordering::Relaxed);
        }
        if mask.contains(ActivePoolDirtyMask::LINEAR_VELOCITY)
            && let Ok(mut velocity) = writers.p2().get_mut(entity)
        {
            velocity.0 = row.linear_velocity;
            counter.fetch_add(1, Ordering::Relaxed);
        }
        if mask.contains(ActivePoolDirtyMask::ANGULAR_VELOCITY)
            && let Ok(mut velocity) = writers.p3().get_mut(entity)
        {
            velocity.0 = row.angular_velocity;
            counter.fetch_add(1, Ordering::Relaxed);
        }
        writeback = writeback.saturating_add(1);
    }
    let _ = counter.load(Ordering::Relaxed);
    diagnostics.writeback_count = writeback;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).expect("entity index should be in range")
    }

    #[test]
    fn allocate_and_lookup_returns_consistent_handle() {
        let mut pool = ActivePhysicsPool::default();
        pool.begin_tick();

        let entity = make_entity(7);
        let mut promotions = 0;
        let outcome =
            pool.allocate_or_lookup(entity, ActiveBodyKind::Dynamic, 16, 0, &mut promotions);
        let handle = match outcome {
            AllocationOutcome::Allocated(h) => h,
            other => panic!("expected new allocation, got {other:?}"),
        };
        assert_eq!(pool.handle_for_entity(entity), Some(handle));
        assert_eq!(pool.active_row_count(), 1);
        assert!(pool.row(handle).is_some());

        // Re-allocating the same entity returns the existing handle.
        let outcome =
            pool.allocate_or_lookup(entity, ActiveBodyKind::Dynamic, 16, 0, &mut promotions);
        assert_eq!(outcome, AllocationOutcome::Existing(handle));
        assert_eq!(pool.active_row_count(), 1);
    }

    #[test]
    fn stale_handle_is_rejected_after_remove() {
        let mut pool = ActivePhysicsPool::default();
        pool.begin_tick();

        let entity = make_entity(11);
        let mut promotions = 0;
        let handle = match pool.allocate_or_lookup(
            entity,
            ActiveBodyKind::Dynamic,
            16,
            0,
            &mut promotions,
        ) {
            AllocationOutcome::Allocated(h) => h,
            other => panic!("expected allocation, got {other:?}"),
        };
        let stale = handle;

        assert!(pool.remove_entity(entity));
        assert_eq!(pool.active_row_count(), 0);
        assert!(pool.row(stale).is_none(), "stale handle must be rejected");
        assert_eq!(pool.handle_for_entity(entity), None);
    }

    #[test]
    fn slot_reuse_increments_generation() {
        let mut pool = ActivePhysicsPool::default();
        pool.begin_tick();

        let mut promotions = 0;
        let entity_a = make_entity(21);
        let handle_a = match pool.allocate_or_lookup(
            entity_a,
            ActiveBodyKind::Dynamic,
            16,
            0,
            &mut promotions,
        ) {
            AllocationOutcome::Allocated(h) => h,
            other => panic!("{other:?}"),
        };
        assert_eq!(handle_a.generation, ActiveBodyGeneration(0));

        assert!(pool.remove_entity(entity_a));

        let entity_b = make_entity(22);
        let handle_b = match pool.allocate_or_lookup(
            entity_b,
            ActiveBodyKind::Dynamic,
            16,
            0,
            &mut promotions,
        ) {
            AllocationOutcome::Allocated(h) => h,
            other => panic!("{other:?}"),
        };
        // Same slot id, but generation moved forward.
        assert_eq!(handle_a.id, handle_b.id);
        assert_ne!(handle_a.generation, handle_b.generation);
        assert_eq!(handle_b.generation, ActiveBodyGeneration(1));

        // The original handle must not resolve to the new occupant.
        assert!(
            pool.row(handle_a).is_none(),
            "stale handle must not resolve after slot reuse"
        );
        assert!(pool.row(handle_b).is_some());
    }

    #[test]
    fn upsert_records_dirty_mask_and_skips_clean_rows() {
        let mut pool = ActivePhysicsPool::default();
        pool.begin_tick();

        let entity = make_entity(34);
        let mut promotions = 0;
        let handle = match pool.allocate_or_lookup(
            entity,
            ActiveBodyKind::Dynamic,
            16,
            0,
            &mut promotions,
        ) {
            AllocationOutcome::Allocated(h) => h,
            other => panic!("{other:?}"),
        };

        // First upsert from non-default state should mark dirty.
        let mask = pool
            .upsert_from_ecs(handle, Vec3::X, Quat::IDENTITY, Vec3::Y, Vec3::ZERO)
            .expect("upsert should succeed for a valid handle");
        assert!(!mask.is_clean());
        assert!(mask.contains(ActivePoolDirtyMask::POSITION));
        assert!(mask.contains(ActivePoolDirtyMask::LINEAR_VELOCITY));

        // Re-upsert with identical state should return a clean mask and
        // leave dirty_rows untouched.
        let dirty_before = pool.dirty_rows.len();
        let mask = pool
            .upsert_from_ecs(handle, Vec3::X, Quat::IDENTITY, Vec3::Y, Vec3::ZERO)
            .expect("upsert should succeed for a valid handle");
        assert!(mask.is_clean());
        assert_eq!(pool.dirty_rows.len(), dirty_before);
    }

    #[test]
    fn upsert_with_stale_handle_increments_rejections() {
        let mut pool = ActivePhysicsPool::default();
        pool.begin_tick();

        let entity = make_entity(101);
        let mut promotions = 0;
        let handle = match pool.allocate_or_lookup(
            entity,
            ActiveBodyKind::Dynamic,
            16,
            0,
            &mut promotions,
        ) {
            AllocationOutcome::Allocated(h) => h,
            other => panic!("{other:?}"),
        };
        assert!(pool.remove_entity(entity));

        // Tick advances and a fresh budget begins.
        pool.begin_tick();
        let result =
            pool.upsert_from_ecs(handle, Vec3::ONE, Quat::IDENTITY, Vec3::ZERO, Vec3::ZERO);
        assert!(result.is_none());
        assert_eq!(pool.stale_rejections, 1);
    }

    #[test]
    fn capacity_exhaustion_returns_capacity_reached() {
        let mut pool = ActivePhysicsPool::default();
        pool.begin_tick();
        let mut promotions = 0;

        // Tiny capacity so we exhaust the pool.
        let mut handles = Vec::new();
        for index in 0..2 {
            let entity = make_entity(index + 1);
            let handle = match pool.allocate_or_lookup(
                entity,
                ActiveBodyKind::Dynamic,
                2,
                0,
                &mut promotions,
            ) {
                AllocationOutcome::Allocated(h) => h,
                other => panic!("{other:?}"),
            };
            handles.push(handle);
        }
        let extra_entity = make_entity(99);
        let outcome =
            pool.allocate_or_lookup(extra_entity, ActiveBodyKind::Dynamic, 2, 0, &mut promotions);
        assert_eq!(outcome, AllocationOutcome::CapacityReached);
        assert_eq!(pool.active_row_count(), 2);
    }

    #[test]
    fn max_promotions_per_tick_defers_excess_allocations() {
        let mut pool = ActivePhysicsPool::default();
        pool.begin_tick();
        let mut promotions = 0;

        let first = pool.allocate_or_lookup(
            make_entity(40),
            ActiveBodyKind::Dynamic,
            16,
            1,
            &mut promotions,
        );
        let second = pool.allocate_or_lookup(
            make_entity(41),
            ActiveBodyKind::Dynamic,
            16,
            1,
            &mut promotions,
        );
        match first {
            AllocationOutcome::Allocated(_) => {}
            other => panic!("first allocation should succeed: {other:?}"),
        }
        assert_eq!(second, AllocationOutcome::Deferred);
        assert_eq!(pool.active_row_count(), 1);
    }

    #[test]
    fn dirty_mask_helpers_are_consistent() {
        let mut mask = ActivePoolDirtyMask::NONE;
        assert!(mask.is_clean());
        mask = mask.union(ActivePoolDirtyMask::POSITION);
        assert!(!mask.is_clean());
        assert!(mask.contains(ActivePoolDirtyMask::POSITION));
        assert!(!mask.contains(ActivePoolDirtyMask::LINEAR_VELOCITY));
        let cleared = mask.difference(ActivePoolDirtyMask::POSITION);
        assert!(cleared.is_clean());

        let transform_only = ActivePoolDirtyMask::TRANSFORM;
        assert!(transform_only.contains(ActivePoolDirtyMask::POSITION));
        assert!(transform_only.contains(ActivePoolDirtyMask::ROTATION));
        assert!(!transform_only.contains(ActivePoolDirtyMask::LINEAR_VELOCITY));
    }

    #[test]
    fn small_active_scene_runs_through_extraction_and_writeback() {
        use avian3d::prelude::{
            ActiveDynamicBody, ActiveKinematicBody, AngularVelocity, LinearVelocity, Position,
            Rotation,
        };

        // Build a minimal app that wires the active pool but drives the
        // `PhysicsSchedule` directly each tick. Avoids the FixedUpdate timing
        // dependency in `PhysicsSchedulePlugin` so the test stays
        // deterministic without a custom `TimeUpdateStrategy`.
        let mut app = App::new();
        app.init_schedule(PhysicsSchedule)
            .add_plugins((MinimalPlugins, ActivePhysicsPoolPlugin))
            .configure_sets(
                PhysicsSchedule,
                (PhysicsStepSystems::First, PhysicsStepSystems::Last).chain(),
            )
            .add_systems(Update, |world: &mut World| {
                world.run_schedule(PhysicsSchedule);
            });

        let dynamic_entity = app
            .world_mut()
            .spawn((
                ActiveDynamicBody,
                Position(Vec3::new(1.0, 2.0, 3.0)),
                Rotation::default(),
                LinearVelocity(Vec3::new(0.5, 0.0, 0.0)),
                AngularVelocity::default(),
            ))
            .id();
        let kinematic_entity = app
            .world_mut()
            .spawn((
                ActiveKinematicBody,
                Position(Vec3::new(-2.0, 0.0, 0.0)),
                Rotation::default(),
                LinearVelocity::default(),
                AngularVelocity(Vec3::Y),
            ))
            .id();

        app.finish();
        // First update runs extraction and assigns handles.
        app.update();

        let pool = app.world().resource::<ActivePhysicsPool>();
        let dynamic_handle = pool
            .handle_for_entity(dynamic_entity)
            .expect("dynamic body should be tracked");
        let kinematic_handle = pool
            .handle_for_entity(kinematic_entity)
            .expect("kinematic body should be tracked");
        assert_eq!(pool.active_row_count(), 2);
        assert_eq!(
            pool.row(dynamic_handle).map(|row| row.body_kind),
            Some(ActiveBodyKind::Dynamic)
        );
        assert_eq!(
            pool.row(kinematic_handle).map(|row| row.body_kind),
            Some(ActiveBodyKind::Kinematic)
        );

        let diagnostics = app.world().resource::<ActivePoolDiagnostics>();
        assert_eq!(diagnostics.active_rows, 2);
        assert_eq!(diagnostics.allocations_this_tick, 2);
        assert!(diagnostics.extraction_count >= 1);

        // Mutate ECS state between ticks; the next extraction should pick it
        // up without allocating new rows.
        app.world_mut()
            .entity_mut(dynamic_entity)
            .insert(Position(Vec3::new(4.0, 5.0, 6.0)));
        app.update();

        let pool = app.world().resource::<ActivePhysicsPool>();
        let row = pool
            .row(dynamic_handle)
            .expect("dynamic body should still be tracked");
        assert_eq!(row.position, Vec3::new(4.0, 5.0, 6.0));
        let diagnostics = app.world().resource::<ActivePoolDiagnostics>();
        assert_eq!(diagnostics.active_rows, 2);
        assert_eq!(diagnostics.allocations_this_tick, 0);

        // Drop the kinematic body; lifecycle should remove its pool row.
        app.world_mut()
            .entity_mut(kinematic_entity)
            .remove::<ActiveKinematicBody>();
        app.update();

        let pool = app.world().resource::<ActivePhysicsPool>();
        assert!(
            pool.handle_for_entity(kinematic_entity).is_none(),
            "kinematic body should be removed from the pool after losing its marker"
        );
        assert_eq!(pool.active_row_count(), 1);
    }

    #[test]
    fn pool_implements_avian_active_set_iteration() {
        let mut pool = ActivePhysicsPool::default();
        pool.begin_tick();
        let mut promotions = 0;
        let entity_a = make_entity(91);
        let entity_b = make_entity(92);

        let handle_a = match pool.allocate_or_lookup(
            entity_a,
            ActiveBodyKind::Dynamic,
            16,
            0,
            &mut promotions,
        ) {
            AllocationOutcome::Allocated(h) => h,
            other => panic!("{other:?}"),
        };
        let handle_b = match pool.allocate_or_lookup(
            entity_b,
            ActiveBodyKind::Kinematic,
            16,
            0,
            &mut promotions,
        ) {
            AllocationOutcome::Allocated(h) => h,
            other => panic!("{other:?}"),
        };

        let active_set: &dyn PhysicsActiveSet = &pool;
        assert_eq!(active_set.active_count(), 2);

        let mut collected: Vec<AvianActivePoolHandle> = Vec::new();
        active_set.for_each_handle(&mut |handle| collected.push(handle));
        assert_eq!(collected.len(), 2);
        assert!(collected.contains(&handle_a.to_avian()));
        assert!(collected.contains(&handle_b.to_avian()));

        // entity_for round-trip resolves to the original entities.
        assert_eq!(active_set.entity_for(handle_a.to_avian()), Some(entity_a));
        assert_eq!(active_set.entity_for(handle_b.to_avian()), Some(entity_b));

        // Stale handle (different generation) is rejected.
        let stale = AvianActivePoolHandle::new(handle_a.id.raw(), handle_a.generation.raw() + 1);
        assert_eq!(active_set.entity_for(stale), None);
    }

    #[test]
    fn pool_implements_avian_body_source_state_reads() {
        let mut pool = ActivePhysicsPool::default();
        pool.begin_tick();
        let mut promotions = 0;
        let entity = make_entity(60);
        let handle = match pool.allocate_or_lookup(
            entity,
            ActiveBodyKind::Dynamic,
            16,
            0,
            &mut promotions,
        ) {
            AllocationOutcome::Allocated(h) => h,
            other => panic!("{other:?}"),
        };
        pool.upsert_from_ecs(handle, Vec3::X, Quat::IDENTITY, Vec3::Y, Vec3::Z)
            .expect("upsert should succeed");

        let body_source: &dyn AvianActiveBodySource = &pool;
        let avian_handle = handle.to_avian();
        assert_eq!(body_source.position(avian_handle), Some(Vec3::X));
        assert_eq!(body_source.rotation(avian_handle), Some(Quat::IDENTITY));
        assert_eq!(body_source.linear_velocity(avian_handle), Some(Vec3::Y));
        assert_eq!(body_source.angular_velocity(avian_handle), Some(Vec3::Z));
    }

    #[test]
    fn pool_plugin_enrolls_dense_data_plane_mode() {
        use avian3d::prelude::{PhysicsDataPlaneMode, PhysicsDataPlanePolicy};

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, ActivePhysicsPoolPlugin));
        let policy = app
            .world()
            .get_resource::<PhysicsDataPlanePolicy>()
            .expect("dense plugin should register data plane policy");
        assert_eq!(policy.mode, PhysicsDataPlaneMode::DenseActivePool);
    }
}
