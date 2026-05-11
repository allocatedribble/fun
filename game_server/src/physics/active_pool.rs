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
use hashbrown::HashMap;

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

/// Hot-storage layout for the active pool.
///
/// V2-P2 promotes the prototype's row-shaped [`ActiveBodyRow`] storage
/// into a SoA-lane layout where each per-row field lives in its own
/// dense `Vec<T>`. Both layouts are functionally equivalent — the
/// authoritative state, generational handle semantics, dirty-mask
/// tracking, and stale-handle rejection all behave identically. The
/// new layout exists so SIMD / GPU lane scans can sweep contiguous
/// arrays without per-row branching.
///
/// Default remains [`Self::RowPrototype`] until benchmark evidence
/// from `fun-bench physics-scale --layout soa-lanes` justifies the
/// production switch. Per V2-P2 doctrine, the row-prototype path is
/// the rollback target if SoA fails to win.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub enum ActivePoolLayout {
    /// Original AoS shape (`Vec<PoolSlot>` of `ActiveBodyRow`).
    /// Production default until SoA wins on benchmark.
    #[default]
    RowPrototype,
    /// SoA lane shape (one `Vec<T>` per per-row field). Opt-in via
    /// `ActivePoolPolicy::layout`.
    SoaLanes,
}

impl ActivePoolLayout {
    /// Stable diagnostic name for telemetry / config bundles.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RowPrototype => "row_prototype",
            Self::SoaLanes => "soa_lanes",
        }
    }
}

/// Placeholder for the eventual collider-proxy handle. Lives as a
/// SoA lane today so the `body_lane_view` shape is fully populated;
/// real collider handles wire through when the broad-phase domain
/// crate lands.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct ColliderProxyHandlePlaceholder(pub u64);

impl ColliderProxyHandlePlaceholder {
    /// Sentinel for "no collider proxy attached."
    pub const NULL: Self = Self(0);
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
    /// Hot-storage layout selector. V2-P2 default remains
    /// [`ActivePoolLayout::RowPrototype`] (current production
    /// behavior); set to [`ActivePoolLayout::SoaLanes`] to exercise
    /// the V2-P2 SoA path.
    pub layout: ActivePoolLayout,
}

impl Default for ActivePoolPolicy {
    fn default() -> Self {
        Self {
            capacity: 16_384,
            deterministic_order: true,
            writeback_mode: ActivePoolWritebackMode::DirtyOnly,
            max_promotions_per_tick: 0,
            layout: ActivePoolLayout::RowPrototype,
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

/// V2-P2 SoA storage. One `Vec<T>` per per-row field; every column has
/// the same length. Indices match the slot id used in
/// [`ActiveBodyHandle`]. `occupied[i]` reports whether index `i` is
/// live; freed slots are tracked in `free_slots` so reuse is LIFO with
/// generation bump.
///
/// The lane shape mirrors the spec list:
/// `entities`, `generations`, `positions`, `rotations`,
/// `linear_velocities`, `angular_velocities`, `body_kinds`,
/// `dirty_masks`, `last_changed_ticks`, `collider_proxy_handles`. Plus
/// `occupied` so reads can short-circuit freed slots without a side
/// table.
#[derive(Debug, Default)]
struct SoaStorage {
    entities: Vec<Entity>,
    generations: Vec<ActiveBodyGeneration>,
    occupied: Vec<bool>,
    positions: Vec<Vec3>,
    rotations: Vec<Quat>,
    linear_velocities: Vec<Vec3>,
    angular_velocities: Vec<Vec3>,
    body_kinds: Vec<ActiveBodyKind>,
    dirty_masks: Vec<ActivePoolDirtyMask>,
    last_changed_ticks: Vec<u32>,
    collider_proxy_handles: Vec<ColliderProxyHandlePlaceholder>,
}

impl SoaStorage {
    /// Number of slot entries (live + freed). Mirrors `slots.len()`.
    fn slot_count(&self) -> usize {
        self.entities.len()
    }

    /// True if slot `i` is currently occupied.
    #[inline]
    fn is_occupied(&self, i: usize) -> bool {
        self.occupied.get(i).copied().unwrap_or(false)
    }

    /// Allocate a new slot at the end of every column. Returns the
    /// new slot index.
    fn push_new_slot(&mut self, entity: Entity, body_kind: ActiveBodyKind) -> u32 {
        let id = self.entities.len() as u32;
        self.entities.push(entity);
        self.generations.push(ActiveBodyGeneration::ZERO);
        self.occupied.push(true);
        self.positions.push(Vec3::ZERO);
        self.rotations.push(Quat::IDENTITY);
        self.linear_velocities.push(Vec3::ZERO);
        self.angular_velocities.push(Vec3::ZERO);
        self.body_kinds.push(body_kind);
        self.dirty_masks.push(ActivePoolDirtyMask::NONE);
        self.last_changed_ticks.push(0);
        self.collider_proxy_handles
            .push(ColliderProxyHandlePlaceholder::NULL);
        id
    }

    /// Reuse a previously freed slot. Caller is responsible for
    /// passing a slot index that's actually free.
    fn reuse_slot(&mut self, id: u32, entity: Entity, body_kind: ActiveBodyKind) {
        let i = id as usize;
        self.entities[i] = entity;
        self.occupied[i] = true;
        self.positions[i] = Vec3::ZERO;
        self.rotations[i] = Quat::IDENTITY;
        self.linear_velocities[i] = Vec3::ZERO;
        self.angular_velocities[i] = Vec3::ZERO;
        self.body_kinds[i] = body_kind;
        self.dirty_masks[i] = ActivePoolDirtyMask::NONE;
        self.last_changed_ticks[i] = 0;
        // collider proxy handles intentionally not reset — the caller
        // assigns these via a dedicated update method when wiring
        // collider proxies lands.
    }

    /// Free slot `i`; bump generation; clear lane fields to defaults.
    fn free_slot(&mut self, id: u32) {
        let i = id as usize;
        self.occupied[i] = false;
        self.entities[i] = Entity::PLACEHOLDER;
        self.positions[i] = Vec3::ZERO;
        self.rotations[i] = Quat::IDENTITY;
        self.linear_velocities[i] = Vec3::ZERO;
        self.angular_velocities[i] = Vec3::ZERO;
        self.dirty_masks[i] = ActivePoolDirtyMask::NONE;
        self.last_changed_ticks[i] = 0;
        let g = self.generations[i].raw().wrapping_add(1);
        self.generations[i] = ActiveBodyGeneration(g);
    }

    /// Build an [`ActiveBodyRow`] view from the SoA columns at index
    /// `i`. Used by trait impls + debug accessors that still need
    /// the row shape.
    fn row_at(&self, i: usize) -> ActiveBodyRow {
        ActiveBodyRow {
            entity: self.entities[i],
            position: self.positions[i],
            rotation: self.rotations[i],
            linear_velocity: self.linear_velocities[i],
            angular_velocity: self.angular_velocities[i],
            body_kind: self.body_kinds[i],
        }
    }
}

/// Read-only borrow over every SoA lane.
///
/// Returned by [`ActivePhysicsPool::body_lane_view`]. All slices have
/// the same length — the pool's slot count (live + freed). Use
/// `occupied[i]` to short-circuit freed slots.
#[derive(Copy, Clone, Debug)]
pub struct ActiveBodyLaneView<'a> {
    /// Per-slot ECS entity (`Entity::PLACEHOLDER` when freed).
    pub entities: &'a [Entity],
    /// Per-slot generation.
    pub generations: &'a [ActiveBodyGeneration],
    /// Per-slot occupied flag.
    pub occupied: &'a [bool],
    /// World-space positions.
    pub positions: &'a [Vec3],
    /// World-space rotations.
    pub rotations: &'a [Quat],
    /// Linear velocities.
    pub linear_velocities: &'a [Vec3],
    /// Angular velocities.
    pub angular_velocities: &'a [Vec3],
    /// Body kinds.
    pub body_kinds: &'a [ActiveBodyKind],
    /// Per-slot cumulative dirty mask since the last drain.
    pub dirty_masks: &'a [ActivePoolDirtyMask],
    /// Per-slot last-changed tick.
    pub last_changed_ticks: &'a [u32],
    /// Collider proxy handles (placeholder until broad-phase wires up).
    pub collider_proxy_handles: &'a [ColliderProxyHandlePlaceholder],
}

/// Mutable borrow over every SoA lane.
///
/// Returned by [`ActivePhysicsPool::body_lane_view_mut`]. Same shape
/// as [`ActiveBodyLaneView`] but with `&mut [T]` slices. The borrow
/// split exposes every lane as `&mut` simultaneously (Rust's borrow
/// checker allows this for disjoint struct fields).
#[derive(Debug)]
pub struct ActiveBodyLaneViewMut<'a> {
    /// Read-only entity column (the lane's identity).
    pub entities: &'a [Entity],
    /// Read-only generation column.
    pub generations: &'a [ActiveBodyGeneration],
    /// Read-only occupied flag.
    pub occupied: &'a [bool],
    /// Mutable positions.
    pub positions: &'a mut [Vec3],
    /// Mutable rotations.
    pub rotations: &'a mut [Quat],
    /// Mutable linear velocities.
    pub linear_velocities: &'a mut [Vec3],
    /// Mutable angular velocities.
    pub angular_velocities: &'a mut [Vec3],
    /// Mutable body kinds.
    pub body_kinds: &'a mut [ActiveBodyKind],
    /// Mutable dirty masks.
    pub dirty_masks: &'a mut [ActivePoolDirtyMask],
    /// Mutable last-changed ticks.
    pub last_changed_ticks: &'a mut [u32],
    /// Mutable collider-proxy handles.
    pub collider_proxy_handles: &'a mut [ColliderProxyHandlePlaceholder],
}

/// V2-P2 memory-accounting snapshot. Reports the dense storage's
/// resource footprint per the spec metric family.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ActivePoolMemoryAccounting {
    /// Layout the snapshot was taken under.
    pub layout: ActivePoolLayout,
    /// Total allocated bytes across the dense storage (capacity).
    pub capacity_bytes: u64,
    /// Bytes occupied by live rows (`active_row_count * per_row_bytes`).
    pub live_bytes: u64,
    /// Fragmentation as parts-per-million:
    /// `(capacity_bytes - live_bytes) * 1_000_000 / capacity_bytes`.
    /// 0 when capacity is 0.
    pub fragmentation_ppm: u32,
    /// Free slot count (slot id reuse list size).
    pub free_slot_count: u32,
    /// Per-row dirty rows held since the last drain.
    pub dirty_row_count: u32,
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
    /// Hot-storage layout. V2-P2 SoA path is opt-in via
    /// [`ActivePoolPolicy::layout`] (see [`Self::set_layout`]).
    /// Default = [`ActivePoolLayout::RowPrototype`].
    layout: ActivePoolLayout,
    /// AoS slot storage (used when `layout == RowPrototype`).
    /// Empty when SoA is active.
    slots: Vec<PoolSlot>,
    /// V2-P2 SoA storage (used when `layout == SoaLanes`).
    /// Empty when row prototype is active.
    soa: SoaStorage,
    /// Free-slot LIFO list. Layout-agnostic.
    free_slots: Vec<u32>,
    /// Entity → handle resolution table. Layout-agnostic.
    handles_by_entity: HashMap<Entity, ActiveBodyHandle>,
    /// Per-tick dirty-row queue. Drained by writeback consumers.
    dirty_rows: Vec<ActiveBodyDirtyRow>,
    tick: u32,
    stale_rejections: u32,
}

impl ActivePhysicsPool {
    /// Active layout. V2-P2: defaults to [`ActivePoolLayout::RowPrototype`].
    #[inline]
    pub fn layout(&self) -> ActivePoolLayout {
        self.layout
    }

    /// Set the hot-storage layout. Only valid on an empty pool;
    /// switching layouts on a populated pool is a future migration
    /// pass.
    pub fn set_layout(&mut self, layout: ActivePoolLayout) {
        debug_assert!(
            self.slots.is_empty()
                && self.soa.slot_count() == 0
                && self.handles_by_entity.is_empty(),
            "ActivePhysicsPool::set_layout called on a populated pool; \
             layout changes require an explicit migration pass."
        );
        self.layout = layout;
    }

    /// Number of currently occupied rows.
    pub fn active_row_count(&self) -> u32 {
        (self.slot_count() as usize - self.free_slots.len()) as u32
    }

    /// Number of free slots available without growing the dense storage.
    pub fn free_slot_count(&self) -> u32 {
        self.free_slots.len() as u32
    }

    /// Total slot count (used + free); reflects the dense storage length.
    /// Layout-agnostic.
    pub fn slot_count(&self) -> u32 {
        match self.layout {
            ActivePoolLayout::RowPrototype => self.slots.len() as u32,
            ActivePoolLayout::SoaLanes => self.soa.slot_count() as u32,
        }
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
    ///
    /// Returns a borrow on the row prototype path (zero-copy). On the
    /// SoA path the row is materialised on demand from the SoA
    /// columns, so `row_owned()` is the SoA-path equivalent and this
    /// method returns `None` to keep the lifetime contract honest —
    /// callers wanting the row in either layout should use
    /// [`Self::row_owned`].
    pub fn row(&self, handle: ActiveBodyHandle) -> Option<&ActiveBodyRow> {
        match self.layout {
            ActivePoolLayout::RowPrototype => {
                let slot = self.slots.get(handle.id.raw() as usize)?;
                if !slot.occupied || slot.generation != handle.generation {
                    return None;
                }
                Some(&slot.row)
            }
            ActivePoolLayout::SoaLanes => None,
        }
    }

    /// Read a row by handle in either layout. On the row-prototype
    /// path this clones the stored row; on the SoA path it
    /// materialises a row from the SoA columns. Use this from
    /// layout-agnostic code (e.g. trait impls).
    pub fn row_owned(&self, handle: ActiveBodyHandle) -> Option<ActiveBodyRow> {
        match self.layout {
            ActivePoolLayout::RowPrototype => self.row(handle).copied(),
            ActivePoolLayout::SoaLanes => {
                let i = handle.id.raw() as usize;
                if !self.soa.is_occupied(i) {
                    return None;
                }
                if self.soa.generations.get(i).copied() != Some(handle.generation) {
                    return None;
                }
                Some(self.soa.row_at(i))
            }
        }
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
        let (raw_id, generation) = match self.layout {
            ActivePoolLayout::RowPrototype => {
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
                (raw_id, self.slots[raw_id as usize].generation)
            }
            ActivePoolLayout::SoaLanes => {
                let raw_id = if let Some(id) = self.free_slots.pop() {
                    self.soa.reuse_slot(id, entity, body_kind);
                    id
                } else {
                    self.soa.push_new_slot(entity, body_kind)
                };
                (raw_id, self.soa.generations[raw_id as usize])
            }
        };
        let handle = ActiveBodyHandle {
            id: ActiveBodyId(raw_id),
            generation,
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
        let raw_id = handle.id.raw();
        let removed = match self.layout {
            ActivePoolLayout::RowPrototype => {
                let slot = match self.slots.get_mut(raw_id as usize) {
                    Some(slot) => slot,
                    None => return false,
                };
                if !slot.occupied || slot.generation != handle.generation {
                    return false;
                }
                slot.occupied = false;
                slot.row = ActiveBodyRow::empty(Entity::PLACEHOLDER);
                slot.generation = ActiveBodyGeneration(slot.generation.raw().wrapping_add(1));
                true
            }
            ActivePoolLayout::SoaLanes => {
                let i = raw_id as usize;
                if !self.soa.is_occupied(i) {
                    return false;
                }
                if self.soa.generations.get(i).copied() != Some(handle.generation) {
                    return false;
                }
                self.soa.free_slot(raw_id);
                true
            }
        };
        if removed {
            self.free_slots.push(raw_id);
        }
        removed
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
        let i = handle.id.raw() as usize;
        let mask = match self.layout {
            ActivePoolLayout::RowPrototype => {
                let Some(slot) = self.slots.get_mut(i) else {
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
                mask
            }
            ActivePoolLayout::SoaLanes => {
                if !self.soa.is_occupied(i) {
                    self.stale_rejections = self.stale_rejections.saturating_add(1);
                    return None;
                }
                if self.soa.generations.get(i).copied() != Some(handle.generation) {
                    self.stale_rejections = self.stale_rejections.saturating_add(1);
                    return None;
                }
                let mut mask = ActivePoolDirtyMask::NONE;
                if self.soa.positions[i] != position {
                    self.soa.positions[i] = position;
                    mask = mask.union(ActivePoolDirtyMask::POSITION);
                }
                if self.soa.rotations[i] != rotation {
                    self.soa.rotations[i] = rotation;
                    mask = mask.union(ActivePoolDirtyMask::ROTATION);
                }
                if self.soa.linear_velocities[i] != linear_velocity {
                    self.soa.linear_velocities[i] = linear_velocity;
                    mask = mask.union(ActivePoolDirtyMask::LINEAR_VELOCITY);
                }
                if self.soa.angular_velocities[i] != angular_velocity {
                    self.soa.angular_velocities[i] = angular_velocity;
                    mask = mask.union(ActivePoolDirtyMask::ANGULAR_VELOCITY);
                }
                if !mask.is_clean() {
                    self.soa.dirty_masks[i] = self.soa.dirty_masks[i].union(mask);
                    self.soa.last_changed_ticks[i] = tick;
                }
                mask
            }
        };
        if !mask.is_clean() {
            self.dirty_rows.push(ActiveBodyDirtyRow {
                handle,
                dirty_mask: mask,
                tick,
            });
        }
        Some(mask)
    }

    /// Borrow the SoA lanes (read-only). Returns `None` when the
    /// active layout is [`ActivePoolLayout::RowPrototype`] — the row
    /// prototype path doesn't materialise SoA lanes.
    pub fn body_lane_view(&self) -> Option<ActiveBodyLaneView<'_>> {
        if !matches!(self.layout, ActivePoolLayout::SoaLanes) {
            return None;
        }
        Some(ActiveBodyLaneView {
            entities: &self.soa.entities,
            generations: &self.soa.generations,
            occupied: &self.soa.occupied,
            positions: &self.soa.positions,
            rotations: &self.soa.rotations,
            linear_velocities: &self.soa.linear_velocities,
            angular_velocities: &self.soa.angular_velocities,
            body_kinds: &self.soa.body_kinds,
            dirty_masks: &self.soa.dirty_masks,
            last_changed_ticks: &self.soa.last_changed_ticks,
            collider_proxy_handles: &self.soa.collider_proxy_handles,
        })
    }

    /// Borrow the SoA lanes mutably. Same `None`-on-row-prototype
    /// contract as [`Self::body_lane_view`].
    pub fn body_lane_view_mut(&mut self) -> Option<ActiveBodyLaneViewMut<'_>> {
        if !matches!(self.layout, ActivePoolLayout::SoaLanes) {
            return None;
        }
        Some(ActiveBodyLaneViewMut {
            entities: &self.soa.entities,
            generations: &self.soa.generations,
            occupied: &self.soa.occupied,
            positions: &mut self.soa.positions,
            rotations: &mut self.soa.rotations,
            linear_velocities: &mut self.soa.linear_velocities,
            angular_velocities: &mut self.soa.angular_velocities,
            body_kinds: &mut self.soa.body_kinds,
            dirty_masks: &mut self.soa.dirty_masks,
            last_changed_ticks: &mut self.soa.last_changed_ticks,
            collider_proxy_handles: &mut self.soa.collider_proxy_handles,
        })
    }

    /// Iterate body rows in `chunk_size`-sized slot windows. Each
    /// chunk yields the (slot_id, row, occupied) tuples for the
    /// window — callers filter `occupied` for live-only scans.
    /// Layout-agnostic.
    pub fn iter_body_chunks(&self, chunk_size: usize) -> ActiveBodyChunkIter<'_> {
        ActiveBodyChunkIter {
            pool: self,
            chunk_size: chunk_size.max(1),
            cursor: 0,
        }
    }

    /// Iterate dirty rows in `chunk_size`-sized windows of the
    /// per-tick `dirty_rows` queue. Always layout-agnostic — dirty
    /// rows live in the same queue regardless of storage.
    pub fn iter_dirty_chunks(
        &self,
        chunk_size: usize,
    ) -> std::slice::Chunks<'_, ActiveBodyDirtyRow> {
        self.dirty_rows.chunks(chunk_size.max(1))
    }

    /// Snapshot the V2-P2 memory accounting fields per spec.
    pub fn memory_accounting(&self) -> ActivePoolMemoryAccounting {
        let (capacity_bytes, live_bytes) = match self.layout {
            ActivePoolLayout::RowPrototype => {
                let per_slot = core::mem::size_of::<PoolSlot>() as u64;
                let cap_bytes = (self.slots.capacity() as u64) * per_slot;
                let live_rows = self.active_row_count() as u64;
                let live_bytes = live_rows * per_slot;
                (cap_bytes, live_bytes)
            }
            ActivePoolLayout::SoaLanes => {
                // Sum capacity bytes across every lane.
                let cap = self.soa.entities.capacity() as u64;
                let cap_bytes = cap
                    * (core::mem::size_of::<Entity>() as u64
                        + core::mem::size_of::<ActiveBodyGeneration>() as u64
                        + core::mem::size_of::<bool>() as u64
                        + core::mem::size_of::<Vec3>() as u64 * 3 // pos + lin + ang
                        + core::mem::size_of::<Quat>() as u64
                        + core::mem::size_of::<ActiveBodyKind>() as u64
                        + core::mem::size_of::<ActivePoolDirtyMask>() as u64
                        + core::mem::size_of::<u32>() as u64
                        + core::mem::size_of::<ColliderProxyHandlePlaceholder>() as u64);
                let per_row = core::mem::size_of::<Entity>() as u64
                    + core::mem::size_of::<ActiveBodyGeneration>() as u64
                    + core::mem::size_of::<bool>() as u64
                    + core::mem::size_of::<Vec3>() as u64 * 3
                    + core::mem::size_of::<Quat>() as u64
                    + core::mem::size_of::<ActiveBodyKind>() as u64
                    + core::mem::size_of::<ActivePoolDirtyMask>() as u64
                    + core::mem::size_of::<u32>() as u64
                    + core::mem::size_of::<ColliderProxyHandlePlaceholder>() as u64;
                let live_rows = self.active_row_count() as u64;
                let live_bytes = live_rows * per_row;
                (cap_bytes, live_bytes)
            }
        };
        let fragmentation_ppm = capacity_bytes
            .saturating_sub(live_bytes)
            .saturating_mul(1_000_000)
            .checked_div(capacity_bytes)
            .map(|v| v as u32)
            .unwrap_or(0);
        ActivePoolMemoryAccounting {
            layout: self.layout,
            capacity_bytes,
            live_bytes,
            fragmentation_ppm,
            free_slot_count: self.free_slot_count(),
            dirty_row_count: self.dirty_rows.len() as u32,
        }
    }
}

/// Iterator over body chunks. Yields `ActiveBodyChunk` views of size
/// `chunk_size` until every slot has been visited.
pub struct ActiveBodyChunkIter<'a> {
    pool: &'a ActivePhysicsPool,
    chunk_size: usize,
    cursor: usize,
}

/// One chunk of body rows. Rows + occupied flags align element-wise
/// with `slot_ids` so consumers can re-form handles by combining the
/// slot id with the pool's generation lookup.
#[derive(Debug)]
pub struct ActiveBodyChunk {
    /// First slot id in this chunk.
    pub start: u32,
    /// Materialised body rows for this chunk's slots.
    pub rows: Vec<ActiveBodyRow>,
    /// Per-slot occupied flag.
    pub occupied: Vec<bool>,
    /// Per-slot generation.
    pub generations: Vec<ActiveBodyGeneration>,
}

impl Iterator for ActiveBodyChunkIter<'_> {
    type Item = ActiveBodyChunk;

    fn next(&mut self) -> Option<Self::Item> {
        let total = self.pool.slot_count() as usize;
        if self.cursor >= total {
            return None;
        }
        let end = (self.cursor + self.chunk_size).min(total);
        let mut rows = Vec::with_capacity(end - self.cursor);
        let mut occupied = Vec::with_capacity(end - self.cursor);
        let mut generations = Vec::with_capacity(end - self.cursor);
        for i in self.cursor..end {
            match self.pool.layout {
                ActivePoolLayout::RowPrototype => {
                    let slot = &self.pool.slots[i];
                    rows.push(slot.row);
                    occupied.push(slot.occupied);
                    generations.push(slot.generation);
                }
                ActivePoolLayout::SoaLanes => {
                    rows.push(self.pool.soa.row_at(i));
                    occupied.push(self.pool.soa.occupied[i]);
                    generations.push(self.pool.soa.generations[i]);
                }
            }
        }
        let chunk = ActiveBodyChunk {
            start: self.cursor as u32,
            rows,
            occupied,
            generations,
        };
        self.cursor = end;
        Some(chunk)
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
        match self.layout {
            ActivePoolLayout::RowPrototype => {
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
            ActivePoolLayout::SoaLanes => {
                for (slot_index, &occupied) in self.soa.occupied.iter().enumerate() {
                    if !occupied {
                        continue;
                    }
                    f(AvianActivePoolHandle::new(
                        slot_index as u32,
                        self.soa.generations[slot_index].raw(),
                    ));
                }
            }
        }
    }

    fn entity_for(&self, handle: AvianActivePoolHandle) -> Option<Entity> {
        self.row_owned(ActiveBodyHandle::from_avian(handle))
            .map(|row| row.entity)
    }
}

impl AvianActiveBodySource for ActivePhysicsPool {
    fn position(&self, handle: AvianActivePoolHandle) -> Option<Vec3> {
        self.row_owned(ActiveBodyHandle::from_avian(handle))
            .map(|row| row.position)
    }

    fn rotation(&self, handle: AvianActivePoolHandle) -> Option<Quat> {
        self.row_owned(ActiveBodyHandle::from_avian(handle))
            .map(|row| row.rotation)
    }

    fn linear_velocity(&self, handle: AvianActivePoolHandle) -> Option<Vec3> {
        self.row_owned(ActiveBodyHandle::from_avian(handle))
            .map(|row| row.linear_velocity)
    }

    fn angular_velocity(&self, handle: AvianActivePoolHandle) -> Option<Vec3> {
        self.row_owned(ActiveBodyHandle::from_avian(handle))
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

    // ========================================================================
    // V2-P2 SoA-lane tests
    // ========================================================================

    /// Build an empty pool configured for the SoA layout.
    fn soa_pool() -> ActivePhysicsPool {
        let mut pool = ActivePhysicsPool::default();
        pool.set_layout(ActivePoolLayout::SoaLanes);
        pool
    }

    fn allocate_soa(pool: &mut ActivePhysicsPool, entity: Entity) -> ActiveBodyHandle {
        let mut promotions = 0;
        match pool.allocate_or_lookup(entity, ActiveBodyKind::Dynamic, 1024, 0, &mut promotions) {
            AllocationOutcome::Allocated(h) | AllocationOutcome::Existing(h) => h,
            other => panic!("expected allocation, got {other:?}"),
        }
    }

    #[test]
    fn soa_default_layout_is_row_prototype() {
        let pool = ActivePhysicsPool::default();
        assert_eq!(pool.layout(), ActivePoolLayout::RowPrototype);
    }

    #[test]
    fn soa_set_layout_promotes_to_lanes() {
        let mut pool = ActivePhysicsPool::default();
        pool.set_layout(ActivePoolLayout::SoaLanes);
        assert_eq!(pool.layout(), ActivePoolLayout::SoaLanes);
        assert!(
            pool.body_lane_view().is_some(),
            "SoA layout exposes lane view"
        );
        // Row-prototype lane view is None on the SoA path.
        assert!(pool.row(ActiveBodyHandle::default()).is_none());
    }

    #[test]
    fn soa_allocation_populates_every_lane() {
        let mut pool = soa_pool();
        pool.begin_tick();
        let entity = make_entity(7);
        let handle = allocate_soa(&mut pool, entity);
        let lanes = pool.body_lane_view().expect("SoA lane view");
        let i = handle.id.raw() as usize;
        // Every lane has exactly one populated entry.
        assert_eq!(lanes.entities.len(), 1);
        assert_eq!(lanes.entities[i], entity);
        assert!(lanes.occupied[i]);
        assert_eq!(lanes.body_kinds[i], ActiveBodyKind::Dynamic);
        assert_eq!(lanes.dirty_masks[i], ActivePoolDirtyMask::NONE);
        assert_eq!(
            lanes.collider_proxy_handles[i],
            ColliderProxyHandlePlaceholder::NULL
        );
    }

    #[test]
    fn soa_remove_frees_slot_and_bumps_generation() {
        let mut pool = soa_pool();
        pool.begin_tick();
        let entity = make_entity(0);
        let handle = allocate_soa(&mut pool, entity);
        assert!(pool.remove_entity(entity));
        // Slot is freed but the lane entry still exists (occupied=false).
        let lanes = pool.body_lane_view().unwrap();
        assert!(!lanes.occupied[handle.id.raw() as usize]);
        // Stale handle no longer resolves.
        assert!(pool.row_owned(handle).is_none());
        // Slot reuse picks the same id back up with a bumped generation.
        let entity2 = make_entity(1);
        let handle2 = allocate_soa(&mut pool, entity2);
        assert_eq!(handle.id, handle2.id, "slot reused");
        assert_ne!(handle.generation, handle2.generation, "generation bumped");
    }

    #[test]
    fn soa_stale_handle_rejection_in_upsert() {
        let mut pool = soa_pool();
        pool.begin_tick();
        let entity = make_entity(0);
        let handle = allocate_soa(&mut pool, entity);
        pool.remove_entity(entity);
        // Stale upsert returns None and increments stale_rejections.
        let result = pool.upsert_from_ecs(handle, Vec3::X, Quat::IDENTITY, Vec3::Y, Vec3::Z);
        assert!(result.is_none());
        assert_eq!(pool.stale_rejections, 1);
    }

    #[test]
    fn soa_dirty_mask_accumulates_per_row() {
        let mut pool = soa_pool();
        pool.begin_tick();
        let handle = allocate_soa(&mut pool, make_entity(0));
        // Position change → POSITION dirty bit.
        pool.upsert_from_ecs(handle, Vec3::X, Quat::IDENTITY, Vec3::ZERO, Vec3::ZERO);
        let lanes = pool.body_lane_view().unwrap();
        let i = handle.id.raw() as usize;
        assert!(lanes.dirty_masks[i].contains(ActivePoolDirtyMask::POSITION));
        assert_eq!(lanes.last_changed_ticks[i], pool.current_tick());
        // Velocity change accumulates into the same per-row mask.
        pool.upsert_from_ecs(handle, Vec3::X, Quat::IDENTITY, Vec3::Y, Vec3::ZERO);
        let lanes = pool.body_lane_view().unwrap();
        assert!(lanes.dirty_masks[i].contains(ActivePoolDirtyMask::POSITION));
        assert!(lanes.dirty_masks[i].contains(ActivePoolDirtyMask::LINEAR_VELOCITY));
    }

    #[test]
    fn soa_chunk_iteration_visits_every_slot_in_order() {
        let mut pool = soa_pool();
        pool.begin_tick();
        for i in 0..7 {
            allocate_soa(&mut pool, make_entity(i));
        }
        let chunks: Vec<_> = pool.iter_body_chunks(3).collect();
        // 7 slots, chunk_size 3 → 3 chunks of (3, 3, 1).
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].rows.len(), 3);
        assert_eq!(chunks[0].start, 0);
        assert_eq!(chunks[1].rows.len(), 3);
        assert_eq!(chunks[1].start, 3);
        assert_eq!(chunks[2].rows.len(), 1);
        assert_eq!(chunks[2].start, 6);
        // Build twice + confirm chunk content matches: deterministic order.
        let chunks_again: Vec<_> = pool.iter_body_chunks(3).collect();
        for (a, b) in chunks.iter().zip(chunks_again.iter()) {
            assert_eq!(a.start, b.start);
            assert_eq!(a.occupied, b.occupied);
        }
    }

    #[test]
    fn soa_dirty_chunks_iterate_dirty_queue() {
        let mut pool = soa_pool();
        pool.begin_tick();
        for i in 0..5 {
            let h = allocate_soa(&mut pool, make_entity(i));
            // Use `i + 1` so every upsert is a guaranteed delta from
            // the default Vec3::ZERO position; otherwise i=0 produces
            // no dirty mask and the queue holds only 4 rows.
            pool.upsert_from_ecs(
                h,
                Vec3::splat((i + 1) as f32),
                Quat::IDENTITY,
                Vec3::ZERO,
                Vec3::ZERO,
            );
        }
        let chunks: Vec<_> = pool.iter_dirty_chunks(2).collect();
        assert_eq!(chunks.iter().map(|c| c.len()).sum::<usize>(), 5);
        assert_eq!(chunks.len(), 3); // (2, 2, 1)
    }

    #[test]
    fn soa_layout_parity_with_row_prototype_for_basic_writeback() {
        // Same allocation + upsert sequence in both layouts must
        // produce identical row state + identical per-tick dirty queue.
        let entities: Vec<Entity> = (0..5).map(make_entity).collect();
        let positions: Vec<Vec3> = (0..5).map(|i| Vec3::splat(i as f32)).collect();

        let mut row_pool = ActivePhysicsPool::default();
        row_pool.begin_tick();
        let mut soa_pool = soa_pool();
        soa_pool.begin_tick();

        let mut row_handles = Vec::new();
        let mut soa_handles = Vec::new();
        for &e in &entities {
            row_handles.push(allocate_soa(&mut row_pool, e));
            soa_handles.push(allocate_soa(&mut soa_pool, e));
        }
        for (i, h) in row_handles.iter().enumerate() {
            row_pool.upsert_from_ecs(*h, positions[i], Quat::IDENTITY, Vec3::ZERO, Vec3::ZERO);
        }
        for (i, h) in soa_handles.iter().enumerate() {
            soa_pool.upsert_from_ecs(*h, positions[i], Quat::IDENTITY, Vec3::ZERO, Vec3::ZERO);
        }

        // Per-row state matches between layouts.
        for (i, &e) in entities.iter().enumerate() {
            let row_view = row_pool.row_owned(row_handles[i]).unwrap();
            let soa_view = soa_pool.row_owned(soa_handles[i]).unwrap();
            assert_eq!(row_view.entity, e);
            assert_eq!(row_view.position, soa_view.position);
            assert_eq!(row_view.body_kind, soa_view.body_kind);
        }
        // Dirty queue size matches.
        assert_eq!(row_pool.dirty_rows.len(), soa_pool.dirty_rows.len());
    }

    #[test]
    fn soa_memory_accounting_reports_nonzero_capacity() {
        let mut pool = soa_pool();
        pool.begin_tick();
        for i in 0..3 {
            allocate_soa(&mut pool, make_entity(i));
        }
        let acct = pool.memory_accounting();
        assert_eq!(acct.layout, ActivePoolLayout::SoaLanes);
        assert!(acct.capacity_bytes > 0, "capacity_bytes should be nonzero");
        assert!(acct.live_bytes > 0, "live_bytes should be nonzero");
        assert_eq!(acct.free_slot_count, 0, "no removals → no free slots");
    }

    #[test]
    fn row_memory_accounting_reports_nonzero_capacity() {
        let mut pool = ActivePhysicsPool::default();
        pool.begin_tick();
        for i in 0..3 {
            allocate_soa(&mut pool, make_entity(i));
        }
        let acct = pool.memory_accounting();
        assert_eq!(acct.layout, ActivePoolLayout::RowPrototype);
        assert!(acct.capacity_bytes > 0);
        assert!(acct.live_bytes > 0);
    }

    #[test]
    fn soa_fragmentation_increases_after_remove() {
        let mut pool = soa_pool();
        pool.begin_tick();
        let mut handles = Vec::new();
        for i in 0..10 {
            handles.push((make_entity(i), allocate_soa(&mut pool, make_entity(i))));
        }
        let baseline = pool.memory_accounting();
        // Remove half.
        for &(e, _) in &handles[..5] {
            pool.remove_entity(e);
        }
        let after = pool.memory_accounting();
        assert!(
            after.fragmentation_ppm > baseline.fragmentation_ppm,
            "fragmentation increased after removals: baseline={} after={}",
            baseline.fragmentation_ppm,
            after.fragmentation_ppm
        );
        assert_eq!(after.free_slot_count, 5);
    }

    #[test]
    fn soa_active_set_iteration_skips_freed_slots() {
        let mut pool = soa_pool();
        pool.begin_tick();
        let h1 = allocate_soa(&mut pool, make_entity(0));
        let _h2 = allocate_soa(&mut pool, make_entity(1));
        let h3 = allocate_soa(&mut pool, make_entity(2));
        pool.remove_entity(make_entity(1));
        let mut visited = Vec::new();
        let active_set: &dyn PhysicsActiveSet = &pool;
        active_set.for_each_handle(&mut |h| visited.push(h));
        assert_eq!(visited.len(), 2, "freed slot must not appear in iteration");
        assert!(visited.iter().any(|h| h.id == h1.id.raw()));
        assert!(visited.iter().any(|h| h.id == h3.id.raw()));
    }
}
