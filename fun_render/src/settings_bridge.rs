use bevy::prelude::Resource;
use fun_renderer::{
    BudgetPreset, DiagnosticsVisibility, GraphicsBackendSetting, RendererCapabilityFacts,
    RendererQualityTier, RendererSettingsAdjustment, RendererSettingsSelection,
    RendererUserSettings, RuntimeBackendSetting, select_capability_aware_renderer_defaults,
};
use serde::Serialize;

use crate::RendererBridgeSettings;

pub const RENDERER_SETTINGS_UI_BRIDGE_SCHEMA: &str = "fun.render.settings_ui_bridge.v1";

#[derive(Debug, Clone, PartialEq, Eq, Resource, Serialize)]
pub struct RendererSettingsUiModel {
    pub schema: &'static str,
    pub selected: RendererSettingsUiSelection,
    pub quality_options: Vec<RendererSettingsUiOption>,
    pub upscaler_options: Vec<RendererSettingsUiOption>,
    pub frame_generation_options: Vec<RendererSettingsUiOption>,
    pub disabled_reasons: Vec<RendererSettingsUiDisabledReason>,
    pub diagnostics_visible: bool,
    pub benchmark_repro_label: RendererSettingsBenchmarkReproLabel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RendererSettingsUiSelection {
    pub runtime_backend: &'static str,
    pub graphics_backend: &'static str,
    pub quality_preset: &'static str,
    pub upscaler: &'static str,
    pub frame_generation: &'static str,
    pub shadow_quality: &'static str,
    pub gi_quality: &'static str,
    pub reflection_quality: &'static str,
    pub virtual_geometry_budget: &'static str,
    pub texture_streaming_budget: &'static str,
    pub diagnostics_visibility: &'static str,
}

impl RendererSettingsUiSelection {
    #[must_use]
    pub const fn from_user_settings(settings: RendererUserSettings) -> Self {
        Self {
            runtime_backend: settings.runtime_backend.as_str(),
            graphics_backend: settings.graphics_backend.as_str(),
            quality_preset: settings.quality_preset.as_str(),
            upscaler: settings.upscaler.as_str(),
            frame_generation: settings.frame_generation.as_str(),
            shadow_quality: settings.shadow_quality.as_str(),
            gi_quality: settings.gi_quality.as_str(),
            reflection_quality: settings.reflection_quality.as_str(),
            virtual_geometry_budget: settings.virtual_geometry_budget.as_str(),
            texture_streaming_budget: settings.texture_streaming_budget.as_str(),
            diagnostics_visibility: settings.diagnostics_visibility.as_str(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RendererSettingsUiOption {
    pub id: &'static str,
    pub label: &'static str,
    pub enabled: bool,
    pub reason: RendererSettingsUiOptionReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RendererSettingsUiOptionReason {
    Available,
    UnsupportedByCapability,
    DisabledByPolicy,
    ProductFailClosed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RendererSettingsUiDisabledReason {
    pub setting: &'static str,
    pub requested: &'static str,
    pub selected: &'static str,
    pub reason: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RendererSettingsBenchmarkReproLabel {
    pub quality_preset: &'static str,
    pub runtime_backend: &'static str,
    pub graphics_backend: &'static str,
    pub page_pool_pages: u32,
    pub shadow_page_budget: u32,
    pub light_candidate_budget: u32,
    pub gi_cache_update_budget: u32,
    pub pipeline_warmup_policy: &'static str,
    pub runtime_pipeline_creation: &'static str,
    pub fallback_strictness: &'static str,
}

impl RendererSettingsUiModel {
    #[must_use]
    pub fn from_selection(selection: &RendererSettingsSelection) -> Self {
        Self {
            schema: RENDERER_SETTINGS_UI_BRIDGE_SCHEMA,
            selected: RendererSettingsUiSelection::from_user_settings(selection.selected_user),
            quality_options: quality_options(selection),
            upscaler_options: upscaler_options(selection),
            frame_generation_options: frame_generation_options(selection),
            disabled_reasons: disabled_reasons(&selection.adjustments),
            diagnostics_visible: !matches!(
                selection.selected_user.diagnostics_visibility,
                DiagnosticsVisibility::Hidden
            ),
            benchmark_repro_label: RendererSettingsBenchmarkReproLabel {
                quality_preset: selection.benchmark_repro.quality_preset.as_str(),
                runtime_backend: selection.benchmark_repro.runtime_backend.as_str(),
                graphics_backend: selection.benchmark_repro.graphics_backend.as_str(),
                page_pool_pages: selection.benchmark_repro.page_pool_pages,
                shadow_page_budget: selection.benchmark_repro.shadow_page_budget,
                light_candidate_budget: selection.benchmark_repro.light_candidate_budget,
                gi_cache_update_budget: selection.benchmark_repro.gi_cache_update_budget,
                pipeline_warmup_policy: selection.benchmark_repro.pipeline_warmup_policy.as_str(),
                runtime_pipeline_creation: selection
                    .benchmark_repro
                    .runtime_pipeline_creation
                    .as_str(),
                fallback_strictness: selection.benchmark_repro.fallback_strictness.as_str(),
            },
        }
    }
}

#[must_use]
pub fn renderer_settings_ui_model_from_bridge(
    settings: RendererBridgeSettings,
) -> RendererSettingsUiModel {
    let runtime_backend = match settings.backend_selection.resolved {
        fun_renderer::FunRendererRuntimeBackend::Auto => RuntimeBackendSetting::Auto,
        fun_renderer::FunRendererRuntimeBackend::Fun => RuntimeBackendSetting::NewCore,
        fun_renderer::FunRendererRuntimeBackend::Legacy => RuntimeBackendSetting::Legacy,
    };
    let graphics_backend = match settings.preferred_backend {
        fun_renderer::FunRendererBackend::Dx12 => GraphicsBackendSetting::Dx12,
        fun_renderer::FunRendererBackend::Vulkan => GraphicsBackendSetting::Vulkan,
        fun_renderer::FunRendererBackend::Metal => GraphicsBackendSetting::Metal,
    };
    let capabilities = RendererCapabilityFacts::from_renderer_features(
        settings.features.renderer_core_toggles(),
        runtime_backend,
        graphics_backend,
        fun_renderer::RendererRuntimeMode::Product,
        settings.features.cef_gpu_only,
    );
    let selection = select_capability_aware_renderer_defaults(capabilities);
    RendererSettingsUiModel::from_selection(&selection)
}

fn quality_options(selection: &RendererSettingsSelection) -> Vec<RendererSettingsUiOption> {
    RendererQualityTier::ORDER
        .iter()
        .copied()
        .map(|tier| RendererSettingsUiOption {
            id: tier.as_str(),
            label: tier.as_str(),
            enabled: tier.rank() <= selection.selected_user.quality_preset.rank(),
            reason: if selection.product_ui_usable {
                RendererSettingsUiOptionReason::Available
            } else {
                RendererSettingsUiOptionReason::ProductFailClosed
            },
        })
        .collect()
}

fn upscaler_options(selection: &RendererSettingsSelection) -> Vec<RendererSettingsUiOption> {
    const OPTIONS: [(&str, &str, bool); 4] = [
        ("native_fallback", "native_fallback", true),
        ("dlss_super_resolution", "dlss_super_resolution", false),
        ("fsr2", "fsr2", false),
        ("fsr3", "fsr3", false),
    ];
    OPTIONS
        .iter()
        .map(|(id, label, native)| {
            let enabled = *native
                || (*id == "dlss_super_resolution" && selection.capabilities.upscalers.dlss_sr)
                || (*id == "fsr2" && selection.capabilities.upscalers.fsr2)
                || (*id == "fsr3" && selection.capabilities.upscalers.fsr3);
            RendererSettingsUiOption {
                id,
                label,
                enabled,
                reason: option_reason(enabled, selection.product_ui_usable),
            }
        })
        .collect()
}

fn frame_generation_options(
    selection: &RendererSettingsSelection,
) -> Vec<RendererSettingsUiOption> {
    const OPTIONS: [(&str, &str, bool); 3] = [
        ("disabled", "disabled", true),
        ("dlss_frame_generation", "dlss_frame_generation", false),
        ("fsr_frame_generation", "fsr_frame_generation", false),
    ];
    OPTIONS
        .iter()
        .map(|(id, label, always)| {
            let enabled = *always
                || (*id == "dlss_frame_generation"
                    && selection.capabilities.frame_generation.dlss_fg)
                || (*id == "fsr_frame_generation"
                    && selection.capabilities.frame_generation.fsr_fg);
            RendererSettingsUiOption {
                id,
                label,
                enabled,
                reason: option_reason(enabled, selection.product_ui_usable),
            }
        })
        .collect()
}

const fn option_reason(enabled: bool, product_ui_usable: bool) -> RendererSettingsUiOptionReason {
    if !product_ui_usable {
        RendererSettingsUiOptionReason::ProductFailClosed
    } else if enabled {
        RendererSettingsUiOptionReason::Available
    } else {
        RendererSettingsUiOptionReason::UnsupportedByCapability
    }
}

fn disabled_reasons(
    adjustments: &[RendererSettingsAdjustment],
) -> Vec<RendererSettingsUiDisabledReason> {
    adjustments
        .iter()
        .map(|adjustment| RendererSettingsUiDisabledReason {
            setting: adjustment.setting.as_str(),
            requested: adjustment.requested,
            selected: adjustment.selected,
            reason: adjustment.reason.as_str(),
        })
        .collect()
}

#[must_use]
pub const fn virtual_geometry_budget_to_ui_label(budget: BudgetPreset) -> &'static str {
    budget.as_str()
}

#[cfg(test)]
mod tests {
    use bevy::prelude::App;

    use super::*;
    use crate::{BridgeFeatureToggles, RendererBridgeSettings, install_renderer_bridge_api};

    #[test]
    fn ui_model_contains_only_cef_safe_renderer_settings() {
        let selection = select_capability_aware_renderer_defaults(
            RendererCapabilityFacts::high_end_dx12_nvidia(),
        );
        let model = RendererSettingsUiModel::from_selection(&selection);
        let payload = serde_json::to_string(&model).expect("ui model should serialize");

        assert_eq!(model.schema, RENDERER_SETTINGS_UI_BRIDGE_SCHEMA);
        assert!(payload.contains("\"quality_preset\":\"high\""));
        assert!(payload.contains("\"runtime_backend\":\"fun\""));
        assert!(payload.contains("\"benchmark_repro_label\""));
        assert!(!payload.contains("adapter_name"));
        assert!(!payload.contains("driver"));
    }

    #[test]
    fn bridge_install_inserts_settings_ui_model_for_game_boot() {
        let mut app = App::new();
        install_renderer_bridge_api(
            &mut app,
            RendererBridgeSettings::from_runtime_backend(
                fun_renderer::FunRendererRuntimeBackend::Fun,
            ),
        );

        let model = app
            .world()
            .get_resource::<RendererSettingsUiModel>()
            .expect("bridge should expose settings model to CEF/Svelte");

        assert_eq!(model.schema, RENDERER_SETTINGS_UI_BRIDGE_SCHEMA);
        let expected_runtime_backend = if BridgeFeatureToggles::COMPILED.new_core {
            "fun"
        } else {
            "legacy"
        };
        assert_eq!(model.selected.runtime_backend, expected_runtime_backend);
        assert!(!model.quality_options.is_empty());
    }
}
