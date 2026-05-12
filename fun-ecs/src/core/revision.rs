use crate::{FunArtifactId, FunResourceId, FunResourceTableId, FunStorageChunkId};

pub const FUN_ECS_MAX_REVISION_TOUCH_ROWS: usize = 256;
pub const FUN_ECS_MAX_RESOURCE_REVISION_RECORDS: usize = 4096;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FunRevision(pub u64);

impl FunRevision {
    pub const INITIAL: Self = Self(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

macro_rules! fun_revision_wrapper {
    ($name:ident) => {
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub FunRevision);

        impl $name {
            pub const INITIAL: Self = Self(FunRevision::INITIAL);

            #[must_use]
            pub const fn new(value: u64) -> Self {
                Self(FunRevision::new(value))
            }

            #[must_use]
            pub const fn from_revision(revision: FunRevision) -> Self {
                Self(revision)
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

        impl From<FunRevision> for $name {
            fn from(value: FunRevision) -> Self {
                Self::from_revision(value)
            }
        }

        impl From<$name> for FunRevision {
            fn from(value: $name) -> Self {
                value.revision()
            }
        }
    };
}

fun_revision_wrapper!(ResourceTableRevision);
fun_revision_wrapper!(ArtifactRevision);
fun_revision_wrapper!(SpatialPageRevision);
fun_revision_wrapper!(ExternalSlabRevision);
fun_revision_wrapper!(FrameRevision);
fun_revision_wrapper!(ScheduleRevision);
fun_revision_wrapper!(HandoffRevision);

pub type FunWorldRevision = FunRevision;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RevisionCategory {
    Structure = 0,
    Data = 1,
    Spatial = 2,
    Artifact = 3,
    Handoff = 4,
    Frame = 5,
    Schedule = 6,
    ExternalSlab = 7,
    #[default]
    Unknown = 255,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceRevisionRecord {
    pub resource: FunResourceId,
    pub table: FunResourceTableId,
    pub chunk: FunStorageChunkId,
    pub table_revision: ResourceTableRevision,
    pub data_revision: FunRevision,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ResourceRevisionLedger {
    pub rows: Vec<ResourceRevisionRecord>,
}

impl ResourceRevisionLedger {
    pub fn touch(
        &mut self,
        resource: FunResourceId,
        table: FunResourceTableId,
        chunk: FunStorageChunkId,
        revision: FunRevision,
    ) -> Result<(), FunRevisionLedgerError> {
        if !resource.is_valid() || !table.is_valid() {
            return Err(FunRevisionLedgerError::InvalidResourceTouch);
        }
        if let Some(row) = self
            .rows
            .iter_mut()
            .find(|row| row.resource == resource && row.table == table && row.chunk == chunk)
        {
            row.table_revision = ResourceTableRevision::from_revision(revision);
            row.data_revision = revision;
            return Ok(());
        }
        if self.rows.len() >= FUN_ECS_MAX_RESOURCE_REVISION_RECORDS {
            return Err(FunRevisionLedgerError::ResourceRevisionLedgerFull);
        }
        self.rows.push(ResourceRevisionRecord {
            resource,
            table,
            chunk,
            table_revision: ResourceTableRevision::from_revision(revision),
            data_revision: revision,
        });
        self.rows
            .sort_by_key(|row| (row.resource, row.table, row.chunk));
        Ok(())
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WorldRevisionLedger {
    pub structure_revision: FunRevision,
    pub data_revision: FunRevision,
    pub spatial_revision: SpatialPageRevision,
    pub artifact_revision: ArtifactRevision,
    pub handoff_revision: HandoffRevision,
    pub frame_revision: FrameRevision,
    pub schedule_revision: ScheduleRevision,
    pub external_slab_revision: ExternalSlabRevision,
    pub resource_revisions: ResourceRevisionLedger,
}

impl WorldRevisionLedger {
    #[must_use]
    pub fn at_revision(revision: FunRevision) -> Self {
        Self {
            structure_revision: revision,
            data_revision: revision,
            spatial_revision: SpatialPageRevision::from_revision(revision),
            artifact_revision: ArtifactRevision::from_revision(revision),
            handoff_revision: HandoffRevision::from_revision(revision),
            frame_revision: FrameRevision::from_revision(revision),
            schedule_revision: ScheduleRevision::from_revision(revision),
            external_slab_revision: ExternalSlabRevision::from_revision(revision),
            resource_revisions: ResourceRevisionLedger::default(),
        }
    }

    #[must_use]
    pub const fn category_revision(&self, category: RevisionCategory) -> FunRevision {
        match category {
            RevisionCategory::Structure => self.structure_revision,
            RevisionCategory::Data => self.data_revision,
            RevisionCategory::Spatial => self.spatial_revision.revision(),
            RevisionCategory::Artifact => self.artifact_revision.revision(),
            RevisionCategory::Handoff => self.handoff_revision.revision(),
            RevisionCategory::Frame => self.frame_revision.revision(),
            RevisionCategory::Schedule => self.schedule_revision.revision(),
            RevisionCategory::ExternalSlab => self.external_slab_revision.revision(),
            RevisionCategory::Unknown => FunRevision::INITIAL,
        }
    }

    pub fn advance_category(&mut self, category: RevisionCategory) -> (FunRevision, FunRevision) {
        let previous = self.category_revision(category);
        let new = previous.next();
        match category {
            RevisionCategory::Structure => self.structure_revision = new,
            RevisionCategory::Data => self.data_revision = new,
            RevisionCategory::Spatial => self.spatial_revision = SpatialPageRevision::from(new),
            RevisionCategory::Artifact => self.artifact_revision = ArtifactRevision::from(new),
            RevisionCategory::Handoff => self.handoff_revision = HandoffRevision::from(new),
            RevisionCategory::Frame => self.frame_revision = FrameRevision::from(new),
            RevisionCategory::Schedule => self.schedule_revision = ScheduleRevision::from(new),
            RevisionCategory::ExternalSlab => {
                self.external_slab_revision = ExternalSlabRevision::from(new);
            }
            RevisionCategory::Unknown => {}
        }
        (previous, new)
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FunCommandApplyRevisionReport {
    pub observed_revision: FunRevision,
    pub previous_revision: FunRevision,
    pub new_revision: FunRevision,
    pub touched_resources: Vec<FunResourceId>,
    pub touched_tables: Vec<FunResourceTableId>,
    pub touched_artifact_keys: Vec<FunArtifactId>,
}

impl FunCommandApplyRevisionReport {
    #[must_use]
    pub fn new(observed_revision: FunRevision, previous_revision: FunRevision) -> Self {
        Self {
            observed_revision,
            previous_revision,
            new_revision: previous_revision,
            touched_resources: Vec::new(),
            touched_tables: Vec::new(),
            touched_artifact_keys: Vec::new(),
        }
    }

    pub fn touch_resource(&mut self, resource: FunResourceId) {
        if resource.is_valid() && !self.touched_resources.contains(&resource) {
            self.touched_resources.push(resource);
            self.touched_resources.sort_unstable();
        }
    }

    pub fn touch_table(&mut self, table: FunResourceTableId) {
        if table.is_valid() && !self.touched_tables.contains(&table) {
            self.touched_tables.push(table);
            self.touched_tables.sort_unstable();
        }
    }

    pub fn touch_artifact(&mut self, artifact: FunArtifactId) {
        if artifact.is_valid() && !self.touched_artifact_keys.contains(&artifact) {
            self.touched_artifact_keys.push(artifact);
            self.touched_artifact_keys.sort_unstable();
        }
    }

    pub fn finish(&mut self, new_revision: FunRevision) {
        self.new_revision = new_revision;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FunRevisionLedgerError {
    InvalidResourceTouch = 0,
    ResourceRevisionLedgerFull = 1,
}

#[cfg(test)]
mod tests {
    use crate::FunEcsResourceKind;

    use super::*;

    #[test]
    fn world_revision_ledger_advances_categories_independently() {
        let mut ledger = WorldRevisionLedger::default();

        let (previous, new) = ledger.advance_category(RevisionCategory::Structure);
        assert_eq!(previous, FunRevision::INITIAL);
        assert_eq!(new, FunRevision::new(1));
        assert_eq!(ledger.structure_revision, FunRevision::new(1));
        assert_eq!(ledger.artifact_revision, ArtifactRevision::INITIAL);
    }

    #[test]
    fn resource_revision_ledger_records_table_revisions() {
        let mut ledger = ResourceRevisionLedger::default();
        let resource = FunResourceId::from_resource_kind(FunEcsResourceKind::RendererHandoffQueue);
        let table =
            FunResourceTableId::from_resource_kind(FunEcsResourceKind::RendererHandoffQueue);

        ledger
            .touch(
                resource,
                table,
                FunStorageChunkId::new(5),
                FunRevision::new(9),
            )
            .expect("touch resource");
        assert_eq!(ledger.rows.len(), 1);
        assert_eq!(ledger.rows[0].table_revision, ResourceTableRevision::new(9));
    }
}
