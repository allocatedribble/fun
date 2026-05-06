use crate::{LuxLightId, LuxLightKind};

pub const SHADOW_POLICY_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShadowQualityTier {
    Off,
    Low,
    Medium,
    #[default]
    High,
    Cinematic,
}

impl ShadowQualityTier {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Cinematic => "cinematic",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoftShadowMode {
    Hard,
    Pcf,
    #[default]
    ContactAware,
    StochasticSoft,
}

impl SoftShadowMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hard => "hard",
            Self::Pcf => "pcf",
            Self::ContactAware => "contact_aware",
            Self::StochasticSoft => "stochastic_soft",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShadowReconstructionMode {
    None,
    Temporal,
    #[default]
    SpatialTemporal,
    Denoised,
}

impl ShadowReconstructionMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Temporal => "temporal",
            Self::SpatialTemporal => "spatial_temporal",
            Self::Denoised => "denoised",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShadowCastDecision {
    #[default]
    DisabledByPolicy,
    CastDirectionalClipmap,
    CastLocalPaged,
}

impl ShadowCastDecision {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DisabledByPolicy => "disabled_by_policy",
            Self::CastDirectionalClipmap => "cast_directional_clipmap",
            Self::CastLocalPaged => "cast_local_paged",
        }
    }

    #[must_use]
    pub const fn casts_shadow(self) -> bool {
        !matches!(self, Self::DisabledByPolicy)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowPolicyConfig {
    pub quality_tier: ShadowQualityTier,
    pub soft_shadow_mode: SoftShadowMode,
    pub reconstruction_mode: ShadowReconstructionMode,
    pub local_light_shadow_budget: u32,
    pub directional_clipmap_budget: u32,
    pub min_shadow_intensity_lux: f32,
    pub max_shadow_casters_per_frame: u32,
}

impl ShadowPolicyConfig {
    pub const DEFAULT: Self = Self {
        quality_tier: ShadowQualityTier::High,
        soft_shadow_mode: SoftShadowMode::ContactAware,
        reconstruction_mode: ShadowReconstructionMode::SpatialTemporal,
        local_light_shadow_budget: 96,
        directional_clipmap_budget: 64,
        min_shadow_intensity_lux: 0.05,
        max_shadow_casters_per_frame: 128,
    };
}

impl Default for ShadowPolicyConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowLightInput {
    pub light_id: LuxLightId,
    pub kind: LuxLightKind,
    pub intensity_lux: f32,
    pub importance: u8,
    pub casts_shadow_authoring: bool,
}

impl ShadowLightInput {
    #[must_use]
    pub const fn new(
        light_id: LuxLightId,
        kind: LuxLightKind,
        intensity_lux: f32,
        importance: u8,
        casts_shadow_authoring: bool,
    ) -> Self {
        Self {
            light_id,
            kind,
            intensity_lux,
            importance,
            casts_shadow_authoring,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ShadowReceiverDemand {
    pub visible_receiver_demand: u16,
    pub screen_coverage: u16,
    pub contrast: u16,
    pub temporal_instability: u16,
    pub gameplay_salience: u16,
    pub editor_focus: u16,
}

impl ShadowReceiverDemand {
    #[must_use]
    pub fn score(self) -> u16 {
        let score = u32::from(self.visible_receiver_demand) * 4
            + u32::from(self.screen_coverage) * 3
            + u32::from(self.contrast) * 2
            + u32::from(self.temporal_instability) * 2
            + u32::from(self.gameplay_salience) * 4
            + u32::from(self.editor_focus) * 5;
        score.min(u32::from(u16::MAX)) as u16
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShadowPolicyDecision {
    pub light_id: LuxLightId,
    pub decision: ShadowCastDecision,
    pub quality_tier: ShadowQualityTier,
    pub soft_shadow_mode: SoftShadowMode,
    pub reconstruction_mode: ShadowReconstructionMode,
    pub page_budget: u32,
    pub priority: u16,
}

impl ShadowPolicyDecision {
    #[must_use]
    pub const fn casts_shadow(self) -> bool {
        self.decision.casts_shadow()
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ShadowPolicyDiagnostics {
    pub evaluated_lights: u32,
    pub shadow_casting_lights: u32,
    pub disabled_lights: u32,
    pub local_caster_count: u32,
    pub directional_caster_count: u32,
    pub local_budget_pages: u32,
    pub directional_budget_pages: u32,
}

impl ShadowPolicyDiagnostics {
    #[must_use]
    pub fn from_decisions(decisions: &[ShadowPolicyDecision]) -> Self {
        let mut diagnostics = Self {
            evaluated_lights: decisions.len() as u32,
            ..Self::default()
        };
        for decision in decisions {
            match decision.decision {
                ShadowCastDecision::DisabledByPolicy => {
                    diagnostics.disabled_lights = diagnostics.disabled_lights.saturating_add(1);
                }
                ShadowCastDecision::CastDirectionalClipmap => {
                    diagnostics.shadow_casting_lights =
                        diagnostics.shadow_casting_lights.saturating_add(1);
                    diagnostics.directional_caster_count =
                        diagnostics.directional_caster_count.saturating_add(1);
                    diagnostics.directional_budget_pages = diagnostics
                        .directional_budget_pages
                        .saturating_add(decision.page_budget);
                }
                ShadowCastDecision::CastLocalPaged => {
                    diagnostics.shadow_casting_lights =
                        diagnostics.shadow_casting_lights.saturating_add(1);
                    diagnostics.local_caster_count =
                        diagnostics.local_caster_count.saturating_add(1);
                    diagnostics.local_budget_pages = diagnostics
                        .local_budget_pages
                        .saturating_add(decision.page_budget);
                }
            }
        }
        diagnostics
    }
}

#[derive(Debug, Clone)]
pub struct ShadowPolicyEngine {
    config: ShadowPolicyConfig,
    diagnostics: ShadowPolicyDiagnostics,
}

impl ShadowPolicyEngine {
    #[must_use]
    pub const fn new(config: ShadowPolicyConfig) -> Self {
        Self {
            config,
            diagnostics: ShadowPolicyDiagnostics {
                evaluated_lights: 0,
                shadow_casting_lights: 0,
                disabled_lights: 0,
                local_caster_count: 0,
                directional_caster_count: 0,
                local_budget_pages: 0,
                directional_budget_pages: 0,
            },
        }
    }

    #[must_use]
    pub const fn config(&self) -> ShadowPolicyConfig {
        self.config
    }

    #[must_use]
    pub const fn diagnostics(&self) -> ShadowPolicyDiagnostics {
        self.diagnostics
    }

    #[must_use]
    pub fn evaluate_light(
        &mut self,
        input: ShadowLightInput,
        receiver: ShadowReceiverDemand,
    ) -> ShadowPolicyDecision {
        let decision = self.decide_light(input, receiver);
        self.diagnostics = ShadowPolicyDiagnostics::from_decisions(&[decision]);
        decision
    }

    #[must_use]
    pub fn evaluate_many(
        &mut self,
        inputs: &[ShadowLightInput],
        receiver: ShadowReceiverDemand,
    ) -> Vec<ShadowPolicyDecision> {
        let mut decisions: Vec<ShadowPolicyDecision> = inputs
            .iter()
            .copied()
            .map(|input| self.decide_light(input, receiver))
            .collect();
        let mut casting_indices: Vec<usize> = decisions
            .iter()
            .enumerate()
            .filter_map(|(index, decision)| decision.casts_shadow().then_some(index))
            .collect();
        casting_indices.sort_by(|left, right| {
            decisions[*right]
                .priority
                .cmp(&decisions[*left].priority)
                .then_with(|| {
                    decisions[*left]
                        .light_id
                        .0
                        .cmp(&decisions[*right].light_id.0)
                })
        });
        for index in casting_indices
            .into_iter()
            .skip(self.config.max_shadow_casters_per_frame as usize)
        {
            decisions[index].decision = ShadowCastDecision::DisabledByPolicy;
            decisions[index].page_budget = 0;
        }
        self.diagnostics = ShadowPolicyDiagnostics::from_decisions(&decisions);
        decisions
    }

    fn decide_light(
        &self,
        input: ShadowLightInput,
        receiver: ShadowReceiverDemand,
    ) -> ShadowPolicyDecision {
        let priority = policy_priority(input, receiver);
        let mut decision = ShadowPolicyDecision {
            light_id: input.light_id,
            decision: ShadowCastDecision::DisabledByPolicy,
            quality_tier: self.config.quality_tier,
            soft_shadow_mode: self.config.soft_shadow_mode,
            reconstruction_mode: self.config.reconstruction_mode,
            page_budget: 0,
            priority,
        };

        if matches!(self.config.quality_tier, ShadowQualityTier::Off)
            || !input.casts_shadow_authoring
            || input.intensity_lux < self.config.min_shadow_intensity_lux
        {
            return decision;
        }

        match input.kind {
            LuxLightKind::Directional => {
                decision.decision = ShadowCastDecision::CastDirectionalClipmap;
                decision.page_budget = self.config.directional_clipmap_budget;
            }
            LuxLightKind::Punctual | LuxLightKind::Area => {
                decision.decision = ShadowCastDecision::CastLocalPaged;
                decision.page_budget = self.config.local_light_shadow_budget;
            }
            LuxLightKind::EmissiveCandidate | LuxLightKind::Probe => {}
        }
        decision
    }
}

impl Default for ShadowPolicyEngine {
    fn default() -> Self {
        Self::new(ShadowPolicyConfig::DEFAULT)
    }
}

fn policy_priority(input: ShadowLightInput, receiver: ShadowReceiverDemand) -> u16 {
    let intensity_bucket = input.intensity_lux.max(0.0).log10().max(0.0) * 32.0;
    let priority =
        u32::from(receiver.score()) + u32::from(input.importance) * 4 + intensity_bucket as u32;
    priority.min(u32::from(u16::MAX)) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    fn light(id: u64, kind: LuxLightKind, importance: u8) -> ShadowLightInput {
        ShadowLightInput::new(LuxLightId::new(id), kind, 10_000.0, importance, true)
    }

    fn demand(value: u16) -> ShadowReceiverDemand {
        ShadowReceiverDemand {
            visible_receiver_demand: value,
            screen_coverage: value / 2,
            contrast: 24,
            temporal_instability: 12,
            gameplay_salience: value / 4,
            editor_focus: 0,
        }
    }

    #[test]
    fn shadow_policy_selects_directional_clipmaps_without_storage_ownership() {
        let mut policy = ShadowPolicyEngine::default();

        let decision = policy.evaluate_light(light(1, LuxLightKind::Directional, 200), demand(100));

        assert_eq!(
            decision.decision,
            ShadowCastDecision::CastDirectionalClipmap
        );
        assert_eq!(
            decision.page_budget,
            ShadowPolicyConfig::DEFAULT.directional_clipmap_budget
        );
        assert_eq!(policy.diagnostics().directional_caster_count, 1);
        assert_eq!(policy.diagnostics().local_caster_count, 0);
    }

    #[test]
    fn shadow_policy_caps_local_light_budget() {
        let config = ShadowPolicyConfig {
            local_light_shadow_budget: 2,
            max_shadow_casters_per_frame: 4,
            ..ShadowPolicyConfig::DEFAULT
        };
        let mut policy = ShadowPolicyEngine::new(config);
        let inputs: Vec<ShadowLightInput> = (0..16)
            .map(|index| light(index + 1, LuxLightKind::Punctual, index as u8))
            .collect();

        let decisions = policy.evaluate_many(&inputs, demand(80));
        let casting_count = decisions
            .iter()
            .filter(|decision| decision.casts_shadow())
            .count();

        assert_eq!(casting_count, 4);
        assert_eq!(policy.diagnostics().local_caster_count, 4);
        assert_eq!(policy.diagnostics().local_budget_pages, 8);
        assert_eq!(policy.diagnostics().disabled_lights, 12);
    }

    #[test]
    fn shadow_policy_quality_off_disables_shadow_casters() {
        let mut policy = ShadowPolicyEngine::new(ShadowPolicyConfig {
            quality_tier: ShadowQualityTier::Off,
            ..ShadowPolicyConfig::DEFAULT
        });

        let decision = policy.evaluate_light(light(7, LuxLightKind::Area, 255), demand(255));

        assert_eq!(decision.decision, ShadowCastDecision::DisabledByPolicy);
        assert_eq!(decision.page_budget, 0);
        assert_eq!(policy.diagnostics().disabled_lights, 1);
    }

    #[test]
    fn shadow_policy_priority_uses_receiver_demand_and_light_importance() {
        let mut policy = ShadowPolicyEngine::default();

        let low = policy.evaluate_light(light(1, LuxLightKind::Punctual, 1), demand(1));
        let high = policy.evaluate_light(light(2, LuxLightKind::Punctual, 255), demand(255));

        assert!(high.priority > low.priority);
    }

    #[test]
    fn policy_diagnostics_track_budget_without_storage_state() {
        let mut policy = ShadowPolicyEngine::new(ShadowPolicyConfig {
            directional_clipmap_budget: 8,
            local_light_shadow_budget: 3,
            max_shadow_casters_per_frame: 8,
            ..ShadowPolicyConfig::DEFAULT
        });
        let inputs = [
            light(1, LuxLightKind::Directional, 255),
            light(2, LuxLightKind::Punctual, 128),
            ShadowLightInput::new(LuxLightId::new(3), LuxLightKind::Probe, 1.0, 64, true),
        ];

        let decisions = policy.evaluate_many(&inputs, demand(100));

        assert_eq!(
            decisions[0].decision,
            ShadowCastDecision::CastDirectionalClipmap
        );
        assert_eq!(decisions[1].decision, ShadowCastDecision::CastLocalPaged);
        assert_eq!(decisions[2].decision, ShadowCastDecision::DisabledByPolicy);
        assert_eq!(policy.diagnostics().directional_budget_pages, 8);
        assert_eq!(policy.diagnostics().local_budget_pages, 3);
        assert_eq!(policy.diagnostics().shadow_casting_lights, 2);
    }
}
