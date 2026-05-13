use avis_runtime::prelude::PhysicsPlugins;
use bevy::prelude::*;
use fun_ecs::AvisEcsBridge;

pub(crate) use avis_runtime::{
    math::{AdjustPrecision, AsF32},
    prelude::{
        Collider, MoveAndSlide, MoveAndSlideConfig, MoveAndSlideHitResponse, Position, RigidBody,
        Rotation, ShapeCastConfig, SpatialQueryFilter,
    },
};

pub(crate) const GAME_CLIENT_AVIS_STACK_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub(crate) struct ClientAvisStackContract {
    pub schema_version: u16,
    pub fun_ecs_declarations_are_authoritative: bool,
    pub avis_owns_hot_physics_state: bool,
    pub client_uses_fun_ecs_handoff_contract: bool,
}

impl Default for ClientAvisStackContract {
    fn default() -> Self {
        let bridge = AvisEcsBridge::new();
        Self {
            schema_version: GAME_CLIENT_AVIS_STACK_SCHEMA_VERSION,
            fun_ecs_declarations_are_authoritative: bridge.ecs_owns_declarations,
            avis_owns_hot_physics_state: bridge.avis_owns_hot_physics_state,
            client_uses_fun_ecs_handoff_contract: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub(crate) struct ClientAvisEcsBridge {
    pub bridge: AvisEcsBridge,
}

impl Default for ClientAvisEcsBridge {
    fn default() -> Self {
        Self {
            bridge: AvisEcsBridge::new(),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct AvisClientPlugin;

impl Plugin for AvisClientPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClientAvisStackContract::default())
            .insert_resource(ClientAvisEcsBridge::default())
            .add_plugins(PhysicsPlugins::default())
            .add_systems(Startup, log_avis_client_stack_contract);
    }
}

fn log_avis_client_stack_contract(_contract: Res<ClientAvisStackContract>) {
    game_shared::fun_diag_info!(
        target: "fun::client::avis",
        schema_version = _contract.schema_version,
        fun_ecs_declarations_are_authoritative = _contract.fun_ecs_declarations_are_authoritative,
        avis_owns_hot_physics_state = _contract.avis_owns_hot_physics_state,
        client_uses_fun_ecs_handoff_contract = _contract.client_uses_fun_ecs_handoff_contract,
        "client Avis bridge ready"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_avis_contract_matches_fun_ecs_bridge() {
        let bridge = AvisEcsBridge::new();
        let contract = ClientAvisStackContract::default();

        assert!(contract.fun_ecs_declarations_are_authoritative);
        assert_eq!(
            contract.fun_ecs_declarations_are_authoritative,
            bridge.ecs_owns_declarations
        );
        assert_eq!(
            contract.avis_owns_hot_physics_state,
            bridge.avis_owns_hot_physics_state
        );
        assert!(contract.client_uses_fun_ecs_handoff_contract);
        assert!(!bridge.hot_physics_state_in_ecs);
    }
}
