use std::time::Instant;

use fun_scheduler_types::EcsEntityId;

use crate::{
    EcsArtifactConsumer, EcsCameraTransformSample, EcsDecodedPageQueue, EcsDerivedArtifactKind,
    EcsDerivedArtifactRecord, EcsPageResidencyRecord, EcsPageResidencyState, EcsPageResidencyTable,
    EcsProceduralTerrainSource, EcsProceduralWorldManifest, EcsSourceAcquireQueue,
    EcsSpatialCommand, EcsSpatialCommandBuffer, EcsSpatialDomainKind, EcsSpatialGridDesc,
    EcsSpatialGridId, EcsSpatialGridRegistry, EcsSpatialPageKey, EcsSpatialSourceId,
    EcsSpatialValidationError, EcsStreamCameraId, EcsStreamInterestKind, EcsStreamInterestTable,
    EcsStreamPriority, EcsStreamSourceDescriptor, EcsStreamSourceId, EcsStreamWaveLedger,
    EcsStreamingSourceSnapshot, EcsViewFrustum, IVec3, NetworkPlayerId,
    ProceduralGeneratedPageClass, ProceduralGenerationScratch, ProceduralPageDigest,
    ProceduralPageDigestSample, ProceduralTerrainArtifactFlags, RendererVisibilityHint,
    SpatialStreamCamera, StreamCameraRole, StreamWaveReason, acquire_sources, build_interest,
    build_prototype_terrain_artifacts, decode_pages_with_procedural_manifest_and_scratch,
    diff_requests, generate_procedural_page_digest, plan_stream_wave, publish_renderer_handoffs,
    sense_sources, terrain_page_for_world_ft,
};

pub const PROCEDURAL_TERRAIN_BENCHMARK_SEED: u64 = 0x4d59_5df4_d0f3_3173;
pub const PROCEDURAL_TERRAIN_COLD_SPAWN_CAMERA_FT: IVec3 = IVec3::new(0, 96, 0);
pub const PROCEDURAL_TERRAIN_COLD_SPAWN_REQUIRED_SHELLS: u16 = 2;
pub const PROCEDURAL_TERRAIN_COLD_SPAWN_DESIRED_SHELLS: u16 = 5;
pub const PROCEDURAL_TERRAIN_TREADMILL_FRAMES: u32 = 12;
pub const PROCEDURAL_TERRAIN_TREADMILL_STEP_FT: i32 = 32;
pub const PROCEDURAL_TERRAIN_TREADMILL_REQUIRED_SHELLS: u16 = 1;
pub const PROCEDURAL_TERRAIN_TREADMILL_DESIRED_SHELLS: u16 = 3;
pub const PROCEDURAL_TERRAIN_TELEPORT_DESTINATION_FT: IVec3 = IVec3::new(16_384, 96, -8_192);
pub const PROCEDURAL_TERRAIN_NEGATIVE_CAMERA_FT: IVec3 = IVec3::new(-96, 96, -96);

const BENCHMARK_FRAME_RATE_HZ: u64 = 60;
const BENCHMARK_Q16_ONE: u64 = 65_536;
const TREADMILL_MAX_GENERATED_PER_FRAME: usize = 48;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ProceduralTerrainPrototypeBenchmarkKind {
    #[default]
    ColdSpawn = 0,
    StreamingTreadmill = 1,
    Teleport = 2,
    MultiplayerDigest = 3,
    NegativeCoordinateWorld = 4,
}

impl ProceduralTerrainPrototypeBenchmarkKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ColdSpawn => "cold_spawn",
            Self::StreamingTreadmill => "streaming_treadmill",
            Self::Teleport => "teleport",
            Self::MultiplayerDigest => "multiplayer_digest",
            Self::NegativeCoordinateWorld => "negative_coordinate_world",
        }
    }
}

pub const PROCEDURAL_TERRAIN_PROTOTYPE_BENCHMARKS: [ProceduralTerrainPrototypeBenchmarkKind; 5] = [
    ProceduralTerrainPrototypeBenchmarkKind::ColdSpawn,
    ProceduralTerrainPrototypeBenchmarkKind::StreamingTreadmill,
    ProceduralTerrainPrototypeBenchmarkKind::Teleport,
    ProceduralTerrainPrototypeBenchmarkKind::MultiplayerDigest,
    ProceduralTerrainPrototypeBenchmarkKind::NegativeCoordinateWorld,
];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProceduralTerrainPrototypeBenchmarkReport {
    pub kind: ProceduralTerrainPrototypeBenchmarkKind,
    pub seed: u64,
    pub camera_start_ft: IVec3,
    pub required_shells: u16,
    pub desired_shells: u16,
    pub frames: u32,
    pub requested_pages: u32,
    pub generated_pages: u32,
    pub skipped_empty_pages: u32,
    pub skipped_uniform_pages: u32,
    pub surface_pages: u32,
    pub coarse_pages: u32,
    pub artifact_ready_count: u32,
    pub renderer_handoffs: u32,
    pub time_to_first_coarse_ns: u64,
    pub time_to_first_surface_ns: u64,
    pub frame_p95_ns: u64,
    pub pages_generated_per_sec_q16: u64,
    pub evictions_per_sec_q16: u64,
    pub upload_handoff_rate_q16: u64,
    pub scheduler_queue_age_frames: u32,
    pub worst_frame_stall_ns: u64,
    pub stale_request_cancellations: u32,
    pub new_shell0_generation_latency_ns: u64,
    pub old_wave_retired: bool,
    pub digest_stability_matches: bool,
    pub digest_matches: u32,
    pub mismatch_count: u32,
    pub generation_order_independent: bool,
    pub latency_to_local_terrain_ns: u64,
    pub stable_page_keys: bool,
    pub deterministic_output: bool,
    pub floor_division_bug_count: u32,
}

impl ProceduralTerrainPrototypeBenchmarkReport {
    #[must_use]
    fn from_pipeline(
        kind: ProceduralTerrainPrototypeBenchmarkKind,
        manifest: EcsProceduralWorldManifest,
        camera_start_ft: IVec3,
        required_shells: u16,
        desired_shells: u16,
        pipeline: &GenerationPipelineReport,
    ) -> Self {
        Self {
            kind,
            seed: manifest.world_seed,
            camera_start_ft,
            required_shells,
            desired_shells,
            frames: pipeline.frames,
            requested_pages: pipeline.requested_pages,
            generated_pages: pipeline.generated_pages,
            skipped_empty_pages: pipeline.skipped_empty_pages,
            skipped_uniform_pages: pipeline.skipped_uniform_pages,
            surface_pages: pipeline.surface_pages,
            coarse_pages: pipeline.coarse_pages,
            artifact_ready_count: pipeline.artifact_ready_count,
            renderer_handoffs: pipeline.renderer_handoffs,
            time_to_first_coarse_ns: pipeline.time_to_first_coarse_ns,
            time_to_first_surface_ns: pipeline.time_to_first_surface_ns,
            frame_p95_ns: pipeline.frame_p95_ns,
            worst_frame_stall_ns: pipeline.worst_frame_stall_ns,
            ..Self::default()
        }
    }
}

pub fn run_procedural_terrain_prototype_benchmark(
    kind: ProceduralTerrainPrototypeBenchmarkKind,
) -> Result<ProceduralTerrainPrototypeBenchmarkReport, EcsSpatialValidationError> {
    match kind {
        ProceduralTerrainPrototypeBenchmarkKind::ColdSpawn => run_cold_spawn_benchmark(),
        ProceduralTerrainPrototypeBenchmarkKind::StreamingTreadmill => {
            run_streaming_treadmill_benchmark()
        }
        ProceduralTerrainPrototypeBenchmarkKind::Teleport => run_teleport_benchmark(),
        ProceduralTerrainPrototypeBenchmarkKind::MultiplayerDigest => {
            run_multiplayer_digest_benchmark()
        }
        ProceduralTerrainPrototypeBenchmarkKind::NegativeCoordinateWorld => {
            run_negative_coordinate_world_benchmark()
        }
    }
}

fn run_cold_spawn_benchmark()
-> Result<ProceduralTerrainPrototypeBenchmarkReport, EcsSpatialValidationError> {
    let manifest = benchmark_manifest();
    let pages = pages_for_camera(
        CameraBenchmarkInput::new(
            manifest,
            0,
            PROCEDURAL_TERRAIN_COLD_SPAWN_CAMERA_FT,
            IVec3::zero(),
            PROCEDURAL_TERRAIN_COLD_SPAWN_REQUIRED_SHELLS,
            PROCEDURAL_TERRAIN_COLD_SPAWN_DESIRED_SHELLS,
            false,
        ),
        &EcsPageResidencyTable::default(),
    )?;
    let pipeline = run_generation_pipeline(manifest, &pages, true)?;
    Ok(ProceduralTerrainPrototypeBenchmarkReport::from_pipeline(
        ProceduralTerrainPrototypeBenchmarkKind::ColdSpawn,
        manifest,
        PROCEDURAL_TERRAIN_COLD_SPAWN_CAMERA_FT,
        PROCEDURAL_TERRAIN_COLD_SPAWN_REQUIRED_SHELLS,
        PROCEDURAL_TERRAIN_COLD_SPAWN_DESIRED_SHELLS,
        &pipeline,
    ))
}

fn run_streaming_treadmill_benchmark()
-> Result<ProceduralTerrainPrototypeBenchmarkReport, EcsSpatialValidationError> {
    let manifest = benchmark_manifest();
    let mut residency = EcsPageResidencyTable::default();
    let mut requested_pages = Vec::new();
    let mut pending_pages = Vec::new();
    let mut resident_pages = Vec::new();
    let mut total = GenerationPipelineReport::default();
    let mut request_commands = 0_u32;
    let mut evictions = 0_u32;
    let mut max_queue_age = 0_u32;

    for frame in 0..PROCEDURAL_TERRAIN_TREADMILL_FRAMES {
        let origin = IVec3::new(
            0,
            PROCEDURAL_TERRAIN_COLD_SPAWN_CAMERA_FT.y,
            (frame as i32).saturating_mul(PROCEDURAL_TERRAIN_TREADMILL_STEP_FT),
        );
        let (interests, desired_pages) = interests_for_camera(
            CameraBenchmarkInput::new(
                manifest,
                u64::from(frame),
                origin,
                IVec3::new(0, 0, PROCEDURAL_TERRAIN_TREADMILL_STEP_FT),
                PROCEDURAL_TERRAIN_TREADMILL_REQUIRED_SHELLS,
                PROCEDURAL_TERRAIN_TREADMILL_DESIRED_SHELLS,
                false,
            ),
            &residency,
        )?;
        let mut commands = EcsSpatialCommandBuffer::default();
        diff_requests(&interests, &residency, &requested_pages, &[], &mut commands)?;
        apply_stream_commands_to_pending(
            frame,
            &commands,
            &mut requested_pages,
            &mut pending_pages,
            &mut request_commands,
        );
        evictions = evictions.saturating_add(evict_pages_outside_shell(
            &mut resident_pages,
            &desired_pages,
        ));
        max_queue_age = max_queue_age.max(queue_age_frames(frame, &pending_pages));

        pending_pages.sort_by_key(|pending| (pending.requested_frame, page_sort_key(pending.page)));
        let generation_limit = usize::from(
            manifest
                .generator_desc()
                .budgets
                .max_pages_generated_per_frame,
        )
        .min(TREADMILL_MAX_GENERATED_PER_FRAME)
        .min(pending_pages.len());
        let pages_to_generate: Vec<_> = pending_pages
            .drain(..generation_limit)
            .map(|pending| pending.page)
            .collect();
        if pages_to_generate.is_empty() {
            continue;
        }

        let generated = run_generation_pipeline(manifest, &pages_to_generate, true)?;
        for (page, _digest) in &generated.digests {
            remove_page(&mut requested_pages, *page);
            if !resident_pages.contains(page) {
                resident_pages.push(*page);
            }
            mark_page_ready(&mut residency, *page, manifest, frame)?;
        }
        total.merge(&generated);
    }

    let mut report = ProceduralTerrainPrototypeBenchmarkReport::from_pipeline(
        ProceduralTerrainPrototypeBenchmarkKind::StreamingTreadmill,
        manifest,
        PROCEDURAL_TERRAIN_COLD_SPAWN_CAMERA_FT,
        PROCEDURAL_TERRAIN_TREADMILL_REQUIRED_SHELLS,
        PROCEDURAL_TERRAIN_TREADMILL_DESIRED_SHELLS,
        &total,
    );
    report.frames = PROCEDURAL_TERRAIN_TREADMILL_FRAMES;
    report.requested_pages = request_commands;
    report.pages_generated_per_sec_q16 =
        per_second_q16(u64::from(report.generated_pages), report.frames);
    report.evictions_per_sec_q16 = per_second_q16(u64::from(evictions), report.frames);
    report.upload_handoff_rate_q16 =
        per_second_q16(u64::from(report.renderer_handoffs), report.frames);
    report.scheduler_queue_age_frames = max_queue_age;
    Ok(report)
}

fn run_teleport_benchmark()
-> Result<ProceduralTerrainPrototypeBenchmarkReport, EcsSpatialValidationError> {
    let manifest = benchmark_manifest();
    let old_pages = pages_for_camera(
        CameraBenchmarkInput::new(
            manifest,
            0,
            PROCEDURAL_TERRAIN_COLD_SPAWN_CAMERA_FT,
            IVec3::zero(),
            PROCEDURAL_TERRAIN_COLD_SPAWN_REQUIRED_SHELLS,
            PROCEDURAL_TERRAIN_COLD_SPAWN_DESIRED_SHELLS,
            false,
        ),
        &EcsPageResidencyTable::default(),
    )?;
    let (new_interests, _new_pages) = interests_for_camera(
        CameraBenchmarkInput::new(
            manifest,
            1,
            PROCEDURAL_TERRAIN_TELEPORT_DESTINATION_FT,
            IVec3::zero(),
            PROCEDURAL_TERRAIN_COLD_SPAWN_REQUIRED_SHELLS,
            PROCEDURAL_TERRAIN_COLD_SPAWN_DESIRED_SHELLS,
            true,
        ),
        &EcsPageResidencyTable::default(),
    )?;
    let mut diff_commands = EcsSpatialCommandBuffer::default();
    let diff = diff_requests(
        &new_interests,
        &EcsPageResidencyTable::default(),
        &old_pages,
        &[],
        &mut diff_commands,
    )?;

    let mut wave_ledger = EcsStreamWaveLedger::default();
    let old_snapshot = snapshot_for_camera(CameraBenchmarkInput::new(
        manifest,
        0,
        PROCEDURAL_TERRAIN_COLD_SPAWN_CAMERA_FT,
        IVec3::zero(),
        PROCEDURAL_TERRAIN_COLD_SPAWN_REQUIRED_SHELLS,
        PROCEDURAL_TERRAIN_COLD_SPAWN_DESIRED_SHELLS,
        false,
    ));
    let new_snapshot = snapshot_for_camera(CameraBenchmarkInput::new(
        manifest,
        1,
        PROCEDURAL_TERRAIN_TELEPORT_DESTINATION_FT,
        IVec3::zero(),
        PROCEDURAL_TERRAIN_COLD_SPAWN_REQUIRED_SHELLS,
        PROCEDURAL_TERRAIN_COLD_SPAWN_DESIRED_SHELLS,
        true,
    ));
    let old_wave = plan_stream_wave(
        &old_snapshot,
        &mut wave_ledger,
        0,
        PROCEDURAL_TERRAIN_COLD_SPAWN_DESIRED_SHELLS,
        StreamWaveReason::ColdStart,
    );
    let new_wave = plan_stream_wave(
        &new_snapshot,
        &mut wave_ledger,
        0,
        PROCEDURAL_TERRAIN_COLD_SPAWN_DESIRED_SHELLS,
        StreamWaveReason::CameraMoved,
    );

    let shell0 = terrain_page_for_world_ft(
        manifest.terrain_grid,
        PROCEDURAL_TERRAIN_TELEPORT_DESTINATION_FT,
        0,
    );
    let started = Instant::now();
    let shell0_pipeline = run_generation_pipeline(manifest, &[shell0], true)?;
    let latency_ns = elapsed_ns(started);
    let digest_a = generate_procedural_page_digest(manifest, shell0)?;
    let digest_b = generate_procedural_page_digest(manifest, shell0)?;

    let mut report = ProceduralTerrainPrototypeBenchmarkReport::from_pipeline(
        ProceduralTerrainPrototypeBenchmarkKind::Teleport,
        manifest,
        PROCEDURAL_TERRAIN_TELEPORT_DESTINATION_FT,
        PROCEDURAL_TERRAIN_COLD_SPAWN_REQUIRED_SHELLS,
        PROCEDURAL_TERRAIN_COLD_SPAWN_DESIRED_SHELLS,
        &shell0_pipeline,
    );
    report.requested_pages = diff.requested;
    report.stale_request_cancellations = diff.cancelled;
    report.new_shell0_generation_latency_ns = latency_ns;
    report.old_wave_retired = old_wave.zip(new_wave).is_some_and(|(old, new)| {
        old.wave_id != new.wave_id && new.reason == StreamWaveReason::TeleportOrCut
    });
    report.digest_stability_matches = digest_a == digest_b;
    Ok(report)
}

fn run_multiplayer_digest_benchmark()
-> Result<ProceduralTerrainPrototypeBenchmarkReport, EcsSpatialValidationError> {
    let manifest = benchmark_manifest();
    let pages = pages_for_camera(
        CameraBenchmarkInput::new(
            manifest,
            0,
            PROCEDURAL_TERRAIN_COLD_SPAWN_CAMERA_FT,
            IVec3::zero(),
            PROCEDURAL_TERRAIN_COLD_SPAWN_REQUIRED_SHELLS,
            PROCEDURAL_TERRAIN_COLD_SPAWN_REQUIRED_SHELLS,
            false,
        ),
        &EcsPageResidencyTable::default(),
    )?;
    let server_digests = digest_pages_in_order(manifest, &pages)?;

    let client_started = Instant::now();
    let client_one_digests = digest_pages_in_order(manifest, &pages)?;
    let client_latency_ns = elapsed_ns(client_started);

    let mut reversed_pages = pages.clone();
    reversed_pages.reverse();
    let client_two_digests = digest_pages_in_order(manifest, &reversed_pages)?;

    let mut digest_matches = 0_u32;
    let mut mismatch_count = 0_u32;
    for player_id in [NetworkPlayerId::new(1), NetworkPlayerId::new(2)] {
        let client_digests = if player_id.get() == 1 {
            &client_one_digests
        } else {
            &client_two_digests
        };
        for (server_page, server_digest) in &server_digests {
            let client_digest = find_digest_for_page(client_digests, *server_page);
            if let Some(client_digest) = client_digest {
                let sample =
                    ProceduralPageDigestSample::new(player_id, *server_page, *server_digest, 0);
                let probe = sample.into_probe(client_digest);
                if probe.validate().is_ok() {
                    digest_matches = digest_matches.saturating_add(1);
                } else {
                    mismatch_count = mismatch_count.saturating_add(1);
                }
            } else {
                mismatch_count = mismatch_count.saturating_add(1);
            }
        }
    }

    let mut server_sorted = server_digests;
    let mut client_two_sorted = client_two_digests;
    sort_digests(&mut server_sorted);
    sort_digests(&mut client_two_sorted);

    Ok(ProceduralTerrainPrototypeBenchmarkReport {
        kind: ProceduralTerrainPrototypeBenchmarkKind::MultiplayerDigest,
        seed: manifest.world_seed,
        camera_start_ft: PROCEDURAL_TERRAIN_COLD_SPAWN_CAMERA_FT,
        required_shells: PROCEDURAL_TERRAIN_COLD_SPAWN_REQUIRED_SHELLS,
        desired_shells: PROCEDURAL_TERRAIN_COLD_SPAWN_REQUIRED_SHELLS,
        frames: 1,
        requested_pages: pages.len() as u32,
        generated_pages: pages.len() as u32,
        digest_matches,
        mismatch_count,
        generation_order_independent: server_sorted == client_two_sorted,
        latency_to_local_terrain_ns: client_latency_ns,
        ..ProceduralTerrainPrototypeBenchmarkReport::default()
    })
}

fn run_negative_coordinate_world_benchmark()
-> Result<ProceduralTerrainPrototypeBenchmarkReport, EcsSpatialValidationError> {
    let manifest = benchmark_manifest();
    let pages_a = pages_for_camera(
        CameraBenchmarkInput::new(
            manifest,
            0,
            PROCEDURAL_TERRAIN_NEGATIVE_CAMERA_FT,
            IVec3::zero(),
            PROCEDURAL_TERRAIN_COLD_SPAWN_REQUIRED_SHELLS,
            PROCEDURAL_TERRAIN_TREADMILL_DESIRED_SHELLS,
            false,
        ),
        &EcsPageResidencyTable::default(),
    )?;
    let pages_b = pages_for_camera(
        CameraBenchmarkInput::new(
            manifest,
            0,
            PROCEDURAL_TERRAIN_NEGATIVE_CAMERA_FT,
            IVec3::zero(),
            PROCEDURAL_TERRAIN_COLD_SPAWN_REQUIRED_SHELLS,
            PROCEDURAL_TERRAIN_TREADMILL_DESIRED_SHELLS,
            false,
        ),
        &EcsPageResidencyTable::default(),
    )?;
    let pipeline = run_generation_pipeline(manifest, &pages_a, false)?;
    let mut generated_digests = pipeline.digests.clone();
    let mut repeated_digests = digest_pages_in_order(manifest, &pages_b)?;
    sort_digests(&mut generated_digests);
    sort_digests(&mut repeated_digests);

    let mut report = ProceduralTerrainPrototypeBenchmarkReport::from_pipeline(
        ProceduralTerrainPrototypeBenchmarkKind::NegativeCoordinateWorld,
        manifest,
        PROCEDURAL_TERRAIN_NEGATIVE_CAMERA_FT,
        PROCEDURAL_TERRAIN_COLD_SPAWN_REQUIRED_SHELLS,
        PROCEDURAL_TERRAIN_TREADMILL_DESIRED_SHELLS,
        &pipeline,
    );
    report.stable_page_keys = pages_a == pages_b;
    report.deterministic_output = generated_digests == repeated_digests;
    report.floor_division_bug_count = floor_division_bug_count(manifest.terrain_grid);
    Ok(report)
}

#[derive(Debug, Default, Clone)]
struct GenerationPipelineReport {
    requested_pages: u32,
    generated_pages: u32,
    skipped_empty_pages: u32,
    skipped_uniform_pages: u32,
    surface_pages: u32,
    coarse_pages: u32,
    artifact_ready_count: u32,
    renderer_handoffs: u32,
    time_to_first_coarse_ns: u64,
    time_to_first_surface_ns: u64,
    frame_p95_ns: u64,
    worst_frame_stall_ns: u64,
    frames: u32,
    digests: Vec<(EcsSpatialPageKey, ProceduralPageDigest)>,
}

impl GenerationPipelineReport {
    fn merge(&mut self, other: &Self) {
        self.requested_pages = self.requested_pages.saturating_add(other.requested_pages);
        self.generated_pages = self.generated_pages.saturating_add(other.generated_pages);
        self.skipped_empty_pages = self
            .skipped_empty_pages
            .saturating_add(other.skipped_empty_pages);
        self.skipped_uniform_pages = self
            .skipped_uniform_pages
            .saturating_add(other.skipped_uniform_pages);
        self.surface_pages = self.surface_pages.saturating_add(other.surface_pages);
        self.coarse_pages = self.coarse_pages.saturating_add(other.coarse_pages);
        self.artifact_ready_count = self
            .artifact_ready_count
            .saturating_add(other.artifact_ready_count);
        self.renderer_handoffs = self
            .renderer_handoffs
            .saturating_add(other.renderer_handoffs);
        self.time_to_first_coarse_ns =
            first_nonzero(self.time_to_first_coarse_ns, other.time_to_first_coarse_ns);
        self.time_to_first_surface_ns = first_nonzero(
            self.time_to_first_surface_ns,
            other.time_to_first_surface_ns,
        );
        self.frame_p95_ns = self.frame_p95_ns.max(other.frame_p95_ns);
        self.worst_frame_stall_ns = self.worst_frame_stall_ns.max(other.worst_frame_stall_ns);
        self.frames = self.frames.saturating_add(other.frames);
        self.digests.extend(other.digests.iter().copied());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingPage {
    page: EcsSpatialPageKey,
    requested_frame: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CameraBenchmarkInput {
    manifest: EcsProceduralWorldManifest,
    frame: u64,
    origin_world_ft: IVec3,
    velocity_world_ft_s: IVec3,
    required_shells: u16,
    desired_shells: u16,
    cut_or_teleport: bool,
}

impl CameraBenchmarkInput {
    #[must_use]
    pub const fn new(
        manifest: EcsProceduralWorldManifest,
        frame: u64,
        origin_world_ft: IVec3,
        velocity_world_ft_s: IVec3,
        required_shells: u16,
        desired_shells: u16,
        cut_or_teleport: bool,
    ) -> Self {
        Self {
            manifest,
            frame,
            origin_world_ft,
            velocity_world_ft_s,
            required_shells,
            desired_shells,
            cut_or_teleport,
        }
    }
}

fn run_generation_pipeline(
    manifest: EcsProceduralWorldManifest,
    pages: &[EcsSpatialPageKey],
    build_renderer_artifacts: bool,
) -> Result<GenerationPipelineReport, EcsSpatialValidationError> {
    manifest.validate()?;
    let source = EcsProceduralTerrainSource::new(EcsSpatialSourceId::new(1), manifest);
    let batch_size = usize::from(
        manifest
            .generator_desc()
            .budgets
            .max_pages_generated_per_frame,
    )
    .max(1);
    let started = Instant::now();
    let mut scratch = ProceduralGenerationScratch::default();
    let mut next_artifact_id = 0_u64;
    let mut frame_durations = Vec::new();
    let mut report = GenerationPipelineReport {
        requested_pages: pages.len() as u32,
        ..GenerationPipelineReport::default()
    };

    for chunk in pages.chunks(batch_size) {
        let frame_started = Instant::now();
        let mut acquired = EcsSourceAcquireQueue::default();
        acquire_sources(&source, chunk, manifest.region_edge_pages, &mut acquired)?;
        let mut decoded = EcsDecodedPageQueue::default();
        let decode_report = decode_pages_with_procedural_manifest_and_scratch(
            &acquired,
            &[],
            report.frames.saturating_add(1),
            manifest,
            &mut scratch,
            &mut decoded,
        )?;
        report.generated_pages = report
            .generated_pages
            .saturating_add(decode_report.generated_pages);

        for page in &decoded.rows {
            if let Some(brick) = &page.voxel_brick {
                count_generated_class(&mut report, brick.generated_class);
                report.digests.push((
                    page.key,
                    ProceduralPageDigest::from_decoded_page(manifest, page)?,
                ));
            }
        }

        if build_renderer_artifacts {
            let mut artifact_commands = EcsSpatialCommandBuffer::default();
            build_prototype_terrain_artifacts(
                &decoded.rows,
                Some(EcsArtifactConsumer::Renderer),
                ProceduralTerrainArtifactFlags::FIRST_VISUAL,
                &mut next_artifact_id,
                &mut artifact_commands,
            )?;
            let artifact_records = artifact_records_from_commands(&artifact_commands);
            report.artifact_ready_count = report
                .artifact_ready_count
                .saturating_add(artifact_records.len() as u32);
            for artifact in &artifact_records {
                if artifact.kind == EcsDerivedArtifactKind::TerrainCoarseProxy {
                    report.time_to_first_coarse_ns =
                        first_nonzero(report.time_to_first_coarse_ns, elapsed_ns(started));
                }
                if artifact.kind == EcsDerivedArtifactKind::TerrainSurfacePackets {
                    report.time_to_first_surface_ns =
                        first_nonzero(report.time_to_first_surface_ns, elapsed_ns(started));
                }
            }

            let mut handoff_commands = EcsSpatialCommandBuffer::default();
            let handoffs = publish_renderer_handoffs(
                &artifact_records,
                RendererVisibilityHint::VisibleNear,
                &mut handoff_commands,
            )?;
            report.renderer_handoffs = report.renderer_handoffs.saturating_add(handoffs.published);
        }

        let frame_ns = elapsed_ns(frame_started);
        report.worst_frame_stall_ns = report.worst_frame_stall_ns.max(frame_ns);
        frame_durations.push(frame_ns);
        report.frames = report.frames.saturating_add(1);
    }
    report.frame_p95_ns = percentile_95(&mut frame_durations);
    Ok(report)
}

fn count_generated_class(
    report: &mut GenerationPipelineReport,
    class: ProceduralGeneratedPageClass,
) {
    match class {
        ProceduralGeneratedPageClass::EmptyAir => {
            report.skipped_empty_pages = report.skipped_empty_pages.saturating_add(1);
        }
        ProceduralGeneratedPageClass::UniformSolid => {
            report.skipped_uniform_pages = report.skipped_uniform_pages.saturating_add(1);
        }
        ProceduralGeneratedPageClass::SurfaceMixed
        | ProceduralGeneratedPageClass::MostlyAirWithFeatures => {
            report.surface_pages = report.surface_pages.saturating_add(1);
            report.coarse_pages = report.coarse_pages.saturating_add(1);
        }
        ProceduralGeneratedPageClass::MostlySolid => {
            report.coarse_pages = report.coarse_pages.saturating_add(1);
        }
        ProceduralGeneratedPageClass::DebugOnly => {}
    }
}

fn artifact_records_from_commands(
    commands: &EcsSpatialCommandBuffer,
) -> Vec<EcsDerivedArtifactRecord> {
    commands
        .commands
        .iter()
        .filter_map(|command| match command {
            EcsSpatialCommand::PublishArtifact(artifact) => Some(*artifact),
            _ => None,
        })
        .collect()
}

fn interests_for_camera(
    input: CameraBenchmarkInput,
    residency: &EcsPageResidencyTable,
) -> Result<(EcsStreamInterestTable, Vec<EcsSpatialPageKey>), EcsSpatialValidationError> {
    let snapshot = snapshot_for_camera(input);
    let registry = terrain_grid_registry(input.manifest.terrain_grid)?;
    let interests = build_interest(&snapshot, &registry, residency)?;
    let pages = interests.desired_pages();
    Ok((interests, pages))
}

fn pages_for_camera(
    input: CameraBenchmarkInput,
    residency: &EcsPageResidencyTable,
) -> Result<Vec<EcsSpatialPageKey>, EcsSpatialValidationError> {
    let (_interests, pages) = interests_for_camera(input, residency)?;
    Ok(pages)
}

fn snapshot_for_camera(input: CameraBenchmarkInput) -> EcsStreamingSourceSnapshot {
    let camera = SpatialStreamCamera {
        role: StreamCameraRole::MainView,
        enabled: true,
        priority: 255,
        required_shells: input.required_shells,
        desired_shells: input.desired_shells,
        velocity_lookahead_s: 0.5,
    };
    let transform = EcsCameraTransformSample {
        camera_entity: EcsEntityId::new(1),
        camera_id: EcsStreamCameraId::new(1),
        origin_world_ft: input.origin_world_ft,
        velocity_world_ft_s: input.velocity_world_ft_s,
        view_frustum: EcsViewFrustum::default(),
        cut_or_teleport: input.cut_or_teleport,
    };
    let source = EcsStreamSourceDescriptor {
        source: EcsStreamSourceId::new(1),
        domain: EcsSpatialDomainKind::Terrain,
        grid_id: input.manifest.terrain_grid,
        priority: 255,
    };
    sense_sources(input.frame, &[(camera, transform)], &[source])
}

fn terrain_grid_registry(
    grid_id: EcsSpatialGridId,
) -> Result<EcsSpatialGridRegistry, EcsSpatialValidationError> {
    let mut registry = EcsSpatialGridRegistry::default();
    registry.push(grid_id, EcsSpatialGridDesc::terrain_foot_default())?;
    Ok(registry)
}

fn benchmark_manifest() -> EcsProceduralWorldManifest {
    EcsProceduralWorldManifest {
        world_seed: PROCEDURAL_TERRAIN_BENCHMARK_SEED,
        ..EcsProceduralWorldManifest::BEDROCK_QUARRY
    }
}

fn apply_stream_commands_to_pending(
    frame: u32,
    commands: &EcsSpatialCommandBuffer,
    requested_pages: &mut Vec<EcsSpatialPageKey>,
    pending_pages: &mut Vec<PendingPage>,
    request_commands: &mut u32,
) {
    for command in &commands.commands {
        match *command {
            EcsSpatialCommand::RequestPage(page) => {
                if !requested_pages.contains(&page) {
                    requested_pages.push(page);
                    *request_commands = request_commands.saturating_add(1);
                }
                if !pending_pages.iter().any(|pending| pending.page == page) {
                    pending_pages.push(PendingPage {
                        page,
                        requested_frame: frame,
                    });
                }
            }
            EcsSpatialCommand::CancelPage(page) => {
                remove_page(requested_pages, page);
                pending_pages.retain(|pending| pending.page != page);
            }
            EcsSpatialCommand::PinPage(_)
            | EcsSpatialCommand::UnpinPage(_)
            | EcsSpatialCommand::PublishArtifact(_)
            | EcsSpatialCommand::RetireArtifact(_)
            | EcsSpatialCommand::MarkDirty(_)
            | EcsSpatialCommand::ApplyVoxelEdit(_)
            | EcsSpatialCommand::PublishRendererHandoff(_)
            | EcsSpatialCommand::PublishLuxHandoff(_)
            | EcsSpatialCommand::PublishPhysicsCook(_) => {}
        }
    }
}

fn evict_pages_outside_shell(
    resident_pages: &mut Vec<EcsSpatialPageKey>,
    desired_pages: &[EcsSpatialPageKey],
) -> u32 {
    let before = resident_pages.len();
    resident_pages.retain(|page| desired_pages.contains(page));
    (before.saturating_sub(resident_pages.len())).min(u32::MAX as usize) as u32
}

fn mark_page_ready(
    residency: &mut EcsPageResidencyTable,
    page: EcsSpatialPageKey,
    manifest: EcsProceduralWorldManifest,
    frame: u32,
) -> Result<(), EcsSpatialValidationError> {
    if let Some(record) = residency.get_mut_by_key(page) {
        record.state = EcsPageResidencyState::FullyReady;
        record.last_used_frame = frame;
        return Ok(());
    }

    let mut record = EcsPageResidencyRecord::new(
        page,
        EcsStreamPriority::new(EcsStreamInterestKind::VisibleNear, 0, page.level),
        frame.saturating_add(1),
    );
    record.state = EcsPageResidencyState::FullyReady;
    record.source_epoch = manifest.source_epoch();
    record.last_requested_frame = frame;
    record.last_visible_frame = frame;
    record.last_used_frame = frame;
    residency.push(record)
}

fn queue_age_frames(frame: u32, pending_pages: &[PendingPage]) -> u32 {
    pending_pages
        .iter()
        .map(|pending| frame.saturating_sub(pending.requested_frame))
        .max()
        .unwrap_or_default()
}

fn remove_page(pages: &mut Vec<EcsSpatialPageKey>, page: EcsSpatialPageKey) {
    if let Some(index) = pages.iter().position(|candidate| *candidate == page) {
        pages.swap_remove(index);
    }
}

fn digest_pages_in_order(
    manifest: EcsProceduralWorldManifest,
    pages: &[EcsSpatialPageKey],
) -> Result<Vec<(EcsSpatialPageKey, ProceduralPageDigest)>, EcsSpatialValidationError> {
    let mut digests = Vec::with_capacity(pages.len());
    for page in pages {
        digests.push((*page, generate_procedural_page_digest(manifest, *page)?));
    }
    Ok(digests)
}

fn find_digest_for_page(
    digests: &[(EcsSpatialPageKey, ProceduralPageDigest)],
    page: EcsSpatialPageKey,
) -> Option<ProceduralPageDigest> {
    digests
        .iter()
        .find_map(|(candidate, digest)| (*candidate == page).then_some(*digest))
}

fn sort_digests(digests: &mut [(EcsSpatialPageKey, ProceduralPageDigest)]) {
    digests.sort_by_key(|(page, _digest)| page_sort_key(*page));
}

fn page_sort_key(page: EcsSpatialPageKey) -> (u8, u64, u8, i32, i32, i32, u8) {
    (
        page.domain as u8,
        page.grid_id.get(),
        page.level,
        page.x,
        page.y,
        page.z,
        page.channel as u8,
    )
}

fn floor_division_bug_count(grid_id: EcsSpatialGridId) -> u32 {
    let cases = [
        (IVec3::new(-1, 0, -1), -1, -1),
        (IVec3::new(-32, 0, -32), -1, -1),
        (IVec3::new(-33, 0, -33), -2, -2),
        (IVec3::new(-64, 0, -64), -2, -2),
        (IVec3::new(-65, 0, -65), -3, -3),
    ];
    cases
        .iter()
        .filter(|(world_ft, expected_x, expected_z)| {
            let page = terrain_page_for_world_ft(grid_id, *world_ft, 0);
            page.x != *expected_x || page.z != *expected_z
        })
        .count()
        .min(u32::MAX as usize) as u32
}

fn per_second_q16(count: u64, frames: u32) -> u64 {
    if frames == 0 {
        return 0;
    }
    count
        .saturating_mul(BENCHMARK_FRAME_RATE_HZ)
        .saturating_mul(BENCHMARK_Q16_ONE)
        / u64::from(frames)
}

fn percentile_95(values: &mut [u64]) -> u64 {
    if values.is_empty() {
        return 0;
    }
    values.sort_unstable();
    let index = values.len().saturating_sub(1).saturating_mul(95) / 100;
    values[index]
}

fn elapsed_ns(start: Instant) -> u64 {
    start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}

fn first_nonzero(current: u64, candidate: u64) -> u64 {
    if current == 0 {
        candidate.max(1)
    } else {
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prototype_benchmark_plan_covers_requested_scenarios() {
        assert_eq!(PROCEDURAL_TERRAIN_PROTOTYPE_BENCHMARKS.len(), 5);
        assert!(
            PROCEDURAL_TERRAIN_PROTOTYPE_BENCHMARKS
                .contains(&ProceduralTerrainPrototypeBenchmarkKind::ColdSpawn)
        );
        assert!(
            PROCEDURAL_TERRAIN_PROTOTYPE_BENCHMARKS
                .contains(&ProceduralTerrainPrototypeBenchmarkKind::StreamingTreadmill)
        );
        assert!(
            PROCEDURAL_TERRAIN_PROTOTYPE_BENCHMARKS
                .contains(&ProceduralTerrainPrototypeBenchmarkKind::Teleport)
        );
        assert!(
            PROCEDURAL_TERRAIN_PROTOTYPE_BENCHMARKS
                .contains(&ProceduralTerrainPrototypeBenchmarkKind::MultiplayerDigest)
        );
        assert!(
            PROCEDURAL_TERRAIN_PROTOTYPE_BENCHMARKS
                .contains(&ProceduralTerrainPrototypeBenchmarkKind::NegativeCoordinateWorld)
        );
    }

    #[test]
    fn cold_spawn_report_measures_required_fields() {
        let report = run_procedural_terrain_prototype_benchmark(
            ProceduralTerrainPrototypeBenchmarkKind::ColdSpawn,
        )
        .expect("cold spawn benchmark runs");

        assert_eq!(
            report.camera_start_ft,
            PROCEDURAL_TERRAIN_COLD_SPAWN_CAMERA_FT
        );
        assert_eq!(
            report.required_shells,
            PROCEDURAL_TERRAIN_COLD_SPAWN_REQUIRED_SHELLS
        );
        assert_eq!(
            report.desired_shells,
            PROCEDURAL_TERRAIN_COLD_SPAWN_DESIRED_SHELLS
        );
        assert!(report.generated_pages != 0);
        assert!(report.skipped_empty_pages != 0);
        assert!(report.skipped_uniform_pages != 0);
        assert!(report.renderer_handoffs != 0);
        assert!(report.frame_p95_ns != 0);
    }

    #[test]
    fn multiplayer_digest_report_proves_order_independence() {
        let report = run_procedural_terrain_prototype_benchmark(
            ProceduralTerrainPrototypeBenchmarkKind::MultiplayerDigest,
        )
        .expect("multiplayer digest benchmark runs");

        assert_eq!(report.mismatch_count, 0);
        assert_eq!(report.digest_matches, report.requested_pages * 2);
        assert!(report.generation_order_independent);
        assert!(report.latency_to_local_terrain_ns != 0);
    }

    #[test]
    fn negative_coordinate_report_catches_floor_division_contract() {
        let report = run_procedural_terrain_prototype_benchmark(
            ProceduralTerrainPrototypeBenchmarkKind::NegativeCoordinateWorld,
        )
        .expect("negative coordinate benchmark runs");

        assert!(report.stable_page_keys);
        assert!(report.deterministic_output);
        assert_eq!(report.floor_division_bug_count, 0);
        assert!(report.generated_pages != 0);
    }
}
