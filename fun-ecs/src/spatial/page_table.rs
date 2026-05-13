use crate::Resource;
use std::collections::HashMap;

use crate::{
    DenseSlotMap, ECS_SPATIAL_MAX_PAGE_RECORDS, EcsPageResidencyRecord, EcsSpatialPageId,
    EcsSpatialPageKey, EcsSpatialValidationError,
};

#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct EcsPageResidencyTable {
    pub records: DenseSlotMap<EcsSpatialPageId, EcsPageResidencyRecord>,
    pub key_to_id: HashMap<EcsSpatialPageKey, EcsSpatialPageId>,
}

impl EcsPageResidencyTable {
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn push(
        &mut self,
        record: EcsPageResidencyRecord,
    ) -> Result<(), EcsSpatialValidationError> {
        let id = EcsSpatialPageId::new(self.len() as u64 + 1);
        self.insert(id, record)
    }

    pub fn insert(
        &mut self,
        id: EcsSpatialPageId,
        record: EcsPageResidencyRecord,
    ) -> Result<(), EcsSpatialValidationError> {
        if !self.is_consistent() {
            return Err(EcsSpatialValidationError::PageTableColumnMismatch);
        }
        if !self.records.contains_key(id) && self.len() >= ECS_SPATIAL_MAX_PAGE_RECORDS {
            return Err(EcsSpatialValidationError::PageTableFull);
        }
        if self
            .key_to_id
            .get(&record.key)
            .is_some_and(|existing_id| *existing_id != id)
        {
            return Err(EcsSpatialValidationError::PageTableDuplicateKey);
        }
        if let Some(old) = self.records.insert(id, record) {
            self.key_to_id.remove(&old.key);
        }
        self.key_to_id.insert(record.key, id);
        Ok(())
    }

    #[must_use]
    pub fn record(&self, index: usize) -> Option<EcsPageResidencyRecord> {
        self.records.get_index(index).map(|(_id, record)| *record)
    }

    #[must_use]
    pub fn get(&self, id: EcsSpatialPageId) -> Option<&EcsPageResidencyRecord> {
        self.records.get(id)
    }

    #[must_use]
    pub fn get_by_key(&self, key: EcsSpatialPageKey) -> Option<&EcsPageResidencyRecord> {
        self.key_to_id
            .get(&key)
            .and_then(|id| self.records.get(*id))
    }

    pub fn get_mut_by_key(
        &mut self,
        key: EcsSpatialPageKey,
    ) -> Option<&mut EcsPageResidencyRecord> {
        let id = *self.key_to_id.get(&key)?;
        self.records.get_mut(id)
    }

    pub fn mark_dirty_by_key(&mut self, key: EcsSpatialPageKey, dirty_epoch: u32) -> bool {
        if let Some(record) = self.get_mut_by_key(key) {
            record.dirty_epoch = record.dirty_epoch.max(dirty_epoch);
            true
        } else {
            false
        }
    }

    #[must_use]
    pub fn is_consistent(&self) -> bool {
        self.records.is_consistent()
            && self.key_to_id.len() == self.records.len()
            && self
                .records
                .iter()
                .all(|(id, record)| self.key_to_id.get(&record.key) == Some(&id))
    }
}

pub type EcsSpatialPageTable = EcsPageResidencyTable;
