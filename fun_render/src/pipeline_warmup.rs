use std::time::{Duration, Instant};

use bevy::{
    prelude::*,
    render::{Render, RenderApp, RenderSystems, render_resource::PipelineCache},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunPipelineWarmupMode {
    Off,
    Basic,
    Scene,
    Exhaustive,
}

impl FunPipelineWarmupMode {
    pub const fn as_env_value(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Basic => "basic",
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

#[derive(Debug, Default)]
struct FunPipelineWarmupState {
    frames_run: u32,
}

pub fn install_fun_pipeline_warmup(app: &mut App) {
    let config = FunPipelineWarmupConfig::from_env();
    if !config.is_enabled() {
        return;
    }
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
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

fn run_fun_pipeline_warmup(
    mut pipeline_cache: ResMut<PipelineCache>,
    config: Res<FunPipelineWarmupConfig>,
    mut state: Local<FunPipelineWarmupState>,
) {
    if !should_run_warmup(config.mode, state.frames_run) {
        return;
    }
    let waiting_before = pipeline_cache.waiting_pipelines().count();
    let started = Instant::now();
    pipeline_cache.process_queue();
    let elapsed = started.elapsed();
    let waiting_after = pipeline_cache.waiting_pipelines().count();
    state.frames_run = state.frames_run.saturating_add(1);

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

const fn should_run_warmup(mode: FunPipelineWarmupMode, frames_run: u32) -> bool {
    match mode {
        FunPipelineWarmupMode::Off => false,
        FunPipelineWarmupMode::Basic => frames_run == 0,
        FunPipelineWarmupMode::Scene => frames_run < 120,
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
        assert!(!should_run_warmup(FunPipelineWarmupMode::Off, 0));
        assert!(should_run_warmup(FunPipelineWarmupMode::Basic, 0));
        assert!(!should_run_warmup(FunPipelineWarmupMode::Basic, 1));
        assert!(should_run_warmup(FunPipelineWarmupMode::Scene, 119));
        assert!(!should_run_warmup(FunPipelineWarmupMode::Scene, 120));
        assert!(should_run_warmup(
            FunPipelineWarmupMode::Exhaustive,
            u32::MAX
        ));
    }
}
