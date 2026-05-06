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
