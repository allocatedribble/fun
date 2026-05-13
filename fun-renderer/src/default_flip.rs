use std::{
    fmt::Write as _,
    fs, io,
    path::{Path, PathBuf},
};

use serde::Serialize;

use crate::{
    FUN_RENDER_DONOR_PACKAGE_NAME, FUN_RENDERER_AI_OWNER_PACKAGE_NAME, FUN_RENDERER_CRATE_NAME,
    FUN_RENDERER_CURRENT_AUTO_RESOLUTION, FUN_RENDERER_NATIVE_UI_RUNTIME_POLICY,
    FUN_RENDERER_RUNTIME_BACKEND_ENV, FUN_RENDERER_SCENE_OWNER_PACKAGE_NAME,
    FUN_RENDERER_UI_RUNTIME_POLICY, FunRendererRuntimeBackend, fun_lux,
};

pub const RENDERER_DEFAULT_FLIP_SCHEMA: &str = "fun.renderer.default_flip.v1";
pub const RENDERER_DEFAULT_FLIP_SCHEMA_VERSION: u16 = 1;
pub const RENDERER_DEFAULT_FLIP_STAGE_COUNT: usize = 6;
pub const RENDERER_DEFAULT_FLIP_ARTIFACT_ENV: &str = "FUN_RENDERER_DEFAULT_FLIP_ARTIFACT";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RendererDefaultFlipStage {
    InternalMinimalScene,
    BenchmarkScenes,
    EditorLauncherViewports,
    SelectedGameClientScenes,
    GlobalDefault,
    LegacyDiagnosticOnly,
}

impl RendererDefaultFlipStage {
    pub const ORDER: [Self; RENDERER_DEFAULT_FLIP_STAGE_COUNT] = [
        Self::InternalMinimalScene,
        Self::BenchmarkScenes,
        Self::EditorLauncherViewports,
        Self::SelectedGameClientScenes,
        Self::GlobalDefault,
        Self::LegacyDiagnosticOnly,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InternalMinimalScene => "internal_minimal_scene",
            Self::BenchmarkScenes => "benchmark_scenes",
            Self::EditorLauncherViewports => "editor_launcher_viewports",
            Self::SelectedGameClientScenes => "selected_game_client_scenes",
            Self::GlobalDefault => "global_default",
            Self::LegacyDiagnosticOnly => "legacy_diagnostic_only",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RendererDefaultFlipStageDescriptor {
    pub stage: RendererDefaultFlipStage,
    pub stable_id: &'static str,
    pub default_backend: &'static str,
    pub new_renderer_default: bool,
    pub legacy_product_path_allowed: bool,
    pub benchmark_artifact_required: bool,
    pub renderer_visible_handoff_required: bool,
}

impl RendererDefaultFlipStageDescriptor {
    #[must_use]
    pub const fn new(
        stage: RendererDefaultFlipStage,
        stable_id: &'static str,
        new_renderer_default: bool,
        legacy_product_path_allowed: bool,
        benchmark_artifact_required: bool,
        renderer_visible_handoff_required: bool,
    ) -> Self {
        Self {
            stage,
            stable_id,
            default_backend: if new_renderer_default {
                "fun"
            } else {
                "legacy"
            },
            new_renderer_default,
            legacy_product_path_allowed,
            benchmark_artifact_required,
            renderer_visible_handoff_required,
        }
    }
}

pub const RENDERER_DEFAULT_FLIP_STAGES: [RendererDefaultFlipStageDescriptor;
    RENDERER_DEFAULT_FLIP_STAGE_COUNT] = [
    RendererDefaultFlipStageDescriptor::new(
        RendererDefaultFlipStage::InternalMinimalScene,
        "renderer.default_flip.stage_1.internal_minimal_scene",
        true,
        true,
        true,
        false,
    ),
    RendererDefaultFlipStageDescriptor::new(
        RendererDefaultFlipStage::BenchmarkScenes,
        "renderer.default_flip.stage_2.benchmark_scenes",
        true,
        true,
        true,
        false,
    ),
    RendererDefaultFlipStageDescriptor::new(
        RendererDefaultFlipStage::EditorLauncherViewports,
        "renderer.default_flip.stage_3.editor_launcher_viewports",
        true,
        true,
        true,
        true,
    ),
    RendererDefaultFlipStageDescriptor::new(
        RendererDefaultFlipStage::SelectedGameClientScenes,
        "renderer.default_flip.stage_4.selected_game_client_scenes",
        true,
        true,
        true,
        true,
    ),
    RendererDefaultFlipStageDescriptor::new(
        RendererDefaultFlipStage::GlobalDefault,
        "renderer.default_flip.stage_5.global_default",
        true,
        false,
        true,
        true,
    ),
    RendererDefaultFlipStageDescriptor::new(
        RendererDefaultFlipStage::LegacyDiagnosticOnly,
        "renderer.default_flip.stage_6.legacy_diagnostic_only",
        true,
        false,
        true,
        true,
    ),
];

pub const RENDERER_ACTIVE_DEFAULT_FLIP_STAGE: RendererDefaultFlipStage =
    RendererDefaultFlipStage::GlobalDefault;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RendererLegacyRetirementPolicy {
    pub legacy_product_backend_allowed: bool,
    pub legacy_diagnostic_backend_allowed: bool,
    pub product_retired_engine_ui_allowed: bool,
    pub product_cpu_native_ui_fallback_allowed: bool,
    pub duplicate_upload_systems_allowed: bool,
    pub duplicate_lighting_shadow_policy_allowed: bool,
    pub stale_transition_feature_flags_allowed: bool,
}

pub const RENDERER_LEGACY_RETIREMENT_POLICY: RendererLegacyRetirementPolicy =
    RendererLegacyRetirementPolicy {
        legacy_product_backend_allowed: false,
        legacy_diagnostic_backend_allowed: true,
        product_retired_engine_ui_allowed: FUN_RENDERER_UI_RUNTIME_POLICY
            .retired_engine_ui_runtime_product_allowed,
        product_cpu_native_ui_fallback_allowed: FUN_RENDERER_NATIVE_UI_RUNTIME_POLICY
            .cpu_on_paint_runtime_fallback_allowed,
        duplicate_upload_systems_allowed: false,
        duplicate_lighting_shadow_policy_allowed: false,
        stale_transition_feature_flags_allowed: false,
    };

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RendererDefaultFlipStatus {
    pub schema: &'static str,
    pub schema_version: u16,
    pub active_stage: RendererDefaultFlipStage,
    pub env: &'static str,
    pub auto_resolves_to: &'static str,
    pub default_backend: &'static str,
    pub legacy_product_backend_allowed: bool,
    pub legacy_diagnostic_backend_allowed: bool,
    pub product_retired_engine_ui_allowed: bool,
    pub product_cpu_native_ui_fallback_allowed: bool,
    pub renderer_core_owner: &'static str,
    pub bridge_owner: &'static str,
    pub scene_owner: &'static str,
    pub lighting_owner: &'static str,
    pub ai_owner: &'static str,
}

impl RendererDefaultFlipStatus {
    #[must_use]
    pub const fn current() -> Self {
        Self {
            schema: RENDERER_DEFAULT_FLIP_SCHEMA,
            schema_version: RENDERER_DEFAULT_FLIP_SCHEMA_VERSION,
            active_stage: RENDERER_ACTIVE_DEFAULT_FLIP_STAGE,
            env: FUN_RENDERER_RUNTIME_BACKEND_ENV,
            auto_resolves_to: FUN_RENDERER_CURRENT_AUTO_RESOLUTION.as_env_value(),
            default_backend: FunRendererRuntimeBackend::Fun.as_env_value(),
            legacy_product_backend_allowed: RENDERER_LEGACY_RETIREMENT_POLICY
                .legacy_product_backend_allowed,
            legacy_diagnostic_backend_allowed: RENDERER_LEGACY_RETIREMENT_POLICY
                .legacy_diagnostic_backend_allowed,
            product_retired_engine_ui_allowed: RENDERER_LEGACY_RETIREMENT_POLICY
                .product_retired_engine_ui_allowed,
            product_cpu_native_ui_fallback_allowed: RENDERER_LEGACY_RETIREMENT_POLICY
                .product_cpu_native_ui_fallback_allowed,
            renderer_core_owner: FUN_RENDERER_CRATE_NAME,
            bridge_owner: FUN_RENDER_DONOR_PACKAGE_NAME,
            scene_owner: FUN_RENDERER_SCENE_OWNER_PACKAGE_NAME,
            lighting_owner: fun_lux::FUN_LUX_CRATE_NAME,
            ai_owner: FUN_RENDERER_AI_OWNER_PACKAGE_NAME,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererDefaultFlipArtifact {
    pub schema: &'static str,
    pub schema_version: u16,
    pub path: Option<PathBuf>,
    pub content: String,
}

impl RendererDefaultFlipArtifact {
    #[must_use]
    pub fn from_status(status: &RendererDefaultFlipStatus) -> Self {
        let mut content = String::new();
        writeln!(content, "schema={}", status.schema).expect("write to string");
        writeln!(content, "schema_version={}", status.schema_version).expect("write to string");
        writeln!(content, "active_stage={}", status.active_stage.as_str())
            .expect("write to string");
        writeln!(content, "env={}", status.env).expect("write to string");
        writeln!(content, "auto_resolves_to={}", status.auto_resolves_to).expect("write to string");
        writeln!(content, "default_backend={}", status.default_backend).expect("write to string");
        writeln!(
            content,
            "legacy_product_backend_allowed={}",
            status.legacy_product_backend_allowed
        )
        .expect("write to string");
        writeln!(
            content,
            "legacy_diagnostic_backend_allowed={}",
            status.legacy_diagnostic_backend_allowed
        )
        .expect("write to string");
        writeln!(
            content,
            "product_retired_engine_ui_allowed={}",
            status.product_retired_engine_ui_allowed
        )
        .expect("write to string");
        writeln!(
            content,
            "product_cpu_native_ui_fallback_allowed={}",
            status.product_cpu_native_ui_fallback_allowed
        )
        .expect("write to string");
        writeln!(
            content,
            "renderer_core_owner={}",
            status.renderer_core_owner
        )
        .expect("write to string");
        writeln!(content, "bridge_owner={}", status.bridge_owner).expect("write to string");
        writeln!(content, "scene_owner={}", status.scene_owner).expect("write to string");
        writeln!(content, "lighting_owner={}", status.lighting_owner).expect("write to string");
        writeln!(content, "ai_owner={}", status.ai_owner).expect("write to string");

        for descriptor in RENDERER_DEFAULT_FLIP_STAGES {
            writeln!(
                content,
                "stage id={} stage={} default_backend={} legacy_product_path_allowed={} benchmark_artifact_required={} renderer_visible_handoff_required={}",
                descriptor.stable_id,
                descriptor.stage.as_str(),
                descriptor.default_backend,
                descriptor.legacy_product_path_allowed,
                descriptor.benchmark_artifact_required,
                descriptor.renderer_visible_handoff_required,
            )
            .expect("write to string");
        }

        Self {
            schema: RENDERER_DEFAULT_FLIP_SCHEMA,
            schema_version: RENDERER_DEFAULT_FLIP_SCHEMA_VERSION,
            path: None,
            content,
        }
    }

    pub fn write_to(mut self, path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, &self.content)?;
        self.path = Some(path.to_path_buf());
        Ok(self)
    }
}

#[must_use]
pub fn current_default_flip_status() -> RendererDefaultFlipStatus {
    RendererDefaultFlipStatus::current()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_flip_stage_catalog_is_ordered_and_reaches_legacy_retirement() {
        assert_eq!(
            RendererDefaultFlipStage::ORDER.len(),
            RENDERER_DEFAULT_FLIP_STAGE_COUNT
        );
        assert_eq!(
            RENDERER_DEFAULT_FLIP_STAGES[0].stage,
            RendererDefaultFlipStage::InternalMinimalScene
        );
        assert_eq!(
            RENDERER_DEFAULT_FLIP_STAGES[4].stage,
            RendererDefaultFlipStage::GlobalDefault
        );
        assert_eq!(
            RENDERER_DEFAULT_FLIP_STAGES[5].stage,
            RendererDefaultFlipStage::LegacyDiagnosticOnly
        );
        assert!(!RENDERER_DEFAULT_FLIP_STAGES[4].legacy_product_path_allowed);
        assert!(!RENDERER_DEFAULT_FLIP_STAGES[5].legacy_product_path_allowed);
    }

    #[test]
    fn current_default_flip_status_makes_fun_default_and_retires_legacy_product_path() {
        let status = current_default_flip_status();

        assert_eq!(status.active_stage, RendererDefaultFlipStage::GlobalDefault);
        assert_eq!(status.auto_resolves_to, "fun");
        assert_eq!(status.default_backend, "fun");
        assert!(!status.legacy_product_backend_allowed);
        assert!(status.legacy_diagnostic_backend_allowed);
        assert!(!status.product_retired_engine_ui_allowed);
        assert!(!status.product_cpu_native_ui_fallback_allowed);
        assert_eq!(status.renderer_core_owner, "fun_renderer");
        assert_eq!(status.lighting_owner, "fun_lux");
        assert_eq!(status.scene_owner, "fun-scene");
        assert_eq!(status.ai_owner, "fun-ai");
    }

    #[test]
    fn default_flip_artifact_records_stage_policy_and_owners() {
        let artifact = RendererDefaultFlipArtifact::from_status(&current_default_flip_status());

        assert!(
            artifact
                .content
                .contains("schema=fun.renderer.default_flip.v1")
        );
        assert!(artifact.content.contains("auto_resolves_to=fun"));
        assert!(artifact.content.contains("default_backend=fun"));
        assert!(
            artifact
                .content
                .contains("legacy_product_backend_allowed=false")
        );
        assert!(
            artifact
                .content
                .contains("product_retired_engine_ui_allowed=false")
        );
        assert!(
            artifact
                .content
                .contains("product_cpu_native_ui_fallback_allowed=false")
        );
        assert!(
            artifact
                .content
                .contains("renderer_core_owner=fun_renderer")
        );
        assert!(artifact.content.contains("lighting_owner=fun_lux"));

        if let Ok(path) = std::env::var(RENDERER_DEFAULT_FLIP_ARTIFACT_ENV) {
            artifact
                .write_to(path)
                .expect("default flip artifact should be writable");
        }
    }
}
