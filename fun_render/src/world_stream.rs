use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use bevy::{
    pbr::experimental::meshlet::{MeshletMesh, MeshletMesh3d},
    prelude::*,
    solari::prelude::RaytracingMesh3d,
};
use game_shared::{CatalogGeometry, RenderAssetId};
use thunder::prelude::*;

use crate::{
    ClientOpaqueRenderer, ClientRenderConfig, CompiledRenderAsset, DynamicInstanceClass,
    DynamicInstanceTable, FunGeometryClass, FunMaterialClass, FunRenderDistanceBand, FunRenderPath,
    FunRenderPathArbiter, FunRenderPathInput, InstanceGpuRecord, MaterialKey, RenderGeometryClass,
    RenderGeometryPolicy, StaticInstanceTable, StaticRenderBatch, StaticRenderBatchBuilder,
    StaticRenderBatchSpawnRecord, WorldRenderCatalog, material_policy_for_stream_color,
    static_catalog_spec_is_batchable, static_catalog_spec_needs_identity_proxy,
    warn_missing_catalog_ref,
};

#[derive(Debug, Default, Resource)]
pub struct RenderWorldContext {
    pub level_id: Option<String>,
    pub revision: Option<WorldRevision>,
    pub manifest_signature: u64,
    pub expected_chunks: u16,
    pub received_chunks: HashSet<u16>,
    pub queued_catalog_asset_loads: HashSet<WorldCatalogRef>,
    pub spawned_entities: HashMap<NetEntity, Entity>,
    pub spawned_static_batches: HashSet<Entity>,
}

impl RenderWorldContext {
    pub fn is_complete(&self) -> bool {
        self.expected_chunks > 0 && self.received_chunks.len() >= self.expected_chunks as usize
    }
}

#[derive(Debug, Default, Resource)]
pub struct RenderWorldStatus {
    pub ready: bool,
    pub render_prep_pending_chunks: usize,
    pub render_prep_applied_chunks: u64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RenderWorldApplyOptions {
    pub stream_verbose: bool,
    pub render_verbose: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderWorldChunkOutcome {
    pub validation_error: Option<WorldStreamValidationError>,
    pub render_prep_phase_mask: u8,
    pub phase_a_validated_entities: usize,
    pub phase_b_resolved_entities: usize,
    pub phase_c_static_instances_appended: usize,
    pub phase_d_uploads_scheduled: usize,
    pub phase_d_upload_bytes_scheduled: u64,
    pub phase_e_resident_visible_cells: usize,
    pub world_revision_changed: bool,
    pub spawned_entities: usize,
    pub duplicate_entities: usize,
    pub catalog_lookup_ns: u64,
    pub catalog_backed_entities: usize,
    pub missing_catalog_assets: usize,
    pub queued_catalog_asset_loads: usize,
    pub fallback_proxy_entities: usize,
    pub primitive_entities: usize,
    pub primitive_cache_hits: usize,
    pub primitive_cache_fallbacks: usize,
    pub primitive_cache_misses: usize,
    pub static_batch_entities: usize,
    pub static_batch_instances: usize,
    pub static_batch_cells: usize,
    pub static_identity_proxy_entities: usize,
    pub static_instance_table_records: usize,
    pub static_instance_table_bytes: u64,
    pub dynamic_instance_table_records: usize,
    pub dynamic_instance_dirty_bytes: u64,
    pub meshlet_mesh_assets_built: usize,
    pub raster_mesh_assets_built: usize,
    pub material_assets_created: usize,
}

pub const FUN_RENDER_WORLD_PREP_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RenderWorldPrepPhase {
    ReceiveValidate,
    ResolveCatalogRefs,
    AppendStaticInstances,
    ScheduleGpuBufferUploads,
    MarkCellResidentVisible,
}

impl RenderWorldPrepPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReceiveValidate => "receive_validate",
            Self::ResolveCatalogRefs => "resolve_catalog_refs",
            Self::AppendStaticInstances => "append_static_instances",
            Self::ScheduleGpuBufferUploads => "schedule_gpu_buffer_uploads",
            Self::MarkCellResidentVisible => "mark_cell_resident_visible",
        }
    }

    pub const fn bit(self) -> u8 {
        match self {
            Self::ReceiveValidate => 1 << 0,
            Self::ResolveCatalogRefs => 1 << 1,
            Self::AppendStaticInstances => 1 << 2,
            Self::ScheduleGpuBufferUploads => 1 << 3,
            Self::MarkCellResidentVisible => 1 << 4,
        }
    }
}

pub const RENDER_WORLD_PREP_PHASES: [RenderWorldPrepPhase; 5] = [
    RenderWorldPrepPhase::ReceiveValidate,
    RenderWorldPrepPhase::ResolveCatalogRefs,
    RenderWorldPrepPhase::AppendStaticInstances,
    RenderWorldPrepPhase::ScheduleGpuBufferUploads,
    RenderWorldPrepPhase::MarkCellResidentVisible,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RenderWorldFallbackProxyReason {
    MissingCatalogAsset,
    MissingPrimitiveCacheEntry,
}

impl RenderWorldFallbackProxyReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MissingCatalogAsset => "missing_catalog_asset",
            Self::MissingPrimitiveCacheEntry => "missing_primitive_cache_entry",
        }
    }
}

#[derive(Debug, Clone, Copy, Component, PartialEq, Eq)]
pub struct RenderWorldFallbackProxy {
    pub reason: RenderWorldFallbackProxyReason,
    pub catalog_ref: Option<WorldCatalogRef>,
    pub primitive: Option<PrimitiveRenderCachePrimitive>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PrimitiveRenderCachePrimitive {
    Plane,
    Cuboid,
}

impl PrimitiveRenderCachePrimitive {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Plane => "plane",
            Self::Cuboid => "cuboid",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PrimitiveSizeBucket {
    pub x_mm: i32,
    pub y_mm: i32,
    pub z_mm: i32,
}

impl PrimitiveSizeBucket {
    pub const fn from_millimeters(x_mm: i32, y_mm: i32, z_mm: i32) -> Self {
        Self { x_mm, y_mm, z_mm }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PrimitiveGeometryPolicyKey {
    Hybrid,
    RasterOnly,
    GpuCulledRaster,
    MeshletWhereSupported,
    VirtualStaticExperimental,
}

impl From<RenderGeometryPolicy> for PrimitiveGeometryPolicyKey {
    fn from(value: RenderGeometryPolicy) -> Self {
        match value {
            RenderGeometryPolicy::Hybrid => Self::Hybrid,
            RenderGeometryPolicy::RasterOnly => Self::RasterOnly,
            RenderGeometryPolicy::GpuCulledRaster => Self::GpuCulledRaster,
            RenderGeometryPolicy::MeshletWhereSupported => Self::MeshletWhereSupported,
            RenderGeometryPolicy::VirtualStaticExperimental => Self::VirtualStaticExperimental,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PrimitiveRenderCacheKey {
    pub primitive: PrimitiveRenderCachePrimitive,
    pub size_bucket: PrimitiveSizeBucket,
    pub material: MaterialKey,
    pub geometry_policy: PrimitiveGeometryPolicyKey,
}

#[derive(Debug, Clone)]
pub struct PrimitiveRenderHandles {
    pub catalog_asset_id: Option<RenderAssetId>,
    pub raster_mesh: Option<Handle<Mesh>>,
    pub meshlet_mesh: Option<Handle<MeshletMesh>>,
    pub material: Option<Handle<StandardMaterial>>,
    pub ray_proxy: Option<Handle<Mesh>>,
    pub geometry_class: RenderGeometryClass,
    pub fun_geometry_class: FunGeometryClass,
    pub render_path: FunRenderPath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveRenderCacheLookupKind {
    Exact,
    ShapeMaterialFallback,
    PrimitiveFallback,
}

#[derive(Debug, Clone)]
pub struct PrimitiveRenderCacheLookup {
    pub key: PrimitiveRenderCacheKey,
    pub kind: PrimitiveRenderCacheLookupKind,
    pub handles: PrimitiveRenderHandles,
}

#[derive(Debug, Default, Resource)]
pub struct PrimitiveRenderCache {
    handles: HashMap<PrimitiveRenderCacheKey, PrimitiveRenderHandles>,
    fallback_by_shape: HashMap<
        (
            PrimitiveRenderCachePrimitive,
            PrimitiveSizeBucket,
            PrimitiveGeometryPolicyKey,
        ),
        PrimitiveRenderCacheKey,
    >,
    fallback_by_primitive: HashMap<
        (PrimitiveRenderCachePrimitive, PrimitiveGeometryPolicyKey),
        PrimitiveRenderCacheKey,
    >,
}

impl PrimitiveRenderCache {
    pub fn len(&self) -> usize {
        self.handles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }

    pub fn from_catalog(catalog: &WorldRenderCatalog, render_config: &ClientRenderConfig) -> Self {
        let mut cache = Self::default();
        let geometry_policy = PrimitiveGeometryPolicyKey::from(render_config.geometry_policy);
        for compiled in catalog.compiled_assets() {
            let Some(geometry) = compiled.entry.geometry else {
                continue;
            };
            let cache_primitive = primitive_kind_from_catalog_geometry(geometry);
            let key = PrimitiveRenderCacheKey {
                primitive: cache_primitive,
                size_bucket: size_bucket_from_catalog_geometry(geometry),
                material: compiled.material_policy.shared_material_key,
                geometry_policy,
            };
            let handles = PrimitiveRenderHandles {
                catalog_asset_id: Some(compiled.entry.asset_id),
                raster_mesh: compiled.raster_mesh.clone(),
                meshlet_mesh: compiled.meshlet_mesh.clone(),
                material: compiled.material.clone(),
                ray_proxy: compiled.ray_proxy.clone(),
                geometry_class: compiled.geometry_class,
                fun_geometry_class: compiled.fun_geometry_class,
                render_path: compiled.render_path,
            };
            cache.insert(key, handles);
        }
        cache
    }

    pub fn lookup(
        &self,
        primitive: WorldPrimitive,
        material: Option<PackedColorRgba8>,
        render_config: &ClientRenderConfig,
    ) -> Option<PrimitiveRenderCacheLookup> {
        let key = PrimitiveRenderCacheKey {
            primitive: primitive_kind_from_world_primitive(primitive),
            size_bucket: size_bucket_from_world_primitive(primitive),
            material: material_key_from_stream_color(material),
            geometry_policy: PrimitiveGeometryPolicyKey::from(render_config.geometry_policy),
        };

        if let Some(handles) = self.handles.get(&key) {
            return Some(PrimitiveRenderCacheLookup {
                key,
                kind: PrimitiveRenderCacheLookupKind::Exact,
                handles: handles.clone(),
            });
        }

        let shape_key = (key.primitive, key.size_bucket, key.geometry_policy);
        if let Some(fallback_key) = self.fallback_by_shape.get(&shape_key).copied()
            && let Some(handles) = self.handles.get(&fallback_key)
        {
            return Some(PrimitiveRenderCacheLookup {
                key: fallback_key,
                kind: PrimitiveRenderCacheLookupKind::ShapeMaterialFallback,
                handles: handles.clone(),
            });
        }

        let primitive_key = (key.primitive, key.geometry_policy);
        if let Some(fallback_key) = self.fallback_by_primitive.get(&primitive_key).copied()
            && let Some(handles) = self.handles.get(&fallback_key)
        {
            return Some(PrimitiveRenderCacheLookup {
                key: fallback_key,
                kind: PrimitiveRenderCacheLookupKind::PrimitiveFallback,
                handles: handles.clone(),
            });
        }

        None
    }

    fn insert(&mut self, key: PrimitiveRenderCacheKey, handles: PrimitiveRenderHandles) {
        self.fallback_by_shape
            .entry((key.primitive, key.size_bucket, key.geometry_policy))
            .or_insert(key);
        self.fallback_by_primitive
            .entry((key.primitive, key.geometry_policy))
            .or_insert(key);
        self.handles.entry(key).or_insert(handles);
    }
}

pub fn prewarm_primitive_render_cache(
    mut commands: Commands,
    catalog: Res<WorldRenderCatalog>,
    render_config: Res<ClientRenderConfig>,
) {
    let cache = PrimitiveRenderCache::from_catalog(&catalog, &render_config);
    game_shared::fun_diag_info!(
        target: "fun::render_catalog",
        primitive_cache_entries = cache.len(),
        geometry_policy = ?render_config.geometry_policy,
        "primitive render cache prewarmed from catalog handles"
    );
    commands.insert_resource(cache);
}

pub fn despawn_render_context(
    commands: &mut Commands,
    context: &mut RenderWorldContext,
    status: &mut RenderWorldStatus,
) {
    let mut despawned = HashSet::new();
    for entity in context.spawned_entities.values().copied() {
        if despawned.insert(entity) {
            commands.entity(entity).despawn();
        }
    }
    for entity in context.spawned_static_batches.iter().copied() {
        if despawned.insert(entity) {
            commands.entity(entity).despawn();
        }
    }
    context.level_id = None;
    context.revision = None;
    context.manifest_signature = 0;
    context.expected_chunks = 0;
    context.received_chunks.clear();
    context.queued_catalog_asset_loads.clear();
    context.spawned_entities.clear();
    context.spawned_static_batches.clear();
    status.ready = false;
    status.render_prep_pending_chunks = 0;
    status.render_prep_applied_chunks = 0;
}

struct RenderWorldChunkPrepPlan<'a> {
    entities: Vec<RenderWorldPreparedEntity<'a>>,
    static_batch_builder: StaticRenderBatchBuilder,
    pending_static_batch_entities: HashSet<NetEntity>,
}

impl<'a> RenderWorldChunkPrepPlan<'a> {
    fn new(chunk_index: u16, capacity: usize) -> Self {
        Self {
            entities: Vec::with_capacity(capacity),
            static_batch_builder: StaticRenderBatchBuilder::new(chunk_index),
            pending_static_batch_entities: HashSet::new(),
        }
    }
}

struct RenderWorldPreparedEntity<'a> {
    spec: &'a WorldEntitySpec,
    source: RenderWorldPreparedSource<'a>,
}

enum RenderWorldPreparedSource<'a> {
    StaticCatalog {
        catalog_ref: WorldCatalogRef,
        compiled: &'a CompiledRenderAsset,
        needs_identity_proxy: bool,
    },
    Catalog {
        catalog_ref: WorldCatalogRef,
        compiled: &'a CompiledRenderAsset,
    },
    MissingCatalog {
        catalog_ref: WorldCatalogRef,
    },
    Primitive {
        primitive: WorldPrimitive,
        cached: Option<PrimitiveRenderCacheLookup>,
    },
    MetadataOnly,
}

#[allow(
    clippy::too_many_arguments,
    reason = "world-stream render hydration keeps catalog/cache resources explicit for deterministic editor and client callers"
)]
pub fn apply_render_world_chunk(
    commands: &mut Commands,
    context: &mut RenderWorldContext,
    status: &mut RenderWorldStatus,
    static_instance_table: &mut StaticInstanceTable,
    dynamic_instance_table: &mut DynamicInstanceTable,
    catalog: &WorldRenderCatalog,
    primitive_cache: &PrimitiveRenderCache,
    render_config: &ClientRenderConfig,
    options: RenderWorldApplyOptions,
    chunk: &WorldStreamChunk,
) -> RenderWorldChunkOutcome {
    let mut outcome = RenderWorldChunkOutcome::default();
    if !receive_and_validate_render_world_chunk(chunk, &mut outcome) {
        game_shared::fun_diag_warn!(
            target: "fun::stream",
            validation_error = ?outcome.validation_error,
            chunk_index = chunk.chunk_index,
            chunk_count = chunk.chunk_count,
            "rejected streamed render world chunk before render prep"
        );
        return outcome;
    }

    let opaque_renderer = opaque_renderer_from_render_config(render_config);
    if context.revision != Some(chunk.revision)
        || context.level_id.as_deref() != Some(chunk.level_id.0.as_str())
        || context.manifest_signature != chunk.manifest_signature
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
        static_instance_table.clear_for_world_swap();
        dynamic_instance_table.clear_for_world_swap();

        context.level_id = Some(chunk.level_id.0.clone());
        context.revision = Some(chunk.revision);
        context.manifest_signature = chunk.manifest_signature;
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

    let mut prep_plan = resolve_render_world_chunk_catalog_refs(
        context,
        catalog,
        primitive_cache,
        render_config,
        options,
        chunk,
        &mut outcome,
    );
    append_static_instances_to_batch_builders(
        commands,
        context,
        render_config,
        opaque_renderer,
        options,
        &mut prep_plan,
        &mut outcome,
    );
    spawn_dynamic_or_fallback_prepared_entities(
        commands,
        context,
        render_config,
        options,
        &prep_plan,
        dynamic_instance_table,
        &mut outcome,
    );

    spawn_static_render_batch_entities(
        commands,
        context,
        static_instance_table,
        options,
        prep_plan.static_batch_builder.finish(),
        &mut outcome,
    );

    schedule_render_world_gpu_uploads(static_instance_table, dynamic_instance_table, &mut outcome);
    mark_chunk_resident_and_visible(context, chunk, &mut outcome);
    game_shared::fun_diag_info_if!(
        options.stream_verbose,
        target: "fun::stream",
        received_chunks = context.received_chunks.len(),
        expected_chunks = context.expected_chunks,
        spawned_entities = context.spawned_entities.len(),
        phase_mask = outcome.render_prep_phase_mask,
        static_batch_entities = outcome.static_batch_entities,
        static_batch_instances = outcome.static_batch_instances,
        static_batch_cells = outcome.static_batch_cells,
        upload_schedules = outcome.phase_d_uploads_scheduled,
        upload_bytes = outcome.phase_d_upload_bytes_scheduled,
        missing_catalog_assets = outcome.missing_catalog_assets,
        static_instance_table_records = outcome.static_instance_table_records,
        dynamic_instance_table_records = outcome.dynamic_instance_table_records,
        "streamed render world chunk complete"
    );

    outcome
}

fn receive_and_validate_render_world_chunk(
    chunk: &WorldStreamChunk,
    outcome: &mut RenderWorldChunkOutcome,
) -> bool {
    mark_render_prep_phase(outcome, RenderWorldPrepPhase::ReceiveValidate);
    outcome.phase_a_validated_entities = chunk.entities.len();
    match chunk.validate_basic() {
        Ok(()) => true,
        Err(error) => {
            outcome.validation_error = Some(error);
            false
        }
    }
}

fn resolve_render_world_chunk_catalog_refs<'a>(
    context: &mut RenderWorldContext,
    catalog: &'a WorldRenderCatalog,
    primitive_cache: &PrimitiveRenderCache,
    render_config: &ClientRenderConfig,
    options: RenderWorldApplyOptions,
    chunk: &'a WorldStreamChunk,
    outcome: &mut RenderWorldChunkOutcome,
) -> RenderWorldChunkPrepPlan<'a> {
    let _ = options;
    mark_render_prep_phase(outcome, RenderWorldPrepPhase::ResolveCatalogRefs);
    let mut prep_plan = RenderWorldChunkPrepPlan::new(chunk.chunk_index, chunk.entities.len());
    let mut planned_entities = HashSet::with_capacity(chunk.entities.len());

    for spec in &chunk.entities {
        if context.spawned_entities.contains_key(&spec.entity)
            || !planned_entities.insert(spec.entity)
        {
            outcome.duplicate_entities = outcome.duplicate_entities.saturating_add(1);
            game_shared::fun_diag_warn!(
                target: "fun::stream",
                net_entity = spec.entity.0,
                name = %spec.name,
                "skipping duplicate streamed render entity"
            );
            continue;
        }

        let source = resolve_render_world_entity_source(
            context,
            catalog,
            primitive_cache,
            render_config,
            spec,
            outcome,
        );
        outcome.phase_b_resolved_entities = outcome.phase_b_resolved_entities.saturating_add(1);
        game_shared::fun_diag_debug_if!(
            options.stream_verbose,
            target: "fun::stream::prep",
            net_entity = spec.entity.0,
            name = %spec.name,
            phase = RenderWorldPrepPhase::ResolveCatalogRefs.as_str(),
            "resolved streamed entity render source"
        );
        prep_plan
            .entities
            .push(RenderWorldPreparedEntity { spec, source });
    }

    prep_plan
}

fn resolve_render_world_entity_source<'a>(
    context: &mut RenderWorldContext,
    catalog: &'a WorldRenderCatalog,
    primitive_cache: &PrimitiveRenderCache,
    render_config: &ClientRenderConfig,
    spec: &WorldEntitySpec,
    outcome: &mut RenderWorldChunkOutcome,
) -> RenderWorldPreparedSource<'a> {
    if let Some(catalog_ref) = spec.catalog {
        let lookup_started = Instant::now();
        let compiled = catalog.lookup(catalog_ref);
        outcome.catalog_lookup_ns = outcome.catalog_lookup_ns.saturating_add(
            lookup_started
                .elapsed()
                .as_nanos()
                .min(u128::from(u64::MAX)) as u64,
        );
        return match compiled {
            Some(compiled) if static_catalog_spec_is_batchable(spec, compiled) => {
                RenderWorldPreparedSource::StaticCatalog {
                    catalog_ref,
                    compiled,
                    needs_identity_proxy: static_catalog_spec_needs_identity_proxy(spec, compiled),
                }
            }
            Some(compiled) => RenderWorldPreparedSource::Catalog {
                catalog_ref,
                compiled,
            },
            None => {
                warn_missing_catalog_ref(catalog_ref, &spec.name);
                queue_missing_catalog_asset_load(context, catalog_ref, outcome);
                RenderWorldPreparedSource::MissingCatalog { catalog_ref }
            }
        };
    }

    if let Some(primitive) = spec.render {
        outcome.primitive_entities = outcome.primitive_entities.saturating_add(1);
        let lookup_started = Instant::now();
        let cached = primitive_cache.lookup(primitive, spec.color, render_config);
        outcome.catalog_lookup_ns = outcome.catalog_lookup_ns.saturating_add(
            lookup_started
                .elapsed()
                .as_nanos()
                .min(u128::from(u64::MAX)) as u64,
        );
        match cached.as_ref().map(|cached| cached.kind) {
            Some(PrimitiveRenderCacheLookupKind::Exact) => {
                outcome.primitive_cache_hits = outcome.primitive_cache_hits.saturating_add(1);
            }
            Some(
                PrimitiveRenderCacheLookupKind::ShapeMaterialFallback
                | PrimitiveRenderCacheLookupKind::PrimitiveFallback,
            ) => {
                outcome.primitive_cache_fallbacks =
                    outcome.primitive_cache_fallbacks.saturating_add(1);
            }
            None => {
                outcome.primitive_cache_misses = outcome.primitive_cache_misses.saturating_add(1);
            }
        }
        return RenderWorldPreparedSource::Primitive { primitive, cached };
    }

    RenderWorldPreparedSource::MetadataOnly
}

fn append_static_instances_to_batch_builders(
    commands: &mut Commands,
    context: &mut RenderWorldContext,
    render_config: &ClientRenderConfig,
    opaque_renderer: ClientOpaqueRenderer,
    options: RenderWorldApplyOptions,
    prep_plan: &mut RenderWorldChunkPrepPlan<'_>,
    outcome: &mut RenderWorldChunkOutcome,
) {
    let _ = options;
    mark_render_prep_phase(outcome, RenderWorldPrepPhase::AppendStaticInstances);
    for prepared in &mut prep_plan.entities {
        let RenderWorldPreparedSource::StaticCatalog {
            catalog_ref,
            compiled,
            needs_identity_proxy,
        } = prepared.source
        else {
            continue;
        };

        if !prep_plan.static_batch_builder.push_catalog_instance(
            prepared.spec,
            compiled,
            render_config,
            opaque_renderer,
            !needs_identity_proxy,
        ) {
            prepared.source = RenderWorldPreparedSource::Catalog {
                catalog_ref,
                compiled,
            };
            continue;
        }

        prep_plan
            .pending_static_batch_entities
            .insert(prepared.spec.entity);
        outcome.catalog_backed_entities = outcome.catalog_backed_entities.saturating_add(1);
        outcome.spawned_entities = outcome.spawned_entities.saturating_add(1);
        outcome.phase_c_static_instances_appended =
            outcome.phase_c_static_instances_appended.saturating_add(1);
        if needs_identity_proxy {
            let proxy = spawn_static_batch_identity_proxy_entity(commands, prepared.spec);
            context.spawned_entities.insert(prepared.spec.entity, proxy);
            outcome.static_identity_proxy_entities =
                outcome.static_identity_proxy_entities.saturating_add(1);
            game_shared::fun_diag_debug_if!(
                options.stream_verbose,
                target: "fun::stream",
                ecs_entity = ?proxy,
                net_entity = prepared.spec.entity.0,
                name = %prepared.spec.name,
                "spawned static render batch identity proxy"
            );
        }
    }
}

fn spawn_dynamic_or_fallback_prepared_entities(
    commands: &mut Commands,
    context: &mut RenderWorldContext,
    render_config: &ClientRenderConfig,
    options: RenderWorldApplyOptions,
    prep_plan: &RenderWorldChunkPrepPlan<'_>,
    dynamic_instance_table: &mut DynamicInstanceTable,
    outcome: &mut RenderWorldChunkOutcome,
) {
    for prepared in &prep_plan.entities {
        if prep_plan
            .pending_static_batch_entities
            .contains(&prepared.spec.entity)
        {
            continue;
        }

        let entity = spawn_prepared_render_entity_from_spec(
            commands,
            render_config,
            options,
            prepared.spec,
            &prepared.source,
            outcome,
        );
        game_shared::fun_diag_debug_if!(
            options.stream_verbose,
            target: "fun::stream",
            ecs_entity = ?entity,
            net_entity = prepared.spec.entity.0,
            name = %prepared.spec.name,
            "spawned streamed render entity"
        );
        context
            .spawned_entities
            .insert(prepared.spec.entity, entity);
        register_dynamic_instance_if_needed(prepared.spec, dynamic_instance_table, outcome);
        outcome.spawned_entities = outcome.spawned_entities.saturating_add(1);
    }
}

fn schedule_render_world_gpu_uploads(
    static_instance_table: &StaticInstanceTable,
    dynamic_instance_table: &DynamicInstanceTable,
    outcome: &mut RenderWorldChunkOutcome,
) {
    mark_render_prep_phase(outcome, RenderWorldPrepPhase::ScheduleGpuBufferUploads);
    for plan in static_instance_table
        .pending_upload_plans()
        .into_iter()
        .chain(dynamic_instance_table.pending_upload_plans())
    {
        outcome.phase_d_uploads_scheduled = outcome.phase_d_uploads_scheduled.saturating_add(1);
        outcome.phase_d_upload_bytes_scheduled = outcome
            .phase_d_upload_bytes_scheduled
            .saturating_add(plan.bytes);
    }
}

fn mark_chunk_resident_and_visible(
    context: &mut RenderWorldContext,
    chunk: &WorldStreamChunk,
    outcome: &mut RenderWorldChunkOutcome,
) {
    mark_render_prep_phase(outcome, RenderWorldPrepPhase::MarkCellResidentVisible);
    context.received_chunks.insert(chunk.chunk_index);
    outcome.phase_e_resident_visible_cells = outcome.static_batch_cells;
}

fn queue_missing_catalog_asset_load(
    context: &mut RenderWorldContext,
    catalog_ref: WorldCatalogRef,
    outcome: &mut RenderWorldChunkOutcome,
) {
    outcome.missing_catalog_assets = outcome.missing_catalog_assets.saturating_add(1);
    if context.queued_catalog_asset_loads.insert(catalog_ref) {
        outcome.queued_catalog_asset_loads = outcome.queued_catalog_asset_loads.saturating_add(1);
    }
}

fn mark_render_prep_phase(outcome: &mut RenderWorldChunkOutcome, phase: RenderWorldPrepPhase) {
    outcome.render_prep_phase_mask |= phase.bit();
}

fn spawn_static_batch_identity_proxy_entity(
    commands: &mut Commands,
    spec: &WorldEntitySpec,
) -> Entity {
    commands
        .spawn((
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
        ))
        .id()
}

fn spawn_static_render_batch_entities(
    commands: &mut Commands,
    context: &mut RenderWorldContext,
    static_instance_table: &mut StaticInstanceTable,
    _options: RenderWorldApplyOptions,
    records: Vec<StaticRenderBatchSpawnRecord>,
    outcome: &mut RenderWorldChunkOutcome,
) {
    let mut cells = HashSet::new();
    for mut record in records {
        let instance_count = record.batch.instance_count();
        let map_to_batch_entities = record.map_to_batch_entities.clone();
        cells.insert(record.batch.cell_id);
        let allocation = static_instance_table.register_static_batch(&record.batch);
        record.batch.instance_range = Some(allocation.range);
        record.batch.gpu_instance_buffer_handle = allocation.gpu_buffer_handle;
        let batch_entity = spawn_static_render_batch_entity(commands, record);
        context.spawned_static_batches.insert(batch_entity);
        for net_entity in map_to_batch_entities {
            context.spawned_entities.insert(net_entity, batch_entity);
        }
        outcome.static_batch_entities = outcome.static_batch_entities.saturating_add(1);
        outcome.static_batch_instances = outcome
            .static_batch_instances
            .saturating_add(instance_count);
        outcome.static_instance_table_records = outcome
            .static_instance_table_records
            .saturating_add(instance_count);
        outcome.static_instance_table_bytes = outcome
            .static_instance_table_bytes
            .saturating_add(allocation.range.byte_len());
        game_shared::fun_diag_debug_if!(
            _options.render_verbose,
            target: "fun::render::static_batch",
            ecs_entity = ?batch_entity,
            instances = instance_count,
            instance_range_start = allocation.range.start,
            instance_range_count = allocation.range.count,
            "spawned static render batch entity"
        );
    }
    outcome.static_batch_cells = cells.len();
}

fn register_dynamic_instance_if_needed(
    spec: &WorldEntitySpec,
    dynamic_instance_table: &mut DynamicInstanceTable,
    outcome: &mut RenderWorldChunkOutcome,
) {
    let Some(dynamic_class) = DynamicInstanceClass::from_replication_class(spec.class) else {
        return;
    };
    let record = InstanceGpuRecord::from_stream_spec(spec, dynamic_class);
    dynamic_instance_table.upsert_instance(spec.entity, dynamic_class, record);
    outcome.dynamic_instance_table_records =
        outcome.dynamic_instance_table_records.saturating_add(1);
    outcome.dynamic_instance_dirty_bytes = outcome
        .dynamic_instance_dirty_bytes
        .saturating_add(InstanceGpuRecord::SIZE_BYTES);
}

fn spawn_static_render_batch_entity(
    commands: &mut Commands,
    record: StaticRenderBatchSpawnRecord,
) -> Entity {
    let batch_name = static_batch_name(&record.batch);
    let transform = record
        .batch
        .transforms
        .first()
        .copied()
        .map(transform_from_quantized)
        .unwrap_or_default();
    let render_geometry_class = record.batch.key.render_geometry_class;
    let geometry_class = record.batch.key.geometry_class;
    let render_path = record.batch.key.render_path;
    commands
        .spawn((
            Name::new(batch_name),
            render_geometry_class,
            geometry_class,
            render_path,
            transform,
            GlobalTransform::default(),
            Visibility::default(),
            record.cell,
            record.batch,
        ))
        .id()
}

fn static_batch_name(batch: &StaticRenderBatch) -> String {
    format!(
        "StaticRenderBatch cell={} asset={} path={} instances={}",
        batch.cell_id.0,
        batch.key.catalog_asset_id,
        batch.key.render_path.as_str(),
        batch.instance_count()
    )
}

const fn opaque_renderer_from_render_config(
    render_config: &ClientRenderConfig,
) -> ClientOpaqueRenderer {
    if render_config.solari_enabled {
        ClientOpaqueRenderer::Deferred
    } else {
        ClientOpaqueRenderer::Forward
    }
}

fn spawn_prepared_render_entity_from_spec(
    commands: &mut Commands,
    render_config: &ClientRenderConfig,
    options: RenderWorldApplyOptions,
    spec: &WorldEntitySpec,
    source: &RenderWorldPreparedSource<'_>,
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

    match source {
        RenderWorldPreparedSource::StaticCatalog {
            catalog_ref,
            compiled,
            ..
        }
        | RenderWorldPreparedSource::Catalog {
            catalog_ref,
            compiled,
        } => {
            outcome.catalog_backed_entities = outcome.catalog_backed_entities.saturating_add(1);
            insert_compiled_render_handles(
                &mut entity_commands,
                compiled,
                render_config.solari_enabled,
            );
            log_catalog_render_insert(
                options,
                spec,
                *catalog_ref,
                compiled,
                render_config.solari_enabled,
            );
        }
        RenderWorldPreparedSource::MissingCatalog { catalog_ref } => {
            insert_fallback_proxy_components(
                &mut entity_commands,
                spec,
                render_config,
                RenderWorldFallbackProxyReason::MissingCatalogAsset,
                Some(*catalog_ref),
                None,
            );
            outcome.fallback_proxy_entities = outcome.fallback_proxy_entities.saturating_add(1);
        }
        RenderWorldPreparedSource::Primitive { primitive, cached } => {
            if let Some(cached) = cached {
                insert_primitive_render_handles(
                    &mut entity_commands,
                    &cached.handles,
                    render_config.solari_enabled,
                );

                game_shared::fun_diag_debug_if!(
                    options.render_verbose,
                    target: "fun::render::entity",
                    name = %spec.name,
                    primitive = cached.key.primitive.as_str(),
                    size_x_mm = cached.key.size_bucket.x_mm,
                    size_y_mm = cached.key.size_bucket.y_mm,
                    size_z_mm = cached.key.size_bucket.z_mm,
                    material_preset_id = cached.key.material.material_preset_id,
                    lookup_kind = ?cached.kind,
                    catalog_asset_id = cached.handles.catalog_asset_id.map(|id| id.0),
                    fun_geometry_class = cached.handles.fun_geometry_class.as_str(),
                    render_path = cached.handles.render_path.as_str(),
                    meshlet = cached.handles.render_path.uses_meshlet(),
                    raytracing = render_config.solari_enabled && cached.handles.ray_proxy.is_some(),
                    "inserted cached primitive render handles"
                );
            } else {
                insert_fallback_proxy_components(
                    &mut entity_commands,
                    spec,
                    render_config,
                    RenderWorldFallbackProxyReason::MissingPrimitiveCacheEntry,
                    None,
                    Some(primitive_kind_from_world_primitive(*primitive)),
                );
                outcome.fallback_proxy_entities = outcome.fallback_proxy_entities.saturating_add(1);
                game_shared::fun_diag_warn!(
                    target: "fun::render::entity",
                    name = %spec.name,
                    primitive = primitive_kind_from_world_primitive(*primitive).as_str(),
                    size_x_mm = size_bucket_from_world_primitive(*primitive).x_mm,
                    size_y_mm = size_bucket_from_world_primitive(*primitive).y_mm,
                    size_z_mm = size_bucket_from_world_primitive(*primitive).z_mm,
                    "streamed primitive had no prewarmed render-cache entry; spawned fallback proxy"
                );
            }
        }
        RenderWorldPreparedSource::MetadataOnly => {}
    }

    entity_commands.id()
}

#[allow(
    clippy::too_many_arguments,
    reason = "spawn wiring keeps catalog and primitive cache resources explicit while construction stays in prewarm"
)]
pub fn spawn_render_entity_from_spec(
    commands: &mut Commands,
    catalog: &WorldRenderCatalog,
    primitive_cache: &PrimitiveRenderCache,
    render_config: &ClientRenderConfig,
    options: RenderWorldApplyOptions,
    spec: &WorldEntitySpec,
    outcome: &mut RenderWorldChunkOutcome,
) -> Entity {
    let _ = (options.stream_verbose, options.render_verbose);
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
            outcome.catalog_backed_entities = outcome.catalog_backed_entities.saturating_add(1);
            entity_commands.insert((
                compiled.geometry_class,
                compiled.fun_geometry_class,
                compiled.render_path,
            ));
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
                fun_geometry_class = compiled.fun_geometry_class.as_str(),
                render_path = compiled.render_path.as_str(),
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
        outcome.primitive_entities = outcome.primitive_entities.saturating_add(1);
        let lookup_started = Instant::now();
        let cached = primitive_cache.lookup(primitive, spec.color, render_config);
        outcome.catalog_lookup_ns = outcome.catalog_lookup_ns.saturating_add(
            lookup_started
                .elapsed()
                .as_nanos()
                .min(u128::from(u64::MAX)) as u64,
        );

        if let Some(cached) = cached {
            match cached.kind {
                PrimitiveRenderCacheLookupKind::Exact => {
                    outcome.primitive_cache_hits = outcome.primitive_cache_hits.saturating_add(1);
                }
                PrimitiveRenderCacheLookupKind::ShapeMaterialFallback
                | PrimitiveRenderCacheLookupKind::PrimitiveFallback => {
                    outcome.primitive_cache_fallbacks =
                        outcome.primitive_cache_fallbacks.saturating_add(1);
                }
            }
            insert_primitive_render_handles(
                &mut entity_commands,
                &cached.handles,
                render_config.solari_enabled,
            );

            game_shared::fun_diag_debug_if!(
                options.render_verbose,
                target: "fun::render::entity",
                name = %spec.name,
                primitive = cached.key.primitive.as_str(),
                size_x_mm = cached.key.size_bucket.x_mm,
                size_y_mm = cached.key.size_bucket.y_mm,
                size_z_mm = cached.key.size_bucket.z_mm,
                material_preset_id = cached.key.material.material_preset_id,
                lookup_kind = ?cached.kind,
                catalog_asset_id = cached.handles.catalog_asset_id.map(|id| id.0),
                fun_geometry_class = cached.handles.fun_geometry_class.as_str(),
                render_path = cached.handles.render_path.as_str(),
                meshlet = cached.handles.render_path.uses_meshlet(),
                raytracing = render_config.solari_enabled && cached.handles.ray_proxy.is_some(),
                "inserted cached primitive render handles"
            );
        } else {
            outcome.primitive_cache_misses = outcome.primitive_cache_misses.saturating_add(1);
            let fun_geometry_class = fun_geometry_class_from_spec(spec);
            let render_path =
                primitive_render_path(spec, primitive, fun_geometry_class, render_config);
            let is_static = fun_geometry_class.is_static() && spec.class == ReplicationClass::World;
            entity_commands.insert((
                render_geometry_class_for_path(render_path, fun_geometry_class, is_static),
                fun_geometry_class,
                render_path,
            ));
            game_shared::fun_diag_warn!(
                target: "fun::render::entity",
                name = %spec.name,
                primitive = primitive_kind_from_world_primitive(primitive).as_str(),
                size_x_mm = size_bucket_from_world_primitive(primitive).x_mm,
                size_y_mm = size_bucket_from_world_primitive(primitive).y_mm,
                size_z_mm = size_bucket_from_world_primitive(primitive).z_mm,
                "streamed primitive had no prewarmed render-cache entry; skipped runtime asset construction"
            );
        }
    }

    entity_commands.id()
}

fn insert_compiled_render_handles(
    entity_commands: &mut EntityCommands<'_>,
    compiled: &CompiledRenderAsset,
    solari_enabled: bool,
) {
    entity_commands.insert((
        compiled.geometry_class,
        compiled.fun_geometry_class,
        compiled.render_path,
    ));
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

    if solari_enabled && let Some(ray_proxy) = compiled.ray_proxy.as_ref() {
        entity_commands.insert(RaytracingMesh3d(ray_proxy.clone()));
    }
}

fn insert_fallback_proxy_components(
    entity_commands: &mut EntityCommands<'_>,
    spec: &WorldEntitySpec,
    render_config: &ClientRenderConfig,
    reason: RenderWorldFallbackProxyReason,
    catalog_ref: Option<WorldCatalogRef>,
    primitive: Option<PrimitiveRenderCachePrimitive>,
) {
    let _ = (render_config, reason, catalog_ref, primitive);
    let fun_geometry_class = fun_geometry_class_from_spec(spec);
    let render_path = FunRenderPath::StandardRaster;
    let is_static = fun_geometry_class.is_static() && spec.class == ReplicationClass::World;
    entity_commands.insert((
        RenderWorldFallbackProxy {
            reason,
            catalog_ref,
            primitive,
        },
        render_geometry_class_for_path(render_path, fun_geometry_class, is_static),
        fun_geometry_class,
        render_path,
    ));
    game_shared::fun_diag_debug!(
        target: "fun::render::entity",
        name = %spec.name,
        reason = reason.as_str(),
        catalog_ref = ?catalog_ref,
        primitive = ?primitive.map(PrimitiveRenderCachePrimitive::as_str),
        geometry_policy = ?render_config.geometry_policy,
        "spawned streamed render fallback proxy"
    );
}

fn log_catalog_render_insert(
    options: RenderWorldApplyOptions,
    spec: &WorldEntitySpec,
    catalog_ref: WorldCatalogRef,
    compiled: &CompiledRenderAsset,
    solari_enabled: bool,
) {
    let _ = (options, spec, catalog_ref, compiled, solari_enabled);
    game_shared::fun_diag_info_if!(
        options.render_verbose,
        target: "fun::render_catalog",
        name = %spec.name,
        asset_id = catalog_ref.asset_id,
        material_id = catalog_ref.material_id,
        collider_id = catalog_ref.collider_id,
        catalog_asset = compiled.entry.name,
        geometry_class = ?compiled.geometry_class,
        fun_geometry_class = compiled.fun_geometry_class.as_str(),
        render_path = compiled.render_path.as_str(),
        triangles = compiled.triangle_count,
        meshlet = compiled.geometry_class.uses_meshlet(),
        raster = compiled.geometry_class.uses_raster_mesh(),
        raytracing = solari_enabled && compiled.ray_proxy.is_some(),
        "inserted catalog-backed streamed render components"
    );
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

fn insert_primitive_render_handles(
    entity_commands: &mut EntityCommands<'_>,
    handles: &PrimitiveRenderHandles,
    solari_enabled: bool,
) {
    entity_commands.insert((
        handles.geometry_class,
        handles.fun_geometry_class,
        handles.render_path,
    ));

    if handles.render_path.uses_meshlet() {
        if let (Some(meshlet_mesh), Some(material)) =
            (handles.meshlet_mesh.as_ref(), handles.material.as_ref())
        {
            entity_commands.insert((
                MeshletMesh3d(meshlet_mesh.clone()),
                MeshMaterial3d::<StandardMaterial>(material.clone()),
            ));
        }
    } else if handles.render_path.emits_visible_raster()
        && let (Some(mesh), Some(material)) =
            (handles.raster_mesh.as_ref(), handles.material.as_ref())
    {
        entity_commands.insert((
            Mesh3d(mesh.clone()),
            MeshMaterial3d::<StandardMaterial>(material.clone()),
        ));
    }

    if solari_enabled && let Some(ray_proxy) = handles.ray_proxy.as_ref() {
        entity_commands.insert(RaytracingMesh3d(ray_proxy.clone()));
    }
}

fn primitive_kind_from_world_primitive(primitive: WorldPrimitive) -> PrimitiveRenderCachePrimitive {
    match primitive {
        WorldPrimitive::Plane { .. } => PrimitiveRenderCachePrimitive::Plane,
        WorldPrimitive::Cuboid { .. } => PrimitiveRenderCachePrimitive::Cuboid,
    }
}

fn primitive_kind_from_catalog_geometry(
    geometry: CatalogGeometry,
) -> PrimitiveRenderCachePrimitive {
    match geometry {
        CatalogGeometry::Plane { .. } => PrimitiveRenderCachePrimitive::Plane,
        CatalogGeometry::Cuboid { .. } => PrimitiveRenderCachePrimitive::Cuboid,
    }
}

fn size_bucket_from_world_primitive(primitive: WorldPrimitive) -> PrimitiveSizeBucket {
    match primitive {
        WorldPrimitive::Plane { size } | WorldPrimitive::Cuboid { size } => {
            size_bucket_from_quantized_vec(size)
        }
    }
}

fn size_bucket_from_catalog_geometry(geometry: CatalogGeometry) -> PrimitiveSizeBucket {
    match geometry {
        CatalogGeometry::Plane { size } | CatalogGeometry::Cuboid { size } => {
            PrimitiveSizeBucket::from_millimeters(
                meters_to_millimeters(size[0]),
                meters_to_millimeters(size[1]),
                meters_to_millimeters(size[2]),
            )
        }
    }
}

const fn size_bucket_from_quantized_vec(size: QuantizedVec3) -> PrimitiveSizeBucket {
    PrimitiveSizeBucket::from_millimeters(size.x, size.y, size.z)
}

fn meters_to_millimeters(value: f32) -> i32 {
    (value * 1_000.0).round() as i32
}

fn material_key_from_stream_color(color: Option<PackedColorRgba8>) -> MaterialKey {
    material_policy_for_stream_color(color).shared_material_key
}

fn primitive_render_path(
    spec: &WorldEntitySpec,
    primitive: WorldPrimitive,
    geometry_class: FunGeometryClass,
    render_config: &ClientRenderConfig,
) -> FunRenderPath {
    let material_class = material_class_for_geometry(geometry_class);
    if matches!(
        material_class,
        FunMaterialClass::Transparent | FunMaterialClass::Viewmodel
    ) {
        return FunRenderPathArbiter::default().decide(FunRenderPathInput {
            material_class,
            geometry_class,
            viewmodel: geometry_class == FunGeometryClass::Viewmodel,
            ..Default::default()
        });
    }

    let is_static = geometry_class.is_static() && spec.class == ReplicationClass::World;
    let arbiter = FunRenderPathArbiter {
        static_meshlet_min_triangles: render_config.meshlet_min_triangles as u32,
        dynamic_meshlet_min_triangles: render_config.meshlet_min_triangles.saturating_mul(4) as u32,
        ..default()
    };
    let triangle_count = primitive_triangle_count(primitive);
    let input = FunRenderPathInput {
        triangle_count,
        meshlet_count: triangle_count.div_ceil(64).max(1),
        projected_screen_area: 0.08,
        material_class,
        geometry_class,
        is_static,
        high_cpu_culling_cost: !is_static,
        stable_buffer_layout: true,
        distance_band: FunRenderDistanceBand::Mid,
        ..Default::default()
    };

    match render_config.geometry_policy {
        RenderGeometryPolicy::RasterOnly => primitive_raster_only_path(input, arbiter),
        RenderGeometryPolicy::GpuCulledRaster => primitive_gpu_culled_raster_path(input, arbiter),
        RenderGeometryPolicy::MeshletWhereSupported => {
            primitive_meshlet_where_supported_path(input, arbiter, render_config.meshlets_enabled)
        }
        RenderGeometryPolicy::VirtualStaticExperimental => {
            primitive_meshlet_where_supported_path(input, arbiter, render_config.meshlets_enabled)
        }
        RenderGeometryPolicy::Hybrid => {
            primitive_meshlet_where_supported_path(input, arbiter, render_config.meshlets_enabled)
        }
    }
}

fn primitive_raster_only_path(
    input: FunRenderPathInput,
    arbiter: FunRenderPathArbiter,
) -> FunRenderPath {
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

fn primitive_gpu_culled_raster_path(
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

fn primitive_meshlet_where_supported_path(
    input: FunRenderPathInput,
    arbiter: FunRenderPathArbiter,
    meshlets_enabled: bool,
) -> FunRenderPath {
    if meshlets_enabled {
        arbiter.decide(input)
    } else {
        primitive_gpu_culled_raster_path(input, arbiter)
    }
}

fn render_geometry_class_for_path(
    render_path: FunRenderPath,
    geometry_class: FunGeometryClass,
    is_static: bool,
) -> RenderGeometryClass {
    match render_path {
        FunRenderPath::MeshletStaticDense => RenderGeometryClass::MeshletStaticDense,
        FunRenderPath::MeshletDynamicDense => RenderGeometryClass::MeshletDynamicDense,
        FunRenderPath::RayProxyOnly => RenderGeometryClass::RayProxyOnly,
        FunRenderPath::Viewmodel => RenderGeometryClass::Viewmodel,
        FunRenderPath::InstancedRaster if is_static => RenderGeometryClass::InstancedStaticRaster,
        FunRenderPath::GpuCulledIndirect if is_static => RenderGeometryClass::GpuCulledStaticRaster,
        FunRenderPath::GpuCulledIndirect => RenderGeometryClass::GpuCulledDynamicRaster,
        FunRenderPath::StandardRaster
            if material_class_for_geometry(geometry_class) == FunMaterialClass::Transparent =>
        {
            RenderGeometryClass::TransparentRaster
        }
        FunRenderPath::StandardRaster if geometry_class == FunGeometryClass::Foliage => {
            RenderGeometryClass::FoliageAggregate
        }
        FunRenderPath::StandardRaster | FunRenderPath::InstancedRaster => {
            RenderGeometryClass::SimpleRaster
        }
        FunRenderPath::CefUi | FunRenderPath::DebugOnly => RenderGeometryClass::SimpleRaster,
    }
}

fn fun_geometry_class_from_spec(spec: &WorldEntitySpec) -> FunGeometryClass {
    match spec.class {
        ReplicationClass::Pawn => FunGeometryClass::SkinnedCharacter,
        ReplicationClass::Projectile => FunGeometryClass::Particle,
        ReplicationClass::Destructible
        | ReplicationClass::Objective
        | ReplicationClass::Custom(_) => FunGeometryClass::DynamicOpaque,
        ReplicationClass::Vehicle => FunGeometryClass::Vehicle,
        ReplicationClass::World => FunGeometryClass::StaticOpaqueSimple,
    }
}

fn material_class_for_geometry(geometry_class: FunGeometryClass) -> FunMaterialClass {
    match geometry_class {
        FunGeometryClass::Particle | FunGeometryClass::Decal => FunMaterialClass::Transparent,
        FunGeometryClass::Viewmodel => FunMaterialClass::Viewmodel,
        FunGeometryClass::Ui => FunMaterialClass::Ui,
        FunGeometryClass::StaticOpaqueDense
        | FunGeometryClass::DynamicOpaque
        | FunGeometryClass::SkinnedCharacter
        | FunGeometryClass::Vehicle => FunMaterialClass::OpaqueComplex,
        FunGeometryClass::StaticOpaqueSimple | FunGeometryClass::Foliage => {
            FunMaterialClass::OpaqueSimple
        }
    }
}

const fn primitive_triangle_count(primitive: WorldPrimitive) -> u32 {
    match primitive {
        WorldPrimitive::Plane { .. } => 2,
        WorldPrimitive::Cuboid { .. } => 12,
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CompiledRenderAsset;

    fn test_render_config() -> ClientRenderConfig {
        let mut config = ClientRenderConfig::from_env();
        config.solari_enabled = false;
        config.meshlets_enabled = false;
        config.geometry_policy = RenderGeometryPolicy::Hybrid;
        config
    }

    fn test_wall_catalog() -> WorldRenderCatalog {
        test_catalog_for_asset(game_shared::ASSET_WALL)
    }

    fn test_catalog_for_asset(asset_id: game_shared::RenderAssetId) -> WorldRenderCatalog {
        let mut catalog = WorldRenderCatalog::default();
        let entry = game_shared::demo_catalog_entry(asset_id).expect("demo catalog entry exists");
        let material_policy = crate::material_policy_for_catalog_entry(entry);
        let render_batch_key = crate::render_batch_key_for_catalog_entry(
            entry,
            FunRenderPath::StandardRaster,
            RenderGeometryClass::SimpleRaster,
            FunGeometryClass::StaticOpaqueSimple,
            FunMaterialClass::OpaqueSimple,
            material_policy,
        );
        catalog.insert_compiled_for_test(CompiledRenderAsset {
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
        });
        catalog
    }

    #[derive(Resource)]
    struct TestChunk(WorldStreamChunk);

    #[derive(Resource)]
    struct TestOutcome(RenderWorldChunkOutcome);

    #[allow(
        clippy::too_many_arguments,
        reason = "test system mirrors the production chunk-apply resource boundary"
    )]
    fn apply_static_batch_test_chunk(
        mut commands: Commands,
        mut context: ResMut<RenderWorldContext>,
        mut status: ResMut<RenderWorldStatus>,
        mut static_instance_table: ResMut<StaticInstanceTable>,
        mut dynamic_instance_table: ResMut<DynamicInstanceTable>,
        catalog: Res<WorldRenderCatalog>,
        primitive_cache: Res<PrimitiveRenderCache>,
        render_config: Res<ClientRenderConfig>,
        chunk: Res<TestChunk>,
    ) {
        let outcome = apply_render_world_chunk(
            &mut commands,
            &mut context,
            &mut status,
            &mut static_instance_table,
            &mut dynamic_instance_table,
            &catalog,
            &primitive_cache,
            &render_config,
            RenderWorldApplyOptions::default(),
            &chunk.0,
        );
        commands.insert_resource(TestOutcome(outcome));
    }

    fn static_catalog_spec(entity: u64, asset_id: game_shared::RenderAssetId) -> WorldEntitySpec {
        WorldEntitySpec {
            entity: NetEntity(entity),
            name: format!("static-catalog-{entity}"),
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
    fn render_prep_phase_order_is_stable() {
        assert_eq!(FUN_RENDER_WORLD_PREP_SCHEMA_VERSION, 1);
        assert_eq!(
            RENDER_WORLD_PREP_PHASES.map(RenderWorldPrepPhase::as_str),
            [
                "receive_validate",
                "resolve_catalog_refs",
                "append_static_instances",
                "schedule_gpu_buffer_uploads",
                "mark_cell_resident_visible",
            ]
        );
    }

    #[test]
    fn apply_render_world_chunk_collapses_static_catalog_visuals_into_batches() {
        let mut app = App::new();
        app.insert_resource(test_render_config())
            .insert_resource(test_catalog_for_asset(game_shared::ASSET_FLOOR))
            .insert_resource(PrimitiveRenderCache::default())
            .init_resource::<StaticInstanceTable>()
            .init_resource::<DynamicInstanceTable>()
            .insert_resource(TestChunk(WorldStreamChunk {
                level_id: WorldLevelId("static-batch-test".to_owned()),
                revision: WorldRevision(1),
                chunk_index: 0,
                chunk_count: 1,
                manifest_signature: 0x5a17_c001,
                entities: vec![
                    static_catalog_spec(1, game_shared::ASSET_FLOOR),
                    static_catalog_spec(2, game_shared::ASSET_FLOOR),
                ],
            }))
            .init_resource::<RenderWorldContext>()
            .init_resource::<RenderWorldStatus>()
            .add_systems(Update, apply_static_batch_test_chunk);

        app.update();

        let outcome = app.world().resource::<TestOutcome>().0;
        assert_eq!(outcome.spawned_entities, 2);
        assert_eq!(outcome.static_batch_entities, 1);
        assert_eq!(outcome.static_batch_instances, 2);
        assert_eq!(outcome.static_identity_proxy_entities, 0);
        assert_eq!(outcome.static_instance_table_records, 2);
        assert_eq!(outcome.phase_c_static_instances_appended, 2);
        assert_eq!(outcome.phase_d_uploads_scheduled, 1);
        assert_eq!(
            outcome.static_instance_table_bytes,
            InstanceGpuRecord::SIZE_BYTES * 2
        );
        assert_eq!(
            outcome.render_prep_phase_mask,
            RENDER_WORLD_PREP_PHASES
                .iter()
                .fold(0, |mask, phase| mask | phase.bit())
        );

        let context = app.world().resource::<RenderWorldContext>();
        assert_eq!(context.spawned_entities.len(), 2);
        assert_eq!(context.spawned_static_batches.len(), 1);
        assert_eq!(
            context
                .spawned_entities
                .values()
                .copied()
                .collect::<HashSet<_>>()
                .len(),
            1
        );

        let world = app.world_mut();
        let mut batches = world.query::<&StaticRenderBatch>();
        let instance_counts = batches
            .iter(world)
            .map(StaticRenderBatch::instance_count)
            .collect::<Vec<_>>();
        assert_eq!(instance_counts, vec![2]);
    }

    #[test]
    fn missing_catalog_asset_queues_load_and_spawns_fallback_proxy() {
        let missing_ref = WorldCatalogRef {
            asset_id: 999_001,
            material_id: 0,
            collider_id: 0,
        };
        let mut app = App::new();
        app.insert_resource(test_render_config())
            .insert_resource(WorldRenderCatalog::default())
            .insert_resource(PrimitiveRenderCache::default())
            .init_resource::<StaticInstanceTable>()
            .init_resource::<DynamicInstanceTable>()
            .insert_resource(TestChunk(WorldStreamChunk {
                level_id: WorldLevelId("missing-catalog-test".to_owned()),
                revision: WorldRevision(1),
                chunk_index: 0,
                chunk_count: 1,
                manifest_signature: 0x5a17_c002,
                entities: vec![WorldEntitySpec {
                    catalog: Some(missing_ref),
                    ..static_catalog_spec(1, game_shared::ASSET_FLOOR)
                }],
            }))
            .init_resource::<RenderWorldContext>()
            .init_resource::<RenderWorldStatus>()
            .add_systems(Update, apply_static_batch_test_chunk);

        app.update();

        let outcome = app.world().resource::<TestOutcome>().0;
        assert_eq!(outcome.missing_catalog_assets, 1);
        assert_eq!(outcome.queued_catalog_asset_loads, 1);
        assert_eq!(outcome.fallback_proxy_entities, 1);
        assert_eq!(outcome.static_batch_entities, 0);
        assert_eq!(outcome.meshlet_mesh_assets_built, 0);
        assert_eq!(outcome.raster_mesh_assets_built, 0);
        assert_eq!(outcome.material_assets_created, 0);
        assert!(
            app.world()
                .resource::<RenderWorldContext>()
                .queued_catalog_asset_loads
                .contains(&missing_ref)
        );

        let world = app.world_mut();
        let mut proxies = world.query::<&RenderWorldFallbackProxy>();
        let proxies = proxies.iter(world).collect::<Vec<_>>();
        assert_eq!(proxies.len(), 1);
        assert_eq!(
            proxies[0].reason,
            RenderWorldFallbackProxyReason::MissingCatalogAsset
        );
        assert_eq!(proxies[0].catalog_ref, Some(missing_ref));
    }

    #[test]
    fn static_destructible_catalog_object_starts_in_static_batch() {
        let mut destructible = static_catalog_spec(9, game_shared::ASSET_WALL);
        destructible.class = ReplicationClass::Destructible;

        let mut app = App::new();
        app.insert_resource(test_render_config())
            .insert_resource(test_wall_catalog())
            .insert_resource(PrimitiveRenderCache::default())
            .init_resource::<StaticInstanceTable>()
            .init_resource::<DynamicInstanceTable>()
            .insert_resource(TestChunk(WorldStreamChunk {
                level_id: WorldLevelId("destructible-static-test".to_owned()),
                revision: WorldRevision(1),
                chunk_index: 0,
                chunk_count: 1,
                manifest_signature: 0x5a17_c003,
                entities: vec![destructible],
            }))
            .init_resource::<RenderWorldContext>()
            .init_resource::<RenderWorldStatus>()
            .add_systems(Update, apply_static_batch_test_chunk);

        app.update();

        let outcome = app.world().resource::<TestOutcome>().0;
        assert_eq!(outcome.static_batch_entities, 1);
        assert_eq!(outcome.static_batch_instances, 1);
        assert_eq!(outcome.dynamic_instance_table_records, 0);
        assert_eq!(
            app.world()
                .resource::<DynamicInstanceTable>()
                .records()
                .len(),
            0
        );
    }

    #[test]
    fn primitive_render_cache_uses_catalog_handles_for_common_shape_and_material() {
        let config = test_render_config();
        let catalog = test_wall_catalog();
        let cache = PrimitiveRenderCache::from_catalog(&catalog, &config);
        let wall_material = game_shared::material_preset(game_shared::MATERIAL_WALL)
            .expect("wall material preset exists");
        let lookup = cache
            .lookup(
                WorldPrimitive::Cuboid {
                    size: QuantizedVec3::from_f32([5.0, 3.0, 1.0], Quantization::MILLIMETERS),
                },
                Some(PackedColorRgba8 {
                    r: wall_material.srgb[0],
                    g: wall_material.srgb[1],
                    b: wall_material.srgb[2],
                    a: wall_material.srgb[3],
                }),
                &config,
            )
            .expect("common wall primitive should be catalog-backed");

        assert_eq!(lookup.kind, PrimitiveRenderCacheLookupKind::Exact);
        assert_eq!(
            lookup.handles.catalog_asset_id,
            Some(game_shared::ASSET_WALL)
        );
        assert!(lookup.handles.raster_mesh.is_some());
        assert!(lookup.handles.material.is_some());
    }

    #[test]
    fn primitive_render_cache_falls_back_by_shape_without_creating_materials() {
        let config = test_render_config();
        let catalog = test_wall_catalog();
        let cache = PrimitiveRenderCache::from_catalog(&catalog, &config);
        let lookup = cache
            .lookup(
                WorldPrimitive::Cuboid {
                    size: QuantizedVec3::from_f32([5.0, 3.0, 1.0], Quantization::MILLIMETERS),
                },
                Some(PackedColorRgba8::srgb(1, 2, 3)),
                &config,
            )
            .expect("common shape should fall back to prewarmed material handle");

        assert_eq!(
            lookup.kind,
            PrimitiveRenderCacheLookupKind::ShapeMaterialFallback
        );
        assert_eq!(
            lookup.handles.catalog_asset_id,
            Some(game_shared::ASSET_WALL)
        );
    }

    #[test]
    fn primitive_render_cache_miss_does_not_build_runtime_assets() {
        let config = test_render_config();
        let cache = PrimitiveRenderCache::default();
        let missing = cache.lookup(
            WorldPrimitive::Cuboid {
                size: QuantizedVec3::from_f32([7.0, 7.0, 7.0], Quantization::MILLIMETERS),
            },
            Some(PackedColorRgba8::srgb(1, 2, 3)),
            &config,
        );

        assert!(missing.is_none());
    }
}
