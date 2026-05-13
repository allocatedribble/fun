use fun_ecs::{
    entity::Entity,
    prelude::{Component, Resource},
    world::World,
};
use serde::{Deserialize, Serialize};

use crate::{EditorSelection, GameplaySalient, LuxLight, ViewportRenderPolicy};

pub const EDITOR_OPERATION_SCHEMA_VERSION: u16 = 1;
pub const EDITOR_OPERATION_MAX_ENTITY_URI_BYTES: usize = 160;
pub const EDITOR_OPERATION_MAX_ASSET_URI_BYTES: usize = 192;
pub const EDITOR_OPERATION_OUTCOME_CAPACITY: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorIntegrationPolicy {
    pub native_ui_svelte_edits_typed_scene_data: bool,
    pub host_applies_operations_through_fun_scene: bool,
    pub runtime_external_ui_allowed: bool,
    pub native_ui_gpu_only_required: bool,
    pub overlays_are_renderer_debug_or_native_ui_late_composite: bool,
    pub fun_assets_are_declarative: bool,
    pub dynamic_rust_expressions_are_macro_only: bool,
    pub observers_drive_normal_ecs_changes_only: bool,
    pub observers_allowed_on_renderer_hot_path: bool,
}

impl EditorIntegrationPolicy {
    pub const DEFAULT: Self = Self {
        native_ui_svelte_edits_typed_scene_data: true,
        host_applies_operations_through_fun_scene: true,
        runtime_external_ui_allowed: false,
        native_ui_gpu_only_required: true,
        overlays_are_renderer_debug_or_native_ui_late_composite: true,
        fun_assets_are_declarative: true,
        dynamic_rust_expressions_are_macro_only: true,
        observers_drive_normal_ecs_changes_only: true,
        observers_allowed_on_renderer_hot_path: false,
    };
}

impl Default for EditorIntegrationPolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EditorOperationKind {
    SpawnSceneComponent,
    PatchComponentField,
    DeleteEntity,
    DuplicateEntity,
    AttachChildScene,
    AdjustLight,
    AdjustRenderPolicy,
    MarkSelectionSalience,
}

impl EditorOperationKind {
    pub const ALL: [Self; 8] = [
        Self::SpawnSceneComponent,
        Self::PatchComponentField,
        Self::DeleteEntity,
        Self::DuplicateEntity,
        Self::AttachChildScene,
        Self::AdjustLight,
        Self::AdjustRenderPolicy,
        Self::MarkSelectionSalience,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SpawnSceneComponent => "spawn_scene_component",
            Self::PatchComponentField => "patch_component",
            Self::DeleteEntity => "delete_entity",
            Self::DuplicateEntity => "duplicate_entity",
            Self::AttachChildScene => "attach_child_scene",
            Self::AdjustLight => "adjust_light",
            Self::AdjustRenderPolicy => "adjust_render_policy",
            Self::MarkSelectionSalience => "mark_selection_salience",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorOperationDescriptor {
    pub kind: EditorOperationKind,
    pub stable_id: &'static str,
    pub mutates_ecs: bool,
    pub requires_scene_resolver: bool,
}

pub const EDITOR_OPERATION_DESCRIPTORS: [EditorOperationDescriptor; 8] = [
    EditorOperationDescriptor {
        kind: EditorOperationKind::SpawnSceneComponent,
        stable_id: "spawn_scene_component",
        mutates_ecs: true,
        requires_scene_resolver: true,
    },
    EditorOperationDescriptor {
        kind: EditorOperationKind::PatchComponentField,
        stable_id: "patch_component",
        mutates_ecs: true,
        requires_scene_resolver: false,
    },
    EditorOperationDescriptor {
        kind: EditorOperationKind::DeleteEntity,
        stable_id: "delete_entity",
        mutates_ecs: true,
        requires_scene_resolver: false,
    },
    EditorOperationDescriptor {
        kind: EditorOperationKind::DuplicateEntity,
        stable_id: "duplicate_entity",
        mutates_ecs: true,
        requires_scene_resolver: true,
    },
    EditorOperationDescriptor {
        kind: EditorOperationKind::AttachChildScene,
        stable_id: "attach_child_scene",
        mutates_ecs: true,
        requires_scene_resolver: true,
    },
    EditorOperationDescriptor {
        kind: EditorOperationKind::AdjustLight,
        stable_id: "adjust_light",
        mutates_ecs: true,
        requires_scene_resolver: false,
    },
    EditorOperationDescriptor {
        kind: EditorOperationKind::AdjustRenderPolicy,
        stable_id: "adjust_render_policy",
        mutates_ecs: true,
        requires_scene_resolver: false,
    },
    EditorOperationDescriptor {
        kind: EditorOperationKind::MarkSelectionSalience,
        stable_id: "mark_selection_salience",
        mutates_ecs: true,
        requires_scene_resolver: false,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditorOverlaySurface {
    RendererDebugPrimitive,
    NativeUiLateComposite,
}

impl EditorOverlaySurface {
    pub const ALL: [Self; 2] = [Self::RendererDebugPrimitive, Self::NativeUiLateComposite];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RendererDebugPrimitive => "renderer_debug_primitive",
            Self::NativeUiLateComposite => "native_ui_late_composite",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditorObserverTriggerKind {
    SelectionChanged,
    GizmoDragged,
    LightChanged,
    PrefabInstantiated,
    TriggerVolumeEdited,
}

impl EditorObserverTriggerKind {
    pub const ALL: [Self; 5] = [
        Self::SelectionChanged,
        Self::GizmoDragged,
        Self::LightChanged,
        Self::PrefabInstantiated,
        Self::TriggerVolumeEdited,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SelectionChanged => "selection_changed",
            Self::GizmoDragged => "gizmo_dragged",
            Self::LightChanged => "light_changed",
            Self::PrefabInstantiated => "prefab_instantiated",
            Self::TriggerVolumeEdited => "trigger_volume_edited",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorObserverDescriptor {
    pub trigger: EditorObserverTriggerKind,
    pub produces_normal_ecs_changes: bool,
    pub renderer_hot_path_allowed: bool,
}

pub const EDITOR_OBSERVER_DESCRIPTORS: [EditorObserverDescriptor; 5] = [
    EditorObserverDescriptor {
        trigger: EditorObserverTriggerKind::SelectionChanged,
        produces_normal_ecs_changes: true,
        renderer_hot_path_allowed: false,
    },
    EditorObserverDescriptor {
        trigger: EditorObserverTriggerKind::GizmoDragged,
        produces_normal_ecs_changes: true,
        renderer_hot_path_allowed: false,
    },
    EditorObserverDescriptor {
        trigger: EditorObserverTriggerKind::LightChanged,
        produces_normal_ecs_changes: true,
        renderer_hot_path_allowed: false,
    },
    EditorObserverDescriptor {
        trigger: EditorObserverTriggerKind::PrefabInstantiated,
        produces_normal_ecs_changes: true,
        renderer_hot_path_allowed: false,
    },
    EditorObserverDescriptor {
        trigger: EditorObserverTriggerKind::TriggerVolumeEdited,
        produces_normal_ecs_changes: true,
        renderer_hot_path_allowed: false,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EditorComponentKind {
    #[serde(alias = "renderable")]
    Renderable,
    #[serde(alias = "virtual_geometry_authoring")]
    VirtualGeometryAuthoring,
    #[serde(alias = "renderer_bounds")]
    RendererBounds,
    #[serde(alias = "lux_light")]
    LuxLight,
    #[serde(alias = "lux_emissive")]
    LuxEmissive,
    #[serde(alias = "lux_gi_participant")]
    LuxGiParticipant,
    #[serde(alias = "virtual_shadow_caster")]
    VirtualShadowCaster,
    #[serde(alias = "virtual_shadow_receiver")]
    VirtualShadowReceiver,
    #[serde(alias = "native_ui_surface")]
    NativeUiSurface,
    #[serde(alias = "upscale_policy")]
    UpscalePolicy,
    #[serde(alias = "viewport_render_policy")]
    ViewportRenderPolicy,
    #[serde(alias = "gameplay_salient")]
    GameplaySalient,
    #[serde(alias = "editor_selection")]
    EditorSelection,
    #[serde(alias = "streaming_priority")]
    StreamingPriority,
    #[serde(alias = "temporal_instability")]
    TemporalInstability,
}

impl EditorComponentKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Renderable => "Renderable",
            Self::VirtualGeometryAuthoring => "VirtualGeometryAuthoring",
            Self::RendererBounds => "RendererBounds",
            Self::LuxLight => "LuxLight",
            Self::LuxEmissive => "LuxEmissive",
            Self::LuxGiParticipant => "LuxGiParticipant",
            Self::VirtualShadowCaster => "VirtualShadowCaster",
            Self::VirtualShadowReceiver => "VirtualShadowReceiver",
            Self::NativeUiSurface => "NativeUiSurface",
            Self::UpscalePolicy => "UpscalePolicy",
            Self::ViewportRenderPolicy => "ViewportRenderPolicy",
            Self::GameplaySalient => "GameplaySalient",
            Self::EditorSelection => "EditorSelection",
            Self::StreamingPriority => "StreamingPriority",
            Self::TemporalInstability => "TemporalInstability",
        }
    }

    #[must_use]
    pub const fn accepts_field(self, field: EditorComponentField) -> bool {
        match self {
            Self::Renderable => matches!(
                field,
                EditorComponentField::Geometry
                    | EditorComponentField::Material
                    | EditorComponentField::Flags
            ),
            Self::VirtualGeometryAuthoring => matches!(
                field,
                EditorComponentField::Mode
                    | EditorComponentField::PagePriority
                    | EditorComponentField::DynamicPolicy
            ),
            Self::RendererBounds => matches!(
                field,
                EditorComponentField::LocalBounds | EditorComponentField::StreamingRadius
            ),
            Self::LuxLight => matches!(
                field,
                EditorComponentField::Kind
                    | EditorComponentField::Color
                    | EditorComponentField::IntensityLux
                    | EditorComponentField::Range
                    | EditorComponentField::ShadowPolicy
                    | EditorComponentField::Importance
            ),
            Self::LuxEmissive => matches!(
                field,
                EditorComponentField::Luminance | EditorComponentField::CandidatePolicy
            ),
            Self::LuxGiParticipant => matches!(
                field,
                EditorComponentField::BouncePolicy | EditorComponentField::CachePolicy
            ),
            Self::VirtualShadowCaster => matches!(
                field,
                EditorComponentField::ShadowCasterPolicy | EditorComponentField::ShadowInvalidation
            ),
            Self::VirtualShadowReceiver => matches!(
                field,
                EditorComponentField::ShadowReceiverPriority
                    | EditorComponentField::ShadowFilterPolicy
            ),
            Self::NativeUiSurface => matches!(field, EditorComponentField::Route),
            Self::UpscalePolicy => matches!(
                field,
                EditorComponentField::SuperResolution
                    | EditorComponentField::FrameGeneration
                    | EditorComponentField::HudlessRequired
            ),
            Self::ViewportRenderPolicy => matches!(
                field,
                EditorComponentField::Quality
                    | EditorComponentField::Latency
                    | EditorComponentField::EditorMode
            ),
            Self::GameplaySalient | Self::StreamingPriority => {
                matches!(field, EditorComponentField::Score)
            }
            Self::EditorSelection => matches!(
                field,
                EditorComponentField::Rank | EditorComponentField::Salience
            ),
            Self::TemporalInstability => matches!(
                field,
                EditorComponentField::Motion | EditorComponentField::Topology
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EditorComponentField {
    #[serde(alias = "geometry")]
    Geometry,
    #[serde(alias = "material")]
    Material,
    #[serde(alias = "flags")]
    Flags,
    #[serde(alias = "mode")]
    Mode,
    #[serde(alias = "page_priority")]
    PagePriority,
    #[serde(alias = "dynamic_policy")]
    DynamicPolicy,
    #[serde(alias = "local_bounds")]
    LocalBounds,
    #[serde(alias = "streaming_radius")]
    StreamingRadius,
    #[serde(alias = "kind")]
    Kind,
    #[serde(alias = "color")]
    Color,
    #[serde(alias = "intensity_lux")]
    IntensityLux,
    #[serde(alias = "range")]
    Range,
    #[serde(alias = "shadow_policy")]
    ShadowPolicy,
    #[serde(alias = "importance")]
    Importance,
    #[serde(alias = "luminance")]
    Luminance,
    #[serde(alias = "candidate_policy")]
    CandidatePolicy,
    #[serde(alias = "bounce_policy")]
    BouncePolicy,
    #[serde(alias = "cache_policy")]
    CachePolicy,
    #[serde(alias = "shadow_caster_policy")]
    ShadowCasterPolicy,
    #[serde(alias = "shadow_invalidation")]
    ShadowInvalidation,
    #[serde(alias = "shadow_receiver_priority")]
    ShadowReceiverPriority,
    #[serde(alias = "shadow_filter_policy")]
    ShadowFilterPolicy,
    #[serde(alias = "route")]
    Route,
    #[serde(alias = "super_resolution")]
    SuperResolution,
    #[serde(alias = "frame_generation")]
    FrameGeneration,
    #[serde(alias = "hudless_required")]
    HudlessRequired,
    #[serde(alias = "quality")]
    Quality,
    #[serde(alias = "latency")]
    Latency,
    #[serde(alias = "editor_mode")]
    EditorMode,
    #[serde(alias = "score")]
    Score,
    #[serde(alias = "rank")]
    Rank,
    #[serde(alias = "salience")]
    Salience,
    #[serde(alias = "motion")]
    Motion,
    #[serde(alias = "topology")]
    Topology,
}

impl EditorComponentField {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Geometry => "geometry",
            Self::Material => "material",
            Self::Flags => "flags",
            Self::Mode => "mode",
            Self::PagePriority => "page_priority",
            Self::DynamicPolicy => "dynamic_policy",
            Self::LocalBounds => "local_bounds",
            Self::StreamingRadius => "streaming_radius",
            Self::Kind => "kind",
            Self::Color => "color",
            Self::IntensityLux => "intensity_lux",
            Self::Range => "range",
            Self::ShadowPolicy => "shadow_policy",
            Self::Importance => "importance",
            Self::Luminance => "luminance",
            Self::CandidatePolicy => "candidate_policy",
            Self::BouncePolicy => "bounce_policy",
            Self::CachePolicy => "cache_policy",
            Self::ShadowCasterPolicy => "shadow_caster_policy",
            Self::ShadowInvalidation => "shadow_invalidation",
            Self::ShadowReceiverPriority => "shadow_receiver_priority",
            Self::ShadowFilterPolicy => "shadow_filter_policy",
            Self::Route => "route",
            Self::SuperResolution => "super_resolution",
            Self::FrameGeneration => "frame_generation",
            Self::HudlessRequired => "hudless_required",
            Self::Quality => "quality",
            Self::Latency => "latency",
            Self::EditorMode => "editor_mode",
            Self::Score => "score",
            Self::Rank => "rank",
            Self::Salience => "salience",
            Self::Motion => "motion",
            Self::Topology => "topology",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EditorFieldValue {
    Bool(bool),
    U8(u8),
    U16(u16),
    U32(u32),
    F32(f32),
    Vec3([f32; 3]),
    Token(String),
}

impl EditorFieldValue {
    #[must_use]
    pub const fn as_u8(&self) -> Option<u8> {
        match self {
            Self::U8(value) => Some(*value),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_u16(&self) -> Option<u16> {
        match self {
            Self::U8(value) => Some(*value as u16),
            Self::U16(value) => Some(*value),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            Self::U8(value) => Some(f32::from(*value)),
            Self::U16(value) => Some(f32::from(*value)),
            Self::U32(value) => Some(*value as f32),
            Self::F32(value) => Some(*value),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditorEntityRef {
    pub uri: String,
}

impl EditorEntityRef {
    #[must_use]
    pub fn new(uri: impl Into<String>) -> Self {
        Self { uri: uri.into() }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.uri.as_str()
    }

    pub fn validate(&self) -> Result<(), EditorOperationValidationError> {
        validate_editor_uri(
            self.uri.as_str(),
            "scene://",
            EDITOR_OPERATION_MAX_ENTITY_URI_BYTES,
            EditorOperationValidationError::MissingEntityUri,
            EditorOperationValidationError::InvalidEntityUri,
            EditorOperationValidationError::EntityUriTooLong,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneAssetRef {
    pub uri: String,
}

impl SceneAssetRef {
    #[must_use]
    pub fn new(uri: impl Into<String>) -> Self {
        Self { uri: uri.into() }
    }

    pub fn validate(&self) -> Result<(), EditorOperationValidationError> {
        if self.uri.ends_with(".fun") {
            return validate_common_uri_bytes(
                self.uri.as_str(),
                EDITOR_OPERATION_MAX_ASSET_URI_BYTES,
                EditorOperationValidationError::MissingAssetUri,
                EditorOperationValidationError::InvalidAssetUri,
                EditorOperationValidationError::AssetUriTooLong,
            );
        }
        if self.uri.starts_with("scene://") || self.uri.starts_with("asset://") {
            return validate_common_uri_bytes(
                self.uri.as_str(),
                EDITOR_OPERATION_MAX_ASSET_URI_BYTES,
                EditorOperationValidationError::MissingAssetUri,
                EditorOperationValidationError::InvalidAssetUri,
                EditorOperationValidationError::AssetUriTooLong,
            );
        }
        Err(EditorOperationValidationError::InvalidAssetUri)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditorComponentPatch {
    pub field: EditorComponentField,
    pub value: EditorFieldValue,
}

impl EditorComponentPatch {
    pub fn validate_for(
        &self,
        component: EditorComponentKind,
    ) -> Result<(), EditorOperationValidationError> {
        if !component.accepts_field(self.field) {
            return Err(EditorOperationValidationError::FieldNotValidForComponent);
        }
        if !field_value_is_valid(self.field, &self.value) {
            return Err(EditorOperationValidationError::ValueNotValidForField);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", deny_unknown_fields)]
pub enum EditorOperation {
    #[serde(rename = "spawn_scene_component")]
    SpawnSceneComponent {
        entity: EditorEntityRef,
        component: EditorComponentKind,
        fields: Vec<EditorComponentPatch>,
    },
    #[serde(rename = "patch_component")]
    PatchComponentField {
        entity: EditorEntityRef,
        component: EditorComponentKind,
        field: EditorComponentField,
        value: EditorFieldValue,
    },
    #[serde(rename = "delete_entity")]
    DeleteEntity { entity: EditorEntityRef },
    #[serde(rename = "duplicate_entity")]
    DuplicateEntity {
        source: EditorEntityRef,
        destination: EditorEntityRef,
    },
    #[serde(rename = "attach_child_scene")]
    AttachChildScene {
        parent: EditorEntityRef,
        scene: SceneAssetRef,
    },
    #[serde(rename = "adjust_light")]
    AdjustLight {
        entity: EditorEntityRef,
        field: EditorComponentField,
        value: EditorFieldValue,
    },
    #[serde(rename = "adjust_render_policy")]
    AdjustRenderPolicy {
        entity: EditorEntityRef,
        field: EditorComponentField,
        value: EditorFieldValue,
    },
    #[serde(rename = "mark_selection_salience")]
    MarkSelectionSalience {
        entity: EditorEntityRef,
        rank: u16,
        selection_salience: u8,
        gameplay_score: u8,
    },
}

impl EditorOperation {
    #[must_use]
    pub const fn kind(&self) -> EditorOperationKind {
        match self {
            Self::SpawnSceneComponent { .. } => EditorOperationKind::SpawnSceneComponent,
            Self::PatchComponentField { .. } => EditorOperationKind::PatchComponentField,
            Self::DeleteEntity { .. } => EditorOperationKind::DeleteEntity,
            Self::DuplicateEntity { .. } => EditorOperationKind::DuplicateEntity,
            Self::AttachChildScene { .. } => EditorOperationKind::AttachChildScene,
            Self::AdjustLight { .. } => EditorOperationKind::AdjustLight,
            Self::AdjustRenderPolicy { .. } => EditorOperationKind::AdjustRenderPolicy,
            Self::MarkSelectionSalience { .. } => EditorOperationKind::MarkSelectionSalience,
        }
    }

    pub fn validate_product(&self) -> Result<(), EditorOperationValidationError> {
        match self {
            Self::SpawnSceneComponent {
                entity,
                component,
                fields,
            } => {
                entity.validate()?;
                if fields.is_empty() {
                    return Err(EditorOperationValidationError::MissingComponentFields);
                }
                for field in fields {
                    field.validate_for(*component)?;
                }
                Ok(())
            }
            Self::PatchComponentField {
                entity,
                component,
                field,
                value,
            } => {
                entity.validate()?;
                EditorComponentPatch {
                    field: *field,
                    value: value.clone(),
                }
                .validate_for(*component)
            }
            Self::DeleteEntity { entity } => entity.validate(),
            Self::DuplicateEntity {
                source,
                destination,
            } => {
                source.validate()?;
                destination.validate()
            }
            Self::AttachChildScene { parent, scene } => {
                parent.validate()?;
                scene.validate()
            }
            Self::AdjustLight {
                entity,
                field,
                value,
            } => {
                entity.validate()?;
                EditorComponentPatch {
                    field: *field,
                    value: value.clone(),
                }
                .validate_for(EditorComponentKind::LuxLight)
            }
            Self::AdjustRenderPolicy {
                entity,
                field,
                value,
            } => {
                entity.validate()?;
                if !matches!(
                    field,
                    EditorComponentField::Quality
                        | EditorComponentField::Latency
                        | EditorComponentField::EditorMode
                        | EditorComponentField::SuperResolution
                        | EditorComponentField::FrameGeneration
                        | EditorComponentField::HudlessRequired
                ) {
                    return Err(EditorOperationValidationError::FieldNotValidForComponent);
                }
                if !field_value_is_valid(*field, value) {
                    return Err(EditorOperationValidationError::ValueNotValidForField);
                }
                Ok(())
            }
            Self::MarkSelectionSalience { entity, .. } => entity.validate(),
        }
    }

    #[must_use]
    pub fn entity_ref(&self) -> Option<&EditorEntityRef> {
        match self {
            Self::SpawnSceneComponent { entity, .. }
            | Self::PatchComponentField { entity, .. }
            | Self::DeleteEntity { entity }
            | Self::AdjustLight { entity, .. }
            | Self::AdjustRenderPolicy { entity, .. }
            | Self::MarkSelectionSalience { entity, .. } => Some(entity),
            Self::DuplicateEntity { source, .. } => Some(source),
            Self::AttachChildScene { parent, .. } => Some(parent),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditorOperationEnvelope {
    #[serde(default)]
    pub operations: Vec<EditorOperation>,
    #[serde(default)]
    pub operation: Option<EditorOperation>,
}

impl EditorOperationEnvelope {
    #[must_use]
    pub fn single(operation: EditorOperation) -> Self {
        Self {
            operations: Vec::new(),
            operation: Some(operation),
        }
    }

    #[must_use]
    pub fn operation_count(&self) -> usize {
        self.operations.len() + usize::from(self.operation.is_some())
    }

    pub fn validate_product(&self) -> Result<(), EditorOperationValidationError> {
        if self.operation_count() == 0 {
            return Err(EditorOperationValidationError::EmptyOperationEnvelope);
        }
        if let Some(operation) = &self.operation {
            operation.validate_product()?;
        }
        for operation in &self.operations {
            operation.validate_product()?;
        }
        Ok(())
    }

    #[must_use]
    pub fn into_operations(mut self) -> Vec<EditorOperation> {
        if let Some(operation) = self.operation.take() {
            self.operations.push(operation);
        }
        self.operations
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditorOperationValidationError {
    EmptyOperationEnvelope,
    MissingEntityUri,
    InvalidEntityUri,
    EntityUriTooLong,
    MissingAssetUri,
    InvalidAssetUri,
    AssetUriTooLong,
    MissingComponentFields,
    FieldNotValidForComponent,
    ValueNotValidForField,
}

impl EditorOperationValidationError {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EmptyOperationEnvelope => "empty_operation_envelope",
            Self::MissingEntityUri => "missing_entity_uri",
            Self::InvalidEntityUri => "invalid_entity_uri",
            Self::EntityUriTooLong => "entity_uri_too_long",
            Self::MissingAssetUri => "missing_asset_uri",
            Self::InvalidAssetUri => "invalid_asset_uri",
            Self::AssetUriTooLong => "asset_uri_too_long",
            Self::MissingComponentFields => "missing_component_fields",
            Self::FieldNotValidForComponent => "field_not_valid_for_component",
            Self::ValueNotValidForField => "value_not_valid_for_field",
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Resource)]
pub struct EditorOperationQueue {
    pub operations: Vec<EditorOperation>,
}

impl EditorOperationQueue {
    pub fn push(&mut self, operation: EditorOperation) {
        self.operations.push(operation);
    }

    pub fn extend(&mut self, operations: impl IntoIterator<Item = EditorOperation>) {
        self.operations.extend(operations);
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.operations.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct EditorSceneEntityIndex {
    pub records: Vec<EditorSceneEntityRecord>,
}

impl EditorSceneEntityIndex {
    pub fn insert(&mut self, entity_ref: EditorEntityRef, entity: Entity) {
        if let Some(record) = self
            .records
            .iter_mut()
            .find(|record| record.entity_ref == entity_ref)
        {
            record.entity = entity;
        } else {
            self.records
                .push(EditorSceneEntityRecord { entity_ref, entity });
        }
        self.records
            .sort_by(|left, right| left.entity_ref.uri.cmp(&right.entity_ref.uri));
    }

    #[must_use]
    pub fn resolve(&self, entity_ref: &EditorEntityRef) -> Option<Entity> {
        self.records
            .iter()
            .find(|record| record.entity_ref == *entity_ref)
            .map(|record| record.entity)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorSceneEntityRecord {
    pub entity_ref: EditorEntityRef,
    pub entity: Entity,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct EditorOperationOutcomes {
    pub records: Vec<EditorOperationOutcome>,
}

impl EditorOperationOutcomes {
    pub fn push(&mut self, outcome: EditorOperationOutcome) {
        self.records.push(outcome);
        if self.records.len() > EDITOR_OPERATION_OUTCOME_CAPACITY {
            let overflow = self
                .records
                .len()
                .saturating_sub(EDITOR_OPERATION_OUTCOME_CAPACITY);
            self.records.drain(0..overflow);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorOperationOutcome {
    pub kind: EditorOperationKind,
    pub status: EditorOperationStatus,
    pub target: Option<EditorEntityRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditorOperationStatus {
    Applied,
    DeferredToSceneResolver,
    MissingEntity,
    MissingComponent,
    InvalidOperation,
}

impl EditorOperationStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::DeferredToSceneResolver => "deferred_to_scene_resolver",
            Self::MissingEntity => "missing_entity",
            Self::MissingComponent => "missing_component",
            Self::InvalidOperation => "invalid_operation",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub struct EditorManagedEntity;

pub fn apply_editor_operation_queue(world: &mut World) {
    let mut queue = world
        .remove_resource::<EditorOperationQueue>()
        .unwrap_or_default();
    let mut outcomes = world
        .remove_resource::<EditorOperationOutcomes>()
        .unwrap_or_default();
    let index = world
        .remove_resource::<EditorSceneEntityIndex>()
        .unwrap_or_default();

    for operation in queue.operations.drain(..) {
        let status = apply_editor_operation(world, &index, &operation);
        outcomes.push(EditorOperationOutcome {
            kind: operation.kind(),
            status,
            target: operation.entity_ref().cloned(),
        });
    }

    world.insert_resource(index);
    world.insert_resource(queue);
    world.insert_resource(outcomes);
}

fn apply_editor_operation(
    world: &mut World,
    index: &EditorSceneEntityIndex,
    operation: &EditorOperation,
) -> EditorOperationStatus {
    if operation.validate_product().is_err() {
        return EditorOperationStatus::InvalidOperation;
    }
    match operation {
        EditorOperation::PatchComponentField {
            entity,
            component,
            field,
            value,
        } => apply_component_patch(world, index, entity, *component, *field, value),
        EditorOperation::AdjustLight {
            entity,
            field,
            value,
        } => apply_component_patch(
            world,
            index,
            entity,
            EditorComponentKind::LuxLight,
            *field,
            value,
        ),
        EditorOperation::MarkSelectionSalience {
            entity,
            rank,
            selection_salience,
            gameplay_score,
        } => apply_selection_salience(
            world,
            index,
            entity,
            *rank,
            *selection_salience,
            *gameplay_score,
        ),
        EditorOperation::DeleteEntity { entity } => apply_delete_entity(world, index, entity),
        EditorOperation::SpawnSceneComponent { .. }
        | EditorOperation::DuplicateEntity { .. }
        | EditorOperation::AttachChildScene { .. }
        | EditorOperation::AdjustRenderPolicy { .. } => {
            EditorOperationStatus::DeferredToSceneResolver
        }
    }
}

fn apply_component_patch(
    world: &mut World,
    index: &EditorSceneEntityIndex,
    entity_ref: &EditorEntityRef,
    component: EditorComponentKind,
    field: EditorComponentField,
    value: &EditorFieldValue,
) -> EditorOperationStatus {
    let Some(entity) = index.resolve(entity_ref) else {
        return EditorOperationStatus::MissingEntity;
    };
    match (component, field) {
        (EditorComponentKind::LuxLight, EditorComponentField::IntensityLux) => {
            let Some(light) = world.get_mut::<LuxLight>(entity) else {
                return EditorOperationStatus::MissingComponent;
            };
            let Some(value) = value.as_f32() else {
                return EditorOperationStatus::InvalidOperation;
            };
            light.intensity_lux = value;
            EditorOperationStatus::Applied
        }
        (EditorComponentKind::GameplaySalient, EditorComponentField::Score) => {
            let Some(score) = value.as_u8() else {
                return EditorOperationStatus::InvalidOperation;
            };
            world.entity_mut(entity).insert(GameplaySalient { score });
            EditorOperationStatus::Applied
        }
        (EditorComponentKind::EditorSelection, EditorComponentField::Rank) => {
            let Some(rank) = value.as_u16() else {
                return EditorOperationStatus::InvalidOperation;
            };
            let selection = world
                .get::<EditorSelection>(entity)
                .copied()
                .unwrap_or_default();
            world.entity_mut(entity).insert(EditorSelection {
                rank,
                salience: selection.salience,
            });
            EditorOperationStatus::Applied
        }
        (EditorComponentKind::EditorSelection, EditorComponentField::Salience) => {
            let Some(salience) = value.as_u8() else {
                return EditorOperationStatus::InvalidOperation;
            };
            let selection = world
                .get::<EditorSelection>(entity)
                .copied()
                .unwrap_or_default();
            world.entity_mut(entity).insert(EditorSelection {
                rank: selection.rank,
                salience,
            });
            EditorOperationStatus::Applied
        }
        (EditorComponentKind::ViewportRenderPolicy, _) => {
            if world.get::<ViewportRenderPolicy>(entity).is_some() {
                EditorOperationStatus::DeferredToSceneResolver
            } else {
                EditorOperationStatus::MissingComponent
            }
        }
        _ => EditorOperationStatus::DeferredToSceneResolver,
    }
}

fn apply_selection_salience(
    world: &mut World,
    index: &EditorSceneEntityIndex,
    entity_ref: &EditorEntityRef,
    rank: u16,
    selection_salience: u8,
    gameplay_score: u8,
) -> EditorOperationStatus {
    let Some(entity) = index.resolve(entity_ref) else {
        return EditorOperationStatus::MissingEntity;
    };
    world
        .entity_mut(entity)
        .insert(EditorSelection {
            rank,
            salience: selection_salience,
        })
        .insert(GameplaySalient {
            score: gameplay_score,
        });
    EditorOperationStatus::Applied
}

fn apply_delete_entity(
    world: &mut World,
    index: &EditorSceneEntityIndex,
    entity_ref: &EditorEntityRef,
) -> EditorOperationStatus {
    let Some(entity) = index.resolve(entity_ref) else {
        return EditorOperationStatus::MissingEntity;
    };
    if world.get_entity(entity).is_err() {
        return EditorOperationStatus::MissingEntity;
    }
    world.entity_mut(entity).despawn();
    EditorOperationStatus::Applied
}

fn validate_editor_uri(
    value: &str,
    required_prefix: &str,
    max_len: usize,
    missing: EditorOperationValidationError,
    invalid: EditorOperationValidationError,
    too_long: EditorOperationValidationError,
) -> Result<(), EditorOperationValidationError> {
    validate_common_uri_bytes(value, max_len, missing, invalid, too_long)?;
    if value.starts_with(required_prefix) {
        Ok(())
    } else {
        Err(invalid)
    }
}

fn validate_common_uri_bytes(
    value: &str,
    max_len: usize,
    missing: EditorOperationValidationError,
    invalid: EditorOperationValidationError,
    too_long: EditorOperationValidationError,
) -> Result<(), EditorOperationValidationError> {
    if value.is_empty() {
        return Err(missing);
    }
    if value.len() > max_len {
        return Err(too_long);
    }
    if !value.bytes().all(valid_uri_byte) {
        return Err(invalid);
    }
    Ok(())
}

fn valid_uri_byte(value: u8) -> bool {
    value.is_ascii_alphanumeric()
        || matches!(
            value,
            b':' | b'/' | b'.' | b'-' | b'_' | b'#' | b'@' | b'%' | b'+'
        )
}

fn field_value_is_valid(field: EditorComponentField, value: &EditorFieldValue) -> bool {
    match field {
        EditorComponentField::IntensityLux
        | EditorComponentField::Range
        | EditorComponentField::Luminance
        | EditorComponentField::StreamingRadius => value.as_f32().is_some(),
        EditorComponentField::Score
        | EditorComponentField::Salience
        | EditorComponentField::Motion
        | EditorComponentField::Topology => value.as_u8().is_some(),
        EditorComponentField::Rank => value.as_u16().is_some(),
        EditorComponentField::HudlessRequired => matches!(value, EditorFieldValue::Bool(_)),
        EditorComponentField::Color | EditorComponentField::LocalBounds => {
            matches!(
                value,
                EditorFieldValue::Vec3(_) | EditorFieldValue::Token(_)
            )
        }
        EditorComponentField::Geometry
        | EditorComponentField::Material
        | EditorComponentField::Flags => {
            matches!(
                value,
                EditorFieldValue::U8(_)
                    | EditorFieldValue::U16(_)
                    | EditorFieldValue::U32(_)
                    | EditorFieldValue::Token(_)
            )
        }
        _ => matches!(value, EditorFieldValue::Token(_)),
    }
}

pub fn editor_empty_scene_asset_is_declarative() -> impl crate::FunSceneList {
    crate::FunEmptySceneList
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_policy_keeps_native_ui_svelte_and_fun_scene_authoritative() {
        let policy = EditorIntegrationPolicy::DEFAULT;

        assert!(policy.native_ui_svelte_edits_typed_scene_data);
        assert!(policy.host_applies_operations_through_fun_scene);
        assert!(!policy.runtime_external_ui_allowed);
        assert!(policy.native_ui_gpu_only_required);
        assert!(policy.overlays_are_renderer_debug_or_native_ui_late_composite);
        assert!(policy.fun_assets_are_declarative);
        assert!(policy.dynamic_rust_expressions_are_macro_only);
    }

    #[test]
    fn editor_operation_json_uses_unprefixed_component_names() {
        let json = br#"{
            "op": "patch_component",
            "entity": { "uri": "scene://arena-blockout/Floor" },
            "component": "LuxLight",
            "field": "intensity_lux",
            "value": 65000.0
        }"#;
        let operation: EditorOperation =
            serde_json::from_slice(json).expect("operation should deserialize");

        assert_eq!(operation.kind(), EditorOperationKind::PatchComponentField);
        operation
            .validate_product()
            .expect("operation should validate");

        let legacy = br#"{
            "op": "patch_component",
            "entity": { "uri": "scene://arena-blockout/Floor" },
            "component": "FunLuxLight",
            "field": "intensity_lux",
            "value": 65000.0
        }"#;
        assert!(serde_json::from_slice::<EditorOperation>(legacy).is_err());
    }

    #[test]
    fn editor_operation_queue_patches_light_and_selection_through_ecs() {
        let mut world = World::new();
        world.init_resource::<EditorOperationQueue>();
        world.init_resource::<EditorSceneEntityIndex>();
        world.init_resource::<EditorOperationOutcomes>();

        let entity = world.spawn(LuxLight::directional(80_000.0)).id();
        world
            .resource_mut::<EditorSceneEntityIndex>()
            .insert(EditorEntityRef::new("scene://arena-blockout/Sun"), entity);
        world.resource_mut::<EditorOperationQueue>().extend([
            EditorOperation::PatchComponentField {
                entity: EditorEntityRef::new("scene://arena-blockout/Sun"),
                component: EditorComponentKind::LuxLight,
                field: EditorComponentField::IntensityLux,
                value: EditorFieldValue::F32(65_000.0),
            },
            EditorOperation::MarkSelectionSalience {
                entity: EditorEntityRef::new("scene://arena-blockout/Sun"),
                rank: 0,
                selection_salience: 255,
                gameplay_score: 192,
            },
        ]);

        apply_editor_operation_queue(&mut world);

        let light = world
            .get::<LuxLight>(entity)
            .expect("light component remains present");
        assert_eq!(light.intensity_lux, 65_000.0);
        assert_eq!(
            world.get::<EditorSelection>(entity),
            Some(&EditorSelection {
                rank: 0,
                salience: 255,
            })
        );
        assert_eq!(
            world.get::<GameplaySalient>(entity),
            Some(&GameplaySalient { score: 192 })
        );
        assert_eq!(world.resource::<EditorOperationQueue>().len(), 0);
        assert_eq!(world.resource::<EditorOperationOutcomes>().records.len(), 2);
        assert!(
            world
                .resource::<EditorOperationOutcomes>()
                .records
                .iter()
                .all(|record| record.status == EditorOperationStatus::Applied)
        );
    }

    #[test]
    fn editor_observers_do_not_claim_renderer_hot_path_work() {
        assert_eq!(
            EditorObserverTriggerKind::ALL.len(),
            EDITOR_OBSERVER_DESCRIPTORS.len()
        );
        assert!(
            EDITOR_OBSERVER_DESCRIPTORS
                .iter()
                .all(|descriptor| descriptor.produces_normal_ecs_changes
                    && !descriptor.renderer_hot_path_allowed)
        );
        assert!(EditorOverlaySurface::ALL.iter().all(|surface| *surface
            != EditorOverlaySurface::RendererDebugPrimitive
            || surface.as_str() == "renderer_debug_primitive"));
    }

    #[test]
    fn fun_asset_refs_are_declarative_not_rust_expression_slots() {
        let op = EditorOperation::AttachChildScene {
            parent: EditorEntityRef::new("scene://arena-blockout/Floor"),
            scene: SceneAssetRef::new("asset://scenes/arena_blockout.fun"),
        };
        op.validate_product()
            .expect("asset reference should be declarative");

        let invalid = EditorOperation::AttachChildScene {
            parent: EditorEntityRef::new("scene://arena-blockout/Floor"),
            scene: SceneAssetRef::new("rust://spawn(dynamic_expr())"),
        };
        assert_eq!(
            invalid.validate_product(),
            Err(EditorOperationValidationError::InvalidAssetUri)
        );
    }
}
