#![forbid(unsafe_code)]

extern crate self as fun_scene;

pub mod asset;
pub mod authoring;
pub mod diagnostics;
pub mod format;
pub mod heuristic_components;
pub mod lux_components;
pub mod manifest;
pub mod observers;
pub mod patch;
pub mod plugin;
pub mod prelude;
pub mod presentation_components;
pub mod renderer_components;
pub mod resolved;
pub mod scene;
pub mod scene_list;
pub mod schedule;
pub mod spawn;
pub mod stable_identity;
pub mod streaming;
pub mod template;
pub mod template_value;
pub mod ui_components;
pub mod validation;

pub use bevy_scene;
#[deprecated(note = "use fun_scene::fun instead of the temporary bsn alias")]
pub use fun_scene_macros::fun as bsn;
#[deprecated(note = "use fun_scene::fun_list instead of the temporary bsn_list alias")]
pub use fun_scene_macros::fun_list as bsn_list;
pub use fun_scene_macros::{fun, fun_list};

pub use asset::*;
pub use authoring::*;
pub use diagnostics::*;
pub use format::*;
pub use heuristic_components::*;
pub use lux_components::*;
pub use manifest::*;
pub use observers::*;
pub use patch::*;
pub use plugin::*;
pub use presentation_components::*;
pub use renderer_components::*;
pub use resolved::*;
pub use scene::*;
pub use scene_list::*;
pub use schedule::*;
pub use spawn::*;
pub use stable_identity::*;
pub use streaming::*;
pub use template::*;
pub use template_value::*;
pub use ui_components::*;
pub use validation::*;

pub const FUN_SCENE_SCHEMA_VERSION: u16 = 1;
pub const FUN_SCENE_PACKAGE_NAME: &str = "fun-scene";
pub const FUN_SCENE_CRATE_NAME: &str = "fun_scene";
pub const FUN_SCENE_MACRO_PACKAGE_NAME: &str = "fun-scene-macros";
pub const FUN_SCENE_AUTHORING_MACRO: &str = "fun";
pub const FUN_SCENE_LIST_AUTHORING_MACRO: &str = "fun_list";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunSceneOwner {
    SceneAuthoring,
    Renderer,
    Lighting,
    ServerAuthority,
    Editor,
}

impl FunSceneOwner {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SceneAuthoring => FUN_SCENE_CRATE_NAME,
            Self::Renderer => "fun_renderer",
            Self::Lighting => "fun_lux",
            Self::ServerAuthority => "game_server",
            Self::Editor => "fun_host",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunSceneProductTopology {
    pub scene_package: &'static str,
    pub scene_crate: &'static str,
    pub macro_package: &'static str,
    pub authoring_macro: &'static str,
    pub list_authoring_macro: &'static str,
    pub source_model: &'static str,
}

pub const FUN_SCENE_PRODUCT_TOPOLOGY: FunSceneProductTopology = FunSceneProductTopology {
    scene_package: FUN_SCENE_PACKAGE_NAME,
    scene_crate: FUN_SCENE_CRATE_NAME,
    macro_package: FUN_SCENE_MACRO_PACKAGE_NAME,
    authoring_macro: FUN_SCENE_AUTHORING_MACRO,
    list_authoring_macro: FUN_SCENE_LIST_AUTHORING_MACRO,
    source_model: "bevy_scene_bsn_reused_then_fun_owned",
};

#[cfg(test)]
mod tests {
    use bevy_color::Color;
    use bevy_ecs::world::World;
    use bevy_transform::components::Transform;
    use thunder::prelude::{NetEntity, WorldLevelId, WorldRevision, WorldStreamChunk};

    use super::*;

    #[test]
    fn topology_names_match_fun_scene_decision() {
        assert_eq!(FUN_SCENE_PRODUCT_TOPOLOGY.scene_package, "fun-scene");
        assert_eq!(FUN_SCENE_PRODUCT_TOPOLOGY.scene_crate, "fun_scene");
        assert_eq!(FUN_SCENE_PRODUCT_TOPOLOGY.macro_package, "fun-scene-macros");
        assert_eq!(FUN_SCENE_PRODUCT_TOPOLOGY.authoring_macro, "fun");
        assert_eq!(FUN_SCENE_PRODUCT_TOPOLOGY.list_authoring_macro, "fun_list");
        assert_eq!(
            FUN_SCENE_PRODUCT_TOPOLOGY.source_model,
            "bevy_scene_bsn_reused_then_fun_owned"
        );
    }

    #[test]
    fn policy_confirms_ecs_first_scene_authority() {
        let policy = core::hint::black_box(FunSceneAuthoringPolicy::DEFAULT);

        assert!(policy.bevy_scene_bsn_reuse_confirmed);
        assert!(policy.fun_macro_is_primary_authoring_surface);
        assert!(policy.deterministic_manifest_required);
        assert!(policy.server_editor_renderer_shared_authority);
        assert!(policy.renderer_consumes_ecs_archetypes);
        assert!(policy.lighting_consumes_ecs_declarations);
    }

    #[test]
    fn renderer_and_lighting_declarations_are_ecs_components() {
        let mut world = World::new();
        let entity = world
            .spawn((
                FunSceneEntityId::new(42),
                FunSceneRendererDeclaration::new(
                    FunSceneRendererDeclarationKind::RuntimeProceduralGeometry,
                ),
                FunSceneLightingDeclaration::new(FunSceneLightingDeclarationKind::DirectLight),
            ))
            .id();

        let id = world
            .get::<FunSceneEntityId>(entity)
            .expect("scene entity id should be an ECS component");
        assert!(id.is_valid());

        let render = world
            .get::<FunSceneRendererDeclaration>(entity)
            .expect("renderer declaration should be an ECS component");
        assert_eq!(render.owner, FunSceneOwner::Renderer);
        assert_eq!(
            render.kind.as_str(),
            FunSceneRendererDeclarationKind::RuntimeProceduralGeometry.as_str()
        );

        let lighting = world
            .get::<FunSceneLightingDeclaration>(entity)
            .expect("lighting declaration should be an ECS component");
        assert_eq!(lighting.owner, FunSceneOwner::Lighting);
        assert_eq!(
            lighting.kind.as_str(),
            FunSceneLightingDeclarationKind::DirectLight.as_str()
        );
    }

    #[test]
    fn stable_identity_taxonomy_uses_thunder_world_ids() {
        let identity = SceneStableIdentity::new(NetEntity(99));
        assert!(identity.is_valid());

        let revision = SceneRevision::new(WorldRevision(7));
        assert_eq!(revision.0, WorldRevision(7));

        let chunk = WorldStreamChunk {
            level_id: WorldLevelId("arena/blockout".to_owned()),
            revision: WorldRevision(11),
            chunk_index: 3,
            chunk_count: 4,
            manifest_signature: 123,
            entities: Vec::new(),
        };
        let chunk_id = SceneChunkId::from_world_stream_chunk(&chunk);
        assert_eq!(chunk_id.level, chunk.level_id);
        assert_eq!(chunk_id.chunk_index, 3);
        assert_eq!(chunk_id.revision, WorldRevision(11));
    }

    #[test]
    fn renderer_taxonomy_keeps_authoring_data_in_ecs_components() {
        let mut world = World::new();
        let entity = world
            .spawn((
                Renderable::new(
                    GeometryRef::new(5),
                    MaterialRef::new(4),
                    RenderableFlags::STATIC_WORLD,
                ),
                VirtualGeometryAuthoring {
                    mode: VirtualGeometryMode::StaticClusterPages,
                    page_priority: PagePriorityHint::WorldCritical,
                    dynamic_policy: DynamicGeometryPolicy::StaticOnly,
                },
                RendererBounds {
                    local_bounds: Default::default(),
                    streaming_radius: 32.0,
                },
            ))
            .id();

        let renderable = world
            .get::<Renderable>(entity)
            .expect("renderable authoring should be an ECS component");
        assert!(renderable.geometry.is_valid());
        assert!(renderable.material.is_valid());
        assert!(renderable.flags.contains(RenderableFlags::STATIC));
        assert!(renderable.flags.contains(RenderableFlags::SHADOW_CASTER));
        assert!(renderable.flags.contains(RenderableFlags::SHADOW_RECEIVER));
    }

    #[test]
    fn lux_ui_and_upscale_taxonomy_encode_product_rules() {
        let mut world = World::new();
        let entity = world
            .spawn((
                LuxLight::directional(110_000.0),
                LuxEmissive {
                    luminance: 2500.0,
                    candidate_policy: EmissiveCandidatePolicy::AutoPromote,
                },
                LuxGiParticipant {
                    bounce_policy: GiBouncePolicy::DynamicBudgeted,
                    cache_policy: GiCachePolicy::Probe,
                },
                VirtualShadowCaster {
                    policy: ShadowCasterPolicy::VirtualPages,
                    invalidation: ShadowInvalidationPolicy::OnTransformOrGeometryChange,
                },
                VirtualShadowReceiver {
                    priority: ShadowReceiverPriority::High,
                    filter_policy: ShadowFilterPolicy::ContactAware,
                },
                CefSurface::product(CefRoute::HUD, UiLayer::Hud),
                ViewportUiTarget {
                    viewport: ViewportId::PRIMARY,
                    scale_policy: UiScalePolicy::DpiAware,
                },
                UpscalePolicy::default(),
                ViewportRenderPolicy {
                    quality: RendererQuality::Quality,
                    latency: LatencyPolicy::LowLatency,
                    editor_mode: EditorViewportMode::EditorDocked,
                },
            ))
            .id();

        let light = world
            .get::<LuxLight>(entity)
            .expect("lux light authoring should be an ECS component");
        assert_eq!(light.kind, LuxLightKind::Directional);
        assert_eq!(light.shadow_policy, LuxShadowPolicy::VirtualDemandPaged);

        let caster = world
            .get::<VirtualShadowCaster>(entity)
            .expect("virtual shadow caster authoring should be an ECS component");
        assert_eq!(caster.policy, ShadowCasterPolicy::VirtualPages);
        assert_eq!(
            caster.invalidation,
            ShadowInvalidationPolicy::OnTransformOrGeometryChange
        );

        let cef = world
            .get::<CefSurface>(entity)
            .expect("CEF surface authoring should be an ECS component");
        assert!(product_scene_accepts_cef_surface(cef));

        let mut invalid_cef = cef.clone();
        invalid_cef.gpu_only = false;
        assert_eq!(
            invalid_cef.validate_product(),
            Err(CefSurfaceValidationError::ProductRequiresGpuOnly)
        );

        let upscale = world
            .get::<UpscalePolicy>(entity)
            .expect("upscale policy should be an ECS component");
        assert!(upscale.hudless_required);
    }

    #[test]
    fn fun_macro_authors_virtual_geometry_and_shadow_components() {
        fn megastructure_wall(
            id: NetEntity,
            geometry: GeometryRef,
            material: MaterialRef,
        ) -> impl FunScene {
            fun! {
                #MegastructureWall
                fun_value(SceneStableIdentity(id))
                Renderable {
                    geometry: {geometry},
                    material: {material},
                    flags: RenderableFlags::STATIC_WORLD
                }
                VirtualGeometryAuthoring {
                    mode: VirtualGeometryMode::StaticClusterPages,
                    page_priority: PagePriorityHint::WorldCritical,
                    dynamic_policy: DynamicGeometryPolicy::StaticOnly
                }
                VirtualShadowCaster {
                    policy: ShadowCasterPolicy::VirtualPages,
                    invalidation: ShadowInvalidationPolicy::OnTransformOrGeometryChange
                }
                VirtualShadowReceiver {
                    priority: ShadowReceiverPriority::High,
                    filter_policy: ShadowFilterPolicy::ContactAware
                }
                LuxGiParticipant {
                    bounce_policy: GiBouncePolicy::StaticSingleBounce,
                    cache_policy: GiCachePolicy::Surface
                }
                Transform::default()
            }
        }

        let _scene = megastructure_wall(NetEntity(500), GeometryRef::new(20), MaterialRef::new(30));
    }

    #[test]
    fn fun_macro_authors_scheduler_input_components() {
        fn selected_dynamic_cover(id: NetEntity) -> impl FunScene {
            fun! {
                #SelectedDynamicCover
                fun_value(SceneStableIdentity(id))
                GameplaySalient { score: 220 }
                EditorSelection {
                    rank: 0,
                    salience: 255
                }
                StreamingPriority { score: 240 }
                TemporalInstability {
                    motion: 192,
                    topology: 64
                }
            }
        }

        let _scene = selected_dynamic_cover(NetEntity(600));
        assert_eq!(TemporalInstability::DESTRUCTIBLE.score(), u8::MAX);
    }

    #[test]
    fn fun_macro_can_author_cinematic_lux_light() {
        fn cinematic_sun() -> impl FunScene {
            fun! {
                #Sun
                LuxLight {
                    kind: LuxLightKind::Directional,
                    color: {Color::srgb(1.0, 0.94, 0.82)},
                    intensity_lux: 80_000.0,
                    range: 0.0,
                    shadow_policy: LuxShadowPolicy::VirtualDirectionalClipmap,
                    importance: LuxImportance::Critical
                }
            }
        }

        let _scene = cinematic_sun();
    }

    #[test]
    fn validation_policy_requires_safe_asset_boundary() {
        let policy = FunSceneValidationPolicy::DEFAULT;

        assert!(policy.reject_arbitrary_rust_expressions);
        assert!(policy.validate_schema_before_spawn);
        assert!(policy.require_deterministic_stable_ids);
        assert!(policy.retain_renderer_lux_component_authority);
    }
}
