#[cfg(all(feature = "diagnostics", debug_assertions))]
use std::time::Instant;
use std::{
    collections::{HashMap, HashSet},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use avian3d::prelude::PhysicsPlugins;
use bevy::{
    app::ScheduleRunnerPlugin, asset::AssetPlugin, log::LogPlugin, mesh::MeshPlugin, prelude::*,
    scene::ScenePlugin,
};
use bevy_quinnet::server::{
    ConnectionEvent, EndpointAddrConfiguration, QuinnetServer, QuinnetServerPlugin,
    ServerEndpointConfiguration, ServerEndpointConfigurationDefaultables,
    certificate::CertificateRetrievalMode,
};
use game_scene::StreamedWorldEntity;
use game_shared::{DEFAULT_TICK_RATE_HZ, DEMO_LEVEL_ID, GAME_SERVER_BIND_ADDR, GAME_TITLE};
use thunder::prelude::*;
use tracing::{error, info};

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
    .init_resource::<ServerEditorInspectorState>()
    .insert_resource(ServerLogConfig::from_env())
    .init_resource::<ConnectedClients>()
    .init_resource::<ReadyClients>()
    .init_resource::<ClientRelevanceSets>()
    .init_resource::<PendingWorldStreams>()
    .add_systems(
        Startup,
        (
            start_endpoint,
            start_server_editor_inspector,
            game_scene::spawn_default_scene,
            game_scene::apply_scene_stable_identities,
        )
            .chain(),
    )
    .add_systems(
        Update,
        (
            receive_client_control,
            apply_server_editor_mutations,
            rebuild_world_stream,
            update_server_editor_inspector_snapshot,
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

#[derive(Resource)]
struct ServerEditorSchema {
    #[cfg_attr(not(all(feature = "diagnostics", debug_assertions)), allow(dead_code))]
    registry: game_shared::EditorSchemaRegistry<'static>,
}

#[derive(Debug, Clone, Resource, Default)]
struct ServerEditorInspectorState {
    state: game_shared::EditorInspectorRuntimeState,
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
            reflect: "game_scene::StreamedWorldEntity",
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

fn start_server_editor_inspector(inspector: Res<ServerEditorInspectorState>) {
    let execute_server_code_enabled = std::env::var_os("FUN_EDITOR_ENABLE_SERVER_EXEC").is_some();
    let component_schemas = server_editor_component_schemas();
    inspector.state.replace_entities(
        game_shared::EditorWorldRevision(0),
        game_shared::EditorSchemaRevision(1),
        component_schemas.clone(),
        Vec::new(),
    );
    let mut config = game_shared::EditorInspectorServiceConfig::local_development(
        game_shared::EditorTargetKind::Server,
        "fun",
        game_shared::local_development_capabilities(
            game_shared::EditorTargetKind::Server,
            execute_server_code_enabled,
        ),
    );
    config.tick_rate_hz = DEFAULT_TICK_RATE_HZ.round() as u32;
    config.component_schemas = component_schemas;
    config.runtime_state = inspector.state.clone();

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

#[derive(Debug, Default, Resource)]
struct ServerWorldStream {
    revision: WorldRevision,
    editor_revision: WorldRevision,
    signature: Vec<NetEntity>,
    chunks: Vec<WorldStreamChunk>,
}

#[derive(Debug, Default, Resource)]
struct ConnectedClients {
    ids: HashSet<u64>,
}

#[derive(Debug, Default, Resource)]
struct ReadyClients {
    ids: HashSet<u64>,
}

#[derive(Debug, Default, Resource)]
struct ClientRelevanceSets {
    sets: HashMap<u64, ServerClientRelevanceSet>,
}

#[derive(Debug, Default, Clone)]
struct ServerClientRelevanceSet {
    tick: NetworkTick,
    entities: HashSet<NetEntity>,
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
#[allow(
    clippy::type_complexity,
    reason = "Bevy query tuple keeps streamable inventory diagnostics aligned with the exact ECS read set"
)]
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
            transform: game_scene::qtransform(transform),
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
    manifest.editor_revision = manifest.revision;
    manifest.signature = signature;
    manifest.chunks = game_scene::chunk_world_specs(DEMO_LEVEL_ID, manifest.revision, specs);
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
    mut ready: ResMut<ReadyClients>,
    mut relevance: ResMut<ClientRelevanceSets>,
    mut pending: ResMut<PendingWorldStreams>,
    _log_config: Res<ServerLogConfig>,
) {
    for event in events.read() {
        connected.ids.insert(event.id);
        ready.ids.remove(&event.id);
        relevance.sets.remove(&event.id);
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
    mut ready: ResMut<ReadyClients>,
    mut relevance: ResMut<ClientRelevanceSets>,
    manifest: Res<ServerWorldStream>,
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
                    ready.ids.insert(client_id);
                    relevance.sets.insert(
                        client_id,
                        ServerClientRelevanceSet {
                            tick: NetworkTick(manifest.editor_revision.0),
                            entities: manifest.signature.iter().copied().collect(),
                        },
                    );
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

#[allow(
    clippy::too_many_arguments,
    reason = "editor mutation application is a Bevy system with explicit resource/query access at one deterministic stage"
)]
fn apply_server_editor_mutations(
    inspector: Res<ServerEditorInspectorState>,
    schema: Res<ServerEditorSchema>,
    mut manifest: ResMut<ServerWorldStream>,
    mut server: ResMut<QuinnetServer>,
    ready: Res<ReadyClients>,
    relevance: Res<ClientRelevanceSets>,
    identities: Query<(&NetworkIdentity, &NetworkAuthority, Option<&Name>)>,
    mut transforms: Query<(
        &NetworkIdentity,
        &NetworkAuthority,
        Option<&Name>,
        &mut Transform,
    )>,
) {
    let transactions = inspector.state.drain_mutations(32);
    if transactions.is_empty() {
        return;
    }

    let known_entities = identities
        .iter()
        .map(|(identity, authority, name)| {
            game_shared::EditorVisibleEntityIdentity::from_network_identity(
                identity.entity,
                identity.class,
                authority.mode,
                name.map(|name| name.as_str()),
                game_shared::EditorEntityMutability::PersistentMutable,
            )
        })
        .collect::<Vec<_>>();
    let granted_capabilities =
        game_shared::local_development_capabilities(game_shared::EditorTargetKind::Server, false);

    for transaction in transactions {
        let current_revision = game_shared::EditorWorldRevision(manifest.editor_revision.0);
        let transaction_id = transaction.transaction_id;
        let (status, applied_ops, conflict_count, mut diagnostics, editor_delta, snapshot_delta) =
            apply_server_transform_transaction(
                &transaction,
                current_revision,
                &schema,
                &known_entities,
                &granted_capabilities,
                &mut transforms,
            );

        let world_revision = if status == game_shared::EditorMutationStatus::Accepted {
            manifest.editor_revision =
                WorldRevision(manifest.editor_revision.0.saturating_add(1).max(1));
            game_shared::EditorWorldRevision(manifest.editor_revision.0)
        } else {
            current_revision
        };

        let mut events = Vec::new();
        let ack = game_shared::EditorMutationAck {
            header: editor_response_header(&transaction.header, world_revision),
            transaction_id,
            status,
            world_revision,
            applied_ops,
            conflict_count,
            diagnostics: diagnostics.clone(),
        };
        events.push(game_shared::EditorEventPayload::MutationAck { ack });

        if let Some(mut delta) = editor_delta {
            delta.world_revision = world_revision;
            events.push(game_shared::EditorEventPayload::EntityDelta { delta });
        }

        let relevant_clients = snapshot_delta
            .as_ref()
            .map(|delta| {
                send_editor_snapshot_delta(&mut server, &ready, &relevance, world_revision, delta)
            })
            .unwrap_or(0);

        diagnostics.push(editor_mutation_diagnostic(
            game_shared::DiagnosticLevel::Info,
            "mutation_relevant_clients",
            transaction_id,
            world_revision,
            format!("relevant_clients={relevant_clients}"),
        ));

        for diagnostic in diagnostics {
            inspector
                .state
                .emit_diagnostic(game_shared::DiagnosticPacket::Event { event: diagnostic });
        }
        inspector
            .state
            .emit_diagnostic(game_shared::DiagnosticPacket::Counter {
                counter: game_shared::DiagnosticCounter {
                    target: "server".to_owned(),
                    name: "editor_mutation_applied_ops".to_owned(),
                    value: game_shared::DiagnosticValue::U64 {
                        value: u64::from(applied_ops),
                    },
                    unit: game_shared::DiagnosticUnit::Count,
                    window: game_shared::DiagnosticWindow {
                        sample_count: 1,
                        duration_ns: 0,
                    },
                },
            });

        inspector
            .state
            .complete_mutation(transaction_id, world_revision, events);
    }
}

fn apply_server_transform_transaction(
    transaction: &game_shared::EditorMutationTransaction,
    current_revision: game_shared::EditorWorldRevision,
    schema: &ServerEditorSchema,
    known_entities: &[game_shared::EditorVisibleEntityIdentity],
    granted_capabilities: &[game_shared::EditorCapability],
    transforms: &mut Query<(
        &NetworkIdentity,
        &NetworkAuthority,
        Option<&Name>,
        &mut Transform,
    )>,
) -> (
    game_shared::EditorMutationStatus,
    u32,
    u32,
    Vec<game_shared::EditorDiagnosticEvent>,
    Option<game_shared::EditorEntityDelta>,
    Option<EntityDelta>,
) {
    let transaction_id = transaction.transaction_id;
    if transaction.ops.len() != 1 {
        return rejected_mutation(
            transaction_id,
            current_revision,
            "mutation_invalid_op_count",
            "server Transform patch accepts exactly one operation",
        );
    }

    let game_shared::EditorMutationOp::PatchEntity { patch } = &transaction.ops[0] else {
        return rejected_mutation(
            transaction_id,
            current_revision,
            "mutation_unsupported_op",
            "server Transform patch only supports PatchEntity",
        );
    };

    if patch.component != game_shared::EDITOR_COMPONENT_KIND_TRANSFORM {
        return rejected_mutation(
            transaction_id,
            current_revision,
            "mutation_unsupported_component",
            "first mutation path only accepts Transform",
        );
    }

    if let Err(error) = game_shared::validate_component_mutation(
        schema.registry,
        game_shared::EditorComponentMutationRequest {
            entity: patch.entity,
            component_kind: patch.component,
            payload: &patch.payload,
            granted_capabilities,
            base_world_revision: transaction.base_world_revision,
            current_world_revision: current_revision,
            known_entities,
            execution_budget: game_shared::EditorExecutionBudget {
                max_ops: 1,
                used_ops: 0,
            },
        },
    ) {
        return rejected_mutation(
            transaction_id,
            current_revision,
            "mutation_schema_validation_failed",
            format!("schema validation failed: {error:?}"),
        );
    }

    let Some(transform_patch) = game_shared::decode_editor_transform_patch(&patch.payload) else {
        return rejected_mutation(
            transaction_id,
            current_revision,
            "mutation_payload_decode_failed",
            "Transform patch payload did not decode",
        );
    };

    let Some((identity, authority, name, mut transform)) = transforms
        .iter_mut()
        .find(|(identity, _, _, _)| identity.entity == patch.entity)
    else {
        return rejected_mutation(
            transaction_id,
            current_revision,
            "mutation_transform_missing",
            "entity exists but does not expose a mutable Transform component",
        );
    };

    let translation = transform_patch.translation();
    let rotation = transform_patch.rotation_xyzw();
    let scale = transform_patch.scale();
    transform.translation = Vec3::from_array(translation);
    let rotation_quat = Quat::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3]);
    let rotation_len = rotation_quat.length_squared();
    transform.rotation = if rotation_len.is_finite() && rotation_len > 0.0 {
        rotation_quat.normalize()
    } else {
        Quat::IDENTITY
    };
    transform.scale = Vec3::from_array(scale);

    let diagnostic = editor_mutation_diagnostic(
        game_shared::DiagnosticLevel::Info,
        "mutation_transform_applied",
        transaction_id,
        current_revision,
        format!(
            "entity={} name={} translation=({:.3},{:.3},{:.3})",
            patch.entity.0,
            name.map(|name| name.as_str()).unwrap_or("<unnamed>"),
            translation[0],
            translation[1],
            translation[2]
        ),
    );
    let editor_delta = game_shared::EditorEntityDelta {
        world_revision: current_revision,
        entity: patch.entity,
        components: vec![game_shared::EditorComponentPayload {
            component: patch.component,
            payload: patch.payload.clone(),
        }],
    };
    let snapshot_delta = EntityDelta {
        entity: patch.entity,
        class: identity.class,
        authority: authority.mode,
        transform: Some(game_scene::qtransform(&transform)),
        body: None,
        components: Vec::new(),
    };

    (
        game_shared::EditorMutationStatus::Accepted,
        1,
        0,
        vec![diagnostic],
        Some(editor_delta),
        Some(snapshot_delta),
    )
}

fn rejected_mutation(
    transaction_id: game_shared::EditorTransactionId,
    world_revision: game_shared::EditorWorldRevision,
    code: &'static str,
    message: impl Into<String>,
) -> (
    game_shared::EditorMutationStatus,
    u32,
    u32,
    Vec<game_shared::EditorDiagnosticEvent>,
    Option<game_shared::EditorEntityDelta>,
    Option<EntityDelta>,
) {
    (
        game_shared::EditorMutationStatus::RejectedValidation,
        0,
        1,
        vec![editor_mutation_diagnostic(
            game_shared::DiagnosticLevel::Warn,
            code,
            transaction_id,
            world_revision,
            message,
        )],
        None,
        None,
    )
}

fn send_editor_snapshot_delta(
    server: &mut QuinnetServer,
    ready: &ReadyClients,
    relevance: &ClientRelevanceSets,
    world_revision: game_shared::EditorWorldRevision,
    delta: &EntityDelta,
) -> usize {
    if ready.ids.is_empty() {
        return 0;
    }
    let packet = ServerPacket::Snapshot {
        snapshot: SnapshotPacket {
            sequence: PacketSequence((world_revision.0 & u64::from(u32::MAX)) as u32),
            server_tick: NetworkTick(world_revision.0),
            baseline_tick: None,
            last_processed_input: PacketSequence(0),
            budget: SnapshotBudget {
                max_bytes: 1_100,
                max_entities: 1,
                max_component_deltas: 0,
            },
            entities: vec![delta.clone()],
        },
    };
    let Ok(bytes) = encode_server_packet(&packet) else {
        return 0;
    };
    let Some(endpoint) = server.get_endpoint_mut() else {
        return 0;
    };

    let mut sent = 0usize;
    for client_id in &ready.ids {
        let relevant = relevance
            .sets
            .get(client_id)
            .map(|set| set.tick.0 <= world_revision.0 && set.entities.contains(&delta.entity))
            .unwrap_or(false);
        if !relevant {
            continue;
        }
        endpoint.try_send_payload_on(*client_id, ServerChannel::Snapshot, bytes.clone());
        sent = sent.saturating_add(1);
    }
    sent
}

fn editor_response_header(
    request: &game_shared::EditorPacketHeader,
    world_revision: game_shared::EditorWorldRevision,
) -> game_shared::EditorPacketHeader {
    game_shared::EditorPacketHeader::new(
        request.request_id,
        request.target,
        PacketSequence(request.sequence.0.saturating_add(1)),
        Some(world_revision),
        request.size_budget,
    )
}

fn editor_mutation_diagnostic(
    level: game_shared::DiagnosticLevel,
    code: &'static str,
    transaction_id: game_shared::EditorTransactionId,
    world_revision: game_shared::EditorWorldRevision,
    message: impl Into<String>,
) -> game_shared::EditorDiagnosticEvent {
    game_shared::DiagnosticEvent {
        target: "server".to_owned(),
        level,
        timestamp_or_tick: game_shared::DiagnosticTimestampOrTick::TimestampNs { ns: unix_ns() },
        fields: vec![
            game_shared::DiagnosticField::text("stream", "mutation_transactions"),
            game_shared::DiagnosticField::text("stage", game_shared::EDITOR_COMMAND_APPLY_STAGE),
            game_shared::DiagnosticField::text("code", code),
            game_shared::DiagnosticField::u64("transaction_id", transaction_id.0),
            game_shared::DiagnosticField::u64("world_revision", world_revision.0),
            game_shared::DiagnosticField::text("message", message),
        ],
        source: game_shared::DiagnosticSource::static_location(file!(), line!(), module_path!()),
        frame_index: None,
        span_id: None,
    }
}

#[allow(
    clippy::type_complexity,
    reason = "Bevy query tuple documents the authoritative server components exposed to the editor snapshot"
)]
fn update_server_editor_inspector_snapshot(
    inspector: Res<ServerEditorInspectorState>,
    manifest: Res<ServerWorldStream>,
    query: Query<(
        &NetworkIdentity,
        &NetworkAuthority,
        Option<&Name>,
        Option<&Transform>,
        Option<&StreamedWorldEntity>,
    )>,
    mut last_diagnostic_revision: Local<u64>,
) {
    let schema_revision = game_shared::EditorSchemaRevision(1);
    let world_revision = game_shared::EditorWorldRevision(manifest.editor_revision.0);
    let schemas = server_editor_component_schemas();
    let mut rows = Vec::with_capacity(query.iter().count());

    for (identity, authority, name, transform, streamed) in &query {
        if !identity.entity.is_valid() {
            continue;
        }

        let mut components = Vec::with_capacity(5);
        if let Some(transform) = transform {
            components.push(editor_component_value(
                game_shared::EDITOR_COMPONENT_KIND_TRANSFORM,
                schema_revision,
                transform_preview(transform),
            ));
        }
        if let Some(name) = name {
            components.push(editor_component_value(
                game_shared::EDITOR_COMPONENT_KIND_NAME,
                schema_revision,
                name.as_str().to_owned(),
            ));
        }
        components.push(editor_component_value(
            game_shared::EDITOR_COMPONENT_KIND_NETWORK_IDENTITY,
            schema_revision,
            format!(
                "net={} shard={} local={} class={:?}",
                identity.entity.0,
                identity.entity.shard(),
                identity.entity.local(),
                identity.class
            ),
        ));
        components.push(editor_component_value(
            game_shared::EDITOR_COMPONENT_KIND_NETWORK_AUTHORITY,
            schema_revision,
            format!("{:?}", authority.mode),
        ));
        if let Some(streamed) = streamed {
            components.push(editor_component_value(
                game_shared::EDITOR_COMPONENT_KIND_WORLD_CATALOG_REF,
                schema_revision,
                streamed_world_preview(streamed),
            ));
        }

        rows.push(game_shared::EditorEntityRow {
            entity: identity.entity,
            target: game_shared::EditorTargetKind::Server,
            display_label: name
                .map(|name| name.as_str().to_owned())
                .unwrap_or_else(|| format!("NetEntity-{}", identity.entity.0)),
            component_count: components.len().min(u16::MAX as usize) as u16,
            schema_revision,
            components,
        });
    }

    inspector
        .state
        .replace_entities(world_revision, schema_revision, schemas, rows);

    if inspector.state.editor_attached() && *last_diagnostic_revision != world_revision.0 {
        *last_diagnostic_revision = world_revision.0;
        inspector
            .state
            .emit_diagnostic(game_shared::DiagnosticPacket::Counter {
                counter: game_shared::DiagnosticCounter {
                    target: "server".to_owned(),
                    name: "world_revision".to_owned(),
                    value: game_shared::DiagnosticValue::U64 {
                        value: world_revision.0,
                    },
                    unit: game_shared::DiagnosticUnit::Count,
                    window: game_shared::DiagnosticWindow {
                        sample_count: 1,
                        duration_ns: 0,
                    },
                },
            });
    }
}

fn editor_component_value(
    component_kind: ComponentKind,
    schema_revision: game_shared::EditorSchemaRevision,
    value_preview: String,
) -> game_shared::EditorEntityComponentValue {
    game_shared::EditorEntityComponentValue {
        component_kind,
        schema_revision,
        value_preview,
        raw_payload: Vec::new(),
    }
}

fn transform_preview(transform: &Transform) -> String {
    format!(
        "translation=({:.2},{:.2},{:.2}) rotation=({:.3},{:.3},{:.3},{:.3}) scale=({:.2},{:.2},{:.2})",
        transform.translation.x,
        transform.translation.y,
        transform.translation.z,
        transform.rotation.x,
        transform.rotation.y,
        transform.rotation.z,
        transform.rotation.w,
        transform.scale.x,
        transform.scale.y,
        transform.scale.z
    )
}

fn streamed_world_preview(streamed: &StreamedWorldEntity) -> String {
    format!(
        "catalog={} render={:?} collider={:?} color={:?}",
        streamed
            .catalog
            .map(|catalog| format!(
                "asset={} material={} collider={}",
                catalog.asset_id, catalog.material_id, catalog.collider_id
            ))
            .unwrap_or_else(|| "none".to_owned()),
        streamed.render,
        streamed.collider,
        streamed.color
    )
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
fn unix_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos().min(u128::from(u64::MAX)) as u64)
        .unwrap_or_default()
}
