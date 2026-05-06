use bevy_camera::primitives::Aabb;
use bevy_ecs::prelude::Component;

use crate::{FunFromTemplate, FunSceneOwner};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GeometryRef(pub u32);

impl GeometryRef {
    pub const INVALID: Self = Self(0);

    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != Self::INVALID.0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaterialRef(pub u32);

impl MaterialRef {
    pub const INVALID: Self = Self(0);

    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != Self::INVALID.0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderableFlags(pub u32);

impl RenderableFlags {
    pub const NONE: Self = Self(0);
    pub const STATIC: Self = Self(1 << 0);
    pub const DYNAMIC: Self = Self(1 << 1);
    pub const SHADOW_CASTER: Self = Self(1 << 2);
    pub const SHADOW_RECEIVER: Self = Self(1 << 3);
    pub const PROCEDURAL_SOURCE: Self = Self(1 << 4);
    pub const STATIC_WORLD: Self =
        Self(Self::STATIC.0 | Self::SHADOW_CASTER.0 | Self::SHADOW_RECEIVER.0);

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[derive(Debug, Default, Clone, FunFromTemplate, Component)]
pub struct Renderable {
    pub geometry: GeometryRef,
    pub material: MaterialRef,
    pub flags: RenderableFlags,
}

impl Renderable {
    #[must_use]
    pub const fn new(geometry: GeometryRef, material: MaterialRef, flags: RenderableFlags) -> Self {
        Self {
            geometry,
            material,
            flags,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VirtualGeometryMode {
    #[default]
    Disabled,
    StaticClusterPages,
    DynamicClusterPages,
    RuntimeProceduralPages,
    #[deprecated(note = "use StaticClusterPages")]
    StaticPages,
    #[deprecated(note = "use DynamicClusterPages")]
    DynamicPages,
    #[deprecated(note = "use RuntimeProceduralPages")]
    ProceduralPages,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PagePriorityHint {
    Low,
    #[default]
    Normal,
    High,
    Critical,
    WorldCritical,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DynamicGeometryPolicy {
    #[default]
    StaticOnly,
    TransformOnly,
    RebuildPages,
    RuntimeProcedural,
    #[deprecated(note = "use StaticOnly")]
    Static,
}

#[derive(Debug, Default, Clone, FunFromTemplate, Component)]
pub struct VirtualGeometryAuthoring {
    pub mode: VirtualGeometryMode,
    pub page_priority: PagePriorityHint,
    pub dynamic_policy: DynamicGeometryPolicy,
}

#[derive(Debug, Default, Clone, FunFromTemplate, Component)]
pub struct RendererBounds {
    pub local_bounds: Aabb,
    pub streaming_radius: f32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SceneGeometryKind {
    #[default]
    Static,
    Streamable,
    Dynamic,
    ProceduralChunk,
}

impl SceneGeometryKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Streamable => "streamable",
            Self::Dynamic => "dynamic",
            Self::ProceduralChunk => "procedural_chunk",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DynamicGeometryClass {
    SkinnedPlayer,
    Npc,
    Vehicle,
    Weapon,
    DestructionFragment,
    ProceduralTerrainChunk,
    TemporaryFxGeometry,
    EditorGizmoDebugShape,
    #[default]
    GenericDynamic,
}

impl DynamicGeometryClass {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SkinnedPlayer => "skinned_player",
            Self::Npc => "npc",
            Self::Vehicle => "vehicle",
            Self::Weapon => "weapon",
            Self::DestructionFragment => "destruction_fragment",
            Self::ProceduralTerrainChunk => "procedural_terrain_chunk",
            Self::TemporaryFxGeometry => "temporary_fx_geometry",
            Self::EditorGizmoDebugShape => "editor_gizmo_debug_shape",
            Self::GenericDynamic => "generic_dynamic",
        }
    }

    #[must_use]
    pub const fn skinned(self) -> bool {
        matches!(self, Self::SkinnedPlayer | Self::Npc)
    }

    #[must_use]
    pub const fn procedural(self) -> bool {
        matches!(self, Self::ProceduralTerrainChunk)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DynamicGeometryLifetime {
    PersistentActor,
    StreamedChunk,
    DestructionDebris,
    TransientFrame,
    EditorOnly,
    #[default]
    RuntimeDynamic,
}

impl DynamicGeometryLifetime {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PersistentActor => "persistent_actor",
            Self::StreamedChunk => "streamed_chunk",
            Self::DestructionDebris => "destruction_debris",
            Self::TransientFrame => "transient_frame",
            Self::EditorOnly => "editor_only",
            Self::RuntimeDynamic => "runtime_dynamic",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DynamicGeometryUpdateHint {
    Spawned,
    TransformChanged,
    SkinningPoseChanged,
    VertexDataChanged,
    IndexDataChanged,
    MaterialChanged,
    ProceduralChunkEdited,
    DestructionFractured,
    LifetimeExpired,
    EditorGizmoChanged,
    #[default]
    Unknown,
}

impl DynamicGeometryUpdateHint {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Spawned => "spawned",
            Self::TransformChanged => "transform_changed",
            Self::SkinningPoseChanged => "skinning_pose_changed",
            Self::VertexDataChanged => "vertex_data_changed",
            Self::IndexDataChanged => "index_data_changed",
            Self::MaterialChanged => "material_changed",
            Self::ProceduralChunkEdited => "procedural_chunk_edited",
            Self::DestructionFractured => "destruction_fractured",
            Self::LifetimeExpired => "lifetime_expired",
            Self::EditorGizmoChanged => "editor_gizmo_changed",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProceduralChunkOwner {
    pub chunk_id: u64,
    pub revision: u64,
}

impl ProceduralChunkOwner {
    pub const NONE: Self = Self {
        chunk_id: 0,
        revision: 0,
    };

    #[must_use]
    pub const fn new(chunk_id: u64, revision: u64) -> Self {
        Self { chunk_id, revision }
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.chunk_id != 0
    }
}

#[derive(Debug, Default, Clone, FunFromTemplate, Component)]
pub struct GeometryDeclaration {
    pub kind: SceneGeometryKind,
    pub lighting_metadata: bool,
    pub procedural_owner: ProceduralChunkOwner,
}

#[derive(Debug, Default, Clone, FunFromTemplate, Component)]
pub struct DynamicGeometryAuthoring {
    pub class: DynamicGeometryClass,
    pub lifetime: DynamicGeometryLifetime,
    pub update_hint: DynamicGeometryUpdateHint,
    pub procedural_owner: ProceduralChunkOwner,
    pub allow_dynamic_clusters: bool,
    pub priority_hint: PagePriorityHint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunSceneRendererDeclarationKind {
    GpuSceneObject,
    VirtualGeometrySource,
    VirtualShadowCaster,
    StreamingPageSource,
    RuntimeProceduralGeometry,
}

impl FunSceneRendererDeclarationKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GpuSceneObject => "gpu_scene_object",
            Self::VirtualGeometrySource => "virtual_geometry_source",
            Self::VirtualShadowCaster => "virtual_shadow_caster",
            Self::StreamingPageSource => "streaming_page_source",
            Self::RuntimeProceduralGeometry => "runtime_procedural_geometry",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct FunSceneRendererDeclaration {
    pub kind: FunSceneRendererDeclarationKind,
    pub owner: FunSceneOwner,
}

impl FunSceneRendererDeclaration {
    #[must_use]
    pub const fn new(kind: FunSceneRendererDeclarationKind) -> Self {
        Self {
            kind,
            owner: FunSceneOwner::Renderer,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_geometry_taxonomy_matches_runtime_targets() {
        assert_eq!(SceneGeometryKind::Dynamic.as_str(), "dynamic");
        assert_eq!(
            SceneGeometryKind::ProceduralChunk.as_str(),
            "procedural_chunk"
        );
        assert!(DynamicGeometryClass::SkinnedPlayer.skinned());
        assert!(DynamicGeometryClass::Npc.skinned());
        assert!(DynamicGeometryClass::ProceduralTerrainChunk.procedural());
        assert_eq!(
            DynamicGeometryClass::DestructionFragment.as_str(),
            "destruction_fragment"
        );
        assert_eq!(
            DynamicGeometryClass::EditorGizmoDebugShape.as_str(),
            "editor_gizmo_debug_shape"
        );
        assert_eq!(
            DynamicGeometryLifetime::DestructionDebris.as_str(),
            "destruction_debris"
        );
        assert_eq!(
            DynamicGeometryUpdateHint::ProceduralChunkEdited.as_str(),
            "procedural_chunk_edited"
        );
    }

    #[test]
    fn procedural_chunk_owner_tracks_revision_without_runtime_entity_identity() {
        let owner = ProceduralChunkOwner::new(42, 7);

        assert!(owner.is_valid());
        assert_eq!(owner.chunk_id, 42);
        assert_eq!(owner.revision, 7);
        assert!(!ProceduralChunkOwner::NONE.is_valid());
    }
}
