use crate::{EcsSpatialPageKey, FunEntity, FunRevision, experiments::StorageExperimentError};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MaterializedGroupKind {
    #[default]
    TransformRenderableVisibility = 0,
    TransformPhysicsReplication = 1,
    StreamPageResidencyArtifactStatus = 2,
    UiMountRvelteRendererPacket = 3,
    AiActorPerceptionNavigationIntent = 4,
}

impl MaterializedGroupKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::TransformRenderableVisibility => "transform_renderable_visibility",
            Self::TransformPhysicsReplication => "transform_physics_replication",
            Self::StreamPageResidencyArtifactStatus => "stream_page_residency_artifact_status",
            Self::UiMountRvelteRendererPacket => "ui_mount_rvelte_renderer_packet",
            Self::AiActorPerceptionNavigationIntent => "ai_actor_perception_navigation_intent",
        }
    }

    #[must_use]
    pub const fn required_mask(self) -> MaterializedGroupComponentMask {
        match self {
            Self::TransformRenderableVisibility => MaterializedGroupComponentMask::TRANSFORM
                .union(MaterializedGroupComponentMask::RENDERABLE)
                .union(MaterializedGroupComponentMask::VISIBILITY_POLICY),
            Self::TransformPhysicsReplication => MaterializedGroupComponentMask::TRANSFORM
                .union(MaterializedGroupComponentMask::PHYSICS_PROXY)
                .union(MaterializedGroupComponentMask::REPLICATION),
            Self::StreamPageResidencyArtifactStatus => MaterializedGroupComponentMask::STREAM_PAGE
                .union(MaterializedGroupComponentMask::RESIDENCY)
                .union(MaterializedGroupComponentMask::ARTIFACT_STATUS),
            Self::UiMountRvelteRendererPacket => MaterializedGroupComponentMask::UI_MOUNT
                .union(MaterializedGroupComponentMask::RVELTE_STATE)
                .union(MaterializedGroupComponentMask::RENDERER_PACKET),
            Self::AiActorPerceptionNavigationIntent => MaterializedGroupComponentMask::AI_ACTOR
                .union(MaterializedGroupComponentMask::PERCEPTION)
                .union(MaterializedGroupComponentMask::NAVIGATION_INTENT),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaterializedGroupComponentMask {
    pub bits: u64,
}

impl MaterializedGroupComponentMask {
    pub const NONE: Self = Self { bits: 0 };
    pub const TRANSFORM: Self = Self { bits: 1 << 0 };
    pub const RENDERABLE: Self = Self { bits: 1 << 1 };
    pub const VISIBILITY_POLICY: Self = Self { bits: 1 << 2 };
    pub const PHYSICS_PROXY: Self = Self { bits: 1 << 3 };
    pub const REPLICATION: Self = Self { bits: 1 << 4 };
    pub const STREAM_PAGE: Self = Self { bits: 1 << 5 };
    pub const RESIDENCY: Self = Self { bits: 1 << 6 };
    pub const ARTIFACT_STATUS: Self = Self { bits: 1 << 7 };
    pub const UI_MOUNT: Self = Self { bits: 1 << 8 };
    pub const RVELTE_STATE: Self = Self { bits: 1 << 9 };
    pub const RENDERER_PACKET: Self = Self { bits: 1 << 10 };
    pub const AI_ACTOR: Self = Self { bits: 1 << 11 };
    pub const PERCEPTION: Self = Self { bits: 1 << 12 };
    pub const NAVIGATION_INTENT: Self = Self { bits: 1 << 13 };

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

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaterializedGroupSourceRow {
    pub stable_subject: u64,
    pub entity: Option<FunEntity>,
    pub page: Option<EcsSpatialPageKey>,
    pub components: MaterializedGroupComponentMask,
    pub source_revision: FunRevision,
}

impl MaterializedGroupSourceRow {
    #[must_use]
    pub const fn entity(
        stable_subject: u64,
        entity: FunEntity,
        components: MaterializedGroupComponentMask,
        source_revision: FunRevision,
    ) -> Self {
        Self {
            stable_subject,
            entity: Some(entity),
            page: None,
            components,
            source_revision,
        }
    }

    #[must_use]
    pub const fn page(
        stable_subject: u64,
        page: EcsSpatialPageKey,
        components: MaterializedGroupComponentMask,
        source_revision: FunRevision,
    ) -> Self {
        Self {
            stable_subject,
            entity: None,
            page: Some(page),
            components,
            source_revision,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaterializedGroupRow {
    pub stable_subject: u64,
    pub entity: Option<FunEntity>,
    pub page: Option<EcsSpatialPageKey>,
    pub component_mask: MaterializedGroupComponentMask,
    pub source_revision: FunRevision,
    pub group_revision: FunRevision,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaterializedGroupDigest {
    pub value: u64,
}

impl MaterializedGroupDigest {
    #[must_use]
    pub fn from_group(group: &MaterializedGroup) -> Self {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        hash = group_hash_u8(hash, group.kind as u8);
        hash = group_hash_u64(hash, group.revision.get());
        hash = group_hash_u64(hash, group.rows.len() as u64);
        for row in &group.rows {
            hash = group_hash_u64(hash, row.stable_subject);
            hash = group_hash_u64(hash, row.component_mask.bits);
            hash = group_hash_u64(hash, row.source_revision.get());
            hash = group_hash_u64(hash, row.group_revision.get());
            if let Some(entity) = row.entity {
                hash = group_hash_u64(hash, entity.scheduler_bits());
            }
            if let Some(page) = row.page {
                hash = group_hash_u64(hash, page.chunk_key().get());
            }
        }
        Self { value: hash }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedGroup {
    pub kind: MaterializedGroupKind,
    pub rows: Vec<MaterializedGroupRow>,
    pub revision: FunRevision,
    pub digest: MaterializedGroupDigest,
}

impl MaterializedGroup {
    #[must_use]
    pub fn empty(kind: MaterializedGroupKind, revision: FunRevision) -> Self {
        let mut group = Self {
            kind,
            rows: Vec::new(),
            revision,
            digest: MaterializedGroupDigest::default(),
        };
        group.digest = MaterializedGroupDigest::from_group(&group);
        group
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
    pub fn contains_subject(&self, stable_subject: u64) -> bool {
        self.rows
            .iter()
            .any(|row| row.stable_subject == stable_subject)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedGroupMaintainer {
    pub kind: MaterializedGroupKind,
    pub revision: FunRevision,
    pub last_digest: MaterializedGroupDigest,
}

impl MaterializedGroupMaintainer {
    #[must_use]
    pub fn new(kind: MaterializedGroupKind) -> Self {
        Self {
            kind,
            revision: FunRevision::INITIAL,
            last_digest: MaterializedGroupDigest::default(),
        }
    }

    pub fn rebuild(
        &mut self,
        sources: &[MaterializedGroupSourceRow],
    ) -> Result<MaterializedGroup, StorageExperimentError> {
        let required = self.kind.required_mask();
        if required.is_empty() {
            return Err(StorageExperimentError::InvalidMaterializedGroup);
        }
        self.revision = self.revision.next();
        let mut rows: Vec<MaterializedGroupRow> = sources
            .iter()
            .copied()
            .filter(|source| source.components.contains(required))
            .map(|source| MaterializedGroupRow {
                stable_subject: source.stable_subject,
                entity: source.entity,
                page: source.page,
                component_mask: source.components,
                source_revision: source.source_revision,
                group_revision: self.revision,
            })
            .collect();
        rows.sort_by_key(|row| {
            (
                row.stable_subject,
                row.entity.map(FunEntity::scheduler_bits).unwrap_or(0),
                row.page.map(|page| page.chunk_key().get()).unwrap_or(0),
            )
        });
        rows.dedup_by_key(|row| row.stable_subject);
        let mut group = MaterializedGroup {
            kind: self.kind,
            rows,
            revision: self.revision,
            digest: MaterializedGroupDigest::default(),
        };
        group.digest = MaterializedGroupDigest::from_group(&group);
        self.last_digest = group.digest;
        Ok(group)
    }
}

const fn group_hash_u8(hash: u64, value: u8) -> u64 {
    (hash ^ value as u64).wrapping_mul(0x0000_0100_0000_01b3)
}

const fn group_hash_u64(mut hash: u64, value: u64) -> u64 {
    hash = group_hash_u8(hash, (value & 0xff) as u8);
    hash = group_hash_u8(hash, ((value >> 8) & 0xff) as u8);
    hash = group_hash_u8(hash, ((value >> 16) & 0xff) as u8);
    hash = group_hash_u8(hash, ((value >> 24) & 0xff) as u8);
    hash = group_hash_u8(hash, ((value >> 32) & 0xff) as u8);
    hash = group_hash_u8(hash, ((value >> 40) & 0xff) as u8);
    hash = group_hash_u8(hash, ((value >> 48) & 0xff) as u8);
    group_hash_u8(hash, ((value >> 56) & 0xff) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EcsPageChannel, EcsSpatialDomainKind, EcsSpatialGridId, FunEntityGeneration};

    fn entity(slot: u64) -> FunEntity {
        FunEntity::new(slot, FunEntityGeneration::new(1))
    }

    fn page(x: i32) -> EcsSpatialPageKey {
        EcsSpatialPageKey::new(
            EcsSpatialDomainKind::Terrain,
            EcsSpatialGridId::new(1),
            0,
            x,
            0,
            0,
            EcsPageChannel::Surface,
        )
    }

    #[test]
    fn materialized_group_filters_repeated_render_visibility_join() {
        let required = MaterializedGroupKind::TransformRenderableVisibility.required_mask();
        let sources = [
            MaterializedGroupSourceRow::entity(3, entity(3), required, FunRevision::new(1)),
            MaterializedGroupSourceRow::entity(
                1,
                entity(1),
                MaterializedGroupComponentMask::TRANSFORM
                    .union(MaterializedGroupComponentMask::RENDERABLE),
                FunRevision::new(1),
            ),
            MaterializedGroupSourceRow::entity(2, entity(2), required, FunRevision::new(1)),
        ];
        let mut maintainer =
            MaterializedGroupMaintainer::new(MaterializedGroupKind::TransformRenderableVisibility);

        let group = maintainer.rebuild(&sources).expect("group rebuild");

        assert_eq!(group.len(), 2);
        assert!(group.contains_subject(2));
        assert!(group.contains_subject(3));
        assert!(!group.contains_subject(1));
        assert_eq!(group.rows[0].stable_subject, 2);
        assert_eq!(group.digest, maintainer.last_digest);
    }

    #[test]
    fn materialized_group_digest_is_order_stable_for_stream_page_status_join() {
        let required = MaterializedGroupKind::StreamPageResidencyArtifactStatus.required_mask();
        let first = [
            MaterializedGroupSourceRow::page(10, page(10), required, FunRevision::new(4)),
            MaterializedGroupSourceRow::page(20, page(20), required, FunRevision::new(4)),
        ];
        let second = [first[1], first[0]];
        let mut first_maintainer = MaterializedGroupMaintainer::new(
            MaterializedGroupKind::StreamPageResidencyArtifactStatus,
        );
        let mut second_maintainer = MaterializedGroupMaintainer::new(
            MaterializedGroupKind::StreamPageResidencyArtifactStatus,
        );

        let first_group = first_maintainer.rebuild(&first).expect("first group");
        let second_group = second_maintainer.rebuild(&second).expect("second group");

        assert_eq!(first_group.revision, second_group.revision);
        assert_eq!(
            first_group.rows[0].stable_subject,
            second_group.rows[0].stable_subject
        );
        assert_eq!(first_group.digest, second_group.digest);
        assert_ne!(first_group.digest.value, 0);
    }

    #[test]
    fn materialized_group_candidates_cover_requested_hot_joins() {
        let candidates = [
            MaterializedGroupKind::TransformRenderableVisibility,
            MaterializedGroupKind::TransformPhysicsReplication,
            MaterializedGroupKind::StreamPageResidencyArtifactStatus,
            MaterializedGroupKind::UiMountRvelteRendererPacket,
            MaterializedGroupKind::AiActorPerceptionNavigationIntent,
        ];

        for candidate in candidates {
            assert!(!candidate.label().is_empty());
            assert!(!candidate.required_mask().is_empty());
        }
    }
}
