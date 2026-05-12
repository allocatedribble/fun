use crate::{
    DenseResourceTable, DenseResourceTableDigest, DenseResourceTableKey,
    DenseResourceTableRevision, DenseResourceTableRow, FunFrameId, FunResourceTableId, FunRevision,
    experiments::StorageExperimentError,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SnapshotGeneration(pub u64);

impl SnapshotGeneration {
    pub const INVALID: Self = Self(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }

    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SnapshotConsumer {
    #[default]
    Renderer = 0,
    Thunder = 1,
    Rvelte = 2,
    Diagnostics = 3,
}

impl SnapshotConsumer {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Renderer => "renderer",
            Self::Thunder => "thunder",
            Self::Rvelte => "rvelte",
            Self::Diagnostics => "diagnostics",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceTableSnapshot {
    pub table: FunResourceTableId,
    pub table_revision: DenseResourceTableRevision,
    pub row_count: u32,
    pub digest: DenseResourceTableDigest,
    pub snapshot_generation: SnapshotGeneration,
}

impl ResourceTableSnapshot {
    #[must_use]
    pub fn from_table<K, Row>(
        table_id: FunResourceTableId,
        table: &DenseResourceTable<K, Row>,
        snapshot_generation: SnapshotGeneration,
    ) -> Self
    where
        K: DenseResourceTableKey,
        Row: DenseResourceTableRow,
    {
        Self {
            table: table_id,
            table_revision: table.table_revision(),
            row_count: table.len() as u32,
            digest: table.digest(),
            snapshot_generation,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SnapshotLease {
    pub generation: SnapshotGeneration,
    pub consumer: SnapshotConsumer,
    pub acquired_frame: FunFrameId,
    pub min_retained_generation: SnapshotGeneration,
}

impl SnapshotLease {
    #[must_use]
    pub const fn new(
        generation: SnapshotGeneration,
        consumer: SnapshotConsumer,
        acquired_frame: FunFrameId,
    ) -> Self {
        Self {
            generation,
            consumer,
            acquired_frame,
            min_retained_generation: generation,
        }
    }

    #[must_use]
    pub const fn protects(self, generation: SnapshotGeneration) -> bool {
        generation.get() >= self.min_retained_generation.get()
            && generation.get() <= self.generation.get()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldSnapshot {
    pub generation: SnapshotGeneration,
    pub frame: FunFrameId,
    pub world_revision: FunRevision,
    pub table_snapshots: Vec<ResourceTableSnapshot>,
}

impl WorldSnapshot {
    pub fn new(
        generation: SnapshotGeneration,
        frame: FunFrameId,
        world_revision: FunRevision,
    ) -> Result<Self, StorageExperimentError> {
        if !generation.is_valid() {
            return Err(StorageExperimentError::InvalidSnapshotGeneration);
        }
        Ok(Self {
            generation,
            frame,
            world_revision,
            table_snapshots: Vec::new(),
        })
    }

    #[must_use]
    pub fn with_table_snapshot(mut self, table_snapshot: ResourceTableSnapshot) -> Self {
        self.table_snapshots.push(table_snapshot);
        self.table_snapshots
            .sort_by_key(|snapshot| snapshot.table.get());
        self
    }

    #[must_use]
    pub fn table_snapshot(&self, table: FunResourceTableId) -> Option<ResourceTableSnapshot> {
        self.table_snapshots
            .iter()
            .copied()
            .find(|snapshot| snapshot.table == table)
    }

    #[must_use]
    pub const fn lease(&self, consumer: SnapshotConsumer) -> SnapshotLease {
        SnapshotLease::new(self.generation, consumer, self.frame)
    }

    #[must_use]
    pub fn digest(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        hash = snapshot_hash_u64(hash, self.generation.get());
        hash = snapshot_hash_u64(hash, self.frame.get());
        hash = snapshot_hash_u64(hash, self.world_revision.get());
        for snapshot in &self.table_snapshots {
            hash = snapshot_hash_u64(hash, snapshot.table.get());
            hash = snapshot_hash_u64(hash, snapshot.table_revision.get());
            hash = snapshot_hash_u64(hash, u64::from(snapshot.row_count));
            hash = snapshot_hash_u64(hash, snapshot.digest.value);
        }
        hash
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SnapshotRetireQueue {
    pending: Vec<SnapshotGeneration>,
}

impl SnapshotRetireQueue {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enqueue(&mut self, generation: SnapshotGeneration) {
        if generation.is_valid() && !self.pending.contains(&generation) {
            self.pending.push(generation);
            self.pending.sort_unstable();
        }
    }

    #[must_use]
    pub fn pending(&self) -> &[SnapshotGeneration] {
        &self.pending
    }

    pub fn retire_unleased(&mut self, leases: &[SnapshotLease]) -> Vec<SnapshotGeneration> {
        let mut retired = Vec::new();
        self.pending.retain(|generation| {
            let protected = leases.iter().any(|lease| lease.protects(*generation));
            if protected {
                true
            } else {
                retired.push(*generation);
                false
            }
        });
        retired
    }
}

const fn snapshot_hash_u8(hash: u64, value: u8) -> u64 {
    (hash ^ value as u64).wrapping_mul(0x0000_0100_0000_01b3)
}

const fn snapshot_hash_u64(mut hash: u64, value: u64) -> u64 {
    hash = snapshot_hash_u8(hash, (value & 0xff) as u8);
    hash = snapshot_hash_u8(hash, ((value >> 8) & 0xff) as u8);
    hash = snapshot_hash_u8(hash, ((value >> 16) & 0xff) as u8);
    hash = snapshot_hash_u8(hash, ((value >> 24) & 0xff) as u8);
    hash = snapshot_hash_u8(hash, ((value >> 32) & 0xff) as u8);
    hash = snapshot_hash_u8(hash, ((value >> 40) & 0xff) as u8);
    hash = snapshot_hash_u8(hash, ((value >> 48) & 0xff) as u8);
    snapshot_hash_u8(hash, ((value >> 56) & 0xff) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fun_scheduler_types::{EcsChunkKey, ScheduleDeadline};

    use crate::{
        DenseResourcePriorityBand, EcsSpatialPageId, FunEcsResourceKind, FunResourceTableId,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct SnapshotRow {
        chunk: EcsChunkKey,
        digest: u64,
    }

    impl DenseResourceTableRow for SnapshotRow {
        fn spatial_chunk_key(&self) -> Option<EcsChunkKey> {
            Some(self.chunk)
        }

        fn priority_band(&self) -> Option<DenseResourcePriorityBand> {
            Some(DenseResourcePriorityBand::Normal)
        }

        fn deadline_class(&self) -> Option<ScheduleDeadline> {
            Some(ScheduleDeadline::Frame)
        }

        fn stable_digest(&self) -> u64 {
            self.digest
        }
    }

    fn table_id() -> FunResourceTableId {
        FunResourceTableId::from_resource_kind(FunEcsResourceKind::PageResidencyTable)
    }

    fn table() -> DenseResourceTable<EcsSpatialPageId, SnapshotRow> {
        let mut table = DenseResourceTable::with_capacity(
            FunEcsResourceKind::PageResidencyTable,
            table_id(),
            16,
        );
        table
            .push(
                EcsSpatialPageId::new(1),
                SnapshotRow {
                    chunk: EcsChunkKey::new(1),
                    digest: 10,
                },
            )
            .expect("insert row");
        table
    }

    #[test]
    fn renderer_reads_snapshot_generation_while_ecs_mutates_next_generation() {
        let mut table = table();
        let generation = SnapshotGeneration::new(7);
        let snapshot = WorldSnapshot::new(generation, FunFrameId::new(100), FunRevision::new(20))
            .expect("snapshot")
            .with_table_snapshot(ResourceTableSnapshot::from_table(
                table_id(),
                &table,
                generation,
            ));
        let renderer_lease = snapshot.lease(SnapshotConsumer::Renderer);
        let stable_table = snapshot
            .table_snapshot(table_id())
            .expect("table snapshot exists");

        table.update_rows(|_key, row| row.digest = 99);
        let next_snapshot =
            ResourceTableSnapshot::from_table(table_id(), &table, generation.next());

        assert_eq!(renderer_lease.generation, generation);
        assert_ne!(stable_table.digest, next_snapshot.digest);
        assert_eq!(stable_table.snapshot_generation, generation);
        assert_eq!(next_snapshot.snapshot_generation, generation.next());
    }

    #[test]
    fn thunder_rvelte_and_diagnostics_share_consistent_snapshot_state() {
        let table = table();
        let generation = SnapshotGeneration::new(8);
        let snapshot = WorldSnapshot::new(generation, FunFrameId::new(101), FunRevision::new(21))
            .expect("snapshot")
            .with_table_snapshot(ResourceTableSnapshot::from_table(
                table_id(),
                &table,
                generation,
            ));

        let thunder = snapshot.lease(SnapshotConsumer::Thunder);
        let rvelte = snapshot.lease(SnapshotConsumer::Rvelte);
        let diagnostics = snapshot.lease(SnapshotConsumer::Diagnostics);

        assert_eq!(thunder.generation, generation);
        assert_eq!(rvelte.generation, generation);
        assert_eq!(diagnostics.generation, generation);
        assert_eq!(
            snapshot
                .table_snapshot(table_id())
                .expect("table")
                .table_revision,
            table.table_revision()
        );
        assert_ne!(snapshot.digest(), 0);
    }

    #[test]
    fn snapshot_retire_queue_keeps_leased_generations_and_retires_old_ones() {
        let mut queue = SnapshotRetireQueue::new();
        let old = SnapshotGeneration::new(2);
        let live = SnapshotGeneration::new(3);
        queue.enqueue(old);
        queue.enqueue(live);
        queue.enqueue(live);

        let retired = queue.retire_unleased(&[SnapshotLease::new(
            live,
            SnapshotConsumer::Renderer,
            FunFrameId::new(44),
        )]);

        assert_eq!(retired, vec![old]);
        assert_eq!(queue.pending(), &[live]);
    }

    #[test]
    fn world_snapshot_rejects_invalid_generation() {
        assert_eq!(
            WorldSnapshot::new(
                SnapshotGeneration::INVALID,
                FunFrameId::new(1),
                FunRevision::new(1),
            ),
            Err(StorageExperimentError::InvalidSnapshotGeneration)
        );
    }
}
