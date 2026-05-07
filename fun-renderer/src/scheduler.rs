use std::{fmt::Write as _, io, path::Path};

use crate::{page::PagePriorityInputs, virtual_shadow::ShadowPagePriorityInputs};

pub const SHARED_HEURISTIC_SCHEDULER_SCHEMA_VERSION: u16 = 1;
pub const PRIORITY_TERM_COUNT: usize = 10;
pub const SCHEDULER_WORK_SYSTEM_COUNT: usize = 8;
pub const SHARED_PRIORITY_HEATMAP_BUCKET_COUNT: usize = 6;
pub const PRIORITY_SCALE_PER_MILLE: u16 = 1000;
pub const SHARED_SCHEDULER_BENCHMARK_ARTIFACT_ENV: &str =
    "FUN_RENDERER_SHARED_SCHEDULER_BENCHMARK_ARTIFACT";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PriorityTerm {
    ProjectedArea,
    MotionMagnitude,
    TemporalHistoryError,
    LuminanceVariance,
    MaterialRisk,
    AlphaSpecularRisk,
    OcclusionConfidence,
    GameplaySalience,
    EditorFocus,
    CameraProximity,
}

impl PriorityTerm {
    pub const ALL: [Self; PRIORITY_TERM_COUNT] = [
        Self::ProjectedArea,
        Self::MotionMagnitude,
        Self::TemporalHistoryError,
        Self::LuminanceVariance,
        Self::MaterialRisk,
        Self::AlphaSpecularRisk,
        Self::OcclusionConfidence,
        Self::GameplaySalience,
        Self::EditorFocus,
        Self::CameraProximity,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProjectedArea => "projected_area",
            Self::MotionMagnitude => "motion_magnitude",
            Self::TemporalHistoryError => "temporal_history_error",
            Self::LuminanceVariance => "luminance_variance",
            Self::MaterialRisk => "material_risk",
            Self::AlphaSpecularRisk => "alpha_specular_risk",
            Self::OcclusionConfidence => "occlusion_confidence",
            Self::GameplaySalience => "gameplay_salience",
            Self::EditorFocus => "editor_focus",
            Self::CameraProximity => "camera_proximity",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharedPriorityInputs {
    pub projected_area: u16,
    pub motion_magnitude: u16,
    pub temporal_history_error: u16,
    pub luminance_variance: u16,
    pub material_risk: u16,
    pub alpha_specular_risk: u16,
    pub occlusion_confidence: u16,
    pub gameplay_salience: u16,
    pub editor_focus: u16,
    pub camera_proximity: u16,
}

impl SharedPriorityInputs {
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            projected_area: 0,
            motion_magnitude: 0,
            temporal_history_error: 0,
            luminance_variance: 0,
            material_risk: 0,
            alpha_specular_risk: 0,
            occlusion_confidence: 0,
            gameplay_salience: 0,
            editor_focus: 0,
            camera_proximity: 0,
        }
    }

    #[must_use]
    pub const fn stress_high() -> Self {
        Self {
            projected_area: 960,
            motion_magnitude: 880,
            temporal_history_error: 920,
            luminance_variance: 900,
            material_risk: 880,
            alpha_specular_risk: 860,
            occlusion_confidence: 940,
            gameplay_salience: 940,
            editor_focus: 640,
            camera_proximity: 920,
        }
    }

    #[must_use]
    pub const fn editor_locked() -> Self {
        Self {
            projected_area: 640,
            motion_magnitude: 128,
            temporal_history_error: 256,
            luminance_variance: 512,
            material_risk: 512,
            alpha_specular_risk: 512,
            occlusion_confidence: 1000,
            gameplay_salience: 512,
            editor_focus: 1000,
            camera_proximity: 720,
        }
    }

    #[must_use]
    pub const fn term(self, term: PriorityTerm) -> u16 {
        match term {
            PriorityTerm::ProjectedArea => self.projected_area,
            PriorityTerm::MotionMagnitude => self.motion_magnitude,
            PriorityTerm::TemporalHistoryError => self.temporal_history_error,
            PriorityTerm::LuminanceVariance => self.luminance_variance,
            PriorityTerm::MaterialRisk => self.material_risk,
            PriorityTerm::AlphaSpecularRisk => self.alpha_specular_risk,
            PriorityTerm::OcclusionConfidence => self.occlusion_confidence,
            PriorityTerm::GameplaySalience => self.gameplay_salience,
            PriorityTerm::EditorFocus => self.editor_focus,
            PriorityTerm::CameraProximity => self.camera_proximity,
        }
    }
}

impl Default for SharedPriorityInputs {
    fn default() -> Self {
        Self::zero()
    }
}

impl From<PagePriorityInputs> for SharedPriorityInputs {
    fn from(inputs: PagePriorityInputs) -> Self {
        Self {
            projected_area: inputs.projected_area,
            motion_magnitude: inputs.motion_magnitude,
            temporal_history_error: inputs.temporal_instability,
            luminance_variance: inputs.luminance_contrast_importance,
            material_risk: inputs.shadow_receiver_demand / 2,
            alpha_specular_risk: inputs.luminance_contrast_importance / 2,
            occlusion_confidence: inputs.visibility_confidence,
            gameplay_salience: inputs.gameplay_salience,
            editor_focus: inputs.editor_focus,
            camera_proximity: inputs.camera_proximity,
        }
        .clamped()
    }
}

impl From<ShadowPagePriorityInputs> for SharedPriorityInputs {
    fn from(inputs: ShadowPagePriorityInputs) -> Self {
        Self {
            projected_area: inputs.screen_coverage,
            motion_magnitude: 0,
            temporal_history_error: inputs.temporal_instability,
            luminance_variance: inputs.contrast,
            material_risk: inputs.visible_receiver_demand,
            alpha_specular_risk: inputs.contrast / 2,
            occlusion_confidence: 800,
            gameplay_salience: inputs.gameplay_salience,
            editor_focus: inputs.editor_focus,
            camera_proximity: inputs.visible_receiver_demand,
        }
        .clamped()
    }
}

impl SharedPriorityInputs {
    #[must_use]
    pub const fn clamped(self) -> Self {
        Self {
            projected_area: clamp_priority_input(self.projected_area),
            motion_magnitude: clamp_priority_input(self.motion_magnitude),
            temporal_history_error: clamp_priority_input(self.temporal_history_error),
            luminance_variance: clamp_priority_input(self.luminance_variance),
            material_risk: clamp_priority_input(self.material_risk),
            alpha_specular_risk: clamp_priority_input(self.alpha_specular_risk),
            occlusion_confidence: clamp_priority_input(self.occlusion_confidence),
            gameplay_salience: clamp_priority_input(self.gameplay_salience),
            editor_focus: clamp_priority_input(self.editor_focus),
            camera_proximity: clamp_priority_input(self.camera_proximity),
        }
    }
}

const fn clamp_priority_input(value: u16) -> u16 {
    if value > PRIORITY_SCALE_PER_MILLE {
        PRIORITY_SCALE_PER_MILLE
    } else {
        value
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PriorityTermWeights {
    pub projected_area: u16,
    pub motion_magnitude: u16,
    pub temporal_history_error: u16,
    pub luminance_variance: u16,
    pub material_risk: u16,
    pub alpha_specular_risk: u16,
    pub occlusion_confidence: u16,
    pub gameplay_salience: u16,
    pub editor_focus: u16,
    pub camera_proximity: u16,
}

impl PriorityTermWeights {
    pub const BALANCED: Self = Self {
        projected_area: 16,
        motion_magnitude: 9,
        temporal_history_error: 12,
        luminance_variance: 9,
        material_risk: 8,
        alpha_specular_risk: 8,
        occlusion_confidence: 12,
        gameplay_salience: 14,
        editor_focus: 8,
        camera_proximity: 14,
    };

    pub const QUALITY: Self = Self {
        projected_area: 18,
        motion_magnitude: 8,
        temporal_history_error: 13,
        luminance_variance: 12,
        material_risk: 12,
        alpha_specular_risk: 12,
        occlusion_confidence: 12,
        gameplay_salience: 12,
        editor_focus: 8,
        camera_proximity: 14,
    };

    pub const LATENCY: Self = Self {
        projected_area: 14,
        motion_magnitude: 14,
        temporal_history_error: 15,
        luminance_variance: 6,
        material_risk: 6,
        alpha_specular_risk: 6,
        occlusion_confidence: 10,
        gameplay_salience: 16,
        editor_focus: 8,
        camera_proximity: 12,
    };

    pub const EDITOR: Self = Self {
        projected_area: 12,
        motion_magnitude: 6,
        temporal_history_error: 8,
        luminance_variance: 8,
        material_risk: 8,
        alpha_specular_risk: 8,
        occlusion_confidence: 10,
        gameplay_salience: 10,
        editor_focus: 24,
        camera_proximity: 12,
    };

    #[must_use]
    pub const fn term(self, term: PriorityTerm) -> u16 {
        match term {
            PriorityTerm::ProjectedArea => self.projected_area,
            PriorityTerm::MotionMagnitude => self.motion_magnitude,
            PriorityTerm::TemporalHistoryError => self.temporal_history_error,
            PriorityTerm::LuminanceVariance => self.luminance_variance,
            PriorityTerm::MaterialRisk => self.material_risk,
            PriorityTerm::AlphaSpecularRisk => self.alpha_specular_risk,
            PriorityTerm::OcclusionConfidence => self.occlusion_confidence,
            PriorityTerm::GameplaySalience => self.gameplay_salience,
            PriorityTerm::EditorFocus => self.editor_focus,
            PriorityTerm::CameraProximity => self.camera_proximity,
        }
    }
}

impl Default for PriorityTermWeights {
    fn default() -> Self {
        Self::BALANCED
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SchedulerQualityPreset {
    Quality,
    #[default]
    Balanced,
    Latency,
    Editor,
}

impl SchedulerQualityPreset {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Quality => "quality",
            Self::Balanced => "balanced",
            Self::Latency => "latency",
            Self::Editor => "editor",
        }
    }

    #[must_use]
    pub const fn weights(self) -> PriorityTermWeights {
        match self {
            Self::Quality => PriorityTermWeights::QUALITY,
            Self::Balanced => PriorityTermWeights::BALANCED,
            Self::Latency => PriorityTermWeights::LATENCY,
            Self::Editor => PriorityTermWeights::EDITOR,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SchedulerWorkSystem {
    GeometryPages,
    ShadowPages,
    LightCandidates,
    GiCacheUpdates,
    ReflectionRays,
    TextureResidency,
    ShadingRate,
    MlInferenceDensity,
}

impl SchedulerWorkSystem {
    pub const ALL: [Self; SCHEDULER_WORK_SYSTEM_COUNT] = [
        Self::GeometryPages,
        Self::ShadowPages,
        Self::LightCandidates,
        Self::GiCacheUpdates,
        Self::ReflectionRays,
        Self::TextureResidency,
        Self::ShadingRate,
        Self::MlInferenceDensity,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GeometryPages => "geometry_pages",
            Self::ShadowPages => "shadow_pages",
            Self::LightCandidates => "light_candidates",
            Self::GiCacheUpdates => "gi_cache_updates",
            Self::ReflectionRays => "reflection_rays",
            Self::TextureResidency => "texture_residency",
            Self::ShadingRate => "shading_rate",
            Self::MlInferenceDensity => "ml_inference_density",
        }
    }

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::GeometryPages => 0,
            Self::ShadowPages => 1,
            Self::LightCandidates => 2,
            Self::GiCacheUpdates => 3,
            Self::ReflectionRays => 4,
            Self::TextureResidency => 5,
            Self::ShadingRate => 6,
            Self::MlInferenceDensity => 7,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulerIntegrationHook {
    pub system: SchedulerWorkSystem,
    pub stable_id: &'static str,
    pub input_contract: &'static str,
    pub output_contract: &'static str,
}

pub const SHARED_SCHEDULER_INTEGRATION_HOOKS: [SchedulerIntegrationHook;
    SCHEDULER_WORK_SYSTEM_COUNT] = [
    SchedulerIntegrationHook {
        system: SchedulerWorkSystem::GeometryPages,
        stable_id: "renderer.scheduler.hook.geometry_page_requests",
        input_contract: "virtual_geometry_page_request",
        output_contract: "geometry_page_budget_recommendation",
    },
    SchedulerIntegrationHook {
        system: SchedulerWorkSystem::ShadowPages,
        stable_id: "renderer.scheduler.hook.shadow_page_refreshes",
        input_contract: "shadow_page_refresh_request",
        output_contract: "shadow_page_budget_recommendation",
    },
    SchedulerIntegrationHook {
        system: SchedulerWorkSystem::LightCandidates,
        stable_id: "renderer.scheduler.hook.light_candidate_budgets",
        input_contract: "many_light_candidate_pressure",
        output_contract: "light_candidate_budget_recommendation",
    },
    SchedulerIntegrationHook {
        system: SchedulerWorkSystem::GiCacheUpdates,
        stable_id: "renderer.scheduler.hook.gi_cache_updates",
        input_contract: "gi_surface_cache_update_request",
        output_contract: "gi_cache_budget_recommendation",
    },
    SchedulerIntegrationHook {
        system: SchedulerWorkSystem::ReflectionRays,
        stable_id: "renderer.scheduler.hook.reflection_ray_budgets",
        input_contract: "reflection_ray_budget_request",
        output_contract: "reflection_ray_budget_recommendation",
    },
    SchedulerIntegrationHook {
        system: SchedulerWorkSystem::TextureResidency,
        stable_id: "renderer.scheduler.hook.texture_residency",
        input_contract: "texture_residency_page_request",
        output_contract: "texture_residency_budget_recommendation",
    },
    SchedulerIntegrationHook {
        system: SchedulerWorkSystem::ShadingRate,
        stable_id: "renderer.scheduler.hook.shading_rate_decisions",
        input_contract: "shading_rate_tile_request",
        output_contract: "shading_rate_budget_recommendation",
    },
    SchedulerIntegrationHook {
        system: SchedulerWorkSystem::MlInferenceDensity,
        stable_id: "renderer.scheduler.hook.ml_inference_density",
        input_contract: "renderer_model_inference_request",
        output_contract: "ml_inference_density_recommendation",
    },
];

#[must_use]
pub fn scheduler_integration_hook(
    system: SchedulerWorkSystem,
) -> &'static SchedulerIntegrationHook {
    &SHARED_SCHEDULER_INTEGRATION_HOOKS[system.index()]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PriorityTermContribution {
    pub term: PriorityTerm,
    pub raw_value: u16,
    pub weight: u16,
    pub weighted_value: u32,
}

impl PriorityTermContribution {
    pub const ZERO: Self = Self {
        term: PriorityTerm::ProjectedArea,
        raw_value: 0,
        weight: 0,
        weighted_value: 0,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NormalizedPriorityScore {
    pub normalized_per_mille: u16,
    pub weighted_sum: u32,
    pub weight_sum: u32,
    pub contributions: [PriorityTermContribution; PRIORITY_TERM_COUNT],
}

impl NormalizedPriorityScore {
    pub const ZERO: Self = Self {
        normalized_per_mille: 0,
        weighted_sum: 0,
        weight_sum: 0,
        contributions: [PriorityTermContribution::ZERO; PRIORITY_TERM_COUNT],
    };

    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.normalized_per_mille
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        ((self.normalized_per_mille as u32 * u8::MAX as u32) / PRIORITY_SCALE_PER_MILLE as u32)
            as u8
    }

    #[must_use]
    pub fn contribution(self, term: PriorityTerm) -> PriorityTermContribution {
        self.contributions[term_index(term)]
    }
}

const fn term_index(term: PriorityTerm) -> usize {
    match term {
        PriorityTerm::ProjectedArea => 0,
        PriorityTerm::MotionMagnitude => 1,
        PriorityTerm::TemporalHistoryError => 2,
        PriorityTerm::LuminanceVariance => 3,
        PriorityTerm::MaterialRisk => 4,
        PriorityTerm::AlphaSpecularRisk => 5,
        PriorityTerm::OcclusionConfidence => 6,
        PriorityTerm::GameplaySalience => 7,
        PriorityTerm::EditorFocus => 8,
        PriorityTerm::CameraProximity => 9,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeveloperPriorityOverrides {
    pub forced_priority_per_mille: Option<u16>,
    pub budget_scale_per_mille: u16,
    pub disabled_system_mask: u16,
}

impl DeveloperPriorityOverrides {
    pub const NONE: Self = Self {
        forced_priority_per_mille: None,
        budget_scale_per_mille: PRIORITY_SCALE_PER_MILLE,
        disabled_system_mask: 0,
    };

    #[must_use]
    pub const fn disables(self, system: SchedulerWorkSystem) -> bool {
        (self.disabled_system_mask & (1 << system.index())) != 0
    }
}

impl Default for DeveloperPriorityOverrides {
    fn default() -> Self {
        Self::NONE
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulerSceneProfile {
    pub stable_id: &'static str,
    pub geometry_boost_per_mille: u16,
    pub shadow_boost_per_mille: u16,
    pub light_boost_per_mille: u16,
    pub gi_boost_per_mille: u16,
}

impl SchedulerSceneProfile {
    pub const DEFAULT: Self = Self {
        stable_id: "scene_profile.default",
        geometry_boost_per_mille: PRIORITY_SCALE_PER_MILLE,
        shadow_boost_per_mille: PRIORITY_SCALE_PER_MILLE,
        light_boost_per_mille: PRIORITY_SCALE_PER_MILLE,
        gi_boost_per_mille: PRIORITY_SCALE_PER_MILLE,
    };
}

impl Default for SchedulerSceneProfile {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulerBenchmarkProfile {
    pub stable_id: &'static str,
    pub target_p95_frame_us: u32,
    pub target_p99_frame_us: u32,
}

impl SchedulerBenchmarkProfile {
    pub const DEFAULT: Self = Self {
        stable_id: "benchmark_profile.default",
        target_p95_frame_us: 16_666,
        target_p99_frame_us: 25_000,
    };

    pub const STRESS: Self = Self {
        stable_id: "benchmark_profile.pass19_scheduler_stress",
        target_p95_frame_us: 16_666,
        target_p99_frame_us: 25_000,
    };
}

impl Default for SchedulerBenchmarkProfile {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SchedulerDebugControls {
    pub freeze_priorities: bool,
    pub lock_budgets: bool,
    pub force_heatmap_capture: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemBudget {
    pub system: SchedulerWorkSystem,
    pub min_units: u32,
    pub max_units: u32,
    pub high_priority_threshold_per_mille: u16,
    pub estimated_unit_cost_us: u32,
}

impl SystemBudget {
    #[must_use]
    pub const fn new(
        system: SchedulerWorkSystem,
        min_units: u32,
        max_units: u32,
        high_priority_threshold_per_mille: u16,
        estimated_unit_cost_us: u32,
    ) -> Self {
        Self {
            system,
            min_units,
            max_units,
            high_priority_threshold_per_mille,
            estimated_unit_cost_us,
        }
    }
}

pub const DEFAULT_SYSTEM_BUDGETS: [SystemBudget; SCHEDULER_WORK_SYSTEM_COUNT] = [
    SystemBudget::new(SchedulerWorkSystem::GeometryPages, 8, 512, 760, 12),
    SystemBudget::new(SchedulerWorkSystem::ShadowPages, 4, 256, 760, 18),
    SystemBudget::new(SchedulerWorkSystem::LightCandidates, 128, 8_192, 700, 1),
    SystemBudget::new(SchedulerWorkSystem::GiCacheUpdates, 4, 256, 780, 24),
    SystemBudget::new(SchedulerWorkSystem::ReflectionRays, 256, 32_768, 720, 1),
    SystemBudget::new(SchedulerWorkSystem::TextureResidency, 8, 512, 730, 10),
    SystemBudget::new(SchedulerWorkSystem::ShadingRate, 64, 4_096, 650, 1),
    SystemBudget::new(SchedulerWorkSystem::MlInferenceDensity, 0, 16, 840, 120),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharedSchedulerConfig {
    pub quality_preset: SchedulerQualityPreset,
    pub weights: PriorityTermWeights,
    pub system_budgets: [SystemBudget; SCHEDULER_WORK_SYSTEM_COUNT],
    pub developer_overrides: DeveloperPriorityOverrides,
    pub scene_profile: SchedulerSceneProfile,
    pub benchmark_profile: SchedulerBenchmarkProfile,
    pub debug_controls: SchedulerDebugControls,
}

impl SharedSchedulerConfig {
    #[must_use]
    pub const fn for_preset(quality_preset: SchedulerQualityPreset) -> Self {
        Self {
            quality_preset,
            weights: quality_preset.weights(),
            system_budgets: DEFAULT_SYSTEM_BUDGETS,
            developer_overrides: DeveloperPriorityOverrides::NONE,
            scene_profile: SchedulerSceneProfile::DEFAULT,
            benchmark_profile: SchedulerBenchmarkProfile::DEFAULT,
            debug_controls: SchedulerDebugControls {
                freeze_priorities: false,
                lock_budgets: false,
                force_heatmap_capture: false,
            },
        }
    }

    #[must_use]
    pub const fn benchmark_stress() -> Self {
        Self {
            benchmark_profile: SchedulerBenchmarkProfile::STRESS,
            ..Self::for_preset(SchedulerQualityPreset::Balanced)
        }
    }

    #[must_use]
    pub const fn budget(self, system: SchedulerWorkSystem) -> SystemBudget {
        self.system_budgets[system.index()]
    }
}

impl Default for SharedSchedulerConfig {
    fn default() -> Self {
        Self::for_preset(SchedulerQualityPreset::Balanced)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetRecommendationReason {
    DisabledByOverride,
    BelowUsefulThreshold,
    Normal,
    HighPriority,
    LockedByDebugControl,
}

impl BudgetRecommendationReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DisabledByOverride => "disabled_by_override",
            Self::BelowUsefulThreshold => "below_useful_threshold",
            Self::Normal => "normal",
            Self::HighPriority => "high_priority",
            Self::LockedByDebugControl => "locked_by_debug_control",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemBudgetRecommendation {
    pub system: SchedulerWorkSystem,
    pub normalized_priority_per_mille: u16,
    pub requested_units: u32,
    pub recommended_units: u32,
    pub deferred_units: u32,
    pub estimated_cost_us: u32,
    pub reason: BudgetRecommendationReason,
}

impl SystemBudgetRecommendation {
    pub const ZERO: Self = Self {
        system: SchedulerWorkSystem::GeometryPages,
        normalized_priority_per_mille: 0,
        requested_units: 0,
        recommended_units: 0,
        deferred_units: 0,
        estimated_cost_us: 0,
        reason: BudgetRecommendationReason::BelowUsefulThreshold,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulerWorkRequest {
    pub system: SchedulerWorkSystem,
    pub request_id: u64,
    pub priority_inputs: SharedPriorityInputs,
    pub requested_units: u32,
    pub required: bool,
}

impl SchedulerWorkRequest {
    #[must_use]
    pub const fn new(
        system: SchedulerWorkSystem,
        request_id: u64,
        priority_inputs: SharedPriorityInputs,
        requested_units: u32,
    ) -> Self {
        Self {
            system,
            request_id,
            priority_inputs,
            requested_units,
            required: false,
        }
    }

    #[must_use]
    pub const fn required(mut self) -> Self {
        self.required = true;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulerWorkDecision {
    pub request_id: u64,
    pub system: SchedulerWorkSystem,
    pub priority: NormalizedPriorityScore,
    pub recommendation: SystemBudgetRecommendation,
    pub accepted_units: u32,
    pub deferred_units: u32,
    pub high_priority_miss: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedSchedulerFrameReport {
    pub frame_index: u64,
    pub decisions: Vec<SchedulerWorkDecision>,
    pub diagnostics: SharedSchedulerDiagnostics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharedHeuristicScheduler {
    pub config: SharedSchedulerConfig,
    frozen_score: Option<NormalizedPriorityScore>,
}

impl SharedHeuristicScheduler {
    #[must_use]
    pub const fn new(config: SharedSchedulerConfig) -> Self {
        Self {
            config,
            frozen_score: None,
        }
    }

    #[must_use]
    pub fn evaluate(&mut self, inputs: SharedPriorityInputs) -> NormalizedPriorityScore {
        if self.config.debug_controls.freeze_priorities
            && let Some(score) = self.frozen_score
        {
            return score;
        }
        let score = evaluate_priority(
            inputs,
            self.config.weights,
            self.config.developer_overrides.forced_priority_per_mille,
        );
        if self.config.debug_controls.freeze_priorities {
            self.frozen_score = Some(score);
        }
        score
    }

    #[must_use]
    pub fn recommend_budget(
        &self,
        system: SchedulerWorkSystem,
        priority: NormalizedPriorityScore,
        requested_units: u32,
    ) -> SystemBudgetRecommendation {
        recommend_budget(self.config, system, priority, requested_units)
    }

    #[must_use]
    pub fn schedule_frame(
        &mut self,
        frame_index: u64,
        requests: &[SchedulerWorkRequest],
        p95_frame_impact_us: u32,
        p99_frame_impact_us: u32,
    ) -> SharedSchedulerFrameReport {
        let mut scored = Vec::with_capacity(requests.len());
        for request in requests {
            let priority = self.evaluate(request.priority_inputs);
            let recommendation =
                self.recommend_budget(request.system, priority, request.requested_units);
            scored.push((*request, priority, recommendation));
        }
        scored.sort_by(|left, right| {
            right
                .1
                .normalized_per_mille
                .cmp(&left.1.normalized_per_mille)
                .then_with(|| left.0.system.index().cmp(&right.0.system.index()))
                .then_with(|| left.0.request_id.cmp(&right.0.request_id))
        });

        let mut remaining = [0_u32; SCHEDULER_WORK_SYSTEM_COUNT];
        for system in SchedulerWorkSystem::ALL {
            remaining[system.index()] = effective_system_max_units(self.config, system);
        }

        let mut diagnostics = SharedSchedulerDiagnostics::new(
            frame_index,
            p95_frame_impact_us,
            p99_frame_impact_us,
            self.config.benchmark_profile,
        );
        let mut decisions = Vec::with_capacity(scored.len());
        for (request, priority, mut recommendation) in scored {
            let index = request.system.index();
            let accepted = recommendation
                .recommended_units
                .min(remaining[index])
                .min(request.requested_units);
            remaining[index] = remaining[index].saturating_sub(accepted);
            let deferred = request.requested_units.saturating_sub(accepted);
            recommendation.recommended_units = accepted;
            recommendation.deferred_units = deferred;
            recommendation.estimated_cost_us =
                accepted.saturating_mul(self.config.budget(request.system).estimated_unit_cost_us);
            let high_priority_miss = priority.normalized_per_mille
                >= self
                    .config
                    .budget(request.system)
                    .high_priority_threshold_per_mille
                && deferred > 0;
            let decision = SchedulerWorkDecision {
                request_id: request.request_id,
                system: request.system,
                priority,
                recommendation,
                accepted_units: accepted,
                deferred_units: deferred,
                high_priority_miss,
            };
            diagnostics.record_decision(decision);
            decisions.push(decision);
        }
        diagnostics.finish_frame();
        SharedSchedulerFrameReport {
            frame_index,
            decisions,
            diagnostics,
        }
    }
}

impl Default for SharedHeuristicScheduler {
    fn default() -> Self {
        Self::new(SharedSchedulerConfig::default())
    }
}

#[must_use]
pub fn evaluate_priority(
    inputs: SharedPriorityInputs,
    weights: PriorityTermWeights,
    forced_priority_per_mille: Option<u16>,
) -> NormalizedPriorityScore {
    let inputs = inputs.clamped();
    let mut contributions = [PriorityTermContribution::ZERO; PRIORITY_TERM_COUNT];
    let mut weighted_sum = 0_u32;
    let mut weight_sum = 0_u32;
    for term in PriorityTerm::ALL {
        let raw = inputs.term(term);
        let weight = weights.term(term);
        let weighted = u32::from(raw).saturating_mul(u32::from(weight));
        contributions[term_index(term)] = PriorityTermContribution {
            term,
            raw_value: raw,
            weight,
            weighted_value: weighted,
        };
        weighted_sum = weighted_sum.saturating_add(weighted);
        weight_sum = weight_sum.saturating_add(u32::from(weight));
    }
    let normalized = if let Some(forced) = forced_priority_per_mille {
        clamp_priority_input(forced)
    } else {
        weighted_sum
            .checked_div(weight_sum)
            .unwrap_or(0)
            .min(u32::from(PRIORITY_SCALE_PER_MILLE)) as u16
    };
    NormalizedPriorityScore {
        normalized_per_mille: normalized,
        weighted_sum,
        weight_sum,
        contributions,
    }
}

#[must_use]
pub fn recommend_budget(
    config: SharedSchedulerConfig,
    system: SchedulerWorkSystem,
    priority: NormalizedPriorityScore,
    requested_units: u32,
) -> SystemBudgetRecommendation {
    let budget = config.budget(system);
    if requested_units == 0 || priority.normalized_per_mille == 0 {
        return SystemBudgetRecommendation {
            system,
            normalized_priority_per_mille: priority.normalized_per_mille,
            requested_units,
            ..SystemBudgetRecommendation::ZERO
        };
    }
    if config.developer_overrides.disables(system) {
        return SystemBudgetRecommendation {
            system,
            normalized_priority_per_mille: priority.normalized_per_mille,
            requested_units,
            deferred_units: requested_units,
            reason: BudgetRecommendationReason::DisabledByOverride,
            ..SystemBudgetRecommendation::ZERO
        };
    }
    let max_units = effective_system_max_units(config, system);
    let reason = if config.debug_controls.lock_budgets {
        BudgetRecommendationReason::LockedByDebugControl
    } else if priority.normalized_per_mille >= budget.high_priority_threshold_per_mille {
        BudgetRecommendationReason::HighPriority
    } else if priority.normalized_per_mille < 128 {
        BudgetRecommendationReason::BelowUsefulThreshold
    } else {
        BudgetRecommendationReason::Normal
    };
    let recommended = if matches!(reason, BudgetRecommendationReason::BelowUsefulThreshold) {
        budget.min_units.min(requested_units)
    } else {
        let scaled = (u64::from(max_units) * u64::from(priority.normalized_per_mille))
            / u64::from(PRIORITY_SCALE_PER_MILLE);
        (scaled as u32)
            .max(budget.min_units)
            .min(max_units)
            .min(requested_units)
    };
    SystemBudgetRecommendation {
        system,
        normalized_priority_per_mille: priority.normalized_per_mille,
        requested_units,
        recommended_units: recommended,
        deferred_units: requested_units.saturating_sub(recommended),
        estimated_cost_us: recommended.saturating_mul(budget.estimated_unit_cost_us),
        reason,
    }
}

const fn effective_system_max_units(
    config: SharedSchedulerConfig,
    system: SchedulerWorkSystem,
) -> u32 {
    let budget = config.budget(system);
    if config.debug_controls.lock_budgets {
        return budget.max_units;
    }
    let scaled = (budget.max_units as u64)
        .saturating_mul(config.developer_overrides.budget_scale_per_mille as u64)
        / PRIORITY_SCALE_PER_MILLE as u64;
    if scaled > u32::MAX as u64 {
        u32::MAX
    } else {
        scaled as u32
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SharedPriorityHeatmap {
    pub buckets: [u32; SHARED_PRIORITY_HEATMAP_BUCKET_COUNT],
}

impl SharedPriorityHeatmap {
    pub fn record(&mut self, priority_per_mille: u16) {
        let index = if priority_per_mille == 0 {
            0
        } else if priority_per_mille <= 200 {
            1
        } else if priority_per_mille <= 400 {
            2
        } else if priority_per_mille <= 600 {
            3
        } else if priority_per_mille <= 800 {
            4
        } else {
            5
        };
        self.buckets[index] = self.buckets[index].saturating_add(1);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemBudgetAllocation {
    pub system: SchedulerWorkSystem,
    pub requested_units: u32,
    pub accepted_units: u32,
    pub deferred_units: u32,
    pub dropped_work_count: u32,
    pub high_priority_misses: u32,
    pub estimated_cost_us: u32,
}

impl SystemBudgetAllocation {
    pub const ZERO: Self = Self {
        system: SchedulerWorkSystem::GeometryPages,
        requested_units: 0,
        accepted_units: 0,
        deferred_units: 0,
        dropped_work_count: 0,
        high_priority_misses: 0,
        estimated_cost_us: 0,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharedSchedulerDiagnostics {
    pub schema_version: u16,
    pub frame_index: u64,
    pub priority_heatmap: SharedPriorityHeatmap,
    pub budget_allocations: [SystemBudgetAllocation; SCHEDULER_WORK_SYSTEM_COUNT],
    pub dropped_work_count: u32,
    pub deferred_units: u32,
    pub high_priority_misses: u32,
    pub stability_score_per_mille: u16,
    pub p95_frame_impact_us: u32,
    pub p99_frame_impact_us: u32,
    pub benchmark_profile: SchedulerBenchmarkProfile,
}

impl SharedSchedulerDiagnostics {
    #[must_use]
    pub fn new(
        frame_index: u64,
        p95_frame_impact_us: u32,
        p99_frame_impact_us: u32,
        benchmark_profile: SchedulerBenchmarkProfile,
    ) -> Self {
        Self {
            schema_version: SHARED_HEURISTIC_SCHEDULER_SCHEMA_VERSION,
            frame_index,
            priority_heatmap: SharedPriorityHeatmap::default(),
            budget_allocations: empty_budget_allocations(),
            dropped_work_count: 0,
            deferred_units: 0,
            high_priority_misses: 0,
            stability_score_per_mille: PRIORITY_SCALE_PER_MILLE,
            p95_frame_impact_us,
            p99_frame_impact_us,
            benchmark_profile,
        }
    }

    pub fn record_decision(&mut self, decision: SchedulerWorkDecision) {
        self.priority_heatmap
            .record(decision.priority.normalized_per_mille);
        let allocation = &mut self.budget_allocations[decision.system.index()];
        allocation.requested_units = allocation
            .requested_units
            .saturating_add(decision.recommendation.requested_units);
        allocation.accepted_units = allocation
            .accepted_units
            .saturating_add(decision.accepted_units);
        allocation.deferred_units = allocation
            .deferred_units
            .saturating_add(decision.deferred_units);
        allocation.estimated_cost_us = allocation
            .estimated_cost_us
            .saturating_add(decision.recommendation.estimated_cost_us);
        if decision.deferred_units > 0 {
            allocation.dropped_work_count = allocation.dropped_work_count.saturating_add(1);
            self.dropped_work_count = self.dropped_work_count.saturating_add(1);
            self.deferred_units = self.deferred_units.saturating_add(decision.deferred_units);
        }
        if decision.high_priority_miss {
            allocation.high_priority_misses = allocation.high_priority_misses.saturating_add(1);
            self.high_priority_misses = self.high_priority_misses.saturating_add(1);
        }
    }

    pub fn finish_frame(&mut self) {
        let frame_penalty = frame_time_penalty(
            self.p95_frame_impact_us,
            self.p99_frame_impact_us,
            self.benchmark_profile,
        );
        let miss_penalty = self.high_priority_misses.saturating_mul(80);
        let defer_penalty = (self.deferred_units / 64).min(400);
        let penalty = frame_penalty
            .saturating_add(miss_penalty)
            .saturating_add(defer_penalty);
        self.stability_score_per_mille = PRIORITY_SCALE_PER_MILLE
            .saturating_sub(penalty.min(u32::from(PRIORITY_SCALE_PER_MILLE)) as u16);
    }
}

fn frame_time_penalty(p95_us: u32, p99_us: u32, profile: SchedulerBenchmarkProfile) -> u32 {
    let p95_over = p95_us.saturating_sub(profile.target_p95_frame_us);
    let p99_over = p99_us.saturating_sub(profile.target_p99_frame_us);
    (p95_over / 250).saturating_add(p99_over / 200)
}

fn empty_budget_allocations() -> [SystemBudgetAllocation; SCHEDULER_WORK_SYSTEM_COUNT] {
    let mut allocations = [SystemBudgetAllocation::ZERO; SCHEDULER_WORK_SYSTEM_COUNT];
    for system in SchedulerWorkSystem::ALL {
        allocations[system.index()].system = system;
    }
    allocations
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedSchedulerDebugRow {
    pub system: SchedulerWorkSystem,
    pub request_id: u64,
    pub priority_per_mille: u16,
    pub accepted_units: u32,
    pub deferred_units: u32,
    pub high_priority_miss: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SharedSchedulerDebugOverlay {
    pub rows: Vec<SharedSchedulerDebugRow>,
}

impl SharedSchedulerDebugOverlay {
    #[must_use]
    pub fn from_report(report: &SharedSchedulerFrameReport, max_rows: usize) -> Self {
        let mut rows: Vec<SharedSchedulerDebugRow> = report
            .decisions
            .iter()
            .map(|decision| SharedSchedulerDebugRow {
                system: decision.system,
                request_id: decision.request_id,
                priority_per_mille: decision.priority.normalized_per_mille,
                accepted_units: decision.accepted_units,
                deferred_units: decision.deferred_units,
                high_priority_miss: decision.high_priority_miss,
            })
            .collect();
        rows.sort_by(|left, right| {
            right
                .priority_per_mille
                .cmp(&left.priority_per_mille)
                .then_with(|| left.system.index().cmp(&right.system.index()))
                .then_with(|| left.request_id.cmp(&right.request_id))
        });
        rows.truncate(max_rows);
        Self { rows }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedSchedulerBenchmarkArtifact {
    pub schema_version: u16,
    pub content: String,
}

impl SharedSchedulerBenchmarkArtifact {
    #[must_use]
    pub fn from_report(report: &SharedSchedulerFrameReport) -> Self {
        let mut content = String::with_capacity(2048);
        let diagnostics = report.diagnostics;
        let _ = writeln!(
            content,
            "shared_scheduler_schema_version={}",
            SHARED_HEURISTIC_SCHEDULER_SCHEMA_VERSION
        );
        let _ = writeln!(content, "frame_index={}", report.frame_index);
        let _ = writeln!(
            content,
            "benchmark_profile={}",
            diagnostics.benchmark_profile.stable_id
        );
        let _ = writeln!(
            content,
            "stability_score_per_mille={}",
            diagnostics.stability_score_per_mille
        );
        let _ = writeln!(
            content,
            "p95_frame_impact_us={}",
            diagnostics.p95_frame_impact_us
        );
        let _ = writeln!(
            content,
            "p99_frame_impact_us={}",
            diagnostics.p99_frame_impact_us
        );
        let _ = writeln!(
            content,
            "dropped_work_count={}",
            diagnostics.dropped_work_count
        );
        let _ = writeln!(content, "deferred_units={}", diagnostics.deferred_units);
        let _ = writeln!(
            content,
            "high_priority_misses={}",
            diagnostics.high_priority_misses
        );
        let _ = writeln!(
            content,
            "priority_heatmap={:?}",
            diagnostics.priority_heatmap.buckets
        );
        for allocation in diagnostics.budget_allocations {
            let _ = writeln!(
                content,
                "budget system={} requested={} accepted={} deferred={} dropped={} high_priority_misses={} estimated_cost_us={}",
                allocation.system.as_str(),
                allocation.requested_units,
                allocation.accepted_units,
                allocation.deferred_units,
                allocation.dropped_work_count,
                allocation.high_priority_misses,
                allocation.estimated_cost_us
            );
        }
        let overlay = SharedSchedulerDebugOverlay::from_report(report, 16);
        for row in overlay.rows {
            let _ = writeln!(
                content,
                "overlay system={} request_id={} priority_per_mille={} accepted={} deferred={} high_priority_miss={}",
                row.system.as_str(),
                row.request_id,
                row.priority_per_mille,
                row.accepted_units,
                row.deferred_units,
                row.high_priority_miss
            );
        }
        Self {
            schema_version: SHARED_HEURISTIC_SCHEDULER_SCHEMA_VERSION,
            content,
        }
    }
}

pub fn write_shared_scheduler_benchmark_artifact(
    path: impl AsRef<Path>,
    artifact: &SharedSchedulerBenchmarkArtifact,
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

    #[test]
    fn normalized_priority_score_is_explainable_and_uses_required_terms() {
        let score = evaluate_priority(
            SharedPriorityInputs::stress_high(),
            PriorityTermWeights::BALANCED,
            None,
        );

        assert!(score.normalized_per_mille > 700);
        assert!(score.normalized_per_mille <= PRIORITY_SCALE_PER_MILLE);
        for term in PriorityTerm::ALL {
            let contribution = score.contribution(term);
            assert_eq!(contribution.term, term);
            assert!(contribution.weight > 0);
            assert!(contribution.weighted_value >= u32::from(contribution.raw_value));
        }
        assert_eq!(PriorityTerm::ProjectedArea.as_str(), "projected_area");
        assert_eq!(
            PriorityTerm::TemporalHistoryError.as_str(),
            "temporal_history_error"
        );
    }

    #[test]
    fn integration_hooks_cover_every_budgeted_renderer_system() {
        assert_eq!(
            SHARED_SCHEDULER_INTEGRATION_HOOKS.len(),
            SchedulerWorkSystem::ALL.len()
        );
        for system in SchedulerWorkSystem::ALL {
            let hook = scheduler_integration_hook(system);
            assert_eq!(hook.system, system);
            assert!(hook.stable_id.starts_with("renderer.scheduler.hook."));
            assert!(!hook.input_contract.is_empty());
            assert!(!hook.output_contract.is_empty());
        }
        assert_eq!(
            scheduler_integration_hook(SchedulerWorkSystem::TextureResidency).output_contract,
            "texture_residency_budget_recommendation"
        );
        assert_eq!(
            scheduler_integration_hook(SchedulerWorkSystem::ReflectionRays).input_contract,
            "reflection_ray_budget_request"
        );
    }

    #[test]
    fn presets_overrides_scene_profiles_and_debug_controls_are_tunable() {
        assert!(
            SchedulerQualityPreset::Editor.weights().editor_focus
                > SchedulerQualityPreset::Balanced.weights().editor_focus
        );

        let mut config = SharedSchedulerConfig::for_preset(SchedulerQualityPreset::Latency);
        config.developer_overrides = DeveloperPriorityOverrides {
            forced_priority_per_mille: Some(333),
            budget_scale_per_mille: 500,
            disabled_system_mask: 0,
        };
        config.scene_profile = SchedulerSceneProfile {
            stable_id: "scene_profile.test_override",
            geometry_boost_per_mille: 1100,
            shadow_boost_per_mille: 900,
            light_boost_per_mille: 1000,
            gi_boost_per_mille: 800,
        };
        config.debug_controls = SchedulerDebugControls {
            freeze_priorities: true,
            lock_budgets: false,
            force_heatmap_capture: true,
        };
        let mut scheduler = SharedHeuristicScheduler::new(config);
        let score = scheduler.evaluate(SharedPriorityInputs::stress_high());
        assert_eq!(score.normalized_per_mille, 333);

        let recommendation =
            scheduler.recommend_budget(SchedulerWorkSystem::GeometryPages, score, 512);
        assert!(recommendation.recommended_units <= 256);
        assert_eq!(
            config.scene_profile.stable_id,
            "scene_profile.test_override"
        );
        assert!(config.debug_controls.force_heatmap_capture);
    }

    #[test]
    fn page_shadow_light_budget_stress_scene_shares_one_scheduler() {
        let page_inputs = PagePriorityInputs::editor_critical();
        let shadow_inputs = ShadowPagePriorityInputs {
            visible_receiver_demand: 920,
            screen_coverage: 880,
            contrast: 700,
            temporal_instability: 820,
            gameplay_salience: 900,
            editor_focus: 128,
            light_importance: 900,
        };
        let requests = [
            SchedulerWorkRequest::new(
                SchedulerWorkSystem::GeometryPages,
                1,
                SharedPriorityInputs::from(page_inputs),
                900,
            ),
            SchedulerWorkRequest::new(
                SchedulerWorkSystem::ShadowPages,
                2,
                SharedPriorityInputs::from(shadow_inputs),
                400,
            ),
            SchedulerWorkRequest::new(
                SchedulerWorkSystem::LightCandidates,
                3,
                SharedPriorityInputs::stress_high(),
                12_000,
            ),
            SchedulerWorkRequest::new(
                SchedulerWorkSystem::GiCacheUpdates,
                4,
                SharedPriorityInputs::stress_high(),
                700,
            ),
            SchedulerWorkRequest::new(
                SchedulerWorkSystem::ReflectionRays,
                5,
                SharedPriorityInputs::stress_high(),
                40_000,
            ),
            SchedulerWorkRequest::new(
                SchedulerWorkSystem::TextureResidency,
                6,
                SharedPriorityInputs::from(page_inputs),
                600,
            ),
            SchedulerWorkRequest::new(
                SchedulerWorkSystem::ShadingRate,
                7,
                SharedPriorityInputs::stress_high(),
                5_000,
            ),
            SchedulerWorkRequest::new(
                SchedulerWorkSystem::MlInferenceDensity,
                8,
                SharedPriorityInputs::stress_high(),
                64,
            ),
        ];
        let mut scheduler =
            SharedHeuristicScheduler::new(SharedSchedulerConfig::benchmark_stress());
        let report = scheduler.schedule_frame(19, &requests, 16_200, 24_600);

        assert_eq!(report.decisions.len(), SchedulerWorkSystem::ALL.len());
        assert!(report.diagnostics.deferred_units > 0);
        assert!(report.diagnostics.high_priority_misses > 0);
        assert!(report.diagnostics.priority_heatmap.buckets[5] > 0);
        for system in SchedulerWorkSystem::ALL {
            let allocation = report.diagnostics.budget_allocations[system.index()];
            assert_eq!(allocation.system, system);
            assert!(allocation.requested_units > 0);
        }
        assert!(
            report.diagnostics.budget_allocations[SchedulerWorkSystem::MlInferenceDensity.index()]
                .accepted_units
                <= DEFAULT_SYSTEM_BUDGETS[SchedulerWorkSystem::MlInferenceDensity.index()]
                    .max_units
        );
    }

    #[test]
    fn p95_p99_frame_time_artifact_records_scheduler_diagnostics() {
        let requests = [
            SchedulerWorkRequest::new(
                SchedulerWorkSystem::GeometryPages,
                10,
                SharedPriorityInputs::stress_high(),
                1024,
            ),
            SchedulerWorkRequest::new(
                SchedulerWorkSystem::ShadowPages,
                11,
                SharedPriorityInputs::stress_high(),
                512,
            ),
            SchedulerWorkRequest::new(
                SchedulerWorkSystem::LightCandidates,
                12,
                SharedPriorityInputs::stress_high(),
                16_000,
            ),
            SchedulerWorkRequest::new(
                SchedulerWorkSystem::GiCacheUpdates,
                13,
                SharedPriorityInputs::stress_high(),
                512,
            ),
            SchedulerWorkRequest::new(
                SchedulerWorkSystem::ReflectionRays,
                14,
                SharedPriorityInputs::stress_high(),
                48_000,
            ),
            SchedulerWorkRequest::new(
                SchedulerWorkSystem::TextureResidency,
                15,
                SharedPriorityInputs::stress_high(),
                640,
            ),
            SchedulerWorkRequest::new(
                SchedulerWorkSystem::ShadingRate,
                16,
                SharedPriorityInputs::stress_high(),
                6_000,
            ),
            SchedulerWorkRequest::new(
                SchedulerWorkSystem::MlInferenceDensity,
                17,
                SharedPriorityInputs::stress_high(),
                32,
            ),
        ];
        let mut scheduler =
            SharedHeuristicScheduler::new(SharedSchedulerConfig::benchmark_stress());
        let report = scheduler.schedule_frame(190, &requests, 18_250, 27_500);
        let artifact = SharedSchedulerBenchmarkArtifact::from_report(&report);

        assert!(
            artifact
                .content
                .contains("shared_scheduler_schema_version=1")
        );
        assert!(
            artifact
                .content
                .contains("benchmark_profile=benchmark_profile.pass19_scheduler_stress")
        );
        assert!(artifact.content.contains("p95_frame_impact_us=18250"));
        assert!(artifact.content.contains("p99_frame_impact_us=27500"));
        assert!(artifact.content.contains("budget system=geometry_pages"));
        assert!(artifact.content.contains("budget system=shadow_pages"));
        assert!(artifact.content.contains("budget system=light_candidates"));
        assert!(artifact.content.contains("budget system=gi_cache_updates"));
        assert!(artifact.content.contains("budget system=reflection_rays"));
        assert!(artifact.content.contains("budget system=texture_residency"));
        assert!(artifact.content.contains("budget system=shading_rate"));
        assert!(
            artifact
                .content
                .contains("budget system=ml_inference_density")
        );
        assert!(artifact.content.contains("priority_heatmap="));
        assert!(artifact.content.contains("overlay system="));

        if let Ok(path) = std::env::var(SHARED_SCHEDULER_BENCHMARK_ARTIFACT_ENV) {
            write_shared_scheduler_benchmark_artifact(path, &artifact)
                .expect("shared scheduler artifact should be writable");
        }
    }

    #[test]
    fn debug_freeze_and_lock_controls_keep_reproducible_priority_and_budget() {
        let config = SharedSchedulerConfig {
            debug_controls: SchedulerDebugControls {
                freeze_priorities: true,
                lock_budgets: true,
                force_heatmap_capture: false,
            },
            ..SharedSchedulerConfig::default()
        };
        let mut scheduler = SharedHeuristicScheduler::new(config);
        let first = scheduler.evaluate(SharedPriorityInputs::stress_high());
        let second = scheduler.evaluate(SharedPriorityInputs::zero());
        assert_eq!(first, second);

        let recommendation =
            scheduler.recommend_budget(SchedulerWorkSystem::ShadowPages, first, 512);
        assert_eq!(
            recommendation.reason,
            BudgetRecommendationReason::LockedByDebugControl
        );
        assert!(recommendation.recommended_units > 0);
    }
}
