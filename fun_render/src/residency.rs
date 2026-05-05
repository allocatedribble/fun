use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::Resource;

use crate::FunDrawBudgetLane;

pub const FUN_RESIDENCY_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GeometryPageId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TexturePageId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MaterialEntryId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunResidencyBudget {
    pub max_geometry_page_uploads_per_frame: u32,
    pub max_geometry_upload_bytes_per_frame: u64,
    pub max_texture_uploads_per_frame: u32,
    pub max_texture_upload_bytes_per_frame: u64,
    pub max_material_table_updates_per_frame: u32,
    pub max_descriptor_updates_per_frame: u32,
    pub max_evictions_per_frame: u32,
}

impl FunResidencyBudget {
    pub const fn for_lane(lane: FunDrawBudgetLane) -> Self {
        match lane {
            FunDrawBudgetLane::PresentationFloor => Self {
                max_geometry_page_uploads_per_frame: 2,
                max_geometry_upload_bytes_per_frame: 512 * 1024,
                max_texture_uploads_per_frame: 2,
                max_texture_upload_bytes_per_frame: 1024 * 1024,
                max_material_table_updates_per_frame: 16,
                max_descriptor_updates_per_frame: 16,
                max_evictions_per_frame: 4,
            },
            FunDrawBudgetLane::FullRuntime => Self {
                max_geometry_page_uploads_per_frame: 6,
                max_geometry_upload_bytes_per_frame: 2 * 1024 * 1024,
                max_texture_uploads_per_frame: 4,
                max_texture_upload_bytes_per_frame: 4 * 1024 * 1024,
                max_material_table_updates_per_frame: 48,
                max_descriptor_updates_per_frame: 48,
                max_evictions_per_frame: 12,
            },
            FunDrawBudgetLane::StreamingSpike => Self {
                max_geometry_page_uploads_per_frame: 10,
                max_geometry_upload_bytes_per_frame: 6 * 1024 * 1024,
                max_texture_uploads_per_frame: 6,
                max_texture_upload_bytes_per_frame: 8 * 1024 * 1024,
                max_material_table_updates_per_frame: 96,
                max_descriptor_updates_per_frame: 96,
                max_evictions_per_frame: 24,
            },
            FunDrawBudgetLane::SolariCloudHeavy => Self {
                max_geometry_page_uploads_per_frame: 4,
                max_geometry_upload_bytes_per_frame: 1024 * 1024,
                max_texture_uploads_per_frame: 2,
                max_texture_upload_bytes_per_frame: 2 * 1024 * 1024,
                max_material_table_updates_per_frame: 32,
                max_descriptor_updates_per_frame: 32,
                max_evictions_per_frame: 8,
            },
            FunDrawBudgetLane::Competitive5v5 => Self {
                max_geometry_page_uploads_per_frame: 5,
                max_geometry_upload_bytes_per_frame: 2 * 1024 * 1024,
                max_texture_uploads_per_frame: 3,
                max_texture_upload_bytes_per_frame: 3 * 1024 * 1024,
                max_material_table_updates_per_frame: 40,
                max_descriptor_updates_per_frame: 40,
                max_evictions_per_frame: 10,
            },
            FunDrawBudgetLane::LargeBattle => Self {
                max_geometry_page_uploads_per_frame: 12,
                max_geometry_upload_bytes_per_frame: 8 * 1024 * 1024,
                max_texture_uploads_per_frame: 8,
                max_texture_upload_bytes_per_frame: 10 * 1024 * 1024,
                max_material_table_updates_per_frame: 128,
                max_descriptor_updates_per_frame: 128,
                max_evictions_per_frame: 32,
            },
        }
    }
}

impl Default for FunResidencyBudget {
    fn default() -> Self {
        Self::for_lane(FunDrawBudgetLane::FullRuntime)
    }
}

pub const fn residency_budget_for_lane(lane: FunDrawBudgetLane) -> FunResidencyBudget {
    FunResidencyBudget::for_lane(lane)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FunResidencyFrameContext {
    pub frame_index: u64,
    pub lane: FunDrawBudgetLane,
    pub frame_budget_exceeded: bool,
    pub memory_emergency: bool,
    pub p95_danger: bool,
    pub p95_frame_impact_ns: i64,
}

impl Default for FunResidencyFrameContext {
    fn default() -> Self {
        Self {
            frame_index: 0,
            lane: FunDrawBudgetLane::FullRuntime,
            frame_budget_exceeded: false,
            memory_emergency: false,
            p95_danger: false,
            p95_frame_impact_ns: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunResidencyPageState {
    Resident,
    Requested,
    Evictable,
    Pinned,
    Missing,
    Fallback,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FunResidencyImportanceTag {
    #[default]
    Decoration,
    World,
    LargeOccluder,
    GameplayCritical,
    Weapon,
    Vehicle,
    PlayerFocus,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FunResidencyContentKind {
    #[default]
    FarSmallProp,
    HiddenObject,
    Decoration,
    LowScreenErrorHighCost,
    SilhouetteGeometryNearCamera,
    LargeOccluder,
    GameplayCriticalObject,
    VisibleWeapon,
    Vehicle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FunResidencyPriorityInput {
    pub visible_this_frame: bool,
    pub likely_visible_next_frame: bool,
    pub screen_space_area: f32,
    pub distance_meters: f32,
    pub velocity_toward_camera: f32,
    pub importance_tag: FunResidencyImportanceTag,
    pub occluder_value: f32,
    pub gameplay_importance: f32,
    pub player_focus: f32,
    pub age_since_last_used_frames: u32,
    pub upload_cost_bytes: u64,
}

impl FunResidencyPriorityInput {
    pub fn priority_score(self) -> i32 {
        let visible = i32::from(self.visible_this_frame) * 4_000;
        let next = i32::from(self.likely_visible_next_frame) * 1_500;
        let area = score_f32(self.screen_space_area, 0.0, 1.0, 2_000);
        let near = score_inverse_distance(self.distance_meters);
        let velocity = score_f32(self.velocity_toward_camera, 0.0, 80.0, 800);
        let importance = importance_score(self.importance_tag);
        let occluder = score_f32(self.occluder_value, 0.0, 1.0, 1_200);
        let gameplay = score_f32(self.gameplay_importance, 0.0, 1.0, 1_500);
        let focus = score_f32(self.player_focus, 0.0, 1.0, 2_000);
        let recency_penalty = self.age_since_last_used_frames.min(600) as i32;
        let upload_penalty = (self.upload_cost_bytes / 16_384).min(2_000) as i32;
        visible + next + area + near + velocity + importance + occluder + gameplay + focus
            - recency_penalty
            - upload_penalty
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FunResidencyUploadUrgency {
    Defer,
    Normal,
    Urgent,
}

impl FunResidencyUploadUrgency {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Defer => "defer",
            Self::Normal => "normal",
            Self::Urgent => "urgent",
        }
    }
}

pub const fn residency_upload_urgency(
    content_kind: FunResidencyContentKind,
) -> FunResidencyUploadUrgency {
    match content_kind {
        FunResidencyContentKind::SilhouetteGeometryNearCamera
        | FunResidencyContentKind::LargeOccluder
        | FunResidencyContentKind::GameplayCriticalObject
        | FunResidencyContentKind::VisibleWeapon
        | FunResidencyContentKind::Vehicle => FunResidencyUploadUrgency::Urgent,
        FunResidencyContentKind::FarSmallProp
        | FunResidencyContentKind::HiddenObject
        | FunResidencyContentKind::Decoration
        | FunResidencyContentKind::LowScreenErrorHighCost => FunResidencyUploadUrgency::Defer,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunResidencyUploadDecision {
    Uploaded,
    DeferredBudget,
    DeferredP95Danger,
    MissingFallback,
    AlreadyResident,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunResidencyEvictionDecision {
    Evicted,
    Pinned,
    TooYoung,
    NotEvictable,
    BudgetExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeometryResidencyPage {
    pub id: GeometryPageId,
    pub state: FunResidencyPageState,
    pub size_bytes: u64,
    pub loaded_frame: u64,
    pub last_used_frame: u64,
    pub last_evicted_frame: Option<u64>,
    pub fallback_lod_available: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeometryPageUploadRequest {
    pub page_id: GeometryPageId,
    pub bytes: u64,
    pub priority: FunResidencyPriorityInput,
    pub content_kind: FunResidencyContentKind,
    pub requested_frame: u64,
    pub fallback_lod_available: bool,
}

impl GeometryPageUploadRequest {
    pub fn score(&self) -> i32 {
        self.priority.priority_score()
    }

    pub const fn urgency(&self) -> FunResidencyUploadUrgency {
        residency_upload_urgency(self.content_kind)
    }
}

#[derive(Debug, Clone, Resource)]
pub struct GeometryResidencyManager {
    pub resident_pages: BTreeMap<GeometryPageId, GeometryResidencyPage>,
    pub requested_pages: BTreeSet<GeometryPageId>,
    pub evictable_pages: BTreeSet<GeometryPageId>,
    pub pinned_pages: BTreeSet<GeometryPageId>,
    pub upload_queue: Vec<GeometryPageUploadRequest>,
    pub recently_evicted_pages: BTreeMap<GeometryPageId, GeometryResidencyPage>,
    pub minimum_residency_age_frames: u32,
    pub hysteresis_frames: u32,
}

impl Default for GeometryResidencyManager {
    fn default() -> Self {
        Self {
            resident_pages: BTreeMap::new(),
            requested_pages: BTreeSet::new(),
            evictable_pages: BTreeSet::new(),
            pinned_pages: BTreeSet::new(),
            upload_queue: Vec::new(),
            recently_evicted_pages: BTreeMap::new(),
            minimum_residency_age_frames: 120,
            hysteresis_frames: 180,
        }
    }
}

impl GeometryResidencyManager {
    pub fn request_page(&mut self, request: GeometryPageUploadRequest) {
        self.requested_pages.insert(request.page_id);
        self.upload_queue.push(request);
    }

    pub fn pin_page(&mut self, page_id: GeometryPageId) {
        self.pinned_pages.insert(page_id);
        if let Some(page) = self.resident_pages.get_mut(&page_id) {
            page.state = FunResidencyPageState::Pinned;
        }
    }

    pub fn mark_evictable(&mut self, page_id: GeometryPageId) {
        if !self.pinned_pages.contains(&page_id) {
            self.evictable_pages.insert(page_id);
            if let Some(page) = self.resident_pages.get_mut(&page_id) {
                page.state = FunResidencyPageState::Evictable;
            }
        }
    }

    pub fn plan_frame_uploads(
        &mut self,
        context: FunResidencyFrameContext,
        budget: FunResidencyBudget,
    ) -> FunResidencyFrameReport {
        let mut report = FunResidencyFrameReport::new(context);
        let mut queue = std::mem::take(&mut self.upload_queue);
        queue.sort_by(|a, b| {
            b.urgency()
                .cmp(&a.urgency())
                .then_with(|| b.score().cmp(&a.score()))
                .then_with(|| a.bytes.cmp(&b.bytes))
                .then_with(|| a.page_id.cmp(&b.page_id))
        });

        for request in queue {
            if let Some(page) = self.resident_pages.get(&request.page_id)
                && matches!(
                    page.state,
                    FunResidencyPageState::Resident
                        | FunResidencyPageState::Evictable
                        | FunResidencyPageState::Pinned
                )
            {
                self.requested_pages.remove(&request.page_id);
                continue;
            }

            if self.is_in_hysteresis(request.page_id, context.frame_index) {
                report.missing_pages = report.missing_pages.saturating_add(1);
                if request.fallback_lod_available {
                    report.fallback_lod_count = report.fallback_lod_count.saturating_add(1);
                }
                self.upload_queue.push(request);
                continue;
            }

            if (context.frame_budget_exceeded || context.p95_danger)
                && request.urgency() != FunResidencyUploadUrgency::Urgent
            {
                report.deferred_pages = report.deferred_pages.saturating_add(1);
                self.upload_queue.push(request);
                continue;
            }

            if report.geometry_page_uploads >= budget.max_geometry_page_uploads_per_frame
                || report.geometry_upload_bytes.saturating_add(request.bytes)
                    > budget.max_geometry_upload_bytes_per_frame
            {
                report.deferred_pages = report.deferred_pages.saturating_add(1);
                self.upload_queue.push(request);
                continue;
            }

            self.resident_pages.insert(
                request.page_id,
                GeometryResidencyPage {
                    id: request.page_id,
                    state: FunResidencyPageState::Resident,
                    size_bytes: request.bytes,
                    loaded_frame: context.frame_index,
                    last_used_frame: context.frame_index,
                    last_evicted_frame: None,
                    fallback_lod_available: request.fallback_lod_available,
                },
            );
            self.recently_evicted_pages.remove(&request.page_id);
            self.requested_pages.remove(&request.page_id);
            report.geometry_page_uploads = report.geometry_page_uploads.saturating_add(1);
            report.geometry_upload_bytes =
                report.geometry_upload_bytes.saturating_add(request.bytes);
            report.upload_bytes = report.upload_bytes.saturating_add(request.bytes);
        }

        report
    }

    pub fn evict_page(
        &mut self,
        page_id: GeometryPageId,
        context: FunResidencyFrameContext,
        eviction_count: u32,
        budget: FunResidencyBudget,
    ) -> FunResidencyEvictionDecision {
        if eviction_count >= budget.max_evictions_per_frame {
            return FunResidencyEvictionDecision::BudgetExhausted;
        }
        if self.pinned_pages.contains(&page_id) {
            return FunResidencyEvictionDecision::Pinned;
        }
        if !self.evictable_pages.contains(&page_id) {
            return FunResidencyEvictionDecision::NotEvictable;
        }
        let Some(page) = self.resident_pages.get(&page_id) else {
            return FunResidencyEvictionDecision::NotEvictable;
        };
        let age = context.frame_index.saturating_sub(page.loaded_frame);
        if age < u64::from(self.minimum_residency_age_frames) && !context.memory_emergency {
            return FunResidencyEvictionDecision::TooYoung;
        }

        let mut evicted = self
            .resident_pages
            .remove(&page_id)
            .expect("page exists after previous lookup");
        evicted.state = FunResidencyPageState::Missing;
        evicted.last_evicted_frame = Some(context.frame_index);
        self.evictable_pages.remove(&page_id);
        self.requested_pages.remove(&page_id);
        self.recently_evicted_pages.insert(page_id, evicted);
        FunResidencyEvictionDecision::Evicted
    }

    fn is_in_hysteresis(&self, page_id: GeometryPageId, frame_index: u64) -> bool {
        self.recently_evicted_pages
            .get(&page_id)
            .and_then(|page| page.last_evicted_frame)
            .is_some_and(|last_evicted| {
                frame_index.saturating_sub(last_evicted) < u64::from(self.hysteresis_frames)
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureResidencyPage {
    pub id: TexturePageId,
    pub state: FunResidencyPageState,
    pub upload_bytes: u64,
    pub mip_priority: u8,
    pub loaded_frame: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TexturePageUploadRequest {
    pub page_id: TexturePageId,
    pub upload_bytes: u64,
    pub mip_priority: u8,
    pub priority: FunResidencyPriorityInput,
    pub content_kind: FunResidencyContentKind,
}

#[derive(Debug, Clone, Default, Resource)]
pub struct TextureResidencyManager {
    pub texture_pages: BTreeMap<TexturePageId, TextureResidencyPage>,
    pub requested_pages: BTreeSet<TexturePageId>,
    pub mip_priority: BTreeMap<TexturePageId, u8>,
    pub upload_queue: Vec<TexturePageUploadRequest>,
}

impl TextureResidencyManager {
    pub fn request_texture(&mut self, request: TexturePageUploadRequest) {
        self.requested_pages.insert(request.page_id);
        self.mip_priority
            .insert(request.page_id, request.mip_priority);
        self.upload_queue.push(request);
    }

    pub fn plan_frame_uploads(
        &mut self,
        context: FunResidencyFrameContext,
        budget: FunResidencyBudget,
    ) -> FunResidencyFrameReport {
        let mut report = FunResidencyFrameReport::new(context);
        let mut queue = std::mem::take(&mut self.upload_queue);
        queue.sort_by(|a, b| {
            b.mip_priority
                .cmp(&a.mip_priority)
                .then_with(|| {
                    b.priority
                        .priority_score()
                        .cmp(&a.priority.priority_score())
                })
                .then_with(|| a.upload_bytes.cmp(&b.upload_bytes))
                .then_with(|| a.page_id.cmp(&b.page_id))
        });

        for request in queue {
            if self.texture_pages.contains_key(&request.page_id) {
                self.requested_pages.remove(&request.page_id);
                continue;
            }
            if (context.frame_budget_exceeded || context.p95_danger)
                && residency_upload_urgency(request.content_kind)
                    != FunResidencyUploadUrgency::Urgent
            {
                report.deferred_pages = report.deferred_pages.saturating_add(1);
                self.upload_queue.push(request);
                continue;
            }
            if report.texture_uploads >= budget.max_texture_uploads_per_frame
                || report
                    .texture_upload_bytes
                    .saturating_add(request.upload_bytes)
                    > budget.max_texture_upload_bytes_per_frame
            {
                report.deferred_pages = report.deferred_pages.saturating_add(1);
                self.upload_queue.push(request);
                continue;
            }

            self.texture_pages.insert(
                request.page_id,
                TextureResidencyPage {
                    id: request.page_id,
                    state: FunResidencyPageState::Resident,
                    upload_bytes: request.upload_bytes,
                    mip_priority: request.mip_priority,
                    loaded_frame: context.frame_index,
                },
            );
            self.requested_pages.remove(&request.page_id);
            report.texture_uploads = report.texture_uploads.saturating_add(1);
            report.texture_upload_bytes = report
                .texture_upload_bytes
                .saturating_add(request.upload_bytes);
            report.upload_bytes = report.upload_bytes.saturating_add(request.upload_bytes);
        }

        report
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaterialResidencyUpdate {
    pub material_id: MaterialEntryId,
    pub table_update_required: bool,
    pub descriptor_update_required: bool,
}

#[derive(Debug, Clone, Resource)]
pub struct MaterialResidencyManager {
    pub material_table_entries: BTreeSet<MaterialEntryId>,
    pub descriptor_residency: BTreeSet<MaterialEntryId>,
    pub fallback_material: MaterialEntryId,
    pub update_queue: Vec<MaterialResidencyUpdate>,
}

impl Default for MaterialResidencyManager {
    fn default() -> Self {
        Self {
            material_table_entries: BTreeSet::new(),
            descriptor_residency: BTreeSet::new(),
            fallback_material: MaterialEntryId(0),
            update_queue: Vec::new(),
        }
    }
}

impl MaterialResidencyManager {
    pub fn request_update(&mut self, update: MaterialResidencyUpdate) {
        self.update_queue.push(update);
    }

    pub fn plan_frame_updates(
        &mut self,
        context: FunResidencyFrameContext,
        budget: FunResidencyBudget,
    ) -> FunResidencyFrameReport {
        let mut report = FunResidencyFrameReport::new(context);
        let queue = std::mem::take(&mut self.update_queue);
        for update in queue {
            let table_allowed = !update.table_update_required
                || report.material_table_updates < budget.max_material_table_updates_per_frame;
            let descriptor_allowed = !update.descriptor_update_required
                || report.descriptor_updates < budget.max_descriptor_updates_per_frame;
            if !table_allowed || !descriptor_allowed || context.p95_danger {
                report.deferred_pages = report.deferred_pages.saturating_add(1);
                self.update_queue.push(update);
                continue;
            }
            if update.table_update_required {
                self.material_table_entries.insert(update.material_id);
                report.material_table_updates = report.material_table_updates.saturating_add(1);
            }
            if update.descriptor_update_required {
                self.descriptor_residency.insert(update.material_id);
                report.descriptor_updates = report.descriptor_updates.saturating_add(1);
            }
        }
        report
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunResidencyFrameReport {
    pub schema_version: u16,
    pub frame_index: u64,
    pub lane: FunDrawBudgetLane,
    pub upload_bytes: u64,
    pub geometry_page_uploads: u32,
    pub geometry_upload_bytes: u64,
    pub texture_uploads: u32,
    pub texture_upload_bytes: u64,
    pub material_table_updates: u32,
    pub descriptor_updates: u32,
    pub evictions: u32,
    pub deferred_pages: u32,
    pub missing_pages: u32,
    pub fallback_lod_count: u32,
    pub p95_frame_impact_ns: i64,
    pub p95_danger: bool,
}

impl FunResidencyFrameReport {
    pub const fn new(context: FunResidencyFrameContext) -> Self {
        Self {
            schema_version: FUN_RESIDENCY_SCHEMA_VERSION,
            frame_index: context.frame_index,
            lane: context.lane,
            upload_bytes: 0,
            geometry_page_uploads: 0,
            geometry_upload_bytes: 0,
            texture_uploads: 0,
            texture_upload_bytes: 0,
            material_table_updates: 0,
            descriptor_updates: 0,
            evictions: 0,
            deferred_pages: 0,
            missing_pages: 0,
            fallback_lod_count: 0,
            p95_frame_impact_ns: context.p95_frame_impact_ns,
            p95_danger: context.p95_danger,
        }
    }

    pub fn merge(&mut self, other: Self) {
        self.upload_bytes = self.upload_bytes.saturating_add(other.upload_bytes);
        self.geometry_page_uploads = self
            .geometry_page_uploads
            .saturating_add(other.geometry_page_uploads);
        self.geometry_upload_bytes = self
            .geometry_upload_bytes
            .saturating_add(other.geometry_upload_bytes);
        self.texture_uploads = self.texture_uploads.saturating_add(other.texture_uploads);
        self.texture_upload_bytes = self
            .texture_upload_bytes
            .saturating_add(other.texture_upload_bytes);
        self.material_table_updates = self
            .material_table_updates
            .saturating_add(other.material_table_updates);
        self.descriptor_updates = self
            .descriptor_updates
            .saturating_add(other.descriptor_updates);
        self.evictions = self.evictions.saturating_add(other.evictions);
        self.deferred_pages = self.deferred_pages.saturating_add(other.deferred_pages);
        self.missing_pages = self.missing_pages.saturating_add(other.missing_pages);
        self.fallback_lod_count = self
            .fallback_lod_count
            .saturating_add(other.fallback_lod_count);
        self.p95_danger |= other.p95_danger;
        self.p95_frame_impact_ns = self.p95_frame_impact_ns.max(other.p95_frame_impact_ns);
    }
}

pub fn plan_residency_frame(
    geometry: &mut GeometryResidencyManager,
    textures: &mut TextureResidencyManager,
    materials: &mut MaterialResidencyManager,
    context: FunResidencyFrameContext,
) -> FunResidencyFrameReport {
    let budget = FunResidencyBudget::for_lane(context.lane);
    let mut report = FunResidencyFrameReport::new(context);
    report.merge(geometry.plan_frame_uploads(context, budget));
    report.merge(textures.plan_frame_uploads(context, budget));
    report.merge(materials.plan_frame_updates(context, budget));
    report
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunResidencyPatchOutcome {
    pub steady_state_fps_improved: bool,
    pub streaming_p95_delta_ns: i64,
    pub explicit_follow_up_required: bool,
}

impl FunResidencyPatchOutcome {
    pub const fn from_benchmark_delta(
        steady_state_fps_delta_positive: bool,
        streaming_p95_delta_ns: i64,
    ) -> Self {
        Self {
            steady_state_fps_improved: steady_state_fps_delta_positive,
            streaming_p95_delta_ns,
            explicit_follow_up_required: steady_state_fps_delta_positive
                && streaming_p95_delta_ns > 0,
        }
    }
}

fn score_f32(value: f32, min: f32, max: f32, scale: i32) -> i32 {
    if !value.is_finite() || max <= min {
        return 0;
    }
    let normalized = ((value.clamp(min, max) - min) / (max - min)).clamp(0.0, 1.0);
    (normalized * scale as f32) as i32
}

fn score_inverse_distance(distance_meters: f32) -> i32 {
    if !distance_meters.is_finite() {
        return 0;
    }
    let distance = distance_meters.max(0.0);
    ((1.0 / (1.0 + distance / 25.0)) * 1_500.0) as i32
}

const fn importance_score(tag: FunResidencyImportanceTag) -> i32 {
    match tag {
        FunResidencyImportanceTag::Decoration => 0,
        FunResidencyImportanceTag::World => 400,
        FunResidencyImportanceTag::LargeOccluder => 1_600,
        FunResidencyImportanceTag::GameplayCritical => 2_000,
        FunResidencyImportanceTag::Weapon => 1_800,
        FunResidencyImportanceTag::Vehicle => 1_700,
        FunResidencyImportanceTag::PlayerFocus => 2_200,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(frame_index: u64) -> FunResidencyFrameContext {
        FunResidencyFrameContext {
            frame_index,
            lane: FunDrawBudgetLane::StreamingSpike,
            ..Default::default()
        }
    }

    fn priority_visible_near() -> FunResidencyPriorityInput {
        FunResidencyPriorityInput {
            visible_this_frame: true,
            likely_visible_next_frame: true,
            screen_space_area: 0.4,
            distance_meters: 5.0,
            velocity_toward_camera: 10.0,
            importance_tag: FunResidencyImportanceTag::GameplayCritical,
            occluder_value: 0.4,
            gameplay_importance: 1.0,
            player_focus: 0.7,
            age_since_last_used_frames: 0,
            upload_cost_bytes: 64 * 1024,
        }
    }

    fn priority_far_decoration() -> FunResidencyPriorityInput {
        FunResidencyPriorityInput {
            screen_space_area: 0.01,
            distance_meters: 400.0,
            importance_tag: FunResidencyImportanceTag::Decoration,
            age_since_last_used_frames: 400,
            upload_cost_bytes: 4 * 1024 * 1024,
            ..Default::default()
        }
    }

    fn geometry_request(
        page_id: u64,
        bytes: u64,
        priority: FunResidencyPriorityInput,
        content_kind: FunResidencyContentKind,
    ) -> GeometryPageUploadRequest {
        GeometryPageUploadRequest {
            page_id: GeometryPageId(page_id),
            bytes,
            priority,
            content_kind,
            requested_frame: 0,
            fallback_lod_available: true,
        }
    }

    #[test]
    fn priority_visible_near_gameplay_beats_far_decoration() {
        assert!(
            priority_visible_near().priority_score() > priority_far_decoration().priority_score()
        );
    }

    #[test]
    fn urgent_silhouette_geometry_uploads_before_deferred_decoration() {
        let mut manager = GeometryResidencyManager::default();
        manager.request_page(geometry_request(
            1,
            64 * 1024,
            priority_far_decoration(),
            FunResidencyContentKind::Decoration,
        ));
        manager.request_page(geometry_request(
            2,
            64 * 1024,
            priority_visible_near(),
            FunResidencyContentKind::SilhouetteGeometryNearCamera,
        ));
        let report = manager.plan_frame_uploads(
            context(1),
            FunResidencyBudget {
                max_geometry_page_uploads_per_frame: 1,
                max_geometry_upload_bytes_per_frame: 128 * 1024,
                ..FunResidencyBudget::for_lane(FunDrawBudgetLane::StreamingSpike)
            },
        );

        assert_eq!(report.geometry_page_uploads, 1);
        assert!(manager.resident_pages.contains_key(&GeometryPageId(2)));
        assert!(manager.requested_pages.contains(&GeometryPageId(1)));
    }

    #[test]
    fn geometry_upload_budget_caps_per_frame_and_defers() {
        let mut manager = GeometryResidencyManager::default();
        manager.request_page(geometry_request(
            1,
            96 * 1024,
            priority_visible_near(),
            FunResidencyContentKind::GameplayCriticalObject,
        ));
        manager.request_page(geometry_request(
            2,
            96 * 1024,
            priority_visible_near(),
            FunResidencyContentKind::GameplayCriticalObject,
        ));
        let report = manager.plan_frame_uploads(
            context(1),
            FunResidencyBudget {
                max_geometry_page_uploads_per_frame: 4,
                max_geometry_upload_bytes_per_frame: 128 * 1024,
                ..FunResidencyBudget::for_lane(FunDrawBudgetLane::StreamingSpike)
            },
        );

        assert_eq!(report.geometry_page_uploads, 1);
        assert_eq!(report.deferred_pages, 1);
        assert_eq!(manager.upload_queue.len(), 1);
    }

    #[test]
    fn p95_danger_defers_noncritical_uploads() {
        let mut manager = GeometryResidencyManager::default();
        manager.request_page(geometry_request(
            3,
            64 * 1024,
            priority_far_decoration(),
            FunResidencyContentKind::Decoration,
        ));
        manager.request_page(geometry_request(
            4,
            64 * 1024,
            priority_visible_near(),
            FunResidencyContentKind::VisibleWeapon,
        ));
        let report = manager.plan_frame_uploads(
            FunResidencyFrameContext {
                p95_danger: true,
                frame_budget_exceeded: true,
                ..context(4)
            },
            FunResidencyBudget::for_lane(FunDrawBudgetLane::StreamingSpike),
        );

        assert_eq!(report.geometry_page_uploads, 1);
        assert_eq!(report.deferred_pages, 1);
        assert!(manager.resident_pages.contains_key(&GeometryPageId(4)));
        assert!(manager.requested_pages.contains(&GeometryPageId(3)));
    }

    #[test]
    fn newly_loaded_pages_respect_minimum_residency_age() {
        let mut manager = GeometryResidencyManager::default();
        manager.resident_pages.insert(
            GeometryPageId(5),
            GeometryResidencyPage {
                id: GeometryPageId(5),
                state: FunResidencyPageState::Resident,
                size_bytes: 64 * 1024,
                loaded_frame: 10,
                last_used_frame: 10,
                last_evicted_frame: None,
                fallback_lod_available: true,
            },
        );
        manager.mark_evictable(GeometryPageId(5));

        assert_eq!(
            manager.evict_page(
                GeometryPageId(5),
                context(20),
                0,
                FunResidencyBudget::for_lane(FunDrawBudgetLane::StreamingSpike)
            ),
            FunResidencyEvictionDecision::TooYoung
        );
    }

    #[test]
    fn recently_evicted_page_uses_fallback_to_avoid_thrash() {
        let mut manager = GeometryResidencyManager::default();
        manager.recently_evicted_pages.insert(
            GeometryPageId(6),
            GeometryResidencyPage {
                id: GeometryPageId(6),
                state: FunResidencyPageState::Missing,
                size_bytes: 64 * 1024,
                loaded_frame: 0,
                last_used_frame: 0,
                last_evicted_frame: Some(100),
                fallback_lod_available: true,
            },
        );
        manager.request_page(geometry_request(
            6,
            64 * 1024,
            priority_visible_near(),
            FunResidencyContentKind::GameplayCriticalObject,
        ));
        let report = manager.plan_frame_uploads(
            context(120),
            FunResidencyBudget::for_lane(FunDrawBudgetLane::StreamingSpike),
        );

        assert_eq!(report.geometry_page_uploads, 0);
        assert_eq!(report.missing_pages, 1);
        assert_eq!(report.fallback_lod_count, 1);
    }

    #[test]
    fn texture_upload_budget_caps_bytes_and_count() {
        let mut manager = TextureResidencyManager::default();
        for id in 0..3 {
            manager.request_texture(TexturePageUploadRequest {
                page_id: TexturePageId(id),
                upload_bytes: 512 * 1024,
                mip_priority: 10 - id as u8,
                priority: priority_visible_near(),
                content_kind: FunResidencyContentKind::GameplayCriticalObject,
            });
        }
        let report = manager.plan_frame_uploads(
            context(8),
            FunResidencyBudget {
                max_texture_uploads_per_frame: 2,
                max_texture_upload_bytes_per_frame: 1024 * 1024,
                ..FunResidencyBudget::for_lane(FunDrawBudgetLane::StreamingSpike)
            },
        );

        assert_eq!(report.texture_uploads, 2);
        assert_eq!(report.texture_upload_bytes, 1024 * 1024);
        assert_eq!(report.deferred_pages, 1);
    }

    #[test]
    fn material_descriptor_budget_caps_updates() {
        let mut manager = MaterialResidencyManager::default();
        manager.request_update(MaterialResidencyUpdate {
            material_id: MaterialEntryId(1),
            table_update_required: true,
            descriptor_update_required: true,
        });
        manager.request_update(MaterialResidencyUpdate {
            material_id: MaterialEntryId(2),
            table_update_required: true,
            descriptor_update_required: true,
        });
        let report = manager.plan_frame_updates(
            context(9),
            FunResidencyBudget {
                max_material_table_updates_per_frame: 1,
                max_descriptor_updates_per_frame: 1,
                ..FunResidencyBudget::for_lane(FunDrawBudgetLane::StreamingSpike)
            },
        );

        assert_eq!(report.material_table_updates, 1);
        assert_eq!(report.descriptor_updates, 1);
        assert_eq!(report.deferred_pages, 1);
        assert_eq!(manager.update_queue.len(), 1);
    }

    #[test]
    fn streaming_spike_report_records_acceptance_fields() {
        let mut geometry = GeometryResidencyManager::default();
        let mut textures = TextureResidencyManager::default();
        let mut materials = MaterialResidencyManager::default();
        geometry.request_page(geometry_request(
            10,
            64 * 1024,
            priority_visible_near(),
            FunResidencyContentKind::GameplayCriticalObject,
        ));
        textures.request_texture(TexturePageUploadRequest {
            page_id: TexturePageId(10),
            upload_bytes: 128 * 1024,
            mip_priority: 10,
            priority: priority_visible_near(),
            content_kind: FunResidencyContentKind::GameplayCriticalObject,
        });
        materials.request_update(MaterialResidencyUpdate {
            material_id: MaterialEntryId(10),
            table_update_required: true,
            descriptor_update_required: true,
        });

        let report = plan_residency_frame(
            &mut geometry,
            &mut textures,
            &mut materials,
            FunResidencyFrameContext {
                frame_index: 10,
                lane: FunDrawBudgetLane::StreamingSpike,
                p95_frame_impact_ns: 250_000,
                ..Default::default()
            },
        );

        assert_eq!(report.schema_version, FUN_RESIDENCY_SCHEMA_VERSION);
        assert_eq!(report.lane, FunDrawBudgetLane::StreamingSpike);
        assert_eq!(report.upload_bytes, 192 * 1024);
        assert_eq!(report.deferred_pages, 0);
        assert_eq!(report.missing_pages, 0);
        assert_eq!(report.fallback_lod_count, 0);
        assert_eq!(report.p95_frame_impact_ns, 250_000);
    }

    #[test]
    fn p95_regression_requires_follow_up_even_when_fps_improves() {
        let outcome = FunResidencyPatchOutcome::from_benchmark_delta(true, 1_000_000);

        assert!(outcome.steady_state_fps_improved);
        assert_eq!(outcome.streaming_p95_delta_ns, 1_000_000);
        assert!(outcome.explicit_follow_up_required);
    }
}
