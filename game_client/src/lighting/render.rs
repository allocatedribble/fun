use bevy::{
    core_pipeline::core_3d::graph::{Core3d, Node3d},
    ecs::query::QueryItem,
    prelude::*,
    render::{
        Render, RenderApp, RenderStartup, RenderSystems,
        render_graph::{
            NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner,
        },
        render_resource::WgpuFeatures,
        renderer::{RenderContext, RenderDevice},
        view::ExtractedView,
    },
};
use std::collections::HashMap;

use super::{LightingBackend, LightingPipelineSettings, LightingPipelineView};

pub struct LightingPipelineRenderPlugin;

impl Plugin for LightingPipelineRenderPlugin {
    fn build(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .init_resource::<LightingRenderCapabilities>()
            .init_resource::<LightingViewQueue>()
            .add_systems(RenderStartup, init_lighting_render_capabilities)
            .add_systems(
                Render,
                (
                    prepare_lighting_views.in_set(RenderSystems::PrepareResources),
                    queue_lighting_views.in_set(RenderSystems::Queue),
                ),
            )
            .add_render_graph_node::<ViewNodeRunner<GameLightingViewNode>>(
                Core3d,
                GameLightingNodeLabel::DeferredLightingPass,
            )
            .add_render_graph_edges(
                Core3d,
                (
                    Node3d::StartMainPass,
                    GameLightingNodeLabel::DeferredLightingPass,
                    Node3d::MainOpaquePass,
                ),
            );
    }
}

#[derive(Resource, Debug, Default)]
pub struct LightingRenderCapabilities {
    pub push_constants_supported: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct QueuedLightingView {
    pub backend: LightingBackend,
    pub uses_push_constants: bool,
}

#[derive(Resource, Debug, Default)]
pub struct LightingViewQueue {
    pub prepared: Vec<(Entity, QueuedLightingView)>,
    pub queued: HashMap<Entity, QueuedLightingView>,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub enum GameLightingNodeLabel {
    DeferredLightingPass,
}

fn init_lighting_render_capabilities(mut commands: Commands, render_device: Res<RenderDevice>) {
    commands.insert_resource(LightingRenderCapabilities {
        push_constants_supported: render_device
            .features()
            .contains(WgpuFeatures::PUSH_CONSTANTS),
    });
}

fn prepare_lighting_views(
    capabilities: Res<LightingRenderCapabilities>,
    settings: Res<LightingPipelineSettings>,
    mut queue: ResMut<LightingViewQueue>,
    views: Query<(Entity, &ExtractedView, &LightingPipelineView)>,
) {
    queue.prepared.clear();
    queue.prepared.extend(views.iter().map(|(entity, _, view)| {
        let backend = view.effective_backend(&settings);

        (
            entity,
            QueuedLightingView {
                backend,
                uses_push_constants: capabilities.push_constants_supported,
            },
        )
    }));
}

fn queue_lighting_views(mut queue: ResMut<LightingViewQueue>) {
    let prepared = queue.prepared.clone();
    queue.queued.clear();
    queue.queued.extend(prepared);
}

#[derive(Default)]
struct GameLightingViewNode;

impl ViewNode for GameLightingViewNode {
    type ViewQuery = (Entity, &'static ExtractedView);

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        _render_context: &mut RenderContext,
        (view_entity, _view): QueryItem<Self::ViewQuery>,
        _world: &World,
    ) -> Result<(), NodeRunError> {
        let queue = _world.resource::<LightingViewQueue>();
        let Some(view) = queue.queued.get(&view_entity) else {
            return Ok(());
        };

        match view.backend {
            LightingBackend::Shiny => {
                let _uses_push_constants = view.uses_push_constants;
            }
            LightingBackend::Reference => {
                let _uses_push_constants = view.uses_push_constants;
            }
        }

        Ok(())
    }
}
