use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use fun_ecs::{
    PROCEDURAL_TERRAIN_PROTOTYPE_BENCHMARKS, run_procedural_terrain_prototype_benchmark,
};

fn bench_procedural_terrain_prototype(c: &mut Criterion) {
    let mut group = c.benchmark_group("procedural_terrain/prototype");
    group.sample_size(10);
    for kind in PROCEDURAL_TERRAIN_PROTOTYPE_BENCHMARKS {
        group.bench_with_input(
            BenchmarkId::from_parameter(kind.label()),
            &kind,
            |b, kind| {
                b.iter(|| {
                    let report = run_procedural_terrain_prototype_benchmark(black_box(*kind))
                        .expect("procedural terrain prototype benchmark succeeds");
                    black_box((
                        report.generated_pages,
                        report.skipped_empty_pages,
                        report.skipped_uniform_pages,
                        report.renderer_handoffs,
                        report.frame_p95_ns,
                        report.mismatch_count,
                    ))
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_procedural_terrain_prototype);
criterion_main!(benches);
