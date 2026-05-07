use std::time::{Duration, Instant};

use bevy::{
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems, render_resource::PipelineCache, settings::Backends,
    },
};
use fun_renderer::{
    FunRendererBackend, PipelineRegistry, PipelineWarmupBoundary, PipelineWarmupPlan,
    PipelineWarmupRequest, QualityTier,
};

use crate::selected_render_backend;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunPipelineWarmupMode {
    Off,
    Basic,
    Observed,
    Scene,
    Exhaustive,
}

impl FunPipelineWarmupMode {
    pub const fn as_env_value(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Basic => "basic",
            Self::Observed => "observed",
            Self::Scene => "scene",
            Self::Exhaustive => "exhaustive",
        }
    }
}

#[derive(Debug, Clone, Copy, Resource)]
pub struct FunPipelineWarmupConfig {
    pub mode: FunPipelineWarmupMode,
    pub budget: Duration,
}

impl FunPipelineWarmupConfig {
    pub fn from_env() -> Self {
        Self {
            mode: pipeline_warmup_mode_from_env(),
            budget: pipeline_warmup_budget_from_env(),
        }
    }

    pub const fn is_enabled(self) -> bool {
        !matches!(self.mode, FunPipelineWarmupMode::Off)
    }
}

#[derive(Debug, Clone, Copy, Resource)]
pub struct RendererPipelineWarmupPlan {
    pub boundary: PipelineWarmupBoundary,
    pub backend: FunRendererBackend,
    pub quality_tier: QualityTier,
    pub eligible_pipeline_count: u32,
    pub total_pipeline_count: u32,
    pub shader_module_count: u32,
    pub shader_variant_count: u32,
    pub first_pipeline_label: Option<&'static str>,
}

impl RendererPipelineWarmupPlan {
    fn from_registry_plan(registry: PipelineRegistry, plan: PipelineWarmupPlan) -> Self {
        Self {
            boundary: plan.boundary,
            backend: plan.backend,
            quality_tier: plan.quality_tier,
            eligible_pipeline_count: plan.eligible_pipeline_count as u32,
            total_pipeline_count: plan.total_pipeline_count as u32,
            shader_module_count: registry.shader_module_count() as u32,
            shader_variant_count: registry.shader_variant_count() as u32,
            first_pipeline_label: plan.first_pipeline_label,
        }
    }

    fn diagnostic_fields(
        self,
    ) -> (
        &'static str,
        &'static str,
        &'static str,
        u32,
        u32,
        u32,
        u32,
        &'static str,
    ) {
        (
            self.boundary.as_str(),
            self.backend.as_str(),
            self.quality_tier.as_str(),
            self.eligible_pipeline_count,
            self.total_pipeline_count,
            self.shader_module_count,
            self.shader_variant_count,
            self.first_pipeline_label.unwrap_or("none"),
        )
    }
}

#[derive(Debug, Default)]
struct FunPipelineWarmupState {
    frames_run: u32,
    idle_frames: u32,
}

pub fn install_fun_pipeline_warmup(app: &mut App) {
    let config = FunPipelineWarmupConfig::from_env();
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    let registry = PipelineRegistry::default();
    let request = PipelineWarmupRequest::renderer_initialization(selected_pipeline_backend());
    let plan = registry.warmup_plan(request);
    let bridge_plan = RendererPipelineWarmupPlan::from_registry_plan(registry, plan);
    let (
        boundary,
        backend,
        quality_tier,
        eligible_pipeline_count,
        total_pipeline_count,
        shader_module_count,
        shader_variant_count,
        first_pipeline_label,
    ) = bridge_plan.diagnostic_fields();
    render_app.insert_resource(bridge_plan);
    game_shared::fun_diag_info!(
        target: "fun::render",
        boundary = boundary,
        backend = backend,
        quality_tier = quality_tier,
        eligible_pipelines = eligible_pipeline_count,
        total_pipelines = total_pipeline_count,
        shader_modules = shader_module_count,
        shader_variants = shader_variant_count,
        first_pipeline = first_pipeline_label,
        "Renderer pipeline registry warmup plan"
    );
    let _ = (
        boundary,
        backend,
        quality_tier,
        eligible_pipeline_count,
        total_pipeline_count,
        shader_module_count,
        shader_variant_count,
        first_pipeline_label,
    );
    if !config.is_enabled() {
        return;
    }
    render_app.insert_resource(config);
    render_app.add_systems(
        Render,
        run_fun_pipeline_warmup
            .in_set(RenderSystems::Prepare)
            .ambiguous_with_all(),
    );
    game_shared::fun_diag_info!(
        target: "fun::render",
        mode = config.mode.as_env_value(),
        budget_ms = config.budget.as_secs_f64() * 1000.0,
        "Fun pipeline warmup enabled"
    );
}

fn selected_pipeline_backend() -> FunRendererBackend {
    let selected = selected_render_backend();
    if selected.contains(Backends::DX12) {
        FunRendererBackend::Dx12
    } else if selected.contains(Backends::METAL) {
        FunRendererBackend::Metal
    } else {
        FunRendererBackend::Vulkan
    }
}

fn run_fun_pipeline_warmup(
    mut pipeline_cache: ResMut<PipelineCache>,
    config: Res<FunPipelineWarmupConfig>,
    mut state: Local<FunPipelineWarmupState>,
) {
    if !should_run_warmup(config.mode, &state) {
        return;
    }
    let waiting_before = pipeline_cache.waiting_pipelines().count();
    let started = Instant::now();
    pipeline_cache.process_queue();
    let elapsed = started.elapsed();
    let waiting_after = pipeline_cache.waiting_pipelines().count();
    state.frames_run = state.frames_run.saturating_add(1);
    if waiting_before == 0 && waiting_after == 0 {
        state.idle_frames = state.idle_frames.saturating_add(1);
    } else {
        state.idle_frames = 0;
    }

    if waiting_before > 0 || waiting_after > 0 || elapsed > config.budget {
        game_shared::fun_diag_info!(
            target: "fun::render",
            mode = config.mode.as_env_value(),
            frames_run = state.frames_run,
            waiting_before,
            waiting_after,
            elapsed_ns = elapsed.as_nanos() as u64,
            budget_ns = config.budget.as_nanos() as u64,
            budget_exceeded = elapsed > config.budget,
            "Fun pipeline warmup processed queued pipelines"
        );
        game_shared::fun_diag_info!(
            "[fun render] pipeline warmup mode={} waiting_before={} waiting_after={} elapsed_ns={} budget_ns={} budget_exceeded={}",
            config.mode.as_env_value(),
            waiting_before,
            waiting_after,
            elapsed.as_nanos(),
            config.budget.as_nanos(),
            elapsed > config.budget,
        );
    }
}

const OBSERVED_IDLE_FRAME_LIMIT: u32 = 30;
const OBSERVED_MAX_FRAME_LIMIT: u32 = 240;

const fn should_run_warmup(mode: FunPipelineWarmupMode, state: &FunPipelineWarmupState) -> bool {
    match mode {
        FunPipelineWarmupMode::Off => false,
        FunPipelineWarmupMode::Basic => state.frames_run == 0,
        FunPipelineWarmupMode::Observed => {
            state.frames_run < OBSERVED_MAX_FRAME_LIMIT
                && state.idle_frames < OBSERVED_IDLE_FRAME_LIMIT
        }
        FunPipelineWarmupMode::Scene => state.frames_run < 120,
        FunPipelineWarmupMode::Exhaustive => true,
    }
}

fn pipeline_warmup_mode_from_env() -> FunPipelineWarmupMode {
    let Some(value) = std::env::var("FUN_RENDER_PIPELINE_WARMUP").ok() else {
        return FunPipelineWarmupMode::Off;
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "off" | "0" | "false" | "disabled" => FunPipelineWarmupMode::Off,
        "basic" | "1" | "true" | "on" => FunPipelineWarmupMode::Basic,
        "observed" => FunPipelineWarmupMode::Observed,
        "scene" => FunPipelineWarmupMode::Scene,
        "exhaustive" => FunPipelineWarmupMode::Exhaustive,
        _ => FunPipelineWarmupMode::Off,
    }
}

fn pipeline_warmup_budget_from_env() -> Duration {
    let millis = std::env::var("FUN_RENDER_PIPELINE_WARMUP_BUDGET_MS")
        .ok()
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(2.0)
        .clamp(0.1, 100.0);
    Duration::from_secs_f64(millis / 1000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warmup_run_policy_is_mode_bounded() {
        let state = |frames_run, idle_frames| FunPipelineWarmupState {
            frames_run,
            idle_frames,
        };
        assert!(!should_run_warmup(FunPipelineWarmupMode::Off, &state(0, 0)));
        assert!(should_run_warmup(
            FunPipelineWarmupMode::Basic,
            &state(0, 0)
        ));
        assert!(!should_run_warmup(
            FunPipelineWarmupMode::Basic,
            &state(1, 0)
        ));
        assert!(should_run_warmup(
            FunPipelineWarmupMode::Observed,
            &state(239, 29)
        ));
        assert!(!should_run_warmup(
            FunPipelineWarmupMode::Observed,
            &state(240, 0)
        ));
        assert!(!should_run_warmup(
            FunPipelineWarmupMode::Observed,
            &state(1, 30)
        ));
        assert!(should_run_warmup(
            FunPipelineWarmupMode::Scene,
            &state(119, 0)
        ));
        assert!(!should_run_warmup(
            FunPipelineWarmupMode::Scene,
            &state(120, 0)
        ));
        assert!(should_run_warmup(
            FunPipelineWarmupMode::Exhaustive,
            &state(u32::MAX, u32::MAX)
        ));
    }

    #[test]
    fn warmup_plan_uses_fun_renderer_pipeline_registry() {
        let registry = PipelineRegistry::default();
        let request = PipelineWarmupRequest {
            boundary: PipelineWarmupBoundary::RendererInitialization,
            backend: FunRendererBackend::Dx12,
            quality_tier: QualityTier::Balanced,
            features: fun_renderer::PipelineFeatureMask::ALL,
        };
        let plan =
            RendererPipelineWarmupPlan::from_registry_plan(registry, registry.warmup_plan(request));

        assert!(plan.eligible_pipeline_count >= 10);
        assert_eq!(
            plan.boundary,
            PipelineWarmupBoundary::RendererInitialization
        );
        assert_eq!(plan.backend, FunRendererBackend::Dx12);
        assert_eq!(plan.quality_tier, QualityTier::Balanced);
        assert!(plan.total_pipeline_count >= plan.eligible_pipeline_count);
        assert!(plan.shader_module_count > 0);
        assert!(plan.shader_variant_count > 0);
        assert_eq!(
            plan.first_pipeline_label,
            Some("fun_compute_culling_reset_pipeline")
        );
        assert!(fun_renderer::PipelineQualityTierMask::ALL.contains_tier(plan.quality_tier));
    }
}
