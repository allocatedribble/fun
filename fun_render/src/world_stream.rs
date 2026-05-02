use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use bevy::{
    pbr::experimental::meshlet::{
        MESHLET_DEFAULT_VERTEX_POSITION_QUANTIZATION_FACTOR, MeshletMesh, MeshletMesh3d,
    },
    prelude::*,
    solari::prelude::RaytracingMesh3d,
};
use thunder::prelude::*;

use crate::{
    ClientRenderConfig, RenderGeometryClass, WorldRenderCatalog, warn_missing_catalog_ref,
};

#[derive(Debug, Default, Resource)]
pub struct RenderWorldContext {
    pub level_id: Option<String>,
    pub revision: Option<WorldRevision>,
    pub expected_chunks: u16,
    pub received_chunks: HashSet<u16>,
    pub spawned_entities: HashMap<NetEntity, Entity>,
}

impl RenderWorldContext {
    pub fn is_complete(&self) -> bool {
        self.expected_chunks > 0 && self.received_chunks.len() >= self.expected_chunks as usize
    }
}

#[derive(Debug, Default, Resource)]
pub struct RenderWorldStatus {
    pub ready: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RenderWorldApplyOptions {
    pub stream_verbose: bool,
    pub render_verbose: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderWorldChunkOutcome {
    pub world_revision_changed: bool,
    pub spawned_entities: usize,
    pub duplicate_entities: usize,
    pub catalog_lookup_ns: u64,
}

pub fn despawn_render_context(
    commands: &mut Commands,
    context: &mut RenderWorldContext,
    status: &mut RenderWorldStatus,
) {
    for entity in context.spawned_entities.values().copied() {
        commands.entity(entity).despawn();
    }
    context.level_id = None;
    context.revision = None;
    context.expected_chunks = 0;
    context.received_chunks.clear();
    context.spawned_entities.clear();
    status.ready = false;
}

#[allow(
    clippy::too_many_arguments,
    reason = "world-stream render hydration keeps Bevy asset stores explicit for deterministic editor and client callers"
)]
pub fn apply_render_world_chunk(
    commands: &mut Commands,
    context: &mut RenderWorldContext,
    status: &mut RenderWorldStatus,
    meshes: &mut Assets<Mesh>,
    meshlet_meshes: &mut Assets<MeshletMesh>,
    materials: &mut Assets<StandardMaterial>,
    catalog: &WorldRenderCatalog,
    render_config: &ClientRenderConfig,
    options: RenderWorldApplyOptions,
    chunk: &WorldStreamChunk,
) -> RenderWorldChunkOutcome {
    let mut outcome = RenderWorldChunkOutcome::default();
    if context.revision != Some(chunk.revision)
        || context.level_id.as_deref() != Some(chunk.level_id.0.as_str())
    {
        outcome.world_revision_changed = true;
        game_shared::fun_diag_info_if!(
            options.stream_verbose,
            target: "fun::stream",
            old_level = ?context.level_id,
            old_revision = ?context.revision.map(|revision| revision.0),
            old_spawned_entities = context.spawned_entities.len(),
            new_level = %chunk.level_id.0,
            new_revision = chunk.revision.0,
            expected_chunks = chunk.chunk_count,
            "resetting streamed render world"
        );
        despawn_render_context(commands, context, status);

        context.level_id = Some(chunk.level_id.0.clone());
        context.revision = Some(chunk.revision);
        context.expected_chunks = chunk.chunk_count;
        game_shared::fun_diag_info_if!(
            options.stream_verbose,
            target: "fun::stream",
            level = %chunk.level_id.0,
            revision = chunk.revision.0,
            chunk_count = chunk.chunk_count,
            "receiving streamed render world"
        );
    }

    for spec in &chunk.entities {
        if context.spawned_entities.contains_key(&spec.entity) {
            outcome.duplicate_entities = outcome.duplicate_entities.saturating_add(1);
            game_shared::fun_diag_warn!(
                target: "fun::stream",
                net_entity = spec.entity.0,
                name = %spec.name,
                "skipping duplicate streamed render entity"
            );
            continue;
        }

        let entity = spawn_render_entity_from_spec(
            commands,
            meshes,
            meshlet_meshes,
            materials,
            catalog,
            render_config,
            options,
            spec,
            &mut outcome,
        );
        game_shared::fun_diag_debug_if!(
            options.stream_verbose,
            target: "fun::stream",
            ecs_entity = ?entity,
            net_entity = spec.entity.0,
            name = %spec.name,
            "spawned streamed render entity"
        );
        context.spawned_entities.insert(spec.entity, entity);
        outcome.spawned_entities = outcome.spawned_entities.saturating_add(1);
    }

    context.received_chunks.insert(chunk.chunk_index);
    game_shared::fun_diag_info_if!(
        options.stream_verbose,
        target: "fun::stream",
        received_chunks = context.received_chunks.len(),
        expected_chunks = context.expected_chunks,
        spawned_entities = context.spawned_entities.len(),
        "streamed render world chunk complete"
    );

    outcome
}

#[allow(
    clippy::too_many_arguments,
    reason = "spawn wiring keeps Bevy asset stores explicit while catalog-driven construction is shared by client and editor"
)]
pub fn spawn_render_entity_from_spec(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    meshlet_meshes: &mut Assets<MeshletMesh>,
    materials: &mut Assets<StandardMaterial>,
    catalog: &WorldRenderCatalog,
    render_config: &ClientRenderConfig,
    options: RenderWorldApplyOptions,
    spec: &WorldEntitySpec,
    outcome: &mut RenderWorldChunkOutcome,
) -> Entity {
    let _translation = vec3_from_quantized(spec.transform.translation);
    let mut entity_commands = commands.spawn((
        Name::new(spec.name.clone()),
        NetworkIdentity {
            entity: spec.entity,
            class: spec.class,
        },
        NetworkAuthority {
            mode: spec.authority,
        },
        transform_from_quantized(spec.transform),
        GlobalTransform::default(),
        Visibility::default(),
    ));

    game_shared::fun_diag_debug_if!(
        options.stream_verbose,
        target: "fun::stream::entity",
        net_entity = spec.entity.0,
        name = %spec.name,
        class = ?spec.class,
        authority = ?spec.authority,
        translation_x = _translation.x,
        translation_y = _translation.y,
        translation_z = _translation.z,
        catalog = ?spec.catalog,
        render = ?spec.render,
        color = ?spec.color,
        "streamed render entity spawn spec"
    );

    if let Some(catalog_ref) = spec.catalog {
        let lookup_started = Instant::now();
        let compiled = catalog.lookup(catalog_ref);
        outcome.catalog_lookup_ns = outcome.catalog_lookup_ns.saturating_add(
            lookup_started
                .elapsed()
                .as_nanos()
                .min(u128::from(u64::MAX)) as u64,
        );

        if let Some(compiled) = compiled {
            entity_commands.insert(compiled.geometry_class);
            if compiled.geometry_class.uses_meshlet() {
                if let (Some(meshlet_mesh), Some(material)) =
                    (compiled.meshlet_mesh.as_ref(), compiled.material.as_ref())
                {
                    entity_commands.insert((
                        MeshletMesh3d(meshlet_mesh.clone()),
                        MeshMaterial3d::<StandardMaterial>(material.clone()),
                    ));
                }
            } else if compiled.geometry_class.uses_raster_mesh()
                && let (Some(mesh), Some(material)) =
                    (compiled.raster_mesh.as_ref(), compiled.material.as_ref())
            {
                entity_commands.insert((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d::<StandardMaterial>(material.clone()),
                ));
            }

            if render_config.solari_enabled
                && let Some(ray_proxy) = compiled.ray_proxy.as_ref()
            {
                entity_commands.insert(RaytracingMesh3d(ray_proxy.clone()));
            }

            game_shared::fun_diag_info_if!(
                options.render_verbose,
                target: "fun::render_catalog",
                name = %spec.name,
                asset_id = catalog_ref.asset_id,
                material_id = catalog_ref.material_id,
                collider_id = catalog_ref.collider_id,
                catalog_asset = compiled.entry.name,
                geometry_class = ?compiled.geometry_class,
                triangles = compiled.triangle_count,
                meshlet = compiled.geometry_class.uses_meshlet(),
                raster = compiled.geometry_class.uses_raster_mesh(),
                raytracing = render_config.solari_enabled && compiled.ray_proxy.is_some(),
                "inserted catalog-backed streamed render components"
            );
        } else {
            warn_missing_catalog_ref(catalog_ref, &spec.name);
        }
    } else if let Some(primitive) = spec.render {
        let mesh = mesh_from_primitive(primitive);
        let (raytracing_mesh, meshlet_mesh) = add_scene_mesh_assets(
            meshes,
            meshlet_meshes,
            mesh,
            &spec.name,
            render_config,
            options,
        );
        let material = materials.add(color_from_packed(spec.color));

        if render_config.meshlets_enabled {
            entity_commands.insert((
                MeshletMesh3d(meshlet_mesh.expect("meshlet handle should exist when enabled")),
                MeshMaterial3d::<StandardMaterial>(material),
            ));
        } else if let Some(mesh) = raytracing_mesh.as_ref() {
            entity_commands.insert((
                Mesh3d(mesh.clone()),
                MeshMaterial3d::<StandardMaterial>(material),
            ));
        }

        if render_config.solari_enabled
            && let Some(raytracing_mesh) = raytracing_mesh
        {
            entity_commands.insert(RaytracingMesh3d(raytracing_mesh));
        }

        game_shared::fun_diag_debug_if!(
            options.render_verbose,
            target: "fun::render::entity",
            name = %spec.name,
            meshlet = render_config.meshlets_enabled,
            raytracing = render_config.solari_enabled,
            "inserted streamed primitive render components"
        );
    }

    entity_commands.id()
}

pub fn update_render_context_visibility(
    mut renderables: Query<&mut Visibility, With<RenderGeometryClass>>,
) {
    for mut visibility in &mut renderables {
        if matches!(*visibility, Visibility::Hidden) {
            *visibility = Visibility::Inherited;
        }
    }
}

fn mesh_from_primitive(primitive: WorldPrimitive) -> Mesh {
    match primitive {
        WorldPrimitive::Plane { size } => {
            let size = vec3_from_quantized(size);
            Plane3d::default().mesh().size(size.x, size.z).build()
        }
        WorldPrimitive::Cuboid { size } => {
            let size = vec3_from_quantized(size);
            Cuboid::new(size.x, size.y, size.z).mesh().build()
        }
    }
}

fn add_scene_mesh_assets(
    meshes: &mut Assets<Mesh>,
    meshlet_meshes: &mut Assets<MeshletMesh>,
    mesh: Mesh,
    name: &str,
    render_config: &ClientRenderConfig,
    _options: RenderWorldApplyOptions,
) -> (Option<Handle<Mesh>>, Option<Handle<MeshletMesh>>) {
    let _vertex_count = mesh.count_vertices();
    let meshlet_handle = if render_config.meshlets_enabled {
        let meshlet_mesh =
            MeshletMesh::from_mesh(&mesh, MESHLET_DEFAULT_VERTEX_POSITION_QUANTIZATION_FACTOR)
                .unwrap_or_else(|error| panic!("failed to build {name} meshlet mesh: {error}"));
        Some(meshlet_meshes.add(meshlet_mesh))
    } else {
        None
    };
    let raytracing_handle = if render_config.solari_enabled || !render_config.meshlets_enabled {
        let raytracing_mesh = mesh.with_generated_tangents().unwrap_or_else(|error| {
            panic!("failed to generate {name} raytracing tangents: {error}")
        });
        Some(meshes.add(raytracing_mesh))
    } else {
        None
    };

    game_shared::fun_diag_debug_if!(
        _options.render_verbose,
        target: "fun::render::mesh",
        name,
        vertices = _vertex_count,
        meshlets_enabled = render_config.meshlets_enabled,
        solari_enabled = render_config.solari_enabled,
        raytracing_handle = ?raytracing_handle,
        meshlet_handle = ?meshlet_handle,
        "built streamed mesh assets"
    );

    (raytracing_handle, meshlet_handle)
}

fn color_from_packed(color: Option<PackedColorRgba8>) -> StandardMaterial {
    let color = color.unwrap_or_else(|| PackedColorRgba8::srgb(180, 180, 180));
    Color::srgba_u8(color.r, color.g, color.b, color.a).into()
}

pub fn transform_from_quantized(transform: QuantizedTransform3) -> Transform {
    let rotation = transform.rotation.to_f32();
    Transform::from_translation(vec3_from_quantized(transform.translation)).with_rotation(
        Quat::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3]),
    )
}

pub fn vec3_from_quantized(value: QuantizedVec3) -> Vec3 {
    Vec3::from_array(value.to_f32(Quantization::MILLIMETERS))
}
