use fun_ecs::{
    EcsDerivedArtifactId, EcsLoadWave, EcsRendererHandoffQueue, EcsSpatialPageKey, EcsStreamWaveId,
    LoadAnimationArtifact, LoadAnimationState, LoadAnimationStyle, PackedAabb, PackedRange,
    RendererArtifactHandoff, RendererArtifactHandoffKind, RendererVisibilityHint,
    TerrainMaterialPaletteId, VoxelSurfacePacketArtifact, WorkRequiredness,
};

pub const RENDERER_ECS_HANDOFF_SCHEMA_VERSION: u16 = 1;
pub const RENDERER_TERRAIN_HANDOFF_SYSTEM_COUNT: usize = 5;

pub const RENDERER_TERRAIN_HANDOFF_SYSTEMS: [RendererTerrainHandoffSystem;
    RENDERER_TERRAIN_HANDOFF_SYSTEM_COUNT] = [
    RendererTerrainHandoffSystem::RendererImportTerrainArtifacts,
    RendererTerrainHandoffSystem::RendererUploadTerrainPages,
    RendererTerrainHandoffSystem::RendererPublishVirtualGeometry,
    RendererTerrainHandoffSystem::RendererPublishLoadAnimation,
    RendererTerrainHandoffSystem::RendererRetireTerrainArtifacts,
];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RendererTerrainHandoffSystem {
    #[default]
    RendererImportTerrainArtifacts = 0,
    RendererUploadTerrainPages = 1,
    RendererPublishVirtualGeometry = 2,
    RendererPublishLoadAnimation = 3,
    RendererRetireTerrainArtifacts = 4,
}

impl RendererTerrainHandoffSystem {
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &RENDERER_TERRAIN_HANDOFF_SYSTEMS
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::RendererImportTerrainArtifacts => "renderer_import_terrain_artifacts",
            Self::RendererUploadTerrainPages => "renderer_upload_terrain_pages",
            Self::RendererPublishVirtualGeometry => "renderer_publish_virtual_geometry",
            Self::RendererPublishLoadAnimation => "renderer_publish_load_animation",
            Self::RendererRetireTerrainArtifacts => "renderer_retire_terrain_artifacts",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RendererTerrainUploadTicketId(pub u64);

impl RendererTerrainUploadTicketId {
    pub const INVALID: Self = Self(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RendererTerrainUploadState {
    #[default]
    NotQueued = 0,
    Queued = 1,
    DeferredByBudget = 2,
    Uploaded = 3,
    Retired = 4,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RendererTerrainRealization {
    #[default]
    Imported = 0,
    UploadQueued = 1,
    Uploaded = 2,
    VirtualGeometryPublished = 3,
    GpuSceneIndirectFallbackPublished = 4,
    LoadAnimationPublished = 5,
    Retired = 6,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RendererTerrainPublishPath {
    #[default]
    VirtualGeometry = 0,
    GpuSceneIndirectFallback = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererTerrainArtifactUploadTicket {
    pub ticket_id: RendererTerrainUploadTicketId,
    pub artifact_id: EcsDerivedArtifactId,
    pub source_page: EcsSpatialPageKey,
    pub kind: RendererArtifactHandoffKind,
    pub source_epoch: u32,
    pub artifact_epoch: u32,
    pub requiredness: WorkRequiredness,
    pub visibility_hint: RendererVisibilityHint,
    pub estimated_upload_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererTerrainArtifactRecord {
    pub artifact_id: EcsDerivedArtifactId,
    pub source_page: EcsSpatialPageKey,
    pub kind: RendererArtifactHandoffKind,
    pub source_epoch: u32,
    pub artifact_epoch: u32,
    pub requiredness: WorkRequiredness,
    pub visibility_hint: RendererVisibilityHint,
    pub ticket_id: RendererTerrainUploadTicketId,
    pub upload_state: RendererTerrainUploadState,
    pub realization: RendererTerrainRealization,
    pub estimated_upload_bytes: u64,
    pub published_packet_count: u32,
    pub publish_path: Option<RendererTerrainPublishPath>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererTerrainArtifactTable {
    pub records: Vec<RendererTerrainArtifactRecord>,
    pub pending_uploads: Vec<RendererTerrainArtifactUploadTicket>,
    next_ticket_id: u64,
}

impl Default for RendererTerrainArtifactTable {
    fn default() -> Self {
        Self {
            records: Vec::new(),
            pending_uploads: Vec::new(),
            next_ticket_id: 1,
        }
    }
}

impl RendererTerrainArtifactTable {
    #[must_use]
    pub fn get(&self, artifact_id: EcsDerivedArtifactId) -> Option<&RendererTerrainArtifactRecord> {
        self.records
            .iter()
            .find(|record| record.artifact_id == artifact_id)
    }

    fn get_mut(
        &mut self,
        artifact_id: EcsDerivedArtifactId,
    ) -> Option<&mut RendererTerrainArtifactRecord> {
        self.records
            .iter_mut()
            .find(|record| record.artifact_id == artifact_id)
    }

    fn next_ticket(&mut self) -> RendererTerrainUploadTicketId {
        let id = RendererTerrainUploadTicketId::new(self.next_ticket_id);
        self.next_ticket_id = self.next_ticket_id.saturating_add(1).max(1);
        id
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RendererTerrainImportReport {
    pub inspected: u32,
    pub imported: u32,
    pub coalesced: u32,
    pub rejected_zero_epoch: u32,
    pub rejected_stale_epoch: u32,
    pub upload_tickets_created: u32,
}

pub fn renderer_import_terrain_artifacts(
    queue: &EcsRendererHandoffQueue,
    table: &mut RendererTerrainArtifactTable,
) -> RendererTerrainImportReport {
    let mut report = RendererTerrainImportReport::default();
    for handoff in &queue.items {
        report.inspected = report.inspected.saturating_add(1);
        if handoff.source_epoch == 0 || handoff.artifact_epoch == 0 {
            report.rejected_zero_epoch = report.rejected_zero_epoch.saturating_add(1);
            continue;
        }
        if let Some(existing) = table.get(handoff.artifact_id) {
            if existing.source_epoch > handoff.source_epoch
                || existing.artifact_epoch > handoff.artifact_epoch
            {
                report.rejected_stale_epoch = report.rejected_stale_epoch.saturating_add(1);
                continue;
            }
            if existing.source_epoch == handoff.source_epoch
                && existing.artifact_epoch == handoff.artifact_epoch
                && existing.upload_state != RendererTerrainUploadState::Retired
            {
                report.coalesced = report.coalesced.saturating_add(1);
                continue;
            }
        }

        let ticket = upload_ticket_from_handoff(*handoff, table.next_ticket());
        let record = record_from_ticket(ticket);
        if let Some(existing) = table.get_mut(handoff.artifact_id) {
            *existing = record;
        } else {
            table.records.push(record);
        }
        table.pending_uploads.push(ticket);
        report.imported = report.imported.saturating_add(1);
        report.upload_tickets_created = report.upload_tickets_created.saturating_add(1);
    }
    report
}

fn upload_ticket_from_handoff(
    handoff: RendererArtifactHandoff,
    ticket_id: RendererTerrainUploadTicketId,
) -> RendererTerrainArtifactUploadTicket {
    RendererTerrainArtifactUploadTicket {
        ticket_id,
        artifact_id: handoff.artifact_id,
        source_page: handoff.source_page,
        kind: handoff.kind,
        source_epoch: handoff.source_epoch,
        artifact_epoch: handoff.artifact_epoch,
        requiredness: handoff.requiredness,
        visibility_hint: handoff.visibility_hint,
        estimated_upload_bytes: estimated_upload_bytes(handoff.kind),
    }
}

fn record_from_ticket(
    ticket: RendererTerrainArtifactUploadTicket,
) -> RendererTerrainArtifactRecord {
    RendererTerrainArtifactRecord {
        artifact_id: ticket.artifact_id,
        source_page: ticket.source_page,
        kind: ticket.kind,
        source_epoch: ticket.source_epoch,
        artifact_epoch: ticket.artifact_epoch,
        requiredness: ticket.requiredness,
        visibility_hint: ticket.visibility_hint,
        ticket_id: ticket.ticket_id,
        upload_state: RendererTerrainUploadState::Queued,
        realization: RendererTerrainRealization::UploadQueued,
        estimated_upload_bytes: ticket.estimated_upload_bytes,
        published_packet_count: 0,
        publish_path: None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererTerrainUploadBudget {
    pub max_upload_bytes: u64,
    pub max_tickets: u32,
}

impl Default for RendererTerrainUploadBudget {
    fn default() -> Self {
        Self {
            max_upload_bytes: 8 * 1024 * 1024,
            max_tickets: 128,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RendererTerrainUploadReport {
    pub inspected: u32,
    pub uploaded: u32,
    pub deferred_by_budget: u32,
    pub uploaded_bytes: u64,
}

pub fn renderer_upload_terrain_pages(
    table: &mut RendererTerrainArtifactTable,
    budget: RendererTerrainUploadBudget,
) -> RendererTerrainUploadReport {
    let mut report = RendererTerrainUploadReport::default();
    let mut retained = Vec::new();
    let pending = core::mem::take(&mut table.pending_uploads);
    for ticket in pending {
        report.inspected = report.inspected.saturating_add(1);
        let over_ticket_budget = report.uploaded >= budget.max_tickets;
        let over_byte_budget = report
            .uploaded_bytes
            .saturating_add(ticket.estimated_upload_bytes)
            > budget.max_upload_bytes;
        if over_ticket_budget || over_byte_budget {
            if let Some(record) = table.get_mut(ticket.artifact_id) {
                record.upload_state = RendererTerrainUploadState::DeferredByBudget;
            }
            retained.push(ticket);
            report.deferred_by_budget = report.deferred_by_budget.saturating_add(1);
            continue;
        }

        if let Some(record) = table.get_mut(ticket.artifact_id) {
            record.upload_state = RendererTerrainUploadState::Uploaded;
            record.realization = RendererTerrainRealization::Uploaded;
        }
        report.uploaded = report.uploaded.saturating_add(1);
        report.uploaded_bytes = report
            .uploaded_bytes
            .saturating_add(ticket.estimated_upload_bytes);
    }
    table.pending_uploads = retained;
    report
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RendererTerrainPublishReport {
    pub inspected_packets: u32,
    pub virtual_geometry_published: u32,
    pub gpu_scene_indirect_fallbacks: u32,
    pub skipped_not_uploaded: u32,
}

pub fn renderer_publish_virtual_geometry(
    table: &mut RendererTerrainArtifactTable,
    surface_packets: &[VoxelSurfacePacketArtifact],
    virtual_geometry_executable: bool,
) -> RendererTerrainPublishReport {
    let mut report = RendererTerrainPublishReport::default();
    for packet in surface_packets {
        report.inspected_packets = report.inspected_packets.saturating_add(1);
        let Some(record) = table.records.iter_mut().find(|record| {
            record.source_page == packet.source_page
                && record.source_epoch == packet.source_epoch
                && record.kind == RendererArtifactHandoffKind::TerrainSurfacePackets
        }) else {
            report.skipped_not_uploaded = report.skipped_not_uploaded.saturating_add(1);
            continue;
        };
        if record.upload_state != RendererTerrainUploadState::Uploaded {
            report.skipped_not_uploaded = report.skipped_not_uploaded.saturating_add(1);
            continue;
        }
        record.published_packet_count = packet.packet_count;
        if virtual_geometry_executable {
            record.realization = RendererTerrainRealization::VirtualGeometryPublished;
            record.publish_path = Some(RendererTerrainPublishPath::VirtualGeometry);
            report.virtual_geometry_published = report.virtual_geometry_published.saturating_add(1);
        } else {
            record.realization = RendererTerrainRealization::GpuSceneIndirectFallbackPublished;
            record.publish_path = Some(RendererTerrainPublishPath::GpuSceneIndirectFallback);
            report.gpu_scene_indirect_fallbacks =
                report.gpu_scene_indirect_fallbacks.saturating_add(1);
        }
    }
    report
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RendererLoadAnimationPublishReport {
    pub inspected: u32,
    pub published: u32,
    pub skipped_not_uploaded: u32,
    pub skipped_missing_wave: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RendererLoadAnimationBufferRow {
    pub artifact_id: EcsDerivedArtifactId,
    pub source_page: EcsSpatialPageKey,
    pub wave_id: EcsStreamWaveId,
    pub style: LoadAnimationStyle,
    pub state: LoadAnimationState,
    pub reveal_origin_ws: [f32; 3],
    pub reveal_radius_ft: f32,
    pub reveal_softness_ft: f32,
    pub progress: f32,
    pub fallback_artifact: Option<EcsDerivedArtifactId>,
    pub fine_artifact: Option<EcsDerivedArtifactId>,
    pub temporal_history_reset_epoch: u32,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct RendererLoadAnimationBuffer {
    pub rows: Vec<RendererLoadAnimationBufferRow>,
    pub temporal_history_reset_epoch: u32,
}

impl RendererLoadAnimationBuffer {
    pub fn upsert(&mut self, row: RendererLoadAnimationBufferRow) {
        if let Some(existing) = self.rows.iter_mut().find(|existing| {
            existing.source_page == row.source_page && existing.wave_id == row.wave_id
        }) {
            *existing = row;
        } else {
            self.rows.push(row);
        }
        self.temporal_history_reset_epoch = self
            .temporal_history_reset_epoch
            .max(row.temporal_history_reset_epoch);
    }
}

pub fn renderer_publish_load_animation(
    table: &mut RendererTerrainArtifactTable,
    artifacts: &[LoadAnimationArtifact],
    waves: &[EcsLoadWave],
    buffer: &mut RendererLoadAnimationBuffer,
) -> RendererLoadAnimationPublishReport {
    let mut report = RendererLoadAnimationPublishReport::default();
    for artifact in artifacts {
        report.inspected = report.inspected.saturating_add(1);
        let Some(record_index) = table.records.iter().position(|record| {
            record.source_page == artifact.source_page
                && record.kind == RendererArtifactHandoffKind::LoadAnimationRecords
        }) else {
            report.skipped_not_uploaded = report.skipped_not_uploaded.saturating_add(1);
            continue;
        };
        let record = &table.records[record_index];
        if record.upload_state != RendererTerrainUploadState::Uploaded {
            report.skipped_not_uploaded = report.skipped_not_uploaded.saturating_add(1);
            continue;
        }
        let Some(wave) = waves.iter().find(|wave| wave.wave_id == artifact.wave_id) else {
            report.skipped_missing_wave = report.skipped_missing_wave.saturating_add(1);
            continue;
        };
        let row = RendererLoadAnimationBufferRow {
            artifact_id: record.artifact_id,
            source_page: artifact.source_page,
            wave_id: artifact.wave_id,
            style: wave.style,
            state: artifact.state,
            reveal_origin_ws: artifact.reveal_origin_ws,
            reveal_radius_ft: artifact.reveal_radius_ft,
            reveal_softness_ft: artifact.reveal_softness_ft,
            progress: artifact.progress,
            fallback_artifact: artifact.fallback_artifact,
            fine_artifact: artifact.fine_artifact,
            temporal_history_reset_epoch: artifact.temporal_history_reset_epoch,
        };
        buffer.upsert(row);
        let record = &mut table.records[record_index];
        record.realization = RendererTerrainRealization::LoadAnimationPublished;
        report.published = report.published.saturating_add(1);
    }
    report
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RendererTerrainRetireReason {
    #[default]
    EcsResidencyEvicted = 0,
    GenerationChanged = 1,
    ArtifactEpochChanged = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererTerrainArtifactRetireRequest {
    pub source_page: EcsSpatialPageKey,
    pub live_source_epoch: u32,
    pub live_artifact_epoch: u32,
    pub reason: RendererTerrainRetireReason,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RendererTerrainRetireReport {
    pub inspected: u32,
    pub retired: u32,
}

pub fn renderer_retire_terrain_artifacts(
    table: &mut RendererTerrainArtifactTable,
    requests: &[RendererTerrainArtifactRetireRequest],
) -> RendererTerrainRetireReport {
    let mut report = RendererTerrainRetireReport::default();
    for request in requests {
        for record in &mut table.records {
            if record.source_page != request.source_page {
                continue;
            }
            report.inspected = report.inspected.saturating_add(1);
            let should_retire = match request.reason {
                RendererTerrainRetireReason::EcsResidencyEvicted => true,
                RendererTerrainRetireReason::GenerationChanged => {
                    record.source_epoch != request.live_source_epoch
                }
                RendererTerrainRetireReason::ArtifactEpochChanged => {
                    record.artifact_epoch < request.live_artifact_epoch
                }
            };
            if should_retire && record.realization != RendererTerrainRealization::Retired {
                record.upload_state = RendererTerrainUploadState::Retired;
                record.realization = RendererTerrainRealization::Retired;
                report.retired = report.retired.saturating_add(1);
            }
        }
    }
    report
}

#[must_use]
pub fn terrain_surface_packet_for_handoff(
    handoff: RendererArtifactHandoff,
    packet_count: u32,
    packet_range: PackedRange,
    local_bounds: PackedAabb,
    material_palette_id: TerrainMaterialPaletteId,
    load_animation_id: Option<EcsDerivedArtifactId>,
) -> VoxelSurfacePacketArtifact {
    VoxelSurfacePacketArtifact {
        source_page: handoff.source_page,
        packet_count,
        packet_range,
        local_bounds,
        material_palette_id,
        load_animation_id,
        source_epoch: handoff.source_epoch,
    }
}

const fn estimated_upload_bytes(kind: RendererArtifactHandoffKind) -> u64 {
    match kind {
        RendererArtifactHandoffKind::TerrainSurfacePackets => 128 * 1024,
        RendererArtifactHandoffKind::TerrainCoarseProxy => 32 * 1024,
        RendererArtifactHandoffKind::TerrainMaterialPage => 64 * 1024,
        RendererArtifactHandoffKind::TerrainSdfPage => 96 * 1024,
        RendererArtifactHandoffKind::FoliageClusters => 96 * 1024,
        RendererArtifactHandoffKind::CanopyOpacity => 32 * 1024,
        RendererArtifactHandoffKind::LoadAnimationRecords => 16 * 1024,
        RendererArtifactHandoffKind::FoliageVirtualGeometry => 192 * 1024,
        RendererArtifactHandoffKind::FoliageInstanceClusters => 96 * 1024,
        RendererArtifactHandoffKind::FoliageCardsImpostors => 64 * 1024,
        RendererArtifactHandoffKind::FoliageClusteredAnimation => 48 * 1024,
        RendererArtifactHandoffKind::FoliageBiomeTint => 16 * 1024,
        RendererArtifactHandoffKind::FoliageHorizonImpostorField => 64 * 1024,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fun_ecs::{
        EcsEntityId, EcsPageChannel, EcsSpatialDomainKind, EcsSpatialGridId, EcsSpatialPageKey,
        EcsStreamWaveRecord, IVec3, StreamWaveReason,
    };

    fn page() -> EcsSpatialPageKey {
        EcsSpatialPageKey::new(
            EcsSpatialDomainKind::Terrain,
            EcsSpatialGridId::new(1),
            0,
            2,
            3,
            4,
            EcsPageChannel::Surface,
        )
    }

    fn handoff(id: u64, kind: RendererArtifactHandoffKind) -> RendererArtifactHandoff {
        RendererArtifactHandoff {
            artifact_id: EcsDerivedArtifactId::new(id),
            source_page: page(),
            kind,
            source_epoch: 7,
            artifact_epoch: 9,
            requiredness: WorkRequiredness::Required,
            visibility_hint: RendererVisibilityHint::VisibleNear,
        }
    }

    fn load_wave() -> EcsLoadWave {
        EcsLoadWave::from_stream_wave(
            EcsStreamWaveRecord {
                wave_id: EcsStreamWaveId::new(8),
                origin_page: page(),
                origin_world_ft: IVec3::new(64, 96, 128),
                camera_entity: EcsEntityId::new(2),
                created_frame: 3,
                max_shell_planned: 4,
                reason: StreamWaveReason::ColdStart,
            },
            1.25,
            LoadAnimationStyle::RadialCameraWave,
        )
    }

    fn load_animation_artifact() -> LoadAnimationArtifact {
        LoadAnimationArtifact {
            source_page: page(),
            wave_id: EcsStreamWaveId::new(8),
            state: LoadAnimationState::FineDitherMorph,
            reveal_origin_ws: [64.0, 96.0, 128.0],
            reveal_radius_ft: 48.0,
            reveal_softness_ft: 8.0,
            progress: 0.5,
            fallback_artifact: Some(EcsDerivedArtifactId::new(3)),
            fine_artifact: Some(EcsDerivedArtifactId::new(4)),
            temporal_history_reset_epoch: 12,
        }
    }

    #[test]
    fn renderer_import_consumes_ecs_handoff_rows_without_mutating_queue_state() {
        let mut queue = EcsRendererHandoffQueue::default();
        queue
            .push(handoff(
                1,
                RendererArtifactHandoffKind::TerrainSurfacePackets,
            ))
            .expect("queue handoff");
        let before = queue.clone();
        let mut table = RendererTerrainArtifactTable::default();
        let report = renderer_import_terrain_artifacts(&queue, &mut table);

        assert_eq!(queue, before);
        assert_eq!(report.imported, 1);
        assert_eq!(report.upload_tickets_created, 1);
        assert_eq!(table.records.len(), 1);
        assert_eq!(table.pending_uploads.len(), 1);
        assert_eq!(
            table.records[0].realization,
            RendererTerrainRealization::UploadQueued
        );
    }

    #[test]
    fn renderer_import_rejects_zero_or_stale_epochs() {
        let mut queue = EcsRendererHandoffQueue::default();
        let mut zero = handoff(1, RendererArtifactHandoffKind::TerrainSurfacePackets);
        zero.source_epoch = 0;
        queue.push(zero).expect("zero handoff");
        let mut table = RendererTerrainArtifactTable::default();
        let report = renderer_import_terrain_artifacts(&queue, &mut table);
        assert_eq!(report.rejected_zero_epoch, 1);
        assert!(table.records.is_empty());

        let mut newer = handoff(2, RendererArtifactHandoffKind::TerrainSurfacePackets);
        newer.source_epoch = 11;
        newer.artifact_epoch = 12;
        let mut newer_queue = EcsRendererHandoffQueue::default();
        newer_queue.push(newer).expect("newer handoff");
        assert_eq!(
            renderer_import_terrain_artifacts(&newer_queue, &mut table).imported,
            1
        );

        let stale = handoff(2, RendererArtifactHandoffKind::TerrainSurfacePackets);
        let mut stale_queue = EcsRendererHandoffQueue::default();
        stale_queue.push(stale).expect("stale handoff");
        let stale_report = renderer_import_terrain_artifacts(&stale_queue, &mut table);
        assert_eq!(stale_report.rejected_stale_epoch, 1);
    }

    #[test]
    fn renderer_upload_terrain_pages_respects_upload_budget() {
        let mut queue = EcsRendererHandoffQueue::default();
        queue
            .push(handoff(
                1,
                RendererArtifactHandoffKind::TerrainSurfacePackets,
            ))
            .expect("surface");
        queue
            .push(handoff(2, RendererArtifactHandoffKind::TerrainMaterialPage))
            .expect("material");
        let mut table = RendererTerrainArtifactTable::default();
        renderer_import_terrain_artifacts(&queue, &mut table);

        let report = renderer_upload_terrain_pages(
            &mut table,
            RendererTerrainUploadBudget {
                max_upload_bytes: 128 * 1024,
                max_tickets: 1,
            },
        );

        assert_eq!(report.uploaded, 1);
        assert_eq!(report.deferred_by_budget, 1);
        assert_eq!(table.pending_uploads.len(), 1);
        assert_eq!(
            table
                .get(EcsDerivedArtifactId::new(1))
                .map(|record| record.upload_state),
            Some(RendererTerrainUploadState::Uploaded)
        );
    }

    #[test]
    fn renderer_publishes_surface_packets_through_gpu_scene_indirect_fallback() {
        let surface_handoff = handoff(1, RendererArtifactHandoffKind::TerrainSurfacePackets);
        let mut queue = EcsRendererHandoffQueue::default();
        queue.push(surface_handoff).expect("surface handoff");
        let mut table = RendererTerrainArtifactTable::default();
        renderer_import_terrain_artifacts(&queue, &mut table);
        renderer_upload_terrain_pages(&mut table, RendererTerrainUploadBudget::default());

        let packet = terrain_surface_packet_for_handoff(
            surface_handoff,
            12,
            PackedRange::new(4, 12),
            PackedAabb::new([0, 0, 0], [32, 32, 32]),
            TerrainMaterialPaletteId::new(3),
            None,
        );
        let report = renderer_publish_virtual_geometry(&mut table, &[packet], false);
        let record = table.get(EcsDerivedArtifactId::new(1)).expect("record");

        assert_eq!(report.gpu_scene_indirect_fallbacks, 1);
        assert_eq!(
            record.realization,
            RendererTerrainRealization::GpuSceneIndirectFallbackPublished
        );
        assert_eq!(
            record.publish_path,
            Some(RendererTerrainPublishPath::GpuSceneIndirectFallback)
        );
        assert_eq!(record.published_packet_count, 12);
        assert_eq!(packet.source_page, surface_handoff.source_page);
    }

    #[test]
    fn renderer_publish_load_animation_writes_uploaded_animation_records() {
        let mut queue = EcsRendererHandoffQueue::default();
        queue
            .push(handoff(
                4,
                RendererArtifactHandoffKind::LoadAnimationRecords,
            ))
            .expect("load animation");
        let mut table = RendererTerrainArtifactTable::default();
        renderer_import_terrain_artifacts(&queue, &mut table);
        renderer_upload_terrain_pages(&mut table, RendererTerrainUploadBudget::default());

        let mut buffer = RendererLoadAnimationBuffer::default();
        let report = renderer_publish_load_animation(
            &mut table,
            &[load_animation_artifact()],
            &[load_wave()],
            &mut buffer,
        );

        assert_eq!(report.published, 1);
        assert_eq!(buffer.rows.len(), 1);
        assert_eq!(buffer.rows[0].artifact_id, EcsDerivedArtifactId::new(4));
        assert_eq!(buffer.rows[0].style, LoadAnimationStyle::RadialCameraWave);
        assert_eq!(buffer.rows[0].state, LoadAnimationState::FineDitherMorph);
        assert_eq!(buffer.rows[0].reveal_origin_ws, [64.0, 96.0, 128.0]);
        assert_eq!(buffer.rows[0].progress, 0.5);
        assert_eq!(buffer.temporal_history_reset_epoch, 12);
        assert_eq!(
            table
                .get(EcsDerivedArtifactId::new(4))
                .map(|record| record.realization),
            Some(RendererTerrainRealization::LoadAnimationPublished)
        );
    }

    #[test]
    fn renderer_retires_gpu_artifacts_when_ecs_generation_changes_or_evicts() {
        let mut queue = EcsRendererHandoffQueue::default();
        queue
            .push(handoff(
                1,
                RendererArtifactHandoffKind::TerrainSurfacePackets,
            ))
            .expect("surface");
        let mut table = RendererTerrainArtifactTable::default();
        renderer_import_terrain_artifacts(&queue, &mut table);
        renderer_upload_terrain_pages(&mut table, RendererTerrainUploadBudget::default());

        let report = renderer_retire_terrain_artifacts(
            &mut table,
            &[RendererTerrainArtifactRetireRequest {
                source_page: page(),
                live_source_epoch: 8,
                live_artifact_epoch: 9,
                reason: RendererTerrainRetireReason::GenerationChanged,
            }],
        );

        assert_eq!(report.retired, 1);
        assert_eq!(
            table
                .get(EcsDerivedArtifactId::new(1))
                .map(|record| record.realization),
            Some(RendererTerrainRealization::Retired)
        );
    }

    #[test]
    fn renderer_system_catalog_matches_ecs_handoff_pipeline() {
        let labels: Vec<&'static str> = RendererTerrainHandoffSystem::all()
            .iter()
            .map(|system| system.label())
            .collect();
        assert_eq!(labels.len(), RENDERER_TERRAIN_HANDOFF_SYSTEM_COUNT);
        assert_eq!(
            labels,
            vec![
                "renderer_import_terrain_artifacts",
                "renderer_upload_terrain_pages",
                "renderer_publish_virtual_geometry",
                "renderer_publish_load_animation",
                "renderer_retire_terrain_artifacts",
            ]
        );
    }
}
