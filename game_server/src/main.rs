#[cfg(all(feature = "diagnostics", debug_assertions))]
use std::time::Instant;
use std::{collections::HashSet, time::Duration};

use avian3d::prelude::{Collider, PhysicsPlugins, RigidBody};
use bevy::{
    app::ScheduleRunnerPlugin,
    asset::AssetPlugin,
    log::LogPlugin,
    mesh::MeshPlugin,
    prelude::*,
    scene::{
        ScenePlugin,
        prelude::{CommandsSceneExt, Scene as BsnScene, bsn, bsn_list},
        template_value,
    },
};
use bevy_quinnet::server::{
    ConnectionEvent, EndpointAddrConfiguration, QuinnetServer, QuinnetServerPlugin,
    ServerEndpointConfiguration, ServerEndpointConfigurationDefaultables,
    certificate::CertificateRetrievalMode,
};
use game_shared::{
    ASSET_COVER_CUBE, ASSET_FLOOR, ASSET_FLOOR_COLLIDER, ASSET_RAMP, ASSET_WALL,
    COLLIDER_COVER_CUBE, COLLIDER_FLOOR, COLLIDER_RAMP, COLLIDER_WALL, DEFAULT_TICK_RATE_HZ,
    DEMO_LEVEL_ID, GAME_SERVER_BIND_ADDR, GAME_TITLE, MATERIAL_COVER, MATERIAL_FLOOR,
    MATERIAL_RAMP, MATERIAL_WALL,
};
use thunder::prelude::*;
use tracing::{error, info};

const WORLD_STREAM_ENTITIES_PER_CHUNK: usize = 16;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
            1.0 / DEFAULT_TICK_RATE_HZ,
        ))),
        AssetPlugin::default(),
        LogPlugin::default(),
        MeshPlugin,
        ScenePlugin,
        PhysicsPlugins::default(),
        QuinnetServerPlugin::default(),
        ThunderPlugin::default(),
    ))
    .init_resource::<ServerWorldStream>()
    .init_resource::<ServerEditorSchema>()
    .insert_resource(ServerLogConfig::from_env())
    .init_resource::<ConnectedClients>()
    .init_resource::<PendingWorldStreams>()
    .add_systems(
        Startup,
        (
            start_endpoint,
            start_server_editor_inspector,
            spawn_demo_world,
        )
            .chain(),
    )
    .add_systems(
        Update,
        (
            receive_client_control,
            rebuild_world_stream,
            queue_world_stream_for_new_clients,
            send_pending_world_streams,
        )
            .chain(),
    );

    #[cfg(all(feature = "diagnostics", debug_assertions))]
    app.init_resource::<ServerWorldDiagnostics>()
        .init_resource::<ServerProfiler>()
        .init_resource::<ServerEditorControlPlane>()
        .add_systems(
            Startup,
            (log_server_editor_control_plane, log_server_editor_schema),
        )
        .add_systems(PreUpdate, begin_server_profiler_tick)
        .add_systems(Update, log_streamable_inventory);

    app.run();
}

fn start_endpoint(mut server: ResMut<QuinnetServer>) {
    let limits = ChannelLimits::default();
    server
        .start_endpoint(ServerEndpointConfiguration {
            addr_config: EndpointAddrConfiguration::from_string(GAME_SERVER_BIND_ADDR)
                .expect("game server bind address should be valid"),
            cert_mode: CertificateRetrievalMode::GenerateSelfSigned {
                server_hostname: "127.0.0.1".to_owned(),
            },
            defaultables: ServerEndpointConfigurationDefaultables {
                send_channels_cfg: ServerChannel::channels_configuration(limits),
                ..Default::default()
            },
        })
        .expect("game server endpoint should start");

    info!(
        "Starting {GAME_TITLE} game server at {DEFAULT_TICK_RATE_HZ:.0} Hz on {GAME_SERVER_BIND_ADDR}"
    );
}

fn spawn_demo_world(mut commands: Commands) {
    info!("[server world] queueing demo world BSN spawn");
    commands.spawn_scene_list(bsn_list![
        (
            #Floor
            template_value(Networked::world())
            template_value(StreamedWorldEntity::catalog(
                catalog_ref(ASSET_FLOOR.0, MATERIAL_FLOOR.0, 0),
            ))
            Transform::default()
        ),
        (
            #FloorCollider
            Name::new("FloorCollider")
            template_value(Networked::world())
            template_value(StreamedWorldEntity::catalog(
                catalog_ref(ASSET_FLOOR_COLLIDER.0, MATERIAL_FLOOR.0, COLLIDER_FLOOR.0),
            ))
            template_value(RigidBody::Static)
            Collider::cuboid(60.0, 0.5, 60.0)
            Transform::from_xyz(0.0, -0.25, 0.0)
        ),
        (
            #Wall
            template_value(Networked::world())
            template_value(StreamedWorldEntity::catalog(
                catalog_ref(ASSET_WALL.0, MATERIAL_WALL.0, COLLIDER_WALL.0),
            ))
            template_value(RigidBody::Static)
            Collider::cuboid(5.0, 3.0, 1.0)
            Transform::from_xyz(0.0, 1.5, -8.0)
        ),
        (
            #Ramp
            template_value(Networked::world())
            template_value(StreamedWorldEntity::catalog(
                catalog_ref(ASSET_RAMP.0, MATERIAL_RAMP.0, COLLIDER_RAMP.0),
            ))
            template_value(RigidBody::Static)
            Collider::cuboid(3.0, 0.5, 6.0)
            template_value(Transform::from_xyz(-6.0, 0.25, -2.0)
                .with_rotation(Quat::from_rotation_z(-12.0_f32.to_radians())))
        ),
        demo_cube(Vec3::new(3.0, 1.0, 2.0)),
        demo_cube(Vec3::new(5.0, 1.0, -1.5)),
        demo_cube(Vec3::new(7.0, 2.0, 4.0)),
    ]);
}

fn demo_cube(translation: Vec3) -> impl BsnScene {
    bsn! {
        template_value(Networked::world())
        template_value(StreamedWorldEntity::catalog(
            catalog_ref(ASSET_COVER_CUBE.0, MATERIAL_COVER.0, COLLIDER_COVER_CUBE.0),
        ))
        template_value(RigidBody::Static)
        Collider::cuboid(1.0, 1.0, 1.0)
        template_value(Transform::from_translation(translation))
    }
}

#[derive(Debug, Default, Clone, Component)]
struct StreamedWorldEntity {
    catalog: Option<WorldCatalogRef>,
    render: Option<WorldPrimitive>,
    collider: Option<WorldCollider>,
    color: Option<PackedColorRgba8>,
}

impl StreamedWorldEntity {
    fn catalog(catalog: WorldCatalogRef) -> Self {
        Self {
            catalog: Some(catalog),
            render: None,
            collider: None,
            color: None,
        }
    }
}

#[derive(Resource)]
struct ServerEditorSchema {
    #[cfg_attr(not(all(feature = "diagnostics", debug_assertions)), allow(dead_code))]
    registry: game_shared::EditorSchemaRegistry<'static>,
}

impl Default for ServerEditorSchema {
    fn default() -> Self {
        Self {
            registry: game_shared::EditorSchemaRegistry::new(
                server_editor_components(),
                server_editor_resources(),
            ),
        }
    }
}

fn server_editor_components() -> &'static [game_shared::EditorComponentRegistration] {
    game_shared::editor_component_registry! {
        transform => {
            kind: game_shared::EDITOR_COMPONENT_KIND_TRANSFORM,
            type: Transform,
            reflect: "bevy_transform::components::transform::Transform",
            serializer: "fun.editor.transform.encode.v1",
            deserializer: "fun.editor.transform.decode.v1",
            validator: game_shared::validate_non_empty_payload,
            capability: game_shared::EditorCapability::MutateEntities,
            mutability: game_shared::EditorMutability::RuntimeMutable,
            serialization: game_shared::EditorSerializationPolicy::Compactly,
            replication: game_shared::EditorReplicationPolicy::ServerAuthoritative,
            ui: game_shared::EditorUiHints::new(
                "Transform",
                "Transform",
                game_shared::EditorSchemaWidget::Transform3d,
                game_shared::EditorSchemaImportance::Primary,
            ),
            diagnostics: game_shared::EditorDiagnosticLabels::new(
                "fun::editor::schema",
                "server_transform",
            ),
            max_payload_bytes: 128,
        },
        name => {
            kind: game_shared::EDITOR_COMPONENT_KIND_NAME,
            type: Name,
            reflect: "bevy_ecs::name::Name",
            serializer: "fun.editor.name.encode.v1",
            deserializer: "fun.editor.name.decode.v1",
            validator: game_shared::validate_non_empty_payload,
            capability: game_shared::EditorCapability::MutateEntities,
            mutability: game_shared::EditorMutability::RuntimeMutable,
            serialization: game_shared::EditorSerializationPolicy::Compactly,
            replication: game_shared::EditorReplicationPolicy::ServerAuthoritative,
            ui: game_shared::EditorUiHints::new(
                "Name",
                "Identity",
                game_shared::EditorSchemaWidget::Text,
                game_shared::EditorSchemaImportance::Secondary,
            ),
            diagnostics: game_shared::EditorDiagnosticLabels::new(
                "fun::editor::schema",
                "server_name",
            ),
            max_payload_bytes: 256,
        },
        network_identity => {
            kind: game_shared::EDITOR_COMPONENT_KIND_NETWORK_IDENTITY,
            type: NetworkIdentity,
            reflect: "thunder::bevy_integration::NetworkIdentity",
            serializer: "fun.editor.network_identity.encode.v1",
            deserializer: "fun.editor.network_identity.decode.v1",
            validator: game_shared::reject_editor_mutation,
            capability: game_shared::EditorCapability::MutateEntities,
            mutability: game_shared::EditorMutability::ReadOnly,
            serialization: game_shared::EditorSerializationPolicy::Compactly,
            replication: game_shared::EditorReplicationPolicy::ServerAuthoritative,
            ui: game_shared::EditorUiHints::new(
                "Network Identity",
                "Network",
                game_shared::EditorSchemaWidget::ReadOnlyStruct,
                game_shared::EditorSchemaImportance::Diagnostic,
            ),
            diagnostics: game_shared::EditorDiagnosticLabels::new(
                "fun::editor::schema",
                "server_network_identity",
            ),
            max_payload_bytes: 64,
        },
        network_authority => {
            kind: game_shared::EDITOR_COMPONENT_KIND_NETWORK_AUTHORITY,
            type: NetworkAuthority,
            reflect: "thunder::bevy_integration::NetworkAuthority",
            serializer: "fun.editor.network_authority.encode.v1",
            deserializer: "fun.editor.network_authority.decode.v1",
            validator: game_shared::reject_editor_mutation,
            capability: game_shared::EditorCapability::MutateEntities,
            mutability: game_shared::EditorMutability::ReadOnly,
            serialization: game_shared::EditorSerializationPolicy::Compactly,
            replication: game_shared::EditorReplicationPolicy::ServerAuthoritative,
            ui: game_shared::EditorUiHints::new(
                "Authority",
                "Network",
                game_shared::EditorSchemaWidget::ReadOnlyStruct,
                game_shared::EditorSchemaImportance::Diagnostic,
            ),
            diagnostics: game_shared::EditorDiagnosticLabels::new(
                "fun::editor::schema",
                "server_network_authority",
            ),
            max_payload_bytes: 32,
        },
        streamed_world => {
            kind: game_shared::EDITOR_COMPONENT_KIND_WORLD_CATALOG_REF,
            type: StreamedWorldEntity,
            reflect: "game_server::StreamedWorldEntity",
            serializer: "fun.editor.streamed_world.encode.v1",
            deserializer: "fun.editor.streamed_world.decode.v1",
            validator: game_shared::validate_non_empty_payload,
            capability: game_shared::EditorCapability::ApplyScenePatch,
            mutability: game_shared::EditorMutability::PersistentMutable,
            serialization: game_shared::EditorSerializationPolicy::Compactly,
            replication: game_shared::EditorReplicationPolicy::ServerAuthoritative,
            ui: game_shared::EditorUiHints::new(
                "World Catalog",
                "World",
                game_shared::EditorSchemaWidget::ReadOnlyStruct,
                game_shared::EditorSchemaImportance::Advanced,
            ),
            diagnostics: game_shared::EditorDiagnosticLabels::new(
                "fun::editor::schema",
                "server_streamed_world",
            ),
            max_payload_bytes: 128,
        },
    }
}

fn server_editor_resources() -> &'static [game_shared::EditorResourceRegistration] {
    game_shared::editor_resource_registry! {
        diagnostics => {
            kind: game_shared::EDITOR_RESOURCE_KIND_RUNTIME_DIAGNOSTICS,
            type: ServerLogConfig,
            reflect: "game_server::ServerLogConfig",
            serializer: "fun.editor.server_log_config.encode.v1",
            deserializer: "fun.editor.server_log_config.decode.v1",
            validator: game_shared::validate_non_empty_payload,
            capability: game_shared::EditorCapability::ControlRuntime,
            mutability: game_shared::EditorMutability::RuntimeMutable,
            policy: game_shared::EditorResourcePolicy::VisualRuntimeSetting,
            ui: game_shared::EditorUiHints::new(
                "Runtime Diagnostics",
                "Diagnostics",
                game_shared::EditorSchemaWidget::Toggle,
                game_shared::EditorSchemaImportance::Advanced,
            ),
            diagnostics: game_shared::EditorDiagnosticLabels::new(
                "fun::editor::schema",
                "server_runtime_diagnostics",
            ),
            max_payload_bytes: 16,
        },
        match_state => {
            kind: game_shared::EDITOR_RESOURCE_KIND_MATCH_STATE,
            type: ServerWorldStream,
            reflect: "game_server::ServerWorldStream",
            serializer: "fun.editor.server_world_stream.encode.v1",
            deserializer: "fun.editor.server_world_stream.decode.v1",
            validator: game_shared::reject_editor_mutation,
            capability: game_shared::EditorCapability::PersistIteration,
            mutability: game_shared::EditorMutability::ReadOnly,
            policy: game_shared::EditorResourcePolicy::CriticalServerState,
            ui: game_shared::EditorUiHints::new(
                "Match World Stream",
                "Server",
                game_shared::EditorSchemaWidget::ResourcePanel,
                game_shared::EditorSchemaImportance::Diagnostic,
            ),
            diagnostics: game_shared::EditorDiagnosticLabels::new(
                "fun::editor::schema",
                "server_match_state",
            ),
            max_payload_bytes: 0,
        },
    }
}

fn server_editor_component_schemas() -> Vec<game_shared::EditorComponentSchema> {
    server_editor_components()
        .iter()
        .enumerate()
        .map(|(index, registration)| {
            registration.schema(game_shared::EditorStableTypeId((index as u64) + 1))
        })
        .collect()
}

fn start_server_editor_inspector() {
    let execute_server_code_enabled = std::env::var_os("FUN_EDITOR_ENABLE_SERVER_EXEC").is_some();
    let mut config = game_shared::EditorInspectorServiceConfig::local_development(
        game_shared::EditorTargetKind::Server,
        "fun",
        game_shared::local_development_capabilities(
            game_shared::EditorTargetKind::Server,
            execute_server_code_enabled,
        ),
    );
    config.tick_rate_hz = DEFAULT_TICK_RATE_HZ.round() as u32;
    config.component_schemas = server_editor_component_schemas();

    match game_shared::spawn_editor_inspector_service(config) {
        Ok(summary) => {
            if summary.callback_started || summary.bind_addr.is_some() {
                info!(
                    target: "fun::editor::control",
                    callback_started = summary.callback_started,
                    bind_addr = ?summary.bind_addr,
                    "server editor inspector service started"
                );
            }
        }
        Err(error) => {
            tracing::warn!(
                target: "fun::editor::control",
                %error,
                "server editor inspector service did not start"
            );
        }
    }
}

fn catalog_ref(asset_id: u32, material_id: u32, collider_id: u32) -> WorldCatalogRef {
    WorldCatalogRef {
        asset_id,
        material_id,
        collider_id,
    }
}

#[derive(Debug, Default, Resource)]
struct ServerWorldStream {
    revision: WorldRevision,
    signature: Vec<NetEntity>,
    chunks: Vec<WorldStreamChunk>,
}

#[derive(Debug, Default, Resource)]
struct ConnectedClients {
    ids: HashSet<u64>,
}

#[derive(Debug, Default, Resource)]
struct PendingWorldStreams {
    ids: HashSet<u64>,
}

#[cfg_attr(not(all(feature = "diagnostics", debug_assertions)), allow(dead_code))]
#[derive(Debug, Clone, Copy, Resource)]
struct ServerLogConfig {
    stream_verbose: bool,
    net_verbose: bool,
    benchmark_minimal: bool,
}

#[cfg_attr(not(all(feature = "diagnostics", debug_assertions)), allow(dead_code))]
impl ServerLogConfig {
    fn from_env() -> Self {
        Self {
            stream_verbose: std::env::var_os("FUN_LOG_STREAM_VERBOSE").is_some(),
            net_verbose: std::env::var_os("FUN_LOG_NET_VERBOSE").is_some(),
            benchmark_minimal: std::env::var_os("FUN_BENCHMARK_LOG_MINIMAL").is_some(),
        }
    }

    fn stream_verbose(self) -> bool {
        self.stream_verbose && !self.benchmark_minimal
    }

    fn net_verbose(self) -> bool {
        self.net_verbose && !self.benchmark_minimal
    }
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
#[derive(Debug, Default, Resource)]
struct ServerWorldDiagnostics {
    logged_streamable_inventory: bool,
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
#[derive(Debug, Default, Resource)]
struct ServerProfiler {
    server_tick: u64,
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
#[derive(Debug, Resource)]
struct ServerEditorControlPlane {
    config: game_shared::EditorControlConfig,
    granted_capabilities: Vec<game_shared::EditorCapability>,
    diagnostic_streams: Vec<game_shared::EditorDiagnosticStream>,
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
impl Default for ServerEditorControlPlane {
    fn default() -> Self {
        let execute_server_code_enabled =
            std::env::var_os("FUN_EDITOR_ENABLE_SERVER_EXEC").is_some();

        Self {
            config: game_shared::EditorControlConfig::local_development(
                game_shared::EditorTargetKind::Server,
            ),
            granted_capabilities: game_shared::local_development_capabilities(
                game_shared::EditorTargetKind::Server,
                execute_server_code_enabled,
            ),
            diagnostic_streams: game_shared::default_server_editor_diagnostic_subscriptions(),
        }
    }
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
fn log_server_editor_control_plane(control: Res<ServerEditorControlPlane>) {
    let execute_server_code_enabled = control
        .granted_capabilities
        .contains(&game_shared::EditorCapability::ExecuteServerCode);

    game_shared::fun_diag_info!(
        target: "fun::editor::control",
        protocol_version = game_shared::EDITOR_PROTOCOL_VERSION,
        target_kind = ?control.config.target_kind,
        bind_mode = ?control.config.bind_mode,
        bind_addr = control.config.bind_addr.as_str(),
        enabled = control.config.enabled,
        remote_control_permitted = control.config.permits_remote_editor_control(),
        execute_server_code_enabled = execute_server_code_enabled,
        command_apply_stage = game_shared::EDITOR_COMMAND_APPLY_STAGE,
        granted_capabilities = control.granted_capabilities.len(),
        diagnostic_streams = control.diagnostic_streams.len(),
        "server editor control plane ready"
    );
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
fn log_server_editor_schema(schema: Res<ServerEditorSchema>) {
    let critical_resources = schema
        .registry
        .resources
        .iter()
        .filter(|resource| {
            matches!(
                resource.policy,
                game_shared::EditorResourcePolicy::CriticalServerState
            )
        })
        .count();

    game_shared::fun_diag_info!(
        target: "fun::editor::schema",
        components = schema.registry.components.len(),
        resources = schema.registry.resources.len(),
        critical_resources,
        "server editor schema registered"
    );
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
fn begin_server_profiler_tick(mut profiler: ResMut<ServerProfiler>) {
    profiler.server_tick = profiler.server_tick.saturating_add(1);
    game_shared::fun_diag_trace!(
        target: "fun::server::profiler",
        server_tick = profiler.server_tick,
        client_id = tracing::field::Empty,
        packet_type = "server_tick",
        world_revision = tracing::field::Empty,
        chunk_index = tracing::field::Empty,
        chunk_count = tracing::field::Empty,
        bytes = 0usize,
        stage = "fixed_rate_update",
        duration_ns = 0u64,
        "server profiler event"
    );
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
#[derive(Debug, Clone, Copy)]
struct ServerProfilerEvent {
    stage: &'static str,
    packet_type: &'static str,
    client_id: Option<u64>,
    world_revision: Option<u64>,
    chunk_index: Option<u16>,
    chunk_count: Option<u16>,
    bytes: usize,
    duration_ns: u64,
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
fn server_profiler_event(profiler: &ServerProfiler, event: ServerProfilerEvent) {
    game_shared::fun_diag_info!(
        target: "fun::server::profiler",
        server_tick = profiler.server_tick,
        client_id = ?event.client_id,
        packet_type = event.packet_type,
        world_revision = ?event.world_revision,
        chunk_index = ?event.chunk_index,
        chunk_count = ?event.chunk_count,
        bytes = event.bytes,
        stage = event.stage,
        duration_ns = event.duration_ns,
        "server profiler event"
    );
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
fn elapsed_ns(started: Instant) -> u64 {
    started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
fn log_streamable_inventory(
    mut diagnostics: ResMut<ServerWorldDiagnostics>,
    log_config: Res<ServerLogConfig>,
    query: Query<(
        Entity,
        Option<&Name>,
        Option<&NetworkIdentity>,
        Option<&Transform>,
        &StreamedWorldEntity,
    )>,
) {
    if diagnostics.logged_streamable_inventory || query.is_empty() || !log_config.stream_verbose() {
        return;
    }

    diagnostics.logged_streamable_inventory = true;
    game_shared::fun_diag_info!(
        target: "fun::server::stream",
        entities = query.iter().count(),
        "streamable inventory"
    );

    for (entity, name, identity, transform, streamed) in &query {
        game_shared::fun_diag_info!(
            target: "fun::server::stream::entity",
            entity = ?entity,
            name = name.map(|name| name.as_str()).unwrap_or("<unnamed>"),
            identity = identity.map(|identity| identity.entity.0),
            transform = transform
                .map(|transform| format!(
                    "({:.2},{:.2},{:.2})",
                    transform.translation.x, transform.translation.y, transform.translation.z
                ))
                .unwrap_or_else(|| "missing".to_owned()),
            catalog = catalog_summary(streamed.catalog),
            render = render_summary(streamed.render),
            collider = collider_summary(streamed.collider),
            "streamable entity"
        );
    }
}

fn rebuild_world_stream(
    mut manifest: ResMut<ServerWorldStream>,
    mut pending: ResMut<PendingWorldStreams>,
    connected: Res<ConnectedClients>,
    _log_config: Res<ServerLogConfig>,
    #[cfg(all(feature = "diagnostics", debug_assertions))] profiler: Res<ServerProfiler>,
    query: Query<(
        &NetworkIdentity,
        &NetworkAuthority,
        &Transform,
        &StreamedWorldEntity,
        Option<&Name>,
    )>,
) {
    let entity_count = query.iter().count();
    if entity_count == 0 {
        return;
    }

    #[cfg(all(feature = "diagnostics", debug_assertions))]
    let rebuild_started = Instant::now();
    let mut specs = Vec::with_capacity(entity_count);
    for (identity, authority, transform, streamed, name) in &query {
        if !identity.entity.is_valid() {
            return;
        }
        let editor_identity = game_shared::EditorVisibleEntityIdentity::from_network_identity(
            identity.entity,
            identity.class,
            authority.mode,
            name.map(|name| name.as_str()),
            game_shared::EditorEntityMutability::PersistentMutable,
        );

        specs.push(WorldEntitySpec {
            entity: editor_identity.entity,
            name: editor_identity
                .name
                .unwrap_or_else(|| format!("NetEntity-{}", editor_identity.entity.0)),
            class: editor_identity.replication_class,
            authority: editor_identity.authority,
            transform: qtransform(transform),
            catalog: streamed.catalog,
            render: streamed.render,
            collider: streamed.collider,
            color: streamed.color,
        });
    }

    specs.sort_by_key(|spec| spec.entity);
    let signature = specs.iter().map(|spec| spec.entity).collect::<Vec<_>>();
    if signature == manifest.signature {
        return;
    }

    manifest.revision = WorldRevision(manifest.revision.0.saturating_add(1).max(1));
    manifest.signature = signature;
    manifest.chunks = chunk_world_specs(DEMO_LEVEL_ID, manifest.revision, specs);
    pending.ids.clear();
    pending.ids.extend(connected.ids.iter().copied());

    game_shared::fun_diag_info_if!(
        _log_config.stream_verbose(),
        target: "fun::server::stream",
        revision = manifest.revision.0,
        specs = manifest.signature.len(),
        chunks = manifest.chunks.len(),
        pending_clients = pending.ids.len(),
        "built world stream"
    );
    #[cfg(all(feature = "diagnostics", debug_assertions))]
    {
        let duration_ns = elapsed_ns(rebuild_started);
        server_profiler_event(
            &profiler,
            ServerProfilerEvent {
                stage: "world_stream_rebuild",
                packet_type: "world_stream_manifest",
                client_id: None,
                world_revision: Some(manifest.revision.0),
                chunk_index: None,
                chunk_count: Some(manifest.chunks.len().min(u16::MAX as usize) as u16),
                bytes: 0,
                duration_ns,
            },
        );
        for chunk in &manifest.chunks {
            server_profiler_event(
                &profiler,
                ServerProfilerEvent {
                    stage: "world_stream_chunk_ready",
                    packet_type: "world_stream",
                    client_id: None,
                    world_revision: Some(chunk.revision.0),
                    chunk_index: Some(chunk.chunk_index.saturating_add(1)),
                    chunk_count: Some(chunk.chunk_count),
                    bytes: 0,
                    duration_ns: 0,
                },
            );
        }
    }
    game_shared::fun_diag_block_if!(_log_config.stream_verbose(), {
        for chunk in &manifest.chunks {
            game_shared::fun_diag_info!(
                target: "fun::server::stream",
                chunk_number = chunk.chunk_index + 1,
                chunk_count = chunk.chunk_count,
                level = %chunk.level_id.0,
                entities = chunk.entities.len(),
                "world stream chunk"
            );
        }
    });
}

fn queue_world_stream_for_new_clients(
    mut events: MessageReader<ConnectionEvent>,
    mut connected: ResMut<ConnectedClients>,
    mut pending: ResMut<PendingWorldStreams>,
    _log_config: Res<ServerLogConfig>,
) {
    for event in events.read() {
        connected.ids.insert(event.id);
        pending.ids.insert(event.id);
        game_shared::fun_diag_info_if!(
            _log_config.net_verbose(),
            target: "fun::server::net",
            client_id = event.id,
            pending_world_streams = pending.ids.len(),
            "client connected"
        );
    }
}

fn receive_client_control(
    mut server: ResMut<QuinnetServer>,
    _log_config: Res<ServerLogConfig>,
    #[cfg(all(feature = "diagnostics", debug_assertions))] profiler: Res<ServerProfiler>,
) {
    let Some(endpoint) = server.get_endpoint_mut() else {
        return;
    };

    for client_id in endpoint.clients() {
        while let Some(payload) = endpoint.try_receive_payload(client_id, ClientChannel::Control) {
            let _bytes_len = payload.as_ref().len();
            game_shared::fun_diag_info_if!(
                _log_config.net_verbose(),
                target: "fun::server::net",
                client_id,
                bytes = _bytes_len,
                "received control payload"
            );
            #[cfg(all(feature = "diagnostics", debug_assertions))]
            let decode_started = Instant::now();
            match decode_client_packet(payload.as_ref()) {
                Ok(ClientPacket::Hello { hello: _hello }) => {
                    #[cfg(all(feature = "diagnostics", debug_assertions))]
                    server_profiler_event(
                        &profiler,
                        ServerProfilerEvent {
                            stage: "receive_decode",
                            packet_type: "client_hello",
                            client_id: Some(client_id),
                            world_revision: None,
                            chunk_index: None,
                            chunk_count: None,
                            bytes: _bytes_len,
                            duration_ns: elapsed_ns(decode_started),
                        },
                    );
                    game_shared::fun_diag_info_if!(
                        _log_config.net_verbose(),
                        target: "fun::server::net",
                        client_id,
                        "client completed Thunder hello"
                    );
                }
                Ok(ClientPacket::WorldReady { ack: _ack }) => {
                    #[cfg(all(feature = "diagnostics", debug_assertions))]
                    server_profiler_event(
                        &profiler,
                        ServerProfilerEvent {
                            stage: "receive_decode",
                            packet_type: "client_world_ready",
                            client_id: Some(client_id),
                            world_revision: Some(_ack.revision.0),
                            chunk_index: None,
                            chunk_count: None,
                            bytes: _bytes_len,
                            duration_ns: elapsed_ns(decode_started),
                        },
                    );
                    game_shared::fun_diag_info_if!(
                        _log_config.net_verbose(),
                        target: "fun::server::net",
                        client_id,
                        level = %_ack.level_id.0,
                        revision = _ack.revision.0,
                        "client loaded world"
                    );
                }
                Ok(_packet) => {
                    #[cfg(all(feature = "diagnostics", debug_assertions))]
                    server_profiler_event(
                        &profiler,
                        ServerProfilerEvent {
                            stage: "receive_decode",
                            packet_type: "client_control",
                            client_id: Some(client_id),
                            world_revision: None,
                            chunk_index: None,
                            chunk_count: None,
                            bytes: _bytes_len,
                            duration_ns: elapsed_ns(decode_started),
                        },
                    );
                    game_shared::fun_diag_debug_if!(
                        _log_config.net_verbose(),
                        target: "fun::server::net",
                        client_id,
                        packet = ?_packet,
                        "ignoring client control packet"
                    );
                }
                Err(error) => {
                    #[cfg(all(feature = "diagnostics", debug_assertions))]
                    server_profiler_event(
                        &profiler,
                        ServerProfilerEvent {
                            stage: "receive_decode_error",
                            packet_type: "client_control_invalid",
                            client_id: Some(client_id),
                            world_revision: None,
                            chunk_index: None,
                            chunk_count: None,
                            bytes: _bytes_len,
                            duration_ns: elapsed_ns(decode_started),
                        },
                    );
                    error!(target: "fun::server::net", client_id, %error, "failed to decode client control packet");
                }
            }
        }
    }
}

fn send_pending_world_streams(
    mut server: ResMut<QuinnetServer>,
    mut pending: ResMut<PendingWorldStreams>,
    manifest: Res<ServerWorldStream>,
    _log_config: Res<ServerLogConfig>,
    #[cfg(all(feature = "diagnostics", debug_assertions))] profiler: Res<ServerProfiler>,
) {
    if manifest.chunks.is_empty() || pending.ids.is_empty() {
        return;
    }

    let Some(endpoint) = server.get_endpoint_mut() else {
        return;
    };

    let pending_clients = pending.ids.iter().copied().collect::<Vec<_>>();
    for client_id in pending_clients {
        game_shared::fun_diag_info_if!(
            _log_config.stream_verbose(),
            target: "fun::server::stream",
            revision = manifest.revision.0,
            client_id,
            chunks = manifest.chunks.len(),
            "sending world stream"
        );
        let welcome = ServerPacket::Welcome {
            welcome: ServerWelcome {
                client_id: NetClientId(client_id),
                server_tick: NetworkTick::ZERO,
                feature_bits: 0,
                baseline_tick: NetworkTick::ZERO,
            },
        };

        #[cfg(all(feature = "diagnostics", debug_assertions))]
        let welcome_started = Instant::now();
        match encode_server_packet(&welcome) {
            Ok(bytes) => {
                let _byte_len = bytes.len();
                endpoint.try_send_payload_on(client_id, ServerChannel::Control, bytes);
                #[cfg(all(feature = "diagnostics", debug_assertions))]
                server_profiler_event(
                    &profiler,
                    ServerProfilerEvent {
                        stage: "send_world_stream",
                        packet_type: "server_welcome",
                        client_id: Some(client_id),
                        world_revision: Some(manifest.revision.0),
                        chunk_index: None,
                        chunk_count: Some(manifest.chunks.len().min(u16::MAX as usize) as u16),
                        bytes: _byte_len,
                        duration_ns: elapsed_ns(welcome_started),
                    },
                );
                game_shared::fun_diag_info_if!(
                    _log_config.stream_verbose(),
                    target: "fun::server::stream",
                    client_id,
                    bytes = _byte_len,
                    "sent welcome"
                );
            }
            Err(error) => {
                #[cfg(all(feature = "diagnostics", debug_assertions))]
                server_profiler_event(
                    &profiler,
                    ServerProfilerEvent {
                        stage: "send_world_stream_error",
                        packet_type: "server_welcome",
                        client_id: Some(client_id),
                        world_revision: Some(manifest.revision.0),
                        chunk_index: None,
                        chunk_count: Some(manifest.chunks.len().min(u16::MAX as usize) as u16),
                        bytes: 0,
                        duration_ns: elapsed_ns(welcome_started),
                    },
                );
                error!(target: "fun::server::stream", client_id, %error, "failed to encode welcome");
                continue;
            }
        }

        for chunk in &manifest.chunks {
            let packet = ServerPacket::WorldStream {
                chunk: chunk.clone(),
            };
            #[cfg(all(feature = "diagnostics", debug_assertions))]
            let chunk_started = Instant::now();
            match encode_server_packet(&packet) {
                Ok(bytes) => {
                    let _byte_len = bytes.len();
                    endpoint.try_send_payload_on(client_id, ServerChannel::Stream, bytes);
                    #[cfg(all(feature = "diagnostics", debug_assertions))]
                    server_profiler_event(
                        &profiler,
                        ServerProfilerEvent {
                            stage: "send_world_stream",
                            packet_type: "world_stream",
                            client_id: Some(client_id),
                            world_revision: Some(chunk.revision.0),
                            chunk_index: Some(chunk.chunk_index.saturating_add(1)),
                            chunk_count: Some(chunk.chunk_count),
                            bytes: _byte_len,
                            duration_ns: elapsed_ns(chunk_started),
                        },
                    );
                    game_shared::fun_diag_info_if!(
                        _log_config.stream_verbose(),
                        target: "fun::server::stream",
                        chunk_number = chunk.chunk_index + 1,
                        chunk_count = chunk.chunk_count,
                        client_id,
                        entities = chunk.entities.len(),
                        bytes = _byte_len,
                        "sent stream chunk"
                    );
                }
                Err(error) => {
                    #[cfg(all(feature = "diagnostics", debug_assertions))]
                    server_profiler_event(
                        &profiler,
                        ServerProfilerEvent {
                            stage: "send_world_stream_error",
                            packet_type: "world_stream",
                            client_id: Some(client_id),
                            world_revision: Some(chunk.revision.0),
                            chunk_index: Some(chunk.chunk_index.saturating_add(1)),
                            chunk_count: Some(chunk.chunk_count),
                            bytes: 0,
                            duration_ns: elapsed_ns(chunk_started),
                        },
                    );
                    error!(target: "fun::server::stream", client_id, %error, "failed to encode world stream");
                    continue;
                }
            }
        }

        pending.ids.remove(&client_id);
        game_shared::fun_diag_info_if!(
            _log_config.stream_verbose(),
            target: "fun::server::stream",
            revision = manifest.revision.0,
            client_id,
            "completed world stream"
        );
    }
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
fn catalog_summary(catalog: Option<WorldCatalogRef>) -> String {
    catalog
        .map(|catalog| {
            format!(
                "asset={} material={} collider={}",
                catalog.asset_id, catalog.material_id, catalog.collider_id
            )
        })
        .unwrap_or_else(|| "none".to_owned())
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
fn render_summary(render: Option<WorldPrimitive>) -> String {
    match render {
        Some(WorldPrimitive::Plane { size }) => {
            let size = size.to_f32(Quantization::MILLIMETERS);
            format!("plane({:.2},{:.2},{:.2})", size[0], size[1], size[2])
        }
        Some(WorldPrimitive::Cuboid { size }) => {
            let size = size.to_f32(Quantization::MILLIMETERS);
            format!("cuboid({:.2},{:.2},{:.2})", size[0], size[1], size[2])
        }
        None => "none".to_owned(),
    }
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
fn collider_summary(collider: Option<WorldCollider>) -> String {
    match collider {
        Some(WorldCollider::Cuboid { size }) => {
            let size = size.to_f32(Quantization::MILLIMETERS);
            format!("cuboid({:.2},{:.2},{:.2})", size[0], size[1], size[2])
        }
        None => "none".to_owned(),
    }
}
fn chunk_world_specs(
    level_id: &str,
    revision: WorldRevision,
    specs: Vec<WorldEntitySpec>,
) -> Vec<WorldStreamChunk> {
    let chunk_count = specs
        .len()
        .div_ceil(WORLD_STREAM_ENTITIES_PER_CHUNK)
        .max(1)
        .min(u16::MAX as usize) as u16;

    specs
        .chunks(WORLD_STREAM_ENTITIES_PER_CHUNK)
        .enumerate()
        .map(|(chunk_index, entities)| WorldStreamChunk {
            level_id: WorldLevelId(level_id.to_owned()),
            revision,
            chunk_index: chunk_index as u16,
            chunk_count,
            entities: entities.to_vec(),
        })
        .collect()
}

fn qvec(value: Vec3) -> QuantizedVec3 {
    QuantizedVec3::from_f32(value.to_array(), Quantization::MILLIMETERS)
}

fn qtransform(transform: &Transform) -> QuantizedTransform3 {
    QuantizedTransform3 {
        translation: qvec(transform.translation),
        rotation: QuantizedQuat::from_f32([
            transform.rotation.x,
            transform.rotation.y,
            transform.rotation.z,
            transform.rotation.w,
        ]),
    }
}
