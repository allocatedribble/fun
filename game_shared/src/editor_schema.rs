//! Editor-visible ECS schema and mutation validation contracts.
//!
//! This module stays Bevy-free on purpose. Runtime crates register concrete
//! Bevy component/resource types through the macros below, while the editor
//! protocol sees stable [`NetEntity`], [`ComponentKind`], and [`ResourceKind`]
//! values.

use crate::{
    EditorCapability, EditorComponentSchema, EditorMutability, EditorReplicationPolicy,
    EditorSerializationPolicy, EditorWorldRevision, ResourceKind, capability_is_granted,
};
pub use thunder::protocol::{AuthorityMode, ComponentKind, NetEntity, ReplicationClass};

/// Stable component kind for an editor-visible transform payload.
pub const EDITOR_COMPONENT_KIND_TRANSFORM: ComponentKind = ComponentKind(1);
/// Stable component kind for network identity metadata.
pub const EDITOR_COMPONENT_KIND_NETWORK_IDENTITY: ComponentKind = ComponentKind(2);
/// Stable component kind for network authority metadata.
pub const EDITOR_COMPONENT_KIND_NETWORK_AUTHORITY: ComponentKind = ComponentKind(3);
/// Stable component kind for streamed world catalog metadata.
pub const EDITOR_COMPONENT_KIND_WORLD_CATALOG_REF: ComponentKind = ComponentKind(4);
/// Stable component kind for an entity display name.
pub const EDITOR_COMPONENT_KIND_NAME: ComponentKind = ComponentKind(5);

/// Runtime visual/debug resource kind for Solari editor settings.
pub const EDITOR_RESOURCE_KIND_SOLARI_DEBUG: ResourceKind = ResourceKind(1);
/// Critical server/match state resource kind.
pub const EDITOR_RESOURCE_KIND_MATCH_STATE: ResourceKind = ResourceKind(2);
/// Runtime diagnostic settings resource kind.
pub const EDITOR_RESOURCE_KIND_RUNTIME_DIAGNOSTICS: ResourceKind = ResourceKind(3);

/// Entity-level editor mutability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum EditorEntityMutability {
    /// Visible to the editor, but not mutable.
    ReadOnly,
    /// Runtime-only edits may be applied.
    RuntimeMutable,
    /// Runtime edits may be persisted through the iteration pipeline.
    PersistentMutable,
    /// Critical authoritative server state that requires stricter review.
    CriticalServerState,
}

impl EditorEntityMutability {
    #[must_use]
    pub const fn allows_component_mutation(self) -> bool {
        matches!(
            self,
            Self::RuntimeMutable | Self::PersistentMutable | Self::CriticalServerState
        )
    }
}

/// Editor-visible identity for a server/network entity.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorVisibleEntityIdentity {
    pub entity: NetEntity,
    pub replication_class: ReplicationClass,
    pub authority: AuthorityMode,
    pub name: Option<String>,
    pub mutability: EditorEntityMutability,
}

impl EditorVisibleEntityIdentity {
    #[must_use]
    pub fn from_network_identity(
        entity: NetEntity,
        replication_class: ReplicationClass,
        authority: AuthorityMode,
        name: Option<impl Into<String>>,
        mutability: EditorEntityMutability,
    ) -> Self {
        Self {
            entity,
            replication_class,
            authority,
            name: name.map(Into::into),
            mutability,
        }
    }
}

/// Editor display weight for a registered component/resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditorSchemaImportance {
    Primary,
    Secondary,
    Advanced,
    Diagnostic,
}

/// Preferred editor widget for a component/resource payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditorSchemaWidget {
    Transform3d,
    Text,
    Number,
    Toggle,
    Bytes,
    ReadOnlyStruct,
    ResourcePanel,
}

/// UI hints generated from the same registry as network/editor schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EditorUiHints {
    pub display_label: &'static str,
    pub group: &'static str,
    pub widget: EditorSchemaWidget,
    pub importance: EditorSchemaImportance,
}

impl EditorUiHints {
    #[must_use]
    pub const fn new(
        display_label: &'static str,
        group: &'static str,
        widget: EditorSchemaWidget,
        importance: EditorSchemaImportance,
    ) -> Self {
        Self {
            display_label,
            group,
            widget,
            importance,
        }
    }
}

/// Stable diagnostic labels attached to schema-driven editor mutations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EditorDiagnosticLabels {
    pub target: &'static str,
    pub event: &'static str,
}

impl EditorDiagnosticLabels {
    #[must_use]
    pub const fn new(target: &'static str, event: &'static str) -> Self {
        Self { target, event }
    }
}

/// Reflect registration metadata supplied by the runtime crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EditorReflectTypeRegistration {
    pub reflect_type_name: &'static str,
    pub registration_symbol: &'static str,
}

/// Named codec hooks used by runtime-specific serializers/deserializers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EditorPayloadCodec {
    pub serializer: &'static str,
    pub deserializer: &'static str,
    pub max_payload_bytes: u32,
}

impl EditorPayloadCodec {
    #[must_use]
    pub const fn compactly(
        serializer: &'static str,
        deserializer: &'static str,
        max_payload_bytes: u32,
    ) -> Self {
        Self {
            serializer,
            deserializer,
            max_payload_bytes,
        }
    }
}

/// Deterministic typed error for editor mutation validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMutationError {
    UnknownEntity,
    UnknownComponentKind,
    MissingCapability,
    StaleRevision,
    ValidationFailed,
    PayloadTooLarge,
    ExecutionBudgetExceeded,
    UnknownResourceKind,
}

/// Small execution budget checked before expensive validation or mutation work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EditorExecutionBudget {
    pub max_ops: u32,
    pub used_ops: u32,
}

impl EditorExecutionBudget {
    #[must_use]
    pub const fn remaining_ops(self) -> u32 {
        self.max_ops.saturating_sub(self.used_ops)
    }

    #[must_use]
    pub const fn has_capacity(self) -> bool {
        self.remaining_ops() > 0
    }
}

impl Default for EditorExecutionBudget {
    fn default() -> Self {
        Self {
            max_ops: 1,
            used_ops: 0,
        }
    }
}

/// Input supplied to a registered mutation validator.
#[derive(Debug, Clone, Copy)]
pub struct EditorMutationValidationInput<'a> {
    pub entity: Option<NetEntity>,
    pub component_kind: Option<ComponentKind>,
    pub resource_kind: Option<ResourceKind>,
    pub payload: &'a [u8],
    pub granted_capabilities: &'a [EditorCapability],
    pub required_capability: EditorCapability,
    pub base_world_revision: EditorWorldRevision,
    pub current_world_revision: EditorWorldRevision,
    pub execution_budget: EditorExecutionBudget,
}

pub type EditorMutationValidator =
    for<'a> fn(EditorMutationValidationInput<'a>) -> Result<(), EditorMutationError>;

/// One component registration shared by runtime, network, editor, and diagnostics.
#[derive(Debug, Clone, Copy)]
pub struct EditorComponentRegistration {
    pub component_kind: ComponentKind,
    pub rust_type_name: &'static str,
    pub reflect: EditorReflectTypeRegistration,
    pub codec: EditorPayloadCodec,
    pub mutation_validator: EditorMutationValidator,
    pub required_capability: EditorCapability,
    pub mutability: EditorMutability,
    pub serialization_policy: EditorSerializationPolicy,
    pub replication_policy: EditorReplicationPolicy,
    pub ui_hints: EditorUiHints,
    pub diagnostics: EditorDiagnosticLabels,
}

impl EditorComponentRegistration {
    #[must_use]
    pub fn schema(&self, stable_type_id: crate::EditorStableTypeId) -> EditorComponentSchema {
        EditorComponentSchema {
            stable_type_id,
            component_kind: self.component_kind,
            display_label: self.ui_hints.display_label.to_owned(),
            mutability: self.mutability,
            serialization_policy: self.serialization_policy,
            replication_policy: self.replication_policy,
        }
    }
}

/// Resource mutation sensitivity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditorResourcePolicy {
    ReadOnly,
    VisualRuntimeSetting,
    CriticalServerState,
}

/// One resource registration shared by runtime, editor, and diagnostics.
#[derive(Debug, Clone, Copy)]
pub struct EditorResourceRegistration {
    pub resource_kind: ResourceKind,
    pub rust_type_name: &'static str,
    pub reflect: EditorReflectTypeRegistration,
    pub codec: EditorPayloadCodec,
    pub mutation_validator: EditorMutationValidator,
    pub required_capability: EditorCapability,
    pub mutability: EditorMutability,
    pub policy: EditorResourcePolicy,
    pub ui_hints: EditorUiHints,
    pub diagnostics: EditorDiagnosticLabels,
}

/// Borrowed schema registry for a runtime.
#[derive(Debug, Clone, Copy)]
pub struct EditorSchemaRegistry<'a> {
    pub components: &'a [EditorComponentRegistration],
    pub resources: &'a [EditorResourceRegistration],
}

impl<'a> EditorSchemaRegistry<'a> {
    #[must_use]
    pub const fn new(
        components: &'a [EditorComponentRegistration],
        resources: &'a [EditorResourceRegistration],
    ) -> Self {
        Self {
            components,
            resources,
        }
    }

    #[must_use]
    pub fn component(&self, kind: ComponentKind) -> Option<&'a EditorComponentRegistration> {
        self.components
            .iter()
            .find(|registration| registration.component_kind == kind)
    }

    #[must_use]
    pub fn resource(&self, kind: ResourceKind) -> Option<&'a EditorResourceRegistration> {
        self.resources
            .iter()
            .find(|registration| registration.resource_kind == kind)
    }
}

/// Component mutation request checked before runtime ECS access.
#[derive(Debug, Clone, Copy)]
pub struct EditorComponentMutationRequest<'a> {
    pub entity: NetEntity,
    pub component_kind: ComponentKind,
    pub payload: &'a [u8],
    pub granted_capabilities: &'a [EditorCapability],
    pub base_world_revision: EditorWorldRevision,
    pub current_world_revision: EditorWorldRevision,
    pub known_entities: &'a [EditorVisibleEntityIdentity],
    pub execution_budget: EditorExecutionBudget,
}

/// Resource mutation request checked before runtime resource access.
#[derive(Debug, Clone, Copy)]
pub struct EditorResourceMutationRequest<'a> {
    pub resource_kind: ResourceKind,
    pub payload: &'a [u8],
    pub granted_capabilities: &'a [EditorCapability],
    pub base_world_revision: EditorWorldRevision,
    pub current_world_revision: EditorWorldRevision,
    pub execution_budget: EditorExecutionBudget,
}

/// Validates an editor component mutation without touching ECS state.
pub fn validate_component_mutation(
    registry: EditorSchemaRegistry<'_>,
    request: EditorComponentMutationRequest<'_>,
) -> Result<(), EditorMutationError> {
    let entity = request
        .known_entities
        .iter()
        .find(|entity| entity.entity == request.entity)
        .ok_or(EditorMutationError::UnknownEntity)?;

    if !entity.mutability.allows_component_mutation() {
        return Err(EditorMutationError::ValidationFailed);
    }

    let registration = registry
        .component(request.component_kind)
        .ok_or(EditorMutationError::UnknownComponentKind)?;

    validate_common_mutation(
        request.payload,
        request.granted_capabilities,
        registration.required_capability,
        registration.codec.max_payload_bytes,
        request.base_world_revision,
        request.current_world_revision,
        request.execution_budget,
    )?;

    (registration.mutation_validator)(EditorMutationValidationInput {
        entity: Some(request.entity),
        component_kind: Some(request.component_kind),
        resource_kind: None,
        payload: request.payload,
        granted_capabilities: request.granted_capabilities,
        required_capability: registration.required_capability,
        base_world_revision: request.base_world_revision,
        current_world_revision: request.current_world_revision,
        execution_budget: request.execution_budget,
    })
}

/// Validates an editor resource mutation without touching runtime resources.
pub fn validate_resource_mutation(
    registry: EditorSchemaRegistry<'_>,
    request: EditorResourceMutationRequest<'_>,
) -> Result<(), EditorMutationError> {
    let registration = registry
        .resource(request.resource_kind)
        .ok_or(EditorMutationError::UnknownResourceKind)?;

    if registration.mutability == EditorMutability::ReadOnly {
        return Err(EditorMutationError::ValidationFailed);
    }

    validate_common_mutation(
        request.payload,
        request.granted_capabilities,
        registration.required_capability,
        registration.codec.max_payload_bytes,
        request.base_world_revision,
        request.current_world_revision,
        request.execution_budget,
    )?;

    (registration.mutation_validator)(EditorMutationValidationInput {
        entity: None,
        component_kind: None,
        resource_kind: Some(request.resource_kind),
        payload: request.payload,
        granted_capabilities: request.granted_capabilities,
        required_capability: registration.required_capability,
        base_world_revision: request.base_world_revision,
        current_world_revision: request.current_world_revision,
        execution_budget: request.execution_budget,
    })
}

fn validate_common_mutation(
    payload: &[u8],
    granted_capabilities: &[EditorCapability],
    required_capability: EditorCapability,
    max_payload_bytes: u32,
    base_world_revision: EditorWorldRevision,
    current_world_revision: EditorWorldRevision,
    execution_budget: EditorExecutionBudget,
) -> Result<(), EditorMutationError> {
    if base_world_revision < current_world_revision {
        return Err(EditorMutationError::StaleRevision);
    }
    if !execution_budget.has_capacity() {
        return Err(EditorMutationError::ExecutionBudgetExceeded);
    }
    if payload.len() > max_payload_bytes as usize {
        return Err(EditorMutationError::PayloadTooLarge);
    }
    if !capability_is_granted(granted_capabilities, required_capability) {
        return Err(EditorMutationError::MissingCapability);
    }
    Ok(())
}

/// Accepts any payload that passed common schema validation.
pub fn validate_payload_shape(
    _input: EditorMutationValidationInput<'_>,
) -> Result<(), EditorMutationError> {
    Ok(())
}

/// Rejects empty mutation payloads for components/resources that require data.
pub fn validate_non_empty_payload(
    input: EditorMutationValidationInput<'_>,
) -> Result<(), EditorMutationError> {
    if input.payload.is_empty() {
        Err(EditorMutationError::ValidationFailed)
    } else {
        Ok(())
    }
}

/// Rejects all mutation attempts for read-only schema entries.
pub fn reject_editor_mutation(
    _input: EditorMutationValidationInput<'_>,
) -> Result<(), EditorMutationError> {
    Err(EditorMutationError::ValidationFailed)
}

#[macro_export]
macro_rules! editor_component_registry {
    (
        $(
            $name:ident => {
                kind: $kind:expr,
                type: $rust_type:ty,
                reflect: $reflect_type:expr,
                serializer: $serializer:expr,
                deserializer: $deserializer:expr,
                validator: $validator:expr,
                capability: $capability:expr,
                mutability: $mutability:expr,
                serialization: $serialization:expr,
                replication: $replication:expr,
                ui: $ui:expr,
                diagnostics: $diagnostics:expr,
                max_payload_bytes: $max_payload_bytes:expr $(,)?
            }
        ),* $(,)?
    ) => {
        {
            static REGISTRY: &[$crate::EditorComponentRegistration] = &[
                $(
                    $crate::EditorComponentRegistration {
                        component_kind: $kind,
                        rust_type_name: stringify!($rust_type),
                        reflect: $crate::EditorReflectTypeRegistration {
                            reflect_type_name: $reflect_type,
                            registration_symbol: concat!(module_path!(), "::", stringify!($name)),
                        },
                        codec: $crate::EditorPayloadCodec::compactly(
                            $serializer,
                            $deserializer,
                            $max_payload_bytes,
                        ),
                        mutation_validator: $validator,
                        required_capability: $capability,
                        mutability: $mutability,
                        serialization_policy: $serialization,
                        replication_policy: $replication,
                        ui_hints: $ui,
                        diagnostics: $diagnostics,
                    }
                ),*
            ];
            REGISTRY
        }
    };
}

#[macro_export]
macro_rules! editor_resource_registry {
    (
        $(
            $name:ident => {
                kind: $kind:expr,
                type: $rust_type:ty,
                reflect: $reflect_type:expr,
                serializer: $serializer:expr,
                deserializer: $deserializer:expr,
                validator: $validator:expr,
                capability: $capability:expr,
                mutability: $mutability:expr,
                policy: $policy:expr,
                ui: $ui:expr,
                diagnostics: $diagnostics:expr,
                max_payload_bytes: $max_payload_bytes:expr $(,)?
            }
        ),* $(,)?
    ) => {
        {
            static REGISTRY: &[$crate::EditorResourceRegistration] = &[
                $(
                    $crate::EditorResourceRegistration {
                        resource_kind: $kind,
                        rust_type_name: stringify!($rust_type),
                        reflect: $crate::EditorReflectTypeRegistration {
                            reflect_type_name: $reflect_type,
                            registration_symbol: concat!(module_path!(), "::", stringify!($name)),
                        },
                        codec: $crate::EditorPayloadCodec::compactly(
                            $serializer,
                            $deserializer,
                            $max_payload_bytes,
                        ),
                        mutation_validator: $validator,
                        required_capability: $capability,
                        mutability: $mutability,
                        policy: $policy,
                        ui_hints: $ui,
                        diagnostics: $diagnostics,
                    }
                ),*
            ];
            REGISTRY
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EditorStableTypeId;

    #[allow(dead_code)]
    #[derive(Debug)]
    struct TransformPayload;

    #[allow(dead_code)]
    #[derive(Debug)]
    struct CriticalMatchState;

    fn test_components() -> &'static [EditorComponentRegistration] {
        editor_component_registry! {
            transform => {
                kind: EDITOR_COMPONENT_KIND_TRANSFORM,
                type: TransformPayload,
                reflect: "fun::TransformPayload",
                serializer: "fun.editor.transform.encode.v1",
                deserializer: "fun.editor.transform.decode.v1",
                validator: validate_non_empty_payload,
                capability: EditorCapability::MutateEntities,
                mutability: EditorMutability::RuntimeMutable,
                serialization: EditorSerializationPolicy::Compactly,
                replication: EditorReplicationPolicy::ServerAuthoritative,
                ui: EditorUiHints::new(
                    "Transform",
                    "Transform",
                    EditorSchemaWidget::Transform3d,
                    EditorSchemaImportance::Primary,
                ),
                diagnostics: EditorDiagnosticLabels::new(
                    "fun::editor::schema",
                    "component_transform",
                ),
                max_payload_bytes: 64,
            },
        }
    }

    fn test_resources() -> &'static [EditorResourceRegistration] {
        editor_resource_registry! {
            match_state => {
                kind: EDITOR_RESOURCE_KIND_MATCH_STATE,
                type: CriticalMatchState,
                reflect: "fun::CriticalMatchState",
                serializer: "fun.editor.match_state.encode.v1",
                deserializer: "fun.editor.match_state.decode.v1",
                validator: validate_non_empty_payload,
                capability: EditorCapability::PersistIteration,
                mutability: EditorMutability::PersistentMutable,
                policy: EditorResourcePolicy::CriticalServerState,
                ui: EditorUiHints::new(
                    "Match State",
                    "Server",
                    EditorSchemaWidget::ResourcePanel,
                    EditorSchemaImportance::Advanced,
                ),
                diagnostics: EditorDiagnosticLabels::new(
                    "fun::editor::schema",
                    "resource_match_state",
                ),
                max_payload_bytes: 128,
            },
        }
    }

    fn visible_entity(mutability: EditorEntityMutability) -> EditorVisibleEntityIdentity {
        EditorVisibleEntityIdentity::from_network_identity(
            NetEntity::from_parts(1, 2),
            ReplicationClass::World,
            AuthorityMode::StaticServer,
            Some("Floor"),
            mutability,
        )
    }

    #[test]
    fn component_registry_generates_schema_without_duplicate_handwriting() {
        let registration = &test_components()[0];
        let schema = registration.schema(EditorStableTypeId(99));

        assert_eq!(schema.component_kind, EDITOR_COMPONENT_KIND_TRANSFORM);
        assert_eq!(schema.display_label, "Transform");
        assert!(registration.rust_type_name.ends_with("TransformPayload"));
        assert_eq!(
            registration.reflect.registration_symbol,
            "game_shared::editor_schema::tests::transform"
        );
    }

    #[test]
    fn component_mutation_validation_rejects_unknown_entity() {
        let registry = EditorSchemaRegistry::new(test_components(), &[]);
        let request = EditorComponentMutationRequest {
            entity: NetEntity::from_parts(9, 9),
            component_kind: EDITOR_COMPONENT_KIND_TRANSFORM,
            payload: &[1],
            granted_capabilities: &[EditorCapability::MutateEntities],
            base_world_revision: EditorWorldRevision(1),
            current_world_revision: EditorWorldRevision(1),
            known_entities: &[visible_entity(EditorEntityMutability::RuntimeMutable)],
            execution_budget: EditorExecutionBudget::default(),
        };

        assert_eq!(
            validate_component_mutation(registry, request),
            Err(EditorMutationError::UnknownEntity)
        );
    }

    #[test]
    fn component_mutation_validation_rejects_unknown_component_kind() {
        let registry = EditorSchemaRegistry::new(test_components(), &[]);
        let request = EditorComponentMutationRequest {
            entity: NetEntity::from_parts(1, 2),
            component_kind: ComponentKind(999),
            payload: &[1],
            granted_capabilities: &[EditorCapability::MutateEntities],
            base_world_revision: EditorWorldRevision(1),
            current_world_revision: EditorWorldRevision(1),
            known_entities: &[visible_entity(EditorEntityMutability::RuntimeMutable)],
            execution_budget: EditorExecutionBudget::default(),
        };

        assert_eq!(
            validate_component_mutation(registry, request),
            Err(EditorMutationError::UnknownComponentKind)
        );
    }

    #[test]
    fn component_mutation_validation_rejects_missing_capability() {
        let registry = EditorSchemaRegistry::new(test_components(), &[]);
        let request = EditorComponentMutationRequest {
            entity: NetEntity::from_parts(1, 2),
            component_kind: EDITOR_COMPONENT_KIND_TRANSFORM,
            payload: &[1],
            granted_capabilities: &[EditorCapability::ReadComponents],
            base_world_revision: EditorWorldRevision(1),
            current_world_revision: EditorWorldRevision(1),
            known_entities: &[visible_entity(EditorEntityMutability::RuntimeMutable)],
            execution_budget: EditorExecutionBudget::default(),
        };

        assert_eq!(
            validate_component_mutation(registry, request),
            Err(EditorMutationError::MissingCapability)
        );
    }

    #[test]
    fn component_mutation_validation_rejects_stale_revision() {
        let registry = EditorSchemaRegistry::new(test_components(), &[]);
        let request = EditorComponentMutationRequest {
            entity: NetEntity::from_parts(1, 2),
            component_kind: EDITOR_COMPONENT_KIND_TRANSFORM,
            payload: &[1],
            granted_capabilities: &[EditorCapability::MutateEntities],
            base_world_revision: EditorWorldRevision(1),
            current_world_revision: EditorWorldRevision(2),
            known_entities: &[visible_entity(EditorEntityMutability::RuntimeMutable)],
            execution_budget: EditorExecutionBudget::default(),
        };

        assert_eq!(
            validate_component_mutation(registry, request),
            Err(EditorMutationError::StaleRevision)
        );
    }

    #[test]
    fn component_mutation_validation_rejects_empty_payload() {
        let registry = EditorSchemaRegistry::new(test_components(), &[]);
        let request = EditorComponentMutationRequest {
            entity: NetEntity::from_parts(1, 2),
            component_kind: EDITOR_COMPONENT_KIND_TRANSFORM,
            payload: &[],
            granted_capabilities: &[EditorCapability::MutateEntities],
            base_world_revision: EditorWorldRevision(1),
            current_world_revision: EditorWorldRevision(1),
            known_entities: &[visible_entity(EditorEntityMutability::RuntimeMutable)],
            execution_budget: EditorExecutionBudget::default(),
        };

        assert_eq!(
            validate_component_mutation(registry, request),
            Err(EditorMutationError::ValidationFailed)
        );
    }

    #[test]
    fn component_mutation_validation_rejects_payload_too_large() {
        let registry = EditorSchemaRegistry::new(test_components(), &[]);
        let payload = [0_u8; 65];
        let request = EditorComponentMutationRequest {
            entity: NetEntity::from_parts(1, 2),
            component_kind: EDITOR_COMPONENT_KIND_TRANSFORM,
            payload: &payload,
            granted_capabilities: &[EditorCapability::MutateEntities],
            base_world_revision: EditorWorldRevision(1),
            current_world_revision: EditorWorldRevision(1),
            known_entities: &[visible_entity(EditorEntityMutability::RuntimeMutable)],
            execution_budget: EditorExecutionBudget::default(),
        };

        assert_eq!(
            validate_component_mutation(registry, request),
            Err(EditorMutationError::PayloadTooLarge)
        );
    }

    #[test]
    fn component_mutation_validation_rejects_exhausted_budget() {
        let registry = EditorSchemaRegistry::new(test_components(), &[]);
        let request = EditorComponentMutationRequest {
            entity: NetEntity::from_parts(1, 2),
            component_kind: EDITOR_COMPONENT_KIND_TRANSFORM,
            payload: &[1],
            granted_capabilities: &[EditorCapability::MutateEntities],
            base_world_revision: EditorWorldRevision(1),
            current_world_revision: EditorWorldRevision(1),
            known_entities: &[visible_entity(EditorEntityMutability::RuntimeMutable)],
            execution_budget: EditorExecutionBudget {
                max_ops: 1,
                used_ops: 1,
            },
        };

        assert_eq!(
            validate_component_mutation(registry, request),
            Err(EditorMutationError::ExecutionBudgetExceeded)
        );
    }

    #[test]
    fn resource_mutation_requires_explicit_stricter_capability() {
        let registry = EditorSchemaRegistry::new(&[], test_resources());
        let request = EditorResourceMutationRequest {
            resource_kind: EDITOR_RESOURCE_KIND_MATCH_STATE,
            payload: &[1],
            granted_capabilities: &[EditorCapability::ControlRuntime],
            base_world_revision: EditorWorldRevision(1),
            current_world_revision: EditorWorldRevision(1),
            execution_budget: EditorExecutionBudget::default(),
        };

        assert_eq!(
            validate_resource_mutation(registry, request),
            Err(EditorMutationError::MissingCapability)
        );
    }
}
