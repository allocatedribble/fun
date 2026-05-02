use std::hint::black_box;

use bevy::{
    math::Vec2,
    prelude::{Quat, Vec3},
    solari::prelude::{SolariDenoiseMode, SolariPlugins, SolariSettings},
};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use game_client::{benchmark_parse_solari_denoise_mode, first_person};
use thunder::prelude::*;

fn solari_and_denoiser_costs(c: &mut Criterion) {
    let mut group = c.benchmark_group("client/solari_setup");
    for mode in [
        SolariDenoiseMode::Off,
        SolariDenoiseMode::CheapTemporal,
        SolariDenoiseMode::BalancedFast,
        SolariDenoiseMode::Balanced,
        SolariDenoiseMode::Quality,
        SolariDenoiseMode::DlssRayReconstruction,
    ] {
        group.bench_with_input(
            BenchmarkId::new("settings_clone", format!("{mode:?}")),
            &mode,
            |b, mode| {
                let settings = SolariSettings {
                    denoise_mode: *mode,
                    ..Default::default()
                };
                b.iter(|| black_box(settings.clone()));
            },
        );
    }
    group.bench_function("required_wgpu_features", |b| {
        b.iter(|| black_box(SolariPlugins::required_wgpu_features()));
    });
    group.finish();

    let mut group = c.benchmark_group("client/denoiser_mode_selection");
    for mode in [
        "off",
        "cheap-temporal",
        "balanced-fast",
        "balanced",
        "quality",
        "dlss-rr",
        "ray-reconstruction",
    ] {
        group.bench_with_input(BenchmarkId::from_parameter(mode), mode, |b, mode| {
            b.iter(|| {
                black_box(benchmark_parse_solari_denoise_mode(
                    Some(black_box(mode)),
                    false,
                ))
            });
        });
    }
    group.bench_function("default_balanced_fast_rr_available", |b| {
        b.iter(|| black_box(benchmark_parse_solari_denoise_mode(None, false)));
    });
    group.bench_function("default_balanced_fast_rr_disabled", |b| {
        b.iter(|| black_box(benchmark_parse_solari_denoise_mode(None, true)));
    });
    group.finish();
}

fn movement_and_physics_costs(c: &mut Criterion) {
    let mut group = c.benchmark_group("client/movement_math");
    let inputs = [
        ("idle", Vec2::ZERO),
        ("forward", Vec2::new(0.0, 1.0)),
        ("strafe", Vec2::new(1.0, 0.0)),
        ("diagonal", Vec2::new(1.0, 1.0).normalize()),
    ];
    for (name, input) in inputs {
        group.bench_with_input(BenchmarkId::from_parameter(name), &input, |b, input| {
            b.iter(|| {
                black_box(first_person::benchmark_desired_planar_velocity(
                    black_box(*input),
                    black_box(1.37),
                    black_box(7.5),
                ))
            });
        });
    }
    group.bench_function("max_slope_dot", |b| {
        b.iter(|| black_box(first_person::benchmark_max_slope_dot()));
    });
    group.finish();

    let states = sample_body_states(1024);
    let quantized = states
        .iter()
        .map(|state| state.quantize(Quantization::MILLIMETERS))
        .collect::<Vec<_>>();

    let mut group = c.benchmark_group("client/physics_quantization");
    group.throughput(Throughput::Elements(states.len() as u64));
    group.bench_function("body_state_quantize_1024", |b| {
        b.iter(|| {
            for state in &states {
                black_box(black_box(*state).quantize(Quantization::MILLIMETERS));
            }
        });
    });
    group.bench_function("body_state_dequantize_1024", |b| {
        b.iter(|| {
            for state in &quantized {
                black_box(black_box(*state).dequantize(Quantization::MILLIMETERS));
            }
        });
    });
    group.bench_function("prediction_error_1024", |b| {
        b.iter(|| {
            for pair in states.windows(2) {
                black_box(black_box(pair[0]).prediction_error(black_box(pair[1])));
            }
        });
    });
    group.finish();
}

fn networking_costs(c: &mut Criterion) {
    let input_packet = ClientPacket::Input {
        input: ClientInputFrame {
            sequence: PacketSequence(31337),
            target: TimelineStamp {
                tick: NetworkTick(42),
                subtick: Subtick::default(),
            },
            last_received_server_tick: NetworkTick(39),
            input_kind: 1,
            payload: vec![1, 0, 1, 0, 255, 128, 64, 32],
        },
    };
    let encoded_input = encode_client_packet(&input_packet).expect("sample input packet encodes");

    let mut group = c.benchmark_group("client/network_protocol");
    group.throughput(Throughput::Bytes(encoded_input.len() as u64));
    group.bench_function("encode_client_input", |b| {
        b.iter(|| black_box(encode_client_packet(black_box(&input_packet)).unwrap()));
    });
    group.bench_function("decode_client_input", |b| {
        b.iter(|| black_box(decode_client_packet(black_box(&encoded_input)).unwrap()));
    });

    for entity_count in [16_usize, 128, 512] {
        let packet = ServerPacket::WorldStream {
            chunk: sample_world_stream_chunk(entity_count),
        };
        let encoded = encode_server_packet(&packet).expect("sample world stream packet encodes");
        group.throughput(Throughput::Bytes(encoded.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("encode_world_stream", entity_count),
            &packet,
            |b, packet| {
                b.iter(|| black_box(encode_server_packet(black_box(packet)).unwrap()));
            },
        );
        group.bench_with_input(
            BenchmarkId::new("decode_world_stream", entity_count),
            &encoded,
            |b, encoded| {
                b.iter(|| black_box(decode_server_packet(black_box(encoded)).unwrap()));
            },
        );
    }
    group.finish();

    let manifest = sample_streaming_manifest(512);
    let cache = sample_resource_cache(256);
    let mut group = c.benchmark_group("client/streaming_plan");
    group.throughput(Throughput::Elements(manifest.resources.len() as u64));
    group.bench_function("plan_loads_512", |b| {
        b.iter(|| black_box(black_box(&manifest).plan_loads(black_box(&cache), black_box(256))));
    });
    group.bench_function("cache_directive_512", |b| {
        b.iter(|| {
            black_box(CacheDirective::from_manifest(
                black_box(&manifest),
                black_box(&cache),
                black_box(256),
            ))
        });
    });
    group.finish();

    let grid = sample_interest_grid(10_000);
    let observer = Observer::new([160.0, 12.0, -96.0], 220.0);
    let mut hits = Vec::with_capacity(512);
    let mut group = c.benchmark_group("client/network_relevance");
    group.throughput(Throughput::Elements(10_000));
    group.bench_function("collect_relevant_10000", |b| {
        b.iter(|| {
            grid.collect_relevant(black_box(observer), black_box(&mut hits));
            black_box(hits.len());
        });
    });
    group.finish();
}

fn sample_body_states(count: usize) -> Vec<BodyState3> {
    (0..count)
        .map(|index| {
            let t = index as f32 * 0.017;
            let rotation = Quat::from_rotation_y(t * 0.5).to_array();
            BodyState3 {
                translation: [t.sin() * 30.0, 2.0 + (t * 0.25).cos(), t.cos() * 30.0],
                rotation,
                linear_velocity: [t.cos() * 4.0, t.sin(), t.sin() * 4.0],
                angular_velocity: [0.0, t * 0.25, 0.0],
                sleeping: false,
            }
        })
        .collect()
}

fn sample_world_stream_chunk(entity_count: usize) -> WorldStreamChunk {
    WorldStreamChunk {
        level_id: WorldLevelId("criterion_arena".to_owned()),
        revision: WorldRevision(7),
        chunk_index: 0,
        chunk_count: 1,
        manifest_signature: 0x6372_6974_6572_6961,
        entities: (0..entity_count)
            .map(|index| sample_world_entity(index as u64))
            .collect(),
    }
}

fn sample_world_entity(index: u64) -> WorldEntitySpec {
    let position = Vec3::new(
        (index % 64) as f32 * 2.0,
        ((index / 64) % 8) as f32,
        (index / 512) as f32 * 2.0,
    );
    WorldEntitySpec {
        entity: NetEntity(index + 1),
        name: format!("criterion-entity-{index}"),
        class: ReplicationClass::World,
        authority: AuthorityMode::StaticServer,
        transform: QuantizedTransform3 {
            translation: QuantizedVec3::from_f32(position.to_array(), Quantization::MILLIMETERS),
            rotation: QuantizedQuat::from_f32(Quat::IDENTITY.to_array()),
        },
        catalog: None,
        render: Some(WorldPrimitive::Cuboid {
            size: QuantizedVec3::from_f32([1.0, 1.0, 1.0], Quantization::MILLIMETERS),
        }),
        collider: Some(WorldCollider::Cuboid {
            size: QuantizedVec3::from_f32([1.0, 1.0, 1.0], Quantization::MILLIMETERS),
        }),
        color: Some(PackedColorRgba8::srgb(204, 102, 77)),
    }
}

fn sample_streaming_manifest(resource_count: usize) -> StreamingManifest {
    StreamingManifest {
        version: ManifestVersion(11),
        scope: "criterion-world".to_owned(),
        server_tick: NetworkTick(9000),
        resources: (0..resource_count)
            .map(|index| {
                let required = index % 5 == 0;
                let preload = index % 3 == 0;
                ResourceManifestEntry {
                    id: ResourceId(index as u64 + 1),
                    name: format!("resource-{index}"),
                    version: ResourceVersion(2),
                    kind: match index % 5 {
                        0 => ResourceKind::Mesh,
                        1 => ResourceKind::Image,
                        2 => ResourceKind::Material,
                        3 => ResourceKind::Shader,
                        _ => ResourceKind::WorldData,
                    },
                    byte_len: 16_384 + (index as u64 % 64) * 1024,
                    digest: ResourceDigest::new(IntegrityAlgorithm::Xxh3_128, digest_bytes(index)),
                    source: ResourceSource::ServerStream {
                        chunk_bytes: 32 * 1024,
                    },
                    cache: ResourceCachePolicy {
                        pin: if required {
                            CachePin::Pinned
                        } else {
                            CachePin::KeepWarm
                        },
                        verify_on_session_start: required,
                        retention_priority: 1.0 + (index % 8) as f32,
                        ..Default::default()
                    },
                    load: ResourceLoadPolicy {
                        required_before_play: required,
                        preload,
                        relevance: ((resource_count - index) as f32 / resource_count as f32)
                            * 100.0,
                        relevant_entity: Some(NetEntity(index as u64 + 1)),
                    },
                    dependencies: if index > 2 {
                        vec![ResourceId(index as u64 - 1), ResourceId(index as u64 - 2)]
                    } else {
                        Vec::new()
                    },
                    deltas: vec![ResourceDelta {
                        from_version: ResourceVersion(1),
                        to_version: ResourceVersion(2),
                        source_digest: ResourceDigest::new(
                            IntegrityAlgorithm::Xxh3_128,
                            digest_bytes(index + 10_000),
                        ),
                        target_digest: ResourceDigest::new(
                            IntegrityAlgorithm::Xxh3_128,
                            digest_bytes(index),
                        ),
                        patch_digest: ResourceDigest::new(
                            IntegrityAlgorithm::Xxh3_128,
                            digest_bytes(index + 20_000),
                        ),
                        patch_bytes: 4096 + (index as u64 % 32) * 256,
                        source: ResourceDeltaSource::ServerPatch {
                            chunk_bytes: 16 * 1024,
                        },
                    }],
                }
            })
            .collect(),
    }
}

fn sample_resource_cache(resource_count: usize) -> ResourceCache {
    let mut cache = ResourceCache::with_capacity(resource_count);
    for index in 0..resource_count {
        cache.upsert(CachedResource {
            id: ResourceId(index as u64 + 1),
            version: if index % 4 == 0 {
                ResourceVersion(2)
            } else {
                ResourceVersion(1)
            },
            digest: ResourceDigest::new(
                IntegrityAlgorithm::Xxh3_128,
                if index % 4 == 0 {
                    digest_bytes(index)
                } else {
                    digest_bytes(index + 10_000)
                },
            ),
            byte_len: 16_384 + (index as u64 % 64) * 1024,
            state: if index % 11 == 0 {
                CacheEntryState::Corrupt
            } else {
                CacheEntryState::Verified
            },
        });
    }
    cache
}

fn sample_interest_grid(object_count: usize) -> InterestGrid {
    let policy = RelevancePolicy {
        cell_size: 64.0,
        max_entities_per_client: 512,
        hysteresis_multiplier: 1.2,
    };
    let mut grid = InterestGrid::with_capacity(policy, object_count);
    for index in 0..object_count {
        let x = ((index % 160) as f32 - 80.0) * 8.0;
        let z = (((index / 160) % 160) as f32 - 80.0) * 8.0;
        let y = ((index / 25_600) % 16) as f32 * 2.0;
        let mut object = ReplicatedObject::new(NetEntity(index as u64 + 1), [x, y, z], 12.0);
        object.base_priority = 1.0 + (index % 16) as f32 * 0.1;
        object.layers = if index % 7 == 0 {
            RelevanceLayers::GROUND_VEHICLES
        } else {
            RelevanceLayers::INFANTRY
        };
        object.always_relevant = index % 997 == 0;
        grid.upsert(object);
    }
    grid
}

fn digest_bytes(seed: usize) -> Vec<u8> {
    (0..16)
        .map(|offset| seed.wrapping_mul(31).wrapping_add(offset * 17) as u8)
        .collect()
}

criterion_group!(
    client_costs,
    solari_and_denoiser_costs,
    movement_and_physics_costs,
    networking_costs
);
criterion_main!(client_costs);
