use std::{collections::BTreeMap, fmt::Write as _, io, path::Path};

use crate::LuxDenoiseReconstructionPath;

pub const GI_SCHEMA_VERSION: u16 = 1;
pub const GI_REFLECTION_SCENE_ARTIFACT_ENV: &str = "FUN_LUX_GI_REFLECTION_SCENE_ARTIFACT";
pub const GI_PROCEDURAL_INVALIDATION_ARTIFACT_ENV: &str =
    "FUN_LUX_GI_PROCEDURAL_INVALIDATION_ARTIFACT";
pub const GI_CACHE_STABILITY_ARTIFACT_ENV: &str = "FUN_LUX_GI_CACHE_STABILITY_ARTIFACT";
pub const GI_PRODUCTION_WORLD_REPRESENTATIONS: [IndirectWorldRepresentation; 1] =
    [IndirectWorldRepresentation::SurfaceCache];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GiQualityTier {
    Tier0AmbientProbeFallback,
    Tier1ScreenSpace,
    #[default]
    Tier2ScreenSpaceSurfaceCache,
    Tier3SelectiveHardwareRtAssist,
    Tier4ExperimentalRadianceNeuralCache,
}

impl GiQualityTier {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tier0AmbientProbeFallback => "tier0_ambient_probe_fallback",
            Self::Tier1ScreenSpace => "tier1_screen_space",
            Self::Tier2ScreenSpaceSurfaceCache => "tier2_screen_space_surface_cache",
            Self::Tier3SelectiveHardwareRtAssist => "tier3_selective_hardware_rt_assist",
            Self::Tier4ExperimentalRadianceNeuralCache => {
                "tier4_experimental_radiance_neural_cache"
            }
        }
    }

    #[must_use]
    pub const fn tier_index(self) -> u8 {
        match self {
            Self::Tier0AmbientProbeFallback => 0,
            Self::Tier1ScreenSpace => 1,
            Self::Tier2ScreenSpaceSurfaceCache => 2,
            Self::Tier3SelectiveHardwareRtAssist => 3,
            Self::Tier4ExperimentalRadianceNeuralCache => 4,
        }
    }

    #[must_use]
    pub const fn uses_screen_tracing(self) -> bool {
        !matches!(self, Self::Tier0AmbientProbeFallback)
    }

    #[must_use]
    pub const fn uses_world_cache(self) -> bool {
        matches!(
            self,
            Self::Tier2ScreenSpaceSurfaceCache
                | Self::Tier3SelectiveHardwareRtAssist
                | Self::Tier4ExperimentalRadianceNeuralCache
        )
    }

    #[must_use]
    pub const fn may_use_hardware_rt(self) -> bool {
        matches!(
            self,
            Self::Tier3SelectiveHardwareRtAssist | Self::Tier4ExperimentalRadianceNeuralCache
        )
    }

    #[must_use]
    pub const fn may_use_experimental_neural_cache(self) -> bool {
        matches!(self, Self::Tier4ExperimentalRadianceNeuralCache)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IndirectWorldRepresentation {
    #[default]
    SurfaceCache,
}

impl IndirectWorldRepresentation {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SurfaceCache => "surface_cache",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GiRuntimeCapabilities {
    pub hardware_ray_tracing: bool,
    pub experimental_neural_cache: bool,
}

impl GiRuntimeCapabilities {
    pub const SOFTWARE_ONLY: Self = Self {
        hardware_ray_tracing: false,
        experimental_neural_cache: false,
    };

    pub const FULL_EXPERIMENTAL: Self = Self {
        hardware_ray_tracing: true,
        experimental_neural_cache: true,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GiQualitySettings {
    pub tier: GiQualityTier,
    pub world_representation: IndirectWorldRepresentation,
    pub screen_trace_enabled: bool,
    pub world_cache_enabled: bool,
    pub hardware_rt_enabled: bool,
    pub experimental_neural_cache_enabled: bool,
    pub reconstruction_path: LuxDenoiseReconstructionPath,
}

impl GiQualitySettings {
    #[must_use]
    pub const fn for_tier(tier: GiQualityTier, capabilities: GiRuntimeCapabilities) -> Self {
        Self {
            tier,
            world_representation: IndirectWorldRepresentation::SurfaceCache,
            screen_trace_enabled: tier.uses_screen_tracing(),
            world_cache_enabled: tier.uses_world_cache(),
            hardware_rt_enabled: tier.may_use_hardware_rt() && capabilities.hardware_ray_tracing,
            experimental_neural_cache_enabled: tier.may_use_experimental_neural_cache()
                && capabilities.experimental_neural_cache,
            reconstruction_path: if tier.may_use_experimental_neural_cache()
                && capabilities.experimental_neural_cache
            {
                LuxDenoiseReconstructionPath::ModelAssisted
            } else if tier.tier_index() >= 2 {
                LuxDenoiseReconstructionPath::BalancedQuality
            } else {
                LuxDenoiseReconstructionPath::BalancedFast
            },
        }
    }
}

impl Default for GiQualitySettings {
    fn default() -> Self {
        Self::for_tier(
            GiQualityTier::Tier2ScreenSpaceSurfaceCache,
            GiRuntimeCapabilities::SOFTWARE_ONLY,
        )
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SurfaceCacheCellId(pub u64);

impl SurfaceCacheCellId {
    pub const INVALID: Self = Self(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfaceCacheOccupancy {
    #[default]
    Empty,
    Candidate,
    Occupied,
    Pinned,
}

impl SurfaceCacheOccupancy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Candidate => "candidate",
            Self::Occupied => "occupied",
            Self::Pinned => "pinned",
        }
    }

    #[must_use]
    pub const fn contributes_lighting(self) -> bool {
        matches!(self, Self::Occupied | Self::Pinned)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfaceCacheValidity {
    #[default]
    Invalid,
    Requested,
    Updating,
    Valid,
    Stale,
}

impl SurfaceCacheValidity {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Invalid => "invalid",
            Self::Requested => "requested",
            Self::Updating => "updating",
            Self::Valid => "valid",
            Self::Stale => "stale",
        }
    }

    #[must_use]
    pub const fn requires_update(self) -> bool {
        matches!(self, Self::Invalid | Self::Requested | Self::Stale)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HistoryRejectionReason {
    #[default]
    None,
    DepthDisocclusion,
    NormalChanged,
    MaterialChanged,
    LightingDiscontinuity,
    CameraCut,
    SceneRevisionChanged,
}

impl HistoryRejectionReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::DepthDisocclusion => "depth_disocclusion",
            Self::NormalChanged => "normal_changed",
            Self::MaterialChanged => "material_changed",
            Self::LightingDiscontinuity => "lighting_discontinuity",
            Self::CameraCut => "camera_cut",
            Self::SceneRevisionChanged => "scene_revision_changed",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GiInvalidationReason {
    #[default]
    None,
    ProceduralEdit,
    Destruction,
    TimeOfDayJump,
    WeatherChange,
    MaterialChange,
    CameraCut,
    MajorSceneStreamingEvent,
    HistoryRejected,
    CacheBudgetPressure,
}

impl GiInvalidationReason {
    pub const DYNAMIC_SCENE_REASONS: [Self; 7] = [
        Self::ProceduralEdit,
        Self::Destruction,
        Self::TimeOfDayJump,
        Self::WeatherChange,
        Self::MaterialChange,
        Self::CameraCut,
        Self::MajorSceneStreamingEvent,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ProceduralEdit => "procedural_edit",
            Self::Destruction => "destruction",
            Self::TimeOfDayJump => "time_of_day_jump",
            Self::WeatherChange => "weather_change",
            Self::MaterialChange => "material_change",
            Self::CameraCut => "camera_cut",
            Self::MajorSceneStreamingEvent => "major_scene_streaming_event",
            Self::HistoryRejected => "history_rejected",
            Self::CacheBudgetPressure => "cache_budget_pressure",
        }
    }

    #[must_use]
    pub const fn is_global_reset(self) -> bool {
        matches!(
            self,
            Self::TimeOfDayJump
                | Self::WeatherChange
                | Self::CameraCut
                | Self::MajorSceneStreamingEvent
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceCacheCell {
    pub cell_id: SurfaceCacheCellId,
    pub center_world: [f32; 3],
    pub normal_world: [f32; 3],
    pub radiance_rgb: [f32; 3],
    pub occupancy: SurfaceCacheOccupancy,
    pub age_frames: u32,
    pub validity: SurfaceCacheValidity,
    pub update_cost_micros: u32,
    pub history_rejection: HistoryRejectionReason,
    pub invalidation_reason: GiInvalidationReason,
    pub last_updated_frame: u64,
    pub scene_revision: u64,
}

impl SurfaceCacheCell {
    #[must_use]
    pub const fn occupied(
        cell_id: SurfaceCacheCellId,
        center_world: [f32; 3],
        normal_world: [f32; 3],
        radiance_rgb: [f32; 3],
        frame_index: u64,
        scene_revision: u64,
    ) -> Self {
        Self {
            cell_id,
            center_world,
            normal_world,
            radiance_rgb,
            occupancy: SurfaceCacheOccupancy::Occupied,
            age_frames: 0,
            validity: SurfaceCacheValidity::Valid,
            update_cost_micros: 0,
            history_rejection: HistoryRejectionReason::None,
            invalidation_reason: GiInvalidationReason::None,
            last_updated_frame: frame_index,
            scene_revision,
        }
    }

    pub fn advance_frame(&mut self, max_valid_age_frames: u32) {
        if self.occupancy.contributes_lighting() {
            self.age_frames = self.age_frames.saturating_add(1);
            if self.age_frames > max_valid_age_frames
                && self.validity == SurfaceCacheValidity::Valid
            {
                self.validity = SurfaceCacheValidity::Stale;
            }
        }
    }

    pub fn invalidate(&mut self, reason: GiInvalidationReason, scene_revision: u64) {
        self.validity = SurfaceCacheValidity::Invalid;
        self.invalidation_reason = reason;
        self.scene_revision = scene_revision;
        self.history_rejection = match reason {
            GiInvalidationReason::MaterialChange => HistoryRejectionReason::MaterialChanged,
            GiInvalidationReason::CameraCut => HistoryRejectionReason::CameraCut,
            GiInvalidationReason::HistoryRejected => HistoryRejectionReason::SceneRevisionChanged,
            GiInvalidationReason::ProceduralEdit
            | GiInvalidationReason::Destruction
            | GiInvalidationReason::MajorSceneStreamingEvent => {
                HistoryRejectionReason::SceneRevisionChanged
            }
            GiInvalidationReason::TimeOfDayJump | GiInvalidationReason::WeatherChange => {
                HistoryRejectionReason::LightingDiscontinuity
            }
            GiInvalidationReason::None | GiInvalidationReason::CacheBudgetPressure => {
                HistoryRejectionReason::None
            }
        };
    }

    pub fn mark_update_requested(&mut self) {
        if self.validity.requires_update() {
            self.validity = SurfaceCacheValidity::Requested;
        }
    }

    pub fn mark_updated(
        &mut self,
        radiance_rgb: [f32; 3],
        update_cost_micros: u32,
        frame_index: u64,
        scene_revision: u64,
    ) {
        self.radiance_rgb = radiance_rgb;
        self.update_cost_micros = update_cost_micros;
        self.last_updated_frame = frame_index;
        self.scene_revision = scene_revision;
        self.age_frames = 0;
        self.validity = SurfaceCacheValidity::Valid;
        self.history_rejection = HistoryRejectionReason::None;
        self.invalidation_reason = GiInvalidationReason::None;
    }

    #[must_use]
    pub const fn requires_update(self) -> bool {
        self.occupancy.contributes_lighting() && self.validity.requires_update()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceCacheUpdateRequest {
    pub cell_id: SurfaceCacheCellId,
    pub reason: GiInvalidationReason,
    pub priority: u16,
    pub estimated_cost_micros: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceCacheDiagnostics {
    pub cell_count: u32,
    pub occupied_count: u32,
    pub valid_count: u32,
    pub stale_count: u32,
    pub invalid_count: u32,
    pub requested_update_count: u32,
    pub total_update_cost_micros: u64,
    pub oldest_age_frames: u32,
    pub history_rejection_count: u32,
    pub procedural_invalidations: u32,
    pub destruction_invalidations: u32,
    pub global_invalidations: u32,
    pub stability_score_per_mille: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceCache {
    pub representation: IndirectWorldRepresentation,
    pub max_valid_age_frames: u32,
    pub frame_index: u64,
    pub scene_revision: u64,
    cells: BTreeMap<SurfaceCacheCellId, SurfaceCacheCell>,
}

impl SurfaceCache {
    #[must_use]
    pub const fn new(max_valid_age_frames: u32) -> Self {
        Self {
            representation: IndirectWorldRepresentation::SurfaceCache,
            max_valid_age_frames,
            frame_index: 0,
            scene_revision: 0,
            cells: BTreeMap::new(),
        }
    }

    pub fn cells(&self) -> impl Iterator<Item = &SurfaceCacheCell> {
        self.cells.values()
    }

    #[must_use]
    pub fn get(&self, cell_id: SurfaceCacheCellId) -> Option<&SurfaceCacheCell> {
        self.cells.get(&cell_id)
    }

    pub fn upsert(&mut self, cell: SurfaceCacheCell) -> bool {
        let inserted = !self.cells.contains_key(&cell.cell_id);
        self.scene_revision = self.scene_revision.max(cell.scene_revision);
        self.frame_index = self.frame_index.max(cell.last_updated_frame);
        self.cells.insert(cell.cell_id, cell);
        inserted
    }

    pub fn advance_frame(&mut self) {
        self.frame_index = self.frame_index.saturating_add(1);
        for cell in self.cells.values_mut() {
            cell.advance_frame(self.max_valid_age_frames);
        }
    }

    pub fn invalidate_cell(
        &mut self,
        cell_id: SurfaceCacheCellId,
        reason: GiInvalidationReason,
        scene_revision: u64,
    ) -> bool {
        let Some(cell) = self.cells.get_mut(&cell_id) else {
            return false;
        };
        self.scene_revision = self.scene_revision.max(scene_revision);
        cell.invalidate(reason, scene_revision);
        true
    }

    pub fn invalidate_all(&mut self, reason: GiInvalidationReason, scene_revision: u64) -> u32 {
        self.scene_revision = self.scene_revision.max(scene_revision);
        let mut invalidated = 0u32;
        for cell in self.cells.values_mut() {
            if cell.occupancy.contributes_lighting() {
                cell.invalidate(reason, scene_revision);
                invalidated = invalidated.saturating_add(1);
            }
        }
        invalidated
    }

    pub fn request_updates(&mut self, max_requests: usize) -> Vec<SurfaceCacheUpdateRequest> {
        let mut requests = self
            .cells
            .values_mut()
            .filter(|cell| cell.requires_update())
            .map(|cell| {
                cell.mark_update_requested();
                SurfaceCacheUpdateRequest {
                    cell_id: cell.cell_id,
                    reason: cell.invalidation_reason,
                    priority: surface_cache_update_priority(cell),
                    estimated_cost_micros: surface_cache_estimated_cost(cell),
                }
            })
            .collect::<Vec<_>>();
        requests.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| left.cell_id.cmp(&right.cell_id))
        });
        requests.truncate(max_requests);
        requests
    }

    #[must_use]
    pub fn diagnostics(&self) -> SurfaceCacheDiagnostics {
        let mut diagnostics = SurfaceCacheDiagnostics {
            cell_count: self.cells.len() as u32,
            ..SurfaceCacheDiagnostics::default()
        };
        for cell in self.cells.values() {
            if cell.occupancy.contributes_lighting() {
                diagnostics.occupied_count = diagnostics.occupied_count.saturating_add(1);
            }
            match cell.validity {
                SurfaceCacheValidity::Valid => {
                    diagnostics.valid_count = diagnostics.valid_count.saturating_add(1);
                }
                SurfaceCacheValidity::Stale => {
                    diagnostics.stale_count = diagnostics.stale_count.saturating_add(1);
                    diagnostics.requested_update_count =
                        diagnostics.requested_update_count.saturating_add(1);
                }
                SurfaceCacheValidity::Invalid => {
                    diagnostics.invalid_count = diagnostics.invalid_count.saturating_add(1);
                    diagnostics.requested_update_count =
                        diagnostics.requested_update_count.saturating_add(1);
                }
                SurfaceCacheValidity::Requested | SurfaceCacheValidity::Updating => {
                    diagnostics.requested_update_count =
                        diagnostics.requested_update_count.saturating_add(1);
                }
            }
            diagnostics.total_update_cost_micros = diagnostics
                .total_update_cost_micros
                .saturating_add(u64::from(cell.update_cost_micros));
            diagnostics.oldest_age_frames = diagnostics.oldest_age_frames.max(cell.age_frames);
            if cell.history_rejection != HistoryRejectionReason::None {
                diagnostics.history_rejection_count =
                    diagnostics.history_rejection_count.saturating_add(1);
            }
            match cell.invalidation_reason {
                GiInvalidationReason::ProceduralEdit => {
                    diagnostics.procedural_invalidations =
                        diagnostics.procedural_invalidations.saturating_add(1);
                }
                GiInvalidationReason::Destruction => {
                    diagnostics.destruction_invalidations =
                        diagnostics.destruction_invalidations.saturating_add(1);
                }
                reason if reason.is_global_reset() => {
                    diagnostics.global_invalidations =
                        diagnostics.global_invalidations.saturating_add(1);
                }
                _ => {}
            }
        }
        diagnostics.stability_score_per_mille = surface_cache_stability_score(&diagnostics);
        diagnostics
    }
}

impl Default for SurfaceCache {
    fn default() -> Self {
        Self::new(120)
    }
}

#[must_use]
pub fn surface_cache_update_priority(cell: &SurfaceCacheCell) -> u16 {
    let validity = match cell.validity {
        SurfaceCacheValidity::Invalid => 800u32,
        SurfaceCacheValidity::Requested => 700,
        SurfaceCacheValidity::Updating => 500,
        SurfaceCacheValidity::Stale => 400,
        SurfaceCacheValidity::Valid => 0,
    };
    let reason = match cell.invalidation_reason {
        GiInvalidationReason::Destruction => 500u32,
        GiInvalidationReason::ProceduralEdit => 420,
        GiInvalidationReason::MaterialChange => 360,
        GiInvalidationReason::TimeOfDayJump
        | GiInvalidationReason::WeatherChange
        | GiInvalidationReason::CameraCut
        | GiInvalidationReason::MajorSceneStreamingEvent => 320,
        GiInvalidationReason::HistoryRejected => 280,
        GiInvalidationReason::CacheBudgetPressure => 120,
        GiInvalidationReason::None => 0,
    };
    let occupancy = match cell.occupancy {
        SurfaceCacheOccupancy::Pinned => 300u32,
        SurfaceCacheOccupancy::Occupied => 200,
        SurfaceCacheOccupancy::Candidate => 80,
        SurfaceCacheOccupancy::Empty => 0,
    };
    let age = cell.age_frames.min(255);
    (validity + reason + occupancy + age).min(u32::from(u16::MAX)) as u16
}

#[must_use]
pub fn surface_cache_estimated_cost(cell: &SurfaceCacheCell) -> u32 {
    let base = match cell.occupancy {
        SurfaceCacheOccupancy::Pinned => 180,
        SurfaceCacheOccupancy::Occupied => 120,
        SurfaceCacheOccupancy::Candidate => 64,
        SurfaceCacheOccupancy::Empty => 0,
    };
    let invalidation = match cell.invalidation_reason {
        GiInvalidationReason::Destruction | GiInvalidationReason::ProceduralEdit => 140,
        GiInvalidationReason::TimeOfDayJump | GiInvalidationReason::WeatherChange => 96,
        GiInvalidationReason::MaterialChange => 80,
        GiInvalidationReason::CameraCut
        | GiInvalidationReason::MajorSceneStreamingEvent
        | GiInvalidationReason::HistoryRejected => 64,
        GiInvalidationReason::CacheBudgetPressure | GiInvalidationReason::None => 24,
    };
    base + invalidation + cell.age_frames.min(64)
}

#[must_use]
pub fn surface_cache_stability_score(diagnostics: &SurfaceCacheDiagnostics) -> u16 {
    if diagnostics.occupied_count == 0 {
        return 1000;
    }
    let valid = u64::from(diagnostics.valid_count) * 1000 / u64::from(diagnostics.occupied_count);
    let rejection_penalty = u64::from(diagnostics.history_rejection_count).saturating_mul(40);
    let invalid_penalty = u64::from(diagnostics.invalid_count).saturating_mul(20);
    valid
        .saturating_sub(rejection_penalty)
        .saturating_sub(invalid_penalty)
        .min(1000) as u16
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndirectSceneChange {
    pub reason: GiInvalidationReason,
    pub affected_cell: Option<SurfaceCacheCellId>,
    pub scene_revision: u64,
    pub severity: u16,
}

impl IndirectSceneChange {
    #[must_use]
    pub const fn new(
        reason: GiInvalidationReason,
        affected_cell: Option<SurfaceCacheCellId>,
        scene_revision: u64,
        severity: u16,
    ) -> Self {
        Self {
            reason,
            affected_cell,
            scene_revision,
            severity,
        }
    }
}

pub fn apply_scene_changes(
    cache: &mut SurfaceCache,
    changes: &[IndirectSceneChange],
) -> SurfaceCacheDiagnostics {
    for change in changes {
        if change.reason.is_global_reset() || change.affected_cell.is_none() {
            let _ = cache.invalidate_all(change.reason, change.scene_revision);
            continue;
        }
        if let Some(cell_id) = change.affected_cell {
            let _ = cache.invalidate_cell(cell_id, change.reason, change.scene_revision);
        }
    }
    cache.diagnostics()
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReflectionRegionId(pub u64);

impl ReflectionRegionId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReflectionMaterialClass {
    Mirror,
    Glossy,
    #[default]
    Rough,
    Transparent,
    Water,
}

impl ReflectionMaterialClass {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mirror => "mirror",
            Self::Glossy => "glossy",
            Self::Rough => "rough",
            Self::Transparent => "transparent",
            Self::Water => "water",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReflectionRegionInput {
    pub region_id: ReflectionRegionId,
    pub material_class: ReflectionMaterialClass,
    pub roughness_per_mille: u16,
    pub screen_trace_confidence_per_mille: u16,
    pub surface_cache_cell: Option<SurfaceCacheCellId>,
    pub hardware_rt_eligible: bool,
    pub needs_denoise: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReflectionSource {
    Disabled,
    #[default]
    AmbientProbeFallback,
    ScreenTrace,
    SurfaceCache,
    HardwareRtAssist,
    ExperimentalNeuralCache,
    DenoiseReconstruct,
}

impl ReflectionSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::AmbientProbeFallback => "ambient_probe_fallback",
            Self::ScreenTrace => "screen_trace",
            Self::SurfaceCache => "surface_cache",
            Self::HardwareRtAssist => "hardware_rt_assist",
            Self::ExperimentalNeuralCache => "experimental_neural_cache",
            Self::DenoiseReconstruct => "denoise_reconstruct",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReflectionSourceDecision {
    pub region_id: ReflectionRegionId,
    pub material_class: ReflectionMaterialClass,
    pub primary_source: ReflectionSource,
    pub reconstruction_source: Option<ReflectionSource>,
    pub source_order: [ReflectionSource; 5],
    pub source_count: u8,
    pub cache_miss: bool,
}

impl ReflectionSourceDecision {
    #[must_use]
    pub fn source_order(&self) -> &[ReflectionSource] {
        &self.source_order[..usize::from(self.source_count)]
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ReflectionSourceMixDiagnostics {
    pub region_count: u32,
    pub screen_trace_count: u32,
    pub surface_cache_count: u32,
    pub hardware_rt_count: u32,
    pub neural_cache_count: u32,
    pub ambient_probe_count: u32,
    pub denoise_reconstruct_count: u32,
    pub cache_miss_count: u32,
}

pub fn select_reflection_sources(
    settings: GiQualitySettings,
    cache: &SurfaceCache,
    regions: &[ReflectionRegionInput],
) -> Vec<ReflectionSourceDecision> {
    regions
        .iter()
        .map(|region| select_reflection_source(settings, cache, *region))
        .collect()
}

#[must_use]
pub fn select_reflection_source(
    settings: GiQualitySettings,
    cache: &SurfaceCache,
    region: ReflectionRegionInput,
) -> ReflectionSourceDecision {
    let cache_hit = region.surface_cache_cell.is_some_and(|cell_id| {
        cache
            .get(cell_id)
            .is_some_and(|cell| cell.validity == SurfaceCacheValidity::Valid)
    });
    let cache_miss = region.surface_cache_cell.is_some() && !cache_hit;

    let primary_source =
        if settings.screen_trace_enabled && region.screen_trace_confidence_per_mille >= 700 {
            ReflectionSource::ScreenTrace
        } else if settings.world_cache_enabled && cache_hit {
            ReflectionSource::SurfaceCache
        } else if settings.hardware_rt_enabled && region.hardware_rt_eligible {
            ReflectionSource::HardwareRtAssist
        } else if settings.experimental_neural_cache_enabled {
            ReflectionSource::ExperimentalNeuralCache
        } else if settings.screen_trace_enabled && region.screen_trace_confidence_per_mille > 0 {
            ReflectionSource::ScreenTrace
        } else {
            ReflectionSource::AmbientProbeFallback
        };

    let reconstruction_source = if region.needs_denoise
        || matches!(
            primary_source,
            ReflectionSource::SurfaceCache
                | ReflectionSource::HardwareRtAssist
                | ReflectionSource::ExperimentalNeuralCache
        ) {
        Some(ReflectionSource::DenoiseReconstruct)
    } else {
        None
    };

    let mut source_order = [ReflectionSource::Disabled; 5];
    let mut source_count = 0u8;
    push_source(
        &mut source_order,
        &mut source_count,
        ReflectionSource::ScreenTrace,
    );
    if settings.world_cache_enabled {
        push_source(
            &mut source_order,
            &mut source_count,
            ReflectionSource::SurfaceCache,
        );
    }
    if settings.hardware_rt_enabled {
        push_source(
            &mut source_order,
            &mut source_count,
            ReflectionSource::HardwareRtAssist,
        );
    }
    if settings.experimental_neural_cache_enabled {
        push_source(
            &mut source_order,
            &mut source_count,
            ReflectionSource::ExperimentalNeuralCache,
        );
    }
    push_source(
        &mut source_order,
        &mut source_count,
        ReflectionSource::DenoiseReconstruct,
    );

    ReflectionSourceDecision {
        region_id: region.region_id,
        material_class: region.material_class,
        primary_source,
        reconstruction_source,
        source_order,
        source_count,
        cache_miss,
    }
}

fn push_source(order: &mut [ReflectionSource; 5], count: &mut u8, source: ReflectionSource) {
    let index = usize::from(*count);
    if index < order.len() {
        order[index] = source;
        *count = count.saturating_add(1);
    }
}

#[must_use]
pub fn reflection_source_mix(
    decisions: &[ReflectionSourceDecision],
) -> ReflectionSourceMixDiagnostics {
    let mut diagnostics = ReflectionSourceMixDiagnostics {
        region_count: decisions.len() as u32,
        ..ReflectionSourceMixDiagnostics::default()
    };
    for decision in decisions {
        match decision.primary_source {
            ReflectionSource::ScreenTrace => {
                diagnostics.screen_trace_count = diagnostics.screen_trace_count.saturating_add(1);
            }
            ReflectionSource::SurfaceCache => {
                diagnostics.surface_cache_count = diagnostics.surface_cache_count.saturating_add(1);
            }
            ReflectionSource::HardwareRtAssist => {
                diagnostics.hardware_rt_count = diagnostics.hardware_rt_count.saturating_add(1);
            }
            ReflectionSource::ExperimentalNeuralCache => {
                diagnostics.neural_cache_count = diagnostics.neural_cache_count.saturating_add(1);
            }
            ReflectionSource::AmbientProbeFallback => {
                diagnostics.ambient_probe_count = diagnostics.ambient_probe_count.saturating_add(1);
            }
            ReflectionSource::Disabled | ReflectionSource::DenoiseReconstruct => {}
        }
        if decision.reconstruction_source == Some(ReflectionSource::DenoiseReconstruct) {
            diagnostics.denoise_reconstruct_count =
                diagnostics.denoise_reconstruct_count.saturating_add(1);
        }
        if decision.cache_miss {
            diagnostics.cache_miss_count = diagnostics.cache_miss_count.saturating_add(1);
        }
    }
    diagnostics
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GiDebugViewKind {
    #[default]
    CacheOccupancy,
    CacheValidity,
    CacheInvalidation,
    ReflectionSourceMix,
    CacheStability,
}

impl GiDebugViewKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CacheOccupancy => "cache_occupancy",
            Self::CacheValidity => "cache_validity",
            Self::CacheInvalidation => "cache_invalidation",
            Self::ReflectionSourceMix => "reflection_source_mix",
            Self::CacheStability => "cache_stability",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GiDebugArtifact {
    pub schema_version: u16,
    pub view: GiDebugViewKind,
    pub content: String,
}

impl GiDebugArtifact {
    #[must_use]
    pub fn reflection_scene(
        settings: GiQualitySettings,
        cache: &SurfaceCache,
        decisions: &[ReflectionSourceDecision],
    ) -> Self {
        let diagnostics = cache.diagnostics();
        let mix = reflection_source_mix(decisions);
        let mut content = String::new();
        let _ = writeln!(content, "gi_schema_version={GI_SCHEMA_VERSION}");
        let _ = writeln!(content, "quality_tier={}", settings.tier.as_str());
        let _ = writeln!(
            content,
            "world_representation={}",
            settings.world_representation.as_str()
        );
        let _ = writeln!(content, "surface_cache_cells={}", diagnostics.cell_count);
        let _ = writeln!(
            content,
            "surface_cache_valid_cells={}",
            diagnostics.valid_count
        );
        let _ = writeln!(content, "reflection_regions={}", mix.region_count);
        let _ = writeln!(content, "screen_trace_regions={}", mix.screen_trace_count);
        let _ = writeln!(content, "surface_cache_regions={}", mix.surface_cache_count);
        let _ = writeln!(content, "hardware_rt_regions={}", mix.hardware_rt_count);
        let _ = writeln!(
            content,
            "denoise_reconstruct_regions={}",
            mix.denoise_reconstruct_count
        );
        for decision in decisions.iter().take(16) {
            let _ = writeln!(
                content,
                "region={} material={} primary={} reconstruct={} cache_miss={} source_order={}",
                decision.region_id.0,
                decision.material_class.as_str(),
                decision.primary_source.as_str(),
                decision
                    .reconstruction_source
                    .map(ReflectionSource::as_str)
                    .unwrap_or("none"),
                decision.cache_miss,
                source_order_labels(decision)
            );
        }
        Self {
            schema_version: GI_SCHEMA_VERSION,
            view: GiDebugViewKind::ReflectionSourceMix,
            content,
        }
    }

    #[must_use]
    pub fn procedural_invalidation(
        cache: &SurfaceCache,
        changes: &[IndirectSceneChange],
        diagnostics: SurfaceCacheDiagnostics,
    ) -> Self {
        let mut content = String::new();
        let _ = writeln!(
            content,
            "gi_invalidation_schema_version={GI_SCHEMA_VERSION}"
        );
        let _ = writeln!(content, "surface_cache_cells={}", diagnostics.cell_count);
        let _ = writeln!(
            content,
            "requested_update_count={}",
            diagnostics.requested_update_count
        );
        let _ = writeln!(
            content,
            "procedural_invalidations={}",
            diagnostics.procedural_invalidations
        );
        let _ = writeln!(
            content,
            "destruction_invalidations={}",
            diagnostics.destruction_invalidations
        );
        let _ = writeln!(
            content,
            "global_invalidations={}",
            diagnostics.global_invalidations
        );
        for change in changes.iter().take(16) {
            let _ = writeln!(
                content,
                "scene_change reason={} affected_cell={} scene_revision={} severity={}",
                change.reason.as_str(),
                change
                    .affected_cell
                    .map(|cell_id| cell_id.0)
                    .unwrap_or(SurfaceCacheCellId::INVALID.0),
                change.scene_revision,
                change.severity
            );
        }
        for cell in cache.cells().take(16) {
            let _ = writeln!(
                content,
                "cell={} validity={} reason={} age={} history_rejection={}",
                cell.cell_id.0,
                cell.validity.as_str(),
                cell.invalidation_reason.as_str(),
                cell.age_frames,
                cell.history_rejection.as_str()
            );
        }
        Self {
            schema_version: GI_SCHEMA_VERSION,
            view: GiDebugViewKind::CacheInvalidation,
            content,
        }
    }

    #[must_use]
    pub fn cache_stability(cache: &SurfaceCache, frames_sampled: u32) -> Self {
        let diagnostics = cache.diagnostics();
        let mut content = String::new();
        let _ = writeln!(
            content,
            "gi_cache_stability_schema_version={GI_SCHEMA_VERSION}"
        );
        let _ = writeln!(content, "frames_sampled={frames_sampled}");
        let _ = writeln!(content, "surface_cache_cells={}", diagnostics.cell_count);
        let _ = writeln!(content, "occupied_cells={}", diagnostics.occupied_count);
        let _ = writeln!(content, "valid_cells={}", diagnostics.valid_count);
        let _ = writeln!(content, "stale_cells={}", diagnostics.stale_count);
        let _ = writeln!(content, "invalid_cells={}", diagnostics.invalid_count);
        let _ = writeln!(
            content,
            "history_rejection_count={}",
            diagnostics.history_rejection_count
        );
        let _ = writeln!(
            content,
            "stability_score_per_mille={}",
            diagnostics.stability_score_per_mille
        );
        Self {
            schema_version: GI_SCHEMA_VERSION,
            view: GiDebugViewKind::CacheStability,
            content,
        }
    }
}

fn source_order_labels(decision: &ReflectionSourceDecision) -> String {
    let mut labels = String::new();
    for source in decision.source_order() {
        if !labels.is_empty() {
            labels.push('>');
        }
        labels.push_str(source.as_str());
    }
    labels
}

pub fn write_gi_artifact(path: impl AsRef<Path>, artifact: &GiDebugArtifact) -> io::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, &artifact.content)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(id: u64, frame_index: u64, scene_revision: u64) -> SurfaceCacheCell {
        SurfaceCacheCell::occupied(
            SurfaceCacheCellId::new(id),
            [id as f32, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.2, 0.3, 0.4],
            frame_index,
            scene_revision,
        )
    }

    fn cache_with_cells(count: u64) -> SurfaceCache {
        let mut cache = SurfaceCache::new(2);
        for id in 1..=count {
            assert!(cache.upsert(cell(id, 0, 1)));
        }
        cache
    }

    #[test]
    fn quality_ladder_uses_one_surface_cache_world_representation() {
        assert_eq!(
            GI_PRODUCTION_WORLD_REPRESENTATIONS,
            [IndirectWorldRepresentation::SurfaceCache]
        );

        let tier0 = GiQualitySettings::for_tier(
            GiQualityTier::Tier0AmbientProbeFallback,
            GiRuntimeCapabilities::FULL_EXPERIMENTAL,
        );
        assert!(!tier0.screen_trace_enabled);
        assert!(!tier0.world_cache_enabled);
        assert!(!tier0.hardware_rt_enabled);

        let tier2 = GiQualitySettings::for_tier(
            GiQualityTier::Tier2ScreenSpaceSurfaceCache,
            GiRuntimeCapabilities::SOFTWARE_ONLY,
        );
        assert!(tier2.screen_trace_enabled);
        assert!(tier2.world_cache_enabled);
        assert_eq!(
            tier2.world_representation,
            IndirectWorldRepresentation::SurfaceCache
        );
        assert!(!tier2.hardware_rt_enabled);

        let tier3 = GiQualitySettings::for_tier(
            GiQualityTier::Tier3SelectiveHardwareRtAssist,
            GiRuntimeCapabilities {
                hardware_ray_tracing: true,
                experimental_neural_cache: false,
            },
        );
        assert!(tier3.hardware_rt_enabled);
        assert!(!tier3.experimental_neural_cache_enabled);

        let tier4 = GiQualitySettings::for_tier(
            GiQualityTier::Tier4ExperimentalRadianceNeuralCache,
            GiRuntimeCapabilities::FULL_EXPERIMENTAL,
        );
        assert!(tier4.experimental_neural_cache_enabled);
        assert!(tier4.reconstruction_path.requires_fun_ai_runtime());
    }

    #[test]
    fn surface_cache_tracks_lifecycle_cost_age_validity_and_history_rejection() {
        let mut cache = SurfaceCache::new(1);
        assert!(cache.upsert(cell(1, 0, 1)));
        assert_eq!(cache.diagnostics().valid_count, 1);

        cache.advance_frame();
        cache.advance_frame();
        let stale = cache
            .get(SurfaceCacheCellId::new(1))
            .expect("cell should exist");
        assert_eq!(stale.validity, SurfaceCacheValidity::Stale);
        assert_eq!(stale.age_frames, 2);

        assert!(cache.invalidate_cell(
            SurfaceCacheCellId::new(1),
            GiInvalidationReason::ProceduralEdit,
            2
        ));
        let request = cache
            .request_updates(1)
            .pop()
            .expect("invalidated cell should request update");
        assert_eq!(request.reason, GiInvalidationReason::ProceduralEdit);
        assert!(request.priority > 0);
        assert!(request.estimated_cost_micros > 0);

        let frame_index = cache.frame_index;
        let scene_revision = cache.scene_revision;
        cache
            .cells
            .get_mut(&SurfaceCacheCellId::new(1))
            .expect("cell should exist")
            .mark_updated([1.0, 0.8, 0.5], 320, frame_index, scene_revision);

        let updated = cache
            .get(SurfaceCacheCellId::new(1))
            .expect("cell should exist");
        assert_eq!(updated.validity, SurfaceCacheValidity::Valid);
        assert_eq!(updated.update_cost_micros, 320);
        assert_eq!(updated.history_rejection, HistoryRejectionReason::None);
    }

    #[test]
    fn dynamic_scene_invalidation_reasons_are_explicit_and_deterministic() {
        assert_eq!(GiInvalidationReason::DYNAMIC_SCENE_REASONS.len(), 7);
        assert!(
            GiInvalidationReason::DYNAMIC_SCENE_REASONS
                .contains(&GiInvalidationReason::ProceduralEdit)
        );
        assert!(
            GiInvalidationReason::DYNAMIC_SCENE_REASONS
                .contains(&GiInvalidationReason::Destruction)
        );
        assert!(
            GiInvalidationReason::DYNAMIC_SCENE_REASONS
                .contains(&GiInvalidationReason::TimeOfDayJump)
        );
        assert!(
            GiInvalidationReason::DYNAMIC_SCENE_REASONS
                .contains(&GiInvalidationReason::WeatherChange)
        );
        assert!(
            GiInvalidationReason::DYNAMIC_SCENE_REASONS
                .contains(&GiInvalidationReason::MaterialChange)
        );
        assert!(GiInvalidationReason::CameraCut.is_global_reset());
        assert!(GiInvalidationReason::MajorSceneStreamingEvent.is_global_reset());

        let mut cache = cache_with_cells(3);
        let changes = [
            IndirectSceneChange::new(
                GiInvalidationReason::ProceduralEdit,
                Some(SurfaceCacheCellId::new(1)),
                2,
                200,
            ),
            IndirectSceneChange::new(GiInvalidationReason::Destruction, None, 3, 255),
        ];
        let diagnostics = apply_scene_changes(&mut cache, &changes);

        assert_eq!(diagnostics.invalid_count, 3);
        assert_eq!(diagnostics.destruction_invalidations, 3);
        assert_eq!(diagnostics.requested_update_count, 3);
    }

    #[test]
    fn reflection_source_ladder_prefers_screen_then_cache_then_optional_rt() {
        let mut cache = cache_with_cells(1);
        let valid_cell = SurfaceCacheCellId::new(1);
        let regions = [
            ReflectionRegionInput {
                region_id: ReflectionRegionId::new(1),
                material_class: ReflectionMaterialClass::Mirror,
                roughness_per_mille: 20,
                screen_trace_confidence_per_mille: 900,
                surface_cache_cell: Some(valid_cell),
                hardware_rt_eligible: true,
                needs_denoise: false,
            },
            ReflectionRegionInput {
                region_id: ReflectionRegionId::new(2),
                material_class: ReflectionMaterialClass::Glossy,
                roughness_per_mille: 250,
                screen_trace_confidence_per_mille: 100,
                surface_cache_cell: Some(valid_cell),
                hardware_rt_eligible: true,
                needs_denoise: true,
            },
            ReflectionRegionInput {
                region_id: ReflectionRegionId::new(3),
                material_class: ReflectionMaterialClass::Water,
                roughness_per_mille: 80,
                screen_trace_confidence_per_mille: 0,
                surface_cache_cell: Some(SurfaceCacheCellId::new(99)),
                hardware_rt_eligible: true,
                needs_denoise: true,
            },
        ];

        let tier2 = GiQualitySettings::for_tier(
            GiQualityTier::Tier2ScreenSpaceSurfaceCache,
            GiRuntimeCapabilities::SOFTWARE_ONLY,
        );
        let tier2_decisions = select_reflection_sources(tier2, &cache, &regions);
        assert_eq!(
            tier2_decisions[0].primary_source,
            ReflectionSource::ScreenTrace
        );
        assert_eq!(
            tier2_decisions[1].primary_source,
            ReflectionSource::SurfaceCache
        );
        assert_eq!(
            tier2_decisions[2].primary_source,
            ReflectionSource::AmbientProbeFallback
        );
        assert!(tier2_decisions[2].cache_miss);

        let tier3 = GiQualitySettings::for_tier(
            GiQualityTier::Tier3SelectiveHardwareRtAssist,
            GiRuntimeCapabilities {
                hardware_ray_tracing: true,
                experimental_neural_cache: false,
            },
        );
        let tier3_decisions = select_reflection_sources(tier3, &cache, &regions);
        assert_eq!(
            tier3_decisions[2].primary_source,
            ReflectionSource::HardwareRtAssist
        );
        assert_eq!(
            tier3_decisions[2].reconstruction_source,
            Some(ReflectionSource::DenoiseReconstruct)
        );

        assert_eq!(
            tier3_decisions[2].source_order(),
            &[
                ReflectionSource::ScreenTrace,
                ReflectionSource::SurfaceCache,
                ReflectionSource::HardwareRtAssist,
                ReflectionSource::DenoiseReconstruct,
            ]
        );

        cache.invalidate_all(GiInvalidationReason::CameraCut, 9);
        let no_rt = select_reflection_sources(tier2, &cache, &regions);
        assert_ne!(
            no_rt[2].primary_source,
            ReflectionSource::HardwareRtAssist,
            "tier 2 must not require hardware RT"
        );
    }

    #[test]
    fn cache_stability_score_drops_on_invalidations_and_recovers_after_updates() {
        let mut cache = cache_with_cells(4);
        assert_eq!(cache.diagnostics().stability_score_per_mille, 1000);

        let _ = cache.invalidate_cell(
            SurfaceCacheCellId::new(1),
            GiInvalidationReason::MaterialChange,
            2,
        );
        let _ = cache.invalidate_cell(
            SurfaceCacheCellId::new(2),
            GiInvalidationReason::HistoryRejected,
            3,
        );
        let degraded = cache.diagnostics();
        assert!(degraded.stability_score_per_mille < 1000);
        assert_eq!(degraded.history_rejection_count, 2);

        let frame_index = cache.frame_index;
        let scene_revision = cache.scene_revision;
        for cell_id in [SurfaceCacheCellId::new(1), SurfaceCacheCellId::new(2)] {
            cache
                .cells
                .get_mut(&cell_id)
                .expect("cell should exist")
                .mark_updated([0.6, 0.6, 0.6], 160, frame_index, scene_revision);
        }
        assert_eq!(cache.diagnostics().stability_score_per_mille, 1000);
    }

    #[test]
    fn gi_reflection_invalidation_and_stability_artifacts_are_stable() {
        let settings = GiQualitySettings::for_tier(
            GiQualityTier::Tier3SelectiveHardwareRtAssist,
            GiRuntimeCapabilities {
                hardware_ray_tracing: true,
                experimental_neural_cache: false,
            },
        );
        let mut cache = cache_with_cells(8);
        let regions = [
            ReflectionRegionInput {
                region_id: ReflectionRegionId::new(10),
                material_class: ReflectionMaterialClass::Mirror,
                roughness_per_mille: 10,
                screen_trace_confidence_per_mille: 950,
                surface_cache_cell: Some(SurfaceCacheCellId::new(1)),
                hardware_rt_eligible: true,
                needs_denoise: false,
            },
            ReflectionRegionInput {
                region_id: ReflectionRegionId::new(11),
                material_class: ReflectionMaterialClass::Glossy,
                roughness_per_mille: 300,
                screen_trace_confidence_per_mille: 200,
                surface_cache_cell: Some(SurfaceCacheCellId::new(2)),
                hardware_rt_eligible: true,
                needs_denoise: true,
            },
            ReflectionRegionInput {
                region_id: ReflectionRegionId::new(12),
                material_class: ReflectionMaterialClass::Water,
                roughness_per_mille: 100,
                screen_trace_confidence_per_mille: 0,
                surface_cache_cell: Some(SurfaceCacheCellId::new(99)),
                hardware_rt_eligible: true,
                needs_denoise: true,
            },
        ];
        let decisions = select_reflection_sources(settings, &cache, &regions);
        let scene = GiDebugArtifact::reflection_scene(settings, &cache, &decisions);
        assert!(scene.content.contains("world_representation=surface_cache"));
        assert!(
            scene
                .content
                .contains("source_order=screen_trace>surface_cache")
        );
        assert!(scene.content.contains("hardware_rt_regions=1"));

        let changes = [
            IndirectSceneChange::new(GiInvalidationReason::WeatherChange, None, 3, 180),
            IndirectSceneChange::new(
                GiInvalidationReason::ProceduralEdit,
                Some(SurfaceCacheCellId::new(3)),
                2,
                200,
            ),
        ];
        let invalidation_diagnostics = apply_scene_changes(&mut cache, &changes);
        let invalidation =
            GiDebugArtifact::procedural_invalidation(&cache, &changes, invalidation_diagnostics);
        assert!(
            invalidation
                .content
                .contains("gi_invalidation_schema_version=")
        );
        assert!(invalidation.content.contains("reason=weather_change"));

        let stability = GiDebugArtifact::cache_stability(&cache, 240);
        assert!(stability.content.contains("stability_score_per_mille="));

        if let Ok(path) = std::env::var(GI_REFLECTION_SCENE_ARTIFACT_ENV) {
            write_gi_artifact(path, &scene).expect("GI reflection artifact should be writable");
        }
        if let Ok(path) = std::env::var(GI_PROCEDURAL_INVALIDATION_ARTIFACT_ENV) {
            write_gi_artifact(path, &invalidation)
                .expect("GI invalidation artifact should be writable");
        }
        if let Ok(path) = std::env::var(GI_CACHE_STABILITY_ARTIFACT_ENV) {
            write_gi_artifact(path, &stability).expect("GI stability artifact should be writable");
        }
    }
}
