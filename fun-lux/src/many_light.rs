use std::{collections::BTreeMap, fmt::Write as _, io, path::Path};

use crate::{
    LuxLightId, LuxLightKind,
    shadow::{ShadowLightInput, ShadowPolicyConfig, ShadowPolicyEngine, ShadowReceiverDemand},
};

pub const MANY_LIGHT_SCHEMA_VERSION: u16 = 1;
pub const MANY_LIGHT_BENCHMARK_ARTIFACT_ENV: &str = "FUN_LUX_MANY_LIGHT_BENCHMARK_ARTIFACT";
pub const MANY_LIGHT_SHADOW_ARTIFACT_ENV: &str = "FUN_LUX_MANY_LIGHT_SHADOW_ARTIFACT";
pub const MANY_LIGHT_DEBUG_OVERLAY_ARTIFACT_ENV: &str = "FUN_LUX_MANY_LIGHT_DEBUG_OVERLAY_ARTIFACT";

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LightUpdateStamp(pub u64);

impl LightUpdateStamp {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EmissiveSourceRef(pub u64);

impl EmissiveSourceRef {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightTransform {
    pub position_view: [f32; 3],
    pub direction_view: [f32; 3],
}

impl LightTransform {
    pub const ORIGIN: Self = Self {
        position_view: [0.0, 0.0, 1.0],
        direction_view: [0.0, -1.0, 0.0],
    };

    #[must_use]
    pub const fn new(position_view: [f32; 3], direction_view: [f32; 3]) -> Self {
        Self {
            position_view,
            direction_view,
        }
    }
}

impl Default for LightTransform {
    fn default() -> Self {
        Self::ORIGIN
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightColorIntensity {
    pub color_rgb: [f32; 3],
    pub intensity_lux: f32,
}

impl LightColorIntensity {
    pub const WHITE_1000_LUX: Self = Self {
        color_rgb: [1.0, 1.0, 1.0],
        intensity_lux: 1000.0,
    };

    #[must_use]
    pub const fn new(color_rgb: [f32; 3], intensity_lux: f32) -> Self {
        Self {
            color_rgb,
            intensity_lux,
        }
    }
}

impl Default for LightColorIntensity {
    fn default() -> Self {
        Self::WHITE_1000_LUX
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightShape {
    pub radius_m: f32,
    pub cone_inner_cos: f32,
    pub cone_outer_cos: f32,
}

impl LightShape {
    pub const DIRECTIONAL: Self = Self {
        radius_m: 0.0,
        cone_inner_cos: 1.0,
        cone_outer_cos: 1.0,
    };

    #[must_use]
    pub const fn sphere(radius_m: f32) -> Self {
        Self {
            radius_m,
            cone_inner_cos: 1.0,
            cone_outer_cos: 1.0,
        }
    }
}

impl Default for LightShape {
    fn default() -> Self {
        Self::sphere(10.0)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LightImportanceHints {
    pub artist: u8,
    pub gameplay: u8,
    pub editor: u8,
}

impl LightImportanceHints {
    #[must_use]
    pub const fn new(artist: u8, gameplay: u8, editor: u8) -> Self {
        Self {
            artist,
            gameplay,
            editor,
        }
    }

    #[must_use]
    pub const fn score(self) -> u16 {
        self.artist as u16 * 2 + self.gameplay as u16 * 3 + self.editor as u16 * 4
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ManyLightShadowPolicy {
    #[default]
    None,
    DemandPaged {
        page_budget_hint: u16,
    },
    DirectionalClipmap {
        page_budget_hint: u16,
    },
}

impl ManyLightShadowPolicy {
    #[must_use]
    pub const fn casts_shadow(self) -> bool {
        !matches!(self, Self::None)
    }

    #[must_use]
    pub const fn page_budget_hint(self) -> u16 {
        match self {
            Self::None => 0,
            Self::DemandPaged { page_budget_hint }
            | Self::DirectionalClipmap { page_budget_hint } => page_budget_hint,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::DemandPaged { .. } => "demand_paged",
            Self::DirectionalClipmap { .. } => "directional_clipmap",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuLightRecord {
    pub stable_light_id: LuxLightId,
    pub kind: LuxLightKind,
    pub transform: LightTransform,
    pub color_intensity: LightColorIntensity,
    pub shape: LightShape,
    pub shadow_policy: ManyLightShadowPolicy,
    pub update_stamp: LightUpdateStamp,
    pub importance: LightImportanceHints,
    pub emissive_source: Option<EmissiveSourceRef>,
}

impl GpuLightRecord {
    #[must_use]
    pub fn direct(
        stable_light_id: LuxLightId,
        kind: LuxLightKind,
        transform: LightTransform,
        intensity_lux: f32,
        importance: LightImportanceHints,
    ) -> Self {
        let shadow_policy = if matches!(kind, LuxLightKind::Directional) {
            ManyLightShadowPolicy::DirectionalClipmap {
                page_budget_hint: 16,
            }
        } else {
            ManyLightShadowPolicy::DemandPaged {
                page_budget_hint: 4,
            }
        };
        Self {
            stable_light_id,
            kind,
            transform,
            color_intensity: LightColorIntensity::new([1.0, 1.0, 1.0], intensity_lux),
            shape: if matches!(kind, LuxLightKind::Directional) {
                LightShape::DIRECTIONAL
            } else {
                LightShape::sphere(12.0)
            },
            shadow_policy,
            update_stamp: LightUpdateStamp::new(1),
            importance,
            emissive_source: None,
        }
    }

    #[must_use]
    pub const fn with_update_stamp(mut self, update_stamp: LightUpdateStamp) -> Self {
        self.update_stamp = update_stamp;
        self
    }

    #[must_use]
    pub const fn with_emissive_source(mut self, source: EmissiveSourceRef) -> Self {
        self.kind = LuxLightKind::EmissiveCandidate;
        self.emissive_source = Some(source);
        self
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GpuLightDatabaseDiagnostics {
    pub revision: u64,
    pub light_count: u32,
    pub direct_light_count: u32,
    pub emissive_light_count: u32,
    pub shadow_casting_light_count: u32,
    pub update_stamp_max: u64,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct GpuLightDatabase {
    revision: u64,
    records: Vec<GpuLightRecord>,
    index_by_light: BTreeMap<u64, u32>,
}

impl GpuLightDatabase {
    #[must_use]
    pub fn records(&self) -> &[GpuLightRecord] {
        &self.records
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn get(&self, light_id: LuxLightId) -> Option<&GpuLightRecord> {
        self.index_by_light
            .get(&light_id.0)
            .and_then(|index| self.records.get(*index as usize))
    }

    pub fn upsert(&mut self, mut record: GpuLightRecord) -> u32 {
        self.revision = self.revision.saturating_add(1);
        record.update_stamp = record.update_stamp.next();
        if let Some(index) = self.index_by_light.get(&record.stable_light_id.0).copied() {
            self.records[index as usize] = record;
            return index;
        }
        let Ok(index) = u32::try_from(self.records.len()) else {
            return u32::MAX;
        };
        self.index_by_light.insert(record.stable_light_id.0, index);
        self.records.push(record);
        index
    }

    pub fn remove(&mut self, light_id: LuxLightId) -> bool {
        let Some(index) = self.index_by_light.remove(&light_id.0) else {
            return false;
        };
        self.records.swap_remove(index as usize);
        if let Some(swapped) = self.records.get(index as usize) {
            self.index_by_light.insert(swapped.stable_light_id.0, index);
        }
        self.revision = self.revision.saturating_add(1);
        true
    }

    #[must_use]
    pub fn diagnostics(&self) -> GpuLightDatabaseDiagnostics {
        let mut diagnostics = GpuLightDatabaseDiagnostics {
            revision: self.revision,
            light_count: self.records.len() as u32,
            ..GpuLightDatabaseDiagnostics::default()
        };
        for record in &self.records {
            diagnostics.update_stamp_max = diagnostics.update_stamp_max.max(record.update_stamp.0);
            if record.emissive_source.is_some() || record.kind == LuxLightKind::EmissiveCandidate {
                diagnostics.emissive_light_count =
                    diagnostics.emissive_light_count.saturating_add(1);
            } else {
                diagnostics.direct_light_count = diagnostics.direct_light_count.saturating_add(1);
            }
            if record.shadow_policy.casts_shadow() {
                diagnostics.shadow_casting_light_count =
                    diagnostics.shadow_casting_light_count.saturating_add(1);
            }
        }
        diagnostics
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmissiveSourceRecord {
    pub source: EmissiveSourceRef,
    pub stable_light_id: LuxLightId,
    pub transform: LightTransform,
    pub color_intensity: LightColorIntensity,
    pub luminance: f32,
    pub area_m2: f32,
    pub importance: LightImportanceHints,
    pub promotion_policy: EmissivePromotionPolicy,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum EmissivePromotionPolicy {
    Never,
    #[default]
    ImportanceThreshold,
    Always,
}

impl EmissiveSourceRecord {
    #[must_use]
    pub fn promoted_light(self) -> Option<GpuLightRecord> {
        let luminous_power = self.luminance.max(0.0) * self.area_m2.max(0.0);
        let important = self.importance.score() >= 320 || luminous_power >= 4096.0;
        match self.promotion_policy {
            EmissivePromotionPolicy::Never => None,
            EmissivePromotionPolicy::ImportanceThreshold if !important => None,
            EmissivePromotionPolicy::ImportanceThreshold | EmissivePromotionPolicy::Always => {
                Some(GpuLightRecord {
                    stable_light_id: self.stable_light_id,
                    kind: LuxLightKind::EmissiveCandidate,
                    transform: self.transform,
                    color_intensity: self.color_intensity,
                    shape: LightShape::sphere(self.area_m2.sqrt().max(0.25)),
                    shadow_policy: ManyLightShadowPolicy::None,
                    update_stamp: LightUpdateStamp::new(1),
                    importance: self.importance,
                    emissive_source: Some(self.source),
                })
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LightClusterKey {
    pub tile_x: u16,
    pub tile_y: u16,
    pub depth_slice: u16,
}

impl LightClusterKey {
    #[must_use]
    pub const fn new(tile_x: u16, tile_y: u16, depth_slice: u16) -> Self {
        Self {
            tile_x,
            tile_y,
            depth_slice,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClusterGridConfig {
    pub tile_count_x: u16,
    pub tile_count_y: u16,
    pub depth_slices: u16,
    pub max_candidates_per_cluster: u16,
}

impl ClusterGridConfig {
    pub const DEFAULT: Self = Self {
        tile_count_x: 16,
        tile_count_y: 9,
        depth_slices: 16,
        max_candidates_per_cluster: 32,
    };

    #[must_use]
    pub const fn stress(
        tile_count_x: u16,
        tile_count_y: u16,
        depth_slices: u16,
        max_candidates_per_cluster: u16,
    ) -> Self {
        Self {
            tile_count_x,
            tile_count_y,
            depth_slices,
            max_candidates_per_cluster,
        }
    }

    #[must_use]
    pub fn cluster_count(self) -> u32 {
        u32::from(self.tile_count_x)
            .saturating_mul(u32::from(self.tile_count_y))
            .saturating_mul(u32::from(self.depth_slices))
    }
}

impl Default for ClusterGridConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum CandidateGenerationMode {
    #[default]
    ClusteredReservoir,
    ForwardPlusFallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateSource {
    DirectLight,
    EmissivePromoted,
}

impl CandidateSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DirectLight => "direct_light",
            Self::EmissivePromoted => "emissive_promoted",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LightCandidate {
    pub cluster: LightClusterKey,
    pub light_id: LuxLightId,
    pub source: CandidateSource,
    pub score: u16,
    pub update_stamp: LightUpdateStamp,
    pub casts_shadow: bool,
    pub shadow_budget_hint: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterCandidateList {
    pub cluster: LightClusterKey,
    pub candidates: Vec<LightCandidate>,
    pub overflowed_candidates: u32,
}

impl ClusterCandidateList {
    #[must_use]
    pub const fn new(cluster: LightClusterKey) -> Self {
        Self {
            cluster,
            candidates: Vec::new(),
            overflowed_candidates: 0,
        }
    }

    fn insert_capped(&mut self, candidate: LightCandidate, max_candidates: u16) {
        self.candidates.push(candidate);
        self.candidates.sort_by(compare_candidate_priority);
        let max = usize::from(max_candidates);
        if self.candidates.len() > max {
            self.candidates.truncate(max);
            self.overflowed_candidates = self.overflowed_candidates.saturating_add(1);
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ClusteredCandidateSet {
    pub mode: CandidateGenerationMode,
    pub grid: ClusterGridConfig,
    pub input_light_count: u32,
    pub promoted_emissive_count: u32,
    pub total_candidate_references: u32,
    pub overflowed_candidates: u32,
    pub lists: BTreeMap<LightClusterKey, ClusterCandidateList>,
}

impl ClusteredCandidateSet {
    #[must_use]
    pub fn occupied_cluster_count(&self) -> u32 {
        self.lists.len() as u32
    }

    #[must_use]
    pub fn candidate_pressure_per_cluster_per_mille(&self) -> u16 {
        let capacity = self
            .grid
            .cluster_count()
            .saturating_mul(u32::from(self.grid.max_candidates_per_cluster));
        if capacity == 0 {
            return 0;
        }
        ((u64::from(self.total_candidate_references) * 1000) / u64::from(capacity)).min(1000) as u16
    }
}

pub fn generate_clustered_candidates(
    database: &GpuLightDatabase,
    emissives: &[EmissiveSourceRecord],
    config: ClusterGridConfig,
) -> ClusteredCandidateSet {
    generate_candidates(
        database,
        emissives,
        config,
        CandidateGenerationMode::ClusteredReservoir,
    )
}

pub fn generate_forward_plus_fallback(
    database: &GpuLightDatabase,
    emissives: &[EmissiveSourceRecord],
    config: ClusterGridConfig,
) -> ClusteredCandidateSet {
    generate_candidates(
        database,
        emissives,
        config,
        CandidateGenerationMode::ForwardPlusFallback,
    )
}

fn generate_candidates(
    database: &GpuLightDatabase,
    emissives: &[EmissiveSourceRecord],
    config: ClusterGridConfig,
    mode: CandidateGenerationMode,
) -> ClusteredCandidateSet {
    let mut candidates = ClusteredCandidateSet {
        mode,
        grid: config,
        input_light_count: database.records().len() as u32,
        ..ClusteredCandidateSet::default()
    };
    for record in database.records() {
        insert_record_candidates(record, config, &mut candidates);
    }
    for emissive in emissives {
        if let Some(record) = emissive.promoted_light() {
            candidates.promoted_emissive_count =
                candidates.promoted_emissive_count.saturating_add(1);
            candidates.input_light_count = candidates.input_light_count.saturating_add(1);
            insert_record_candidates(&record, config, &mut candidates);
        }
    }
    candidates.overflowed_candidates = candidates
        .lists
        .values()
        .map(|list| list.overflowed_candidates)
        .sum();
    candidates
}

fn insert_record_candidates(
    record: &GpuLightRecord,
    config: ClusterGridConfig,
    candidates: &mut ClusteredCandidateSet,
) {
    let source =
        if record.emissive_source.is_some() || record.kind == LuxLightKind::EmissiveCandidate {
            CandidateSource::EmissivePromoted
        } else {
            CandidateSource::DirectLight
        };
    let key = cluster_key_for_record(record, config);
    let candidate = LightCandidate {
        cluster: key,
        light_id: record.stable_light_id,
        source,
        score: candidate_score(record),
        update_stamp: record.update_stamp,
        casts_shadow: record.shadow_policy.casts_shadow(),
        shadow_budget_hint: record.shadow_policy.page_budget_hint(),
    };
    candidates
        .lists
        .entry(key)
        .or_insert_with(|| ClusterCandidateList::new(key))
        .insert_capped(candidate, config.max_candidates_per_cluster);
    candidates.total_candidate_references = candidates.total_candidate_references.saturating_add(1);
}

fn cluster_key_for_record(record: &GpuLightRecord, config: ClusterGridConfig) -> LightClusterKey {
    if matches!(record.kind, LuxLightKind::Directional) {
        return LightClusterKey::new(0, 0, 0);
    }
    let x = normalized_to_tile(
        record.transform.position_view[0],
        config.tile_count_x.saturating_sub(1),
    );
    let y = normalized_to_tile(
        record.transform.position_view[1],
        config.tile_count_y.saturating_sub(1),
    );
    let z = depth_to_slice(
        record.transform.position_view[2],
        config.depth_slices.saturating_sub(1),
    );
    LightClusterKey::new(x, y, z)
}

fn normalized_to_tile(value: f32, max_tile: u16) -> u16 {
    let normalized = ((value.clamp(-1.0, 1.0) + 1.0) * 0.5 * f32::from(max_tile)).round();
    normalized.clamp(0.0, f32::from(max_tile)) as u16
}

fn depth_to_slice(value: f32, max_slice: u16) -> u16 {
    let normalized = (value.clamp(0.0, 4096.0) / 4096.0 * f32::from(max_slice)).round();
    normalized.clamp(0.0, f32::from(max_slice)) as u16
}

fn candidate_score(record: &GpuLightRecord) -> u16 {
    let intensity = (record
        .color_intensity
        .intensity_lux
        .max(0.0)
        .log10()
        .max(0.0)
        * 64.0) as u32;
    let shadow = u32::from(record.shadow_policy.casts_shadow()) * 96;
    let radius = record.shape.radius_m.clamp(0.0, 256.0) as u32;
    let score = u32::from(record.importance.score())
        .saturating_add(intensity)
        .saturating_add(shadow)
        .saturating_add(radius);
    score.min(u32::from(u16::MAX)) as u16
}

fn compare_candidate_priority(left: &LightCandidate, right: &LightCandidate) -> std::cmp::Ordering {
    right
        .score
        .cmp(&left.score)
        .then_with(|| left.cluster.cmp(&right.cluster))
        .then_with(|| left.light_id.0.cmp(&right.light_id.0))
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ReservoirReuseValidity {
    #[default]
    Fresh,
    TemporalReused,
    SpatialReused,
    Rejected,
}

impl ReservoirReuseValidity {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::TemporalReused => "temporal_reused",
            Self::SpatialReused => "spatial_reused",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ReservoirRejectionReason {
    #[default]
    None,
    EmptyCandidateList,
    PreviousMissing,
    UpdateStampChanged,
    SourceLightRemoved,
    ShadowBudgetDenied,
}

impl ReservoirRejectionReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::EmptyCandidateList => "empty_candidate_list",
            Self::PreviousMissing => "previous_missing",
            Self::UpdateStampChanged => "update_stamp_changed",
            Self::SourceLightRemoved => "source_light_removed",
            Self::ShadowBudgetDenied => "shadow_budget_denied",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReservoirRecord {
    pub cluster: LightClusterKey,
    pub candidate_source: CandidateSource,
    pub selected_light: LuxLightId,
    pub selected_score: u16,
    pub selected_update_stamp: LightUpdateStamp,
    pub reuse_validity: ReservoirReuseValidity,
    pub rejection_reason: ReservoirRejectionReason,
    pub shadowed_candidate_count: u16,
}

impl ReservoirRecord {
    #[must_use]
    pub const fn rejected(cluster: LightClusterKey, reason: ReservoirRejectionReason) -> Self {
        Self {
            cluster,
            candidate_source: CandidateSource::DirectLight,
            selected_light: LuxLightId::INVALID,
            selected_score: 0,
            selected_update_stamp: LightUpdateStamp(0),
            reuse_validity: ReservoirReuseValidity::Rejected,
            rejection_reason: reason,
            shadowed_candidate_count: 0,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ReservoirStorage {
    pub frame_index: u64,
    pub records: BTreeMap<LightClusterKey, ReservoirRecord>,
    pub temporal_reuse_count: u32,
    pub spatial_reuse_count: u32,
    pub rejected_count: u32,
    pub shadowed_candidate_count: u32,
}

impl ReservoirStorage {
    #[must_use]
    pub fn selected_light_count(&self) -> u32 {
        self.records
            .values()
            .filter(|record| record.selected_light.is_valid())
            .count() as u32
    }
}

pub fn select_reservoirs(
    candidates: &ClusteredCandidateSet,
    previous: Option<&ReservoirStorage>,
    frame_index: u64,
) -> ReservoirStorage {
    let mut storage = ReservoirStorage {
        frame_index,
        ..ReservoirStorage::default()
    };

    for (cluster, list) in &candidates.lists {
        let Some(best) = list.candidates.first().copied() else {
            storage.records.insert(
                *cluster,
                ReservoirRecord::rejected(*cluster, ReservoirRejectionReason::EmptyCandidateList),
            );
            storage.rejected_count = storage.rejected_count.saturating_add(1);
            continue;
        };
        let mut record = ReservoirRecord {
            cluster: *cluster,
            candidate_source: best.source,
            selected_light: best.light_id,
            selected_score: best.score,
            selected_update_stamp: best.update_stamp,
            reuse_validity: ReservoirReuseValidity::Fresh,
            rejection_reason: ReservoirRejectionReason::None,
            shadowed_candidate_count: list
                .candidates
                .iter()
                .filter(|candidate| candidate.casts_shadow)
                .count()
                .min(usize::from(u16::MAX)) as u16,
        };
        if let Some(previous_record) = previous.and_then(|previous| previous.records.get(cluster)) {
            match list
                .candidates
                .iter()
                .find(|candidate| candidate.light_id == previous_record.selected_light)
            {
                Some(candidate)
                    if candidate.update_stamp == previous_record.selected_update_stamp =>
                {
                    record.candidate_source = candidate.source;
                    record.selected_light = candidate.light_id;
                    record.selected_score = candidate.score;
                    record.selected_update_stamp = candidate.update_stamp;
                    record.reuse_validity = ReservoirReuseValidity::TemporalReused;
                    storage.temporal_reuse_count = storage.temporal_reuse_count.saturating_add(1);
                }
                Some(_) => {
                    record.rejection_reason = ReservoirRejectionReason::UpdateStampChanged;
                }
                None if previous_record.selected_light.is_valid() => {
                    record.rejection_reason = ReservoirRejectionReason::PreviousMissing;
                }
                None => {}
            }
        }
        storage.shadowed_candidate_count = storage
            .shadowed_candidate_count
            .saturating_add(u32::from(record.shadowed_candidate_count));
        storage.records.insert(*cluster, record);
    }

    spatially_reuse_empty_neighbors(candidates.grid, &mut storage);
    storage
}

fn spatially_reuse_empty_neighbors(config: ClusterGridConfig, storage: &mut ReservoirStorage) {
    if storage.records.is_empty() || config.tile_count_x == 0 || config.tile_count_y == 0 {
        return;
    }
    let occupied: Vec<ReservoirRecord> = storage.records.values().copied().collect();
    for depth_slice in 0..config.depth_slices.min(2) {
        for tile_y in 0..config.tile_count_y.min(2) {
            for tile_x in 0..config.tile_count_x.min(2) {
                let key = LightClusterKey::new(tile_x, tile_y, depth_slice);
                if storage.records.contains_key(&key) {
                    continue;
                }
                let Some(source) = occupied.first().copied() else {
                    continue;
                };
                storage.records.insert(
                    key,
                    ReservoirRecord {
                        cluster: key,
                        reuse_validity: ReservoirReuseValidity::SpatialReused,
                        ..source
                    },
                );
                storage.spatial_reuse_count = storage.spatial_reuse_count.saturating_add(1);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManyLightShadowRequest {
    pub light_id: LuxLightId,
    pub cluster: LightClusterKey,
    pub candidate_source: CandidateSource,
    pub priority: u16,
    pub requested_page_budget: u16,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ShadowInteractionDiagnostics {
    pub selected_shadowed_lights: u32,
    pub emitted_shadow_requests: u32,
    pub denied_shadow_requests: u32,
    pub requested_shadow_pages: u32,
    pub highest_shadow_priority: u16,
}

pub fn build_shadow_requests_for_reservoirs(
    reservoirs: &ReservoirStorage,
    database: &GpuLightDatabase,
    policy_config: ShadowPolicyConfig,
    receiver_demand: ShadowReceiverDemand,
    max_requests: u16,
) -> (Vec<ManyLightShadowRequest>, ShadowInteractionDiagnostics) {
    let mut engine = ShadowPolicyEngine::new(policy_config);
    let mut requests = Vec::new();
    let mut diagnostics = ShadowInteractionDiagnostics::default();
    let mut sorted_records: Vec<ReservoirRecord> = reservoirs.records.values().copied().collect();
    sorted_records.sort_by(|left, right| {
        right
            .selected_score
            .cmp(&left.selected_score)
            .then_with(|| left.cluster.cmp(&right.cluster))
            .then_with(|| left.selected_light.0.cmp(&right.selected_light.0))
    });
    for record in sorted_records {
        if requests.len() >= usize::from(max_requests) {
            break;
        }
        let Some(light) = database.get(record.selected_light) else {
            diagnostics.denied_shadow_requests =
                diagnostics.denied_shadow_requests.saturating_add(1);
            continue;
        };
        if !light.shadow_policy.casts_shadow() {
            continue;
        }
        diagnostics.selected_shadowed_lights =
            diagnostics.selected_shadowed_lights.saturating_add(1);
        let decision = engine.evaluate_light(
            ShadowLightInput::new(
                light.stable_light_id,
                light.kind,
                light.color_intensity.intensity_lux,
                light.importance.score().min(u16::from(u8::MAX)) as u8,
                true,
            ),
            receiver_demand,
        );
        if !decision.casts_shadow() {
            diagnostics.denied_shadow_requests =
                diagnostics.denied_shadow_requests.saturating_add(1);
            continue;
        }
        let importance_pages = 1_u16.saturating_add((light.importance.score() / 192).min(15));
        let hinted_pages = light.shadow_policy.page_budget_hint().max(importance_pages);
        let requested_page_budget = hinted_pages.min(decision.page_budget as u16);
        diagnostics.requested_shadow_pages = diagnostics
            .requested_shadow_pages
            .saturating_add(u32::from(requested_page_budget));
        diagnostics.highest_shadow_priority =
            diagnostics.highest_shadow_priority.max(decision.priority);
        requests.push(ManyLightShadowRequest {
            light_id: light.stable_light_id,
            cluster: record.cluster,
            candidate_source: record.candidate_source,
            priority: decision.priority,
            requested_page_budget,
        });
    }
    diagnostics.emitted_shadow_requests = requests.len() as u32;
    (requests, diagnostics)
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ManyLightFrameDiagnostics {
    pub input_lights: u32,
    pub promoted_emissives: u32,
    pub occupied_clusters: u32,
    pub candidate_references: u32,
    pub candidate_pressure_per_cluster_per_mille: u16,
    pub overflowed_candidates: u32,
    pub selected_lights: u32,
    pub temporal_reuse_count: u32,
    pub spatial_reuse_count: u32,
    pub rejected_reservoir_count: u32,
    pub shadowed_candidate_count: u32,
    pub shadow_requests: u32,
    pub requested_shadow_pages: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManyLightDebugArtifact {
    pub schema_version: u16,
    pub content: String,
}

impl ManyLightDebugArtifact {
    #[must_use]
    pub fn benchmark(
        candidates: &ClusteredCandidateSet,
        reservoirs: &ReservoirStorage,
        shadows: ShadowInteractionDiagnostics,
    ) -> Self {
        let diagnostics = frame_diagnostics(candidates, reservoirs, shadows);
        let mut content = String::new();
        let _ = writeln!(
            content,
            "many_light_schema_version={MANY_LIGHT_SCHEMA_VERSION}"
        );
        let _ = writeln!(content, "input_lights={}", diagnostics.input_lights);
        let _ = writeln!(
            content,
            "promoted_emissives={}",
            diagnostics.promoted_emissives
        );
        let _ = writeln!(
            content,
            "occupied_clusters={}",
            diagnostics.occupied_clusters
        );
        let _ = writeln!(
            content,
            "candidate_references={}",
            diagnostics.candidate_references
        );
        let _ = writeln!(
            content,
            "candidate_pressure_per_cluster_per_mille={}",
            diagnostics.candidate_pressure_per_cluster_per_mille
        );
        let _ = writeln!(
            content,
            "overflowed_candidates={}",
            diagnostics.overflowed_candidates
        );
        let _ = writeln!(content, "selected_lights={}", diagnostics.selected_lights);
        let _ = writeln!(
            content,
            "temporal_reuse_count={}",
            diagnostics.temporal_reuse_count
        );
        let _ = writeln!(
            content,
            "spatial_reuse_count={}",
            diagnostics.spatial_reuse_count
        );
        let _ = writeln!(
            content,
            "shadowed_candidate_count={}",
            diagnostics.shadowed_candidate_count
        );
        let _ = writeln!(content, "shadow_requests={}", diagnostics.shadow_requests);
        let _ = writeln!(
            content,
            "requested_shadow_pages={}",
            diagnostics.requested_shadow_pages
        );
        Self {
            schema_version: MANY_LIGHT_SCHEMA_VERSION,
            content,
        }
    }

    #[must_use]
    pub fn debug_overlay(
        candidates: &ClusteredCandidateSet,
        reservoirs: &ReservoirStorage,
    ) -> Self {
        let mut content = String::new();
        let _ = writeln!(
            content,
            "many_light_overlay_schema_version={MANY_LIGHT_SCHEMA_VERSION}"
        );
        let _ = writeln!(content, "mode={:?}", candidates.mode);
        for record in reservoirs.records.values().take(16) {
            let _ = writeln!(
                content,
                "cluster=({}, {}, {}) selected_light={} source={} reuse={} rejection={} score={} shadowed_candidates={}",
                record.cluster.tile_x,
                record.cluster.tile_y,
                record.cluster.depth_slice,
                record.selected_light.0,
                record.candidate_source.as_str(),
                record.reuse_validity.as_str(),
                record.rejection_reason.as_str(),
                record.selected_score,
                record.shadowed_candidate_count
            );
        }
        Self {
            schema_version: MANY_LIGHT_SCHEMA_VERSION,
            content,
        }
    }

    #[must_use]
    pub fn shadow_interaction(
        requests: &[ManyLightShadowRequest],
        diagnostics: ShadowInteractionDiagnostics,
    ) -> Self {
        let mut content = String::new();
        let _ = writeln!(
            content,
            "many_light_shadow_schema_version={MANY_LIGHT_SCHEMA_VERSION}"
        );
        let _ = writeln!(
            content,
            "selected_shadowed_lights={}",
            diagnostics.selected_shadowed_lights
        );
        let _ = writeln!(
            content,
            "emitted_shadow_requests={}",
            diagnostics.emitted_shadow_requests
        );
        let _ = writeln!(
            content,
            "denied_shadow_requests={}",
            diagnostics.denied_shadow_requests
        );
        let _ = writeln!(
            content,
            "requested_shadow_pages={}",
            diagnostics.requested_shadow_pages
        );
        for request in requests {
            let _ = writeln!(
                content,
                "shadow_request light={} cluster=({}, {}, {}) source={} priority={} pages={}",
                request.light_id.0,
                request.cluster.tile_x,
                request.cluster.tile_y,
                request.cluster.depth_slice,
                request.candidate_source.as_str(),
                request.priority,
                request.requested_page_budget
            );
        }
        Self {
            schema_version: MANY_LIGHT_SCHEMA_VERSION,
            content,
        }
    }
}

#[must_use]
pub fn frame_diagnostics(
    candidates: &ClusteredCandidateSet,
    reservoirs: &ReservoirStorage,
    shadows: ShadowInteractionDiagnostics,
) -> ManyLightFrameDiagnostics {
    ManyLightFrameDiagnostics {
        input_lights: candidates.input_light_count,
        promoted_emissives: candidates.promoted_emissive_count,
        occupied_clusters: candidates.occupied_cluster_count(),
        candidate_references: candidates.total_candidate_references,
        candidate_pressure_per_cluster_per_mille: candidates
            .candidate_pressure_per_cluster_per_mille(),
        overflowed_candidates: candidates.overflowed_candidates,
        selected_lights: reservoirs.selected_light_count(),
        temporal_reuse_count: reservoirs.temporal_reuse_count,
        spatial_reuse_count: reservoirs.spatial_reuse_count,
        rejected_reservoir_count: reservoirs.rejected_count,
        shadowed_candidate_count: reservoirs.shadowed_candidate_count,
        shadow_requests: shadows.emitted_shadow_requests,
        requested_shadow_pages: shadows.requested_shadow_pages,
    }
}

pub fn write_many_light_artifact(
    path: impl AsRef<Path>,
    artifact: &ManyLightDebugArtifact,
) -> io::Result<()> {
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

    fn record(
        id: u64,
        x: f32,
        y: f32,
        z: f32,
        intensity_lux: f32,
        importance: u8,
    ) -> GpuLightRecord {
        GpuLightRecord::direct(
            LuxLightId::new(id),
            LuxLightKind::Punctual,
            LightTransform::new([x, y, z], [0.0, -1.0, 0.0]),
            intensity_lux,
            LightImportanceHints::new(importance, importance / 2, 0),
        )
        .with_update_stamp(LightUpdateStamp::new(id))
    }

    fn populated_database(count: u32) -> GpuLightDatabase {
        let mut database = GpuLightDatabase::default();
        for index in 0..count {
            let x = (index % 16) as f32 / 8.0 - 1.0;
            let y = ((index / 16) % 9) as f32 / 4.5 - 1.0;
            let z = 32.0 + (index % 64) as f32 * 8.0;
            let importance = (index % 255) as u8;
            database.upsert(record(
                u64::from(index) + 1,
                x,
                y,
                z,
                250.0 + index as f32 * 8.0,
                importance,
            ));
        }
        database
    }

    #[test]
    fn gpu_light_database_tracks_stable_records_and_update_stamps() {
        let mut database = GpuLightDatabase::default();
        let id = LuxLightId::new(9);

        assert_eq!(database.upsert(record(9, 0.0, 0.0, 10.0, 1000.0, 50)), 0);
        assert_eq!(database.upsert(record(9, 0.25, 0.25, 20.0, 5000.0, 80)), 0);

        let diagnostics = database.diagnostics();
        assert_eq!(diagnostics.light_count, 1);
        assert_eq!(diagnostics.direct_light_count, 1);
        assert_eq!(diagnostics.shadow_casting_light_count, 1);
        assert_eq!(database.get(id).expect("light exists").stable_light_id, id);
        assert!(database.remove(id));
        assert_eq!(database.diagnostics().light_count, 0);
    }

    #[test]
    fn clustered_candidate_generation_caps_pressure_instead_of_lights_times_pixels() {
        let database = populated_database(2048);
        let candidates =
            generate_clustered_candidates(&database, &[], ClusterGridConfig::stress(16, 9, 8, 8));

        assert_eq!(candidates.input_light_count, 2048);
        assert!(candidates.occupied_cluster_count() <= 16 * 9 * 8);
        assert!(candidates.total_candidate_references >= 2048);
        assert!(candidates.overflowed_candidates > 0);
        for list in candidates.lists.values() {
            assert!(list.candidates.len() <= 8);
        }
    }

    #[test]
    fn forward_plus_fallback_preserves_clustered_candidate_lists() {
        let database = populated_database(64);
        let candidates =
            generate_forward_plus_fallback(&database, &[], ClusterGridConfig::stress(8, 4, 4, 4));

        assert_eq!(
            candidates.mode,
            CandidateGenerationMode::ForwardPlusFallback
        );
        assert!(candidates.occupied_cluster_count() > 0);
        for list in candidates.lists.values() {
            assert!(list.candidates.len() <= 4);
        }
    }

    #[test]
    fn reservoir_temporal_reuse_keeps_stable_selected_lights() {
        let database = populated_database(128);
        let candidates =
            generate_clustered_candidates(&database, &[], ClusterGridConfig::stress(8, 4, 4, 8));
        let first = select_reservoirs(&candidates, None, 10);
        let second = select_reservoirs(&candidates, Some(&first), 11);

        assert!(first.selected_light_count() > 0);
        assert!(second.temporal_reuse_count > 0);
        assert_eq!(second.rejected_count, 0);
    }

    #[test]
    fn reservoir_rejects_temporal_reuse_when_light_update_stamp_changes() {
        let mut database = GpuLightDatabase::default();
        database.upsert(record(1, 0.0, 0.0, 10.0, 10_000.0, 200));
        let candidates =
            generate_clustered_candidates(&database, &[], ClusterGridConfig::stress(4, 4, 2, 4));
        let first = select_reservoirs(&candidates, None, 1);
        database.upsert(
            record(1, 0.0, 0.0, 10.0, 20_000.0, 200).with_update_stamp(LightUpdateStamp::new(99)),
        );
        let changed =
            generate_clustered_candidates(&database, &[], ClusterGridConfig::stress(4, 4, 2, 4));
        let second = select_reservoirs(&changed, Some(&first), 2);

        assert!(
            second
                .records
                .values()
                .any(|record| record.rejection_reason
                    == ReservoirRejectionReason::UpdateStampChanged)
        );
    }

    #[test]
    fn important_emissives_are_promoted_but_low_importance_emitters_stay_cache_only() {
        let database = GpuLightDatabase::default();
        let important = EmissiveSourceRecord {
            source: EmissiveSourceRef::new(1),
            stable_light_id: LuxLightId::new(1001),
            transform: LightTransform::new([0.1, 0.1, 40.0], [0.0, -1.0, 0.0]),
            color_intensity: LightColorIntensity::new([1.0, 0.8, 0.6], 6000.0),
            luminance: 8000.0,
            area_m2: 2.0,
            importance: LightImportanceHints::new(200, 80, 0),
            promotion_policy: EmissivePromotionPolicy::ImportanceThreshold,
        };
        let low = EmissiveSourceRecord {
            source: EmissiveSourceRef::new(2),
            stable_light_id: LuxLightId::new(1002),
            luminance: 10.0,
            area_m2: 0.1,
            importance: LightImportanceHints::new(1, 0, 0),
            promotion_policy: EmissivePromotionPolicy::ImportanceThreshold,
            ..important
        };

        let candidates = generate_clustered_candidates(
            &database,
            &[important, low],
            ClusterGridConfig::stress(8, 4, 4, 4),
        );

        assert_eq!(candidates.promoted_emissive_count, 1);
        assert!(
            candidates
                .lists
                .values()
                .flat_map(|list| list.candidates.iter())
                .any(|candidate| candidate.source == CandidateSource::EmissivePromoted)
        );
    }

    #[test]
    fn winning_shadowed_candidates_emit_budgeted_shadow_requests() {
        let mut database = GpuLightDatabase::default();
        database.upsert(record(1, 0.0, 0.0, 20.0, 80_000.0, 255));
        database.upsert(record(2, 0.5, 0.2, 30.0, 5_000.0, 30));
        let candidates =
            generate_clustered_candidates(&database, &[], ClusterGridConfig::stress(8, 4, 4, 4));
        let reservoirs = select_reservoirs(&candidates, None, 1);

        let (requests, diagnostics) = build_shadow_requests_for_reservoirs(
            &reservoirs,
            &database,
            ShadowPolicyConfig {
                local_light_shadow_budget: 8,
                max_shadow_casters_per_frame: 4,
                ..ShadowPolicyConfig::DEFAULT
            },
            ShadowReceiverDemand {
                visible_receiver_demand: 200,
                screen_coverage: 120,
                contrast: 60,
                temporal_instability: 10,
                gameplay_salience: 200,
                editor_focus: 0,
            },
            4,
        );

        assert!(!requests.is_empty());
        assert!(diagnostics.requested_shadow_pages > 0);
        assert!(diagnostics.highest_shadow_priority > 0);
    }

    #[test]
    fn many_light_benchmark_shadow_and_overlay_artifacts_are_stable() {
        let database = populated_database(256);
        let emissive = EmissiveSourceRecord {
            source: EmissiveSourceRef::new(55),
            stable_light_id: LuxLightId::new(9001),
            transform: LightTransform::new([0.0, 0.0, 64.0], [0.0, -1.0, 0.0]),
            color_intensity: LightColorIntensity::new([0.4, 0.8, 1.0], 12_000.0),
            luminance: 12_000.0,
            area_m2: 4.0,
            importance: LightImportanceHints::new(220, 120, 0),
            promotion_policy: EmissivePromotionPolicy::ImportanceThreshold,
        };
        let candidates = generate_clustered_candidates(
            &database,
            &[emissive],
            ClusterGridConfig::stress(8, 4, 4, 8),
        );
        let reservoirs = select_reservoirs(&candidates, None, 99);
        let (requests, shadow_diagnostics) = build_shadow_requests_for_reservoirs(
            &reservoirs,
            &database,
            ShadowPolicyConfig::DEFAULT,
            ShadowReceiverDemand {
                visible_receiver_demand: 120,
                screen_coverage: 80,
                contrast: 32,
                temporal_instability: 8,
                gameplay_salience: 90,
                editor_focus: 0,
            },
            16,
        );

        let benchmark =
            ManyLightDebugArtifact::benchmark(&candidates, &reservoirs, shadow_diagnostics);
        assert!(
            benchmark
                .content
                .contains("candidate_pressure_per_cluster_per_mille=")
        );
        assert!(benchmark.content.contains("selected_lights="));
        let shadow = ManyLightDebugArtifact::shadow_interaction(&requests, shadow_diagnostics);
        assert!(shadow.content.contains("many_light_shadow_schema_version="));
        let overlay = ManyLightDebugArtifact::debug_overlay(&candidates, &reservoirs);
        assert!(overlay.content.contains("cluster=("));

        if let Ok(path) = std::env::var(MANY_LIGHT_BENCHMARK_ARTIFACT_ENV) {
            write_many_light_artifact(path, &benchmark)
                .expect("many-light benchmark artifact should be writable");
        }
        if let Ok(path) = std::env::var(MANY_LIGHT_SHADOW_ARTIFACT_ENV) {
            write_many_light_artifact(path, &shadow)
                .expect("many-light shadow artifact should be writable");
        }
        if let Ok(path) = std::env::var(MANY_LIGHT_DEBUG_OVERLAY_ARTIFACT_ENV) {
            write_many_light_artifact(path, &overlay)
                .expect("many-light debug overlay artifact should be writable");
        }
    }
}
