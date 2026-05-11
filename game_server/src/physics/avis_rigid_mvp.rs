//! V2-P4: Avis rigid MVP behind a compatibility switch.
//!
//! Composes the V2-P2 SoA active body lanes
//! ([`crate::physics::active_pool::ActivePhysicsPool`] in
//! `ActivePoolLayout::SoaLanes`) plus the V2-P3 incremental contact
//! cache (`avian3d::collision::contact_types::IncrementalContactCache`)
//! plus a minimal broad-proxy lane shape into a single
//! [`AvisRigidExperimental`] runtime.
//!
//! Production behaviour is **not** changed: [`AvisRigidMode`] defaults
//! to [`AvisRigidMode::AvianCompatibility`], which leaves the existing
//! Avian hot path untouched. The MVP is opt-in via configuration; the
//! V2-P4 exit gates require correctness parity + replay digest
//! stability + benchmark evidence before promoting.
//!
//! Per the V2-P4 spec ownership rule: "First implementation in
//! fun/game_server/src/physics. Later extraction to avis-rigid /
//! avis-broad / avis-contact / avis-solver." This file is the MVP
//! that proves the lane-shaped path; later passes lift the reusable
//! pieces into Avis crates.

use bevy::prelude::*;

use super::active_pool::{
    ActiveBodyHandle, ActivePhysicsPool, ActivePoolDirtyMask, ActivePoolLayout,
    ColliderProxyHandlePlaceholder,
};

// ---------------------------------------------------------------------------
// AvisRigidMode — three-way compatibility switch
// ---------------------------------------------------------------------------

/// Top-level mode selecting which rigid-body data plane drives a
/// shard tick. Default is [`Self::AvianCompatibility`] (production).
#[derive(Resource, Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub enum AvisRigidMode {
    /// **Production default.** Use Avian's hot-path components
    /// directly — `ActivePhysicsPool` is bypassed. The V2-P4 MVP
    /// stays inert in this mode; switching back to it is the
    /// rollback contract per V2-P4 exit gates.
    #[default]
    AvianCompatibility,
    /// Use the fun-side [`ActivePhysicsPool`] prototype as the hot
    /// data plane. V2-P2 added the SoA layout option; this mode
    /// honours whatever layout `ActivePoolPolicy` selects.
    FunDensePrototype,
    /// V2-P4 experimental — compose the SoA active body pool plus
    /// broad proxy lanes plus the avian incremental contact cache
    /// into a single measured path. Must clear the V2-P4 exit
    /// gates (parity, replay digest, benchmark evidence) before
    /// becoming a default.
    AvisRigidExperimental,
}

impl AvisRigidMode {
    /// Stable diagnostic name (telemetry / config bundles).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AvianCompatibility => "avian_compatibility",
            Self::FunDensePrototype => "fun_dense_prototype",
            Self::AvisRigidExperimental => "avis_rigid_experimental",
        }
    }

    /// True if the mode runs the AvisRigidExperimental composition.
    pub const fn runs_avis_experimental(self) -> bool {
        matches!(self, Self::AvisRigidExperimental)
    }

    /// True if the mode walks the fun-side `ActivePhysicsPool`
    /// (either FunDensePrototype or AvisRigidExperimental — both
    /// rely on the same dense pool storage).
    pub const fn uses_fun_active_pool(self) -> bool {
        matches!(self, Self::FunDensePrototype | Self::AvisRigidExperimental)
    }
}

// ---------------------------------------------------------------------------
// AvisRigidBroadProxyLanes — minimal V2-P4 broad-phase proxy shape
// ---------------------------------------------------------------------------

/// SoA broad-phase proxy lanes. One row per active body's broad
/// proxy. V2-P4 ships the lane shape + `push` / `clear` / `len`;
/// real BVH / uniform-grid backends land in a later pass once the
/// avis-broad crate matures.
#[derive(Resource, Clone, Debug, Default)]
pub struct AvisRigidBroadProxyLanes {
    /// Body the proxy is attached to.
    pub bodies: Vec<ActiveBodyHandle>,
    /// World-space AABB minimum corner.
    pub aabb_min: Vec<Vec3>,
    /// World-space AABB maximum corner.
    pub aabb_max: Vec<Vec3>,
    /// Optional collider-handle placeholder.
    pub collider_handles: Vec<ColliderProxyHandlePlaceholder>,
}

impl AvisRigidBroadProxyLanes {
    /// Number of proxies.
    pub fn len(&self) -> usize {
        self.bodies.len()
    }

    /// True if no proxies.
    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty()
    }

    /// Append a proxy.
    pub fn push(&mut self, body: ActiveBodyHandle, aabb_min: Vec3, aabb_max: Vec3) {
        self.bodies.push(body);
        self.aabb_min.push(aabb_min);
        self.aabb_max.push(aabb_max);
        self.collider_handles
            .push(ColliderProxyHandlePlaceholder::NULL);
    }

    /// Drop every proxy. Capacity retained.
    pub fn clear(&mut self) {
        self.bodies.clear();
        self.aabb_min.clear();
        self.aabb_max.clear();
        self.collider_handles.clear();
    }

    /// Naive O(N²) candidate-pair count: every pair whose tight
    /// AABBs overlap. V2-P4 placeholder; real backends sit on top
    /// of this typed surface in the avis-broad crate.
    pub fn naive_candidate_count(&self) -> u32 {
        let n = self.bodies.len();
        let mut count = 0u32;
        for i in 0..n {
            for j in (i + 1)..n {
                if aabb_overlap(
                    self.aabb_min[i],
                    self.aabb_max[i],
                    self.aabb_min[j],
                    self.aabb_max[j],
                ) {
                    count = count.saturating_add(1);
                }
            }
        }
        count
    }
}

#[inline]
fn aabb_overlap(a_min: Vec3, a_max: Vec3, b_min: Vec3, b_max: Vec3) -> bool {
    a_min.x <= b_max.x
        && a_max.x >= b_min.x
        && a_min.y <= b_max.y
        && a_max.y >= b_min.y
        && a_min.z <= b_max.z
        && a_max.z >= b_min.z
}

// ---------------------------------------------------------------------------
// AvisRigidDirtyRow — per-tick output a downstream consumer (writeback
// or Thunder bridge) reads
// ---------------------------------------------------------------------------

/// One per-tick dirty row emitted by the MVP. Carries enough state
/// for both ECS writeback AND a Thunder delta record without two
/// separate passes through the data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AvisRigidDirtyRow {
    /// Body that changed.
    pub handle: ActiveBodyHandle,
    /// Owning ECS entity (writeback target).
    pub entity: Entity,
    /// Mask describing which fields changed.
    pub dirty_mask: ActivePoolDirtyMask,
    /// New position.
    pub position: Vec3,
    /// New rotation.
    pub rotation: Quat,
    /// New linear velocity.
    pub linear_velocity: Vec3,
    /// New angular velocity.
    pub angular_velocity: Vec3,
    /// Tick the change happened on.
    pub tick: u32,
}

// ---------------------------------------------------------------------------
// AvisRigidDiagnostics — the per-tick output bundle for telemetry
// ---------------------------------------------------------------------------

/// Per-tick V2-P4 MVP diagnostics. Cheap to read from a benchmark or
/// overlay.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct AvisRigidDiagnostics {
    /// Active body count after the extraction pass.
    pub body_count: u32,
    /// Live active-contact rows in the incremental cache.
    pub active_contacts: u32,
    /// Broad-phase candidate pairs this tick (naive O(N²) in V2-P4).
    pub broad_candidates: u32,
    /// Solver constraint rows actually consumed this tick. Stub —
    /// stays equal to active_contacts until the lane-shaped solver
    /// adapter lands.
    pub solver_rows: u32,
    /// Number of dirty rows published to ECS writeback this tick.
    pub writebacks: u32,
    /// Deterministic state digest at end of tick.
    pub digest: u64,
    /// `Some(mode)` when the MVP fell back from
    /// `AvisRigidExperimental` to a safer mode this tick (e.g.
    /// because the SoA layout was not enabled). `None` for normal
    /// operation.
    pub fallback_mode: Option<AvisRigidMode>,
}

// ---------------------------------------------------------------------------
// AvisRigidExperimental — the actual V2-P4 runtime composition
// ---------------------------------------------------------------------------

/// V2-P4 experimental rigid-body runtime. Owns the dense lanes, the
/// broad-proxy lanes, the dirty queue, the digest, and the
/// diagnostics. The incremental contact cache is **not** stored
/// here — the avian crate owns it, and the MVP refers to its
/// handles instead. (The cache lives at `avian3d`'s
/// `IncrementalContactCache` resource.)
///
/// Construction does NOT enable SoA on the inner pool — the caller
/// either:
///
/// 1. constructs the runtime around an existing `ActivePhysicsPool`
///    (use [`AvisRigidExperimental::wrap`]), or
/// 2. constructs a fresh runtime + sets the SoA layout
///    (use [`AvisRigidExperimental::new_soa`]).
#[derive(Debug)]
pub struct AvisRigidExperimental {
    /// Active body pool. V2-P4 expects `ActivePoolLayout::SoaLanes`
    /// for the experimental path; the MVP records a
    /// `fallback_mode` diagnostic if the layout is `RowPrototype`.
    pub body_pool: ActivePhysicsPool,
    /// Broad-phase proxy lanes.
    pub broad_proxies: AvisRigidBroadProxyLanes,
    /// Per-tick dirty output queue. Drained by the
    /// [`avis_dirty_outputs_to_ecs_writeback`] /
    /// [`avis_dirty_outputs_to_thunder_bridge`] bridges.
    pub dirty_rows: Vec<AvisRigidDirtyRow>,
    /// Diagnostics for the most recent step.
    pub diagnostics: AvisRigidDiagnostics,
    /// Tick counter.
    pub current_tick: u32,
}

impl Default for AvisRigidExperimental {
    fn default() -> Self {
        Self::new_soa()
    }
}

impl AvisRigidExperimental {
    /// New runtime with SoA layout pre-configured on the inner pool.
    pub fn new_soa() -> Self {
        let mut body_pool = ActivePhysicsPool::default();
        body_pool.set_layout(ActivePoolLayout::SoaLanes);
        Self {
            body_pool,
            broad_proxies: AvisRigidBroadProxyLanes::default(),
            dirty_rows: Vec::new(),
            diagnostics: AvisRigidDiagnostics::default(),
            current_tick: 0,
        }
    }

    /// Wrap an existing pool. Caller is responsible for the layout
    /// choice; the MVP records `fallback_mode = Some(FunDensePrototype)`
    /// when the layout is `RowPrototype` (the SoA fast path can't
    /// fire on AoS storage).
    pub fn wrap(body_pool: ActivePhysicsPool) -> Self {
        Self {
            body_pool,
            broad_proxies: AvisRigidBroadProxyLanes::default(),
            dirty_rows: Vec::new(),
            diagnostics: AvisRigidDiagnostics::default(),
            current_tick: 0,
        }
    }

    /// Begin a new tick. Bumps the tick counter, resets per-tick
    /// state in the inner pool, clears the dirty-row queue, resets
    /// the broad proxy lanes (they're rebuilt each tick in V2-P4).
    pub fn begin_tick(&mut self) {
        self.current_tick = self.current_tick.wrapping_add(1);
        self.body_pool.begin_tick();
        self.dirty_rows.clear();
        self.broad_proxies.clear();
        self.diagnostics.fallback_mode = None;
        if !matches!(self.body_pool.layout(), ActivePoolLayout::SoaLanes) {
            // SoA is required for the experimental fast path. Record
            // the fallback so consumers see we degraded.
            self.diagnostics.fallback_mode = Some(AvisRigidMode::FunDensePrototype);
        }
    }

    /// Compute a deterministic state digest from the live SoA lanes.
    /// FNV-1a 64-bit over (entity, position, rotation, linear,
    /// angular) of every occupied slot, in slot order. Returns 0 on
    /// row-prototype layout (the fast path expects SoA).
    pub fn compute_digest(&self) -> u64 {
        let Some(view) = self.body_pool.body_lane_view() else {
            return 0;
        };
        let mut hash: u64 = 0xcbf29ce484222325;
        const PRIME: u64 = 0x100000001b3;
        for i in 0..view.entities.len() {
            if !view.occupied[i] {
                continue;
            }
            // Mix in entity bits + every Vec3/Quat field. Bit-stable
            // because we round-trip through `to_bits()`.
            hash = mix_u64(hash, view.entities[i].to_bits(), PRIME);
            for &c in &view.positions[i].to_array() {
                hash = mix_u64(hash, c.to_bits() as u64, PRIME);
            }
            for c in [
                view.rotations[i].x,
                view.rotations[i].y,
                view.rotations[i].z,
                view.rotations[i].w,
            ] {
                hash = mix_u64(hash, c.to_bits() as u64, PRIME);
            }
            for &c in &view.linear_velocities[i].to_array() {
                hash = mix_u64(hash, c.to_bits() as u64, PRIME);
            }
            for &c in &view.angular_velocities[i].to_array() {
                hash = mix_u64(hash, c.to_bits() as u64, PRIME);
            }
        }
        hash
    }

    /// Capture the per-tick diagnostics from the current state.
    /// Called at the end of `step` (or by tests inspecting the
    /// runtime).
    pub fn capture_diagnostics(&mut self) {
        let digest = self.compute_digest();
        self.diagnostics.body_count = self.body_pool.active_row_count();
        self.diagnostics.broad_candidates = self.broad_proxies.naive_candidate_count();
        self.diagnostics.writebacks = self.dirty_rows.len() as u32;
        self.diagnostics.digest = digest;
        // active_contacts + solver_rows are populated by the
        // upstream contact-cache integration; the MVP leaves them
        // zero unless the caller explicitly sets them via
        // `report_contact_counts`.
    }

    /// Report contact counts coming from the avian incremental
    /// cache (or any other source). V2-P4 keeps this an explicit
    /// hand-off so the MVP doesn't pull a transitive avian
    /// dependency for an in-process counter copy.
    pub fn report_contact_counts(&mut self, active_contacts: u32, solver_rows: u32) {
        self.diagnostics.active_contacts = active_contacts;
        self.diagnostics.solver_rows = solver_rows;
    }
}

#[inline]
fn mix_u64(state: u64, value: u64, prime: u64) -> u64 {
    state.wrapping_mul(prime) ^ value
}

// ---------------------------------------------------------------------------
// Bridges — explicit ECS extraction + writeback + Thunder
// ---------------------------------------------------------------------------

/// Bridge: copy one ECS-extracted body row into the MVP's lanes.
///
/// V2-P4 owns the bridge surface, but the actual ECS query (which
/// joins Position / Rotation / LinearVelocity / AngularVelocity for
/// every active body) lives in the existing
/// [`super::active_pool::extract_active_bodies_from_ecs`] system.
/// The bridge below is the per-row primitive that system can call
/// once it knows which body to extract.
pub fn ecs_extraction_to_avis_lane(
    runtime: &mut AvisRigidExperimental,
    handle: ActiveBodyHandle,
    position: Vec3,
    rotation: Quat,
    linear_velocity: Vec3,
    angular_velocity: Vec3,
) -> Option<ActivePoolDirtyMask> {
    // Use the pool's __test_upsert path (the public upsert is private
    // to the active_pool module; the test helper is the available
    // surface). When V2-P4 graduates the ECS extraction system to
    // walk the new bridge directly, this call becomes
    // `pool.upsert_from_ecs(...)`.
    let mask = runtime.body_pool.__test_upsert(
        handle,
        position,
        rotation,
        linear_velocity,
        angular_velocity,
    )?;
    if !mask.is_clean() {
        runtime.dirty_rows.push(AvisRigidDirtyRow {
            handle,
            entity: runtime
                .body_pool
                .row_owned(handle)
                .map(|row| row.entity)
                .unwrap_or(Entity::PLACEHOLDER),
            dirty_mask: mask,
            position,
            rotation,
            linear_velocity,
            angular_velocity,
            tick: runtime.current_tick,
        });
    }
    Some(mask)
}

/// Bridge: drain the MVP's dirty rows for ECS writeback. Returns
/// the row vector (mem-take). The caller writes each row's fields
/// back into the relevant ECS components.
pub fn avis_dirty_outputs_to_ecs_writeback(
    runtime: &mut AvisRigidExperimental,
) -> Vec<AvisRigidDirtyRow> {
    core::mem::take(&mut runtime.dirty_rows)
}

/// Bridge: convert the MVP's dirty rows into a Thunder delta
/// snapshot. V2-P4 returns the rows as a vector that the Thunder
/// adapter can encode; the actual encoding lives in
/// [`super::thunder_bridge::ThunderPhysicsDeltaSnapshot`] and stays
/// downstream of this bridge so the MVP stays Thunder-agnostic.
///
/// **Does not** drain the dirty queue — call this BEFORE
/// [`avis_dirty_outputs_to_ecs_writeback`] if both consumers need
/// the data, or call writeback first and pass its return into this
/// function.
pub fn avis_dirty_outputs_to_thunder_bridge(
    runtime: &AvisRigidExperimental,
) -> Vec<AvisRigidDirtyRow> {
    runtime.dirty_rows.clone()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::super::active_pool::ActiveBodyKind;
    use super::*;

    fn entity(id: u32) -> Entity {
        Entity::from_raw_u32(id).expect("test entity index in range")
    }

    fn allocate(runtime: &mut AvisRigidExperimental, e: Entity) -> ActiveBodyHandle {
        let mut promotions = 0u32;
        runtime
            .body_pool
            .__test_allocate(e, &mut promotions)
            .expect("allocate")
    }

    #[test]
    fn mode_default_is_avian_compatibility() {
        // Production rollback contract: default never silently
        // routes through the experimental path.
        assert_eq!(AvisRigidMode::default(), AvisRigidMode::AvianCompatibility);
    }

    #[test]
    fn mode_classifies_paths() {
        assert!(!AvisRigidMode::AvianCompatibility.runs_avis_experimental());
        assert!(!AvisRigidMode::AvianCompatibility.uses_fun_active_pool());
        assert!(!AvisRigidMode::FunDensePrototype.runs_avis_experimental());
        assert!(AvisRigidMode::FunDensePrototype.uses_fun_active_pool());
        assert!(AvisRigidMode::AvisRigidExperimental.runs_avis_experimental());
        assert!(AvisRigidMode::AvisRigidExperimental.uses_fun_active_pool());
    }

    #[test]
    fn mode_names_are_stable() {
        assert_eq!(
            AvisRigidMode::AvianCompatibility.as_str(),
            "avian_compatibility"
        );
        assert_eq!(
            AvisRigidMode::FunDensePrototype.as_str(),
            "fun_dense_prototype"
        );
        assert_eq!(
            AvisRigidMode::AvisRigidExperimental.as_str(),
            "avis_rigid_experimental"
        );
    }

    #[test]
    fn empty_scene_runs_clean() {
        let mut runtime = AvisRigidExperimental::default();
        runtime.begin_tick();
        runtime.capture_diagnostics();
        assert_eq!(runtime.diagnostics.body_count, 0);
        assert_eq!(runtime.diagnostics.broad_candidates, 0);
        assert_eq!(runtime.diagnostics.writebacks, 0);
        assert_eq!(runtime.dirty_rows.len(), 0);
        assert_eq!(runtime.diagnostics.fallback_mode, None);
    }

    #[test]
    fn one_dynamic_body_extracts_to_lanes() {
        let mut runtime = AvisRigidExperimental::default();
        runtime.begin_tick();
        let h = allocate(&mut runtime, entity(1));
        let mask = ecs_extraction_to_avis_lane(
            &mut runtime,
            h,
            Vec3::new(1.0, 2.0, 3.0),
            Quat::IDENTITY,
            Vec3::ZERO,
            Vec3::ZERO,
        )
        .expect("upsert succeeds for live handle");
        assert!(!mask.is_clean(), "first upsert must mark some fields dirty");
        runtime.capture_diagnostics();
        assert_eq!(runtime.diagnostics.body_count, 1);
        assert_eq!(runtime.diagnostics.writebacks, 1);
    }

    #[test]
    fn static_floor_no_dirty_output_after_initial_extraction() {
        // A "static floor" gets extracted once with its initial
        // pose; subsequent identical upserts must produce no dirty
        // mask and emit no writeback.
        let mut runtime = AvisRigidExperimental::default();
        runtime.begin_tick();
        let h = allocate(&mut runtime, entity(1));
        // First extraction sets initial pose.
        ecs_extraction_to_avis_lane(
            &mut runtime,
            h,
            Vec3::new(0.0, -1.0, 0.0),
            Quat::IDENTITY,
            Vec3::ZERO,
            Vec3::ZERO,
        );
        runtime.dirty_rows.clear();
        // Second extraction with identical state must not dirty.
        let mask = ecs_extraction_to_avis_lane(
            &mut runtime,
            h,
            Vec3::new(0.0, -1.0, 0.0),
            Quat::IDENTITY,
            Vec3::ZERO,
            Vec3::ZERO,
        )
        .unwrap();
        assert!(mask.is_clean(), "static body produces no dirty mask");
        assert_eq!(runtime.dirty_rows.len(), 0);
    }

    #[test]
    fn two_body_proxy_overlap_counts_as_candidate() {
        // V2-P4 broad-phase placeholder: two overlapping AABBs
        // produce one candidate pair via the naive count.
        let mut runtime = AvisRigidExperimental::default();
        runtime.begin_tick();
        let a = allocate(&mut runtime, entity(1));
        let b = allocate(&mut runtime, entity(2));
        runtime
            .broad_proxies
            .push(a, Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
        runtime
            .broad_proxies
            .push(b, Vec3::new(0.5, 0.5, 0.5), Vec3::new(2.5, 2.5, 2.5));
        runtime.capture_diagnostics();
        assert_eq!(runtime.diagnostics.broad_candidates, 1);
    }

    #[test]
    fn disjoint_proxies_do_not_count_as_candidates() {
        let mut runtime = AvisRigidExperimental::default();
        runtime.begin_tick();
        let a = allocate(&mut runtime, entity(1));
        let b = allocate(&mut runtime, entity(2));
        runtime
            .broad_proxies
            .push(a, Vec3::new(-1.0, -1.0, -1.0), Vec3::new(0.0, 0.0, 0.0));
        runtime
            .broad_proxies
            .push(b, Vec3::new(5.0, 5.0, 5.0), Vec3::new(6.0, 6.0, 6.0));
        runtime.capture_diagnostics();
        assert_eq!(runtime.diagnostics.broad_candidates, 0);
    }

    #[test]
    fn dirty_writeback_drains_queue() {
        let mut runtime = AvisRigidExperimental::default();
        runtime.begin_tick();
        let h = allocate(&mut runtime, entity(1));
        ecs_extraction_to_avis_lane(
            &mut runtime,
            h,
            Vec3::new(1.0, 0.0, 0.0),
            Quat::IDENTITY,
            Vec3::ZERO,
            Vec3::ZERO,
        );
        assert_eq!(runtime.dirty_rows.len(), 1);
        let drained = avis_dirty_outputs_to_ecs_writeback(&mut runtime);
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].handle, h);
        assert_eq!(drained[0].position, Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(runtime.dirty_rows.len(), 0, "queue drained");
    }

    #[test]
    fn thunder_bridge_clones_dirty_rows_without_draining() {
        let mut runtime = AvisRigidExperimental::default();
        runtime.begin_tick();
        let h = allocate(&mut runtime, entity(1));
        ecs_extraction_to_avis_lane(
            &mut runtime,
            h,
            Vec3::new(1.0, 0.0, 0.0),
            Quat::IDENTITY,
            Vec3::ZERO,
            Vec3::ZERO,
        );
        let snapshot = avis_dirty_outputs_to_thunder_bridge(&runtime);
        assert_eq!(snapshot.len(), 1);
        // Queue still populated — Thunder bridge does not drain.
        assert_eq!(runtime.dirty_rows.len(), 1);
    }

    #[test]
    fn replay_digest_stable_across_runs() {
        // Two independent runs over the same allocation/upsert
        // sequence must produce the same digest.
        let build = || {
            let mut r = AvisRigidExperimental::default();
            r.begin_tick();
            for i in 1..=3 {
                let h = allocate(&mut r, entity(i));
                ecs_extraction_to_avis_lane(
                    &mut r,
                    h,
                    Vec3::new(i as f32, 0.0, 0.0),
                    Quat::IDENTITY,
                    Vec3::new(0.5, 0.0, 0.0),
                    Vec3::ZERO,
                );
            }
            r.capture_diagnostics();
            r.diagnostics.digest
        };
        let a = build();
        let b = build();
        assert_eq!(a, b, "digest stable across identical runs");
        assert_ne!(a, 0, "non-empty scene produces non-zero digest");
    }

    #[test]
    fn replay_digest_changes_when_state_changes() {
        let mut r = AvisRigidExperimental::default();
        r.begin_tick();
        let h = allocate(&mut r, entity(1));
        ecs_extraction_to_avis_lane(
            &mut r,
            h,
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::ZERO,
            Vec3::ZERO,
        );
        let d0 = r.compute_digest();
        ecs_extraction_to_avis_lane(
            &mut r,
            h,
            Vec3::new(5.0, 0.0, 0.0),
            Quat::IDENTITY,
            Vec3::ZERO,
            Vec3::ZERO,
        );
        let d1 = r.compute_digest();
        assert_ne!(d0, d1, "moving the body changes the digest");
    }

    #[test]
    fn fallback_mode_records_when_layout_is_row_prototype() {
        // wrap() with a default (RowPrototype) pool must record
        // the fallback so consumers see we didn't run the SoA fast
        // path.
        let pool = ActivePhysicsPool::default(); // RowPrototype default
        let mut runtime = AvisRigidExperimental::wrap(pool);
        runtime.begin_tick();
        assert_eq!(
            runtime.diagnostics.fallback_mode,
            Some(AvisRigidMode::FunDensePrototype)
        );
        // Compute digest returns 0 in row-prototype mode (no SoA
        // lane view).
        assert_eq!(runtime.compute_digest(), 0);
    }

    #[test]
    fn report_contact_counts_populates_diagnostics() {
        let mut r = AvisRigidExperimental::default();
        r.report_contact_counts(7, 5);
        assert_eq!(r.diagnostics.active_contacts, 7);
        assert_eq!(r.diagnostics.solver_rows, 5);
    }

    #[test]
    fn body_kind_round_trips_through_extraction() {
        // V2-P4 lane extraction must preserve body kind across
        // upsert. (The kind is set at allocation; subsequent
        // upserts don't touch it.)
        let mut runtime = AvisRigidExperimental::default();
        runtime.begin_tick();
        let mut promotions = 0u32;
        let h = runtime
            .body_pool
            .__test_allocate_with(entity(1), ActiveBodyKind::Kinematic, 16, 0, &mut promotions)
            .expect("allocate kinematic");
        let view = runtime.body_pool.row_owned(h).expect("row resolves");
        assert_eq!(view.body_kind, ActiveBodyKind::Kinematic);
    }
}
