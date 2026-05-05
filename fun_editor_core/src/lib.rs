#![forbid(unsafe_code)]

use serde::Serialize;

pub const PROJECT_OPEN: &str = "project.open";
pub const PROJECT_EDIT_OPEN: &str = "project.edit.open";
pub const PROJECTS_AUTHORIZED_LIST: &str = "projects.authorized.list";
pub const ENTITY_STREAM_OPEN: &str = "entity_stream.open";
pub const LIVE_ENTITY_STREAM_OPEN: &str = "live_entity_stream.open";
pub const PREVIEW_RENDERER_ENSURE: &str = "preview.renderer.ensure";
pub const PREVIEW_RENDERER_RESIZE: &str = "preview.renderer.resize";
pub const PREVIEW_RENDERER_FRAME_GET: &str = "preview.renderer.frame.get";
pub const PREVIEW_RENDERER_SCENE_SET: &str = "preview.renderer.scene.set";
pub const PREVIEW_RENDERER_STATUS_GET: &str = "preview.renderer.status.get";
pub const MATERIAL_SHADER_LIST: &str = "material.shader.list";
pub const MATERIAL_SHADER_LOAD: &str = "material.shader.load";
pub const MATERIAL_SHADER_SAVE: &str = "material.shader.save";
pub const RUNTIME_DIAGNOSTICS_LIST: &str = "runtime.diagnostics.list";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FunEditorCoreService {
    ProjectIndex,
    FunSceneIndex,
    MaterialShader,
    EntityStream,
    PreviewRenderer,
    RuntimeDiagnostics,
}

impl FunEditorCoreService {
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::ProjectIndex => "project_index",
            Self::FunSceneIndex => "fun_scene_index",
            Self::MaterialShader => "material_shader",
            Self::EntityStream => "entity_stream",
            Self::PreviewRenderer => "preview_renderer",
            Self::RuntimeDiagnostics => "runtime_diagnostics",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FunEditorCommandCategory {
    Project,
    Entity,
    Preview,
    Material,
    RuntimeDiagnostics,
}

impl FunEditorCommandCategory {
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Entity => "entity",
            Self::Preview => "preview",
            Self::Material => "material",
            Self::RuntimeDiagnostics => "runtime_diagnostics",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct FunEditorCommandDescriptor {
    pub id: &'static str,
    pub category: FunEditorCommandCategory,
    pub service: FunEditorCoreService,
    pub summary: &'static str,
}

pub const FUN_EDITOR_COMMANDS: &[FunEditorCommandDescriptor] = &[
    FunEditorCommandDescriptor {
        id: PROJECTS_AUTHORIZED_LIST,
        category: FunEditorCommandCategory::Project,
        service: FunEditorCoreService::ProjectIndex,
        summary: "Lists projects the local host may reveal to the UI.",
    },
    FunEditorCommandDescriptor {
        id: PROJECT_EDIT_OPEN,
        category: FunEditorCommandCategory::Project,
        service: FunEditorCoreService::ProjectIndex,
        summary: "Activates editor mode for an authorized project.",
    },
    FunEditorCommandDescriptor {
        id: PROJECT_OPEN,
        category: FunEditorCommandCategory::Project,
        service: FunEditorCoreService::ProjectIndex,
        summary: "Indexes and opens a project inside the Rust-owned host.",
    },
    FunEditorCommandDescriptor {
        id: ENTITY_STREAM_OPEN,
        category: FunEditorCommandCategory::Entity,
        service: FunEditorCoreService::EntityStream,
        summary: "Creates a bounded project/source entity stream cursor.",
    },
    FunEditorCommandDescriptor {
        id: LIVE_ENTITY_STREAM_OPEN,
        category: FunEditorCommandCategory::Entity,
        service: FunEditorCoreService::EntityStream,
        summary: "Creates a bounded live-runtime entity stream cursor.",
    },
    FunEditorCommandDescriptor {
        id: PREVIEW_RENDERER_ENSURE,
        category: FunEditorCommandCategory::Preview,
        service: FunEditorCoreService::PreviewRenderer,
        summary: "Ensures the editor-owned preview service is ready.",
    },
    FunEditorCommandDescriptor {
        id: PREVIEW_RENDERER_RESIZE,
        category: FunEditorCommandCategory::Preview,
        service: FunEditorCoreService::PreviewRenderer,
        summary: "Resizes the editor preview surface without spawning a runtime.",
    },
    FunEditorCommandDescriptor {
        id: PREVIEW_RENDERER_FRAME_GET,
        category: FunEditorCommandCategory::Preview,
        service: FunEditorCoreService::PreviewRenderer,
        summary: "Reads the latest bounded preview frame summary.",
    },
    FunEditorCommandDescriptor {
        id: PREVIEW_RENDERER_SCENE_SET,
        category: FunEditorCommandCategory::Preview,
        service: FunEditorCoreService::PreviewRenderer,
        summary: "Selects a scene manifest for the editor-owned preview.",
    },
    FunEditorCommandDescriptor {
        id: PREVIEW_RENDERER_STATUS_GET,
        category: FunEditorCommandCategory::Preview,
        service: FunEditorCoreService::PreviewRenderer,
        summary: "Returns current-client preview service status.",
    },
    FunEditorCommandDescriptor {
        id: MATERIAL_SHADER_LIST,
        category: FunEditorCommandCategory::Material,
        service: FunEditorCoreService::MaterialShader,
        summary: "Lists material shader documents visible to the editor.",
    },
    FunEditorCommandDescriptor {
        id: MATERIAL_SHADER_LOAD,
        category: FunEditorCommandCategory::Material,
        service: FunEditorCoreService::MaterialShader,
        summary: "Loads one material shader document for editing.",
    },
    FunEditorCommandDescriptor {
        id: MATERIAL_SHADER_SAVE,
        category: FunEditorCommandCategory::Material,
        service: FunEditorCoreService::MaterialShader,
        summary: "Saves one authorized material shader document.",
    },
    FunEditorCommandDescriptor {
        id: RUNTIME_DIAGNOSTICS_LIST,
        category: FunEditorCommandCategory::RuntimeDiagnostics,
        service: FunEditorCoreService::RuntimeDiagnostics,
        summary: "Lists editor-visible runtime diagnostic streams.",
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct FunEditorServiceDescriptor {
    pub service: FunEditorCoreService,
    pub rust_owner: &'static str,
    pub command_ids: &'static [&'static str],
}

pub const FUN_EDITOR_SERVICES: &[FunEditorServiceDescriptor] = &[
    FunEditorServiceDescriptor {
        service: FunEditorCoreService::ProjectIndex,
        rust_owner: "fun_editor_core",
        command_ids: &[PROJECTS_AUTHORIZED_LIST, PROJECT_EDIT_OPEN, PROJECT_OPEN],
    },
    FunEditorServiceDescriptor {
        service: FunEditorCoreService::FunSceneIndex,
        rust_owner: "fun_editor_core",
        command_ids: &[PROJECT_OPEN],
    },
    FunEditorServiceDescriptor {
        service: FunEditorCoreService::MaterialShader,
        rust_owner: "fun_editor_core",
        command_ids: &[
            MATERIAL_SHADER_LIST,
            MATERIAL_SHADER_LOAD,
            MATERIAL_SHADER_SAVE,
        ],
    },
    FunEditorServiceDescriptor {
        service: FunEditorCoreService::EntityStream,
        rust_owner: "fun_editor_core",
        command_ids: &[ENTITY_STREAM_OPEN, LIVE_ENTITY_STREAM_OPEN],
    },
    FunEditorServiceDescriptor {
        service: FunEditorCoreService::PreviewRenderer,
        rust_owner: "fun_editor_core",
        command_ids: &[
            PREVIEW_RENDERER_ENSURE,
            PREVIEW_RENDERER_RESIZE,
            PREVIEW_RENDERER_FRAME_GET,
            PREVIEW_RENDERER_SCENE_SET,
            PREVIEW_RENDERER_STATUS_GET,
        ],
    },
    FunEditorServiceDescriptor {
        service: FunEditorCoreService::RuntimeDiagnostics,
        rust_owner: "fun_editor_core",
        command_ids: &[RUNTIME_DIAGNOSTICS_LIST],
    },
];

#[must_use]
pub fn editor_command_descriptor(id: &str) -> Option<&'static FunEditorCommandDescriptor> {
    FUN_EDITOR_COMMANDS
        .iter()
        .find(|descriptor| descriptor.id == id)
}

#[must_use]
pub fn editor_service_descriptor(
    service: FunEditorCoreService,
) -> Option<&'static FunEditorServiceDescriptor> {
    FUN_EDITOR_SERVICES
        .iter()
        .find(|descriptor| descriptor.service == service)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_core_keeps_ported_command_ids_stable() {
        for id in [
            PROJECTS_AUTHORIZED_LIST,
            PROJECT_EDIT_OPEN,
            PROJECT_OPEN,
            ENTITY_STREAM_OPEN,
            PREVIEW_RENDERER_ENSURE,
            RUNTIME_DIAGNOSTICS_LIST,
        ] {
            assert!(
                editor_command_descriptor(id).is_some(),
                "missing editor command `{id}`"
            );
        }
    }

    #[test]
    fn preview_service_does_not_claim_process_embedding_commands() {
        let preview = editor_service_descriptor(FunEditorCoreService::PreviewRenderer)
            .expect("preview service");

        assert!(preview.command_ids.contains(&PREVIEW_RENDERER_ENSURE));
        assert!(
            !preview
                .command_ids
                .iter()
                .any(|id| id.contains("window") || id.contains("embed"))
        );
    }
}
