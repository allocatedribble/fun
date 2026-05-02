use bevy::{
    prelude::*,
    render::{
        error_handler::{RenderError, RenderErrorHandler, RenderErrorPolicy},
        render_resource::TextureFormat,
    },
    window::{WindowCreated, WindowResized, WindowScaleFactorChanged},
};
use tracing::info;

use crate::{
    config::{client_render_creation, render_plugin},
    selected_render_backend,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct EditorOffscreenRenderTarget {
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
}

impl EditorOffscreenRenderTarget {
    pub const DEFAULT: Self = Self {
        width: 1280,
        height: 720,
        format: TextureFormat::Rgba16Float,
    };

    pub const fn new(width: u32, height: u32) -> Self {
        let width = if width == 0 { 1 } else { width };
        let height = if height == 0 { 1 } else { height };
        Self {
            width,
            height,
            format: TextureFormat::Rgba16Float,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FunRenderOffscreenPresentationPlugin {
    target: EditorOffscreenRenderTarget,
}

impl FunRenderOffscreenPresentationPlugin {
    pub const fn new(target: EditorOffscreenRenderTarget) -> Self {
        Self { target }
    }
}

impl Plugin for FunRenderOffscreenPresentationPlugin {
    fn build(&self, app: &mut App) {
        let render_backend = selected_render_backend();
        info!(
            target: "fun::render",
            backend = ?render_backend,
            width = self.target.width,
            height = self.target.height,
            format = ?self.target.format,
            "Fun offscreen render backend selected"
        );
        app.insert_resource(self.target);
        app.add_plugins(render_plugin(render_backend))
            .add_message::<WindowResized>()
            .add_message::<WindowCreated>()
            .add_message::<WindowScaleFactorChanged>()
            .insert_resource(RenderErrorHandler(recover_offscreen_render_device));
    }
}

fn recover_offscreen_render_device(
    error: &RenderError,
    _main_world: &mut bevy::ecs::world::World,
    _render_world: &mut bevy::ecs::world::World,
) -> RenderErrorPolicy {
    info!(
        "[fun render] offscreen renderer reported {:?}: {}; skipping recovery for this frame",
        error.ty, error.description
    );
    RenderErrorPolicy::Recover(client_render_creation(selected_render_backend()))
}
