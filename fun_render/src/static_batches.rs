use std::{
    collections::{BTreeMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
};

use bevy::{asset::Asset, pbr::experimental::meshlet::MeshletMesh, prelude::*};
use game_shared::{CatalogGeometry, LightingParticipation, RenderCostClass, material_preset};
use thunder::prelude::*;

use crate::{
    ClientOpaqueRenderer, ClientRenderConfig, CompiledRenderAsset, FunGeometryClass, FunRenderPath,
    InstanceRange, MaterialInstanceTint, MaterialKey, RenderBatchKey, RenderGeometryClass,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StaticRenderCellId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StaticRenderBatchKey {
    pub catalog_asset_id: u32,
    pub mesh_handle_hash: u64,
    pub meshlet_handle_hash: u64,
    pub material_key: MaterialKey,
    pub material_handle_hash: u64,
    pub render_batch_key: RenderBatchKey,
    pub geometry_class: FunGeometryClass,
    pub render_geometry_class: RenderGeometryClass,
    pub render_path: FunRenderPath,
    pub shadow_participation: bool,
    pub solari_ray_proxy_participation: bool,
    pub opaque_renderer: ClientOpaqueRenderer,
}

impl StaticRenderBatchKey {
    pub fn from_compiled(
        compiled: &CompiledRenderAsset,
        render_config: &ClientRenderConfig,
        opaque_renderer: ClientOpaqueRenderer,
    ) -> Self {
        let material_key = material_preset(compiled.entry.material)
            .map(|preset| MaterialKey::from_preset(*preset))
            .unwrap_or(compiled.material_policy.shared_material_key);
        Self {
            catalog_asset_id: compiled.entry.asset_id.0,
            mesh_handle_hash: handle_hash(compiled.raster_mesh.as_ref()),
            meshlet_handle_hash: handle_hash(compiled.meshlet_mesh.as_ref()),
            material_key,
            material_handle_hash: handle_hash(compiled.material.as_ref()),
            render_batch_key: compiled.render_batch_key,
            geometry_class: compiled.fun_geometry_class,
            render_geometry_class: compiled.geometry_class,
            render_path: compiled.render_path,
            shadow_participation: compiled
                .entry
                .lighting
                .contains(LightingParticipation::DIRECT_SHADOW),
            solari_ray_proxy_participation: render_config.solari_enabled
                && (compiled.ray_proxy.is_some()
                    || compiled
                        .entry
                        .lighting
                        .contains(LightingParticipation::RAY_PROXY)),
            opaque_renderer,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StaticRenderBatchBounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Default for StaticRenderBatchBounds {
    fn default() -> Self {
        Self::empty()
    }
}

impl StaticRenderBatchBounds {
    pub const fn empty() -> Self {
        Self {
            min: [f32::INFINITY; 3],
            max: [f32::NEG_INFINITY; 3],
        }
    }

    pub fn from_center_extents(center: Vec3, extents: Vec3) -> Self {
        Self {
            min: (center - extents).to_array(),
            max: (center + extents).to_array(),
        }
    }

    pub fn is_empty(self) -> bool {
        self.min[0] > self.max[0] || self.min[1] > self.max[1] || self.min[2] > self.max[2]
    }

    pub fn include_bounds(&mut self, bounds: Self) {
        if bounds.is_empty() {
            return;
        }
        if self.is_empty() {
            *self = bounds;
            return;
        }
        for axis in 0..3 {
            self.min[axis] = self.min[axis].min(bounds.min[axis]);
            self.max[axis] = self.max[axis].max(bounds.max[axis]);
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StaticRenderBatchInstanceRange {
    pub start: u32,
    pub count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StaticRenderGpuInstanceBufferHandle(pub u64);

#[derive(Debug, Clone, Component)]
pub struct StaticRenderBatch {
    pub cell_id: StaticRenderCellId,
    pub key: StaticRenderBatchKey,
    pub transforms: Vec<QuantizedTransform3>,
    pub instance_tints: Vec<MaterialInstanceTint>,
    pub instance_bounds: Vec<StaticRenderBatchBounds>,
    pub instance_entities: Vec<NetEntity>,
    pub aggregate_bounds: StaticRenderBatchBounds,
    pub mesh: Option<Handle<Mesh>>,
    pub meshlet_mesh: Option<Handle<MeshletMesh>>,
    pub material: Option<Handle<StandardMaterial>>,
    pub ray_proxy: Option<Handle<Mesh>>,
    pub gpu_instance_buffer_handle: Option<StaticRenderGpuInstanceBufferHandle>,
    pub instance_range: Option<InstanceRange>,
}

impl StaticRenderBatch {
    pub fn instance_count(&self) -> usize {
        self.transforms.len()
    }
}

#[derive(Debug, Clone, Component)]
pub struct StaticRenderCell {
    pub cell_id: StaticRenderCellId,
    pub aggregate_bounds: StaticRenderBatchBounds,
    pub batch_ranges: Vec<StaticRenderBatchInstanceRange>,
    pub occluder: bool,
    pub resident: bool,
    pub visible: bool,
}

#[derive(Debug, Clone)]
pub struct StaticRenderBatchSpawnRecord {
    pub batch: StaticRenderBatch,
    pub cell: StaticRenderCell,
    pub map_to_batch_entities: Vec<NetEntity>,
}

#[derive(Debug)]
pub struct StaticRenderBatchBuilder {
    fallback_cell_id: StaticRenderCellId,
    groups: BTreeMap<(StaticRenderCellId, StaticRenderBatchKey), StaticRenderBatchBuildState>,
}

impl StaticRenderBatchBuilder {
    pub const fn new(chunk_index: u16) -> Self {
        Self {
            fallback_cell_id: StaticRenderCellId(chunk_index as u32),
            groups: BTreeMap::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    pub fn len(&self) -> usize {
        self.groups.len()
    }

    pub fn push_catalog_instance(
        &mut self,
        spec: &WorldEntitySpec,
        compiled: &CompiledRenderAsset,
        render_config: &ClientRenderConfig,
        opaque_renderer: ClientOpaqueRenderer,
        map_to_batch_entity: bool,
    ) -> bool {
        if !static_catalog_spec_is_batchable(spec, compiled) {
            return false;
        }
        let key = StaticRenderBatchKey::from_compiled(compiled, render_config, opaque_renderer);
        let cell_id = static_cell_id(compiled, self.fallback_cell_id);
        let Some(geometry) = compiled.entry.geometry else {
            return false;
        };
        let instance_bounds = bounds_for_catalog_instance(geometry, spec.transform);
        let state = self
            .groups
            .entry((cell_id, key))
            .or_insert_with(|| StaticRenderBatchBuildState::new(cell_id, key, compiled));
        state.push(spec, instance_bounds, map_to_batch_entity);
        true
    }

    pub fn finish(self) -> Vec<StaticRenderBatchSpawnRecord> {
        let mut cell_builders = BTreeMap::<StaticRenderCellId, StaticRenderCellBuildState>::new();
        for state in self.groups.values() {
            let cell = cell_builders.entry(state.cell_id).or_default();
            cell.aggregate_bounds.include_bounds(state.aggregate_bounds);
            cell.occluder |= state.occluder;
            let start = cell.instance_count;
            let count = state.transforms.len().min(u32::MAX as usize) as u32;
            cell.batch_ranges
                .push(StaticRenderBatchInstanceRange { start, count });
            cell.instance_count = cell.instance_count.saturating_add(count);
        }

        self.groups
            .into_values()
            .map(|state| {
                let cell = cell_builders
                    .get(&state.cell_id)
                    .expect("cell metadata is built before batch records");
                let cell_id = state.cell_id;
                let map_to_batch_entities = state.map_to_batch_entities.clone();
                StaticRenderBatchSpawnRecord {
                    batch: state.into_batch(),
                    cell: StaticRenderCell {
                        cell_id,
                        aggregate_bounds: cell.aggregate_bounds,
                        batch_ranges: cell.batch_ranges.clone(),
                        occluder: cell.occluder,
                        resident: true,
                        visible: true,
                    },
                    map_to_batch_entities,
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
struct StaticRenderBatchBuildState {
    cell_id: StaticRenderCellId,
    key: StaticRenderBatchKey,
    transforms: Vec<QuantizedTransform3>,
    instance_tints: Vec<MaterialInstanceTint>,
    instance_bounds: Vec<StaticRenderBatchBounds>,
    instance_entities: Vec<NetEntity>,
    map_to_batch_entities: Vec<NetEntity>,
    aggregate_bounds: StaticRenderBatchBounds,
    mesh: Option<Handle<Mesh>>,
    meshlet_mesh: Option<Handle<MeshletMesh>>,
    material: Option<Handle<StandardMaterial>>,
    ray_proxy: Option<Handle<Mesh>>,
    occluder: bool,
}

impl StaticRenderBatchBuildState {
    fn new(
        cell_id: StaticRenderCellId,
        key: StaticRenderBatchKey,
        compiled: &CompiledRenderAsset,
    ) -> Self {
        Self {
            cell_id,
            key,
            transforms: Vec::new(),
            instance_tints: Vec::new(),
            instance_bounds: Vec::new(),
            instance_entities: Vec::new(),
            map_to_batch_entities: Vec::new(),
            aggregate_bounds: StaticRenderBatchBounds::empty(),
            mesh: compiled.raster_mesh.clone(),
            meshlet_mesh: compiled.meshlet_mesh.clone(),
            material: compiled.material.clone(),
            ray_proxy: compiled.ray_proxy.clone(),
            occluder: static_catalog_asset_is_occluder(compiled),
        }
    }

    fn push(
        &mut self,
        spec: &WorldEntitySpec,
        instance_bounds: StaticRenderBatchBounds,
        map_to_batch_entity: bool,
    ) {
        self.transforms.push(spec.transform);
        self.instance_tints
            .push(MaterialInstanceTint::from_packed_color(spec.color));
        self.instance_bounds.push(instance_bounds);
        self.instance_entities.push(spec.entity);
        if map_to_batch_entity {
            self.map_to_batch_entities.push(spec.entity);
        }
        self.aggregate_bounds.include_bounds(instance_bounds);
    }

    fn into_batch(self) -> StaticRenderBatch {
        StaticRenderBatch {
            cell_id: self.cell_id,
            key: self.key,
            transforms: self.transforms,
            instance_tints: self.instance_tints,
            instance_bounds: self.instance_bounds,
            instance_entities: self.instance_entities,
            aggregate_bounds: self.aggregate_bounds,
            mesh: self.mesh,
            meshlet_mesh: self.meshlet_mesh,
            material: self.material,
            ray_proxy: self.ray_proxy,
            gpu_instance_buffer_handle: None,
            instance_range: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct StaticRenderCellBuildState {
    aggregate_bounds: StaticRenderBatchBounds,
    batch_ranges: Vec<StaticRenderBatchInstanceRange>,
    instance_count: u32,
    occluder: bool,
}

pub fn static_catalog_spec_may_batch(spec: &WorldEntitySpec) -> bool {
    spec.catalog.is_some()
        && spec.render.is_none()
        && matches!(
            spec.class,
            ReplicationClass::World | ReplicationClass::Destructible
        )
        && matches!(spec.authority, AuthorityMode::StaticServer)
}

pub fn static_catalog_spec_is_batchable(
    spec: &WorldEntitySpec,
    compiled: &CompiledRenderAsset,
) -> bool {
    static_catalog_spec_may_batch(spec)
        && compiled.entry.geometry.is_some()
        && compiled.fun_geometry_class.is_static()
        && compiled.render_path.emits_visible_raster()
        && !matches!(
            compiled.entry.cost_class,
            RenderCostClass::DenseDynamic
                | RenderCostClass::Transparent
                | RenderCostClass::Viewmodel
                | RenderCostClass::RayProxyOnly
        )
}

pub fn static_catalog_spec_needs_identity_proxy(
    spec: &WorldEntitySpec,
    compiled: &CompiledRenderAsset,
) -> bool {
    spec.collider.is_some() || compiled.entry.collider.is_some()
}

fn static_cell_id(
    compiled: &CompiledRenderAsset,
    fallback_cell_id: StaticRenderCellId,
) -> StaticRenderCellId {
    let cell = compiled.entry.occlusion_cell.0;
    if cell == 0 {
        fallback_cell_id
    } else {
        StaticRenderCellId(cell)
    }
}

fn static_catalog_asset_is_occluder(compiled: &CompiledRenderAsset) -> bool {
    compiled
        .entry
        .lighting
        .contains(LightingParticipation::DIRECT_SHADOW)
        && !matches!(
            compiled.entry.cost_class,
            RenderCostClass::Tiny
                | RenderCostClass::Transparent
                | RenderCostClass::Viewmodel
                | RenderCostClass::RayProxyOnly
        )
}

fn bounds_for_catalog_instance(
    geometry: CatalogGeometry,
    transform: QuantizedTransform3,
) -> StaticRenderBatchBounds {
    let center = vec3_from_quantized(transform.translation);
    let half_extents = catalog_geometry_half_extents(geometry);
    let rotation = quat_from_quantized(transform);
    let rotated_extents = (rotation.mul_vec3(Vec3::X) * half_extents.x).abs()
        + (rotation.mul_vec3(Vec3::Y) * half_extents.y).abs()
        + (rotation.mul_vec3(Vec3::Z) * half_extents.z).abs();
    StaticRenderBatchBounds::from_center_extents(center, rotated_extents)
}

fn catalog_geometry_half_extents(geometry: CatalogGeometry) -> Vec3 {
    match geometry {
        CatalogGeometry::Plane { size } | CatalogGeometry::Cuboid { size } => {
            Vec3::from_array(size) * 0.5
        }
    }
}

fn vec3_from_quantized(value: QuantizedVec3) -> Vec3 {
    Vec3::from_array(value.to_f32(Quantization::MILLIMETERS))
}

fn quat_from_quantized(transform: QuantizedTransform3) -> Quat {
    let rotation = transform.rotation.to_f32();
    Quat::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3])
}

fn handle_hash<T: Asset>(handle: Option<&Handle<T>>) -> u64 {
    let Some(handle) = handle else {
        return 0;
    };
    let mut hasher = DefaultHasher::new();
    handle.id().hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ClientRenderConfig, FunMaterialClass, FunRenderPath, RenderGeometryClass,
        RenderGeometryPolicy, material_policy_for_catalog_entry,
        render_batch_key_for_catalog_entry,
    };

    fn test_render_config() -> ClientRenderConfig {
        let mut config = ClientRenderConfig::from_env();
        config.solari_enabled = true;
        config.meshlets_enabled = false;
        config.geometry_policy = RenderGeometryPolicy::Hybrid;
        config
    }

    fn compiled(asset_id: game_shared::RenderAssetId) -> CompiledRenderAsset {
        let entry = game_shared::demo_catalog_entry(asset_id).expect("demo catalog entry exists");
        let material_policy = material_policy_for_catalog_entry(entry);
        let render_batch_key = render_batch_key_for_catalog_entry(
            entry,
            FunRenderPath::StandardRaster,
            RenderGeometryClass::SimpleRaster,
            FunGeometryClass::StaticOpaqueSimple,
            FunMaterialClass::OpaqueSimple,
            material_policy,
        );
        CompiledRenderAsset {
            entry,
            raster_mesh: Some(Handle::default()),
            meshlet_mesh: None,
            ray_proxy: Some(Handle::default()),
            material: Some(Handle::default()),
            geometry_class: RenderGeometryClass::SimpleRaster,
            fun_geometry_class: FunGeometryClass::StaticOpaqueSimple,
            render_path: FunRenderPath::StandardRaster,
            material_policy,
            render_batch_key,
            triangle_count: 12,
        }
    }

    fn world_spec(entity: u64, asset_id: game_shared::RenderAssetId) -> WorldEntitySpec {
        WorldEntitySpec {
            entity: NetEntity(entity),
            name: format!("static-{entity}"),
            class: ReplicationClass::World,
            authority: AuthorityMode::StaticServer,
            transform: QuantizedTransform3 {
                translation: QuantizedVec3::from_f32(
                    [entity as f32, 0.0, 0.0],
                    Quantization::MILLIMETERS,
                ),
                rotation: Default::default(),
            },
            catalog: Some(WorldCatalogRef {
                asset_id: asset_id.0,
                material_id: 0,
                collider_id: 0,
            }),
            render: None,
            collider: None,
            color: None,
        }
    }

    #[test]
    fn static_batch_builder_groups_catalog_static_instances_by_cell_and_key() {
        let config = test_render_config();
        let wall = compiled(game_shared::ASSET_WALL);
        let mut builder = StaticRenderBatchBuilder::new(7);

        assert!(builder.push_catalog_instance(
            &world_spec(1, game_shared::ASSET_WALL),
            &wall,
            &config,
            ClientOpaqueRenderer::Deferred,
            true,
        ));
        assert!(builder.push_catalog_instance(
            &world_spec(2, game_shared::ASSET_WALL),
            &wall,
            &config,
            ClientOpaqueRenderer::Deferred,
            true,
        ));

        let records = builder.finish();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].batch.cell_id, StaticRenderCellId(2));
        assert_eq!(records[0].batch.instance_count(), 2);
        assert_eq!(
            records[0].batch.instance_tints,
            vec![crate::MaterialInstanceTint::WHITE; 2]
        );
        assert_eq!(records[0].map_to_batch_entities.len(), 2);
        assert_eq!(records[0].cell.batch_ranges[0].count, 2);
    }

    #[test]
    fn static_batch_preserves_per_instance_tint_without_splitting_material_key() {
        let config = test_render_config();
        let wall = compiled(game_shared::ASSET_WALL);
        let mut builder = StaticRenderBatchBuilder::new(7);
        let mut red = world_spec(1, game_shared::ASSET_WALL);
        red.color = Some(PackedColorRgba8::srgb(255, 0, 0));
        let mut blue = world_spec(2, game_shared::ASSET_WALL);
        blue.color = Some(PackedColorRgba8::srgb(0, 0, 255));

        assert!(builder.push_catalog_instance(
            &red,
            &wall,
            &config,
            ClientOpaqueRenderer::Deferred,
            true,
        ));
        assert!(builder.push_catalog_instance(
            &blue,
            &wall,
            &config,
            ClientOpaqueRenderer::Deferred,
            true,
        ));

        let records = builder.finish();

        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0].batch.instance_tints,
            vec![
                crate::MaterialInstanceTint::from_rgba8([255, 0, 0, 255]),
                crate::MaterialInstanceTint::from_rgba8([0, 0, 255, 255])
            ]
        );
    }

    #[test]
    fn static_batch_key_separates_material_and_render_path() {
        let config = test_render_config();
        let wall = compiled(game_shared::ASSET_WALL);
        let ramp = compiled(game_shared::ASSET_RAMP);
        let wall_key =
            StaticRenderBatchKey::from_compiled(&wall, &config, ClientOpaqueRenderer::Deferred);
        let ramp_key =
            StaticRenderBatchKey::from_compiled(&ramp, &config, ClientOpaqueRenderer::Deferred);

        assert_ne!(wall_key.material_key, ramp_key.material_key);
        assert_ne!(wall_key.catalog_asset_id, ramp_key.catalog_asset_id);
    }

    #[test]
    fn static_batch_bounds_include_all_instances() {
        let config = test_render_config();
        let wall = compiled(game_shared::ASSET_WALL);
        let mut builder = StaticRenderBatchBuilder::new(7);

        assert!(builder.push_catalog_instance(
            &world_spec(1, game_shared::ASSET_WALL),
            &wall,
            &config,
            ClientOpaqueRenderer::Deferred,
            true,
        ));
        assert!(builder.push_catalog_instance(
            &world_spec(10, game_shared::ASSET_WALL),
            &wall,
            &config,
            ClientOpaqueRenderer::Deferred,
            true,
        ));

        let records = builder.finish();
        let bounds = records[0].batch.aggregate_bounds;
        assert!(bounds.min[0] < 0.0);
        assert!(bounds.max[0] > 10.0);
    }

    #[test]
    fn static_batch_builder_can_keep_collider_instances_on_proxy_entities() {
        let config = test_render_config();
        let wall = compiled(game_shared::ASSET_WALL);
        let mut builder = StaticRenderBatchBuilder::new(7);

        assert!(builder.push_catalog_instance(
            &world_spec(1, game_shared::ASSET_WALL),
            &wall,
            &config,
            ClientOpaqueRenderer::Deferred,
            false,
        ));

        let records = builder.finish();
        assert_eq!(records[0].batch.instance_entities, vec![NetEntity(1)]);
        assert!(records[0].map_to_batch_entities.is_empty());
    }

    #[test]
    fn dynamic_catalog_spec_is_not_static_batchable() {
        let wall = compiled(game_shared::ASSET_WALL);
        let mut spec = world_spec(1, game_shared::ASSET_WALL);
        spec.authority = AuthorityMode::ServerOnly;

        assert!(!static_catalog_spec_is_batchable(&spec, &wall));
    }

    #[test]
    fn static_destructible_catalog_spec_may_batch_until_damaged() {
        let wall = compiled(game_shared::ASSET_WALL);
        let mut spec = world_spec(1, game_shared::ASSET_WALL);
        spec.class = ReplicationClass::Destructible;

        assert!(static_catalog_spec_may_batch(&spec));
        assert!(static_catalog_spec_is_batchable(&spec, &wall));
    }
}
