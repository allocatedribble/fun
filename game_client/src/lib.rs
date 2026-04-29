pub mod first_person;

use std::{
    collections::{HashMap, HashSet},
    num::NonZeroU32,
    time::Duration,
};

use avian3d::prelude::{Collider, PhysicsPlugins, RigidBody};
use bevy::render::{
    RenderPlugin,
    render_resource::TextureUsages,
    settings::{Backends, InstanceFlags, RenderCreation, WgpuSettings},
};
#[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
use bevy::{
    anti_alias::dlss::{
        Dlss, DlssPerfQualityMode, DlssProjectId, DlssRayReconstructionFeature,
        DlssRayReconstructionSupported,
    },
    asset::uuid::Uuid,
};
use bevy::{
    camera::CameraMainTextureUsages,
    dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin},
    diagnostic::{DiagnosticPath, DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    pbr::{
        DefaultOpaqueRendererMethod,
        experimental::meshlet::{
            MESHLET_DEFAULT_VERTEX_POSITION_QUANTIZATION_FACTOR, MeshletMesh, MeshletMesh3d,
            MeshletPlugin,
        },
    },
    prelude::*,
    render::diagnostic::RenderDiagnosticsPlugin,
    solari::prelude::{RaytracingMesh3d, SolariLighting, SolariPlugins},
    window::PresentMode,
    winit::WinitSettings,
};
use bevy_quinnet::client::{
    ClientConnectionConfiguration, ClientConnectionConfigurationDefaultables, QuinnetClient,
    QuinnetClientPlugin,
    certificate::CertificateVerificationMode,
    connection::{ClientAddrConfiguration, ConnectionEvent},
};
use first_person::FirstPersonControllerPlugin;
use game_shared::{GAME_SERVER_ADDR, GAME_TITLE};
use thunder::prelude::*;

#[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
const DLSS_PROJECT_ID: &str = "7f2c56d9-bbd1-40e6-aeea-ad1cde733e2e";

#[derive(Debug, Clone, Copy, Resource)]
struct ClientRenderConfig {
    solari_enabled: bool,
    dlss_rr_enabled: bool,
    meshlets_enabled: bool,
}

impl ClientRenderConfig {
    fn from_env() -> Self {
        Self {
            solari_enabled: std::env::var_os("FUN_DISABLE_SOLARI").is_none(),
            dlss_rr_enabled: std::env::var_os("FUN_DISABLE_DLSS_RR").is_none(),
            meshlets_enabled: std::env::var_os("FUN_DISABLE_MESHLETS").is_none(),
        }
    }
}

pub struct GameClientPlugin;

impl Plugin for GameClientPlugin {
    fn build(&self, app: &mut App) {
        let render_config = ClientRenderConfig::from_env();
        if render_config.solari_enabled {
            println!(
                "[client render] Solari lighting will start after the streamed world is ready"
            );
        } else {
            println!("[client render] Solari lighting disabled by FUN_DISABLE_SOLARI");
        }
        if render_config.meshlets_enabled {
            println!("[client render] streamed world will use meshlet meshes");
        } else {
            println!("[client render] streamed world meshlets disabled by FUN_DISABLE_MESHLETS");
        }

        let opaque_renderer_method = if render_config.solari_enabled {
            println!("[client render] default opaque renderer: deferred");
            DefaultOpaqueRendererMethod::deferred()
        } else {
            println!("[client render] default opaque renderer: forward");
            DefaultOpaqueRendererMethod::forward()
        };

        app.insert_resource(opaque_renderer_method)
            .insert_resource(render_config)
            .init_resource::<LoadedWorldState>()
            .init_resource::<ClientWorldStatus>()
            .init_resource::<ClientDiagnostics>()
            .add_plugins((
                PhysicsPlugins::default(),
                QuinnetClientPlugin::default(),
                ThunderPlugin::default(),
                FrameTimeDiagnosticsPlugin::default(),
                MeshletPlugin {
                    cluster_buffer_slots: 1 << 14,
                },
                FpsOverlayPlugin {
                    config: FpsOverlayConfig {
                        refresh_interval: Duration::from_secs(1),
                        text_config: bevy::text::TextFont {
                            font_size: bevy::text::FontSize::Px(12.0),
                            ..default()
                        },
                        ..default()
                    },
                },
                FirstPersonControllerPlugin,
            ))
            .add_systems(Startup, (setup_lighting, connect_to_game_server))
            .add_systems(
                Update,
                (
                    send_client_hello,
                    receive_server_control,
                    receive_world_stream,
                    log_client_diagnostics,
                ),
            );

        if render_config.solari_enabled {
            app.add_plugins(SolariPlugins);
        }

        if std::env::var_os("FUN_RENDER_DIAGNOSTICS").is_some() {
            println!("[client render] GPU render diagnostics enabled");
            app.add_plugins((
                RenderDiagnosticsPlugin,
                bevy::diagnostic::SystemInformationDiagnosticsPlugin,
            ));
        }
    }
}

pub fn build_client_app() -> App {
    let mut app = App::new();
    let render_backend = selected_render_backend();
    let present_mode = selected_present_mode();

    #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
    app.insert_resource(DlssProjectId(
        Uuid::parse_str(DLSS_PROJECT_ID).expect("DLSS project ID should be a valid UUID"),
    ));

    println!(
        "[client render] backend={render_backend:?} present_mode={present_mode:?} vsync=false max_frame_latency=3"
    );

    let default_plugins = DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: format!("{GAME_TITLE} Client").into(),
            present_mode,
            desired_maximum_frame_latency: NonZeroU32::new(3),
            ..default()
        }),
        ..default()
    });
    let default_plugins = default_plugins.set(RenderPlugin {
        render_creation: RenderCreation::Automatic(Box::new(WgpuSettings {
            backends: Some(render_backend),
            instance_flags: InstanceFlags::empty().with_env(),
            ..default()
        })),
        ..default()
    });

    app.add_plugins(default_plugins)
        .insert_resource(WinitSettings::continuous())
        .add_plugins(GameClientPlugin);

    app
}

fn selected_render_backend() -> Backends {
    match std::env::var("FUN_RENDER_BACKEND")
        .as_deref()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Ok("dx12") | Ok("d3d12") | Ok("directx12") => Backends::DX12,
        Ok("auto") => Backends::VULKAN | Backends::DX12,
        Ok("vulkan") | Ok("vk") | Ok("") | Err(_) => Backends::VULKAN,
        Ok(other) => {
            println!("[client render] unknown FUN_RENDER_BACKEND={other}; using Vulkan");
            Backends::VULKAN
        }
    }
}

fn selected_present_mode() -> PresentMode {
    match std::env::var("FUN_PRESENT_MODE")
        .as_deref()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Ok("auto_no_vsync") | Ok("autonovsync") | Ok("auto-no-vsync") => PresentMode::AutoNoVsync,
        Ok("auto_vsync") | Ok("autovsync") | Ok("auto-vsync") => PresentMode::AutoVsync,
        Ok("fifo") | Ok("vsync") => PresentMode::Fifo,
        Ok("fifo_relaxed") | Ok("fifo-relaxed") => PresentMode::FifoRelaxed,
        Ok("mailbox") => PresentMode::Mailbox,
        Ok("immediate") | Ok("") | Err(_) => PresentMode::Immediate,
        Ok(other) => {
            println!("[client render] unknown FUN_PRESENT_MODE={other}; using Immediate");
            PresentMode::Immediate
        }
    }
}

fn setup_lighting(mut commands: Commands) {
    println!("[client render] spawning directional light");
    commands.spawn((
        DirectionalLight {
            illuminance: 15_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(8.0, 16.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn connect_to_game_server(mut client: ResMut<QuinnetClient>) {
    if !client.is_disconnected() {
        println!("[client net] Quinnet already has an active connection");
        return;
    }

    println!("[client net] opening connection to {GAME_SERVER_ADDR}");
    let limits = ChannelLimits::default();
    let config = ClientConnectionConfiguration {
        addr_config: ClientAddrConfiguration::from_strings(GAME_SERVER_ADDR, "0.0.0.0:0")
            .expect("game server address should be valid"),
        cert_mode: CertificateVerificationMode::SkipVerification,
        defaultables: ClientConnectionConfigurationDefaultables {
            send_channels_cfg: ClientChannel::channels_configuration(limits),
            ..Default::default()
        },
    };

    match client.open_connection(config) {
        Ok(connection_id) => {
            println!(
                "[client net] connecting to {GAME_SERVER_ADDR} with local connection {connection_id}"
            );
            info!(
                "Connecting to game server {GAME_SERVER_ADDR} with local connection {connection_id}"
            );
        }
        Err(error) => {
            println!("[client net] failed to start game server connection: {error}");
            error!("Failed to start game server connection: {error}");
        }
    }
}

fn send_client_hello(
    mut events: MessageReader<ConnectionEvent>,
    mut client: ResMut<QuinnetClient>,
) {
    for event in events.read() {
        println!("[client net] connection event id={}", event.id);
        let Some(connection) = client.get_connection_mut_by_id(event.id) else {
            println!(
                "[client net] connection event id={} had no matching connection",
                event.id
            );
            continue;
        };

        let hello = ClientPacket::Hello {
            hello: ClientHello {
                protocol_version: 1,
                session_token: Vec::new(),
                feature_bits: 0,
                oldest_input_sequence: PacketSequence(0),
            },
        };

        match encode_client_packet(&hello) {
            Ok(bytes) => {
                let byte_len = bytes.len();
                connection.try_send_payload_on(ClientChannel::Control, bytes);
                println!(
                    "[client net] sent hello on control channel for connection {} ({} bytes)",
                    event.id, byte_len
                );
            }
            Err(error) => {
                println!("[client net] failed to encode client hello: {error}");
                error!("Failed to encode client hello: {error}");
            }
        }
    }
}

fn receive_server_control(mut client: ResMut<QuinnetClient>) {
    let Some(connection) = client.get_connection_mut() else {
        return;
    };

    while let Some(payload) = connection.try_receive_payload(ServerChannel::Control) {
        println!(
            "[client net] received control payload ({} bytes)",
            payload.as_ref().len()
        );
        match decode_server_packet(payload.as_ref()) {
            Ok(ServerPacket::Welcome { welcome }) => {
                println!(
                    "[client net] welcome client_id={} baseline_tick={}",
                    welcome.client_id.0, welcome.baseline_tick.0
                );
                info!("Connected as network client {}", welcome.client_id.0);
            }
            Ok(ServerPacket::Disconnect { reason }) => {
                println!("[client net] server disconnected client: {reason}");
                warn!("Server disconnected client: {reason}");
            }
            Ok(packet) => {
                println!("[client net] ignoring control packet: {packet:?}");
                debug!("Ignoring control packet on client: {packet:?}");
            }
            Err(error) => {
                println!("[client net] failed to decode server control packet: {error}");
                error!("Failed to decode server control packet: {error}");
            }
        }
    }
}

fn receive_world_stream(
    mut commands: Commands,
    mut client: ResMut<QuinnetClient>,
    render_config: Res<ClientRenderConfig>,
    mut loaded_world: ResMut<LoadedWorldState>,
    mut world_status: ResMut<ClientWorldStatus>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut meshlet_meshes: ResMut<Assets<MeshletMesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    solari_cameras: Query<Entity, (With<Camera3d>, Without<SolariLighting>)>,
    mut solari_lighting: Query<&mut SolariLighting>,
    #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))] dlss_rr_supported: Option<
        Res<DlssRayReconstructionSupported>,
    >,
    #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))] dlss_rr_cameras: Query<
        Entity,
        (With<Camera3d>, Without<Dlss<DlssRayReconstructionFeature>>),
    >,
    #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))] mut dlss_rr: Query<
        &mut Dlss<DlssRayReconstructionFeature>,
    >,
) {
    let Some(connection) = client.get_connection_mut() else {
        return;
    };

    while let Some(payload) = connection.try_receive_payload(ServerChannel::Stream) {
        println!(
            "[client stream] received stream payload ({} bytes)",
            payload.as_ref().len()
        );
        let packet = match decode_server_packet(payload.as_ref()) {
            Ok(ServerPacket::WorldStream { chunk }) => chunk,
            Ok(packet) => {
                println!("[client stream] ignoring non-world stream packet: {packet:?}");
                debug!("Ignoring non-world packet on world stream channel: {packet:?}");
                continue;
            }
            Err(error) => {
                println!("[client stream] failed to decode world stream packet: {error}");
                error!("Failed to decode world stream packet: {error}");
                continue;
            }
        };

        println!(
            "[client stream] applying chunk {}/{} level={} revision={} entities={}",
            packet.chunk_index + 1,
            packet.chunk_count,
            packet.level_id.0,
            packet.revision.0,
            packet.entities.len()
        );

        apply_world_stream_chunk(
            &mut commands,
            &mut loaded_world,
            &mut world_status,
            &mut meshes,
            &mut meshlet_meshes,
            &mut materials,
            &render_config,
            &packet,
        );

        if loaded_world.is_complete() {
            let became_ready = !world_status.ready;
            if became_ready {
                println!(
                    "[client stream] streamed world is ready; enabling player movement after colliders are present"
                );
            }
            world_status.ready = true;
            if became_ready {
                enable_solari_lighting_for_ready_world(
                    &mut commands,
                    &render_config,
                    &solari_cameras,
                    &mut solari_lighting,
                );
                #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
                enable_dlss_ray_reconstruction_for_ready_world(
                    &mut commands,
                    &render_config,
                    dlss_rr_supported.as_deref(),
                    &dlss_rr_cameras,
                    &mut dlss_rr,
                );
                #[cfg(any(not(feature = "dlss"), feature = "force_disable_dlss"))]
                println!("[client render] DLSS feature disabled at compile time");
            }
        }

        if loaded_world.is_complete() && !loaded_world.ack_sent {
            let ack = ClientPacket::WorldReady {
                ack: WorldStreamAck {
                    level_id: WorldLevelId(loaded_world.level_id.clone().unwrap_or_default()),
                    revision: loaded_world.revision.unwrap_or_default(),
                },
            };

            match encode_client_packet(&ack) {
                Ok(bytes) => {
                    let byte_len = bytes.len();
                    connection.try_send_payload_on(ClientChannel::Control, bytes);
                    loaded_world.ack_sent = true;
                    println!(
                        "[client stream] sent world-ready ack level={} revision={} ({} bytes)",
                        loaded_world.level_id.as_deref().unwrap_or_default(),
                        loaded_world.revision.unwrap_or_default().0,
                        byte_len
                    );
                }
                Err(error) => {
                    println!("[client stream] failed to encode world-ready ack: {error}");
                    error!("Failed to encode world ready acknowledgement: {error}");
                }
            }
        }
    }
}

fn enable_solari_lighting_for_ready_world(
    commands: &mut Commands,
    render_config: &ClientRenderConfig,
    solari_cameras: &Query<Entity, (With<Camera3d>, Without<SolariLighting>)>,
    solari_lighting: &mut Query<&mut SolariLighting>,
) {
    if !render_config.solari_enabled {
        println!("[client render] streamed world ready; Solari remains disabled");
        return;
    }

    let mut enabled_count = 0usize;
    for camera_entity in solari_cameras.iter() {
        commands.entity(camera_entity).insert((
            CameraMainTextureUsages::default().with(TextureUsages::STORAGE_BINDING),
            SolariLighting::default(),
        ));
        enabled_count += 1;
    }

    if enabled_count > 0 {
        println!(
            "[client render] enabled Solari lighting for {enabled_count} ready camera view(s)"
        );
    }

    reset_solari_lighting_history(solari_lighting);
}

fn reset_solari_lighting_history(solari_lighting: &mut Query<&mut SolariLighting>) {
    let mut reset_count = 0usize;
    for mut lighting in solari_lighting.iter_mut() {
        lighting.reset = true;
        reset_count += 1;
    }

    if reset_count > 0 {
        println!("[client render] reset Solari temporal history for {reset_count} view(s)");
    }
}

#[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
fn enable_dlss_ray_reconstruction_for_ready_world(
    commands: &mut Commands,
    render_config: &ClientRenderConfig,
    dlss_rr_supported: Option<&DlssRayReconstructionSupported>,
    dlss_rr_cameras: &Query<Entity, (With<Camera3d>, Without<Dlss<DlssRayReconstructionFeature>>)>,
    dlss_rr: &mut Query<&mut Dlss<DlssRayReconstructionFeature>>,
) {
    if !render_config.solari_enabled {
        return;
    }

    if !render_config.dlss_rr_enabled {
        println!("[client render] DLSS Ray Reconstruction disabled by FUN_DISABLE_DLSS_RR");
        return;
    }

    if dlss_rr_supported.is_none() {
        println!(
            "[client render] DLSS Ray Reconstruction unavailable; Solari lighting will use its non-DLSS path"
        );
        info!("DLSS Ray Reconstruction unavailable; Solari lighting will use its non-DLSS path");
        return;
    }

    let mut enabled_count = 0usize;
    for camera_entity in dlss_rr_cameras.iter() {
        commands
            .entity(camera_entity)
            .insert(Dlss::<DlssRayReconstructionFeature> {
                perf_quality_mode: DlssPerfQualityMode::Quality,
                reset: true,
                _phantom_data: Default::default(),
            });
        enabled_count += 1;
    }

    if enabled_count > 0 {
        println!(
            "[client render] enabled DLSS Ray Reconstruction for {enabled_count} ready camera view(s)"
        );
        info!("DLSS Ray Reconstruction enabled for Solari lighting");
    }

    reset_dlss_ray_reconstruction_history(dlss_rr);
}

#[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
fn reset_dlss_ray_reconstruction_history(
    dlss_rr: &mut Query<&mut Dlss<DlssRayReconstructionFeature>>,
) {
    let mut reset_count = 0usize;
    for mut dlss in dlss_rr.iter_mut() {
        dlss.reset = true;
        reset_count += 1;
    }

    if reset_count > 0 {
        println!("[client render] reset DLSS Ray Reconstruction history for {reset_count} view(s)");
    }
}

fn apply_world_stream_chunk(
    commands: &mut Commands,
    loaded_world: &mut LoadedWorldState,
    world_status: &mut ClientWorldStatus,
    meshes: &mut Assets<Mesh>,
    meshlet_meshes: &mut Assets<MeshletMesh>,
    materials: &mut Assets<StandardMaterial>,
    render_config: &ClientRenderConfig,
    chunk: &WorldStreamChunk,
) {
    if loaded_world.revision != Some(chunk.revision)
        || loaded_world.level_id.as_deref() != Some(chunk.level_id.0.as_str())
    {
        println!(
            "[client stream] resetting streamed world: old_level={:?} old_revision={:?} old_entities={}",
            loaded_world.level_id,
            loaded_world.revision.map(|revision| revision.0),
            loaded_world.spawned_entities.len()
        );
        for entity in loaded_world.spawned_entities.values().copied() {
            commands.entity(entity).despawn();
        }

        loaded_world.level_id = Some(chunk.level_id.0.clone());
        loaded_world.revision = Some(chunk.revision);
        loaded_world.expected_chunks = chunk.chunk_count;
        loaded_world.received_chunks.clear();
        loaded_world.spawned_entities.clear();
        loaded_world.ack_sent = false;
        world_status.ready = false;
        info!(
            "Receiving world {} revision {} in {} chunks",
            chunk.level_id.0, chunk.revision.0, chunk.chunk_count
        );
        println!(
            "[client stream] receiving world {} revision {} in {} chunks",
            chunk.level_id.0, chunk.revision.0, chunk.chunk_count
        );
    }

    for spec in &chunk.entities {
        if loaded_world.spawned_entities.contains_key(&spec.entity) {
            println!(
                "[client stream] skipping duplicate entity {} ({})",
                spec.entity.0, spec.name
            );
            continue;
        }

        let entity = spawn_streamed_entity(
            commands,
            meshes,
            meshlet_meshes,
            materials,
            render_config,
            spec,
        );
        println!(
            "[client stream] spawned ECS entity {:?} for net entity {} ({})",
            entity, spec.entity.0, spec.name
        );
        loaded_world.spawned_entities.insert(spec.entity, entity);
    }

    loaded_world.received_chunks.insert(chunk.chunk_index);
    println!(
        "[client stream] chunk complete: received_chunks={}/{} spawned_entities={}",
        loaded_world.received_chunks.len(),
        loaded_world.expected_chunks,
        loaded_world.spawned_entities.len()
    );
}

fn spawn_streamed_entity(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    meshlet_meshes: &mut Assets<MeshletMesh>,
    materials: &mut Assets<StandardMaterial>,
    render_config: &ClientRenderConfig,
    spec: &WorldEntitySpec,
) -> Entity {
    let translation = vec3_from_quantized(spec.transform.translation);
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
    ));

    println!(
        "[client stream] spawn spec net={} name={} class={:?} authority={:?} translation=({:.2}, {:.2}, {:.2}) render={} collider={} color={}",
        spec.entity.0,
        spec.name,
        spec.class,
        spec.authority,
        translation.x,
        translation.y,
        translation.z,
        primitive_summary(spec.render),
        collider_summary(spec.collider),
        color_summary(spec.color),
    );

    if let Some(primitive) = spec.render {
        let mesh = mesh_from_primitive(primitive);
        let (raytracing_mesh, meshlet_mesh) =
            add_scene_mesh_assets(meshes, meshlet_meshes, mesh, &spec.name, render_config);
        let material = materials.add(color_from_packed(spec.color));

        if render_config.meshlets_enabled {
            entity_commands.insert((
                MeshletMesh3d(meshlet_mesh.expect("meshlet handle should exist when enabled")),
                MeshMaterial3d::<StandardMaterial>(material.clone()),
            ));
        } else if let Some(mesh) = raytracing_mesh.as_ref() {
            entity_commands.insert((
                Mesh3d(mesh.clone()),
                MeshMaterial3d::<StandardMaterial>(material.clone()),
            ));
        }

        if render_config.solari_enabled
            && let Some(raytracing_mesh) = raytracing_mesh
        {
            entity_commands.insert(RaytracingMesh3d(raytracing_mesh));
        }

        println!(
            "[client render] inserted render components for {} meshlet={} raytracing={}",
            spec.name, render_config.meshlets_enabled, render_config.solari_enabled,
        );
    }

    if let Some(collider) = spec.collider {
        entity_commands.insert((RigidBody::Static, collider_from_stream(collider)));
    }

    entity_commands.id()
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

fn collider_from_stream(collider: WorldCollider) -> Collider {
    match collider {
        WorldCollider::Cuboid { size } => {
            let size = vec3_from_quantized(size);
            Collider::cuboid(size.x, size.y, size.z)
        }
    }
}

fn add_scene_mesh_assets(
    meshes: &mut Assets<Mesh>,
    meshlet_meshes: &mut Assets<MeshletMesh>,
    mesh: Mesh,
    name: &str,
    render_config: &ClientRenderConfig,
) -> (Option<Handle<Mesh>>, Option<Handle<MeshletMesh>>) {
    let vertex_count = mesh.count_vertices();
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

    println!(
        "[client render] built mesh assets for {name}: vertices={} raytracing_handle={:?} meshlet_handle={:?}",
        vertex_count, raytracing_handle, meshlet_handle
    );

    (raytracing_handle, meshlet_handle)
}

fn log_client_diagnostics(
    time: Res<Time>,
    mut diagnostics: ResMut<ClientDiagnostics>,
    render_diagnostics: Res<DiagnosticsStore>,
    loaded_world: Res<LoadedWorldState>,
    cameras: Query<
        (
            Entity,
            Option<&Name>,
            &GlobalTransform,
            Option<&Camera>,
            Option<&Projection>,
            Option<&SolariLighting>,
        ),
        With<Camera3d>,
    >,
    renderables: Query<
        (
            Entity,
            Option<&Name>,
            Option<&GlobalTransform>,
            Option<&Transform>,
            Option<&MeshletMesh3d>,
            Option<&RaytracingMesh3d>,
            Option<&Mesh3d>,
            Option<&MeshMaterial3d<StandardMaterial>>,
            Option<&Collider>,
        ),
        Or<(
            With<MeshletMesh3d>,
            With<RaytracingMesh3d>,
            With<Mesh3d>,
            With<Collider>,
        )>,
    >,
) {
    if !diagnostics.timer.tick(time.delta()).just_finished() {
        return;
    }

    let mut renderable_count = 0usize;
    let mut meshlet_count = 0usize;
    let mut raytracing_count = 0usize;
    let mut mesh3d_count = 0usize;
    let mut material_count = 0usize;
    let mut collider_count = 0usize;
    let mut samples = Vec::new();

    for (
        entity,
        name,
        global_transform,
        transform,
        meshlet,
        raytracing,
        mesh3d,
        material,
        collider,
    ) in &renderables
    {
        renderable_count += 1;
        meshlet_count += usize::from(meshlet.is_some());
        raytracing_count += usize::from(raytracing.is_some());
        mesh3d_count += usize::from(mesh3d.is_some());
        material_count += usize::from(material.is_some());
        collider_count += usize::from(collider.is_some());

        if samples.len() < 6 {
            let translation = global_transform
                .map(|transform| transform.translation())
                .or_else(|| transform.map(|transform| transform.translation))
                .unwrap_or(Vec3::NAN);
            samples.push(format!(
                "{:?}/{} pos=({:.2},{:.2},{:.2}) meshlet={} ray={} mesh3d={} mat={} collider={}",
                entity,
                name.map(|name| name.as_str()).unwrap_or("<unnamed>"),
                translation.x,
                translation.y,
                translation.z,
                meshlet.is_some(),
                raytracing.is_some(),
                mesh3d.is_some(),
                material.is_some(),
                collider.is_some(),
            ));
        }
    }

    let camera_count = cameras.iter().count();
    let active_camera_count = cameras
        .iter()
        .filter(|(_, _, _, camera, _, _)| camera.is_none_or(|camera| camera.is_active))
        .count();
    if camera_count != 1 || active_camera_count != 1 {
        println!(
            "[client camera] expected exactly one active 3D camera, found cameras={} active={}",
            camera_count, active_camera_count
        );
        warn!(
            "Expected exactly one active 3D camera, found cameras={} active={}",
            camera_count, active_camera_count
        );
    }

    println!(
        "[client diag] world level={:?} revision={:?} chunks={}/{} spawned={} ack_sent={} cameras={} active_cameras={} renderables={} meshlet={} raytracing={} mesh3d={} materials={} colliders={}",
        loaded_world.level_id,
        loaded_world.revision.map(|revision| revision.0),
        loaded_world.received_chunks.len(),
        loaded_world.expected_chunks,
        loaded_world.spawned_entities.len(),
        loaded_world.ack_sent,
        camera_count,
        active_camera_count,
        renderable_count,
        meshlet_count,
        raytracing_count,
        mesh3d_count,
        material_count,
        collider_count,
    );

    for (entity, name, transform, camera, projection, solari) in &cameras {
        let translation = transform.translation();
        println!(
            "[client diag] camera {:?}/{} active={} projection={} solari={} pos=({:.2},{:.2},{:.2}) forward=({:.2},{:.2},{:.2})",
            entity,
            name.map(|name| name.as_str()).unwrap_or("<unnamed>"),
            camera.is_none_or(|camera| camera.is_active),
            projection.map(projection_summary).unwrap_or("<none>"),
            solari.is_some(),
            translation.x,
            translation.y,
            translation.z,
            transform.forward().x,
            transform.forward().y,
            transform.forward().z,
        );
    }

    for sample in samples {
        println!("[client diag] renderable {sample}");
    }

    log_render_performance(&render_diagnostics);
}

fn primitive_summary(primitive: Option<WorldPrimitive>) -> String {
    match primitive {
        Some(WorldPrimitive::Plane { size }) => {
            let size = vec3_from_quantized(size);
            format!("plane({:.2},{:.2},{:.2})", size.x, size.y, size.z)
        }
        Some(WorldPrimitive::Cuboid { size }) => {
            let size = vec3_from_quantized(size);
            format!("cuboid({:.2},{:.2},{:.2})", size.x, size.y, size.z)
        }
        None => "none".to_owned(),
    }
}

fn collider_summary(collider: Option<WorldCollider>) -> String {
    match collider {
        Some(WorldCollider::Cuboid { size }) => {
            let size = vec3_from_quantized(size);
            format!("cuboid({:.2},{:.2},{:.2})", size.x, size.y, size.z)
        }
        None => "none".to_owned(),
    }
}

fn color_summary(color: Option<PackedColorRgba8>) -> String {
    color
        .map(|color| format!("rgba({}, {}, {}, {})", color.r, color.g, color.b, color.a))
        .unwrap_or_else(|| "none".to_owned())
}

fn projection_summary(projection: &Projection) -> &'static str {
    match projection {
        Projection::Perspective(_) => "perspective",
        Projection::Orthographic(_) => "orthographic",
        Projection::Custom(_) => "custom",
    }
}

fn log_render_performance(diagnostics: &DiagnosticsStore) {
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|diagnostic| diagnostic.value());
    let frame_ms = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|diagnostic| diagnostic.value());

    let solari_presample = diagnostic_average(
        diagnostics,
        "render/solari_lighting/presample_light_tiles/elapsed_gpu",
    );
    let solari_world_cache = diagnostic_average(
        diagnostics,
        "render/solari_lighting/world_cache/elapsed_gpu",
    );
    let solari_direct = diagnostic_average(
        diagnostics,
        "render/solari_lighting/direct_lighting/elapsed_gpu",
    );
    let solari_diffuse = diagnostic_average(
        diagnostics,
        "render/solari_lighting/diffuse_indirect_lighting/elapsed_gpu",
    );
    let solari_specular = diagnostic_average(
        diagnostics,
        "render/solari_lighting/specular_indirect_lighting/elapsed_gpu",
    );
    let solari_passes = [
        solari_presample,
        solari_world_cache,
        solari_direct,
        solari_diffuse,
        solari_specular,
    ];
    let solari_total = solari_passes
        .into_iter()
        .flatten()
        .fold(None, |total: Option<f64>, value| {
            Some(total.unwrap_or_default() + value)
        });
    let meshlet_visibility = diagnostic_average(
        diagnostics,
        "render/meshlet_visibility_buffer_raster/elapsed_gpu",
    );
    let dlss_rr = diagnostic_average(diagnostics, "render/dlss_ray_reconstruction/elapsed_gpu");

    println!(
        "[client perf] fps={} frame_ms={} solari_gpu_ms={} meshlet_visibility_gpu_ms={} dlss_rr_gpu_ms={}",
        format_optional_number(fps),
        format_optional_number(frame_ms),
        format_optional_number(solari_total),
        format_optional_number(meshlet_visibility),
        format_optional_number(dlss_rr),
    );

    let process_cpu = diagnostic_average_path(
        diagnostics,
        &bevy::diagnostic::SystemInformationDiagnosticsPlugin::PROCESS_CPU_USAGE,
    );
    let process_mem = diagnostic_average_path(
        diagnostics,
        &bevy::diagnostic::SystemInformationDiagnosticsPlugin::PROCESS_MEM_USAGE,
    );
    let system_cpu = diagnostic_average_path(
        diagnostics,
        &bevy::diagnostic::SystemInformationDiagnosticsPlugin::SYSTEM_CPU_USAGE,
    );
    let system_mem = diagnostic_average_path(
        diagnostics,
        &bevy::diagnostic::SystemInformationDiagnosticsPlugin::SYSTEM_MEM_USAGE,
    );
    println!(
        "[client perf] process_cpu_pct={} process_mem_gib={} system_cpu_pct={} system_mem_pct={}",
        format_optional_number(process_cpu),
        format_optional_number(process_mem),
        format_optional_number(system_cpu),
        format_optional_number(system_mem),
    );

    println!(
        "[client perf] solari passes gpu_ms: presample={} world_cache={} direct={} diffuse={} specular={}",
        format_optional_number(solari_presample),
        format_optional_number(solari_world_cache),
        format_optional_number(solari_direct),
        format_optional_number(solari_diffuse),
        format_optional_number(solari_specular),
    );

    let mut render_timings = diagnostics
        .iter()
        .filter_map(|diagnostic| {
            let path = diagnostic.path().as_str();
            if path.starts_with("render/")
                && (path.ends_with("/elapsed_gpu") || path.ends_with("/elapsed_cpu"))
            {
                diagnostic.average().map(|value| (path.to_owned(), value))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    render_timings.sort_by(|(_, left), (_, right)| {
        right.partial_cmp(left).unwrap_or(std::cmp::Ordering::Equal)
    });

    if !render_timings.is_empty() {
        let top_timings = render_timings
            .into_iter()
            .take(8)
            .map(|(path, value)| format!("{path}={}", format_number(value)))
            .collect::<Vec<_>>()
            .join(", ");
        println!("[client perf] top render timings {top_timings}");
    }
}

fn diagnostic_average(diagnostics: &DiagnosticsStore, path: &'static str) -> Option<f64> {
    diagnostics
        .get(&DiagnosticPath::new(path))
        .and_then(|diagnostic| diagnostic.average())
}

fn diagnostic_average_path(diagnostics: &DiagnosticsStore, path: &DiagnosticPath) -> Option<f64> {
    diagnostics
        .get(path)
        .and_then(|diagnostic| diagnostic.average())
}

fn format_optional_number(value: Option<f64>) -> String {
    value
        .map(format_number)
        .unwrap_or_else(|| "pending".to_owned())
}

fn format_number(value: f64) -> String {
    format!("{value:.2}")
}

fn color_from_packed(color: Option<PackedColorRgba8>) -> StandardMaterial {
    let color = color.unwrap_or_else(|| PackedColorRgba8::srgb(180, 180, 180));
    Color::srgba_u8(color.r, color.g, color.b, color.a).into()
}

fn transform_from_quantized(transform: QuantizedTransform3) -> Transform {
    let rotation = transform.rotation.to_f32();
    Transform::from_translation(vec3_from_quantized(transform.translation)).with_rotation(
        Quat::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3]),
    )
}

fn vec3_from_quantized(value: QuantizedVec3) -> Vec3 {
    Vec3::from_array(value.to_f32(Quantization::MILLIMETERS))
}

#[derive(Debug, Default, Resource)]
struct LoadedWorldState {
    level_id: Option<String>,
    revision: Option<WorldRevision>,
    expected_chunks: u16,
    received_chunks: HashSet<u16>,
    spawned_entities: HashMap<NetEntity, Entity>,
    ack_sent: bool,
}

#[derive(Debug, Default, Resource)]
pub(crate) struct ClientWorldStatus {
    pub ready: bool,
}

#[derive(Debug, Resource)]
struct ClientDiagnostics {
    timer: Timer,
}

impl Default for ClientDiagnostics {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(2.0, TimerMode::Repeating),
        }
    }
}

impl LoadedWorldState {
    fn is_complete(&self) -> bool {
        self.expected_chunks > 0 && self.received_chunks.len() >= self.expected_chunks as usize
    }
}
