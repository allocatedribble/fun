use crate::{Component, Resource};
use fun_scheduler_types::{EcsChunkKey, EcsEntityId};

use crate::{
    ECS_SPATIAL_MAX_STREAM_INTERESTS, ECS_SPATIAL_MAX_STREAM_REQUESTS,
    ECS_SPATIAL_MAX_STREAM_WAVES, EcsPageChannel, EcsPageResidencyState, EcsPageResidencyTable,
    EcsSpatialCommand, EcsSpatialCommandBuffer, EcsSpatialDomainKind, EcsSpatialGridId,
    EcsSpatialGridRegistry, EcsSpatialPageKey, EcsSpatialValidationError, EcsStreamCameraId,
    EcsStreamRequestId, EcsStreamSourceId, EcsStreamWaveId, RingBuffer, SpatialStreamCamera,
    StreamCameraRole,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IVec3 {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl IVec3 {
    #[must_use]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    #[must_use]
    pub const fn zero() -> Self {
        Self { x: 0, y: 0, z: 0 }
    }

    #[must_use]
    pub fn chebyshev_len(self) -> u16 {
        self.x
            .unsigned_abs()
            .max(self.y.unsigned_abs())
            .max(self.z.unsigned_abs())
            .min(u32::from(u16::MAX)) as u16
    }

    #[must_use]
    pub const fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsViewFrustum {
    pub planes_q: [[i32; 4]; 6],
}

impl EcsViewFrustum {
    pub const EMPTY: Self = Self {
        planes_q: [[0; 4]; 6],
    };
}

impl Default for EcsViewFrustum {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EcsCameraTransformSample {
    pub camera_entity: EcsEntityId,
    pub camera_id: EcsStreamCameraId,
    pub origin_world_ft: IVec3,
    pub velocity_world_ft_s: IVec3,
    pub view_frustum: EcsViewFrustum,
    pub cut_or_teleport: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EcsStreamSourceDescriptor {
    pub source: EcsStreamSourceId,
    pub domain: EcsSpatialDomainKind,
    pub grid_id: EcsSpatialGridId,
    pub priority: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EcsActiveStreamCameraSnapshot {
    pub camera_entity: EcsEntityId,
    pub camera_id: EcsStreamCameraId,
    pub role: StreamCameraRole,
    pub priority: u8,
    pub origin_world_ft: IVec3,
    pub velocity_world_ft_s: IVec3,
    pub required_shells: u16,
    pub desired_shells: u16,
    pub velocity_lookahead_s: f32,
    pub view_frustum: EcsViewFrustum,
    pub cut_or_teleport: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Resource)]
pub struct EcsStreamingSourceSnapshot {
    pub frame: u64,
    pub active_cameras: Vec<EcsActiveStreamCameraSnapshot>,
    pub active_terrain_grids: Vec<EcsSpatialGridId>,
    pub active_sources: Vec<EcsStreamSourceDescriptor>,
}

impl EcsStreamingSourceSnapshot {
    #[must_use]
    pub fn from_inputs(
        frame: u64,
        cameras: &[(SpatialStreamCamera, EcsCameraTransformSample)],
        sources: &[EcsStreamSourceDescriptor],
    ) -> Self {
        let mut active_cameras = Vec::new();
        for (camera, transform) in cameras {
            if camera.enabled && camera.role != StreamCameraRole::Disabled {
                active_cameras.push(EcsActiveStreamCameraSnapshot {
                    camera_entity: transform.camera_entity,
                    camera_id: transform.camera_id,
                    role: camera.role,
                    priority: camera.priority,
                    origin_world_ft: transform.origin_world_ft,
                    velocity_world_ft_s: transform.velocity_world_ft_s,
                    required_shells: camera.required_shells,
                    desired_shells: camera.desired_shells.max(camera.required_shells),
                    velocity_lookahead_s: camera.velocity_lookahead_s,
                    view_frustum: transform.view_frustum,
                    cut_or_teleport: transform.cut_or_teleport,
                });
            }
        }

        active_cameras.sort_by_key(|camera| {
            (
                core::cmp::Reverse(camera.priority),
                camera.camera_entity.get(),
                camera.camera_id.get(),
            )
        });

        let mut active_sources = sources.to_vec();
        active_sources.sort_by_key(|source| {
            (
                source.domain as u8,
                source.grid_id.get(),
                core::cmp::Reverse(source.priority),
                source.source.get(),
            )
        });

        let mut active_terrain_grids = Vec::new();
        for source in &active_sources {
            if source.domain == EcsSpatialDomainKind::Terrain
                && !active_terrain_grids.contains(&source.grid_id)
            {
                active_terrain_grids.push(source.grid_id);
            }
        }
        active_terrain_grids.sort();

        Self {
            frame,
            active_cameras,
            active_terrain_grids,
            active_sources,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Component)]
pub struct EcsStreamCamera {
    pub id: EcsStreamCameraId,
    pub chunk_key: EcsChunkKey,
    pub radius_pages: u16,
    pub max_requests_per_frame: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct EcsStreamSource {
    pub id: EcsStreamSourceId,
    pub domain: EcsSpatialDomainKind,
    pub grid_id: EcsSpatialGridId,
    pub chunk_key: EcsChunkKey,
    pub priority: u8,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsStreamInterestKind {
    #[default]
    CameraContainingPage = 0,
    CollisionCriticalNear = 1,
    VisibleNear = 2,
    VisibleCoarseFallback = 3,
    VelocityLookahead = 4,
    ShadowCritical = 5,
    PhysicsCook = 6,
    FoliageNear = 7,
    Backfill = 8,
    Diagnostics = 9,
}

impl EcsStreamInterestKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::CameraContainingPage => "camera_containing_page",
            Self::CollisionCriticalNear => "collision_critical_near",
            Self::VisibleNear => "visible_near",
            Self::VisibleCoarseFallback => "visible_coarse_fallback",
            Self::VelocityLookahead => "velocity_lookahead",
            Self::ShadowCritical => "shadow_critical",
            Self::PhysicsCook => "physics_cook",
            Self::FoliageNear => "foliage_near",
            Self::Backfill => "backfill",
            Self::Diagnostics => "diagnostics",
        }
    }

    #[must_use]
    pub const fn priority_rank(self) -> u8 {
        match self {
            Self::CameraContainingPage => 0,
            Self::CollisionCriticalNear => 1,
            Self::VisibleCoarseFallback => 2,
            Self::VelocityLookahead => 3,
            Self::VisibleNear => 4,
            Self::ShadowCritical => 5,
            Self::PhysicsCook => 6,
            Self::FoliageNear => 7,
            Self::Backfill => 8,
            Self::Diagnostics => 9,
        }
    }

    #[must_use]
    pub const fn is_required(self) -> bool {
        matches!(
            self,
            Self::CameraContainingPage
                | Self::CollisionCriticalNear
                | Self::VisibleNear
                | Self::VisibleCoarseFallback
                | Self::VelocityLookahead
                | Self::ShadowCritical
                | Self::PhysicsCook
        )
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsStreamPriority {
    pub criticality: EcsStreamInterestKind,
    pub shell: u16,
    pub level: u8,
    pub screen_error_q: u16,
    pub view_score_q: u16,
    pub velocity_score_q: u16,
    pub gameplay_score_q: u16,
    pub starvation_age_q: u16,
}

impl EcsStreamPriority {
    #[must_use]
    pub const fn new(criticality: EcsStreamInterestKind, shell: u16, level: u8) -> Self {
        Self {
            criticality,
            shell,
            level,
            screen_error_q: 0,
            view_score_q: 0,
            velocity_score_q: 0,
            gameplay_score_q: 0,
            starvation_age_q: 0,
        }
    }

    #[must_use]
    pub const fn with_scores(
        mut self,
        screen_error_q: u16,
        view_score_q: u16,
        velocity_score_q: u16,
        gameplay_score_q: u16,
        starvation_age_q: u16,
    ) -> Self {
        self.screen_error_q = screen_error_q;
        self.view_score_q = view_score_q;
        self.velocity_score_q = velocity_score_q;
        self.gameplay_score_q = gameplay_score_q;
        self.starvation_age_q = starvation_age_q;
        self
    }

    #[must_use]
    pub fn order_key(self) -> EcsStreamPriorityOrderKey {
        EcsStreamPriorityOrderKey {
            shell: self.shell,
            required_rank: if self.criticality.is_required() { 0 } else { 1 },
            criticality_rank: self.criticality.priority_rank(),
            level_rank: core::cmp::Reverse(self.level),
            screen_error_rank: core::cmp::Reverse(self.screen_error_q),
            view_rank: core::cmp::Reverse(self.view_score_q),
            velocity_rank: core::cmp::Reverse(self.velocity_score_q),
            gameplay_rank: core::cmp::Reverse(self.gameplay_score_q),
            starvation_rank: core::cmp::Reverse(self.starvation_age_q),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EcsStreamPriorityOrderKey {
    shell: u16,
    required_rank: u8,
    criticality_rank: u8,
    level_rank: core::cmp::Reverse<u8>,
    screen_error_rank: core::cmp::Reverse<u16>,
    view_rank: core::cmp::Reverse<u16>,
    velocity_rank: core::cmp::Reverse<u16>,
    gameplay_rank: core::cmp::Reverse<u16>,
    starvation_rank: core::cmp::Reverse<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EcsStreamRequest {
    pub id: EcsStreamRequestId,
    pub source: EcsStreamSourceId,
    pub camera: EcsStreamCameraId,
    pub page: EcsSpatialPageKey,
    pub chunk_key: EcsChunkKey,
    pub priority: EcsStreamPriority,
}

impl EcsStreamRequest {
    #[must_use]
    pub fn new(
        id: EcsStreamRequestId,
        source: EcsStreamSourceId,
        camera: EcsStreamCameraId,
        page: EcsSpatialPageKey,
        priority: EcsStreamPriority,
    ) -> Self {
        Self {
            id,
            source,
            camera,
            page,
            chunk_key: page.chunk_key(),
            priority,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct EcsStreamRequestQueue {
    pub requests: Vec<EcsStreamRequest>,
}

impl EcsStreamRequestQueue {
    pub fn push(&mut self, request: EcsStreamRequest) -> Result<(), EcsSpatialValidationError> {
        if self.requests.len() >= ECS_SPATIAL_MAX_STREAM_REQUESTS {
            return Err(EcsSpatialValidationError::StreamRequestQueueFull);
        }
        self.requests.push(request);
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StreamWaveReason {
    #[default]
    ColdStart = 0,
    CameraMoved = 1,
    TeleportOrCut = 2,
    DebugPin = 3,
    Manual = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EcsStreamWaveRecord {
    pub wave_id: EcsStreamWaveId,
    pub origin_page: EcsSpatialPageKey,
    pub origin_world_ft: IVec3,
    pub camera_entity: EcsEntityId,
    pub created_frame: u64,
    pub max_shell_planned: u16,
    pub reason: StreamWaveReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Resource)]
pub struct EcsStreamWaveLedger {
    pub active_wave: EcsStreamWaveId,
    pub waves: RingBuffer<EcsStreamWaveRecord>,
}

impl Default for EcsStreamWaveLedger {
    fn default() -> Self {
        Self {
            active_wave: EcsStreamWaveId::INVALID,
            waves: RingBuffer::with_capacity(ECS_SPATIAL_MAX_STREAM_WAVES),
        }
    }
}

impl EcsStreamWaveLedger {
    #[must_use]
    pub fn next_wave_id(&self) -> EcsStreamWaveId {
        EcsStreamWaveId::new(self.active_wave.get().saturating_add(1).max(1))
    }

    pub fn push(&mut self, record: EcsStreamWaveRecord) {
        self.active_wave = record.wave_id;
        self.waves.push(record);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EcsStreamInterest {
    pub camera: EcsStreamCameraId,
    pub camera_entity: EcsEntityId,
    pub page: EcsSpatialPageKey,
    pub kind: EcsStreamInterestKind,
    pub priority: EcsStreamPriority,
    pub shell: u16,
    pub required: bool,
}

impl EcsStreamInterest {
    #[must_use]
    pub fn order_key(self) -> (EcsStreamPriorityOrderKey, u64, u8, i32, i32, i32) {
        (
            self.priority.order_key(),
            self.camera_entity.get(),
            self.page.level,
            self.page.x,
            self.page.y,
            self.page.z,
        )
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct EcsStreamInterestTable {
    pub interests: Vec<EcsStreamInterest>,
}

impl EcsStreamInterestTable {
    pub fn push(&mut self, interest: EcsStreamInterest) -> Result<(), EcsSpatialValidationError> {
        if self.interests.len() >= ECS_SPATIAL_MAX_STREAM_INTERESTS {
            return Err(EcsSpatialValidationError::StreamInterestTableFull);
        }
        self.interests.push(interest);
        Ok(())
    }

    pub fn sort_deterministic(&mut self) {
        self.interests.sort_by_key(|interest| interest.order_key());
    }

    #[must_use]
    pub fn desired_pages(&self) -> Vec<EcsSpatialPageKey> {
        let mut pages = Vec::new();
        for interest in &self.interests {
            if !pages.contains(&interest.page) {
                pages.push(interest.page);
            }
        }
        pages
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EcsStreamingHole {
    pub page: EcsSpatialPageKey,
    pub fallback: Option<EcsSpatialPageKey>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EcsStreamRequestDiffResult {
    pub requested: u32,
    pub cancelled: u32,
    pub pinned: u32,
    pub unpinned: u32,
    pub holes: Vec<EcsStreamingHole>,
}

pub fn sense_sources(
    frame: u64,
    cameras: &[(SpatialStreamCamera, EcsCameraTransformSample)],
    sources: &[EcsStreamSourceDescriptor],
) -> EcsStreamingSourceSnapshot {
    EcsStreamingSourceSnapshot::from_inputs(frame, cameras, sources)
}

pub fn plan_stream_wave(
    snapshot: &EcsStreamingSourceSnapshot,
    ledger: &mut EcsStreamWaveLedger,
    camera_index: usize,
    max_shell_planned: u16,
    reason: StreamWaveReason,
) -> Option<EcsStreamWaveRecord> {
    let camera = snapshot.active_cameras.get(camera_index)?;
    let grid_id = *snapshot.active_terrain_grids.first()?;
    let origin_page = terrain_page_for_world_ft(grid_id, camera.origin_world_ft, 0);
    let record = EcsStreamWaveRecord {
        wave_id: ledger.next_wave_id(),
        origin_page,
        origin_world_ft: camera.origin_world_ft,
        camera_entity: camera.camera_entity,
        created_frame: snapshot.frame,
        max_shell_planned,
        reason: if camera.cut_or_teleport {
            StreamWaveReason::TeleportOrCut
        } else {
            reason
        },
    };
    ledger.push(record);
    Some(record)
}

pub fn build_interest(
    snapshot: &EcsStreamingSourceSnapshot,
    _grid_registry: &EcsSpatialGridRegistry,
    residency: &EcsPageResidencyTable,
) -> Result<EcsStreamInterestTable, EcsSpatialValidationError> {
    let mut table = EcsStreamInterestTable::default();
    for camera in &snapshot.active_cameras {
        for grid_id in &snapshot.active_terrain_grids {
            let origin_page = terrain_page_for_world_ft(*grid_id, camera.origin_world_ft, 0);
            append_camera_interests(&mut table, camera, origin_page, residency)?;
        }
    }
    table.sort_deterministic();
    Ok(table)
}

fn append_camera_interests(
    table: &mut EcsStreamInterestTable,
    camera: &EcsActiveStreamCameraSnapshot,
    origin_page: EcsSpatialPageKey,
    residency: &EcsPageResidencyTable,
) -> Result<(), EcsSpatialValidationError> {
    let velocity_bias = velocity_bias_page(camera.velocity_world_ft_s, camera.velocity_lookahead_s);
    for shell in 0..=camera.desired_shells {
        for offset in chebyshev_shell_offsets(shell) {
            let page = offset_page(origin_page, offset);
            let kind = classify_interest(camera, shell, offset, velocity_bias, residency, page);
            let mut priority = EcsStreamPriority::new(kind, shell, page.level);
            if kind == EcsStreamInterestKind::VelocityLookahead {
                priority.velocity_score_q = 65_535;
            }
            if kind == EcsStreamInterestKind::CameraContainingPage {
                priority.gameplay_score_q = 65_535;
                priority.view_score_q = 65_535;
            }
            table.push(EcsStreamInterest {
                camera: camera.camera_id,
                camera_entity: camera.camera_entity,
                page,
                kind,
                priority,
                shell,
                required: kind.is_required() || shell <= camera.required_shells,
            })?;
        }
    }
    Ok(())
}

fn classify_interest(
    camera: &EcsActiveStreamCameraSnapshot,
    shell: u16,
    offset: IVec3,
    velocity_bias: IVec3,
    residency: &EcsPageResidencyTable,
    page: EcsSpatialPageKey,
) -> EcsStreamInterestKind {
    if shell == 0 {
        return EcsStreamInterestKind::CameraContainingPage;
    }
    if shell <= camera.required_shells {
        return EcsStreamInterestKind::CollisionCriticalNear;
    }
    if velocity_bias != IVec3::zero()
        && offset.x.signum() == velocity_bias.x.signum()
        && offset.y.signum() == velocity_bias.y.signum()
        && offset.z.signum() == velocity_bias.z.signum()
    {
        return EcsStreamInterestKind::VelocityLookahead;
    }
    if residency
        .get_by_key(page)
        .and_then(|record| record.fallback)
        .is_some()
    {
        return EcsStreamInterestKind::VisibleCoarseFallback;
    }
    if shell <= camera.desired_shells.saturating_sub(1) {
        EcsStreamInterestKind::VisibleNear
    } else {
        EcsStreamInterestKind::Backfill
    }
}

pub fn diff_requests(
    interests: &EcsStreamInterestTable,
    residency: &EcsPageResidencyTable,
    requested_pages: &[EcsSpatialPageKey],
    pinned_pages: &[EcsSpatialPageKey],
    commands: &mut EcsSpatialCommandBuffer,
) -> Result<EcsStreamRequestDiffResult, EcsSpatialValidationError> {
    let mut result = EcsStreamRequestDiffResult::default();
    let desired_pages = interests.desired_pages();

    for interest in &interests.interests {
        let record = residency.get_by_key(interest.page);
        if let Some(record) = record
            && record.state == EcsPageResidencyState::Failed
        {
            result.holes.push(EcsStreamingHole {
                page: interest.page,
                fallback: record.fallback,
            });
            if let Some(fallback) = record.fallback
                && !requested_pages.contains(&fallback)
            {
                commands.push(EcsSpatialCommand::RequestPage(fallback))?;
                result.requested += 1;
            }
            continue;
        }

        let is_available = record.is_some_and(|record| {
            record.state.is_external_resident()
                || matches!(
                    record.state,
                    EcsPageResidencyState::SourceQueued
                        | EcsPageResidencyState::SourceLoading
                        | EcsPageResidencyState::SourceReady
                        | EcsPageResidencyState::Decoding
                        | EcsPageResidencyState::CpuDecoded
                        | EcsPageResidencyState::DerivedBuilding
                        | EcsPageResidencyState::HandoffQueued
                        | EcsPageResidencyState::ExternalPublishing
                        | EcsPageResidencyState::Requested
                )
        });

        if !is_available && !requested_pages.contains(&interest.page) {
            commands.push(EcsSpatialCommand::RequestPage(interest.page))?;
            result.requested += 1;
        }

        if interest.required && !pinned_pages.contains(&interest.page) {
            commands.push(EcsSpatialCommand::PinPage(interest.page))?;
            result.pinned += 1;
        }
    }

    for requested in requested_pages {
        if !desired_pages.contains(requested) {
            commands.push(EcsSpatialCommand::CancelPage(*requested))?;
            result.cancelled += 1;
        }
    }

    for pinned in pinned_pages {
        let still_required = interests
            .interests
            .iter()
            .any(|interest| interest.page == *pinned && interest.required);
        if !still_required {
            commands.push(EcsSpatialCommand::UnpinPage(*pinned))?;
            result.unpinned += 1;
        }
    }

    Ok(result)
}

#[must_use]
pub fn terrain_page_for_world_ft(
    grid_id: EcsSpatialGridId,
    world_ft: IVec3,
    level: u8,
) -> EcsSpatialPageKey {
    let scale = if level >= 20 {
        i32::MAX / 32
    } else {
        1_i32 << u32::from(level)
    };
    let divisor = 32_i32.saturating_mul(scale).max(1);
    EcsSpatialPageKey::new(
        EcsSpatialDomainKind::Terrain,
        grid_id,
        level,
        floor_div(world_ft.x, divisor),
        floor_div(world_ft.y, divisor),
        floor_div(world_ft.z, divisor),
        EcsPageChannel::Occupancy,
    )
}

#[must_use]
pub fn offset_page(page: EcsSpatialPageKey, offset: IVec3) -> EcsSpatialPageKey {
    EcsSpatialPageKey::new(
        page.domain,
        page.grid_id,
        page.level,
        page.x + offset.x,
        page.y + offset.y,
        page.z + offset.z,
        page.channel,
    )
}

#[must_use]
pub fn chebyshev_shell_offsets(shell: u16) -> Vec<IVec3> {
    let shell = i32::from(shell);
    let mut offsets = Vec::new();
    for z in -shell..=shell {
        for y in -shell..=shell {
            for x in -shell..=shell {
                let offset = IVec3::new(x, y, z);
                if offset.chebyshev_len() == shell as u16 {
                    offsets.push(offset);
                }
            }
        }
    }
    offsets.sort();
    offsets
}

fn velocity_bias_page(velocity_ft_s: IVec3, lookahead_s: f32) -> IVec3 {
    if lookahead_s <= 0.0 {
        return IVec3::zero();
    }
    IVec3::new(
        lookahead_axis(velocity_ft_s.x, lookahead_s),
        lookahead_axis(velocity_ft_s.y, lookahead_s),
        lookahead_axis(velocity_ft_s.z, lookahead_s),
    )
}

fn lookahead_axis(velocity: i32, lookahead_s: f32) -> i32 {
    let projected_ft = velocity as f32 * lookahead_s;
    if projected_ft >= 16.0 {
        1
    } else if projected_ft <= -16.0 {
        -1
    } else {
        0
    }
}

fn floor_div(value: i32, divisor: i32) -> i32 {
    let quotient = value / divisor;
    let remainder = value % divisor;
    if remainder != 0 && ((remainder < 0) != (divisor < 0)) {
        quotient - 1
    } else {
        quotient
    }
}
