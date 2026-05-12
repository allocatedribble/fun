pub mod subsystems;

pub use subsystems::{
    AiEcsBridge, AiProposalPlan, AnimationEcsBridge, AnimationPoseArtifactManifest, AvisEcsBridge,
    ConservativePhysicsFallback, LuxDirtyRow, LuxDirtyRowTable, LuxEcsBridge, LuxFallbackPolicy,
    LuxHandoffImportPlan, NetworkDeltaManifest, NetworkRelevanceRow, NetworkRelevanceTable,
    NetworkRollbackApplyPlan, PerPeerEcsBudgetBridge, PhysicsCookImportPlan, PhysicsProxyManifest,
    PhysicsWritebackApplyPlan, RenderableDeclaration, RendererArtifactImportPlan,
    RendererArtifactRetirePlan, RendererEcsBridge, RendererFrameSnapshot,
    RendererPresentDependencyValidator, RvelteEcsBridge, RvelteGenerationSnapshot,
    RvelteInputEnvelope, RveltePaintPacketManifest, RvelteRendererHandoffQueue,
    RvelteStateResource, SubsystemContractError, TelemetryEcsBridge, TelemetryReport,
    TelemetryReportKind, ThunderEcsBridge, WardenEcsBridge,
};

use crate::FunEcsSubsystem;

pub const FUN_ECS_ADAPTER_CONTRACTS: [FunEcsAdapterContract; 10] = [
    FunEcsAdapterContract::new(
        FunEcsAdapterKind::BevyInterop,
        FunEcsSubsystem::FunEcs,
        FunEcsAdapterRole::CompatibilityWorld,
    ),
    FunEcsAdapterContract::new(
        FunEcsAdapterKind::Renderer,
        FunEcsSubsystem::Renderer,
        FunEcsAdapterRole::HandoffConsumer,
    ),
    FunEcsAdapterContract::new(
        FunEcsAdapterKind::Lux,
        FunEcsSubsystem::Lux,
        FunEcsAdapterRole::HandoffConsumer,
    ),
    FunEcsAdapterContract::new(
        FunEcsAdapterKind::Avis,
        FunEcsSubsystem::Avis,
        FunEcsAdapterRole::HandoffConsumer,
    ),
    FunEcsAdapterContract::new(
        FunEcsAdapterKind::Thunder,
        FunEcsSubsystem::Thunder,
        FunEcsAdapterRole::HandoffConsumer,
    ),
    FunEcsAdapterContract::new(
        FunEcsAdapterKind::Rvelte,
        FunEcsSubsystem::Rvelte,
        FunEcsAdapterRole::ExtractionAdapter,
    ),
    FunEcsAdapterContract::new(
        FunEcsAdapterKind::Animation,
        FunEcsSubsystem::Animation,
        FunEcsAdapterRole::ExternalArtifactProducer,
    ),
    FunEcsAdapterContract::new(
        FunEcsAdapterKind::Ai,
        FunEcsSubsystem::Ai,
        FunEcsAdapterRole::AgentProposalProducer,
    ),
    FunEcsAdapterContract::new(
        FunEcsAdapterKind::Warden,
        FunEcsSubsystem::Warden,
        FunEcsAdapterRole::SecurityObserver,
    ),
    FunEcsAdapterContract::new(
        FunEcsAdapterKind::Telemetry,
        FunEcsSubsystem::Telemetry,
        FunEcsAdapterRole::DiagnosticsSink,
    ),
];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FunEcsAdapterKind {
    #[default]
    BevyInterop = 0,
    Renderer = 1,
    Lux = 2,
    Avis = 3,
    Thunder = 4,
    Rvelte = 5,
    Animation = 6,
    Ai = 7,
    Warden = 8,
    Telemetry = 9,
}

impl FunEcsAdapterKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::BevyInterop => "bevy_interop",
            Self::Renderer => "renderer",
            Self::Lux => "lux",
            Self::Avis => "avis",
            Self::Thunder => "thunder",
            Self::Rvelte => "rvelte",
            Self::Animation => "animation",
            Self::Ai => "ai",
            Self::Warden => "warden",
            Self::Telemetry => "telemetry",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FunEcsAdapterRole {
    #[default]
    CompatibilityWorld = 0,
    ExtractionAdapter = 1,
    HandoffConsumer = 2,
    ExternalArtifactProducer = 3,
    AgentProposalProducer = 4,
    SecurityObserver = 5,
    DiagnosticsSink = 6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunEcsAdapterContract {
    pub kind: FunEcsAdapterKind,
    pub subsystem: FunEcsSubsystem,
    pub role: FunEcsAdapterRole,
}

impl FunEcsAdapterContract {
    #[must_use]
    pub const fn new(
        kind: FunEcsAdapterKind,
        subsystem: FunEcsSubsystem,
        role: FunEcsAdapterRole,
    ) -> Self {
        Self {
            kind,
            subsystem,
            role,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bevy_adapter_is_compatibility_only() {
        let bevy = FUN_ECS_ADAPTER_CONTRACTS
            .iter()
            .find(|contract| contract.kind == FunEcsAdapterKind::BevyInterop)
            .expect("bevy adapter contract");

        assert_eq!(bevy.role, FunEcsAdapterRole::CompatibilityWorld);
        assert!(
            FUN_ECS_ADAPTER_CONTRACTS
                .iter()
                .any(|contract| contract.kind == FunEcsAdapterKind::Renderer)
        );
        assert!(
            FUN_ECS_ADAPTER_CONTRACTS
                .iter()
                .any(|contract| contract.kind == FunEcsAdapterKind::Rvelte)
        );
        assert!(
            FUN_ECS_ADAPTER_CONTRACTS
                .iter()
                .any(|contract| contract.kind == FunEcsAdapterKind::Telemetry
                    && contract.role == FunEcsAdapterRole::DiagnosticsSink)
        );
    }
}
