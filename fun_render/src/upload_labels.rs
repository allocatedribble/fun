use crate::{FunUploadBudgetClass, FunUploadSubsystem, UploadWriteLabel};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunUploadResourceKind {
    Buffer,
    Texture,
    Metadata,
    Readback,
    Unknown,
}

impl FunUploadResourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Buffer => "buffer",
            Self::Texture => "texture",
            Self::Metadata => "metadata",
            Self::Readback => "readback",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunUploadExpectedFrequency {
    PerFrame,
    PerDirtyRange,
    PerAssetLoad,
    OnResize,
    DiagnosticOnly,
}

impl FunUploadExpectedFrequency {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PerFrame => "per_frame",
            Self::PerDirtyRange => "per_dirty_range",
            Self::PerAssetLoad => "per_asset_load",
            Self::OnResize => "on_resize",
            Self::DiagnosticOnly => "diagnostic_only",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UploadLabelDescriptor {
    pub label: UploadWriteLabel,
    pub subsystem: FunUploadSubsystem,
    pub resource_kind: FunUploadResourceKind,
    pub expected_frequency: FunUploadExpectedFrequency,
    pub budget_class: FunUploadBudgetClass,
    pub hot_path_allowed: bool,
}

pub const UPLOAD_MESHLET_INSTANCE_RANGE: UploadWriteLabel =
    UploadWriteLabel("meshlet.instance.range");
pub const UPLOAD_INSTANCE_DIRTY_RANGE: UploadWriteLabel = UploadWriteLabel("instance.dirty_range");
pub const UPLOAD_MESHLET_MATERIAL_RANGE: UploadWriteLabel =
    UploadWriteLabel("meshlet.material.range");
pub const UPLOAD_VIEW_VISIBILITY: UploadWriteLabel = UploadWriteLabel("meshlet.view.visibility");
pub const UPLOAD_FRAME_CONSTANTS: UploadWriteLabel = UploadWriteLabel("frame.constants");
pub const UPLOAD_VIEW_CONSTANTS: UploadWriteLabel = UploadWriteLabel("view.constants");
pub const UPLOAD_WORLD_STREAM_STATIC_MESH: UploadWriteLabel =
    UploadWriteLabel("world_stream.static_mesh");
pub const UPLOAD_TEXTURE_DIRTY_RECT: UploadWriteLabel =
    UploadWriteLabel("texture_asset.dirty_rect");
pub const UPLOAD_CEF_CPU_FULL_FRAME: UploadWriteLabel =
    UploadWriteLabel("cef_ui.cpu_paint.full_frame");
pub const UPLOAD_CEF_CPU_DIRTY_RECT: UploadWriteLabel =
    UploadWriteLabel("cef_ui.cpu_paint.dirty_rect");
pub const UPLOAD_CEF_GPU_COPY_METADATA: UploadWriteLabel =
    UploadWriteLabel("cef_ui.gpu_copy.metadata");
pub const UPLOAD_CEF_METADATA: UploadWriteLabel = UploadWriteLabel("cef_ui.metadata");
pub const UPLOAD_CLOUD_PARAMS: UploadWriteLabel = UploadWriteLabel("clouds.params");
pub const UPLOAD_SOLARI_PARAMS: UploadWriteLabel = UploadWriteLabel("solari.params");
pub const UPLOAD_DLSS_CONSTANTS: UploadWriteLabel = UploadWriteLabel("dlss.constants");
pub const UPLOAD_DLSS_PARAMS: UploadWriteLabel = UploadWriteLabel("dlss.params");

pub const FUN_UPLOAD_LABELS: &[UploadLabelDescriptor] = &[
    UploadLabelDescriptor {
        label: UPLOAD_MESHLET_INSTANCE_RANGE,
        subsystem: FunUploadSubsystem::MeshletInstance,
        resource_kind: FunUploadResourceKind::Buffer,
        expected_frequency: FunUploadExpectedFrequency::PerDirtyRange,
        budget_class: FunUploadBudgetClass::SmallBuffer,
        hot_path_allowed: true,
    },
    UploadLabelDescriptor {
        label: UPLOAD_INSTANCE_DIRTY_RANGE,
        subsystem: FunUploadSubsystem::MeshletInstance,
        resource_kind: FunUploadResourceKind::Buffer,
        expected_frequency: FunUploadExpectedFrequency::PerDirtyRange,
        budget_class: FunUploadBudgetClass::LargeBuffer,
        hot_path_allowed: true,
    },
    UploadLabelDescriptor {
        label: UPLOAD_MESHLET_MATERIAL_RANGE,
        subsystem: FunUploadSubsystem::MeshletMaterial,
        resource_kind: FunUploadResourceKind::Buffer,
        expected_frequency: FunUploadExpectedFrequency::PerDirtyRange,
        budget_class: FunUploadBudgetClass::SmallBuffer,
        hot_path_allowed: true,
    },
    UploadLabelDescriptor {
        label: UPLOAD_VIEW_VISIBILITY,
        subsystem: FunUploadSubsystem::MeshletVisibility,
        resource_kind: FunUploadResourceKind::Buffer,
        expected_frequency: FunUploadExpectedFrequency::PerFrame,
        budget_class: FunUploadBudgetClass::SmallBuffer,
        hot_path_allowed: true,
    },
    UploadLabelDescriptor {
        label: UPLOAD_FRAME_CONSTANTS,
        subsystem: FunUploadSubsystem::MaterialUniform,
        resource_kind: FunUploadResourceKind::Buffer,
        expected_frequency: FunUploadExpectedFrequency::PerFrame,
        budget_class: FunUploadBudgetClass::SmallBuffer,
        hot_path_allowed: true,
    },
    UploadLabelDescriptor {
        label: UPLOAD_VIEW_CONSTANTS,
        subsystem: FunUploadSubsystem::MeshletVisibility,
        resource_kind: FunUploadResourceKind::Buffer,
        expected_frequency: FunUploadExpectedFrequency::PerFrame,
        budget_class: FunUploadBudgetClass::SmallBuffer,
        hot_path_allowed: true,
    },
    UploadLabelDescriptor {
        label: UPLOAD_WORLD_STREAM_STATIC_MESH,
        subsystem: FunUploadSubsystem::WorldStream,
        resource_kind: FunUploadResourceKind::Buffer,
        expected_frequency: FunUploadExpectedFrequency::PerAssetLoad,
        budget_class: FunUploadBudgetClass::LargeBuffer,
        hot_path_allowed: false,
    },
    UploadLabelDescriptor {
        label: UPLOAD_TEXTURE_DIRTY_RECT,
        subsystem: FunUploadSubsystem::TextureAsset,
        resource_kind: FunUploadResourceKind::Texture,
        expected_frequency: FunUploadExpectedFrequency::PerDirtyRange,
        budget_class: FunUploadBudgetClass::Texture,
        hot_path_allowed: true,
    },
    UploadLabelDescriptor {
        label: UPLOAD_CEF_CPU_FULL_FRAME,
        subsystem: FunUploadSubsystem::CefCpuPaint,
        resource_kind: FunUploadResourceKind::Texture,
        expected_frequency: FunUploadExpectedFrequency::OnResize,
        budget_class: FunUploadBudgetClass::Cef,
        hot_path_allowed: false,
    },
    UploadLabelDescriptor {
        label: UPLOAD_CEF_CPU_DIRTY_RECT,
        subsystem: FunUploadSubsystem::CefCpuPaint,
        resource_kind: FunUploadResourceKind::Texture,
        expected_frequency: FunUploadExpectedFrequency::PerDirtyRange,
        budget_class: FunUploadBudgetClass::Cef,
        hot_path_allowed: true,
    },
    UploadLabelDescriptor {
        label: UPLOAD_CEF_METADATA,
        subsystem: FunUploadSubsystem::CefCpuPaint,
        resource_kind: FunUploadResourceKind::Metadata,
        expected_frequency: FunUploadExpectedFrequency::PerDirtyRange,
        budget_class: FunUploadBudgetClass::Cef,
        hot_path_allowed: true,
    },
    UploadLabelDescriptor {
        label: UPLOAD_CEF_GPU_COPY_METADATA,
        subsystem: FunUploadSubsystem::CefGpuInterop,
        resource_kind: FunUploadResourceKind::Metadata,
        expected_frequency: FunUploadExpectedFrequency::PerFrame,
        budget_class: FunUploadBudgetClass::Cef,
        hot_path_allowed: true,
    },
    UploadLabelDescriptor {
        label: UPLOAD_CLOUD_PARAMS,
        subsystem: FunUploadSubsystem::Clouds,
        resource_kind: FunUploadResourceKind::Buffer,
        expected_frequency: FunUploadExpectedFrequency::PerFrame,
        budget_class: FunUploadBudgetClass::SmallBuffer,
        hot_path_allowed: true,
    },
    UploadLabelDescriptor {
        label: UPLOAD_SOLARI_PARAMS,
        subsystem: FunUploadSubsystem::Solari,
        resource_kind: FunUploadResourceKind::Buffer,
        expected_frequency: FunUploadExpectedFrequency::PerFrame,
        budget_class: FunUploadBudgetClass::SmallBuffer,
        hot_path_allowed: true,
    },
    UploadLabelDescriptor {
        label: UPLOAD_DLSS_CONSTANTS,
        subsystem: FunUploadSubsystem::Dlss,
        resource_kind: FunUploadResourceKind::Buffer,
        expected_frequency: FunUploadExpectedFrequency::PerFrame,
        budget_class: FunUploadBudgetClass::SmallBuffer,
        hot_path_allowed: true,
    },
    UploadLabelDescriptor {
        label: UPLOAD_DLSS_PARAMS,
        subsystem: FunUploadSubsystem::Dlss,
        resource_kind: FunUploadResourceKind::Buffer,
        expected_frequency: FunUploadExpectedFrequency::PerFrame,
        budget_class: FunUploadBudgetClass::SmallBuffer,
        hot_path_allowed: true,
    },
];

pub fn upload_label_descriptor(label: UploadWriteLabel) -> Option<&'static UploadLabelDescriptor> {
    FUN_UPLOAD_LABELS
        .iter()
        .find(|descriptor| descriptor.label == label)
}

pub fn upload_label_descriptor_or_default(label: UploadWriteLabel) -> UploadLabelDescriptor {
    upload_label_descriptor(label)
        .copied()
        .unwrap_or(UploadLabelDescriptor {
            label,
            subsystem: FunUploadSubsystem::DebugOverlay,
            resource_kind: FunUploadResourceKind::Unknown,
            expected_frequency: FunUploadExpectedFrequency::DiagnosticOnly,
            budget_class: FunUploadBudgetClass::SmallBuffer,
            hot_path_allowed: false,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cef_cpu_labels_are_not_generic_texture_asset_labels() {
        let full = upload_label_descriptor(UPLOAD_CEF_CPU_FULL_FRAME).expect("cef full label");
        let dirty = upload_label_descriptor(UPLOAD_CEF_CPU_DIRTY_RECT).expect("cef dirty label");

        assert_eq!(full.subsystem, FunUploadSubsystem::CefCpuPaint);
        assert_eq!(dirty.subsystem, FunUploadSubsystem::CefCpuPaint);
        assert_eq!(full.budget_class, FunUploadBudgetClass::Cef);
        assert_eq!(dirty.budget_class, FunUploadBudgetClass::Cef);
    }

    #[test]
    fn hot_path_constant_and_dirty_range_labels_are_typed() {
        let frame = upload_label_descriptor(UPLOAD_FRAME_CONSTANTS).expect("frame constants label");
        let view = upload_label_descriptor(UPLOAD_VIEW_CONSTANTS).expect("view constants label");
        let instance =
            upload_label_descriptor(UPLOAD_INSTANCE_DIRTY_RANGE).expect("instance range label");
        let cef_metadata =
            upload_label_descriptor(UPLOAD_CEF_METADATA).expect("cef metadata label");

        assert_eq!(frame.budget_class, FunUploadBudgetClass::SmallBuffer);
        assert_eq!(
            view.expected_frequency,
            FunUploadExpectedFrequency::PerFrame
        );
        assert_eq!(instance.budget_class, FunUploadBudgetClass::LargeBuffer);
        assert_eq!(cef_metadata.subsystem, FunUploadSubsystem::CefCpuPaint);
        assert!(instance.hot_path_allowed);
    }
}
