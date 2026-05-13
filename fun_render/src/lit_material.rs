use bevy::{
    asset::embedded_asset, pbr::Material, prelude::*, reflect::TypePath,
    render::render_resource::AsBindGroup, shader::ShaderRef,
};

use crate::{MaterialAlphaModeKey, MaterialKey};

pub const FUN_RENDERER_LIT_MATERIAL_SHADER: &str = "embedded://fun_render/lit_material.wgsl";

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct FunRendererLitMaterial {
    #[uniform(0)]
    pub base_color: LinearRgba,
    #[uniform(1)]
    pub light_terms: Vec4,
    #[uniform(2)]
    pub light_direction: Vec4,
    pub alpha_mode: AlphaMode,
}

impl FunRendererLitMaterial {
    pub const DEFAULT_LIGHT_TERMS: Vec4 = Vec4::new(0.42, 0.92, 0.35, 0.0);
    pub const DEFAULT_SURFACE_TO_LIGHT: Vec4 = Vec4::new(-0.35, 0.82, 0.45, 0.0);

    #[must_use]
    pub fn from_material_key(key: MaterialKey) -> Self {
        let [r, g, b, a] = key.base_color_rgba8;
        Self {
            base_color: Color::srgba_u8(r, g, b, a).to_linear(),
            light_terms: Self::DEFAULT_LIGHT_TERMS,
            light_direction: Self::DEFAULT_SURFACE_TO_LIGHT,
            alpha_mode: alpha_mode_from_key(key.alpha_mode),
        }
    }
}

impl Material for FunRendererLitMaterial {
    fn fragment_shader() -> ShaderRef {
        FUN_RENDERER_LIT_MATERIAL_SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        self.alpha_mode
    }
}

pub fn load_fun_renderer_lit_material_shader_assets(app: &mut App) {
    embedded_asset!(app, "lit_material.wgsl");
}

const fn alpha_mode_from_key(alpha_mode: MaterialAlphaModeKey) -> AlphaMode {
    match alpha_mode {
        MaterialAlphaModeKey::Opaque => AlphaMode::Opaque,
        MaterialAlphaModeKey::Mask => AlphaMode::Mask(0.5),
        MaterialAlphaModeKey::Blend => AlphaMode::Blend,
    }
}
