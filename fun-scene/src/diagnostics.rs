use bevy_ecs::prelude::Resource;

pub const TELEMETRY_BUDGET_CLASS: &str = "SampledRuntime";
pub const TELEMETRY_RETENTION_CLASS: &str = "KeepSummary";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct FunSceneDiagnosticsPolicy {
    pub compact_by_default: bool,
    pub include_stable_identity_counts: bool,
    pub include_renderer_lux_declarations: bool,
    pub expose_raw_asset_payloads: bool,
}

impl FunSceneDiagnosticsPolicy {
    pub const DEFAULT: Self = Self {
        compact_by_default: true,
        include_stable_identity_counts: true,
        include_renderer_lux_declarations: true,
        expose_raw_asset_payloads: false,
    };
}

impl Default for FunSceneDiagnosticsPolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}
