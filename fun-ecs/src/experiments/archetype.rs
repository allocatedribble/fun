use fun_scheduler_types::EcsChunkKey;

use crate::{
    FunArchetypeId, FunComponentId, FunEntity, FunRevision, FunStorageChunkId,
    experiments::StorageExperimentError,
};

pub const FUN_ECS_ARCHETYPE_DEFAULT_CHUNK_CAPACITY: usize = 128;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkRevision(pub FunRevision);

impl ChunkRevision {
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
    pub const fn next(self) -> Self {
        Self(self.0.next())
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkDirtyMask {
    pub bits: u64,
}

impl ChunkDirtyMask {
    pub const NONE: Self = Self { bits: 0 };

    #[must_use]
    pub const fn mark_component_index(mut self, component_index: u16) -> Self {
        if component_index < 64 {
            self.bits |= 1_u64 << component_index;
        }
        self
    }

    #[must_use]
    pub const fn contains_component_index(self, component_index: u16) -> bool {
        component_index < 64 && (self.bits & (1_u64 << component_index)) != 0
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentColumn {
    pub component: FunComponentId,
    pub column_index: u16,
    pub chunk_row_counts: Vec<u32>,
    pub revision: ChunkRevision,
}

impl ComponentColumn {
    #[must_use]
    pub fn new(component: FunComponentId, column_index: u16) -> Self {
        Self {
            component,
            column_index,
            chunk_row_counts: Vec::new(),
            revision: ChunkRevision::INITIAL,
        }
    }

    fn push_chunk(&mut self) {
        self.chunk_row_counts.push(0);
        self.revision = self.revision.next();
    }

    fn push_row(&mut self, chunk_index: usize) {
        if let Some(rows) = self.chunk_row_counts.get_mut(chunk_index) {
            *rows = rows.saturating_add(1);
            self.revision = self.revision.next();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Archetype {
    pub id: FunArchetypeId,
    pub components: Vec<FunComponentId>,
    pub chunk_capacity: usize,
}

impl Archetype {
    #[must_use]
    pub fn new(id: FunArchetypeId, mut components: Vec<FunComponentId>) -> Self {
        components.retain(|component| component.is_valid());
        components.sort_unstable();
        components.dedup();
        Self {
            id,
            components,
            chunk_capacity: FUN_ECS_ARCHETYPE_DEFAULT_CHUNK_CAPACITY,
        }
    }

    #[must_use]
    pub const fn with_chunk_capacity(mut self, chunk_capacity: usize) -> Self {
        self.chunk_capacity = chunk_capacity;
        self
    }

    #[must_use]
    pub fn component_index(&self, component: FunComponentId) -> Option<u16> {
        self.components
            .binary_search(&component)
            .ok()
            .map(|index| index as u16)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchetypeChunk {
    pub key: EcsChunkKey,
    pub storage_chunk: FunStorageChunkId,
    pub entities: Vec<FunEntity>,
    pub dirty_mask: ChunkDirtyMask,
    pub revision: ChunkRevision,
}

impl ArchetypeChunk {
    #[must_use]
    pub fn new(archetype: FunArchetypeId, chunk_index: u32) -> Self {
        let storage_chunk = FunStorageChunkId::new(chunk_index as u64 + 1);
        Self {
            key: archetype_chunk_key(archetype, chunk_index),
            storage_chunk,
            entities: Vec::new(),
            dirty_mask: ChunkDirtyMask::NONE,
            revision: ChunkRevision::INITIAL,
        }
    }

    #[must_use]
    pub fn is_full(&self, chunk_capacity: usize) -> bool {
        self.entities.len() >= chunk_capacity
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityLocation {
    pub archetype: FunArchetypeId,
    pub chunk_key: EcsChunkKey,
    pub chunk_index: u32,
    pub row_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchetypeTable {
    pub archetype: Archetype,
    pub columns: Vec<ComponentColumn>,
    pub chunks: Vec<ArchetypeChunk>,
    entity_locations: Vec<(FunEntity, EntityLocation)>,
    pub revision: ChunkRevision,
}

impl ArchetypeTable {
    #[must_use]
    pub fn new(archetype: Archetype) -> Self {
        let columns = archetype
            .components
            .iter()
            .copied()
            .enumerate()
            .map(|(index, component)| ComponentColumn::new(component, index as u16))
            .collect();
        Self {
            archetype,
            columns,
            chunks: Vec::new(),
            entity_locations: Vec::new(),
            revision: ChunkRevision::INITIAL,
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entity_locations.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entity_locations.is_empty()
    }

    #[must_use]
    pub fn location(&self, entity: FunEntity) -> Option<EntityLocation> {
        self.entity_locations
            .iter()
            .find_map(|(candidate, location)| (*candidate == entity).then_some(*location))
    }

    pub fn insert_entity(
        &mut self,
        entity: FunEntity,
    ) -> Result<EntityLocation, StorageExperimentError> {
        if !entity.is_valid() {
            return Err(StorageExperimentError::InvalidEntity);
        }
        if self.location(entity).is_some() {
            return Err(StorageExperimentError::DuplicateEntity);
        }
        if self
            .chunks
            .last()
            .is_none_or(|chunk| chunk.is_full(self.archetype.chunk_capacity.max(1)))
        {
            self.push_chunk();
        }
        let chunk_index = self.chunks.len() - 1;
        let chunk = &mut self.chunks[chunk_index];
        let row_index = chunk.entities.len();
        chunk.entities.push(entity);
        chunk.revision = chunk.revision.next();
        for column in &mut self.columns {
            column.push_row(chunk_index);
        }
        self.revision = self.revision.next();
        let location = EntityLocation {
            archetype: self.archetype.id,
            chunk_key: chunk.key,
            chunk_index: chunk_index as u32,
            row_index: row_index as u32,
        };
        self.entity_locations.push((entity, location));
        self.entity_locations
            .sort_by_key(|(entity, _location)| entity.scheduler_bits());
        Ok(location)
    }

    pub fn mark_component_dirty(
        &mut self,
        entity: FunEntity,
        component: FunComponentId,
    ) -> Result<ChunkDirtyMask, StorageExperimentError> {
        let component_index = self
            .archetype
            .component_index(component)
            .ok_or(StorageExperimentError::InvalidComponent)?;
        let location = self
            .location(entity)
            .ok_or(StorageExperimentError::MissingEntity)?;
        let chunk = self
            .chunks
            .get_mut(location.chunk_index as usize)
            .ok_or(StorageExperimentError::MissingEntity)?;
        chunk.dirty_mask = chunk.dirty_mask.mark_component_index(component_index);
        chunk.revision = chunk.revision.next();
        if let Some(column) = self.columns.get_mut(component_index as usize) {
            column.revision = column.revision.next();
        }
        self.revision = self.revision.next();
        Ok(chunk.dirty_mask)
    }

    fn push_chunk(&mut self) {
        let chunk_index = self.chunks.len() as u32;
        self.chunks
            .push(ArchetypeChunk::new(self.archetype.id, chunk_index));
        for column in &mut self.columns {
            column.push_chunk();
        }
    }
}

#[must_use]
pub fn archetype_chunk_key(archetype: FunArchetypeId, chunk_index: u32) -> EcsChunkKey {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    hash = fnv1a_u64(hash, archetype.get());
    hash = fnv1a_u64(hash, u64::from(chunk_index));
    EcsChunkKey::new(hash.max(1))
}

const fn fnv1a_u64(mut hash: u64, value: u64) -> u64 {
    hash = fnv1a_u8(hash, (value & 0xff) as u8);
    hash = fnv1a_u8(hash, ((value >> 8) & 0xff) as u8);
    hash = fnv1a_u8(hash, ((value >> 16) & 0xff) as u8);
    hash = fnv1a_u8(hash, ((value >> 24) & 0xff) as u8);
    hash = fnv1a_u8(hash, ((value >> 32) & 0xff) as u8);
    hash = fnv1a_u8(hash, ((value >> 40) & 0xff) as u8);
    hash = fnv1a_u8(hash, ((value >> 48) & 0xff) as u8);
    fnv1a_u8(hash, ((value >> 56) & 0xff) as u8)
}

const fn fnv1a_u8(hash: u64, value: u8) -> u64 {
    (hash ^ value as u64).wrapping_mul(0x0000_0100_0000_01b3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FunEntityGeneration;

    #[test]
    fn archetype_table_chunks_entities_and_marks_component_dirty() {
        let position = FunComponentId::new(10);
        let velocity = FunComponentId::new(11);
        let archetype = Archetype::new(
            FunArchetypeId::new(4),
            vec![velocity, position, position, FunComponentId::INVALID],
        )
        .with_chunk_capacity(2);
        let mut table = ArchetypeTable::new(archetype);

        let first = table
            .insert_entity(FunEntity::new(1, FunEntityGeneration::new(1)))
            .expect("insert first entity");
        let second = table
            .insert_entity(FunEntity::new(2, FunEntityGeneration::new(1)))
            .expect("insert second entity");
        let third = table
            .insert_entity(FunEntity::new(3, FunEntityGeneration::new(1)))
            .expect("insert third entity");

        assert_eq!(first.chunk_index, second.chunk_index);
        assert_ne!(second.chunk_index, third.chunk_index);
        assert_eq!(table.chunks.len(), 2);
        assert_eq!(table.columns.len(), 2);
        assert_eq!(table.columns[0].chunk_row_counts, vec![2, 1]);

        let dirty = table
            .mark_component_dirty(FunEntity::new(1, FunEntityGeneration::new(1)), position)
            .expect("mark dirty");

        assert!(dirty.contains_component_index(0));
        assert!(!dirty.contains_component_index(1));
        assert!(table.revision.get() > ChunkRevision::INITIAL.get());
    }

    #[test]
    fn archetype_table_rejects_invalid_duplicate_and_unknown_component() {
        let component = FunComponentId::new(1);
        let mut table =
            ArchetypeTable::new(Archetype::new(FunArchetypeId::new(1), vec![component]));
        let entity = FunEntity::new(7, FunEntityGeneration::new(1));

        assert_eq!(
            table.insert_entity(FunEntity::INVALID),
            Err(StorageExperimentError::InvalidEntity)
        );
        table.insert_entity(entity).expect("insert entity");
        assert_eq!(
            table.insert_entity(entity),
            Err(StorageExperimentError::DuplicateEntity)
        );
        assert_eq!(
            table.mark_component_dirty(entity, FunComponentId::new(99)),
            Err(StorageExperimentError::InvalidComponent)
        );
    }
}
