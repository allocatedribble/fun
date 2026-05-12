use crate::{FunEcsComponentKind, FunEcsResourceKind};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FunQueryId(pub u32);

impl FunQueryId {
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FunQueryAccess {
    #[default]
    Read = 0,
    Write = 1,
}

impl FunQueryAccess {
    #[must_use]
    pub const fn conflicts_with(self, other: Self) -> bool {
        matches!((self, other), (Self::Write, _) | (_, Self::Write))
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunResourceAccess {
    pub resource: FunEcsResourceKind,
    pub access: FunQueryAccess,
}

impl FunResourceAccess {
    #[must_use]
    pub const fn read(resource: FunEcsResourceKind) -> Self {
        Self {
            resource,
            access: FunQueryAccess::Read,
        }
    }

    #[must_use]
    pub const fn write(resource: FunEcsResourceKind) -> Self {
        Self {
            resource,
            access: FunQueryAccess::Write,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunQueryMetadata {
    pub id: FunQueryId,
    pub label: &'static str,
    pub component_reads: Vec<FunEcsComponentKind>,
    pub component_writes: Vec<FunEcsComponentKind>,
    pub resource_access: Vec<FunResourceAccess>,
}

impl FunQueryMetadata {
    #[must_use]
    pub fn new(id: FunQueryId, label: &'static str) -> Self {
        Self {
            id,
            label,
            component_reads: Vec::new(),
            component_writes: Vec::new(),
            resource_access: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_component_read(mut self, component: FunEcsComponentKind) -> Self {
        self.component_reads.push(component);
        self
    }

    #[must_use]
    pub fn with_component_write(mut self, component: FunEcsComponentKind) -> Self {
        self.component_writes.push(component);
        self
    }

    #[must_use]
    pub fn with_resource_access(mut self, access: FunResourceAccess) -> Self {
        self.resource_access.push(access);
        self
    }

    pub fn validate(&self) -> Result<(), FunQueryValidationError> {
        if self.label.is_empty() {
            return Err(FunQueryValidationError::EmptyLabel);
        }
        if self
            .component_writes
            .iter()
            .any(|written| self.component_reads.contains(written))
        {
            return Err(FunQueryValidationError::ReadWriteComponentOverlap);
        }
        Ok(())
    }

    #[must_use]
    pub fn conflicts_with(&self, other: &Self) -> bool {
        self.component_writes.iter().any(|written| {
            other.component_reads.contains(written) || other.component_writes.contains(written)
        }) || other
            .component_writes
            .iter()
            .any(|written| self.component_reads.contains(written))
            || self.resource_access.iter().any(|left| {
                other.resource_access.iter().any(|right| {
                    left.resource == right.resource && left.access.conflicts_with(right.access)
                })
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FunQueryValidationError {
    EmptyLabel = 0,
    ReadWriteComponentOverlap = 1,
}

#[cfg(test)]
mod tests {
    use crate::{FunEcsComponentKind, FunEcsResourceKind};

    use super::*;

    #[test]
    fn query_metadata_extracts_conflicting_resource_access() {
        let reader = FunQueryMetadata::new(FunQueryId::new(1), "reader")
            .with_component_read(FunEcsComponentKind::StreamCamera)
            .with_resource_access(FunResourceAccess::read(
                FunEcsResourceKind::PageResidencyTable,
            ));
        let writer = FunQueryMetadata::new(FunQueryId::new(2), "writer")
            .with_component_write(FunEcsComponentKind::StreamCamera)
            .with_resource_access(FunResourceAccess::write(
                FunEcsResourceKind::PageResidencyTable,
            ));

        assert_eq!(reader.validate(), Ok(()));
        assert!(reader.conflicts_with(&writer));
    }
}
