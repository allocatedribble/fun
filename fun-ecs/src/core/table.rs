use core::ops::Range;

use fun_scheduler_types::{EcsChunkKey, ScheduleDeadline, WorkRequiredness};

use crate::{
    EcsArtifactConsumer, EcsDerivedArtifactId, EcsDerivedArtifactRecord, EcsPageResidencyRecord,
    EcsSpatialPageId, EcsSpatialValidationError, EcsStreamInterestKind, EcsStreamPriority,
    FunArchetypeId, FunArtifactId, FunCommandBufferId, FunComponentId, FunEcsResourceChunk,
    FunEcsResourceKind, FunEcsRevision, FunEntity, FunExternalSlabId, FunFrameId, FunResourceId,
    FunResourceTableId, FunRevision, FunStorageChunkId, FunSystemId, FunSystemSetId,
};

pub const DENSE_RESOURCE_TABLE_BENCHMARK_ROW_COUNT: usize = 1_000_000;

const DENSE_TABLE_HASH_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const DENSE_TABLE_HASH_PRIME: u64 = 0x0000_0100_0000_01b3;

pub trait DenseResourceTableKey: Copy + Eq + Ord {
    #[must_use]
    fn is_valid(self) -> bool;

    #[must_use]
    fn stable_key(self) -> u64;
}

macro_rules! dense_resource_key {
    ($name:ty) => {
        impl DenseResourceTableKey for $name {
            fn is_valid(self) -> bool {
                self.is_valid()
            }

            fn stable_key(self) -> u64 {
                self.get() as u64
            }
        }
    };
}

dense_resource_key!(EcsSpatialPageId);
dense_resource_key!(EcsDerivedArtifactId);
dense_resource_key!(FunComponentId);
dense_resource_key!(FunResourceId);
dense_resource_key!(FunResourceTableId);
dense_resource_key!(FunSystemId);
dense_resource_key!(FunSystemSetId);
dense_resource_key!(FunCommandBufferId);
dense_resource_key!(FunArchetypeId);
dense_resource_key!(FunStorageChunkId);
dense_resource_key!(FunExternalSlabId);
dense_resource_key!(FunArtifactId);
dense_resource_key!(FunFrameId);

impl DenseResourceTableKey for FunEntity {
    fn is_valid(self) -> bool {
        self.is_valid()
    }

    fn stable_key(self) -> u64 {
        self.scheduler_bits()
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DenseResourceTableRevision(pub FunRevision);

impl DenseResourceTableRevision {
    pub const INITIAL: Self = Self(FunRevision::INITIAL);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(FunRevision::new(value))
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }

    #[must_use]
    pub const fn revision(self) -> FunRevision {
        self.0
    }

    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.next())
    }
}

impl From<FunRevision> for DenseResourceTableRevision {
    fn from(value: FunRevision) -> Self {
        Self(value)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ResourceTableLayout {
    #[default]
    AoS = 0,
    SoA = 1,
    HybridHotCold = 2,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HotFieldMask {
    pub bits: u64,
}

impl HotFieldMask {
    pub const NONE: Self = Self { bits: 0 };
    pub const STATE: Self = Self { bits: 1 << 0 };
    pub const PRIORITY: Self = Self { bits: 1 << 1 };
    pub const DIRTY: Self = Self { bits: 1 << 2 };
    pub const ARTIFACT_KIND: Self = Self { bits: 1 << 3 };
    pub const CONSUMER: Self = Self { bits: 1 << 4 };
    pub const DEADLINE: Self = Self { bits: 1 << 5 };

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.bits & other.bits) == other.bits
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DenseResourcePriorityBand {
    Critical = 0,
    High = 1,
    #[default]
    Normal = 2,
    Background = 3,
    Diagnostic = 4,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DenseResourceTableAccessKind {
    #[default]
    ContiguousRange = 0,
    SpatialChunk = 1,
    Consumer = 2,
    ArtifactKind = 3,
    PriorityBand = 4,
    DeadlineClass = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DenseResourceTableConfig {
    pub resource: FunEcsResourceKind,
    pub table: FunResourceTableId,
    pub capacity: usize,
    pub layout: ResourceTableLayout,
    pub reverse_domain_index: bool,
}

impl DenseResourceTableConfig {
    #[must_use]
    pub const fn new(
        resource: FunEcsResourceKind,
        table: FunResourceTableId,
        capacity: usize,
    ) -> Self {
        Self {
            resource,
            table,
            capacity,
            layout: ResourceTableLayout::AoS,
            reverse_domain_index: false,
        }
    }

    #[must_use]
    pub const fn with_layout(mut self, layout: ResourceTableLayout) -> Self {
        self.layout = layout;
        self
    }

    #[must_use]
    pub const fn with_reverse_domain_index(mut self, enabled: bool) -> Self {
        self.reverse_domain_index = enabled;
        self
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DenseResourceTableStats {
    pub len: usize,
    pub capacity: usize,
    pub indexed_rows: usize,
    pub reverse_domain_index_rows: usize,
    pub table_revision: DenseResourceTableRevision,
    pub layout: ResourceTableLayout,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DenseResourceTableDigest {
    pub value: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DenseResourceTableAccess {
    pub resource: FunEcsResourceKind,
    pub table: FunResourceTableId,
    pub kind: DenseResourceTableAccessKind,
    pub qualifier: u64,
    pub scheduler_chunk: EcsChunkKey,
    pub first_row: u32,
    pub row_count: u32,
    pub revision: DenseResourceTableRevision,
}

impl DenseResourceTableAccess {
    #[must_use]
    pub const fn to_control_resource_chunk(self) -> FunEcsResourceChunk {
        FunEcsResourceChunk::new(
            self.resource,
            self.scheduler_chunk,
            self.first_row,
            self.row_count,
            FunEcsRevision(self.revision.get()),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ColumnarRowView<K> {
    pub key: K,
    pub row_index: u32,
    pub hot_fields: HotFieldMask,
    pub revision: DenseResourceTableRevision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DenseResourceTableChunkRow<K, Row> {
    pub key: K,
    pub row: Row,
    pub revision: DenseResourceTableRevision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DenseResourceTableChunk<K, Row> {
    pub access: DenseResourceTableAccess,
    pub rows: Vec<DenseResourceTableChunkRow<K, Row>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DenseResourceTableIndex<K> {
    row_positions: Vec<(K, usize)>,
    domain_keys: Vec<(u64, K)>,
    reverse_domain_index: bool,
}

impl<K> Default for DenseResourceTableIndex<K> {
    fn default() -> Self {
        Self {
            row_positions: Vec::new(),
            domain_keys: Vec::new(),
            reverse_domain_index: false,
        }
    }
}

impl<K> DenseResourceTableIndex<K>
where
    K: DenseResourceTableKey,
{
    #[must_use]
    pub fn reverse_domain_index_enabled(&self) -> bool {
        self.reverse_domain_index
    }

    #[must_use]
    pub fn indexed_rows(&self) -> usize {
        self.row_positions.len()
    }

    #[must_use]
    pub fn reverse_domain_index_rows(&self) -> usize {
        self.domain_keys.len()
    }

    #[must_use]
    pub fn row_position(&self, key: K) -> Option<usize> {
        self.row_positions
            .binary_search_by_key(&key, |(candidate, _index)| *candidate)
            .ok()
            .map(|index| self.row_positions[index].1)
    }

    #[must_use]
    pub fn keys_for_domain(&self, domain_key: u64) -> Vec<K> {
        self.domain_keys
            .iter()
            .filter_map(|(candidate, key)| (*candidate == domain_key).then_some(*key))
            .collect()
    }

    fn enable_reverse_domain_index(&mut self) {
        self.reverse_domain_index = true;
    }

    fn insert_row_position(&mut self, key: K, row_index: usize) {
        match self
            .row_positions
            .binary_search_by_key(&key, |(candidate, _index)| *candidate)
        {
            Ok(index) => self.row_positions[index] = (key, row_index),
            Err(index) if index == self.row_positions.len() => {
                self.row_positions.push((key, row_index));
            }
            Err(index) => self.row_positions.insert(index, (key, row_index)),
        }
    }

    fn insert_domain_key(&mut self, domain_key: Option<u64>, key: K) {
        if !self.reverse_domain_index {
            return;
        }
        if let Some(domain_key) = domain_key {
            let entry = (domain_key, key);
            match self.domain_keys.binary_search(&entry) {
                Ok(_) => {}
                Err(index) if index == self.domain_keys.len() => self.domain_keys.push(entry),
                Err(index) => self.domain_keys.insert(index, entry),
            }
        }
    }

    fn remove_domain_key(&mut self, domain_key: Option<u64>, key: K) {
        if !self.reverse_domain_index {
            return;
        }
        if let Some(domain_key) = domain_key
            && let Ok(index) = self.domain_keys.binary_search(&(domain_key, key))
        {
            self.domain_keys.remove(index);
        }
    }
}

pub trait DenseResourceTableRow {
    #[must_use]
    fn domain_key(&self) -> Option<u64> {
        None
    }

    #[must_use]
    fn spatial_chunk_key(&self) -> Option<EcsChunkKey> {
        None
    }

    #[must_use]
    fn consumer_key(&self) -> Option<u16> {
        None
    }

    #[must_use]
    fn artifact_kind_key(&self) -> Option<u16> {
        None
    }

    #[must_use]
    fn priority_band(&self) -> Option<DenseResourcePriorityBand> {
        None
    }

    #[must_use]
    fn deadline_class(&self) -> Option<ScheduleDeadline> {
        None
    }

    #[must_use]
    fn stable_digest(&self) -> u64 {
        0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DenseResourceTable<K, Row> {
    config: DenseResourceTableConfig,
    keys: Vec<K>,
    rows: Vec<Row>,
    row_revisions: Vec<DenseResourceTableRevision>,
    table_revision: DenseResourceTableRevision,
    index: DenseResourceTableIndex<K>,
}

impl<K, Row> DenseResourceTable<K, Row>
where
    K: DenseResourceTableKey,
{
    #[must_use]
    pub fn new(config: DenseResourceTableConfig) -> Self {
        let mut index = DenseResourceTableIndex::default();
        if config.reverse_domain_index {
            index.enable_reverse_domain_index();
        }
        Self {
            config,
            keys: Vec::with_capacity(config.capacity.min(4096)),
            rows: Vec::with_capacity(config.capacity.min(4096)),
            row_revisions: Vec::with_capacity(config.capacity.min(4096)),
            table_revision: DenseResourceTableRevision::INITIAL,
            index,
        }
    }

    #[must_use]
    pub fn with_capacity(
        resource: FunEcsResourceKind,
        table: FunResourceTableId,
        capacity: usize,
    ) -> Self {
        Self::new(DenseResourceTableConfig::new(resource, table, capacity))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    #[must_use]
    pub const fn layout(&self) -> ResourceTableLayout {
        self.config.layout
    }

    #[must_use]
    pub const fn table_revision(&self) -> DenseResourceTableRevision {
        self.table_revision
    }

    #[must_use]
    pub fn index(&self) -> &DenseResourceTableIndex<K> {
        &self.index
    }

    #[must_use]
    pub fn stats(&self) -> DenseResourceTableStats {
        DenseResourceTableStats {
            len: self.len(),
            capacity: self.config.capacity,
            indexed_rows: self.index.indexed_rows(),
            reverse_domain_index_rows: self.index.reverse_domain_index_rows(),
            table_revision: self.table_revision,
            layout: self.config.layout,
        }
    }

    #[must_use]
    pub fn get(&self, key: K) -> Option<&Row> {
        self.index
            .row_position(key)
            .and_then(|index| self.rows.get(index))
    }

    pub fn get_mut(&mut self, key: K) -> Option<&mut Row> {
        let index = self.index.row_position(key)?;
        self.rows.get_mut(index)
    }

    pub fn iter(&self) -> impl Iterator<Item = (K, &Row, DenseResourceTableRevision)> {
        self.keys
            .iter()
            .copied()
            .zip(self.rows.iter())
            .zip(self.row_revisions.iter().copied())
            .map(|((key, row), revision)| (key, row, revision))
    }

    #[must_use]
    pub fn columnar_views(&self, hot_fields: HotFieldMask) -> Vec<ColumnarRowView<K>> {
        self.keys
            .iter()
            .copied()
            .enumerate()
            .map(|(row_index, key)| ColumnarRowView {
                key,
                row_index: row_index as u32,
                hot_fields,
                revision: self.row_revisions[row_index],
            })
            .collect()
    }

    pub fn update_rows<F>(&mut self, mut update: F)
    where
        F: FnMut(K, &mut Row),
    {
        self.advance_table_revision();
        for index in 0..self.rows.len() {
            update(self.keys[index], &mut self.rows[index]);
            self.row_revisions[index] = self.table_revision;
        }
    }

    fn advance_table_revision(&mut self) {
        self.table_revision = self.table_revision.next();
    }
}

impl<K, Row> DenseResourceTable<K, Row>
where
    K: DenseResourceTableKey,
    Row: DenseResourceTableRow,
{
    pub fn push(&mut self, key: K, row: Row) -> Result<(), EcsSpatialValidationError> {
        if !key.is_valid() {
            return Err(EcsSpatialValidationError::InvalidTableKey);
        }
        if self.rows.len() >= self.config.capacity {
            return Err(EcsSpatialValidationError::ResourceTableFull);
        }
        if self.index.row_position(key).is_some() {
            return Err(EcsSpatialValidationError::DuplicateTableKey);
        }
        self.advance_table_revision();
        let row_index = self.rows.len();
        let domain_key = row.domain_key();
        self.keys.push(key);
        self.rows.push(row);
        self.row_revisions.push(self.table_revision);
        self.index.insert_row_position(key, row_index);
        self.index.insert_domain_key(domain_key, key);
        Ok(())
    }

    pub fn upsert(&mut self, key: K, row: Row) -> Result<(), EcsSpatialValidationError> {
        if !key.is_valid() {
            return Err(EcsSpatialValidationError::InvalidTableKey);
        }
        if let Some(row_index) = self.index.row_position(key) {
            let old_domain_key = self.rows[row_index].domain_key();
            let new_domain_key = row.domain_key();
            self.advance_table_revision();
            self.index.remove_domain_key(old_domain_key, key);
            self.rows[row_index] = row;
            self.row_revisions[row_index] = self.table_revision;
            self.index.insert_domain_key(new_domain_key, key);
            return Ok(());
        }
        self.push(key, row)
    }

    #[must_use]
    pub fn digest(&self) -> DenseResourceTableDigest {
        let mut value = fnv1a_u64(DENSE_TABLE_HASH_OFFSET, self.len() as u64);
        value = fnv1a_u64(value, self.table_revision.get());
        for (key, row, revision) in self.iter() {
            value = fnv1a_u64(value, key.stable_key());
            value = fnv1a_u64(value, row.stable_digest());
            value = fnv1a_u64(value, revision.get());
        }
        DenseResourceTableDigest { value }
    }

    #[must_use]
    pub fn chunk_by_range(&self, range: Range<usize>) -> DenseResourceTableChunk<K, Row>
    where
        Row: Clone,
    {
        let start = range.start.min(self.len());
        let end = range.end.min(self.len()).max(start);
        self.chunk_from_indices(
            DenseResourceTableAccessKind::ContiguousRange,
            start as u64,
            (start..end).collect(),
            start,
        )
    }

    #[must_use]
    pub fn chunk_by_spatial_chunk(&self, chunk_key: EcsChunkKey) -> DenseResourceTableChunk<K, Row>
    where
        Row: Clone,
    {
        self.chunk_by_predicate(
            DenseResourceTableAccessKind::SpatialChunk,
            chunk_key.get(),
            chunk_key,
            |row| row.spatial_chunk_key() == Some(chunk_key),
        )
    }

    #[must_use]
    pub fn chunk_by_consumer(&self, consumer: u16) -> DenseResourceTableChunk<K, Row>
    where
        Row: Clone,
    {
        self.chunk_by_predicate(
            DenseResourceTableAccessKind::Consumer,
            u64::from(consumer),
            EcsChunkKey::new(u64::from(consumer).max(1)),
            |row| row.consumer_key() == Some(consumer),
        )
    }

    #[must_use]
    pub fn chunk_by_artifact_kind(&self, artifact_kind: u16) -> DenseResourceTableChunk<K, Row>
    where
        Row: Clone,
    {
        self.chunk_by_predicate(
            DenseResourceTableAccessKind::ArtifactKind,
            u64::from(artifact_kind),
            EcsChunkKey::new(u64::from(artifact_kind).max(1)),
            |row| row.artifact_kind_key() == Some(artifact_kind),
        )
    }

    #[must_use]
    pub fn chunk_by_priority_band(
        &self,
        priority_band: DenseResourcePriorityBand,
    ) -> DenseResourceTableChunk<K, Row>
    where
        Row: Clone,
    {
        self.chunk_by_predicate(
            DenseResourceTableAccessKind::PriorityBand,
            priority_band as u64,
            EcsChunkKey::new(priority_band as u64 + 1),
            |row| row.priority_band() == Some(priority_band),
        )
    }

    #[must_use]
    pub fn chunk_by_deadline_class(
        &self,
        deadline: ScheduleDeadline,
    ) -> DenseResourceTableChunk<K, Row>
    where
        Row: Clone,
    {
        self.chunk_by_predicate(
            DenseResourceTableAccessKind::DeadlineClass,
            deadline as u64,
            EcsChunkKey::new(deadline as u64 + 1),
            |row| row.deadline_class() == Some(deadline),
        )
    }

    #[must_use]
    pub fn keys_for_domain_key(&self, domain_key: u64) -> Vec<K> {
        self.index.keys_for_domain(domain_key)
    }

    fn chunk_by_predicate<F>(
        &self,
        kind: DenseResourceTableAccessKind,
        qualifier: u64,
        scheduler_chunk: EcsChunkKey,
        mut predicate: F,
    ) -> DenseResourceTableChunk<K, Row>
    where
        Row: Clone,
        F: FnMut(&Row) -> bool,
    {
        let indices = self
            .rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| predicate(row).then_some(index))
            .collect();
        self.chunk_from_indices(kind, qualifier, indices, scheduler_chunk.get() as usize)
    }

    fn chunk_from_indices(
        &self,
        kind: DenseResourceTableAccessKind,
        qualifier: u64,
        indices: Vec<usize>,
        first_row_hint: usize,
    ) -> DenseResourceTableChunk<K, Row>
    where
        Row: Clone,
    {
        let first_row = indices.first().copied().unwrap_or(first_row_hint) as u32;
        let rows = indices
            .iter()
            .map(|index| DenseResourceTableChunkRow {
                key: self.keys[*index],
                row: self.rows[*index].clone(),
                revision: self.row_revisions[*index],
            })
            .collect::<Vec<_>>();
        DenseResourceTableChunk {
            access: DenseResourceTableAccess {
                resource: self.config.resource,
                table: self.config.table,
                kind,
                qualifier,
                scheduler_chunk: EcsChunkKey::new(qualifier.max(1)),
                first_row,
                row_count: rows.len() as u32,
                revision: self.table_revision,
            },
            rows,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColdPayloadStore<K, Payload> {
    capacity: usize,
    rows: Vec<(K, Payload)>,
}

impl<K, Payload> ColdPayloadStore<K, Payload>
where
    K: DenseResourceTableKey,
{
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            rows: Vec::new(),
        }
    }

    pub fn push(&mut self, key: K, payload: Payload) -> Result<(), EcsSpatialValidationError> {
        if !key.is_valid() {
            return Err(EcsSpatialValidationError::InvalidTableKey);
        }
        if self.rows.len() >= self.capacity {
            return Err(EcsSpatialValidationError::ResourceTableFull);
        }
        self.rows.push((key, payload));
        Ok(())
    }

    #[must_use]
    pub fn get(&self, key: K) -> Option<&Payload> {
        self.rows
            .iter()
            .find(|(candidate, _payload)| *candidate == key)
            .map(|(_key, payload)| payload)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ResourceTableUseCase {
    SmallCommandQueue = 0,
    LowRowCountLedger = 1,
    Diagnostics = 2,
    PageResidencyStateScan = 3,
    PrioritySorting = 4,
    StreamingShellScan = 5,
    ArtifactReadinessScan = 6,
    #[default]
    Generic = 255,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TableLayoutAdvice {
    pub layout: ResourceTableLayout,
    pub hot_fields: HotFieldMask,
}

pub struct TableLayoutAdvisor;

impl TableLayoutAdvisor {
    #[must_use]
    pub const fn advise(
        use_case: ResourceTableUseCase,
        stats: DenseResourceTableStats,
    ) -> TableLayoutAdvice {
        match use_case {
            ResourceTableUseCase::SmallCommandQueue
            | ResourceTableUseCase::LowRowCountLedger
            | ResourceTableUseCase::Diagnostics => TableLayoutAdvice {
                layout: ResourceTableLayout::AoS,
                hot_fields: HotFieldMask::NONE,
            },
            ResourceTableUseCase::PageResidencyStateScan
            | ResourceTableUseCase::PrioritySorting
            | ResourceTableUseCase::StreamingShellScan
                if stats.len >= 65_536 =>
            {
                TableLayoutAdvice {
                    layout: ResourceTableLayout::SoA,
                    hot_fields: HotFieldMask::STATE
                        .union(HotFieldMask::PRIORITY)
                        .union(HotFieldMask::DIRTY),
                }
            }
            ResourceTableUseCase::ArtifactReadinessScan if stats.len >= 65_536 => {
                TableLayoutAdvice {
                    layout: ResourceTableLayout::HybridHotCold,
                    hot_fields: HotFieldMask::ARTIFACT_KIND
                        .union(HotFieldMask::CONSUMER)
                        .union(HotFieldMask::DEADLINE),
                }
            }
            _ => TableLayoutAdvice {
                layout: stats.layout,
                hot_fields: HotFieldMask::NONE,
            },
        }
    }
}

impl DenseResourceTableRow for EcsPageResidencyRecord {
    fn domain_key(&self) -> Option<u64> {
        Some(self.key.chunk_key().get())
    }

    fn spatial_chunk_key(&self) -> Option<EcsChunkKey> {
        Some(self.key.chunk_key())
    }

    fn priority_band(&self) -> Option<DenseResourcePriorityBand> {
        Some(priority_band_for_stream_priority(self.priority))
    }

    fn deadline_class(&self) -> Option<ScheduleDeadline> {
        Some(match self.priority.criticality {
            EcsStreamInterestKind::CameraContainingPage
            | EcsStreamInterestKind::CollisionCriticalNear => ScheduleDeadline::Frame,
            EcsStreamInterestKind::PhysicsCook => ScheduleDeadline::FixedStep,
            EcsStreamInterestKind::Diagnostics => ScheduleDeadline::IdleWindow,
            _ => ScheduleDeadline::Stream,
        })
    }

    fn stable_digest(&self) -> u64 {
        let mut hash = fnv1a_u64(DENSE_TABLE_HASH_OFFSET, self.key.chunk_key().get());
        hash = fnv1a_u8(hash, self.state as u8);
        hash = fnv1a_u16(hash, self.priority.shell);
        hash = fnv1a_u8(hash, self.priority.level);
        hash = fnv1a_u32(hash, self.dirty_epoch);
        hash = fnv1a_u32(hash, self.artifact_epoch);
        hash
    }
}

impl DenseResourceTableRow for EcsDerivedArtifactRecord {
    fn domain_key(&self) -> Option<u64> {
        Some(self.source_page.chunk_key().get())
    }

    fn spatial_chunk_key(&self) -> Option<EcsChunkKey> {
        Some(self.source_page.chunk_key())
    }

    fn consumer_key(&self) -> Option<u16> {
        Some(self.consumer as u16)
    }

    fn artifact_kind_key(&self) -> Option<u16> {
        Some(self.kind as u16)
    }

    fn priority_band(&self) -> Option<DenseResourcePriorityBand> {
        Some(if self.requiredness.is_optional() {
            DenseResourcePriorityBand::Background
        } else {
            DenseResourcePriorityBand::High
        })
    }

    fn deadline_class(&self) -> Option<ScheduleDeadline> {
        Some(match self.consumer {
            EcsArtifactConsumer::Renderer => ScheduleDeadline::Present,
            EcsArtifactConsumer::AvisPhysics => ScheduleDeadline::FixedStep,
            EcsArtifactConsumer::ThunderNetwork => ScheduleDeadline::Stream,
            EcsArtifactConsumer::Lux if self.requiredness.is_optional() => {
                ScheduleDeadline::IdleWindow
            }
            EcsArtifactConsumer::Lux => ScheduleDeadline::Frame,
            _ => ScheduleDeadline::None,
        })
    }

    fn stable_digest(&self) -> u64 {
        let mut hash = fnv1a_u64(DENSE_TABLE_HASH_OFFSET, self.artifact_id.get());
        hash = fnv1a_u64(hash, self.source_page.chunk_key().get());
        hash = fnv1a_u8(hash, self.kind as u8);
        hash = fnv1a_u8(hash, self.state as u8);
        hash = fnv1a_u8(hash, self.consumer as u8);
        hash = fnv1a_u8(hash, requiredness_code(self.requiredness));
        hash = fnv1a_u32(hash, self.source_epoch);
        hash = fnv1a_u32(hash, self.artifact_epoch);
        hash
    }
}

fn priority_band_for_stream_priority(priority: EcsStreamPriority) -> DenseResourcePriorityBand {
    match priority.criticality {
        EcsStreamInterestKind::CameraContainingPage
        | EcsStreamInterestKind::CollisionCriticalNear => DenseResourcePriorityBand::Critical,
        EcsStreamInterestKind::VisibleNear
        | EcsStreamInterestKind::VisibleCoarseFallback
        | EcsStreamInterestKind::VelocityLookahead
        | EcsStreamInterestKind::ShadowCritical
        | EcsStreamInterestKind::PhysicsCook => DenseResourcePriorityBand::High,
        EcsStreamInterestKind::Diagnostics => DenseResourcePriorityBand::Diagnostic,
        EcsStreamInterestKind::Backfill | EcsStreamInterestKind::FoliageNear => {
            DenseResourcePriorityBand::Background
        }
    }
}

fn requiredness_code(requiredness: WorkRequiredness) -> u8 {
    if requiredness.is_optional() { 1 } else { 0 }
}

const fn fnv1a_u8(hash: u64, value: u8) -> u64 {
    (hash ^ value as u64).wrapping_mul(DENSE_TABLE_HASH_PRIME)
}

const fn fnv1a_u16(hash: u64, value: u16) -> u64 {
    let hash = fnv1a_u8(hash, (value & 0xff) as u8);
    fnv1a_u8(hash, (value >> 8) as u8)
}

const fn fnv1a_u32(hash: u64, value: u32) -> u64 {
    let hash = fnv1a_u16(hash, (value & 0xffff) as u16);
    fnv1a_u16(hash, (value >> 16) as u16)
}

const fn fnv1a_u64(hash: u64, value: u64) -> u64 {
    let hash = fnv1a_u32(hash, (value & 0xffff_ffff) as u32);
    fnv1a_u32(hash, (value >> 32) as u32)
}

#[cfg(test)]
mod tests {
    use fun_scheduler_types::EcsSpatialDomainKind;

    use crate::{
        DenseSlotMap, EcsArtifactState, EcsDerivedArtifactKind, EcsPageChannel,
        EcsPageResidencyState, EcsSpatialGridId, EcsSpatialPageKey, EcsStreamInterestKind,
        EcsStreamPriority,
    };

    use super::*;

    const SMOKE_ROWS: usize = 4096;

    #[test]
    fn dense_resource_table_preserves_stable_keys_and_deterministic_iteration() {
        let mut table = page_table(16, true);
        for index in 0..16 {
            let (key, row) = page_row(index);
            table.push(key, row).expect("push row");
        }

        let keys = table
            .iter()
            .map(|(key, _row, _revision)| key.get())
            .collect::<Vec<_>>();
        assert_eq!(keys, (1..=16).collect::<Vec<_>>());
        assert_eq!(table.stats().len, 16);
        assert_eq!(table.stats().reverse_domain_index_rows, 16);
        assert_eq!(
            table.keys_for_domain_key(page_row(3).1.key.chunk_key().get()),
            vec![EcsSpatialPageId::new(4)]
        );
    }

    #[test]
    fn dense_resource_chunks_cover_range_spatial_consumer_kind_priority_and_deadline() {
        let mut table = artifact_table(64);
        for index in 0..64 {
            let (key, row) = artifact_row(index);
            table.push(key, row).expect("push artifact");
        }

        let range = table.chunk_by_range(4..12);
        assert_eq!(range.rows.len(), 8);
        assert_eq!(
            range.access.kind,
            DenseResourceTableAccessKind::ContiguousRange
        );
        assert_eq!(
            range.access.to_control_resource_chunk().row_count,
            range.rows.len() as u32
        );

        let page_chunk = artifact_row(8).1.source_page.chunk_key();
        let spatial = table.chunk_by_spatial_chunk(page_chunk);
        assert_eq!(spatial.rows.len(), 1);

        let renderer = table.chunk_by_consumer(EcsArtifactConsumer::Renderer as u16);
        assert!(!renderer.rows.is_empty());
        assert!(
            renderer
                .rows
                .iter()
                .all(|row| row.row.consumer == EcsArtifactConsumer::Renderer)
        );

        let surface =
            table.chunk_by_artifact_kind(EcsDerivedArtifactKind::TerrainSurfacePackets as u16);
        assert!(
            surface
                .rows
                .iter()
                .all(|row| row.row.kind == EcsDerivedArtifactKind::TerrainSurfacePackets)
        );

        let high = table.chunk_by_priority_band(DenseResourcePriorityBand::High);
        assert!(
            high.rows
                .iter()
                .all(|row| !row.row.requiredness.is_optional())
        );

        let present = table.chunk_by_deadline_class(ScheduleDeadline::Present);
        assert!(
            present
                .rows
                .iter()
                .all(|row| row.row.consumer == EcsArtifactConsumer::Renderer)
        );
    }

    #[test]
    fn layout_advisor_selects_aos_soa_and_hybrid_hot_cold() {
        let small = DenseResourceTableStats {
            len: 128,
            layout: ResourceTableLayout::AoS,
            ..DenseResourceTableStats::default()
        };
        assert_eq!(
            TableLayoutAdvisor::advise(ResourceTableUseCase::SmallCommandQueue, small).layout,
            ResourceTableLayout::AoS
        );

        let large = DenseResourceTableStats {
            len: 1_000_000,
            layout: ResourceTableLayout::AoS,
            ..DenseResourceTableStats::default()
        };
        let page_advice =
            TableLayoutAdvisor::advise(ResourceTableUseCase::PageResidencyStateScan, large);
        assert_eq!(page_advice.layout, ResourceTableLayout::SoA);
        assert!(page_advice.hot_fields.contains(HotFieldMask::STATE));
        assert!(page_advice.hot_fields.contains(HotFieldMask::PRIORITY));

        let artifact_advice =
            TableLayoutAdvisor::advise(ResourceTableUseCase::ArtifactReadinessScan, large);
        assert_eq!(artifact_advice.layout, ResourceTableLayout::HybridHotCold);
        assert!(artifact_advice.hot_fields.contains(HotFieldMask::CONSUMER));
    }

    #[test]
    fn generic_page_table_matches_current_dense_page_scan_digest() {
        let mut current = DenseSlotMap::<EcsSpatialPageId, EcsPageResidencyRecord>::default();
        let mut generic = page_table(SMOKE_ROWS, false);
        for index in 0..SMOKE_ROWS {
            let (key, row) = page_row(index);
            current.insert(key, row);
            generic.push(key, row).expect("push generic page row");
        }

        let current_report = scan_page_rows(current.values());
        let generic_report = scan_page_rows(generic.iter().map(|(_key, row, _revision)| row));

        assert_eq!(current_report, generic_report);
    }

    #[test]
    fn generic_artifact_table_matches_current_registry_scan_digest() {
        let mut current = DenseSlotMap::<EcsDerivedArtifactId, EcsDerivedArtifactRecord>::default();
        let mut generic = artifact_table(SMOKE_ROWS);
        for index in 0..SMOKE_ROWS {
            let (key, row) = artifact_row(index);
            current.insert(key, row);
            generic.push(key, row).expect("push generic artifact row");
        }

        let current_report = scan_artifact_rows(current.values());
        let generic_report = scan_artifact_rows(generic.iter().map(|(_key, row, _revision)| row));

        assert_eq!(current_report, generic_report);
    }

    #[test]
    #[ignore = "1M acceptance benchmark harness; run explicitly when measuring table promotion"]
    fn dense_resource_table_million_page_residency_acceptance_benchmark() {
        let rows = DENSE_RESOURCE_TABLE_BENCHMARK_ROW_COUNT;
        let mut current = Vec::with_capacity(rows);
        let mut generic = page_table(rows, false);
        for index in 0..rows {
            let (key, row) = page_row(index);
            current.push(row);
            generic.push(key, row).expect("push 1M page row");
        }

        let current_report = scan_page_rows(current.iter());
        generic.update_rows(|_key, row| {
            row.priority.starvation_age_q = row.priority.starvation_age_q.saturating_add(1);
            row.dirty_epoch = row.dirty_epoch.saturating_add(1);
        });
        let generic_report = scan_page_rows(generic.iter().map(|(_key, row, _revision)| row));

        assert_eq!(current_report.rows, generic_report.rows);
        assert_ne!(generic.digest().value, 0);
    }

    #[test]
    #[ignore = "1M acceptance benchmark harness; run explicitly when measuring table promotion"]
    fn dense_resource_table_million_artifact_acceptance_benchmark() {
        let rows = DENSE_RESOURCE_TABLE_BENCHMARK_ROW_COUNT;
        let mut current = Vec::with_capacity(rows);
        let mut generic = artifact_table(rows);
        for index in 0..rows {
            let (key, row) = artifact_row(index);
            current.push(row);
            generic.push(key, row).expect("push 1M artifact row");
        }

        let current_report = scan_artifact_rows(current.iter());
        let generic_report = scan_artifact_rows(generic.iter().map(|(_key, row, _revision)| row));

        assert_eq!(current_report, generic_report);
        assert_ne!(generic.digest().value, 0);
    }

    fn page_table(
        capacity: usize,
        reverse_domain_index: bool,
    ) -> DenseResourceTable<EcsSpatialPageId, EcsPageResidencyRecord> {
        DenseResourceTable::new(
            DenseResourceTableConfig::new(
                FunEcsResourceKind::PageResidencyTable,
                FunResourceTableId::from_resource_kind(FunEcsResourceKind::PageResidencyTable),
                capacity,
            )
            .with_layout(ResourceTableLayout::SoA)
            .with_reverse_domain_index(reverse_domain_index),
        )
    }

    fn artifact_table(
        capacity: usize,
    ) -> DenseResourceTable<EcsDerivedArtifactId, EcsDerivedArtifactRecord> {
        DenseResourceTable::new(
            DenseResourceTableConfig::new(
                FunEcsResourceKind::DerivedArtifactRegistry,
                FunResourceTableId::from_resource_kind(FunEcsResourceKind::DerivedArtifactRegistry),
                capacity,
            )
            .with_layout(ResourceTableLayout::HybridHotCold),
        )
    }

    fn page_row(index: usize) -> (EcsSpatialPageId, EcsPageResidencyRecord) {
        let page = EcsSpatialPageKey::new(
            EcsSpatialDomainKind::Terrain,
            EcsSpatialGridId::new(1),
            (index % 4) as u8,
            index as i32,
            (index / 128) as i32,
            (index / 16_384) as i32,
            EcsPageChannel::Occupancy,
        );
        let criticality = match index % 5 {
            0 => EcsStreamInterestKind::CameraContainingPage,
            1 => EcsStreamInterestKind::VisibleNear,
            2 => EcsStreamInterestKind::PhysicsCook,
            3 => EcsStreamInterestKind::Diagnostics,
            _ => EcsStreamInterestKind::Backfill,
        };
        let mut record = EcsPageResidencyRecord::new(
            page,
            EcsStreamPriority::new(criticality, (index % 32) as u16, page.level),
            index as u32,
        );
        record.state = match index % 4 {
            0 => EcsPageResidencyState::Requested,
            1 => EcsPageResidencyState::CpuDecoded,
            2 => EcsPageResidencyState::FullyReady,
            _ => EcsPageResidencyState::DerivedBuilding,
        };
        record.dirty_epoch = (index % 1024) as u32;
        (EcsSpatialPageId::new(index as u64 + 1), record)
    }

    fn artifact_row(index: usize) -> (EcsDerivedArtifactId, EcsDerivedArtifactRecord) {
        let artifact_id = EcsDerivedArtifactId::new(index as u64 + 1);
        let page = EcsSpatialPageKey::new(
            EcsSpatialDomainKind::Terrain,
            EcsSpatialGridId::new(1),
            (index % 3) as u8,
            index as i32,
            (index / 64) as i32,
            0,
            EcsPageChannel::Surface,
        );
        let (kind, consumer, requiredness) = match index % 6 {
            0 => (
                EcsDerivedArtifactKind::TerrainSurfacePackets,
                EcsArtifactConsumer::Renderer,
                WorkRequiredness::Required,
            ),
            1 => (
                EcsDerivedArtifactKind::FoliageTrunkBranchVirtualGeometry,
                EcsArtifactConsumer::Renderer,
                WorkRequiredness::Optional,
            ),
            2 => (
                EcsDerivedArtifactKind::ShadowInvalidationRows,
                EcsArtifactConsumer::Lux,
                WorkRequiredness::Optional,
            ),
            3 => (
                EcsDerivedArtifactKind::TerrainSdf,
                EcsArtifactConsumer::Lux,
                WorkRequiredness::Required,
            ),
            4 => (
                EcsDerivedArtifactKind::PhysicsCookRequests,
                EcsArtifactConsumer::AvisPhysics,
                WorkRequiredness::Required,
            ),
            _ => (
                EcsDerivedArtifactKind::NetworkRelevanceRows,
                EcsArtifactConsumer::ThunderNetwork,
                WorkRequiredness::Required,
            ),
        };
        (
            artifact_id,
            EcsDerivedArtifactRecord {
                artifact_id,
                source_page: page,
                kind,
                source_epoch: index as u32,
                artifact_epoch: index as u32 + 1,
                state: if index.is_multiple_of(7) {
                    EcsArtifactState::Building
                } else {
                    EcsArtifactState::Ready
                },
                requiredness,
                consumer,
            },
        )
    }

    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
    struct PageScanReport {
        rows: u32,
        requested: u32,
        ready: u32,
        dirty: u32,
        request_diff_candidates: u32,
        digest: u64,
    }

    fn scan_page_rows<'a>(
        rows: impl IntoIterator<Item = &'a EcsPageResidencyRecord>,
    ) -> PageScanReport {
        let mut report = PageScanReport::default();
        let mut digest = DENSE_TABLE_HASH_OFFSET;
        for row in rows {
            report.rows += 1;
            if row.state == EcsPageResidencyState::Requested {
                report.requested += 1;
            }
            if row.state.is_external_resident() {
                report.ready += 1;
            }
            if row.dirty_epoch != 0 {
                report.dirty += 1;
            }
            if !row.state.is_external_resident()
                && row.priority.criticality != EcsStreamInterestKind::Diagnostics
            {
                report.request_diff_candidates += 1;
            }
            digest = fnv1a_u64(digest, row.stable_digest());
        }
        report.digest = digest;
        report
    }

    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
    struct ArtifactScanReport {
        rows: u32,
        ready_renderer: u32,
        optional_lux: u32,
        physics_cooks: u32,
        digest: u64,
    }

    fn scan_artifact_rows<'a>(
        rows: impl IntoIterator<Item = &'a EcsDerivedArtifactRecord>,
    ) -> ArtifactScanReport {
        let mut report = ArtifactScanReport::default();
        let mut digest = DENSE_TABLE_HASH_OFFSET;
        for row in rows {
            report.rows += 1;
            if row.state == EcsArtifactState::Ready
                && row.consumer == EcsArtifactConsumer::Renderer
                && !row.requiredness.is_optional()
            {
                report.ready_renderer += 1;
            }
            if row.consumer == EcsArtifactConsumer::Lux && row.requiredness.is_optional() {
                report.optional_lux += 1;
            }
            if row.consumer == EcsArtifactConsumer::AvisPhysics {
                report.physics_cooks += 1;
            }
            digest = fnv1a_u64(digest, row.stable_digest());
        }
        report.digest = digest;
        report
    }
}
