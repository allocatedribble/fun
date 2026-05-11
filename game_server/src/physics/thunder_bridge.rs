//! Adapter that exposes the Fun active-body pool to Thunder as a
//! [`PhysicsDeltaSource`].
//!
//! The pool publishes per-tick dirty rows in Avian/Bevy world units; the
//! Thunder bridge wants stable [`NetEntity`] handles plus float
//! [`BodyState3`] snapshots. This adapter walks one tick's worth of
//! dirty rows, resolves Entity→NetEntity through a caller-supplied
//! mapping, and stages the snapshot into a pair of [`Vec`]/[`HashMap`]
//! buffers that satisfy the Thunder trait without coupling either crate
//! to the other.

use hashbrown::HashMap;

use bevy::prelude::*;
use thunder::physics::BodyState3;
use thunder::physics_delta::{DirtyMask, PhysicsDeltaSource, PhysicsDirtyRow};
use thunder::protocol::NetEntity;
use thunder::timeline::NetworkTick;

use super::active_pool::{ActiveBodyDirtyRow, ActivePhysicsPool, ActivePoolDirtyMask};

/// Snapshot of a single tick's dirty rows + body states, translated
/// into Thunder-side types.
pub struct ThunderPhysicsDeltaSnapshot {
    rows: Vec<PhysicsDirtyRow>,
    states: HashMap<NetEntity, BodyState3>,
}

impl ThunderPhysicsDeltaSnapshot {
    /// Build a snapshot from the pool's dirty rows.
    ///
    /// `entity_to_net` resolves the ECS [`Entity`] for a row to a stable
    /// [`NetEntity`]; rows whose entity has no mapping are skipped (this
    /// is the same behavior as Thunder's stale-handle filter, just on
    /// the producer side).
    ///
    /// `tick` is the server tick this snapshot represents.
    pub fn new(
        pool: &ActivePhysicsPool,
        dirty: &[ActiveBodyDirtyRow],
        tick: NetworkTick,
        mut entity_to_net: impl FnMut(Entity) -> Option<NetEntity>,
    ) -> Self {
        let mut rows = Vec::with_capacity(dirty.len());
        let mut states = HashMap::with_capacity(dirty.len());

        for record in dirty {
            let Some(row) = pool.row(record.handle) else {
                continue;
            };
            let Some(net_entity) = entity_to_net(row.entity) else {
                continue;
            };
            let mask = active_pool_to_thunder_mask(record.dirty_mask);
            rows.push(PhysicsDirtyRow {
                entity: net_entity,
                mask,
                tick,
                position: row.position.to_array(),
            });
            states.insert(
                net_entity,
                BodyState3 {
                    translation: row.position.to_array(),
                    rotation: [
                        row.rotation.x,
                        row.rotation.y,
                        row.rotation.z,
                        row.rotation.w,
                    ],
                    linear_velocity: row.linear_velocity.to_array(),
                    angular_velocity: row.angular_velocity.to_array(),
                    sleeping: false,
                },
            );
        }

        Self { rows, states }
    }

    /// Number of dirty rows captured.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }
}

impl PhysicsDeltaSource for ThunderPhysicsDeltaSnapshot {
    fn for_each_dirty_row(&self, f: &mut dyn FnMut(&PhysicsDirtyRow)) {
        for row in &self.rows {
            f(row);
        }
    }

    fn body_state(&self, entity: NetEntity) -> Option<BodyState3> {
        self.states.get(&entity).copied()
    }

    fn dirty_row_count(&self) -> usize {
        self.rows.len()
    }
}

/// Convert the pool's typed dirty mask into Thunder's wire-side mask.
///
/// The pool only tracks transform/velocity dirty bits today; lifecycle
/// transitions (`SLEEP_STATE`, `BODY_CREATED`, `BODY_REMOVED`,
/// `SHARD_OWNER_CHANGED`) are surfaced through other channels and are
/// out of scope for the dirty-row stream itself.
fn active_pool_to_thunder_mask(mask: ActivePoolDirtyMask) -> DirtyMask {
    let mut out = DirtyMask::NONE;
    if mask.contains(ActivePoolDirtyMask::POSITION) {
        out = out.union(DirtyMask::POSITION);
    }
    if mask.contains(ActivePoolDirtyMask::ROTATION) {
        out = out.union(DirtyMask::ROTATION);
    }
    if mask.contains(ActivePoolDirtyMask::LINEAR_VELOCITY) {
        out = out.union(DirtyMask::LINEAR_VELOCITY);
    }
    if mask.contains(ActivePoolDirtyMask::ANGULAR_VELOCITY) {
        out = out.union(DirtyMask::ANGULAR_VELOCITY);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physics::active_pool::ActivePhysicsPool;
    use thunder::physics_delta::{PhysicsDeltaAoi, PhysicsDeltaBridge, PhysicsDeltaBudget};
    use thunder::relevance::Observer;

    fn entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).expect("entity index should be in range")
    }

    fn net_entity(value: u64) -> NetEntity {
        NetEntity(value)
    }

    #[test]
    fn one_moved_active_body_yields_one_relevant_delta() {
        let mut pool = ActivePhysicsPool::default();
        pool.begin_tick();
        let mut promotions = 0;

        // Body inside the AOI; it gets a non-zero position upsert that
        // produces a dirty row.
        let near_entity = entity(1);
        let near_handle = pool
            .__test_allocate(near_entity, &mut promotions)
            .expect("near body should allocate");
        pool.__test_upsert(near_handle, Vec3::X, Quat::IDENTITY, Vec3::Y, Vec3::ZERO);

        // Body outside the AOI; even though it moves, the bridge must
        // drop its delta.
        let far_entity = entity(2);
        let far_handle = pool
            .__test_allocate(far_entity, &mut promotions)
            .expect("far body should allocate");
        pool.__test_upsert(
            far_handle,
            Vec3::new(50.0, 0.0, 0.0),
            Quat::IDENTITY,
            Vec3::ZERO,
            Vec3::ZERO,
        );

        let dirty = pool.drain_dirty_rows();
        assert_eq!(dirty.len(), 2);

        let entity_to_net = |entity: Entity| -> Option<NetEntity> {
            if entity == near_entity {
                Some(net_entity(101))
            } else if entity == far_entity {
                Some(net_entity(202))
            } else {
                None
            }
        };
        let snapshot =
            ThunderPhysicsDeltaSnapshot::new(&pool, &dirty, NetworkTick(7), entity_to_net);
        assert_eq!(snapshot.row_count(), 2);

        let aoi = PhysicsDeltaAoi {
            observer: Observer::new([0.0, 0.0, 0.0], 5.0),
            shard_origin: None,
        };
        let mut bridge = PhysicsDeltaBridge::new();
        bridge.begin_client();
        let serialized = bridge.serialize(&snapshot, aoi, PhysicsDeltaBudget::default());
        assert_eq!(serialized, 1, "only the in-AOI body should serialize");
        assert_eq!(bridge.output()[0].entity, net_entity(101));
        let metrics = bridge.metrics();
        assert_eq!(metrics.dirty_bodies_scanned, 2);
        assert_eq!(metrics.relevant_dirty_bodies, 1);
    }

    #[test]
    fn entities_without_net_mapping_are_dropped_at_snapshot_time() {
        let mut pool = ActivePhysicsPool::default();
        pool.begin_tick();
        let mut promotions = 0;
        let entity_a = entity(10);
        let handle = pool
            .__test_allocate(entity_a, &mut promotions)
            .expect("body should allocate");
        pool.__test_upsert(handle, Vec3::X, Quat::IDENTITY, Vec3::ZERO, Vec3::ZERO);
        let dirty = pool.drain_dirty_rows();

        let snapshot = ThunderPhysicsDeltaSnapshot::new(&pool, &dirty, NetworkTick(2), |_| None);
        assert_eq!(snapshot.row_count(), 0);
    }

    #[test]
    fn dirty_mask_translation_preserves_set_bits() {
        let pool_mask = ActivePoolDirtyMask::POSITION
            .union(ActivePoolDirtyMask::ROTATION)
            .union(ActivePoolDirtyMask::LINEAR_VELOCITY)
            .union(ActivePoolDirtyMask::ANGULAR_VELOCITY);
        let thunder_mask = active_pool_to_thunder_mask(pool_mask);
        assert!(thunder_mask.contains(DirtyMask::TRANSFORM));
        assert!(thunder_mask.contains(DirtyMask::VELOCITY));
        assert!(thunder_mask.contains(DirtyMask::FREQUENT_DELTAS));
        // Sleep/lifecycle bits are not produced by the pool today.
        assert!(!thunder_mask.intersects(DirtyMask::LIFECYCLE));
    }
}
