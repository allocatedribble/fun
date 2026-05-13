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
    pub source_digest: u64,
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
    pub source_digest: u64,
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

pub const PROCEDURAL_TERRAIN_RENDERER_GENERATES_TERRAIN_TRUTH: bool = false;
pub const PROCEDURAL_TERRAIN_RENDERER_HANDOFF_KIND_COUNT: usize = 4;
pub const PROCEDURAL_TERRAIN_RENDERER_HANDOFF_KINDS: [RendererArtifactHandoffKind;
    PROCEDURAL_TERRAIN_RENDERER_HANDOFF_KIND_COUNT] = [
    RendererArtifactHandoffKind::TerrainCoarseProxy,
    RendererArtifactHandoffKind::TerrainSurfacePackets,
    RendererArtifactHandoffKind::TerrainMaterialPage,
    RendererArtifactHandoffKind::LoadAnimationRecords,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProceduralTerrainRendererImportPlan {
    pub source_page: EcsSpatialPageKey,
    pub surface_artifact: Option<EcsDerivedArtifactId>,
    pub coarse_artifact: Option<EcsDerivedArtifactId>,
    pub material_artifact: Option<EcsDerivedArtifactId>,
    pub load_animation_artifact: Option<EcsDerivedArtifactId>,
    pub source_epoch: u32,
}

impl ProceduralTerrainRendererImportPlan {
    #[must_use]
    pub const fn empty_for_page(source_page: EcsSpatialPageKey, source_epoch: u32) -> Self {
        Self {
            source_page,
            surface_artifact: None,
            coarse_artifact: None,
            material_artifact: None,
            load_animation_artifact: None,
            source_epoch,
        }
    }

    #[must_use]
    pub fn has_renderable_geometry(self) -> bool {
        self.surface_artifact.is_some() || self.coarse_artifact.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProceduralTerrainRendererPageStamp {
    pub source_page: EcsSpatialPageKey,
    pub source_epoch: u32,
    pub source_digest: u64,
}

impl ProceduralTerrainRendererPageStamp {
    #[must_use]
    pub const fn new(
        source_page: EcsSpatialPageKey,
        source_epoch: u32,
        source_digest: u64,
    ) -> Self {
        Self {
            source_page,
            source_epoch,
            source_digest,
        }
    }

    #[must_use]
    pub fn matches_handoff(self, handoff: RendererArtifactHandoff) -> bool {
        self.source_page == handoff.source_page
            && self.source_epoch == handoff.source_epoch
            && self.source_digest == handoff.source_digest
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ProceduralTerrainRendererImportPlanReport {
    pub inspected: u32,
    pub consumed_handoffs: u32,
    pub plans_built: u32,
    pub rejected_non_terrain_handoff: u32,
    pub rejected_zero_epoch: u32,
    pub rejected_zero_source_digest: u32,
    pub rejected_stale_epoch: u32,
    pub rejected_digest_mismatch: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProceduralTerrainImportAccumulator {
    plan: ProceduralTerrainRendererImportPlan,
    source_digest: u64,
    surface_epoch: u32,
    coarse_epoch: u32,
    material_epoch: u32,
    load_epoch: u32,
}

impl ProceduralTerrainImportAccumulator {
    const fn new(stamp: ProceduralTerrainRendererPageStamp) -> Self {
        Self {
            plan: ProceduralTerrainRendererImportPlan::empty_for_page(
                stamp.source_page,
                stamp.source_epoch,
            ),
            source_digest: stamp.source_digest,
            surface_epoch: 0,
            coarse_epoch: 0,
            material_epoch: 0,
            load_epoch: 0,
        }
    }

    fn attach(
        &mut self,
        handoff: RendererArtifactHandoff,
        report: &mut ProceduralTerrainRendererImportPlanReport,
    ) {
        let (slot, epoch) = match handoff.kind {
            RendererArtifactHandoffKind::TerrainSurfacePackets => {
                (&mut self.plan.surface_artifact, &mut self.surface_epoch)
            }
            RendererArtifactHandoffKind::TerrainCoarseProxy => {
                (&mut self.plan.coarse_artifact, &mut self.coarse_epoch)
            }
            RendererArtifactHandoffKind::TerrainMaterialPage => {
                (&mut self.plan.material_artifact, &mut self.material_epoch)
            }
            RendererArtifactHandoffKind::LoadAnimationRecords => {
                (&mut self.plan.load_animation_artifact, &mut self.load_epoch)
            }
            _ => unreachable!("procedural terrain handoff kind was prefiltered"),
        };
        if *epoch > handoff.artifact_epoch {
            report.rejected_stale_epoch = report.rejected_stale_epoch.saturating_add(1);
            return;
        }
        *slot = Some(handoff.artifact_id);
        *epoch = handoff.artifact_epoch;
        report.consumed_handoffs = report.consumed_handoffs.saturating_add(1);
    }

    fn stamp(&self) -> ProceduralTerrainRendererPageStamp {
        ProceduralTerrainRendererPageStamp {
            source_page: self.plan.source_page,
            source_epoch: self.plan.source_epoch,
            source_digest: self.source_digest,
        }
    }
}

#[must_use]
pub fn procedural_terrain_renderer_consumes_handoff_kind(
    kind: RendererArtifactHandoffKind,
) -> bool {
    matches!(
        kind,
        RendererArtifactHandoffKind::TerrainCoarseProxy
            | RendererArtifactHandoffKind::TerrainSurfacePackets
            | RendererArtifactHandoffKind::TerrainMaterialPage
            | RendererArtifactHandoffKind::LoadAnimationRecords
    )
}

#[must_use]
pub fn build_procedural_terrain_renderer_import_plans(
    handoffs: &[RendererArtifactHandoff],
    expected_stamps: &[ProceduralTerrainRendererPageStamp],
) -> (
    Vec<ProceduralTerrainRendererImportPlan>,
    ProceduralTerrainRendererImportPlanReport,
) {
    let mut report = ProceduralTerrainRendererImportPlanReport::default();
    let mut accumulators: Vec<ProceduralTerrainImportAccumulator> = Vec::new();

    for handoff in handoffs {
        report.inspected = report.inspected.saturating_add(1);
        if !procedural_terrain_renderer_consumes_handoff_kind(handoff.kind) {
            report.rejected_non_terrain_handoff =
                report.rejected_non_terrain_handoff.saturating_add(1);
            continue;
        }
        if handoff.source_epoch == 0 || handoff.artifact_epoch == 0 {
            report.rejected_zero_epoch = report.rejected_zero_epoch.saturating_add(1);
            continue;
        }
        if handoff.source_digest == 0 {
            report.rejected_zero_source_digest =
                report.rejected_zero_source_digest.saturating_add(1);
            continue;
        }

        if let Some(expected) = expected_stamps
            .iter()
            .find(|stamp| stamp.source_page == handoff.source_page)
        {
            if expected.source_epoch != handoff.source_epoch {
                report.rejected_stale_epoch = report.rejected_stale_epoch.saturating_add(1);
                continue;
            }
            if !expected.matches_handoff(*handoff) {
                report.rejected_digest_mismatch = report.rejected_digest_mismatch.saturating_add(1);
                continue;
            }
        }

        let stamp = ProceduralTerrainRendererPageStamp::new(
            handoff.source_page,
            handoff.source_epoch,
            handoff.source_digest,
        );
        let Some(index) = accumulators
            .iter()
            .position(|acc| acc.plan.source_page == handoff.source_page)
        else {
            let mut accumulator = ProceduralTerrainImportAccumulator::new(stamp);
            accumulator.attach(*handoff, &mut report);
            accumulators.push(accumulator);
            continue;
        };

        let existing = accumulators[index].stamp();
        if existing.source_epoch > handoff.source_epoch {
            report.rejected_stale_epoch = report.rejected_stale_epoch.saturating_add(1);
            continue;
        }
        if existing.source_epoch < handoff.source_epoch {
            accumulators[index] = ProceduralTerrainImportAccumulator::new(stamp);
        } else if existing.source_digest != handoff.source_digest {
            report.rejected_digest_mismatch = report.rejected_digest_mismatch.saturating_add(1);
            continue;
        }
        accumulators[index].attach(*handoff, &mut report);
    }

    let plans = accumulators
        .into_iter()
        .map(|accumulator| accumulator.plan)
        .collect::<Vec<_>>();
    report.plans_built = plans.len() as u32;
    (plans, report)
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ProceduralTerrainRenderMode {
    #[default]
    SurfacePackets = 0,
    CoarseProxy = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProceduralTerrainDebugVisual {
    pub material_color_rgba8: [u8; 4],
    pub page_border_rgba8: [u8; 4],
    pub stream_shell_rgba8: [u8; 4],
    pub basic_light_dir_q8: [i8; 3],
    pub ambient_q8: u8,
    pub diffuse_q8: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProceduralTerrainRenderItem {
    pub source_page: EcsSpatialPageKey,
    pub mode: ProceduralTerrainRenderMode,
    pub geometry_artifact: EcsDerivedArtifactId,
    pub material_artifact: Option<EcsDerivedArtifactId>,
    pub load_animation_artifact: Option<EcsDerivedArtifactId>,
    pub source_epoch: u32,
    pub visual: ProceduralTerrainDebugVisual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProceduralTerrainStreamingHole {
    pub source_page: EcsSpatialPageKey,
    pub source_epoch: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ProceduralTerrainRendererRealizationReport {
    pub inspected_plans: u32,
    pub surface_items: u32,
    pub coarse_items: u32,
    pub streaming_holes_logged: u32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ProceduralTerrainRendererRealizationBatch {
    pub render_items: Vec<ProceduralTerrainRenderItem>,
    pub streaming_holes: Vec<ProceduralTerrainStreamingHole>,
    pub report: ProceduralTerrainRendererRealizationReport,
}

#[must_use]
pub fn realize_procedural_terrain_import_plans(
    plans: &[ProceduralTerrainRendererImportPlan],
    requested_pages: &[ProceduralTerrainRendererPageStamp],
) -> ProceduralTerrainRendererRealizationBatch {
    let mut batch = ProceduralTerrainRendererRealizationBatch::default();
    for plan in plans {
        batch.report.inspected_plans = batch.report.inspected_plans.saturating_add(1);
        let Some((mode, geometry_artifact)) = render_mode_and_artifact(*plan) else {
            push_streaming_hole(
                &mut batch,
                ProceduralTerrainStreamingHole {
                    source_page: plan.source_page,
                    source_epoch: plan.source_epoch,
                },
            );
            continue;
        };

        match mode {
            ProceduralTerrainRenderMode::SurfacePackets => {
                batch.report.surface_items = batch.report.surface_items.saturating_add(1);
            }
            ProceduralTerrainRenderMode::CoarseProxy => {
                batch.report.coarse_items = batch.report.coarse_items.saturating_add(1);
            }
        }
        batch.render_items.push(ProceduralTerrainRenderItem {
            source_page: plan.source_page,
            mode,
            geometry_artifact,
            material_artifact: plan.material_artifact,
            load_animation_artifact: plan.load_animation_artifact,
            source_epoch: plan.source_epoch,
            visual: debug_visual_for_plan(*plan, mode),
        });
    }

    for requested in requested_pages {
        if plans.iter().any(|plan| {
            plan.source_page == requested.source_page
                && plan.source_epoch == requested.source_epoch
                && plan.has_renderable_geometry()
        }) {
            continue;
        }
        push_streaming_hole(
            &mut batch,
            ProceduralTerrainStreamingHole {
                source_page: requested.source_page,
                source_epoch: requested.source_epoch,
            },
        );
    }

    batch
}

fn render_mode_and_artifact(
    plan: ProceduralTerrainRendererImportPlan,
) -> Option<(ProceduralTerrainRenderMode, EcsDerivedArtifactId)> {
    if let Some(surface) = plan.surface_artifact {
        Some((ProceduralTerrainRenderMode::SurfacePackets, surface))
    } else {
        plan.coarse_artifact
            .map(|coarse| (ProceduralTerrainRenderMode::CoarseProxy, coarse))
    }
}

fn push_streaming_hole(
    batch: &mut ProceduralTerrainRendererRealizationBatch,
    hole: ProceduralTerrainStreamingHole,
) {
    if batch.streaming_holes.contains(&hole) {
        return;
    }
    batch.streaming_holes.push(hole);
    batch.report.streaming_holes_logged = batch.report.streaming_holes_logged.saturating_add(1);
}

fn debug_visual_for_plan(
    plan: ProceduralTerrainRendererImportPlan,
    mode: ProceduralTerrainRenderMode,
) -> ProceduralTerrainDebugVisual {
    ProceduralTerrainDebugVisual {
        material_color_rgba8: material_color_for_plan(plan, mode),
        page_border_rgba8: page_border_color(plan.source_page),
        stream_shell_rgba8: stream_shell_color(plan.source_page),
        basic_light_dir_q8: [-45, 96, -45],
        ambient_q8: 48,
        diffuse_q8: 176,
    }
}

fn material_color_for_plan(
    plan: ProceduralTerrainRendererImportPlan,
    mode: ProceduralTerrainRenderMode,
) -> [u8; 4] {
    if plan.material_artifact.is_some() {
        [86, 142, 72, 255]
    } else {
        match mode {
            ProceduralTerrainRenderMode::SurfacePackets => [126, 104, 78, 255],
            ProceduralTerrainRenderMode::CoarseProxy => [92, 110, 122, 255],
        }
    }
}

fn page_border_color(page: EcsSpatialPageKey) -> [u8; 4] {
    let hash = page.chunk_key().get();
    [
        96_u8.saturating_add((hash & 0x3f) as u8),
        96_u8.saturating_add(((hash >> 8) & 0x3f) as u8),
        96_u8.saturating_add(((hash >> 16) & 0x3f) as u8),
        255,
    ]
}

fn stream_shell_color(page: EcsSpatialPageKey) -> [u8; 4] {
    let shell = page.x.unsigned_abs() + page.y.unsigned_abs() + page.z.unsigned_abs();
    match shell & 3 {
        0 => [80, 165, 230, 255],
        1 => [92, 196, 128, 255],
        2 => [224, 184, 72, 255],
        _ => [220, 112, 104, 255],
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RendererTerrainImportReport {
    pub inspected: u32,
    pub imported: u32,
    pub coalesced: u32,
    pub rejected_zero_epoch: u32,
    pub rejected_zero_source_digest: u32,
    pub rejected_digest_mismatch: u32,
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
        if handoff.source_digest == 0 {
            report.rejected_zero_source_digest =
                report.rejected_zero_source_digest.saturating_add(1);
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
                && existing.source_digest != handoff.source_digest
            {
                report.rejected_digest_mismatch = report.rejected_digest_mismatch.saturating_add(1);
                continue;
            }
            if existing.source_epoch == handoff.source_epoch
                && existing.artifact_epoch == handoff.artifact_epoch
                && existing.source_digest == handoff.source_digest
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
        source_digest: handoff.source_digest,
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
        source_digest: ticket.source_digest,
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
        page_at(2, 3, 4)
    }

    fn page_at(x: i32, y: i32, z: i32) -> EcsSpatialPageKey {
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

    fn handoff(id: u64, kind: RendererArtifactHandoffKind) -> RendererArtifactHandoff {
        RendererArtifactHandoff {
            artifact_id: EcsDerivedArtifactId::new(id),
            source_page: page(),
            kind,
            source_epoch: 7,
            source_digest: 0x1776_0000_0000_0007,
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
    fn renderer_import_rejects_zero_digest_or_same_epoch_digest_mismatch() {
        let mut zero_digest_queue = EcsRendererHandoffQueue::default();
        let mut zero_digest = handoff(1, RendererArtifactHandoffKind::TerrainSurfacePackets);
        zero_digest.source_digest = 0;
        zero_digest_queue
            .push(zero_digest)
            .expect("zero digest handoff");
        let mut table = RendererTerrainArtifactTable::default();
        let zero_report = renderer_import_terrain_artifacts(&zero_digest_queue, &mut table);
        assert_eq!(zero_report.rejected_zero_source_digest, 1);
        assert!(table.records.is_empty());

        let first = handoff(2, RendererArtifactHandoffKind::TerrainSurfacePackets);
        let mut first_queue = EcsRendererHandoffQueue::default();
        first_queue.push(first).expect("first handoff");
        assert_eq!(
            renderer_import_terrain_artifacts(&first_queue, &mut table).imported,
            1
        );

        let mut mismatch = first;
        mismatch.source_digest ^= 0x55;
        mismatch.artifact_epoch = mismatch.artifact_epoch.saturating_add(1);
        let mut mismatch_queue = EcsRendererHandoffQueue::default();
        mismatch_queue.push(mismatch).expect("mismatch handoff");
        let mismatch_report = renderer_import_terrain_artifacts(&mismatch_queue, &mut table);
        assert_eq!(mismatch_report.rejected_digest_mismatch, 1);
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
    fn procedural_terrain_renderer_import_plan_groups_required_handoffs_and_prefers_surface() {
        let surface = handoff(10, RendererArtifactHandoffKind::TerrainSurfacePackets);
        let coarse = handoff(11, RendererArtifactHandoffKind::TerrainCoarseProxy);
        let material = handoff(12, RendererArtifactHandoffKind::TerrainMaterialPage);
        let load = handoff(13, RendererArtifactHandoffKind::LoadAnimationRecords);
        let expected = ProceduralTerrainRendererPageStamp::new(
            surface.source_page,
            surface.source_epoch,
            surface.source_digest,
        );

        let (plans, report) = build_procedural_terrain_renderer_import_plans(
            &[coarse, material, load, surface],
            &[expected],
        );
        assert_eq!(report.consumed_handoffs, 4);
        assert_eq!(report.plans_built, 1);
        assert_eq!(plans.len(), 1);
        assert_eq!(
            plans[0].surface_artifact,
            Some(EcsDerivedArtifactId::new(10))
        );
        assert_eq!(
            plans[0].coarse_artifact,
            Some(EcsDerivedArtifactId::new(11))
        );
        assert_eq!(
            plans[0].material_artifact,
            Some(EcsDerivedArtifactId::new(12))
        );
        assert_eq!(
            plans[0].load_animation_artifact,
            Some(EcsDerivedArtifactId::new(13))
        );

        let batch = realize_procedural_terrain_import_plans(&plans, &[expected]);
        assert_eq!(batch.render_items.len(), 1);
        assert!(batch.streaming_holes.is_empty());
        assert_eq!(
            batch.render_items[0].mode,
            ProceduralTerrainRenderMode::SurfacePackets
        );
        assert_eq!(
            batch.render_items[0].geometry_artifact,
            EcsDerivedArtifactId::new(10)
        );
        assert_eq!(
            batch.render_items[0].visual.material_color_rgba8,
            [86, 142, 72, 255]
        );
        assert!(batch.render_items[0].visual.diffuse_q8 > batch.render_items[0].visual.ambient_q8);
    }

    #[test]
    fn procedural_terrain_renderer_uses_coarse_proxy_and_logs_streaming_holes() {
        let coarse = handoff(21, RendererArtifactHandoffKind::TerrainCoarseProxy);
        let missing_stamp = ProceduralTerrainRendererPageStamp::new(
            page_at(9, 0, 1),
            coarse.source_epoch,
            coarse.source_digest,
        );
        let expected = ProceduralTerrainRendererPageStamp::new(
            coarse.source_page,
            coarse.source_epoch,
            coarse.source_digest,
        );
        let (plans, report) =
            build_procedural_terrain_renderer_import_plans(&[coarse], &[expected, missing_stamp]);

        assert_eq!(report.plans_built, 1);
        let batch = realize_procedural_terrain_import_plans(&plans, &[expected, missing_stamp]);
        assert_eq!(batch.render_items.len(), 1);
        assert_eq!(batch.streaming_holes.len(), 1);
        assert_eq!(
            batch.render_items[0].mode,
            ProceduralTerrainRenderMode::CoarseProxy
        );
        assert_eq!(batch.report.coarse_items, 1);
        assert_eq!(batch.report.streaming_holes_logged, 1);
    }

    #[test]
    fn procedural_terrain_renderer_import_rejects_epoch_and_digest_mismatches() {
        let mut digest_mismatch = handoff(31, RendererArtifactHandoffKind::TerrainSurfacePackets);
        let expected = ProceduralTerrainRendererPageStamp::new(
            digest_mismatch.source_page,
            digest_mismatch.source_epoch,
            digest_mismatch.source_digest ^ 0xff,
        );
        let mut stale_epoch = digest_mismatch;
        stale_epoch.artifact_id = EcsDerivedArtifactId::new(32);
        stale_epoch.source_epoch = stale_epoch.source_epoch.saturating_sub(1);
        digest_mismatch.source_digest ^= 0xaa;

        let (plans, report) = build_procedural_terrain_renderer_import_plans(
            &[digest_mismatch, stale_epoch],
            &[expected],
        );

        assert!(plans.is_empty());
        assert_eq!(report.rejected_digest_mismatch, 1);
        assert_eq!(report.rejected_stale_epoch, 1);
    }

    #[test]
    fn procedural_terrain_renderer_policy_consumes_artifacts_without_generating_truth() {
        const { assert!(!PROCEDURAL_TERRAIN_RENDERER_GENERATES_TERRAIN_TRUTH) };
        assert_eq!(
            PROCEDURAL_TERRAIN_RENDERER_HANDOFF_KINDS,
            [
                RendererArtifactHandoffKind::TerrainCoarseProxy,
                RendererArtifactHandoffKind::TerrainSurfacePackets,
                RendererArtifactHandoffKind::TerrainMaterialPage,
                RendererArtifactHandoffKind::LoadAnimationRecords,
            ]
        );
        assert!(procedural_terrain_renderer_consumes_handoff_kind(
            RendererArtifactHandoffKind::TerrainSurfacePackets
        ));
        assert!(!procedural_terrain_renderer_consumes_handoff_kind(
            RendererArtifactHandoffKind::TerrainSdfPage
        ));
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
