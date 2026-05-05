use std::collections::BTreeSet;

use bevy::prelude::Resource;
use game_shared::{LightingParticipation, RenderCatalogEntry, VisualImportance, material_preset};
use thunder::prelude::PackedColorRgba8;

use crate::{
    FunDepthPrepassMode, FunDrawBucketKey, FunGeometryClass, FunMaterialClass,
    FunMaterialSignature, FunMeshletFormat, FunRenderPath, FunRenderPhaseKind,
    FunShaderPipelineSignature, FunSkeletalMode, FunTextureTableCompatibility, FunTransparencyMode,
    MaterialAlphaModeKey, MaterialKey, RenderGeometryClass,
};

pub const FUN_MATERIAL_PIPELINE_SCHEMA_VERSION: u16 = 1;

pub const PIPELINE_FEATURE_PARALLAX_MAPPING: u32 = 1 << 0;
pub const PIPELINE_FEATURE_CLEARCOAT: u32 = 1 << 1;
pub const PIPELINE_FEATURE_EMISSIVE_BOOST: u32 = 1 << 2;
pub const PIPELINE_FEATURE_WIND_VERTEX_ANIMATION: u32 = 1 << 3;
pub const PIPELINE_FEATURE_ALPHA_TEST: u32 = 1 << 4;

pub const DATA_FLAG_PARALLAX_MAPPING: u32 = 1 << 16;
pub const DATA_FLAG_CLEARCOAT: u32 = 1 << 17;
pub const DATA_FLAG_EMISSIVE_BOOST: u32 = 1 << 18;
pub const DATA_FLAG_WIND_VERTEX_ANIMATION: u32 = 1 << 19;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MaterialInstanceTint {
    pub rgba8: [u8; 4],
}

impl MaterialInstanceTint {
    pub const WHITE: Self = Self {
        rgba8: [255, 255, 255, 255],
    };

    pub const fn from_rgba8(rgba8: [u8; 4]) -> Self {
        Self { rgba8 }
    }

    pub const fn from_packed_color(color: Option<PackedColorRgba8>) -> Self {
        match color {
            Some(color) => Self {
                rgba8: [color.r, color.g, color.b, color.a],
            },
            None => Self::WHITE,
        }
    }

    pub const fn is_neutral(self) -> bool {
        self.rgba8[0] == 255 && self.rgba8[1] == 255 && self.rgba8[2] == 255 && self.rgba8[3] == 255
    }

    pub const fn packed_rgba8(self) -> u32 {
        (self.rgba8[0] as u32)
            | ((self.rgba8[1] as u32) << 8)
            | ((self.rgba8[2] as u32) << 16)
            | ((self.rgba8[3] as u32) << 24)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MaterialConsolidationReason {
    NoColorUsesDefaultSharedMaterial,
    ExactPreset,
    InstanceTintOnSharedOpaqueMaterial,
    TransparentRequiresSeparateMaterialClass,
}

impl MaterialConsolidationReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoColorUsesDefaultSharedMaterial => "no_color_uses_default_shared_material",
            Self::ExactPreset => "exact_preset",
            Self::InstanceTintOnSharedOpaqueMaterial => "instance_tint_on_shared_opaque_material",
            Self::TransparentRequiresSeparateMaterialClass => {
                "transparent_requires_separate_material_class"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaterialInstancePolicy {
    pub shared_material_key: MaterialKey,
    pub material_signature: FunMaterialSignature,
    pub instance_tint: MaterialInstanceTint,
    pub creates_unique_standard_material: bool,
    pub reason: MaterialConsolidationReason,
}

pub fn material_policy_for_stream_color(color: Option<PackedColorRgba8>) -> MaterialInstancePolicy {
    let Some(color) = color else {
        return material_policy_for_key(
            MaterialKey::DEFAULT_DEBUG,
            MaterialInstanceTint::WHITE,
            MaterialConsolidationReason::NoColorUsesDefaultSharedMaterial,
        );
    };

    let rgba8 = [color.r, color.g, color.b, color.a];
    if let Some(preset) = game_shared::MATERIAL_PRESETS
        .iter()
        .find(|preset| preset.srgb == rgba8)
        .copied()
    {
        return material_policy_for_key(
            MaterialKey::from_preset(preset),
            MaterialInstanceTint::WHITE,
            MaterialConsolidationReason::ExactPreset,
        );
    }

    let shared_material_key = if color.a < 255 {
        MaterialKey {
            base_color_rgba8: [255, 255, 255, color.a],
            alpha_mode: MaterialAlphaModeKey::Blend,
            ..MaterialKey::OPAQUE_INSTANCE_TINT
        }
    } else {
        MaterialKey::OPAQUE_INSTANCE_TINT
    };
    let reason = if color.a < 255 {
        MaterialConsolidationReason::TransparentRequiresSeparateMaterialClass
    } else {
        MaterialConsolidationReason::InstanceTintOnSharedOpaqueMaterial
    };
    material_policy_for_key(
        shared_material_key,
        MaterialInstanceTint::from_rgba8(rgba8),
        reason,
    )
}

pub fn material_policy_for_catalog_entry(entry: &RenderCatalogEntry) -> MaterialInstancePolicy {
    let key = material_preset(entry.material)
        .map(|preset| MaterialKey::from_preset(*preset))
        .unwrap_or(MaterialKey::DEFAULT_DEBUG);
    material_policy_for_key(
        key,
        MaterialInstanceTint::WHITE,
        MaterialConsolidationReason::ExactPreset,
    )
}

fn material_policy_for_key(
    shared_material_key: MaterialKey,
    instance_tint: MaterialInstanceTint,
    reason: MaterialConsolidationReason,
) -> MaterialInstancePolicy {
    MaterialInstancePolicy {
        shared_material_key,
        material_signature: material_signature_for_key(shared_material_key),
        instance_tint,
        creates_unique_standard_material: false,
        reason,
    }
}

pub fn material_signature_for_key(key: MaterialKey) -> FunMaterialSignature {
    let mut hash = FNV_OFFSET;
    for byte in key.base_color_rgba8 {
        fnv_update(&mut hash, byte);
    }
    fnv_update(&mut hash, key.roughness_bucket);
    fnv_update(&mut hash, key.metallic_bucket);
    fnv_update(&mut hash, material_alpha_mode_code(key.alpha_mode));
    for byte in key.material_preset_id.unwrap_or(u32::MAX).to_le_bytes() {
        fnv_update(&mut hash, byte);
    }
    FunMaterialSignature(hash)
}

const fn material_alpha_mode_code(alpha_mode: MaterialAlphaModeKey) -> u8 {
    match alpha_mode {
        MaterialAlphaModeKey::Opaque => 1,
        MaterialAlphaModeKey::Mask => 2,
        MaterialAlphaModeKey::Blend => 3,
    }
}

fn fnv_update(hash: &mut u64, byte: u8) {
    *hash ^= u64::from(byte);
    *hash = hash.wrapping_mul(FNV_PRIME);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RenderPipelineSignatureKind {
    SimpleOpaqueStatic,
    DenseMeshletCluster,
    DynamicOpaque,
    Transparent,
    Viewmodel,
}

impl RenderPipelineSignatureKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SimpleOpaqueStatic => "simple_opaque_static",
            Self::DenseMeshletCluster => "dense_meshlet_cluster",
            Self::DynamicOpaque => "dynamic_opaque",
            Self::Transparent => "transparent",
            Self::Viewmodel => "viewmodel",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderPipelineSignatureDescriptor {
    pub signature: FunShaderPipelineSignature,
    pub kind: RenderPipelineSignatureKind,
    pub label: &'static str,
    pub shader_feature_mask: u32,
    pub data_flag_mask: u32,
    pub prewarm: bool,
}

pub const SIMPLE_OPAQUE_STATIC_PIPELINE: FunShaderPipelineSignature =
    FunShaderPipelineSignature(0x4655_4e50_534f_0001);
pub const DENSE_MESHLET_CLUSTER_PIPELINE: FunShaderPipelineSignature =
    FunShaderPipelineSignature(0x4655_4e50_534f_0002);
pub const DYNAMIC_OPAQUE_PIPELINE: FunShaderPipelineSignature =
    FunShaderPipelineSignature(0x4655_4e50_534f_0003);
pub const TRANSPARENT_PIPELINE: FunShaderPipelineSignature =
    FunShaderPipelineSignature(0x4655_4e50_534f_0004);
pub const VIEWMODEL_PIPELINE: FunShaderPipelineSignature =
    FunShaderPipelineSignature(0x4655_4e50_534f_0005);

pub const FUN_RENDER_PIPELINE_SIGNATURES: &[RenderPipelineSignatureDescriptor] = &[
    RenderPipelineSignatureDescriptor {
        signature: SIMPLE_OPAQUE_STATIC_PIPELINE,
        kind: RenderPipelineSignatureKind::SimpleOpaqueStatic,
        label: "fun_simple_opaque_static",
        shader_feature_mask: 0,
        data_flag_mask: DATA_FLAG_PARALLAX_MAPPING | DATA_FLAG_CLEARCOAT,
        prewarm: true,
    },
    RenderPipelineSignatureDescriptor {
        signature: DENSE_MESHLET_CLUSTER_PIPELINE,
        kind: RenderPipelineSignatureKind::DenseMeshletCluster,
        label: "fun_dense_meshlet_cluster",
        shader_feature_mask: 0,
        data_flag_mask: DATA_FLAG_PARALLAX_MAPPING | DATA_FLAG_CLEARCOAT,
        prewarm: true,
    },
    RenderPipelineSignatureDescriptor {
        signature: DYNAMIC_OPAQUE_PIPELINE,
        kind: RenderPipelineSignatureKind::DynamicOpaque,
        label: "fun_dynamic_opaque",
        shader_feature_mask: 0,
        data_flag_mask: DATA_FLAG_PARALLAX_MAPPING
            | DATA_FLAG_CLEARCOAT
            | DATA_FLAG_WIND_VERTEX_ANIMATION,
        prewarm: true,
    },
    RenderPipelineSignatureDescriptor {
        signature: TRANSPARENT_PIPELINE,
        kind: RenderPipelineSignatureKind::Transparent,
        label: "fun_transparent",
        shader_feature_mask: PIPELINE_FEATURE_ALPHA_TEST,
        data_flag_mask: DATA_FLAG_EMISSIVE_BOOST,
        prewarm: true,
    },
    RenderPipelineSignatureDescriptor {
        signature: VIEWMODEL_PIPELINE,
        kind: RenderPipelineSignatureKind::Viewmodel,
        label: "fun_viewmodel",
        shader_feature_mask: 0,
        data_flag_mask: DATA_FLAG_EMISSIVE_BOOST,
        prewarm: true,
    },
];

#[derive(Debug, Clone, PartialEq, Eq, Resource)]
pub struct RenderPipelineSignatureCatalog {
    signatures: Vec<RenderPipelineSignatureDescriptor>,
}

impl Default for RenderPipelineSignatureCatalog {
    fn default() -> Self {
        Self::prewarmed_defaults()
    }
}

impl RenderPipelineSignatureCatalog {
    pub fn prewarmed_defaults() -> Self {
        Self {
            signatures: FUN_RENDER_PIPELINE_SIGNATURES
                .iter()
                .copied()
                .filter(|descriptor| descriptor.prewarm)
                .collect(),
        }
    }

    pub fn signatures(&self) -> &[RenderPipelineSignatureDescriptor] {
        &self.signatures
    }

    pub fn signature_count(&self) -> usize {
        self.signatures.len()
    }

    pub fn contains(&self, signature: FunShaderPipelineSignature) -> bool {
        self.signatures
            .iter()
            .any(|descriptor| descriptor.signature == signature)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderPipelineFeatureRequest {
    pub shader_feature_mask: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderPipelineFeaturePolicy {
    pub shader_feature_mask: u32,
    pub data_flag_mask: u32,
}

pub fn clamp_pipeline_features_for_visual_importance(
    visual_importance: VisualImportance,
    request: RenderPipelineFeatureRequest,
) -> RenderPipelineFeaturePolicy {
    let optional_features = PIPELINE_FEATURE_PARALLAX_MAPPING
        | PIPELINE_FEATURE_CLEARCOAT
        | PIPELINE_FEATURE_EMISSIVE_BOOST
        | PIPELINE_FEATURE_WIND_VERTEX_ANIMATION;
    let alpha_feature = request.shader_feature_mask & PIPELINE_FEATURE_ALPHA_TEST;
    if matches!(
        visual_importance,
        VisualImportance::Background | VisualImportance::SetDressing
    ) {
        RenderPipelineFeaturePolicy {
            shader_feature_mask: alpha_feature,
            data_flag_mask: (request.shader_feature_mask & optional_features) << 16,
        }
    } else {
        RenderPipelineFeaturePolicy {
            shader_feature_mask: request.shader_feature_mask,
            data_flag_mask: 0,
        }
    }
}

pub fn pipeline_signature_for_render_class(
    render_path: FunRenderPath,
    render_geometry_class: RenderGeometryClass,
    fun_geometry_class: FunGeometryClass,
    material_class: FunMaterialClass,
) -> FunShaderPipelineSignature {
    if render_path == FunRenderPath::Viewmodel
        || render_geometry_class == RenderGeometryClass::Viewmodel
        || fun_geometry_class == FunGeometryClass::Viewmodel
        || material_class == FunMaterialClass::Viewmodel
    {
        return VIEWMODEL_PIPELINE;
    }
    if material_class == FunMaterialClass::Transparent
        || render_geometry_class == RenderGeometryClass::TransparentRaster
    {
        return TRANSPARENT_PIPELINE;
    }
    if matches!(
        render_geometry_class,
        RenderGeometryClass::MeshletStaticDense
            | RenderGeometryClass::MeshletDynamicDense
            | RenderGeometryClass::VirtualStaticCluster
    ) || render_path.uses_meshlet()
    {
        return DENSE_MESHLET_CLUSTER_PIPELINE;
    }
    if matches!(
        render_geometry_class,
        RenderGeometryClass::GpuCulledDynamicRaster
    ) || !fun_geometry_class.is_static()
    {
        return DYNAMIC_OPAQUE_PIPELINE;
    }
    SIMPLE_OPAQUE_STATIC_PIPELINE
}

pub fn mesh_format_for_render_class(
    render_path: FunRenderPath,
    render_geometry_class: RenderGeometryClass,
) -> FunMeshletFormat {
    if render_geometry_class == RenderGeometryClass::VirtualStaticCluster {
        FunMeshletFormat::ClusterMesh
    } else if render_path.uses_meshlet() {
        FunMeshletFormat::MeshletMesh
    } else {
        FunMeshletFormat::RasterMesh
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RenderBatchShadowMode {
    #[default]
    None,
    ReceivesOnly,
    Casts,
    CastsAndReceives,
}

impl RenderBatchShadowMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ReceivesOnly => "receives_only",
            Self::Casts => "casts",
            Self::CastsAndReceives => "casts_and_receives",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextureTableId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RenderBatchKey {
    pub pipeline_signature: FunShaderPipelineSignature,
    pub material_signature: FunMaterialSignature,
    pub mesh_format: FunMeshletFormat,
    pub geometry_class: RenderGeometryClass,
    pub pass_type: FunRenderPhaseKind,
    pub shadow_mode: RenderBatchShadowMode,
    pub texture_table_id: TextureTableId,
}

impl RenderBatchKey {
    pub const fn to_draw_bucket_key(self, render_path: FunRenderPath) -> FunDrawBucketKey {
        FunDrawBucketKey {
            phase: self.pass_type,
            material_signature: self.material_signature,
            shader_pipeline_signature: self.pipeline_signature,
            meshlet_format: self.mesh_format,
            texture_table: texture_table_compatibility(self.texture_table_id),
            depth_prepass: depth_prepass_for_shadow_mode(self.shadow_mode),
            transparency: transparency_for_phase(self.pass_type),
            skeletal: skeletal_for_geometry_class(self.geometry_class),
            render_path,
        }
    }
}

pub fn render_batch_key_for_catalog_entry(
    entry: &RenderCatalogEntry,
    render_path: FunRenderPath,
    render_geometry_class: RenderGeometryClass,
    fun_geometry_class: FunGeometryClass,
    material_class: FunMaterialClass,
    material_policy: MaterialInstancePolicy,
) -> RenderBatchKey {
    RenderBatchKey {
        pipeline_signature: pipeline_signature_for_render_class(
            render_path,
            render_geometry_class,
            fun_geometry_class,
            material_class,
        ),
        material_signature: material_policy.material_signature,
        mesh_format: mesh_format_for_render_class(render_path, render_geometry_class),
        geometry_class: render_geometry_class,
        pass_type: pass_type_for_render_class(render_geometry_class, material_class),
        shadow_mode: shadow_mode_for_lighting(entry.lighting),
        texture_table_id: texture_table_id_for_material_key(material_policy.shared_material_key),
    }
}

pub fn sort_render_batch_keys(keys: &mut [RenderBatchKey]) {
    keys.sort_unstable();
}

pub fn count_unique_pipeline_signatures(keys: &[RenderBatchKey]) -> usize {
    keys.iter()
        .map(|key| key.pipeline_signature)
        .collect::<BTreeSet<_>>()
        .len()
}

pub fn signature_bucket_u32(signature: u64) -> u32 {
    let low = signature as u32;
    let high = (signature >> 32) as u32;
    low ^ high
}

const fn pass_type_for_render_class(
    render_geometry_class: RenderGeometryClass,
    material_class: FunMaterialClass,
) -> FunRenderPhaseKind {
    if matches!(
        render_geometry_class,
        RenderGeometryClass::TransparentRaster | RenderGeometryClass::Viewmodel
    ) || matches!(
        material_class,
        FunMaterialClass::Transparent | FunMaterialClass::Viewmodel
    ) {
        if matches!(render_geometry_class, RenderGeometryClass::Viewmodel)
            || matches!(material_class, FunMaterialClass::Viewmodel)
        {
            FunRenderPhaseKind::MainOpaque
        } else {
            FunRenderPhaseKind::Transparent
        }
    } else {
        FunRenderPhaseKind::MainOpaque
    }
}

const fn shadow_mode_for_lighting(lighting: LightingParticipation) -> RenderBatchShadowMode {
    if lighting.contains(LightingParticipation::DIRECT_SHADOW)
        && lighting.contains(LightingParticipation::GI)
    {
        RenderBatchShadowMode::CastsAndReceives
    } else if lighting.contains(LightingParticipation::DIRECT_SHADOW) {
        RenderBatchShadowMode::Casts
    } else if lighting.contains(LightingParticipation::GI) {
        RenderBatchShadowMode::ReceivesOnly
    } else {
        RenderBatchShadowMode::None
    }
}

const fn texture_table_id_for_material_key(key: MaterialKey) -> TextureTableId {
    match key.alpha_mode {
        MaterialAlphaModeKey::Opaque => TextureTableId(0),
        MaterialAlphaModeKey::Mask => TextureTableId(1),
        MaterialAlphaModeKey::Blend => TextureTableId(2),
    }
}

const fn texture_table_compatibility(
    texture_table_id: TextureTableId,
) -> FunTextureTableCompatibility {
    match texture_table_id.0 {
        0 => FunTextureTableCompatibility::SharedTable,
        1 => FunTextureTableCompatibility::SameAtlas,
        2 => FunTextureTableCompatibility::UniqueTable,
        _ => FunTextureTableCompatibility::Incompatible,
    }
}

const fn depth_prepass_for_shadow_mode(shadow_mode: RenderBatchShadowMode) -> FunDepthPrepassMode {
    match shadow_mode {
        RenderBatchShadowMode::None | RenderBatchShadowMode::ReceivesOnly => {
            FunDepthPrepassMode::DepthOnly
        }
        RenderBatchShadowMode::Casts | RenderBatchShadowMode::CastsAndReceives => {
            FunDepthPrepassMode::ShadowDepth
        }
    }
}

const fn transparency_for_phase(phase: FunRenderPhaseKind) -> FunTransparencyMode {
    match phase {
        FunRenderPhaseKind::Transparent => FunTransparencyMode::AlphaBlend,
        _ => FunTransparencyMode::Opaque,
    }
}

const fn skeletal_for_geometry_class(geometry_class: RenderGeometryClass) -> FunSkeletalMode {
    match geometry_class {
        RenderGeometryClass::Viewmodel => FunSkeletalMode::Viewmodel,
        _ => FunSkeletalMode::Static,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_policy_promotes_exact_preset_without_unique_handle() {
        let preset =
            game_shared::material_preset(game_shared::MATERIAL_WALL).expect("wall preset exists");
        let policy = material_policy_for_stream_color(Some(PackedColorRgba8 {
            r: preset.srgb[0],
            g: preset.srgb[1],
            b: preset.srgb[2],
            a: preset.srgb[3],
        }));

        assert_eq!(
            policy.shared_material_key,
            MaterialKey::from_preset(*preset)
        );
        assert_eq!(policy.instance_tint, MaterialInstanceTint::WHITE);
        assert!(!policy.creates_unique_standard_material);
        assert_eq!(policy.reason, MaterialConsolidationReason::ExactPreset);
    }

    #[test]
    fn color_variation_uses_shared_material_and_instance_tint() {
        let policy = material_policy_for_stream_color(Some(PackedColorRgba8::srgb(11, 22, 33)));

        assert_eq!(
            policy.shared_material_key,
            MaterialKey::OPAQUE_INSTANCE_TINT
        );
        assert_eq!(
            policy.instance_tint,
            MaterialInstanceTint::from_rgba8([11, 22, 33, 255])
        );
        assert!(!policy.creates_unique_standard_material);
        assert_eq!(
            policy.reason,
            MaterialConsolidationReason::InstanceTintOnSharedOpaqueMaterial
        );
    }

    #[test]
    fn background_features_move_to_data_flags_instead_of_shader_defs() {
        let request = RenderPipelineFeatureRequest {
            shader_feature_mask: PIPELINE_FEATURE_PARALLAX_MAPPING
                | PIPELINE_FEATURE_CLEARCOAT
                | PIPELINE_FEATURE_ALPHA_TEST,
        };
        let background =
            clamp_pipeline_features_for_visual_importance(VisualImportance::Background, request);
        let foreground =
            clamp_pipeline_features_for_visual_importance(VisualImportance::Foreground, request);

        assert_eq!(background.shader_feature_mask, PIPELINE_FEATURE_ALPHA_TEST);
        assert_ne!(background.data_flag_mask, 0);
        assert_eq!(foreground.shader_feature_mask, request.shader_feature_mask);
        assert_eq!(foreground.data_flag_mask, 0);
    }

    #[test]
    fn render_pipeline_signatures_cover_the_required_pso_lanes() {
        let catalog = RenderPipelineSignatureCatalog::prewarmed_defaults();

        assert_eq!(catalog.signature_count(), 5);
        assert!(catalog.contains(SIMPLE_OPAQUE_STATIC_PIPELINE));
        assert!(catalog.contains(DENSE_MESHLET_CLUSTER_PIPELINE));
        assert!(catalog.contains(DYNAMIC_OPAQUE_PIPELINE));
        assert!(catalog.contains(TRANSPARENT_PIPELINE));
        assert!(catalog.contains(VIEWMODEL_PIPELINE));
    }

    #[test]
    fn render_batch_key_sorts_and_converts_to_draw_bucket_key() {
        let mut keys = [
            RenderBatchKey {
                pipeline_signature: TRANSPARENT_PIPELINE,
                material_signature: FunMaterialSignature(3),
                mesh_format: FunMeshletFormat::RasterMesh,
                geometry_class: RenderGeometryClass::TransparentRaster,
                pass_type: FunRenderPhaseKind::Transparent,
                shadow_mode: RenderBatchShadowMode::None,
                texture_table_id: TextureTableId(2),
            },
            RenderBatchKey {
                pipeline_signature: SIMPLE_OPAQUE_STATIC_PIPELINE,
                material_signature: FunMaterialSignature(1),
                mesh_format: FunMeshletFormat::RasterMesh,
                geometry_class: RenderGeometryClass::SimpleRaster,
                pass_type: FunRenderPhaseKind::MainOpaque,
                shadow_mode: RenderBatchShadowMode::CastsAndReceives,
                texture_table_id: TextureTableId(0),
            },
        ];

        sort_render_batch_keys(&mut keys);

        assert_eq!(keys[0].pipeline_signature, SIMPLE_OPAQUE_STATIC_PIPELINE);
        let draw_key = keys[0].to_draw_bucket_key(FunRenderPath::GpuCulledIndirect);
        assert_eq!(
            draw_key.shader_pipeline_signature,
            SIMPLE_OPAQUE_STATIC_PIPELINE
        );
        assert_eq!(draw_key.material_signature, FunMaterialSignature(1));
        assert_eq!(
            draw_key.texture_table,
            FunTextureTableCompatibility::SharedTable
        );
    }
}
