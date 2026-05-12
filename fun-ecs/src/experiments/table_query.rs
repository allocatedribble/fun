use core::{marker::PhantomData, ops::Range};

use fun_scheduler_types::{EcsChunkKey, ScheduleDeadline};

use crate::{
    DenseResourcePriorityBand, DenseResourceTable, DenseResourceTableChunk, DenseResourceTableKey,
    DenseResourceTableRevision, DenseResourceTableRow,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TableChanged {
    pub since: DenseResourceTableRevision,
}

impl TableChanged {
    #[must_use]
    pub const fn since(since: DenseResourceTableRevision) -> Self {
        Self { since }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TableAdded {
    pub since: DenseResourceTableRevision,
}

impl TableAdded {
    #[must_use]
    pub const fn since(since: DenseResourceTableRevision) -> Self {
        Self { since }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TableFilter {
    All,
    Changed(TableChanged),
    Added(TableAdded),
    SpatialChunk(EcsChunkKey),
    Consumer(u16),
    ArtifactKind(u16),
    PriorityBand(DenseResourcePriorityBand),
    DeadlineClass(ScheduleDeadline),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableQueryRow<'a, K, Row> {
    pub key: K,
    pub row: &'a Row,
    pub revision: DenseResourceTableRevision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableQuery<Row> {
    filters: Vec<TableFilter>,
    limit: Option<usize>,
    _marker: PhantomData<fn() -> Row>,
}

impl<Row> Default for TableQuery<Row> {
    fn default() -> Self {
        Self {
            filters: vec![TableFilter::All],
            limit: None,
            _marker: PhantomData,
        }
    }
}

impl<Row> TableQuery<Row> {
    #[must_use]
    pub fn all() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn changed_since(since: DenseResourceTableRevision) -> Self {
        Self::default().with_filter(TableFilter::Changed(TableChanged::since(since)))
    }

    #[must_use]
    pub fn added_since(since: DenseResourceTableRevision) -> Self {
        Self::default().with_filter(TableFilter::Added(TableAdded::since(since)))
    }

    #[must_use]
    pub fn with_filter(mut self, filter: TableFilter) -> Self {
        if matches!(filter, TableFilter::All) {
            self.filters.clear();
            self.filters.push(filter);
            return self;
        }
        self.filters
            .retain(|candidate| !matches!(candidate, TableFilter::All));
        if !self.filters.contains(&filter) {
            self.filters.push(filter);
        }
        self
    }

    #[must_use]
    pub const fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    #[must_use]
    pub fn run<'a, K>(
        &self,
        table: &'a DenseResourceTable<K, Row>,
    ) -> Vec<TableQueryRow<'a, K, Row>>
    where
        K: DenseResourceTableKey,
        Row: DenseResourceTableRow,
    {
        let mut rows = Vec::new();
        for (key, row, revision) in table.iter() {
            if self.matches(row, revision) {
                rows.push(TableQueryRow { key, row, revision });
                if self.limit.is_some_and(|limit| rows.len() >= limit) {
                    break;
                }
            }
        }
        rows
    }

    fn matches(&self, row: &Row, revision: DenseResourceTableRevision) -> bool
    where
        Row: DenseResourceTableRow,
    {
        self.filters.iter().all(|filter| match *filter {
            TableFilter::All => true,
            TableFilter::Changed(changed) => revision > changed.since,
            TableFilter::Added(added) => revision > added.since,
            TableFilter::SpatialChunk(chunk) => row.spatial_chunk_key() == Some(chunk),
            TableFilter::Consumer(consumer) => row.consumer_key() == Some(consumer),
            TableFilter::ArtifactKind(kind) => row.artifact_kind_key() == Some(kind),
            TableFilter::PriorityBand(priority) => row.priority_band() == Some(priority),
            TableFilter::DeadlineClass(deadline) => row.deadline_class() == Some(deadline),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableChunkQuery {
    Range(Range<usize>),
    SpatialChunk(EcsChunkKey),
    Consumer(u16),
    ArtifactKind(u16),
    PriorityBand(DenseResourcePriorityBand),
    DeadlineClass(ScheduleDeadline),
}

impl TableChunkQuery {
    #[must_use]
    pub fn run<K, Row>(&self, table: &DenseResourceTable<K, Row>) -> DenseResourceTableChunk<K, Row>
    where
        K: DenseResourceTableKey,
        Row: DenseResourceTableRow + Clone,
    {
        match self {
            Self::Range(range) => table.chunk_by_range(range.clone()),
            Self::SpatialChunk(chunk) => table.chunk_by_spatial_chunk(*chunk),
            Self::Consumer(consumer) => table.chunk_by_consumer(*consumer),
            Self::ArtifactKind(kind) => table.chunk_by_artifact_kind(*kind),
            Self::PriorityBand(priority) => table.chunk_by_priority_band(*priority),
            Self::DeadlineClass(deadline) => table.chunk_by_deadline_class(*deadline),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EcsSpatialPageId, FunEcsResourceKind, FunResourceTableId};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct QueryRow {
        chunk: EcsChunkKey,
        consumer: u16,
        artifact_kind: u16,
        priority: DenseResourcePriorityBand,
        deadline: ScheduleDeadline,
        digest: u64,
    }

    impl DenseResourceTableRow for QueryRow {
        fn spatial_chunk_key(&self) -> Option<EcsChunkKey> {
            Some(self.chunk)
        }

        fn consumer_key(&self) -> Option<u16> {
            Some(self.consumer)
        }

        fn artifact_kind_key(&self) -> Option<u16> {
            Some(self.artifact_kind)
        }

        fn priority_band(&self) -> Option<DenseResourcePriorityBand> {
            Some(self.priority)
        }

        fn deadline_class(&self) -> Option<ScheduleDeadline> {
            Some(self.deadline)
        }

        fn stable_digest(&self) -> u64 {
            self.digest
        }
    }

    fn table() -> DenseResourceTable<EcsSpatialPageId, QueryRow> {
        let mut table = DenseResourceTable::with_capacity(
            FunEcsResourceKind::PageResidencyTable,
            FunResourceTableId::from_resource_kind(FunEcsResourceKind::PageResidencyTable),
            16,
        );
        table
            .push(
                EcsSpatialPageId::new(1),
                QueryRow {
                    chunk: EcsChunkKey::new(10),
                    consumer: 1,
                    artifact_kind: 7,
                    priority: DenseResourcePriorityBand::Critical,
                    deadline: ScheduleDeadline::Frame,
                    digest: 100,
                },
            )
            .expect("first row");
        table
            .push(
                EcsSpatialPageId::new(2),
                QueryRow {
                    chunk: EcsChunkKey::new(20),
                    consumer: 2,
                    artifact_kind: 8,
                    priority: DenseResourcePriorityBand::Background,
                    deadline: ScheduleDeadline::IdleWindow,
                    digest: 200,
                },
            )
            .expect("second row");
        table
    }

    #[test]
    fn table_query_filters_entityless_hot_rows_without_entities() {
        let table = table();

        let rows = TableQuery::<QueryRow>::all()
            .with_filter(TableFilter::Consumer(1))
            .with_filter(TableFilter::DeadlineClass(ScheduleDeadline::Frame))
            .run(&table);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].key, EcsSpatialPageId::new(1));
        assert_eq!(rows[0].row.digest, 100);
    }

    #[test]
    fn table_changed_and_added_filter_by_row_revision() {
        let mut table = table();
        let baseline = table.table_revision();
        table.update_rows(|key, row| {
            if key == EcsSpatialPageId::new(2) {
                row.digest = 250;
            }
        });

        let changed = TableQuery::<QueryRow>::changed_since(baseline).run(&table);
        let added = TableQuery::<QueryRow>::added_since(baseline).run(&table);

        assert_eq!(changed.len(), 2);
        assert_eq!(added.len(), 2);
    }

    #[test]
    fn table_chunk_query_reuses_dense_table_chunk_descriptors() {
        let table = table();

        let chunk = TableChunkQuery::SpatialChunk(EcsChunkKey::new(20)).run(&table);

        assert_eq!(chunk.rows.len(), 1);
        assert_eq!(chunk.rows[0].key, EcsSpatialPageId::new(2));
        assert_eq!(chunk.rows[0].row.digest, 200);
    }
}
