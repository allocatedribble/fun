use crate::{FunEntity, experiments::StorageExperimentError};

pub const FUN_ECS_SPARSE_DEFAULT_PAGE_SIZE: usize = 256;
pub const FUN_ECS_SPARSE_DEFAULT_MAX_PAGES: usize = 4096;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SparseChunkKey(pub u64);

impl SparseChunkKey {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SparseDenseIndex {
    pub page_index: u32,
    pub slot_index: u32,
    pub dense_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SparsePage {
    pub chunk_key: SparseChunkKey,
    pub base_slot: u64,
    pub slots: Vec<Option<SparseDenseIndex>>,
    pub occupied: u32,
}

impl SparsePage {
    #[must_use]
    pub fn new(page_index: u32, page_size: usize) -> Self {
        Self {
            chunk_key: SparseChunkKey::new(u64::from(page_index) + 1),
            base_slot: u64::from(page_index) * page_size as u64 + 1,
            slots: vec![None; page_size],
            occupied: 0,
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.occupied == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SparsePagedPoolConfig {
    pub page_size: usize,
    pub max_pages: usize,
}

impl Default for SparsePagedPoolConfig {
    fn default() -> Self {
        Self {
            page_size: FUN_ECS_SPARSE_DEFAULT_PAGE_SIZE,
            max_pages: FUN_ECS_SPARSE_DEFAULT_MAX_PAGES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SparsePagedPool<T> {
    config: SparsePagedPoolConfig,
    pages: Vec<SparsePage>,
    dense: Vec<(FunEntity, T)>,
}

impl<T> Default for SparsePagedPool<T> {
    fn default() -> Self {
        Self::with_config(SparsePagedPoolConfig::default())
    }
}

impl<T> SparsePagedPool<T> {
    #[must_use]
    pub fn new(page_size: usize) -> Self {
        Self::with_config(SparsePagedPoolConfig {
            page_size,
            ..SparsePagedPoolConfig::default()
        })
    }

    #[must_use]
    pub fn with_config(config: SparsePagedPoolConfig) -> Self {
        Self {
            config,
            pages: Vec::new(),
            dense: Vec::new(),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.dense.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.dense.is_empty()
    }

    #[must_use]
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    #[must_use]
    pub fn pages(&self) -> &[SparsePage] {
        &self.pages
    }

    #[must_use]
    pub fn dense_values(&self) -> &[(FunEntity, T)] {
        &self.dense
    }

    pub fn insert(
        &mut self,
        entity: FunEntity,
        value: T,
    ) -> Result<Option<T>, StorageExperimentError> {
        if !entity.is_valid() {
            return Err(StorageExperimentError::InvalidEntity);
        }
        self.ensure_page_for_entity(entity)?;
        if let Some(index) = self.sparse_index(entity) {
            let old = core::mem::replace(&mut self.dense[index.dense_index as usize].1, value);
            return Ok(Some(old));
        }
        let dense_index = self.dense.len() as u32;
        let (page_index, slot_index) = self.page_slot(entity)?;
        let page = &mut self.pages[page_index as usize];
        page.slots[slot_index as usize] = Some(SparseDenseIndex {
            page_index,
            slot_index,
            dense_index,
        });
        page.occupied = page.occupied.saturating_add(1);
        self.dense.push((entity, value));
        Ok(None)
    }

    #[must_use]
    pub fn get(&self, entity: FunEntity) -> Option<&T> {
        let index = self.sparse_index(entity)?;
        self.dense
            .get(index.dense_index as usize)
            .and_then(|(stored_entity, value)| (*stored_entity == entity).then_some(value))
    }

    pub fn get_mut(&mut self, entity: FunEntity) -> Option<&mut T> {
        let index = self.sparse_index(entity)?;
        self.dense
            .get_mut(index.dense_index as usize)
            .and_then(|(stored_entity, value)| (*stored_entity == entity).then_some(value))
    }

    pub fn remove(&mut self, entity: FunEntity) -> Option<T> {
        let index = self.sparse_index(entity)?;
        if self.dense.get(index.dense_index as usize)?.0 != entity {
            return None;
        }
        let page = self.pages.get_mut(index.page_index as usize)?;
        page.slots[index.slot_index as usize] = None;
        page.occupied = page.occupied.saturating_sub(1);
        let dense_index = index.dense_index as usize;
        let (_removed_entity, removed_value) = self.dense.swap_remove(dense_index);
        if dense_index < self.dense.len() {
            let moved_entity = self.dense[dense_index].0;
            if let Ok((moved_page, moved_slot)) = self.page_slot(moved_entity)
                && let Some(moved_sparse) = self
                    .pages
                    .get_mut(moved_page as usize)
                    .and_then(|page| page.slots.get_mut(moved_slot as usize))
                    .and_then(Option::as_mut)
            {
                moved_sparse.dense_index = dense_index as u32;
            }
        }
        Some(removed_value)
    }

    pub fn iter(&self) -> impl Iterator<Item = (FunEntity, &T)> {
        self.dense.iter().map(|(entity, value)| (*entity, value))
    }

    fn sparse_index(&self, entity: FunEntity) -> Option<SparseDenseIndex> {
        let (page_index, slot_index) = self.page_slot(entity).ok()?;
        self.pages
            .get(page_index as usize)?
            .slots
            .get(slot_index as usize)
            .copied()
            .flatten()
    }

    fn ensure_page_for_entity(&mut self, entity: FunEntity) -> Result<(), StorageExperimentError> {
        let (page_index, _slot_index) = self.page_slot(entity)?;
        if page_index as usize >= self.config.max_pages {
            return Err(StorageExperimentError::SparsePoolFull);
        }
        while self.pages.len() <= page_index as usize {
            let next = self.pages.len() as u32;
            self.pages
                .push(SparsePage::new(next, self.config.page_size));
        }
        Ok(())
    }

    fn page_slot(&self, entity: FunEntity) -> Result<(u32, u32), StorageExperimentError> {
        if self.config.page_size == 0 {
            return Err(StorageExperimentError::SparsePageSizeZero);
        }
        if !entity.is_valid() {
            return Err(StorageExperimentError::InvalidEntity);
        }
        let zero_based_slot = entity.slot - 1;
        Ok((
            (zero_based_slot / self.config.page_size as u64) as u32,
            (zero_based_slot % self.config.page_size as u64) as u32,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FunEntityGeneration;

    #[test]
    fn sparse_paged_pool_inserts_updates_gets_and_removes_with_swap_fixup() {
        let mut pool = SparsePagedPool::new(2);
        let first = FunEntity::new(1, FunEntityGeneration::new(1));
        let second = FunEntity::new(2, FunEntityGeneration::new(1));
        let third = FunEntity::new(3, FunEntityGeneration::new(1));

        assert_eq!(pool.insert(first, "debug-pin").expect("insert"), None);
        assert_eq!(pool.insert(second, "annotation").expect("insert"), None);
        assert_eq!(pool.insert(third, "transient").expect("insert"), None);
        assert_eq!(pool.page_count(), 2);
        assert_eq!(pool.get(second), Some(&"annotation"));

        assert_eq!(
            pool.insert(second, "annotation-updated").expect("update"),
            Some("annotation")
        );
        assert_eq!(pool.get(second), Some(&"annotation-updated"));
        assert_eq!(pool.remove(first), Some("debug-pin"));
        assert_eq!(pool.get(first), None);
        assert_eq!(pool.get(third), Some(&"transient"));
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn sparse_paged_pool_rejects_invalid_entities_zero_pages_and_page_overflow() {
        let mut zero = SparsePagedPool::<u32>::new(0);
        assert_eq!(
            zero.insert(FunEntity::new(1, FunEntityGeneration::new(1)), 1),
            Err(StorageExperimentError::SparsePageSizeZero)
        );

        let mut bounded = SparsePagedPool::with_config(SparsePagedPoolConfig {
            page_size: 2,
            max_pages: 1,
        });
        assert_eq!(
            bounded.insert(FunEntity::INVALID, 1),
            Err(StorageExperimentError::InvalidEntity)
        );
        assert_eq!(
            bounded.insert(FunEntity::new(3, FunEntityGeneration::new(1)), 1),
            Err(StorageExperimentError::SparsePoolFull)
        );
    }
}
