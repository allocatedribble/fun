use bevy_ecs::prelude::Resource;

use crate::{
    ECS_SPATIAL_MAX_LOAD_ANIMATION_ARTIFACTS, EcsArtifactConsumer, EcsArtifactState,
    EcsDerivedArtifactId, EcsDerivedArtifactKind, EcsDerivedArtifactRecord,
    EcsRendererHandoffQueue, EcsSpatialCommand, EcsSpatialCommandBuffer, EcsSpatialPageKey,
    EcsSpatialValidationError, EcsStreamWaveId, EcsStreamWaveRecord, IVec3,
    RendererArtifactHandoffKind, RendererVisibilityHint, WorkRequiredness,
    derived_artifact_source_digest, publish_renderer_handoffs,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LoadAnimationStyle {
    Disabled = 0,
    #[default]
    RadialCameraWave = 1,
    TerrainKnit = 2,
    ScanlineShell = 3,
    DustReveal = 4,
    StormCurtain = 5,
    DebugHeatmap = 6,
}

impl LoadAnimationStyle {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::RadialCameraWave => "radial_camera_wave",
            Self::TerrainKnit => "terrain_knit",
            Self::ScanlineShell => "scanline_shell",
            Self::DustReveal => "dust_reveal",
            Self::StormCurtain => "storm_curtain",
            Self::DebugHeatmap => "debug_heatmap",
        }
    }

    #[must_use]
    pub const fn is_disabled(self) -> bool {
        matches!(self, Self::Disabled)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LoadAnimationState {
    #[default]
    Disabled = 0,
    CoarseFallbackVisible = 1,
    FineRevealQueued = 2,
    FineDitherMorph = 3,
    FullyRevealed = 4,
    DebugHeatmap = 5,
}

impl LoadAnimationState {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::CoarseFallbackVisible => "coarse_fallback_visible",
            Self::FineRevealQueued => "fine_reveal_queued",
            Self::FineDitherMorph => "fine_dither_morph",
            Self::FullyRevealed => "fully_revealed",
            Self::DebugHeatmap => "debug_heatmap",
        }
    }

    #[must_use]
    pub const fn coarse_fallback_visible(self) -> bool {
        matches!(
            self,
            Self::CoarseFallbackVisible
                | Self::FineDitherMorph
                | Self::FullyRevealed
                | Self::DebugHeatmap
        )
    }

    #[must_use]
    pub const fn fine_reveal_active(self) -> bool {
        matches!(
            self,
            Self::FineDitherMorph | Self::FullyRevealed | Self::DebugHeatmap
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EcsLoadWave {
    pub wave_id: EcsStreamWaveId,
    pub origin_page: EcsSpatialPageKey,
    pub origin_world_ft: IVec3,
    pub start_frame: u64,
    pub start_time_s: f32,
    pub style: LoadAnimationStyle,
}

impl EcsLoadWave {
    #[must_use]
    pub fn from_stream_wave(
        stream_wave: EcsStreamWaveRecord,
        start_time_s: f32,
        style: LoadAnimationStyle,
    ) -> Self {
        Self {
            wave_id: stream_wave.wave_id,
            origin_page: stream_wave.origin_page,
            origin_world_ft: stream_wave.origin_world_ft,
            start_frame: stream_wave.created_frame,
            start_time_s,
            style,
        }
    }

    #[must_use]
    pub fn with_default_style(stream_wave: EcsStreamWaveRecord, start_time_s: f32) -> Self {
        Self::from_stream_wave(stream_wave, start_time_s, LoadAnimationStyle::default())
    }

    #[must_use]
    pub fn reveal_origin_ws(self) -> [f32; 3] {
        [
            self.origin_world_ft.x as f32,
            self.origin_world_ft.y as f32,
            self.origin_world_ft.z as f32,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Resource)]
pub struct EcsLoadWaveLedger {
    pub active_wave: EcsStreamWaveId,
    pub waves: Vec<EcsLoadWave>,
}

impl Default for EcsLoadWaveLedger {
    fn default() -> Self {
        Self {
            active_wave: EcsStreamWaveId::INVALID,
            waves: Vec::new(),
        }
    }
}

impl EcsLoadWaveLedger {
    pub fn push(&mut self, wave: EcsLoadWave) -> Result<(), EcsSpatialValidationError> {
        if self.waves.len() >= crate::ECS_SPATIAL_MAX_STREAM_WAVES {
            return Err(EcsSpatialValidationError::LoadAnimationQueueFull);
        }
        self.active_wave = wave.wave_id;
        self.waves.push(wave);
        Ok(())
    }

    #[must_use]
    pub fn get(&self, wave_id: EcsStreamWaveId) -> Option<&EcsLoadWave> {
        self.waves.iter().find(|wave| wave.wave_id == wave_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoadAnimationArtifact {
    pub source_page: EcsSpatialPageKey,
    pub wave_id: EcsStreamWaveId,
    pub state: LoadAnimationState,
    pub reveal_origin_ws: [f32; 3],
    pub reveal_radius_ft: f32,
    pub reveal_softness_ft: f32,
    pub progress: f32,
    pub fallback_artifact: Option<EcsDerivedArtifactId>,
    pub fine_artifact: Option<EcsDerivedArtifactId>,
    pub temporal_history_reset_epoch: u32,
}

impl LoadAnimationArtifact {
    #[must_use]
    pub const fn fine_reveal_active(self) -> bool {
        self.state.fine_reveal_active()
    }

    #[must_use]
    pub const fn coarse_fallback_visible(self) -> bool {
        self.state.coarse_fallback_visible()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadAnimationArtifactInput {
    pub source_page: EcsSpatialPageKey,
    pub fallback_artifact: Option<EcsDerivedArtifactId>,
    pub fine_artifact: Option<EcsDerivedArtifactId>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoadAnimationBuildOptions {
    pub benchmark_truth_mode: bool,
    pub page_edge_ft: f32,
    pub reveal_speed_ft_s: f32,
    pub reveal_softness_ft: f32,
}

impl Default for LoadAnimationBuildOptions {
    fn default() -> Self {
        Self {
            benchmark_truth_mode: false,
            page_edge_ft: 32.0,
            reveal_speed_ft_s: 192.0,
            reveal_softness_ft: 12.0,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LoadAnimationBuildReport {
    pub inspected: u32,
    pub built: u32,
    pub disabled: u32,
    pub temporal_history_reset_hints: u32,
}

#[derive(Debug, Default, Clone, PartialEq, Resource)]
pub struct EcsLoadAnimationArtifactQueue {
    pub items: Vec<LoadAnimationArtifact>,
}

impl EcsLoadAnimationArtifactQueue {
    pub fn push(&mut self, item: LoadAnimationArtifact) -> Result<(), EcsSpatialValidationError> {
        if self.items.len() >= ECS_SPATIAL_MAX_LOAD_ANIMATION_ARTIFACTS {
            return Err(EcsSpatialValidationError::LoadAnimationQueueFull);
        }
        self.items.push(item);
        Ok(())
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }
}

pub fn build_load_animation_records(
    wave: EcsLoadWave,
    inputs: &[LoadAnimationArtifactInput],
    now_frame: u64,
    now_time_s: f32,
    temporal_history_reset_epoch: u32,
    options: LoadAnimationBuildOptions,
    output: &mut EcsLoadAnimationArtifactQueue,
) -> Result<LoadAnimationBuildReport, EcsSpatialValidationError> {
    let mut report = LoadAnimationBuildReport {
        inspected: inputs.len().min(u32::MAX as usize) as u32,
        ..Default::default()
    };
    if wave.style.is_disabled() || options.benchmark_truth_mode {
        report.disabled = report.inspected;
        return Ok(report);
    }

    for input in inputs {
        let artifact = load_animation_artifact(
            wave,
            *input,
            now_frame,
            now_time_s,
            temporal_history_reset_epoch,
            options,
        );
        if artifact.temporal_history_reset_epoch != 0 {
            report.temporal_history_reset_hints =
                report.temporal_history_reset_hints.saturating_add(1);
        }
        output.push(artifact)?;
        report.built = report.built.saturating_add(1);
    }
    Ok(report)
}

fn load_animation_artifact(
    wave: EcsLoadWave,
    input: LoadAnimationArtifactInput,
    now_frame: u64,
    now_time_s: f32,
    temporal_history_reset_epoch: u32,
    options: LoadAnimationBuildOptions,
) -> LoadAnimationArtifact {
    let elapsed_s = (now_time_s - wave.start_time_s).max(0.0);
    let reveal_radius_ft = elapsed_s * options.reveal_speed_ft_s.max(0.0);
    let reveal_softness_ft = options.reveal_softness_ft.max(0.001);
    let distance_ft = reveal_distance_ft(wave, input.source_page, options.page_edge_ft.max(1.0));
    let progress = ((reveal_radius_ft - distance_ft) / reveal_softness_ft).clamp(0.0, 1.0);
    let state = load_animation_state(wave.style, progress, input.fallback_artifact);
    let reset_epoch =
        if now_frame >= wave.start_frame && progress > 0.0 && input.fine_artifact.is_some() {
            temporal_history_reset_epoch
        } else {
            0
        };

    LoadAnimationArtifact {
        source_page: input.source_page,
        wave_id: wave.wave_id,
        state,
        reveal_origin_ws: wave.reveal_origin_ws(),
        reveal_radius_ft,
        reveal_softness_ft,
        progress,
        fallback_artifact: input.fallback_artifact,
        fine_artifact: input.fine_artifact,
        temporal_history_reset_epoch: reset_epoch,
    }
}

fn load_animation_state(
    style: LoadAnimationStyle,
    progress: f32,
    fallback: Option<EcsDerivedArtifactId>,
) -> LoadAnimationState {
    if style == LoadAnimationStyle::DebugHeatmap {
        return LoadAnimationState::DebugHeatmap;
    }
    if progress <= f32::EPSILON {
        return if fallback.is_some() {
            LoadAnimationState::CoarseFallbackVisible
        } else {
            LoadAnimationState::FineRevealQueued
        };
    }
    if progress < 1.0 {
        LoadAnimationState::FineDitherMorph
    } else {
        LoadAnimationState::FullyRevealed
    }
}

fn reveal_distance_ft(wave: EcsLoadWave, page: EcsSpatialPageKey, page_edge_ft: f32) -> f32 {
    let level_scale = if page.level >= 20 {
        (1_u64 << 20) as f32
    } else {
        (1_u64 << u32::from(page.level)) as f32
    };
    let edge = page_edge_ft * level_scale;
    let center_x = page.x as f32 * edge + edge * 0.5;
    let center_y = page.y as f32 * edge + edge * 0.5;
    let center_z = page.z as f32 * edge + edge * 0.5;
    let dx = center_x - wave.origin_world_ft.x as f32;
    let dy = center_y - wave.origin_world_ft.y as f32;
    let dz = center_z - wave.origin_world_ft.z as f32;

    match wave.style {
        LoadAnimationStyle::ScanlineShell => dx.abs().max(dy.abs()).max(dz.abs()),
        LoadAnimationStyle::TerrainKnit => (dx.abs() + dy.abs()) * 0.5 + dz.abs() * 0.25,
        LoadAnimationStyle::DustReveal | LoadAnimationStyle::StormCurtain => {
            (dx * dx + dy * dy).sqrt() + dz.abs() * 0.5
        }
        LoadAnimationStyle::DebugHeatmap
        | LoadAnimationStyle::Disabled
        | LoadAnimationStyle::RadialCameraWave => (dx * dx + dy * dy + dz * dz).sqrt(),
    }
}

#[must_use]
pub fn load_animation_artifact_record(
    artifact_id: EcsDerivedArtifactId,
    source_page: EcsSpatialPageKey,
    source_epoch: u32,
    artifact_epoch: u32,
) -> EcsDerivedArtifactRecord {
    EcsDerivedArtifactRecord {
        artifact_id,
        source_page,
        kind: EcsDerivedArtifactKind::LoadAnimationRecord,
        source_epoch,
        source_digest: derived_artifact_source_digest(source_page, source_epoch, artifact_epoch),
        artifact_epoch,
        state: EcsArtifactState::Ready,
        requiredness: WorkRequiredness::Optional,
        consumer: EcsArtifactConsumer::Renderer,
    }
}

pub fn publish_load_animation_handoffs(
    artifacts: &[EcsDerivedArtifactRecord],
    commands: &mut EcsSpatialCommandBuffer,
) -> Result<crate::EcsRendererHandoffPublishReport, EcsSpatialValidationError> {
    publish_renderer_handoffs(artifacts, RendererVisibilityHint::VisibleNear, commands)
}

pub fn load_animation_handoff_queue_from_commands(
    commands: &EcsSpatialCommandBuffer,
) -> Result<EcsRendererHandoffQueue, EcsSpatialValidationError> {
    let mut queue = EcsRendererHandoffQueue::default();
    for command in &commands.commands {
        if let EcsSpatialCommand::PublishRendererHandoff(handoff) = command
            && handoff.kind == RendererArtifactHandoffKind::LoadAnimationRecords
        {
            queue.push(*handoff)?;
        }
    }
    Ok(queue)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EcsPageChannel, EcsSpatialDomainKind, EcsSpatialGridId, EcsStreamWaveId, StreamWaveReason,
        renderer_handoff_from_artifact,
    };
    use fun_scheduler_types::EcsEntityId;

    fn page(x: i32, y: i32, z: i32) -> EcsSpatialPageKey {
        EcsSpatialPageKey::new(
            EcsSpatialDomainKind::Terrain,
            EcsSpatialGridId::new(1),
            0,
            x,
            y,
            z,
            EcsPageChannel::Surface,
        )
    }

    fn stream_wave() -> EcsStreamWaveRecord {
        EcsStreamWaveRecord {
            wave_id: EcsStreamWaveId::new(9),
            origin_page: page(0, 0, 0),
            origin_world_ft: IVec3::new(4, 5, 6),
            camera_entity: EcsEntityId::new(77),
            created_frame: 120,
            max_shell_planned: 4,
            reason: StreamWaveReason::ColdStart,
        }
    }

    #[test]
    fn default_load_animation_style_is_radial_camera_wave() {
        assert_eq!(
            LoadAnimationStyle::default(),
            LoadAnimationStyle::RadialCameraWave
        );
    }

    #[test]
    fn load_wave_starts_at_active_stream_camera_origin() {
        let wave = EcsLoadWave::with_default_style(stream_wave(), 42.0);

        assert_eq!(wave.wave_id, EcsStreamWaveId::new(9));
        assert_eq!(wave.origin_page, page(0, 0, 0));
        assert_eq!(wave.origin_world_ft, IVec3::new(4, 5, 6));
        assert_eq!(wave.reveal_origin_ws(), [4.0, 5.0, 6.0]);
        assert_eq!(wave.start_frame, 120);
        assert_eq!(wave.start_time_s, 42.0);
    }

    #[test]
    fn benchmark_truth_mode_disables_load_animation_records() {
        let wave = EcsLoadWave::with_default_style(stream_wave(), 0.0);
        let inputs = [LoadAnimationArtifactInput {
            source_page: page(1, 0, 0),
            fallback_artifact: Some(EcsDerivedArtifactId::new(1)),
            fine_artifact: Some(EcsDerivedArtifactId::new(2)),
        }];
        let mut output = EcsLoadAnimationArtifactQueue::default();
        let report = build_load_animation_records(
            wave,
            &inputs,
            121,
            1.0,
            33,
            LoadAnimationBuildOptions {
                benchmark_truth_mode: true,
                ..Default::default()
            },
            &mut output,
        )
        .expect("build load animation records");

        assert_eq!(report.inspected, 1);
        assert_eq!(report.disabled, 1);
        assert_eq!(report.built, 0);
        assert!(output.items.is_empty());
    }

    #[test]
    fn coarse_fallback_appears_before_fine_dither_morph() {
        let wave = EcsLoadWave::with_default_style(stream_wave(), 0.0);
        let input = LoadAnimationArtifactInput {
            source_page: page(2, 0, 0),
            fallback_artifact: Some(EcsDerivedArtifactId::new(10)),
            fine_artifact: Some(EcsDerivedArtifactId::new(11)),
        };
        let mut output = EcsLoadAnimationArtifactQueue::default();
        build_load_animation_records(
            wave,
            &[input],
            121,
            0.0,
            44,
            LoadAnimationBuildOptions {
                reveal_speed_ft_s: 64.0,
                reveal_softness_ft: 16.0,
                ..Default::default()
            },
            &mut output,
        )
        .expect("early build");
        let early = output.items[0];
        assert_eq!(early.state, LoadAnimationState::CoarseFallbackVisible);
        assert_eq!(early.progress, 0.0);
        assert!(early.coarse_fallback_visible());
        assert!(!early.fine_reveal_active());

        output.clear();
        build_load_animation_records(
            wave,
            &[input],
            122,
            2.0,
            44,
            LoadAnimationBuildOptions {
                reveal_speed_ft_s: 64.0,
                reveal_softness_ft: 64.0,
                ..Default::default()
            },
            &mut output,
        )
        .expect("later build");
        let later = output.items[0];
        assert!(matches!(
            later.state,
            LoadAnimationState::FineDitherMorph | LoadAnimationState::FullyRevealed
        ));
        assert!(later.progress > 0.0);
        assert!(later.coarse_fallback_visible());
        assert!(later.fine_reveal_active());
    }

    #[test]
    fn newly_revealed_fine_pages_emit_temporal_history_reset_hints() {
        let wave = EcsLoadWave::with_default_style(stream_wave(), 0.0);
        let input = LoadAnimationArtifactInput {
            source_page: page(0, 0, 0),
            fallback_artifact: Some(EcsDerivedArtifactId::new(10)),
            fine_artifact: Some(EcsDerivedArtifactId::new(11)),
        };
        let mut output = EcsLoadAnimationArtifactQueue::default();
        let report = build_load_animation_records(
            wave,
            &[input],
            121,
            1.0,
            55,
            LoadAnimationBuildOptions {
                reveal_speed_ft_s: 128.0,
                reveal_softness_ft: 64.0,
                ..Default::default()
            },
            &mut output,
        )
        .expect("build reset hint");

        assert_eq!(report.temporal_history_reset_hints, 1);
        assert_eq!(output.items[0].temporal_history_reset_epoch, 55);
    }

    #[test]
    fn load_animation_records_publish_renderer_handoffs() {
        let record =
            load_animation_artifact_record(EcsDerivedArtifactId::new(3), page(0, 0, 0), 7, 8);
        let handoff = renderer_handoff_from_artifact(record, RendererVisibilityHint::VisibleNear)
            .expect("renderer handoff");
        assert_eq!(
            handoff.kind,
            RendererArtifactHandoffKind::LoadAnimationRecords
        );

        let mut commands = EcsSpatialCommandBuffer::default();
        let report = publish_load_animation_handoffs(&[record], &mut commands)
            .expect("publish load handoff");
        let queue = load_animation_handoff_queue_from_commands(&commands).expect("load queue");

        assert_eq!(report.published, 1);
        assert_eq!(queue.items.len(), 1);
        assert_eq!(
            queue.items[0].kind,
            RendererArtifactHandoffKind::LoadAnimationRecords
        );
    }
}
