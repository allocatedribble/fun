use crate::FunEcsResourceKind;

pub const FUN_WORLD_SPATIAL_RESOURCE_KINDS: [FunEcsResourceKind; 13] = [
    FunEcsResourceKind::SpatialPageTable,
    FunEcsResourceKind::PageResidencyTable,
    FunEcsResourceKind::DirtyRegionLedger,
    FunEcsResourceKind::StreamInterestTable,
    FunEcsResourceKind::StreamWaveLedger,
    FunEcsResourceKind::StreamRequestQueue,
    FunEcsResourceKind::SourceAcquireQueue,
    FunEcsResourceKind::DecodedPageQueue,
    FunEcsResourceKind::DerivedArtifactRegistry,
    FunEcsResourceKind::NetworkHandoffQueue,
    FunEcsResourceKind::RendererHandoffQueue,
    FunEcsResourceKind::LuxHandoffQueue,
    FunEcsResourceKind::PhysicsCookQueue,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunWorldResourceSet {
    pub kinds: &'static [FunEcsResourceKind],
}

impl FunWorldResourceSet {
    pub const SPATIAL_BASELINE: Self = Self {
        kinds: &FUN_WORLD_SPATIAL_RESOURCE_KINDS,
    };

    #[must_use]
    pub const fn len(self) -> usize {
        self.kinds.len()
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.kinds.is_empty()
    }
}
