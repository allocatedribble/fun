use bevy_ecs::prelude::Component;

use crate::FunSceneOwner;

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
