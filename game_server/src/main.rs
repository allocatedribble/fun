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
use game_shared::{DEFAULT_TICK_RATE_HZ, DEMO_LEVEL_ID, GAME_SERVER_BIND_ADDR, GAME_TITLE};
use thunder::prelude::*;
use tracing::info;

const WORLD_STREAM_ENTITIES_PER_CHUNK: usize = 16;

fn main() {
    App::new()
        .add_plugins((
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
        .init_resource::<ConnectedClients>()
        .init_resource::<PendingWorldStreams>()
        .init_resource::<ServerWorldDiagnostics>()
        .add_systems(Startup, (start_endpoint, spawn_demo_world).chain())
        .add_systems(
            Update,
            (
                receive_client_control,
                log_streamable_inventory,
                rebuild_world_stream,
                queue_world_stream_for_new_clients,
                send_pending_world_streams,
            )
                .chain(),
        )
        .run();
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
            template_value(StreamedWorldEntity::plane(
                Vec3::new(60.0, 0.0, 60.0),
                PackedColorRgba8::srgb(41, 48, 41),
            ))
            Transform::default()
        ),
        (
            #FloorCollider
            Name::new("FloorCollider")
            template_value(Networked::world())
            template_value(StreamedWorldEntity::collider(Vec3::new(60.0, 0.5, 60.0)))
            template_value(RigidBody::Static)
            Collider::cuboid(60.0, 0.5, 60.0)
            Transform::from_xyz(0.0, -0.25, 0.0)
        ),
        (
            #Wall
            template_value(Networked::world())
            template_value(StreamedWorldEntity::cuboid(
                Vec3::new(5.0, 3.0, 1.0),
                PackedColorRgba8::srgb(82, 89, 107),
            ))
            template_value(RigidBody::Static)
            Collider::cuboid(5.0, 3.0, 1.0)
            Transform::from_xyz(0.0, 1.5, -8.0)
        ),
        (
            #Ramp
            template_value(Networked::world())
            template_value(StreamedWorldEntity::cuboid(
                Vec3::new(3.0, 0.5, 6.0),
                PackedColorRgba8::srgb(97, 71, 56),
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
        template_value(StreamedWorldEntity::cuboid(
            Vec3::new(1.0, 1.0, 1.0),
            PackedColorRgba8::srgb(204, 102, 77),
        ))
        template_value(RigidBody::Static)
        Collider::cuboid(1.0, 1.0, 1.0)
        template_value(Transform::from_translation(translation))
    }
}

#[derive(Debug, Default, Clone, Component)]
struct StreamedWorldEntity {
    render: Option<WorldPrimitive>,
    collider: Option<WorldCollider>,
    color: Option<PackedColorRgba8>,
}

impl StreamedWorldEntity {
    fn plane(size: Vec3, color: PackedColorRgba8) -> Self {
        Self {
            render: Some(WorldPrimitive::Plane { size: qvec(size) }),
            collider: None,
            color: Some(color),
        }
    }

    fn cuboid(size: Vec3, color: PackedColorRgba8) -> Self {
        Self {
            render: Some(WorldPrimitive::Cuboid { size: qvec(size) }),
            collider: Some(WorldCollider::Cuboid { size: qvec(size) }),
            color: Some(color),
        }
    }

    fn collider(size: Vec3) -> Self {
        Self {
            render: None,
            collider: Some(WorldCollider::Cuboid { size: qvec(size) }),
            color: None,
        }
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

#[derive(Debug, Default, Resource)]
struct ServerWorldDiagnostics {
    logged_streamable_inventory: bool,
}

fn log_streamable_inventory(
    mut diagnostics: ResMut<ServerWorldDiagnostics>,
    query: Query<(
        Entity,
        Option<&Name>,
        Option<&NetworkIdentity>,
        Option<&Transform>,
        &StreamedWorldEntity,
    )>,
) {
    if diagnostics.logged_streamable_inventory || query.is_empty() {
        return;
    }

    diagnostics.logged_streamable_inventory = true;
    info!(
        "[server diag] streamable inventory entities={}",
        query.iter().count()
    );

    for (entity, name, identity, transform, streamed) in &query {
        info!(
            "[server diag] streamable {:?}/{} identity={} transform={} render={} collider={}",
            entity,
            name.map(|name| name.as_str()).unwrap_or("<unnamed>"),
            identity
                .map(|identity| identity.entity.0.to_string())
                .unwrap_or_else(|| "missing".to_owned()),
            transform
                .map(|transform| format!(
                    "({:.2},{:.2},{:.2})",
                    transform.translation.x, transform.translation.y, transform.translation.z
                ))
                .unwrap_or_else(|| "missing".to_owned()),
            render_summary(streamed.render),
            collider_summary(streamed.collider),
        );
    }
}

fn rebuild_world_stream(
    mut manifest: ResMut<ServerWorldStream>,
    mut pending: ResMut<PendingWorldStreams>,
    connected: Res<ConnectedClients>,
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

    let mut specs = Vec::with_capacity(entity_count);
    for (identity, authority, transform, streamed, name) in &query {
        if !identity.entity.is_valid() {
            return;
        }

        specs.push(WorldEntitySpec {
            entity: identity.entity,
            name: name
                .map(|name| name.as_str().to_owned())
                .unwrap_or_else(|| format!("NetEntity-{}", identity.entity.0)),
            class: identity.class,
            authority: authority.mode,
            transform: qtransform(transform),
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

    info!(
        "Built world stream revision {} with {} chunks",
        manifest.revision.0,
        manifest.chunks.len()
    );
    info!(
        "[server stream] built world stream revision {} specs={} chunks={} pending_clients={}",
        manifest.revision.0,
        manifest.signature.len(),
        manifest.chunks.len(),
        pending.ids.len()
    );
    for chunk in &manifest.chunks {
        info!(
            "[server stream] chunk {}/{} level={} entities={}",
            chunk.chunk_index + 1,
            chunk.chunk_count,
            chunk.level_id.0,
            chunk.entities.len()
        );
        for spec in &chunk.entities {
            let translation = spec.transform.translation.to_f32(Quantization::MILLIMETERS);
            info!(
                "[server stream] entity net={} name={} class={:?} authority={:?} pos=({:.2},{:.2},{:.2}) render={} collider={}",
                spec.entity.0,
                spec.name,
                spec.class,
                spec.authority,
                translation[0],
                translation[1],
                translation[2],
                render_summary(spec.render),
                collider_summary(spec.collider),
            );
        }
    }
}

fn queue_world_stream_for_new_clients(
    mut events: MessageReader<ConnectionEvent>,
    mut connected: ResMut<ConnectedClients>,
    mut pending: ResMut<PendingWorldStreams>,
) {
    for event in events.read() {
        connected.ids.insert(event.id);
        pending.ids.insert(event.id);
        info!(
            "[server net] client connected id={} pending_world_streams={}",
            event.id,
            pending.ids.len()
        );
        info!("Queued world stream for client {}", event.id);
    }
}

fn receive_client_control(mut server: ResMut<QuinnetServer>) {
    let Some(endpoint) = server.get_endpoint_mut() else {
        return;
    };

    for client_id in endpoint.clients() {
        while let Some(payload) = endpoint.try_receive_payload(client_id, ClientChannel::Control) {
            info!(
                "[server net] received control payload from client {} ({} bytes)",
                client_id,
                payload.as_ref().len()
            );
            match decode_client_packet(payload.as_ref()) {
                Ok(ClientPacket::Hello { hello: _hello }) => {
                    info!("[server net] client {client_id} completed Thunder hello");
                    info!("Client {client_id} completed Thunder hello");
                }
                Ok(ClientPacket::WorldReady { ack }) => {
                    info!(
                        "[server net] client {client_id} loaded world {} revision {}",
                        ack.level_id.0, ack.revision.0
                    );
                    info!(
                        "Client {client_id} loaded world {} revision {}",
                        ack.level_id.0, ack.revision.0
                    );
                }
                Ok(packet) => {
                    info!(
                        "[server net] ignoring client control packet from {client_id}: {packet:?}"
                    );
                    debug!("Ignoring client control packet from {client_id}: {packet:?}");
                }
                Err(error) => {
                    info!(
                        "[server net] failed to decode client control packet from {client_id}: {error}"
                    );
                    error!("Failed to decode client control packet: {error}");
                }
            }
        }
    }
}

fn send_pending_world_streams(
    mut server: ResMut<QuinnetServer>,
    mut pending: ResMut<PendingWorldStreams>,
    manifest: Res<ServerWorldStream>,
) {
    if manifest.chunks.is_empty() || pending.ids.is_empty() {
        return;
    }

    let Some(endpoint) = server.get_endpoint_mut() else {
        return;
    };

    let pending_clients = pending.ids.iter().copied().collect::<Vec<_>>();
    for client_id in pending_clients {
        info!(
            "[server stream] sending revision {} to client {} as {} chunks",
            manifest.revision.0,
            client_id,
            manifest.chunks.len()
        );
        let welcome = ServerPacket::Welcome {
            welcome: ServerWelcome {
                client_id: NetClientId(client_id),
                server_tick: NetworkTick::ZERO,
                feature_bits: 0,
                baseline_tick: NetworkTick::ZERO,
            },
        };

        match encode_server_packet(&welcome) {
            Ok(bytes) => {
                let byte_len = bytes.len();
                endpoint.try_send_payload_on(client_id, ServerChannel::Control, bytes);
                info!(
                    "[server stream] sent welcome to client {} ({} bytes)",
                    client_id, byte_len
                );
            }
            Err(error) => {
                info!("[server stream] failed to encode welcome for client {client_id}: {error}");
                error!("Failed to encode welcome for client {client_id}: {error}");
                continue;
            }
        }

        for chunk in &manifest.chunks {
            let packet = ServerPacket::WorldStream {
                chunk: chunk.clone(),
            };
            match encode_server_packet(&packet) {
                Ok(bytes) => {
                    let byte_len = bytes.len();
                    endpoint.try_send_payload_on(client_id, ServerChannel::Stream, bytes);
                    info!(
                        "[server stream] sent chunk {}/{} to client {} (entities={} bytes={})",
                        chunk.chunk_index + 1,
                        chunk.chunk_count,
                        client_id,
                        chunk.entities.len(),
                        byte_len
                    );
                }
                Err(error) => {
                    info!(
                        "[server stream] failed to encode world stream for client {client_id}: {error}"
                    );
                    error!("Failed to encode world stream for client {client_id}: {error}");
                    continue;
                }
            }
        }

        pending.ids.remove(&client_id);
        info!(
            "Sent world stream revision {} to client {}",
            manifest.revision.0, client_id
        );
        info!(
            "[server stream] completed world stream revision {} to client {}",
            manifest.revision.0, client_id
        );
    }
}

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
