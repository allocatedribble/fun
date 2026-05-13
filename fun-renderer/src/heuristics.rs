use fun_ecs::{
    prelude::{Commands, Component, Entity, Mut, Query, Res, ResMut, Resource},
    schedule::SystemSet,
};
use fun_scene::{
    EditorSelection, GameplaySalient, GlobalTransform, LuxEmissive, LuxGiParticipant,
    LuxImportance, LuxLight, PagePriorityHint, Renderable, SceneChunkId, ShadowFilterPolicy,
    ShadowReceiverPriority, StreamingPriority, TemporalInstability, Transform, ViewVisibility,
    VirtualGeometryAuthoring, VirtualShadowReceiver, Visibility,
};

pub const RENDER_HEURISTIC_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum RenderHeuristicSet {
    BeginFrame,
    ScorePages,
    ScoreShadows,
    ScoreLux,
    PublishDebug,
}

impl RenderHeuristicSet {
    pub const ORDER: [Self; 5] = [
        Self::BeginFrame,
        Self::ScorePages,
        Self::ScoreShadows,
        Self::ScoreLux,
        Self::PublishDebug,
    ];

    #[must_use]
    pub const fn order_key(self) -> u16 {
        match self {
            Self::BeginFrame => 10,
            Self::ScorePages => 20,
            Self::ScoreShadows => 30,
            Self::ScoreLux => 40,
            Self::PublishDebug => 50,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BeginFrame => "begin_heuristic_frame",
            Self::ScorePages => "score_virtual_geometry_pages",
            Self::ScoreShadows => "score_shadow_receivers",
            Self::ScoreLux => "score_lux_lights_and_gi",
            Self::PublishDebug => "publish_heuristic_debug",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderBudgetMode {
    Quality,
    #[default]
    Balanced,
    Latency,
    Editor,
}

impl RenderBudgetMode {
    #[must_use]
    pub const fn priority_scale(self) -> u16 {
        match self {
            Self::Quality | Self::Editor => 256,
            Self::Balanced => 224,
            Self::Latency => 192,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderHeuristicWeights {
    pub screen_area: u16,
    pub visibility: u16,
    pub view_visibility: u16,
    pub renderable: u16,
    pub virtual_geometry: u16,
    pub gameplay_salience: u16,
    pub editor_selection: u16,
    pub streaming_priority: u16,
    pub temporal_instability: u16,
    pub receiver_risk: u16,
    pub light_importance: u16,
    pub emissive_importance: u16,
    pub gi_participation: u16,
    pub scene_chunk: u16,
}

impl Default for RenderHeuristicWeights {
    fn default() -> Self {
        Self {
            screen_area: 8,
            visibility: 8,
            view_visibility: 12,
            renderable: 6,
            virtual_geometry: 10,
            gameplay_salience: 10,
            editor_selection: 14,
            streaming_priority: 10,
            temporal_instability: 8,
            receiver_risk: 12,
            light_importance: 12,
            emissive_importance: 8,
            gi_participation: 8,
            scene_chunk: 6,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderFrameBudget {
    pub target_frame_us: u32,
    pub max_virtual_page_requests: u16,
    pub max_shadow_page_requests: u16,
    pub max_light_candidates: u16,
    pub max_gi_cache_updates: u16,
    pub max_ml_inference_jobs: u16,
}

impl RenderFrameBudget {
    #[must_use]
    pub const fn priority_scale(self) -> u16 {
        if self.target_frame_us <= 8_333 {
            192
        } else if self.target_frame_us <= 16_666 {
            224
        } else {
            256
        }
    }
}

impl Default for RenderFrameBudget {
    fn default() -> Self {
        Self {
            target_frame_us: 16_666,
            max_virtual_page_requests: 512,
            max_shadow_page_requests: 512,
            max_light_candidates: 8_192,
            max_gi_cache_updates: 256,
            max_ml_inference_jobs: 8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct RenderHeuristicScheduler {
    pub mode: RenderBudgetMode,
    pub weights: RenderHeuristicWeights,
    pub frame_budget: RenderFrameBudget,
}

impl Default for RenderHeuristicScheduler {
    fn default() -> Self {
        Self {
            mode: RenderBudgetMode::Balanced,
            weights: RenderHeuristicWeights::default(),
            frame_budget: RenderFrameBudget::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Resource)]
pub struct RendererViews {
    pub views: Vec<RendererView>,
}

impl RendererViews {
    #[must_use]
    pub fn primary(origin: [f32; 3], viewport_area_px: u32) -> Self {
        Self {
            views: vec![RendererView {
                origin,
                viewport_area_px,
                reference_screen_radius_px: 128.0,
            }],
        }
    }
}

impl Default for RendererViews {
    fn default() -> Self {
        Self::primary([0.0, 0.0, 0.0], 1920 * 1080)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RendererView {
    pub origin: [f32; 3],
    pub viewport_area_px: u32,
    pub reference_screen_radius_px: f32,
}

macro_rules! priority_component {
    ($name:ident) => {
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Component)]
        pub struct $name {
            pub value: u8,
        }

        impl $name {
            #[must_use]
            pub const fn new(value: u8) -> Self {
                Self { value }
            }
        }
    };
}

priority_component!(PagePriority);
priority_component!(ShadowPagePriority);
priority_component!(LuxLightPriority);
priority_component!(GiCacheUpdatePriority);
priority_component!(ShadingRatePriority);
priority_component!(MlInferencePriority);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeuristicTarget {
    Page,
    ShadowPage,
    LuxLight,
    GiCacheUpdate,
    ShadingRate,
    MlInference,
}

impl HeuristicTarget {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Page => "page",
            Self::ShadowPage => "shadow_page",
            Self::LuxLight => "lux_light",
            Self::GiCacheUpdate => "gi_cache_update",
            Self::ShadingRate => "shading_rate",
            Self::MlInference => "ml_inference",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HeuristicCauseSet {
    pub bits: u32,
}

impl HeuristicCauseSet {
    pub const TRANSFORM: Self = Self { bits: 1 << 0 };
    pub const GLOBAL_TRANSFORM: Self = Self { bits: 1 << 1 };
    pub const VISIBILITY: Self = Self { bits: 1 << 2 };
    pub const VIEW_VISIBILITY: Self = Self { bits: 1 << 3 };
    pub const RENDERABLE: Self = Self { bits: 1 << 4 };
    pub const VIRTUAL_GEOMETRY: Self = Self { bits: 1 << 5 };
    pub const LUX_LIGHT: Self = Self { bits: 1 << 6 };
    pub const LUX_EMISSIVE: Self = Self { bits: 1 << 7 };
    pub const SHADOW_RECEIVER: Self = Self { bits: 1 << 8 };
    pub const GAMEPLAY_SALIENCE: Self = Self { bits: 1 << 9 };
    pub const EDITOR_SELECTION: Self = Self { bits: 1 << 10 };
    pub const SCENE_CHUNK: Self = Self { bits: 1 << 11 };
    pub const STREAMING_PRIORITY: Self = Self { bits: 1 << 12 };
    pub const TEMPORAL_INSTABILITY: Self = Self { bits: 1 << 13 };
    pub const GI_PARTICIPATION: Self = Self { bits: 1 << 14 };

    #[must_use]
    pub const fn contains(self, cause: Self) -> bool {
        self.bits & cause.bits == cause.bits
    }

    pub fn insert(&mut self, cause: Self) {
        self.bits |= cause.bits;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeuristicCauseLabel {
    pub cause: HeuristicCauseSet,
    pub stable_id: &'static str,
}

pub const HEURISTIC_CAUSE_LABELS: [HeuristicCauseLabel; 15] = [
    HeuristicCauseLabel {
        cause: HeuristicCauseSet::TRANSFORM,
        stable_id: "transform",
    },
    HeuristicCauseLabel {
        cause: HeuristicCauseSet::GLOBAL_TRANSFORM,
        stable_id: "global_transform",
    },
    HeuristicCauseLabel {
        cause: HeuristicCauseSet::VISIBILITY,
        stable_id: "visibility",
    },
    HeuristicCauseLabel {
        cause: HeuristicCauseSet::VIEW_VISIBILITY,
        stable_id: "view_visibility",
    },
    HeuristicCauseLabel {
        cause: HeuristicCauseSet::RENDERABLE,
        stable_id: "renderable",
    },
    HeuristicCauseLabel {
        cause: HeuristicCauseSet::VIRTUAL_GEOMETRY,
        stable_id: "virtual_geometry_authoring",
    },
    HeuristicCauseLabel {
        cause: HeuristicCauseSet::LUX_LIGHT,
        stable_id: "lux_light",
    },
    HeuristicCauseLabel {
        cause: HeuristicCauseSet::LUX_EMISSIVE,
        stable_id: "lux_emissive",
    },
    HeuristicCauseLabel {
        cause: HeuristicCauseSet::SHADOW_RECEIVER,
        stable_id: "virtual_shadow_receiver",
    },
    HeuristicCauseLabel {
        cause: HeuristicCauseSet::GAMEPLAY_SALIENCE,
        stable_id: "gameplay_salient",
    },
    HeuristicCauseLabel {
        cause: HeuristicCauseSet::EDITOR_SELECTION,
        stable_id: "editor_selection",
    },
    HeuristicCauseLabel {
        cause: HeuristicCauseSet::SCENE_CHUNK,
        stable_id: "scene_chunk_id",
    },
    HeuristicCauseLabel {
        cause: HeuristicCauseSet::STREAMING_PRIORITY,
        stable_id: "streaming_priority",
    },
    HeuristicCauseLabel {
        cause: HeuristicCauseSet::TEMPORAL_INSTABILITY,
        stable_id: "temporal_instability",
    },
    HeuristicCauseLabel {
        cause: HeuristicCauseSet::GI_PARTICIPATION,
        stable_id: "gi_participation",
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeuristicPriorityRecord {
    pub entity: Entity,
    pub target: HeuristicTarget,
    pub priority: u8,
    pub causes: HeuristicCauseSet,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PriorityRecords {
    pub records: Vec<HeuristicPriorityRecord>,
}

impl PriorityRecords {
    pub fn clear(&mut self) {
        self.records.clear();
    }

    pub fn set(
        &mut self,
        entity: Entity,
        target: HeuristicTarget,
        priority: u8,
        causes: HeuristicCauseSet,
    ) {
        if let Some(record) = self
            .records
            .iter_mut()
            .find(|record| record.entity == entity && record.target == target)
        {
            record.priority = priority;
            record.causes = causes;
        } else {
            self.records.push(HeuristicPriorityRecord {
                entity,
                target,
                priority,
                causes,
            });
        }
        self.records.sort_by_key(|record| record.entity.to_bits());
    }

    #[must_use]
    pub fn get(&self, entity: Entity) -> Option<HeuristicPriorityRecord> {
        self.records
            .iter()
            .find(|record| record.entity == entity)
            .copied()
    }
}

macro_rules! priority_resource {
    ($name:ident, $target:expr) => {
        #[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
        pub struct $name {
            pub records: PriorityRecords,
        }

        impl $name {
            pub fn clear(&mut self) {
                self.records.clear();
            }

            pub fn set(&mut self, entity: Entity, priority: u8, causes: HeuristicCauseSet) {
                self.records.set(entity, $target, priority, causes);
            }

            #[must_use]
            pub fn get(&self, entity: Entity) -> Option<HeuristicPriorityRecord> {
                self.records.get(entity)
            }
        }
    };
}

priority_resource!(PagePriorities, HeuristicTarget::Page);
priority_resource!(ShadowPagePriorities, HeuristicTarget::ShadowPage);
priority_resource!(LuxLightPriorities, HeuristicTarget::LuxLight);
priority_resource!(GiCacheUpdatePriorities, HeuristicTarget::GiCacheUpdate);
priority_resource!(ShadingRatePriorities, HeuristicTarget::ShadingRate);
priority_resource!(MlInferencePriorities, HeuristicTarget::MlInference);

#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct HeuristicDebugOverlay {
    pub records: Vec<HeuristicPriorityRecord>,
}

impl HeuristicDebugOverlay {
    pub fn clear(&mut self) {
        self.records.clear();
    }

    pub fn record(
        &mut self,
        entity: Entity,
        target: HeuristicTarget,
        priority: u8,
        causes: HeuristicCauseSet,
    ) {
        if let Some(record) = self
            .records
            .iter_mut()
            .find(|record| record.entity == entity && record.target == target)
        {
            record.priority = priority;
            record.causes = causes;
        } else {
            self.records.push(HeuristicPriorityRecord {
                entity,
                target,
                priority,
                causes,
            });
        }
        self.records
            .sort_by_key(|record| (record.entity.to_bits(), record.target.as_str()));
    }

    #[must_use]
    pub fn record_for(
        &self,
        entity: Entity,
        target: HeuristicTarget,
    ) -> Option<HeuristicPriorityRecord> {
        self.records
            .iter()
            .find(|record| record.entity == entity && record.target == target)
            .copied()
    }
}

pub fn begin_heuristic_frame(
    mut page_priorities: ResMut<PagePriorities>,
    mut shadow_priorities: ResMut<ShadowPagePriorities>,
    mut light_priorities: ResMut<LuxLightPriorities>,
    mut gi_priorities: ResMut<GiCacheUpdatePriorities>,
    mut shading_priorities: ResMut<ShadingRatePriorities>,
    mut ml_priorities: ResMut<MlInferencePriorities>,
    mut debug_overlay: ResMut<HeuristicDebugOverlay>,
) {
    page_priorities.clear();
    shadow_priorities.clear();
    light_priorities.clear();
    gi_priorities.clear();
    shading_priorities.clear();
    ml_priorities.clear();
    debug_overlay.clear();
}

// The tuple is intentionally explicit: it is the ECS input contract for page,
// shading-rate, and virtual-geometry priority decisions.
#[allow(clippy::type_complexity)]
pub fn score_virtual_geometry_pages(
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &Transform,
        &GlobalTransform,
        &Renderable,
        &VirtualGeometryAuthoring,
        Option<&Visibility>,
        Option<&ViewVisibility>,
        Option<&GameplaySalient>,
        Option<&EditorSelection>,
        Option<&SceneChunkId>,
        Option<&StreamingPriority>,
        Option<&TemporalInstability>,
        Option<&mut PagePriority>,
        Option<&mut ShadingRatePriority>,
    )>,
    views: Res<RendererViews>,
    scheduler: Res<RenderHeuristicScheduler>,
    mut page_priorities: ResMut<PagePriorities>,
    mut shading_priorities: ResMut<ShadingRatePriorities>,
    mut debug_overlay: ResMut<HeuristicDebugOverlay>,
) {
    for (
        entity,
        transform,
        global_transform,
        renderable,
        virtual_geometry,
        visibility,
        view_visibility,
        gameplay_salient,
        editor_selection,
        scene_chunk,
        streaming_priority,
        temporal_instability,
        page_priority,
        shading_priority,
    ) in &mut query
    {
        let mut causes = HeuristicCauseSet::default();
        let score = score_page(
            transform,
            global_transform,
            renderable,
            virtual_geometry,
            visibility,
            view_visibility,
            gameplay_salient,
            editor_selection,
            scene_chunk,
            streaming_priority,
            temporal_instability,
            &views,
            &scheduler,
            &mut causes,
        );
        write_page_priority(&mut commands, entity, page_priority, score);
        write_shading_rate_priority(&mut commands, entity, shading_priority, score);
        page_priorities.set(entity, score, causes);
        shading_priorities.set(entity, score, causes);
        debug_overlay.record(entity, HeuristicTarget::Page, score, causes);
        debug_overlay.record(entity, HeuristicTarget::ShadingRate, score, causes);
    }
}

// The tuple is intentionally explicit: it is the ECS input contract for virtual
// shadow demand decisions and debug cause reporting.
#[allow(clippy::type_complexity)]
pub fn score_shadow_receivers(
    mut commands: Commands,
    mut query: Query<(
        Entity,
        Option<&Transform>,
        &GlobalTransform,
        &VirtualShadowReceiver,
        Option<&Visibility>,
        Option<&ViewVisibility>,
        Option<&GameplaySalient>,
        Option<&EditorSelection>,
        Option<&SceneChunkId>,
        Option<&StreamingPriority>,
        Option<&TemporalInstability>,
        Option<&mut ShadowPagePriority>,
    )>,
    views: Res<RendererViews>,
    scheduler: Res<RenderHeuristicScheduler>,
    mut priorities: ResMut<ShadowPagePriorities>,
    mut debug_overlay: ResMut<HeuristicDebugOverlay>,
) {
    for (
        entity,
        transform,
        global_transform,
        receiver,
        visibility,
        view_visibility,
        gameplay_salient,
        editor_selection,
        scene_chunk,
        streaming_priority,
        temporal_instability,
        shadow_priority,
    ) in &mut query
    {
        let mut causes = HeuristicCauseSet::default();
        let score = score_shadow_receiver(
            transform,
            global_transform,
            receiver,
            visibility,
            view_visibility,
            gameplay_salient,
            editor_selection,
            scene_chunk,
            streaming_priority,
            temporal_instability,
            &views,
            &scheduler,
            &mut causes,
        );
        write_shadow_page_priority(&mut commands, entity, shadow_priority, score);
        priorities.set(entity, score, causes);
        debug_overlay.record(entity, HeuristicTarget::ShadowPage, score, causes);
    }
}

// The tuple is intentionally explicit: it is the ECS input contract for light
// priority and renderer-side ML fallback decisions.
#[allow(clippy::type_complexity)]
pub fn score_lux_lights(
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &LuxLight,
        Option<&GlobalTransform>,
        Option<&Visibility>,
        Option<&ViewVisibility>,
        Option<&GameplaySalient>,
        Option<&EditorSelection>,
        Option<&StreamingPriority>,
        Option<&TemporalInstability>,
        Option<&mut LuxLightPriority>,
        Option<&mut MlInferencePriority>,
    )>,
    views: Res<RendererViews>,
    scheduler: Res<RenderHeuristicScheduler>,
    mut light_priorities: ResMut<LuxLightPriorities>,
    mut ml_priorities: ResMut<MlInferencePriorities>,
    mut debug_overlay: ResMut<HeuristicDebugOverlay>,
) {
    for (
        entity,
        light,
        global_transform,
        visibility,
        view_visibility,
        gameplay_salient,
        editor_selection,
        streaming_priority,
        temporal_instability,
        light_priority,
        ml_priority,
    ) in &mut query
    {
        let mut causes = HeuristicCauseSet::default();
        let score = score_lux_light(
            light,
            global_transform,
            visibility,
            view_visibility,
            gameplay_salient,
            editor_selection,
            streaming_priority,
            temporal_instability,
            &views,
            &scheduler,
            &mut causes,
        );
        let ml_score =
            score.saturating_add(optional_instability_score(temporal_instability, &mut causes) / 4);
        write_lux_light_priority(&mut commands, entity, light_priority, score);
        write_ml_inference_priority(&mut commands, entity, ml_priority, ml_score);
        light_priorities.set(entity, score, causes);
        ml_priorities.set(entity, ml_score, causes);
        debug_overlay.record(entity, HeuristicTarget::LuxLight, score, causes);
        debug_overlay.record(entity, HeuristicTarget::MlInference, ml_score, causes);
    }
}

// The tuple is intentionally explicit: it is the ECS input contract for GI
// cache refresh priority and emissive promotion decisions.
#[allow(clippy::type_complexity)]
pub fn score_gi_cache_participants(
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &LuxGiParticipant,
        Option<&LuxEmissive>,
        Option<&GlobalTransform>,
        Option<&Visibility>,
        Option<&ViewVisibility>,
        Option<&GameplaySalient>,
        Option<&EditorSelection>,
        Option<&StreamingPriority>,
        Option<&TemporalInstability>,
        Option<&mut GiCacheUpdatePriority>,
    )>,
    views: Res<RendererViews>,
    scheduler: Res<RenderHeuristicScheduler>,
    mut priorities: ResMut<GiCacheUpdatePriorities>,
    mut debug_overlay: ResMut<HeuristicDebugOverlay>,
) {
    for (
        entity,
        gi,
        emissive,
        global_transform,
        visibility,
        view_visibility,
        gameplay_salient,
        editor_selection,
        streaming_priority,
        temporal_instability,
        gi_priority,
    ) in &mut query
    {
        let mut causes = HeuristicCauseSet::default();
        let score = score_gi_participant(
            gi,
            emissive,
            global_transform,
            visibility,
            view_visibility,
            gameplay_salient,
            editor_selection,
            streaming_priority,
            temporal_instability,
            &views,
            &scheduler,
            &mut causes,
        );
        write_gi_cache_update_priority(&mut commands, entity, gi_priority, score);
        priorities.set(entity, score, causes);
        debug_overlay.record(entity, HeuristicTarget::GiCacheUpdate, score, causes);
    }
}

// Explicit arguments keep cause accounting tied to each ECS input instead of
// hiding the scheduler behind an opaque renderer-side blob.
#[allow(clippy::too_many_arguments)]
fn score_page(
    transform: &Transform,
    global_transform: &GlobalTransform,
    renderable: &Renderable,
    virtual_geometry: &VirtualGeometryAuthoring,
    visibility: Option<&Visibility>,
    view_visibility: Option<&ViewVisibility>,
    gameplay_salient: Option<&GameplaySalient>,
    editor_selection: Option<&EditorSelection>,
    scene_chunk: Option<&SceneChunkId>,
    streaming_priority: Option<&StreamingPriority>,
    temporal_instability: Option<&TemporalInstability>,
    views: &RendererViews,
    scheduler: &RenderHeuristicScheduler,
    causes: &mut HeuristicCauseSet,
) -> u8 {
    causes.insert(HeuristicCauseSet::TRANSFORM);
    causes.insert(HeuristicCauseSet::GLOBAL_TRANSFORM);
    if renderable.geometry.is_valid() || renderable.material.is_valid() {
        causes.insert(HeuristicCauseSet::RENDERABLE);
    }
    causes.insert(HeuristicCauseSet::VIRTUAL_GEOMETRY);
    if scene_chunk.is_some() {
        causes.insert(HeuristicCauseSet::SCENE_CHUNK);
    }
    let local_motion_bias = transform.translation.length().min(255.0) as u8;
    weighted_score(
        &[
            (
                screen_area_score(global_transform, views, causes)
                    .saturating_add(local_motion_bias / 16),
                scheduler.weights.screen_area,
            ),
            (
                visibility_score(visibility, view_visibility, causes),
                scheduler.weights.visibility + scheduler.weights.view_visibility,
            ),
            (
                page_hint_score(virtual_geometry.page_priority),
                scheduler.weights.virtual_geometry,
            ),
            (
                optional_salience_score(gameplay_salient, causes),
                scheduler.weights.gameplay_salience,
            ),
            (
                optional_editor_score(editor_selection, causes),
                scheduler.weights.editor_selection,
            ),
            (
                optional_streaming_score(streaming_priority, causes),
                scheduler.weights.streaming_priority,
            ),
            (
                optional_instability_score(temporal_instability, causes),
                scheduler.weights.temporal_instability,
            ),
            (
                if scene_chunk.is_some() { 192 } else { 96 },
                scheduler.weights.scene_chunk,
            ),
        ],
        scheduler,
    )
}

// Explicit arguments keep cause accounting tied to each ECS input instead of
// hiding the scheduler behind an opaque renderer-side blob.
#[allow(clippy::too_many_arguments)]
fn score_shadow_receiver(
    transform: Option<&Transform>,
    global_transform: &GlobalTransform,
    receiver: &VirtualShadowReceiver,
    visibility: Option<&Visibility>,
    view_visibility: Option<&ViewVisibility>,
    gameplay_salient: Option<&GameplaySalient>,
    editor_selection: Option<&EditorSelection>,
    scene_chunk: Option<&SceneChunkId>,
    streaming_priority: Option<&StreamingPriority>,
    temporal_instability: Option<&TemporalInstability>,
    views: &RendererViews,
    scheduler: &RenderHeuristicScheduler,
    causes: &mut HeuristicCauseSet,
) -> u8 {
    if transform.is_some() {
        causes.insert(HeuristicCauseSet::TRANSFORM);
    }
    causes.insert(HeuristicCauseSet::GLOBAL_TRANSFORM);
    causes.insert(HeuristicCauseSet::SHADOW_RECEIVER);
    if scene_chunk.is_some() {
        causes.insert(HeuristicCauseSet::SCENE_CHUNK);
    }
    weighted_score(
        &[
            (
                screen_area_score(global_transform, views, causes),
                scheduler.weights.screen_area,
            ),
            (
                visibility_score(visibility, view_visibility, causes),
                scheduler.weights.visibility + scheduler.weights.view_visibility,
            ),
            (
                receiver_risk_score(receiver),
                scheduler.weights.receiver_risk,
            ),
            (
                optional_salience_score(gameplay_salient, causes),
                scheduler.weights.gameplay_salience,
            ),
            (
                optional_editor_score(editor_selection, causes),
                scheduler.weights.editor_selection,
            ),
            (
                optional_streaming_score(streaming_priority, causes),
                scheduler.weights.streaming_priority,
            ),
            (
                optional_instability_score(temporal_instability, causes),
                scheduler.weights.temporal_instability,
            ),
        ],
        scheduler,
    )
}

// Explicit arguments keep cause accounting tied to each ECS input instead of
// hiding the scheduler behind an opaque renderer-side blob.
#[allow(clippy::too_many_arguments)]
fn score_lux_light(
    light: &LuxLight,
    global_transform: Option<&GlobalTransform>,
    visibility: Option<&Visibility>,
    view_visibility: Option<&ViewVisibility>,
    gameplay_salient: Option<&GameplaySalient>,
    editor_selection: Option<&EditorSelection>,
    streaming_priority: Option<&StreamingPriority>,
    temporal_instability: Option<&TemporalInstability>,
    views: &RendererViews,
    scheduler: &RenderHeuristicScheduler,
    causes: &mut HeuristicCauseSet,
) -> u8 {
    causes.insert(HeuristicCauseSet::LUX_LIGHT);
    if global_transform.is_some() {
        causes.insert(HeuristicCauseSet::GLOBAL_TRANSFORM);
    }
    weighted_score(
        &[
            (
                global_transform
                    .map(|global_transform| screen_area_score(global_transform, views, causes))
                    .unwrap_or(160),
                scheduler.weights.screen_area,
            ),
            (
                visibility_score(visibility, view_visibility, causes),
                scheduler.weights.visibility + scheduler.weights.view_visibility,
            ),
            (
                light_importance_score(light.importance),
                scheduler.weights.light_importance,
            ),
            (
                optional_salience_score(gameplay_salient, causes),
                scheduler.weights.gameplay_salience,
            ),
            (
                optional_editor_score(editor_selection, causes),
                scheduler.weights.editor_selection,
            ),
            (
                optional_streaming_score(streaming_priority, causes),
                scheduler.weights.streaming_priority,
            ),
            (
                optional_instability_score(temporal_instability, causes),
                scheduler.weights.temporal_instability,
            ),
        ],
        scheduler,
    )
}

// Explicit arguments keep cause accounting tied to each ECS input instead of
// hiding the scheduler behind an opaque renderer-side blob.
#[allow(clippy::too_many_arguments)]
fn score_gi_participant(
    _gi: &LuxGiParticipant,
    emissive: Option<&LuxEmissive>,
    global_transform: Option<&GlobalTransform>,
    visibility: Option<&Visibility>,
    view_visibility: Option<&ViewVisibility>,
    gameplay_salient: Option<&GameplaySalient>,
    editor_selection: Option<&EditorSelection>,
    streaming_priority: Option<&StreamingPriority>,
    temporal_instability: Option<&TemporalInstability>,
    views: &RendererViews,
    scheduler: &RenderHeuristicScheduler,
    causes: &mut HeuristicCauseSet,
) -> u8 {
    causes.insert(HeuristicCauseSet::GI_PARTICIPATION);
    if emissive.is_some() {
        causes.insert(HeuristicCauseSet::LUX_EMISSIVE);
    }
    weighted_score(
        &[
            (
                global_transform
                    .map(|global_transform| screen_area_score(global_transform, views, causes))
                    .unwrap_or(128),
                scheduler.weights.screen_area,
            ),
            (
                visibility_score(visibility, view_visibility, causes),
                scheduler.weights.visibility + scheduler.weights.view_visibility,
            ),
            (
                emissive.map(emissive_score).unwrap_or(96),
                scheduler.weights.emissive_importance,
            ),
            (192, scheduler.weights.gi_participation),
            (
                optional_salience_score(gameplay_salient, causes),
                scheduler.weights.gameplay_salience,
            ),
            (
                optional_editor_score(editor_selection, causes),
                scheduler.weights.editor_selection,
            ),
            (
                optional_streaming_score(streaming_priority, causes),
                scheduler.weights.streaming_priority,
            ),
            (
                optional_instability_score(temporal_instability, causes),
                scheduler.weights.temporal_instability,
            ),
        ],
        scheduler,
    )
}

fn weighted_score(inputs: &[(u8, u16)], scheduler: &RenderHeuristicScheduler) -> u8 {
    let mut weighted_sum = 0u32;
    let mut weight_sum = 0u32;
    for (score, weight) in inputs {
        weighted_sum = weighted_sum.saturating_add(u32::from(*score) * u32::from(*weight));
        weight_sum = weight_sum.saturating_add(u32::from(*weight));
    }
    if weight_sum == 0 {
        return 0;
    }
    let average = weighted_sum / weight_sum;
    let scale = scheduler
        .mode
        .priority_scale()
        .min(scheduler.frame_budget.priority_scale());
    ((average.saturating_mul(u32::from(scale))) / 256).min(u32::from(u8::MAX)) as u8
}

fn screen_area_score(
    global_transform: &GlobalTransform,
    views: &RendererViews,
    causes: &mut HeuristicCauseSet,
) -> u8 {
    causes.insert(HeuristicCauseSet::GLOBAL_TRANSFORM);
    if views.views.is_empty() {
        return 96;
    }
    let translation = global_transform.translation();
    let mut best = 0u8;
    for view in &views.views {
        let dx = translation.x - view.origin[0];
        let dy = translation.y - view.origin[1];
        let dz = translation.z - view.origin[2];
        let distance_sq = (dx * dx + dy * dy + dz * dz).max(1.0);
        let viewport_scale = (view.viewport_area_px as f32 / (1920.0 * 1080.0)).clamp(0.25, 4.0);
        let raw = (view.reference_screen_radius_px * view.reference_screen_radius_px)
            * viewport_scale
            / distance_sq;
        best = best.max(raw.clamp(0.0, 255.0) as u8);
    }
    best
}

fn visibility_score(
    visibility: Option<&Visibility>,
    view_visibility: Option<&ViewVisibility>,
    causes: &mut HeuristicCauseSet,
) -> u8 {
    if let Some(visibility) = visibility {
        causes.insert(HeuristicCauseSet::VISIBILITY);
        if *visibility == Visibility::Hidden {
            return 0;
        }
    }
    if let Some(view_visibility) = view_visibility {
        causes.insert(HeuristicCauseSet::VIEW_VISIBILITY);
        if !view_visibility.get() {
            return 24;
        }
        return u8::MAX;
    }
    160
}

fn page_hint_score(priority: PagePriorityHint) -> u8 {
    match priority {
        PagePriorityHint::Low => 32,
        PagePriorityHint::Normal => 128,
        PagePriorityHint::High => 192,
        PagePriorityHint::Critical | PagePriorityHint::WorldCritical => u8::MAX,
    }
}

fn receiver_risk_score(receiver: &VirtualShadowReceiver) -> u8 {
    let priority = match receiver.priority {
        ShadowReceiverPriority::Low => 32,
        ShadowReceiverPriority::Normal => 128,
        ShadowReceiverPriority::High => 192,
        ShadowReceiverPriority::Critical => u8::MAX,
    };
    let filter = match receiver.filter_policy {
        ShadowFilterPolicy::Basic => 96,
        ShadowFilterPolicy::ContactAware => 192,
        ShadowFilterPolicy::Denoised => 224,
    };
    priority.max(filter)
}

fn light_importance_score(importance: LuxImportance) -> u8 {
    match importance {
        LuxImportance::Low => 32,
        LuxImportance::Normal => 128,
        LuxImportance::High => 192,
        LuxImportance::Critical => u8::MAX,
    }
}

fn emissive_score(emissive: &LuxEmissive) -> u8 {
    (emissive.luminance / 64.0).clamp(0.0, 255.0) as u8
}

fn optional_salience_score(
    gameplay_salient: Option<&GameplaySalient>,
    causes: &mut HeuristicCauseSet,
) -> u8 {
    gameplay_salient
        .map(|salient| {
            causes.insert(HeuristicCauseSet::GAMEPLAY_SALIENCE);
            salient.score
        })
        .unwrap_or(96)
}

fn optional_editor_score(
    editor_selection: Option<&EditorSelection>,
    causes: &mut HeuristicCauseSet,
) -> u8 {
    editor_selection
        .map(|selection| {
            causes.insert(HeuristicCauseSet::EDITOR_SELECTION);
            selection
                .salience
                .saturating_add((u16::MAX - selection.rank).min(63) as u8)
        })
        .unwrap_or(0)
}

fn optional_streaming_score(
    streaming_priority: Option<&StreamingPriority>,
    causes: &mut HeuristicCauseSet,
) -> u8 {
    streaming_priority
        .map(|priority| {
            causes.insert(HeuristicCauseSet::STREAMING_PRIORITY);
            priority.score
        })
        .unwrap_or(96)
}

fn optional_instability_score(
    temporal_instability: Option<&TemporalInstability>,
    causes: &mut HeuristicCauseSet,
) -> u8 {
    temporal_instability
        .map(|instability| {
            causes.insert(HeuristicCauseSet::TEMPORAL_INSTABILITY);
            instability.score()
        })
        .unwrap_or(0)
}

fn write_page_priority(
    commands: &mut Commands,
    entity: Entity,
    priority: Option<Mut<PagePriority>>,
    value: u8,
) {
    if let Some(mut priority) = priority {
        priority.value = value;
    } else {
        commands.entity(entity).insert(PagePriority::new(value));
    }
}

fn write_shadow_page_priority(
    commands: &mut Commands,
    entity: Entity,
    priority: Option<Mut<ShadowPagePriority>>,
    value: u8,
) {
    if let Some(mut priority) = priority {
        priority.value = value;
    } else {
        commands
            .entity(entity)
            .insert(ShadowPagePriority::new(value));
    }
}

fn write_lux_light_priority(
    commands: &mut Commands,
    entity: Entity,
    priority: Option<Mut<LuxLightPriority>>,
    value: u8,
) {
    if let Some(mut priority) = priority {
        priority.value = value;
    } else {
        commands.entity(entity).insert(LuxLightPriority::new(value));
    }
}

fn write_gi_cache_update_priority(
    commands: &mut Commands,
    entity: Entity,
    priority: Option<Mut<GiCacheUpdatePriority>>,
    value: u8,
) {
    if let Some(mut priority) = priority {
        priority.value = value;
    } else {
        commands
            .entity(entity)
            .insert(GiCacheUpdatePriority::new(value));
    }
}

fn write_shading_rate_priority(
    commands: &mut Commands,
    entity: Entity,
    priority: Option<Mut<ShadingRatePriority>>,
    value: u8,
) {
    if let Some(mut priority) = priority {
        priority.value = value;
    } else {
        commands
            .entity(entity)
            .insert(ShadingRatePriority::new(value));
    }
}

fn write_ml_inference_priority(
    commands: &mut Commands,
    entity: Entity,
    priority: Option<Mut<MlInferencePriority>>,
    value: u8,
) {
    if let Some(mut priority) = priority {
        priority.value = value;
    } else {
        commands
            .entity(entity)
            .insert(MlInferencePriority::new(value));
    }
}

#[cfg(test)]
mod tests {
    use fun_ecs::{
        schedule::{IntoScheduleConfigs, Schedule},
        world::World,
    };
    use fun_scene::{
        DynamicGeometryPolicy, GeometryRef, GiBouncePolicy, GiCachePolicy, MaterialRef,
        PagePriorityHint, RenderableFlags, ShadowFilterPolicy, VirtualGeometryMode,
    };

    use super::*;

    fn insert_scheduler_resources(world: &mut World) {
        world.insert_resource(RendererViews::primary([0.0, 0.0, -4.0], 1920 * 1080));
        world.insert_resource(RenderHeuristicScheduler::default());
        world.insert_resource(PagePriorities::default());
        world.insert_resource(ShadowPagePriorities::default());
        world.insert_resource(LuxLightPriorities::default());
        world.insert_resource(GiCacheUpdatePriorities::default());
        world.insert_resource(ShadingRatePriorities::default());
        world.insert_resource(MlInferencePriorities::default());
        world.insert_resource(HeuristicDebugOverlay::default());
    }

    #[test]
    fn scheduler_sets_are_explicit_ecs_graph() {
        let mut previous = 0;
        for set in RenderHeuristicSet::ORDER {
            assert!(set.order_key() > previous, "{}", set.as_str());
            previous = set.order_key();
        }
    }

    #[test]
    fn shadow_receiver_priority_records_explain_ecs_causes() {
        let mut world = World::new();
        insert_scheduler_resources(&mut world);
        let entity = world
            .spawn((
                Transform::from_xyz(0.0, 0.0, 0.0),
                GlobalTransform::default(),
                Visibility::Visible,
                ViewVisibility::VISIBLE,
                VirtualShadowReceiver {
                    priority: ShadowReceiverPriority::High,
                    filter_policy: ShadowFilterPolicy::ContactAware,
                },
                GameplaySalient::CRITICAL,
                EditorSelection::PRIMARY,
                StreamingPriority::WORLD_CRITICAL,
                TemporalInstability::TRANSFORM_UNSTABLE,
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems((begin_heuristic_frame, score_shadow_receivers).chain());
        schedule.run(&mut world);

        let priority = world
            .resource::<ShadowPagePriorities>()
            .get(entity)
            .expect("shadow receiver should receive priority");
        assert!(priority.priority > 180);
        assert!(priority.causes.contains(HeuristicCauseSet::SHADOW_RECEIVER));
        assert!(
            priority
                .causes
                .contains(HeuristicCauseSet::GAMEPLAY_SALIENCE)
        );
        assert!(
            priority
                .causes
                .contains(HeuristicCauseSet::EDITOR_SELECTION)
        );

        let debug = world
            .resource::<HeuristicDebugOverlay>()
            .record_for(entity, HeuristicTarget::ShadowPage)
            .expect("debug overlay should carry priority cause record");
        assert_eq!(debug.causes, priority.causes);

        let component = world
            .get::<ShadowPagePriority>(entity)
            .expect("priority should also be published as a component");
        assert_eq!(component.value, priority.priority);
    }

    #[test]
    fn virtual_geometry_pages_score_from_renderable_streaming_and_motion_inputs() {
        let mut world = World::new();
        insert_scheduler_resources(&mut world);
        let entity = world
            .spawn((
                Transform::from_xyz(1.0, 0.0, 0.0),
                GlobalTransform::default(),
                Visibility::Visible,
                ViewVisibility::VISIBLE,
                Renderable::new(
                    GeometryRef::new(9),
                    MaterialRef::new(2),
                    RenderableFlags::STATIC_WORLD,
                ),
                VirtualGeometryAuthoring {
                    mode: VirtualGeometryMode::StaticClusterPages,
                    page_priority: PagePriorityHint::WorldCritical,
                    dynamic_policy: DynamicGeometryPolicy::StaticOnly,
                },
                GameplaySalient::CRITICAL,
                EditorSelection::PRIMARY,
                StreamingPriority::WORLD_CRITICAL,
                TemporalInstability::DESTRUCTIBLE,
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems((begin_heuristic_frame, score_virtual_geometry_pages).chain());
        schedule.run(&mut world);

        let page = world
            .resource::<PagePriorities>()
            .get(entity)
            .expect("virtual geometry entity should receive page priority");
        assert!(page.priority > 160);
        assert!(page.causes.contains(HeuristicCauseSet::RENDERABLE));
        assert!(page.causes.contains(HeuristicCauseSet::VIRTUAL_GEOMETRY));
        assert!(page.causes.contains(HeuristicCauseSet::GAMEPLAY_SALIENCE));
        assert!(page.causes.contains(HeuristicCauseSet::EDITOR_SELECTION));
        assert!(page.causes.contains(HeuristicCauseSet::STREAMING_PRIORITY));
        assert!(
            page.causes
                .contains(HeuristicCauseSet::TEMPORAL_INSTABILITY)
        );
        assert!(world.get::<PagePriority>(entity).is_some());
        assert!(world.get::<ShadingRatePriority>(entity).is_some());
    }

    #[test]
    fn lux_and_gi_priorities_are_resources_and_components() {
        let mut world = World::new();
        insert_scheduler_resources(&mut world);
        let light_entity = world
            .spawn((
                LuxLight::directional(90_000.0),
                GlobalTransform::default(),
                Visibility::Visible,
                ViewVisibility::VISIBLE,
                GameplaySalient::HIGH,
                EditorSelection::PRIMARY,
                StreamingPriority::WORLD_CRITICAL,
                TemporalInstability::DESTRUCTIBLE,
            ))
            .id();
        let gi_entity = world
            .spawn((
                LuxGiParticipant {
                    bounce_policy: GiBouncePolicy::StaticSingleBounce,
                    cache_policy: GiCachePolicy::Surface,
                },
                LuxEmissive {
                    luminance: 4096.0,
                    candidate_policy: fun_scene::EmissiveCandidatePolicy::AlwaysPromote,
                },
                GlobalTransform::default(),
                Visibility::Visible,
                ViewVisibility::VISIBLE,
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                begin_heuristic_frame,
                score_lux_lights,
                score_gi_cache_participants,
            )
                .chain(),
        );
        schedule.run(&mut world);

        let light = world
            .resource::<LuxLightPriorities>()
            .get(light_entity)
            .expect("light should be scored");
        assert!(light.priority > 170);
        assert!(light.causes.contains(HeuristicCauseSet::LUX_LIGHT));
        assert!(light.causes.contains(HeuristicCauseSet::EDITOR_SELECTION));
        assert!(light.causes.contains(HeuristicCauseSet::STREAMING_PRIORITY));
        assert!(world.get::<LuxLightPriority>(light_entity).is_some());
        assert!(world.get::<MlInferencePriority>(light_entity).is_some());

        let gi = world
            .resource::<GiCacheUpdatePriorities>()
            .get(gi_entity)
            .expect("GI participant should be scored");
        assert!(gi.causes.contains(HeuristicCauseSet::GI_PARTICIPATION));
        assert!(gi.causes.contains(HeuristicCauseSet::LUX_EMISSIVE));
        assert!(world.get::<GiCacheUpdatePriority>(gi_entity).is_some());
    }
}
