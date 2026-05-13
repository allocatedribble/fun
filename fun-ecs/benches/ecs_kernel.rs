use bevy_ecs::prelude::{Component, World};
use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use fun_ecs::*;

const ENTITY_ROWS: u32 = 4_096;
const TABLE_ROWS: u32 = 16_384;

#[derive(Component)]
struct BenchPosition(u32);

#[derive(Component)]
struct BenchVelocity;

fn fun_entity(index: u64) -> FunEntity {
    FunEntity::new(index + 1, FunEntityGeneration::new(1))
}

fn terrain_page(index: u32, channel: EcsPageChannel) -> EcsSpatialPageKey {
    EcsSpatialPageKey::new(
        EcsSpatialDomainKind::Terrain,
        EcsSpatialGridId::new(1),
        0,
        (index % 1024) as i32,
        ((index / 1024) % 1024) as i32,
        0,
        channel,
    )
}

fn priority(index: u32) -> EcsStreamPriority {
    EcsStreamPriority::new(EcsStreamInterestKind::VisibleNear, (index % 8) as u16, 0).with_scores(
        (index % 65_535) as u16,
        512,
        128,
        64,
        32,
    )
}

fn residency_record(index: u32) -> EcsPageResidencyRecord {
    let mut record = EcsPageResidencyRecord::new(
        terrain_page(index, EcsPageChannel::Occupancy),
        priority(index),
        index + 1,
    );
    record.state = EcsPageResidencyState::FullyReady;
    record.last_used_frame = index;
    record
}

fn archetype_table(rows: u32) -> ArchetypeTable {
    let position = FunComponentId::new(10);
    let velocity = FunComponentId::new(11);
    let archetype =
        Archetype::new(FunArchetypeId::new(4), vec![position, velocity]).with_chunk_capacity(128);
    let mut table = ArchetypeTable::new(archetype);
    for index in 0..rows {
        table
            .insert_entity(fun_entity(u64::from(index)))
            .expect("archetype table admits bench entity");
    }
    table
}

fn sparse_pool(rows: u32) -> SparsePagedPool<u64> {
    let mut pool = SparsePagedPool::new(256);
    for index in 0..rows {
        pool.insert(fun_entity(u64::from(index)), u64::from(index))
            .expect("sparse pool admits bench component");
    }
    pool
}

type ResidencyDenseTable = DenseResourceTable<EcsSpatialPageId, EcsPageResidencyRecord>;

fn dense_residency_table(rows: u32) -> ResidencyDenseTable {
    let mut table = DenseResourceTable::with_capacity(
        FunEcsResourceKind::PageResidencyTable,
        FunResourceTableId::new(1),
        rows as usize + 1,
    );
    for index in 0..rows {
        table
            .push(
                EcsSpatialPageId::new(u64::from(index) + 1),
                residency_record(index),
            )
            .expect("dense resource table admits bench row");
    }
    table
}

fn artifact_command_buffer(rows: u32) -> EcsSpatialCommandBuffer {
    let mut commands = EcsSpatialCommandBuffer::new(FunRevision::INITIAL);
    for index in 0..rows {
        commands
            .push(EcsSpatialCommand::PublishArtifact(
                EcsDerivedArtifactRecord {
                    artifact_id: EcsDerivedArtifactId::new(u64::from(index) + 1),
                    source_page: terrain_page(index, EcsPageChannel::Surface),
                    kind: EcsDerivedArtifactKind::TerrainSurfacePackets,
                    source_epoch: 1,
                    source_digest: derived_artifact_source_digest(
                        terrain_page(index, EcsPageChannel::Surface),
                        1,
                        index + 1,
                    ),
                    artifact_epoch: index + 1,
                    state: EcsArtifactState::Ready,
                    requiredness: WorkRequiredness::Required,
                    consumer: EcsArtifactConsumer::Renderer,
                },
            ))
            .expect("command buffer admits artifact publish command");
    }
    commands
}

fn compile_synthetic_systems(count: u32) -> u32 {
    let sets = EcsSpatialScheduleSet::all();
    let mut rows = 0_u32;
    for index in 0..count {
        let set = sets[index as usize % sets.len()];
        let descriptor = set.system_descriptor();
        let scheduler = descriptor
            .to_scheduler_descriptor_for::<ProductRegistry>()
            .expect("system descriptor bridges to scheduler");
        rows = rows
            .saturating_add(scheduler.access.accesses.len() as u32)
            .saturating_add(scheduler.awaited_tokens.len() as u32)
            .saturating_add(scheduler.produced_tokens.len() as u32);
    }
    rows
}

fn bench_entity_kernel(c: &mut Criterion) {
    c.bench_function("ecs/entity/spawn_despawn", |b| {
        b.iter(|| {
            let mut world = World::new();
            let entities: Vec<_> = (0..ENTITY_ROWS)
                .map(|index| world.spawn((BenchPosition(index),)).id())
                .collect();
            let mut removed = 0_u32;
            for entity in entities {
                removed += u32::from(world.despawn(entity));
            }
            removed
        });
    });

    c.bench_function("ecs/entity/add_remove_component", |b| {
        b.iter(|| {
            let mut world = World::new();
            let entities: Vec<_> = (0..ENTITY_ROWS)
                .map(|index| world.spawn((BenchPosition(index),)).id())
                .collect();
            for entity in &entities {
                world.entity_mut(*entity).insert(BenchVelocity);
            }
            for entity in &entities {
                world.entity_mut(*entity).remove::<BenchVelocity>();
            }
            entities.len()
        });
    });

    c.bench_function("ecs/entity/query_dense_archetype", |b| {
        let table = archetype_table(ENTITY_ROWS);
        b.iter(|| {
            table
                .chunks
                .iter()
                .flat_map(|chunk| chunk.entities.iter())
                .fold(0_u64, |sum, entity| {
                    sum.wrapping_add(entity.scheduler_bits())
                })
        });
    });

    c.bench_function("ecs/entity/query_sparse_component", |b| {
        let pool = sparse_pool(ENTITY_ROWS);
        b.iter(|| {
            pool.iter().fold(0_u64, |sum, (entity, value)| {
                sum ^ entity.scheduler_bits() ^ *value
            })
        });
    });

    c.bench_function("ecs/entity/changed_filter", |b| {
        let base = archetype_table(ENTITY_ROWS);
        let component = FunComponentId::new(10);
        b.iter_batched(
            || base.clone(),
            |mut table| {
                for index in 0..ENTITY_ROWS {
                    table
                        .mark_component_dirty(fun_entity(u64::from(index)), component)
                        .expect("component dirty mark succeeds");
                }
                table
                    .chunks
                    .iter()
                    .filter(|chunk| !chunk.dirty_mask.is_empty())
                    .count()
            },
            BatchSize::LargeInput,
        );
    });

    c.bench_function("ecs/entity/command_apply", |b| {
        b.iter_batched(
            || {
                (
                    artifact_command_buffer(1_024),
                    EcsDerivedArtifactRegistry::default(),
                )
            },
            |(mut commands, mut registry)| {
                apply_artifact_commands(&mut commands, &mut registry)
                    .expect("artifact command apply succeeds")
                    .applied
            },
            BatchSize::SmallInput,
        );
    });

    c.bench_function("ecs/entity/bevy_changed_query", |b| {
        b.iter(|| {
            let mut world = World::new();
            for index in 0..ENTITY_ROWS {
                world.spawn((BenchPosition(index),));
            }
            let mut query = world.query::<&BenchPosition>();
            query.iter(&world).fold(0_u64, |sum, position| {
                sum.wrapping_add(u64::from(position.0))
            })
        });
    });
}

fn bench_resource_table_kernel(c: &mut Criterion) {
    let mut group = c.benchmark_group("ecs/resource_table");
    group.sample_size(10);

    group.bench_function("insert_rows", |b| {
        b.iter(|| dense_residency_table(black_box(TABLE_ROWS)).len());
    });

    let table = dense_residency_table(TABLE_ROWS);
    group.bench_function("scan_rows", |b| {
        b.iter(|| {
            table
                .iter()
                .filter(|(_key, row, _revision)| row.state.is_external_resident())
                .count()
        });
    });

    group.bench_function("chunk_rows", |b| {
        b.iter(|| table.chunk_by_range(0..512).rows.len());
    });

    group.bench_function("update_priorities", |b| {
        b.iter_batched(
            || table.clone(),
            |mut table| {
                table.update_rows(|key, row| {
                    row.priority = priority(key.get() as u32);
                });
                table.table_revision().get()
            },
            BatchSize::LargeInput,
        );
    });

    group.bench_function("update_epochs", |b| {
        b.iter_batched(
            || table.clone(),
            |mut table| {
                table.update_rows(|_key, row| {
                    row.dirty_epoch = row.dirty_epoch.saturating_add(1);
                    row.artifact_epoch = row.artifact_epoch.saturating_add(1);
                });
                table.table_revision().get()
            },
            BatchSize::LargeInput,
        );
    });

    group.bench_function("compact_evict", |b| {
        b.iter(|| {
            let mut retained = DenseResourceTable::with_capacity(
                FunEcsResourceKind::PageResidencyTable,
                FunResourceTableId::new(2),
                TABLE_ROWS as usize,
            );
            for (key, row, _revision) in table
                .iter()
                .filter(|(_key, row, _revision)| row.last_used_frame % 2 == 0)
            {
                retained
                    .push(key, *row)
                    .expect("retained resource table admits row");
            }
            retained.len()
        });
    });

    group.bench_function("digest", |b| {
        b.iter(|| table.digest().value);
    });

    group.finish();
}

fn bench_scheduler_kernel(c: &mut Criterion) {
    let mut group = c.benchmark_group("ecs/scheduler");
    group.sample_size(10);

    group.bench_function("compile_15_spatial_sets", |b| {
        b.iter(|| {
            EcsSpatialScheduleCompiler::compile(EcsSpatialScheduleBuildInput::default())
                .expect("spatial schedule compiles")
                .build_report
                .work_nodes
        });
    });

    for systems in [100_u32, 1_000] {
        group.bench_with_input(
            BenchmarkId::new("compile_systems", systems),
            &systems,
            |b, systems| {
                b.iter(|| compile_synthetic_systems(black_box(*systems)));
            },
        );
    }

    group.bench_function("compile_chunked_decode_artifact_graph", |b| {
        b.iter(|| {
            EcsSpatialScheduleCompiler::compile(
                EcsSpatialScheduleBuildInput::default()
                    .with_decode_chunk_count(ECS_SPATIAL_MAX_COMPILE_CHUNKS),
            )
            .expect("chunked spatial schedule compiles")
            .chunk_plan
            .chunk_nodes
        });
    });

    group.bench_function("run_deterministic_graph", |b| {
        b.iter_batched(
            FunWorld::default,
            |mut world| {
                world
                    .run_spatial_frame_deterministic()
                    .expect("deterministic frame runs")
                    .scheduler_report
                    .metrics
                    .node_count
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("run_deterministic_parallel_graph", |b| {
        b.iter_batched(
            FunWorld::default,
            |mut world| {
                world
                    .run_spatial_frame_parallel()
                    .expect("deterministic parallel frame runs")
                    .scheduler_report
                    .metrics
                    .node_count
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("apply_barriers", |b| {
        b.iter(|| {
            EcsSpatialScheduleCompiler::compile(EcsSpatialScheduleBuildInput::default())
                .expect("spatial schedule compiles")
                .barrier_plan
                .barrier_nodes
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_entity_kernel,
    bench_resource_table_kernel,
    bench_scheduler_kernel
);
criterion_main!(benches);
