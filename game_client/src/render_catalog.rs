use std::collections::{HashMap, HashSet};

use avian3d::prelude::Collider;
use bevy::{
    pbr::experimental::meshlet::{
        MESHLET_DEFAULT_VERTEX_POSITION_QUANTIZATION_FACTOR, MeshletMesh, RenderPathArbiter,
        RenderPathDecision, RenderPathDistanceBand, RenderPathInput, RenderPathMaterialClass,
    },
    prelude::*,
};
use game_shared::{
    CatalogCollider, CatalogGeometry, DEMO_RENDER_CATALOG, LightingParticipation,
    RenderCatalogEntry, RenderCostClass, VisualImportance, material_preset,
};
use thunder::prelude::WorldCatalogRef;

use crate::{
    ClientRenderConfig, RenderGeometryClass, RenderGeometryPolicy,
    compiled_world::CompiledWorldPackage,
};

#[derive(Debug, Resource, Default)]
pub(crate) struct WorldRenderCatalog {
    assets: HashMap<u32, CompiledRenderAsset>,
    materials: HashMap<u32, Handle<StandardMaterial>>,
}

#[derive(Debug)]
pub(crate) struct CompiledRenderAsset {
    #[cfg_attr(not(all(feature = "diagnostics", debug_assertions)), allow(dead_code))]
    pub entry: &'static RenderCatalogEntry,
    pub raster_mesh: Option<Handle<Mesh>>,
    pub meshlet_mesh: Option<Handle<MeshletMesh>>,
    pub ray_proxy: Option<Handle<Mesh>>,
    pub material: Option<Handle<StandardMaterial>>,
    pub collider: Option<Collider>,
    pub geometry_class: RenderGeometryClass,
    #[cfg_attr(not(all(feature = "diagnostics", debug_assertions)), allow(dead_code))]
    pub triangle_count: usize,
}

impl WorldRenderCatalog {
    pub(crate) fn lookup(&self, catalog_ref: WorldCatalogRef) -> Option<&CompiledRenderAsset> {
        self.assets.get(&catalog_ref.asset_id)
    }

    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    pub(crate) fn len(&self) -> usize {
        self.assets.len()
    }
}

pub(crate) fn prewarm_world_render_catalog(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut meshlet_meshes: ResMut<Assets<MeshletMesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    render_config: Res<ClientRenderConfig>,
) {
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

    for preset in game_shared::MATERIAL_PRESETS {
        let color = Color::srgba_u8(
            preset.srgb[0],
            preset.srgb[1],
            preset.srgb[2],
            preset.srgb[3],
        );
        catalog
            .materials
            .insert(preset.id.0, materials.add(StandardMaterial::from(color)));
    }

    for entry in DEMO_RENDER_CATALOG {
        let triangle_count = entry
            .geometry
            .map(geometry_triangle_count)
            .unwrap_or_default();
        let geometry_class = classify_geometry(entry, triangle_count, &render_config);
        let material = material_preset(entry.material)
            .and_then(|preset| catalog.materials.get(&preset.id.0))
            .cloned();

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
        let collider = entry
            .collider
            .map(|(_, collider)| collider_from_catalog(collider));

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
            triangles = triangle_count,
            has_raster = raster_mesh.is_some(),
            has_meshlet = meshlet_mesh.is_some(),
            has_ray_proxy = ray_proxy.is_some(),
            has_collider = collider.is_some(),
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
                collider,
                geometry_class,
                triangle_count,
            },
        );
    }

    game_shared::fun_diag_info!(
        target: "fun::render_catalog",
        asset_count = catalog.assets.len(),
        material_count = catalog.materials.len(),
        geometry_policy = ?render_config.geometry_policy,
        meshlet_min_triangles = render_config.meshlet_min_triangles,
        "world render catalog prewarmed"
    );
    commands.insert_resource(compiled_package);
    commands.insert_resource(catalog);
}

fn classify_geometry(
    entry: &RenderCatalogEntry,
    triangle_count: usize,
    render_config: &ClientRenderConfig,
) -> RenderGeometryClass {
    if entry.geometry.is_none() {
        return RenderGeometryClass::RayProxyOnly;
    }

    if !render_config.meshlets_enabled {
        return RenderGeometryClass::SimpleRaster;
    }

    match render_config.geometry_policy {
        RenderGeometryPolicy::AllRaster => RenderGeometryClass::SimpleRaster,
        RenderGeometryPolicy::AllMeshlet => RenderGeometryClass::MeshletStaticDense,
        RenderGeometryPolicy::Hybrid => {
            let arbiter = RenderPathArbiter {
                static_meshlet_min_triangles: render_config.meshlet_min_triangles as u32,
                dynamic_meshlet_min_triangles: render_config.meshlet_min_triangles.saturating_mul(4)
                    as u32,
                ..default()
            };
            match arbiter.decide(RenderPathInput {
                triangle_count: triangle_count as u32,
                meshlet_count: triangle_count.div_ceil(64).max(1) as u32,
                projected_screen_area: projected_screen_area_estimate(entry),
                material_class: material_class(entry),
                is_static: entry.cost_class != RenderCostClass::DenseDynamic,
                overdraw_estimate: overdraw_estimate(entry),
                distance_band: distance_band(entry),
                ray_proxy_only: entry.cost_class == RenderCostClass::RayProxyOnly,
                viewmodel: entry.cost_class == RenderCostClass::Viewmodel,
            }) {
                RenderPathDecision::StandardRaster => RenderGeometryClass::SimpleRaster,
                RenderPathDecision::MeshletStaticDense => RenderGeometryClass::MeshletStaticDense,
                RenderPathDecision::MeshletDynamicDense => RenderGeometryClass::MeshletDynamicDense,
                RenderPathDecision::RayProxyOnly => RenderGeometryClass::RayProxyOnly,
                RenderPathDecision::Viewmodel => RenderGeometryClass::Viewmodel,
            }
        }
    }
}

fn material_class(entry: &RenderCatalogEntry) -> RenderPathMaterialClass {
    match entry.cost_class {
        RenderCostClass::Transparent => RenderPathMaterialClass::Transparent,
        RenderCostClass::Viewmodel => RenderPathMaterialClass::Viewmodel,
        _ if entry.lighting.contains(LightingParticipation::EMISSIVE) => {
            RenderPathMaterialClass::Emissive
        }
        _ if entry.lighting.contains(LightingParticipation::SPECULAR) => {
            RenderPathMaterialClass::OpaqueComplex
        }
        _ => RenderPathMaterialClass::OpaqueSimple,
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

fn distance_band(entry: &RenderCatalogEntry) -> RenderPathDistanceBand {
    match entry.visual_importance {
        VisualImportance::Foreground | VisualImportance::GameplayCover => {
            RenderPathDistanceBand::Near
        }
        VisualImportance::Navigation | VisualImportance::SetDressing => RenderPathDistanceBand::Mid,
        VisualImportance::Background => RenderPathDistanceBand::Far,
    }
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
pub(crate) fn catalog_ref_summary(catalog_ref: Option<WorldCatalogRef>) -> String {
    catalog_ref
        .map(|value| {
            format!(
                "asset={} material={} collider={}",
                value.asset_id, value.material_id, value.collider_id
            )
        })
        .unwrap_or_else(|| "none".to_owned())
}

pub(crate) fn warn_missing_catalog_ref(_catalog_ref: WorldCatalogRef, _name: &str) {
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

fn collider_from_catalog(collider: CatalogCollider) -> Collider {
    match collider {
        CatalogCollider::Cuboid { size } => Collider::cuboid(size[0], size[1], size[2]),
    }
}
