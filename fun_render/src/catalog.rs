use hashbrown::{HashMap, HashSet};

use bevy::{
    pbr::experimental::meshlet::{
        MESHLET_DEFAULT_VERTEX_POSITION_QUANTIZATION_FACTOR, MeshletMesh,
    },
    prelude::*,
};
use game_shared::{
    CatalogGeometry, DEMO_RENDER_CATALOG, LightingParticipation, RenderCatalogEntry,
    RenderCostClass, VisualImportance,
};
use thunder::prelude::WorldCatalogRef;

use crate::{
    ClientRenderConfig, CompiledWorldPackage, FunGeometryClass, FunMaterialClass,
    FunRenderDistanceBand, FunRenderPath, FunRenderPathArbiter, FunRenderPathInput,
    MaterialInstancePolicy, RenderBatchKey, RenderGeometryClass, RenderGeometryPolicy,
    RenderPipelineSignatureCatalog, material_policy_for_catalog_entry,
    render_batch_key_for_catalog_entry,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MaterialAlphaModeKey {
    #[default]
    Opaque,
    Mask,
    Blend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MaterialKey {
    pub base_color_rgba8: [u8; 4],
    pub roughness_bucket: u8,
    pub metallic_bucket: u8,
    pub alpha_mode: MaterialAlphaModeKey,
    pub material_preset_id: Option<u32>,
}

impl MaterialKey {
    pub const DEFAULT_ROUGHNESS_BUCKET: u8 = 128;
    pub const DEFAULT_METALLIC_BUCKET: u8 = 0;
    pub const DEFAULT_DEBUG: Self = Self {
        base_color_rgba8: [180, 180, 180, 255],
        roughness_bucket: Self::DEFAULT_ROUGHNESS_BUCKET,
        metallic_bucket: Self::DEFAULT_METALLIC_BUCKET,
        alpha_mode: MaterialAlphaModeKey::Opaque,
        material_preset_id: None,
    };
    pub const OPAQUE_INSTANCE_TINT: Self = Self {
        base_color_rgba8: [255, 255, 255, 255],
        roughness_bucket: Self::DEFAULT_ROUGHNESS_BUCKET,
        metallic_bucket: Self::DEFAULT_METALLIC_BUCKET,
        alpha_mode: MaterialAlphaModeKey::Opaque,
        material_preset_id: None,
    };

    pub const fn from_rgba8(base_color_rgba8: [u8; 4]) -> Self {
        Self {
            base_color_rgba8,
            roughness_bucket: Self::DEFAULT_ROUGHNESS_BUCKET,
            metallic_bucket: Self::DEFAULT_METALLIC_BUCKET,
            alpha_mode: MaterialAlphaModeKey::Opaque,
            material_preset_id: None,
        }
    }

    pub const fn from_preset(preset: game_shared::MaterialPreset) -> Self {
        Self {
            base_color_rgba8: preset.srgb,
            roughness_bucket: Self::DEFAULT_ROUGHNESS_BUCKET,
            metallic_bucket: Self::DEFAULT_METALLIC_BUCKET,
            alpha_mode: MaterialAlphaModeKey::Opaque,
            material_preset_id: Some(preset.id.0),
        }
    }
}

#[derive(Debug, Resource)]
pub struct MaterialHandleCache {
    handles: HashMap<MaterialKey, Handle<StandardMaterial>>,
    fallback_key: MaterialKey,
}

impl Default for MaterialHandleCache {
    fn default() -> Self {
        Self {
            handles: HashMap::new(),
            fallback_key: MaterialKey::DEFAULT_DEBUG,
        }
    }
}

impl MaterialHandleCache {
    pub fn get(&self, key: MaterialKey) -> Option<Handle<StandardMaterial>> {
        self.handles.get(&key).cloned()
    }

    pub fn get_or_fallback(&self, key: MaterialKey) -> Option<Handle<StandardMaterial>> {
        self.get(key).or_else(|| self.get(self.fallback_key))
    }

    pub fn insert_existing(
        &mut self,
        key: MaterialKey,
        handle: Handle<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        self.handles.entry(key).or_insert(handle).clone()
    }

    pub fn get_or_insert_with_assets(
        &mut self,
        key: MaterialKey,
        materials: &mut Assets<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        if let Some(handle) = self.get(key) {
            return handle;
        }
        let handle = materials.add(standard_material_from_key(key));
        self.insert_existing(key, handle)
    }

    pub fn len(&self) -> usize {
        self.handles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }
}

#[derive(Debug, Resource, Default)]
pub struct WorldRenderCatalog {
    assets: HashMap<u32, CompiledRenderAsset>,
    materials: HashMap<u32, Handle<StandardMaterial>>,
}

#[derive(Debug)]
pub struct CompiledRenderAsset {
    #[cfg_attr(not(all(feature = "diagnostics", debug_assertions)), allow(dead_code))]
    pub entry: &'static RenderCatalogEntry,
    pub raster_mesh: Option<Handle<Mesh>>,
    pub meshlet_mesh: Option<Handle<MeshletMesh>>,
    pub ray_proxy: Option<Handle<Mesh>>,
    pub material: Option<Handle<StandardMaterial>>,
    pub geometry_class: RenderGeometryClass,
    pub fun_geometry_class: FunGeometryClass,
    pub render_path: FunRenderPath,
    pub material_policy: MaterialInstancePolicy,
    pub render_batch_key: RenderBatchKey,
    #[cfg_attr(not(all(feature = "diagnostics", debug_assertions)), allow(dead_code))]
    pub triangle_count: usize,
}

impl WorldRenderCatalog {
    pub fn lookup(&self, catalog_ref: WorldCatalogRef) -> Option<&CompiledRenderAsset> {
        self.assets.get(&catalog_ref.asset_id)
    }

    pub fn compiled_assets(&self) -> impl Iterator<Item = &CompiledRenderAsset> {
        self.assets.values()
    }

    pub fn material_handle(&self, material_id: u32) -> Option<Handle<StandardMaterial>> {
        self.materials.get(&material_id).cloned()
    }

    #[cfg(test)]
    pub(crate) fn insert_compiled_for_test(&mut self, asset: CompiledRenderAsset) {
        self.assets.insert(asset.entry.asset_id.0, asset);
    }

    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    pub fn len(&self) -> usize {
        self.assets.len()
    }

    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }
}

pub fn prewarm_world_render_catalog(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut meshlet_meshes: Option<ResMut<Assets<MeshletMesh>>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    render_config: Res<ClientRenderConfig>,
) {
    let mut catalog_render_config = *render_config;
    if catalog_render_config.meshlets_enabled && meshlet_meshes.is_none() {
        catalog_render_config.meshlets_enabled = false;
        game_shared::fun_diag_warn!(
            target: "fun::render_catalog",
            geometry_policy = ?render_config.geometry_policy,
            "meshlet assets unavailable during catalog prewarm; falling back to raster paths"
        );
    }

    let compiled_package = CompiledWorldPackage::demo_package();
    let _occlusion_cells = compiled_package
        .static_assets
        .iter()
        .map(|asset| asset.occlusion_cell)
        .collect::<HashSet<_>>()
        .len();
    let _dense_assets = compiled_package
        .static_assets
        .iter()
        .filter(|asset| {
            matches!(
                asset.cost_class,
                RenderCostClass::DenseStatic | RenderCostClass::DenseDynamic
            )
        })
        .count();
    let _package_asset_ids = compiled_package
        .static_assets
        .iter()
        .map(|asset| asset.asset_id.0.to_string())
        .collect::<Vec<_>>()
        .join(",");
    game_shared::fun_diag_info!(
        target: "fun::render_catalog",
        package_id = compiled_package.id.0,
        revision = compiled_package.revision,
        content_hash = format!("{:016x}", compiled_package.content_hash),
        static_assets = compiled_package.static_assets.len(),
        dense_assets = _dense_assets,
        occlusion_cells = _occlusion_cells,
        asset_ids = _package_asset_ids,
        "compiled world package ready"
    );

    let mut catalog = WorldRenderCatalog::default();
    let mut material_cache = MaterialHandleCache::default();
    material_cache.get_or_insert_with_assets(MaterialKey::DEFAULT_DEBUG, &mut materials);
    material_cache.get_or_insert_with_assets(MaterialKey::OPAQUE_INSTANCE_TINT, &mut materials);

    for preset in game_shared::MATERIAL_PRESETS {
        let key = MaterialKey::from_preset(*preset);
        let handle = material_cache.get_or_insert_with_assets(key, &mut materials);
        catalog.materials.insert(preset.id.0, handle);
    }

    for entry in DEMO_RENDER_CATALOG {
        let triangle_count = entry
            .geometry
            .map(geometry_triangle_count)
            .unwrap_or_default();
        let fun_geometry_class = fun_geometry_class(entry);
        let render_path_selection = select_render_path(
            entry,
            triangle_count,
            fun_geometry_class,
            &catalog_render_config,
        );
        let render_path = render_path_selection.render_path;
        let geometry_class = render_path_selection.geometry_class;
        let material_policy = material_policy_for_catalog_entry(entry);
        let render_batch_key = render_batch_key_for_catalog_entry(
            entry,
            render_path,
            geometry_class,
            fun_geometry_class,
            material_class(entry),
            material_policy,
        );
        let material = material_cache.get_or_fallback(material_policy.shared_material_key);

        let raster_mesh = if geometry_class.uses_raster_mesh() {
            entry.geometry.map(mesh_from_catalog_geometry).map(|mesh| {
                let mesh = mesh.with_generated_tangents().unwrap_or_else(|error| {
                    panic!("failed to generate {} raster tangents: {error}", entry.name)
                });
                meshes.add(mesh)
            })
        } else {
            None
        };

        let meshlet_mesh = if geometry_class.uses_meshlet() {
            if let Some(meshlet_meshes) = meshlet_meshes.as_deref_mut() {
                entry.geometry.map(mesh_from_catalog_geometry).map(|mesh| {
                    let meshlet = MeshletMesh::from_mesh(
                        &mesh,
                        MESHLET_DEFAULT_VERTEX_POSITION_QUANTIZATION_FACTOR,
                    )
                    .unwrap_or_else(|error| {
                        panic!("failed to build {} meshlet mesh: {error}", entry.name)
                    });
                    meshlet_meshes.add(meshlet)
                })
            } else {
                game_shared::fun_diag_warn!(
                    target: "fun::render_catalog",
                    asset_id = entry.asset_id.0,
                    name = entry.name,
                    geometry_class = ?geometry_class,
                    render_path = render_path.as_str(),
                    "meshlet render path selected without meshlet asset storage"
                );
                None
            }
        } else {
            None
        };

        let ray_proxy = entry.geometry.map(mesh_from_catalog_geometry).map(|mesh| {
            let mesh = mesh.with_generated_tangents().unwrap_or_else(|error| {
                panic!(
                    "failed to generate {} ray proxy tangents: {error}",
                    entry.name
                )
            });
            meshes.add(mesh)
        });
        game_shared::fun_diag_info!(
            target: "fun::render_catalog",
            asset_id = entry.asset_id.0,
            name = entry.name,
            visual_mesh = entry.visual_mesh,
            meshlet_mesh = entry.meshlet_mesh,
            fallback_raster_mesh = entry.fallback_raster_mesh,
            ray_proxy = entry.ray_proxy,
            cost_class = ?entry.cost_class,
            geometry_class = ?geometry_class,
            fun_geometry_class = fun_geometry_class.as_str(),
            render_path = render_path.as_str(),
            pipeline_signature = render_batch_key.pipeline_signature.0,
            material_signature = render_batch_key.material_signature.0,
            mesh_format = render_batch_key.mesh_format.as_str(),
            pass_type = render_batch_key.pass_type.as_str(),
            shadow_mode = render_batch_key.shadow_mode.as_str(),
            texture_table_id = render_batch_key.texture_table_id.0,
            triangles = triangle_count,
            has_raster = raster_mesh.is_some(),
            has_meshlet = meshlet_mesh.is_some(),
            has_ray_proxy = ray_proxy.is_some(),
            has_collider = entry.collider.is_some(),
            "prewarmed world render catalog asset"
        );

        catalog.assets.insert(
            entry.asset_id.0,
            CompiledRenderAsset {
                entry,
                raster_mesh,
                meshlet_mesh,
                ray_proxy,
                material,
                geometry_class,
                fun_geometry_class,
                render_path,
                material_policy,
                render_batch_key,
                triangle_count,
            },
        );
    }

    game_shared::fun_diag_info!(
        target: "fun::render_catalog",
        asset_count = catalog.assets.len(),
        material_count = catalog.materials.len(),
        geometry_policy = ?catalog_render_config.geometry_policy,
        meshlets_enabled = catalog_render_config.meshlets_enabled,
        meshlet_min_triangles = catalog_render_config.meshlet_min_triangles,
        "world render catalog prewarmed"
    );
    let pipeline_catalog = RenderPipelineSignatureCatalog::prewarmed_defaults();
    game_shared::fun_diag_info!(
        target: "fun::render_catalog",
        pipeline_signature_count = pipeline_catalog.signature_count(),
        material_handle_count = material_cache.len(),
        "world render material and pipeline signatures prewarmed"
    );
    commands.insert_resource(compiled_package);
    commands.insert_resource(material_cache);
    commands.insert_resource(pipeline_catalog);
    commands.insert_resource(catalog);
}

fn standard_material_from_key(key: MaterialKey) -> StandardMaterial {
    let [r, g, b, a] = key.base_color_rgba8;
    StandardMaterial {
        base_color: Color::srgba_u8(r, g, b, a),
        perceptual_roughness: f32::from(key.roughness_bucket) / f32::from(u8::MAX),
        metallic: f32::from(key.metallic_bucket) / f32::from(u8::MAX),
        alpha_mode: alpha_mode_from_key(key.alpha_mode),
        ..default()
    }
}

const fn alpha_mode_from_key(alpha_mode: MaterialAlphaModeKey) -> AlphaMode {
    match alpha_mode {
        MaterialAlphaModeKey::Opaque => AlphaMode::Opaque,
        MaterialAlphaModeKey::Mask => AlphaMode::Mask(0.5),
        MaterialAlphaModeKey::Blend => AlphaMode::Blend,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CatalogRenderPathSelection {
    render_path: FunRenderPath,
    geometry_class: RenderGeometryClass,
}

fn select_render_path(
    entry: &RenderCatalogEntry,
    triangle_count: usize,
    geometry_class: FunGeometryClass,
    render_config: &ClientRenderConfig,
) -> CatalogRenderPathSelection {
    if entry.geometry.is_none() || entry.cost_class == RenderCostClass::RayProxyOnly {
        return CatalogRenderPathSelection {
            render_path: FunRenderPath::RayProxyOnly,
            geometry_class: RenderGeometryClass::RayProxyOnly,
        };
    }

    if entry.cost_class == RenderCostClass::Viewmodel {
        return CatalogRenderPathSelection {
            render_path: FunRenderPath::Viewmodel,
            geometry_class: RenderGeometryClass::Viewmodel,
        };
    }

    if entry.cost_class == RenderCostClass::Transparent {
        return CatalogRenderPathSelection {
            render_path: FunRenderPath::StandardRaster,
            geometry_class: RenderGeometryClass::TransparentRaster,
        };
    }

    let is_static = entry.cost_class != RenderCostClass::DenseDynamic;
    let arbiter = FunRenderPathArbiter {
        static_meshlet_min_triangles: render_config.meshlet_min_triangles as u32,
        dynamic_meshlet_min_triangles: render_config.meshlet_min_triangles.saturating_mul(4) as u32,
        ..default()
    };
    let input =
        catalog_render_path_input(entry, triangle_count, geometry_class, is_static, arbiter);

    let render_path = match render_config.geometry_policy {
        RenderGeometryPolicy::RasterOnly => raster_only_path(input, arbiter),
        RenderGeometryPolicy::GpuCulledRaster => gpu_culled_raster_path(input, arbiter),
        RenderGeometryPolicy::MeshletWhereSupported => {
            meshlet_where_supported_path(input, arbiter, render_config.meshlets_enabled)
        }
        RenderGeometryPolicy::VirtualStaticExperimental => {
            if virtual_static_cluster_admitted(entry, input, render_config) {
                FunRenderPath::GpuCulledIndirect
            } else {
                meshlet_where_supported_path(input, arbiter, render_config.meshlets_enabled)
            }
        }
        RenderGeometryPolicy::Hybrid => {
            meshlet_where_supported_path(input, arbiter, render_config.meshlets_enabled)
        }
    };

    CatalogRenderPathSelection {
        render_path,
        geometry_class: render_geometry_class_for_selection(
            entry,
            input,
            render_path,
            render_config,
        ),
    }
}

fn catalog_render_path_input(
    entry: &RenderCatalogEntry,
    triangle_count: usize,
    geometry_class: FunGeometryClass,
    is_static: bool,
    arbiter: FunRenderPathArbiter,
) -> FunRenderPathInput {
    let instance_count = catalog_instance_count_estimate(entry);
    FunRenderPathInput {
        triangle_count: triangle_count as u32,
        meshlet_count: triangle_count.div_ceil(64).max(1) as u32,
        projected_screen_area: projected_screen_area_estimate(entry),
        material_class: material_class(entry),
        geometry_class,
        is_static,
        instance_count,
        shared_mesh_material: true,
        simple_transforms: true,
        overdraw_estimate: overdraw_estimate(entry),
        distance_band: distance_band(entry),
        ray_proxy_only: entry.cost_class == RenderCostClass::RayProxyOnly,
        viewmodel: entry.cost_class == RenderCostClass::Viewmodel,
        high_cpu_culling_cost: matches!(
            entry.cost_class,
            RenderCostClass::DenseStatic | RenderCostClass::DenseDynamic
        ) || instance_count >= arbiter.gpu_culled_min_instances,
        stable_buffer_layout: true,
        ..Default::default()
    }
}

fn raster_only_path(input: FunRenderPathInput, arbiter: FunRenderPathArbiter) -> FunRenderPath {
    let path = arbiter.decide(FunRenderPathInput {
        triangle_count: 0,
        meshlet_count: 0,
        high_cpu_culling_cost: false,
        ..input
    });
    if path.uses_meshlet() || path == FunRenderPath::GpuCulledIndirect {
        FunRenderPath::StandardRaster
    } else {
        path
    }
}

fn gpu_culled_raster_path(
    input: FunRenderPathInput,
    arbiter: FunRenderPathArbiter,
) -> FunRenderPath {
    let path = arbiter.decide(FunRenderPathInput {
        triangle_count: 0,
        meshlet_count: 0,
        ..input
    });
    if path.uses_meshlet() {
        FunRenderPath::StandardRaster
    } else {
        path
    }
}

fn meshlet_where_supported_path(
    input: FunRenderPathInput,
    arbiter: FunRenderPathArbiter,
    meshlets_enabled: bool,
) -> FunRenderPath {
    if meshlets_enabled {
        arbiter.decide(input)
    } else {
        gpu_culled_raster_path(input, arbiter)
    }
}

fn render_geometry_class_for_selection(
    entry: &RenderCatalogEntry,
    input: FunRenderPathInput,
    render_path: FunRenderPath,
    render_config: &ClientRenderConfig,
) -> RenderGeometryClass {
    if render_config.geometry_policy == RenderGeometryPolicy::VirtualStaticExperimental
        && render_path == FunRenderPath::GpuCulledIndirect
        && virtual_static_cluster_admitted(entry, input, render_config)
    {
        return RenderGeometryClass::VirtualStaticCluster;
    }

    match render_path {
        FunRenderPath::MeshletStaticDense => RenderGeometryClass::MeshletStaticDense,
        FunRenderPath::MeshletDynamicDense => RenderGeometryClass::MeshletDynamicDense,
        FunRenderPath::RayProxyOnly => RenderGeometryClass::RayProxyOnly,
        FunRenderPath::Viewmodel => RenderGeometryClass::Viewmodel,
        FunRenderPath::InstancedRaster if input.is_static => {
            RenderGeometryClass::InstancedStaticRaster
        }
        FunRenderPath::GpuCulledIndirect if input.is_static => {
            RenderGeometryClass::GpuCulledStaticRaster
        }
        FunRenderPath::GpuCulledIndirect => RenderGeometryClass::GpuCulledDynamicRaster,
        FunRenderPath::StandardRaster if input.material_class == FunMaterialClass::Transparent => {
            RenderGeometryClass::TransparentRaster
        }
        FunRenderPath::StandardRaster if input.geometry_class == FunGeometryClass::Foliage => {
            RenderGeometryClass::FoliageAggregate
        }
        FunRenderPath::StandardRaster | FunRenderPath::InstancedRaster => {
            RenderGeometryClass::SimpleRaster
        }
        FunRenderPath::CefUi | FunRenderPath::DebugOnly => RenderGeometryClass::SimpleRaster,
    }
}

fn catalog_instance_count_estimate(entry: &RenderCatalogEntry) -> u32 {
    match entry.cost_class {
        RenderCostClass::DenseStatic => 256,
        RenderCostClass::DenseDynamic => 192,
        RenderCostClass::Simple | RenderCostClass::Tiny => match entry.visual_importance {
            VisualImportance::SetDressing => 32,
            VisualImportance::Background => 64,
            _ => 1,
        },
        RenderCostClass::Transparent
        | RenderCostClass::Viewmodel
        | RenderCostClass::RayProxyOnly => 1,
    }
}

fn virtual_static_cluster_admitted(
    entry: &RenderCatalogEntry,
    input: FunRenderPathInput,
    render_config: &ClientRenderConfig,
) -> bool {
    input.is_static
        && input.geometry_class == FunGeometryClass::StaticOpaqueDense
        && matches!(
            input.material_class,
            FunMaterialClass::OpaqueSimple
                | FunMaterialClass::OpaqueComplex
                | FunMaterialClass::Emissive
        )
        && input.triangle_count >= render_config.meshlet_min_triangles.saturating_mul(8) as u32
        && input.instance_count >= FunRenderPathArbiter::default().gpu_culled_min_instances
        && input.projected_screen_area >= 0.10
        && entry.occlusion_cell.0 != 0
        && entry.cost_class == RenderCostClass::DenseStatic
}

fn fun_geometry_class(entry: &RenderCatalogEntry) -> FunGeometryClass {
    match entry.cost_class {
        RenderCostClass::Tiny | RenderCostClass::Simple => FunGeometryClass::StaticOpaqueSimple,
        RenderCostClass::DenseStatic => FunGeometryClass::StaticOpaqueDense,
        RenderCostClass::DenseDynamic => FunGeometryClass::DynamicOpaque,
        RenderCostClass::Transparent => FunGeometryClass::Decal,
        RenderCostClass::Viewmodel => FunGeometryClass::Viewmodel,
        RenderCostClass::RayProxyOnly => FunGeometryClass::StaticOpaqueSimple,
    }
}

fn material_class(entry: &RenderCatalogEntry) -> FunMaterialClass {
    match entry.cost_class {
        RenderCostClass::Transparent => FunMaterialClass::Transparent,
        RenderCostClass::Viewmodel => FunMaterialClass::Viewmodel,
        _ if entry.lighting.contains(LightingParticipation::EMISSIVE) => FunMaterialClass::Emissive,
        _ if entry.lighting.contains(LightingParticipation::SPECULAR) => {
            FunMaterialClass::OpaqueComplex
        }
        _ => FunMaterialClass::OpaqueSimple,
    }
}

fn projected_screen_area_estimate(entry: &RenderCatalogEntry) -> f32 {
    match entry.visual_importance {
        VisualImportance::Foreground => 0.35,
        VisualImportance::GameplayCover | VisualImportance::Navigation => 0.12,
        VisualImportance::SetDressing => 0.06,
        VisualImportance::Background => 0.02,
    }
}

fn overdraw_estimate(entry: &RenderCatalogEntry) -> f32 {
    match entry.visual_importance {
        VisualImportance::Foreground => 1.4,
        VisualImportance::GameplayCover => 1.2,
        VisualImportance::Navigation => 1.0,
        VisualImportance::SetDressing => 1.1,
        VisualImportance::Background => 0.8,
    }
}

fn distance_band(entry: &RenderCatalogEntry) -> FunRenderDistanceBand {
    match entry.visual_importance {
        VisualImportance::Foreground | VisualImportance::GameplayCover => {
            FunRenderDistanceBand::Near
        }
        VisualImportance::Navigation | VisualImportance::SetDressing => FunRenderDistanceBand::Mid,
        VisualImportance::Background => FunRenderDistanceBand::Far,
    }
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
pub fn catalog_ref_summary(catalog_ref: Option<WorldCatalogRef>) -> String {
    catalog_ref
        .map(|value| {
            format!(
                "asset={} material={} collider={}",
                value.asset_id, value.material_id, value.collider_id
            )
        })
        .unwrap_or_else(|| "none".to_owned())
}

pub fn warn_missing_catalog_ref(_catalog_ref: WorldCatalogRef, _name: &str) {
    game_shared::fun_diag_warn!(
        target: "fun::render_catalog",
        asset_id = _catalog_ref.asset_id,
        material_id = _catalog_ref.material_id,
        collider_id = _catalog_ref.collider_id,
        name = _name,
        "streamed entity referenced missing render catalog asset"
    );
}

fn geometry_triangle_count(geometry: CatalogGeometry) -> usize {
    match geometry {
        CatalogGeometry::Plane { .. } => 2,
        CatalogGeometry::Cuboid { .. } => 12,
    }
}

fn mesh_from_catalog_geometry(geometry: CatalogGeometry) -> Mesh {
    match geometry {
        CatalogGeometry::Plane { size } => Plane3d::default().mesh().size(size[0], size[2]).build(),
        CatalogGeometry::Cuboid { size } => Cuboid::new(size[0], size[1], size[2]).mesh().build(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_render_config(policy: RenderGeometryPolicy) -> ClientRenderConfig {
        let mut config = ClientRenderConfig::from_env();
        config.geometry_policy = policy;
        config.meshlets_enabled = true;
        config.meshlet_min_triangles = 512;
        config
    }

    fn test_catalog_entry(
        cost_class: RenderCostClass,
        visual_importance: VisualImportance,
    ) -> RenderCatalogEntry {
        RenderCatalogEntry {
            cost_class,
            visual_importance,
            lighting: LightingParticipation::DIRECT_SHADOW
                .union(LightingParticipation::GI)
                .union(LightingParticipation::RAY_PROXY),
            ..*game_shared::demo_catalog_entry(game_shared::ASSET_WALL)
                .expect("demo catalog wall exists")
        }
    }

    #[test]
    fn material_handle_cache_reuses_equivalent_material_keys() {
        let mut materials = Assets::<StandardMaterial>::default();
        let key = MaterialKey::from_rgba8([82, 89, 107, 255]);
        let mut cache = MaterialHandleCache::default();

        let first = cache.get_or_insert_with_assets(key, &mut materials);
        let second = cache.get_or_insert_with_assets(key, &mut materials);

        assert_eq!(first, second);
        assert_eq!(cache.len(), 1);
        assert_eq!(materials.len(), 1);
    }

    #[test]
    fn catalog_prewarm_allows_disabled_meshlets_without_meshlet_assets() {
        let mut config = test_render_config(RenderGeometryPolicy::MeshletWhereSupported);
        config.meshlets_enabled = false;
        let mut app = App::new();
        app.insert_resource(Assets::<Mesh>::default());
        app.insert_resource(Assets::<StandardMaterial>::default());
        app.insert_resource(config);
        app.add_systems(Update, prewarm_world_render_catalog);

        app.update();

        assert!(!app.world().contains_resource::<Assets<MeshletMesh>>());
        let catalog = app.world().resource::<WorldRenderCatalog>();
        assert!(
            catalog
                .compiled_assets()
                .all(|asset| !asset.geometry_class.uses_meshlet())
        );
        assert!(
            catalog
                .compiled_assets()
                .all(|asset| asset.meshlet_mesh.is_none())
        );
    }

    #[test]
    fn catalog_prewarm_falls_back_when_meshlet_assets_are_missing() {
        let config = test_render_config(RenderGeometryPolicy::MeshletWhereSupported);
        let mut app = App::new();
        app.insert_resource(Assets::<Mesh>::default());
        app.insert_resource(Assets::<StandardMaterial>::default());
        app.insert_resource(config);
        app.add_systems(Update, prewarm_world_render_catalog);

        app.update();

        assert!(!app.world().contains_resource::<Assets<MeshletMesh>>());
        let catalog = app.world().resource::<WorldRenderCatalog>();
        assert!(
            catalog
                .compiled_assets()
                .all(|asset| !asset.geometry_class.uses_meshlet())
        );
        assert!(
            catalog
                .compiled_assets()
                .all(|asset| asset.meshlet_mesh.is_none())
        );
    }

    #[test]
    fn catalog_meshlet_policy_requires_dense_static_opaque_candidate() {
        let config = test_render_config(RenderGeometryPolicy::MeshletWhereSupported);
        let dense = test_catalog_entry(RenderCostClass::DenseStatic, VisualImportance::Foreground);
        let dense_selection =
            select_render_path(&dense, 4_096, FunGeometryClass::StaticOpaqueDense, &config);

        assert_eq!(
            dense_selection.render_path,
            FunRenderPath::MeshletStaticDense
        );
        assert_eq!(
            dense_selection.geometry_class,
            RenderGeometryClass::MeshletStaticDense
        );

        let simple = test_catalog_entry(RenderCostClass::Simple, VisualImportance::Foreground);
        let simple_selection = select_render_path(
            &simple,
            4_096,
            FunGeometryClass::StaticOpaqueSimple,
            &config,
        );
        assert!(!simple_selection.render_path.uses_meshlet());

        let transparent =
            test_catalog_entry(RenderCostClass::Transparent, VisualImportance::Foreground);
        let transparent_selection =
            select_render_path(&transparent, 4_096, FunGeometryClass::Decal, &config);
        assert_eq!(
            transparent_selection.geometry_class,
            RenderGeometryClass::TransparentRaster
        );

        let background =
            test_catalog_entry(RenderCostClass::DenseStatic, VisualImportance::Background);
        let background_selection = select_render_path(
            &background,
            4_096,
            FunGeometryClass::StaticOpaqueDense,
            &config,
        );
        assert!(!background_selection.render_path.uses_meshlet());
    }

    #[test]
    fn catalog_gpu_culled_policy_classifies_static_and_dynamic_buckets() {
        let config = test_render_config(RenderGeometryPolicy::GpuCulledRaster);
        let static_entry = test_catalog_entry(
            RenderCostClass::DenseStatic,
            VisualImportance::GameplayCover,
        );
        let dynamic_entry = test_catalog_entry(
            RenderCostClass::DenseDynamic,
            VisualImportance::GameplayCover,
        );

        let static_selection = select_render_path(
            &static_entry,
            12,
            FunGeometryClass::StaticOpaqueDense,
            &config,
        );
        let dynamic_selection =
            select_render_path(&dynamic_entry, 12, FunGeometryClass::DynamicOpaque, &config);

        assert_eq!(
            static_selection.render_path,
            FunRenderPath::GpuCulledIndirect
        );
        assert_eq!(
            static_selection.geometry_class,
            RenderGeometryClass::GpuCulledStaticRaster
        );
        assert_eq!(
            dynamic_selection.render_path,
            FunRenderPath::GpuCulledIndirect
        );
        assert_eq!(
            dynamic_selection.geometry_class,
            RenderGeometryClass::GpuCulledDynamicRaster
        );
    }

    #[test]
    fn catalog_raster_only_can_still_instance_static_catalog_buckets() {
        let config = test_render_config(RenderGeometryPolicy::RasterOnly);
        let entry = test_catalog_entry(RenderCostClass::Simple, VisualImportance::SetDressing);

        let selection =
            select_render_path(&entry, 12, FunGeometryClass::StaticOpaqueSimple, &config);

        assert_eq!(selection.render_path, FunRenderPath::InstancedRaster);
        assert_eq!(
            selection.geometry_class,
            RenderGeometryClass::InstancedStaticRaster
        );
    }

    #[test]
    fn catalog_virtual_static_policy_marks_large_streaming_dense_chunks() {
        let config = test_render_config(RenderGeometryPolicy::VirtualStaticExperimental);
        let entry = test_catalog_entry(
            RenderCostClass::DenseStatic,
            VisualImportance::GameplayCover,
        );

        let selection =
            select_render_path(&entry, 8_192, FunGeometryClass::StaticOpaqueDense, &config);

        assert_eq!(selection.render_path, FunRenderPath::GpuCulledIndirect);
        assert_eq!(
            selection.geometry_class,
            RenderGeometryClass::VirtualStaticCluster
        );
    }
}
