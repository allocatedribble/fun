use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use fun_ecs::*;

const REGION_EDGE_PAGES: u16 = 8;

fn terrain_page(index: u32, channel: EcsPageChannel) -> EcsSpatialPageKey {
    EcsSpatialPageKey::new(
        EcsSpatialDomainKind::Terrain,
        EcsSpatialGridId::new(1),
        0,
        (index % 1024) as i32,
        ((index / 1024) % 1024) as i32,
        (index / 1_048_576) as i32,
        channel,
    )
}

fn priority(index: u32) -> EcsStreamPriority {
    EcsStreamPriority::new(EcsStreamInterestKind::VisibleNear, (index % 8) as u16, 0).with_scores(
        (index % 65_535) as u16,
        128,
        64,
        32,
        16,
    )
}

fn stream_source() -> EcsStreamSourceDescriptor {
    EcsStreamSourceDescriptor {
        source: EcsStreamSourceId::new(1),
        domain: EcsSpatialDomainKind::Terrain,
        grid_id: EcsSpatialGridId::new(1),
        priority: 255,
    }
}

fn stream_camera() -> SpatialStreamCamera {
    SpatialStreamCamera {
        role: StreamCameraRole::MainView,
        enabled: true,
        priority: 255,
        required_shells: 1,
        desired_shells: 2,
        velocity_lookahead_s: 0.5,
    }
}

fn camera_sample(index: u32) -> EcsCameraTransformSample {
    EcsCameraTransformSample {
        camera_entity: EcsEntityId::new(u64::from(index) + 1),
        camera_id: EcsStreamCameraId::new(index + 1),
        origin_world_ft: IVec3::new((index as i32) * 96, 0, 0),
        velocity_world_ft_s: IVec3::new(32, 0, 0),
        view_frustum: EcsViewFrustum::default(),
        cut_or_teleport: false,
    }
}

fn source_snapshot(camera_count: u16) -> EcsStreamingSourceSnapshot {
    let cameras: Vec<(SpatialStreamCamera, EcsCameraTransformSample)> =
        (0..u32::from(camera_count))
            .map(|index| (stream_camera(), camera_sample(index)))
            .collect();
    sense_sources(1, &cameras, &[stream_source()])
}

fn residency_table(rows: u32) -> EcsPageResidencyTable {
    let mut table = EcsPageResidencyTable::default();
    for index in 0..rows {
        let mut record = EcsPageResidencyRecord::new(
            terrain_page(index, EcsPageChannel::Occupancy),
            priority(index),
            index + 1,
        );
        record.state = EcsPageResidencyState::FullyReady;
        table
            .push(record)
            .expect("residency table admits bench row");
    }
    table
}

fn source_acquire_queue(rows: u32) -> EcsSourceAcquireQueue {
    let mut queue = EcsSourceAcquireQueue::default();
    for index in 0..rows {
        let key = terrain_page(index, EcsPageChannel::Occupancy);
        let region = EcsSpatialRegionKey::from_page(key, REGION_EDGE_PAGES);
        let bytes = vec![(index % 251 + 1) as u8];
        let checksum = EcsSourceChecksum::fnv1a64(&bytes);
        let payload =
            EcsCompressedPagePayload::try_new(key, EcsSourcePayloadCodec::None, bytes, checksum)
                .expect("bench source payload is bounded");
        queue
            .push(EcsSourceAcquireRecord {
                request: EcsSourceRequest {
                    request_id: EcsSourceRequestId::new(u64::from(index) + 1),
                    source: EcsSpatialSourceId::new(1),
                    source_kind: EcsSpatialSourceKind::DebugSynthetic,
                    key,
                    region,
                    manifest_epoch: 1,
                    source_epoch: 1,
                    priority: priority(index),
                    payload: EcsSourcePayload::CompressedPage(payload),
                },
                manifest: EcsSpatialRegionManifest {
                    region,
                    source: EcsSpatialSourceId::new(1),
                    source_kind: EcsSpatialSourceKind::DebugSynthetic,
                    manifest_epoch: 1,
                    page_count: u32::from(REGION_EDGE_PAGES).pow(3),
                    channel_mask: EcsPageChannelMask::terrain_primary(),
                    checksum,
                },
            })
            .expect("source acquire queue admits bench row");
    }
    queue
}

fn decoded_pages(rows: u32) -> EcsDecodedPageQueue {
    let acquired = source_acquire_queue(rows);
    let mut decoded = EcsDecodedPageQueue::default();
    decode_pages(&acquired, &[], 7, &mut decoded).expect("decode fixture succeeds");
    decoded
}

fn artifact_rows(rows: u32, consumer: EcsBenchmarkConsumer) -> Vec<EcsDerivedArtifactRecord> {
    let (kind, artifact_consumer, requiredness) = match consumer {
        EcsBenchmarkConsumer::Renderer => (
            EcsDerivedArtifactKind::TerrainSurfacePackets,
            EcsArtifactConsumer::Renderer,
            WorkRequiredness::Required,
        ),
        EcsBenchmarkConsumer::Lux => (
            EcsDerivedArtifactKind::TerrainSdf,
            EcsArtifactConsumer::Lux,
            WorkRequiredness::Required,
        ),
        EcsBenchmarkConsumer::AvisPhysics => (
            EcsDerivedArtifactKind::CollisionSdfProxy,
            EcsArtifactConsumer::AvisPhysics,
            WorkRequiredness::Required,
        ),
        EcsBenchmarkConsumer::Thunder => (
            EcsDerivedArtifactKind::NetworkRelevanceRows,
            EcsArtifactConsumer::ThunderNetwork,
            WorkRequiredness::Required,
        ),
        EcsBenchmarkConsumer::None => (
            EcsDerivedArtifactKind::TerrainSurfacePackets,
            EcsArtifactConsumer::Renderer,
            WorkRequiredness::Required,
        ),
    };
    (0..rows)
        .map(|index| EcsDerivedArtifactRecord {
            artifact_id: EcsDerivedArtifactId::new(u64::from(index) + 1),
            source_page: terrain_page(index, EcsPageChannel::Surface),
            kind,
            source_epoch: 1,
            source_digest: derived_artifact_source_digest(
                terrain_page(index, EcsPageChannel::Surface),
                1,
                index + 1,
            ),
            artifact_epoch: index + 1,
            state: EcsArtifactState::Ready,
            requiredness,
            consumer: artifact_consumer,
        })
        .collect()
}

fn publish_thunder_handoffs(artifacts: &[EcsDerivedArtifactRecord]) -> u32 {
    let mut queue = EcsCrossDomainHandoffQueues::default();
    for artifact in artifacts
        .iter()
        .filter(|artifact| artifact.consumer == EcsArtifactConsumer::ThunderNetwork)
    {
        queue
            .push(EcsCrossDomainHandoff::new(
                EcsHandoffQueueId::new(4),
                EcsCrossDomainHandoffKind::NetworkRelevanceRow,
                artifact.consumer,
                artifact.artifact_id,
                artifact.source_page,
                artifact.artifact_epoch,
            ))
            .expect("thunder handoff queue admits bench row");
    }
    queue.rows.len() as u32
}

fn voxel_edit() -> VoxelEditOp {
    VoxelEditOp::PaintMaterial {
        bounds: EcsAabbI64::new([0, 0, 0], [32, 32, 32]),
        material: TerrainMaterialId::new(1),
    }
}

fn dirty_page_table(rows: u32) -> EcsPageResidencyTable {
    let mut table = EcsPageResidencyTable::default();
    for index in 0..rows {
        let page = terrain_page(index, EcsPageChannel::Surface);
        table
            .push(EcsPageResidencyRecord::new(
                page,
                priority(index),
                index + 1,
            ))
            .expect("dirty page table admits bench row");
    }
    table
}

fn propagate_dirty_region_batch(rows: u32, mut page_table: EcsPageResidencyTable) -> u32 {
    let mut dirty_ledger = EcsDirtyRegionLedger::default();
    let mut next_artifact_id = 0;
    let mut marked = 0_u32;
    for index in 0..rows {
        let page = terrain_page(index, EcsPageChannel::Surface);
        let mut commands = EcsSpatialCommandBuffer::new(FunRevision::INITIAL);
        let report = propagate_voxel_edit(
            voxel_edit(),
            EcsVoxelEditPropagationOptions::far_background(page, index + 1),
            &mut page_table,
            &mut dirty_ledger,
            &mut next_artifact_id,
            &mut commands,
        )
        .expect("dirty region batch propagation succeeds");
        marked = marked.saturating_add(report.dirty_regions_marked);
    }
    marked
}

fn bench_build_interest(c: &mut Criterion) {
    let mut group = c.benchmark_group("spatial/build_interest");
    group.sample_size(10);
    for cameras in [1_u16, 8, 64] {
        let snapshot = source_snapshot(cameras);
        let registry = EcsSpatialGridRegistry::default();
        let residency = EcsPageResidencyTable::default();
        group.bench_with_input(BenchmarkId::from_parameter(cameras), &cameras, |b, _| {
            b.iter(|| {
                build_interest(
                    black_box(&snapshot),
                    black_box(&registry),
                    black_box(&residency),
                )
                .expect("build interest succeeds")
                .interests
                .len()
            });
        });
    }
    group.finish();
}

fn bench_diff_requests(c: &mut Criterion) {
    let snapshot = source_snapshot(1);
    let interests = build_interest(
        &snapshot,
        &EcsSpatialGridRegistry::default(),
        &EcsPageResidencyTable::default(),
    )
    .expect("interest fixture builds");
    let mut group = c.benchmark_group("spatial/diff_requests");
    group.sample_size(10);
    for rows in [0_u32, 100_000, 1_000_000] {
        let residency = residency_table(rows);
        group.bench_with_input(BenchmarkId::from_parameter(rows), &rows, |b, _| {
            b.iter_batched(
                || EcsSpatialCommandBuffer::new(FunRevision::INITIAL),
                |mut commands| {
                    diff_requests(
                        black_box(&interests),
                        black_box(&residency),
                        &[],
                        &[],
                        &mut commands,
                    )
                    .expect("diff requests succeeds")
                    .requested
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_decode_pages(c: &mut Criterion) {
    let mut group = c.benchmark_group("spatial/decode_pages");
    group.sample_size(10);
    for rows in [1_000_u32, 32_000] {
        let acquired = source_acquire_queue(rows);
        group.bench_with_input(BenchmarkId::from_parameter(rows), &rows, |b, _| {
            b.iter_batched(
                EcsDecodedPageQueue::default,
                |mut output| {
                    decode_pages(black_box(&acquired), &[], 7, &mut output)
                        .expect("decode pages succeeds")
                        .decoded
                },
                BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

fn bench_build_artifacts(c: &mut Criterion) {
    let systems = [EcsDerivedArtifactBuildSystem::BuildTerrainSurfacePackets];
    let mut group = c.benchmark_group("spatial/build_artifacts");
    group.sample_size(10);
    for rows in [1_u32, 1_000, 32_000] {
        let decoded = decoded_pages(rows);
        group.bench_with_input(BenchmarkId::from_parameter(rows), &rows, |b, _| {
            b.iter_batched(
                || EcsSpatialCommandBuffer::new(FunRevision::INITIAL),
                |mut commands| {
                    let mut next_artifact_id = 0;
                    build_derived_artifacts(
                        black_box(&decoded.rows),
                        black_box(&systems),
                        &mut next_artifact_id,
                        &mut commands,
                    )
                    .expect("artifact build succeeds")
                    .artifacts_published
                },
                BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

fn bench_publish_handoffs(c: &mut Criterion) {
    let mut group = c.benchmark_group("spatial/publish_handoffs");
    group.sample_size(10);
    for consumer in [
        EcsBenchmarkConsumer::Renderer,
        EcsBenchmarkConsumer::Lux,
        EcsBenchmarkConsumer::AvisPhysics,
        EcsBenchmarkConsumer::Thunder,
    ] {
        let artifacts = artifact_rows(1_024, consumer);
        group.bench_with_input(
            BenchmarkId::new("consumer", consumer.label()),
            &consumer,
            |b, _| {
                b.iter_batched(
                    || EcsSpatialCommandBuffer::new(FunRevision::INITIAL),
                    |mut commands| match consumer {
                        EcsBenchmarkConsumer::Renderer => {
                            publish_renderer_handoffs(
                                black_box(&artifacts),
                                RendererVisibilityHint::VisibleNear,
                                &mut commands,
                            )
                            .expect("renderer handoffs publish")
                            .published
                        }
                        EcsBenchmarkConsumer::Lux => {
                            publish_lux_handoffs(
                                black_box(&artifacts),
                                EcsAabbF32::new([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
                                &mut commands,
                            )
                            .expect("lux handoffs publish")
                            .published
                        }
                        EcsBenchmarkConsumer::AvisPhysics => {
                            publish_physics_cooks(
                                black_box(&artifacts),
                                CollisionCookMode::NarrowBandSdf,
                                Some(FixedStepId::new(1)),
                                &mut commands,
                            )
                            .expect("physics cooks publish")
                            .published
                        }
                        EcsBenchmarkConsumer::Thunder => {
                            publish_thunder_handoffs(black_box(&artifacts))
                        }
                        EcsBenchmarkConsumer::None => 0,
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

fn bench_dirty_propagation(c: &mut Criterion) {
    let source_page = terrain_page(0, EcsPageChannel::Surface);
    let mut page_table = EcsPageResidencyTable::default();
    page_table
        .push(EcsPageResidencyRecord::new(source_page, priority(0), 1))
        .expect("page table admits dirty source page");

    c.bench_function("spatial/dirty_propagation/one_voxel_edit", |b| {
        b.iter_batched(
            || {
                (
                    page_table.clone(),
                    EcsDirtyRegionLedger::default(),
                    EcsSpatialCommandBuffer::new(FunRevision::INITIAL),
                )
            },
            |(mut table, mut ledger, mut commands)| {
                let mut next_artifact_id = 0;
                propagate_voxel_edit(
                    black_box(voxel_edit()),
                    EcsVoxelEditPropagationOptions::far_background(source_page, 1),
                    &mut table,
                    &mut ledger,
                    &mut next_artifact_id,
                    &mut commands,
                )
                .expect("voxel edit propagation succeeds")
                .artifacts_marked_dirty
            },
            BatchSize::SmallInput,
        );
    });

    let mut group = c.benchmark_group("spatial/dirty_propagation/dirty_region_batches");
    group.sample_size(10);
    for rows in [1_000_u32, 16_000] {
        let page_table = dirty_page_table(rows);
        group.bench_with_input(BenchmarkId::from_parameter(rows), &rows, |b, _| {
            b.iter_batched(
                || page_table.clone(),
                |table| propagate_dirty_region_batch(black_box(rows), table),
                BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_build_interest,
    bench_diff_requests,
    bench_decode_pages,
    bench_build_artifacts,
    bench_publish_handoffs,
    bench_dirty_propagation
);
criterion_main!(benches);
