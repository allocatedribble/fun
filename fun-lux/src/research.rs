pub const LUX_RESEARCH_SCHEMA: &str = "fun.lux.research.v1";
pub const LUX_RESEARCH_SCHEMA_VERSION: u16 = 1;
pub const RADIANCE_NEURAL_CACHE_FEATURE_FLAG: &str = "radiance_neural_cache";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadianceNeuralCacheResearchContract {
    pub feature_flag: &'static str,
    pub owner_crate: &'static str,
    pub optional_fun_ai_support: bool,
    pub default_boot_dependency: bool,
    pub deterministic_surface_cache_fallback: bool,
    pub compares_stability: bool,
    pub compares_invalidation_behavior: bool,
    pub compares_latency: bool,
    pub compares_memory_pressure: bool,
    pub compares_visual_quality_proxy: bool,
}

pub const RADIANCE_NEURAL_CACHE_RESEARCH_CONTRACT: RadianceNeuralCacheResearchContract =
    RadianceNeuralCacheResearchContract {
        feature_flag: RADIANCE_NEURAL_CACHE_FEATURE_FLAG,
        owner_crate: crate::FUN_LUX_CRATE_NAME,
        optional_fun_ai_support: true,
        default_boot_dependency: false,
        deterministic_surface_cache_fallback: true,
        compares_stability: true,
        compares_invalidation_behavior: true,
        compares_latency: true,
        compares_memory_pressure: true,
        compares_visual_quality_proxy: true,
    };

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LuxResearchCompiledFeatures {
    pub radiance_neural_cache: bool,
}

impl LuxResearchCompiledFeatures {
    pub const COMPILED: Self = Self {
        radiance_neural_cache: cfg!(feature = "radiance_neural_cache"),
    };
}

impl Default for LuxResearchCompiledFeatures {
    fn default() -> Self {
        Self::COMPILED
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadianceNeuralCacheComparison {
    pub stability_delta_per_mille: i16,
    pub invalidation_false_stale_delta: i32,
    pub latency_delta_us: i32,
    pub memory_pressure_delta_kb: i32,
    pub quality_proxy_delta_per_mille: i16,
    pub deterministic_surface_cache_fallback: bool,
}

impl RadianceNeuralCacheComparison {
    pub const CONSERVATIVE: Self = Self {
        stability_delta_per_mille: 0,
        invalidation_false_stale_delta: 0,
        latency_delta_us: 0,
        memory_pressure_delta_kb: 0,
        quality_proxy_delta_per_mille: 0,
        deterministic_surface_cache_fallback: true,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LuxResearchRecommendation {
    Abandon,
    KeepExperimental,
    PromoteToProductionPass,
}

#[must_use]
pub const fn evaluate_radiance_neural_cache_research(
    comparison: RadianceNeuralCacheComparison,
) -> LuxResearchRecommendation {
    if !comparison.deterministic_surface_cache_fallback {
        return LuxResearchRecommendation::Abandon;
    }
    if comparison.stability_delta_per_mille > 0
        && comparison.invalidation_false_stale_delta <= 0
        && comparison.latency_delta_us <= 0
        && comparison.memory_pressure_delta_kb <= 0
        && comparison.quality_proxy_delta_per_mille > 0
    {
        return LuxResearchRecommendation::PromoteToProductionPass;
    }
    LuxResearchRecommendation::KeepExperimental
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radiance_neural_cache_research_stays_lux_owned_and_optional() {
        let contract = RADIANCE_NEURAL_CACHE_RESEARCH_CONTRACT;

        assert_eq!(contract.owner_crate, crate::FUN_LUX_CRATE_NAME);
        assert_eq!(contract.feature_flag, RADIANCE_NEURAL_CACHE_FEATURE_FLAG);
        assert!(contract.optional_fun_ai_support);
        assert!(!contract.default_boot_dependency);
        assert!(contract.deterministic_surface_cache_fallback);
        assert!(contract.compares_stability);
        assert!(contract.compares_invalidation_behavior);
        assert!(contract.compares_latency);
        assert!(contract.compares_memory_pressure);
        assert!(contract.compares_visual_quality_proxy);
    }

    #[test]
    fn radiance_neural_cache_requires_deterministic_surface_cache_fallback() {
        let mut comparison = RadianceNeuralCacheComparison::CONSERVATIVE;
        comparison.stability_delta_per_mille = 80;
        comparison.quality_proxy_delta_per_mille = 120;
        comparison.latency_delta_us = -40;
        comparison.memory_pressure_delta_kb = -512;
        comparison.deterministic_surface_cache_fallback = false;

        assert_eq!(
            evaluate_radiance_neural_cache_research(comparison),
            LuxResearchRecommendation::Abandon
        );
    }

    #[test]
    fn radiance_neural_cache_can_promote_only_with_better_stability_latency_and_memory() {
        let comparison = RadianceNeuralCacheComparison {
            stability_delta_per_mille: 45,
            invalidation_false_stale_delta: -3,
            latency_delta_us: -60,
            memory_pressure_delta_kb: -2_048,
            quality_proxy_delta_per_mille: 90,
            deterministic_surface_cache_fallback: true,
        };

        assert_eq!(
            evaluate_radiance_neural_cache_research(comparison),
            LuxResearchRecommendation::PromoteToProductionPass
        );

        let mut regressed = comparison;
        regressed.memory_pressure_delta_kb = 4_096;
        assert_eq!(
            evaluate_radiance_neural_cache_research(regressed),
            LuxResearchRecommendation::KeepExperimental
        );
    }
}
