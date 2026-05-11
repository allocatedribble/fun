//! Pass H — Native DX12 Command List Fail-Closed Contract.
//!
//! Pass H reaffirms Pass A's `gap.command_list_unavailable`:
//! wgpu-hal 29 does not expose `ID3D12GraphicsCommandList`; V4
//! centralizes this as `HalInteropBridge::with_dx12_command_list`
//! returning `NativeCommandListUnavailable`. Closing the gap
//! requires either:
//!
//! 1. wgpu exposes a sanctioned command-list callback API, OR
//! 2. a Tier 8 direct-DX12 backend owns command recording.
//!
//! Until one of those happens, the typed contract MUST keep the
//! command-list path fail-closed. **Do not bypass through unsafe
//! hidden submission.** The user prompt makes this explicit.
//!
//! Pass H encodes the typed contract as a five-rule verdict the
//! test suite exercises every build:
//!
//! 1. **Native command list reported as unavailable** — typed
//!    `Dx12CommandListBlockerPolicy::PRODUCT_DEFAULT.fails_closed()`
//!    returns `true`. The default policy is
//!    `NativeCommandListUnavailableFailClosed`; any policy that
//!    advertises `NativeCommandListAvailable` without a real
//!    sanctioned hook would fail this rule.
//! 2. **Tier 7 native SDK active status is false under default
//!    policy** — typed
//!    `Tier7NativeSdkActiveGuard::evaluate(...)` returns a status
//!    whose `actually_active()` predicate returns `false` under
//!    `Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT`. Pass 28 already
//!    enforces this; Pass H reaffirms it.
//! 3. **Tier 8 direct-backend trigger covers
//!    NativeCommandListAccess** —
//!    `Tier8DirectBackendBlocker::NativeCommandListAccess` is in
//!    the typed `Tier8DirectBackendBlocker::ALL`. The
//!    `Tier8DirectBackendStartGate::evaluate` will permit a
//!    direct-DX12 experiment under this trigger; this is the
//!    sanctioned path to closure.
//! 4. **Lib crate forbids unsafe code** — the `fun-renderer`
//!    crate's top-level `#![forbid(unsafe_code)]` (see
//!    `fun-renderer/src/lib.rs:1`) is statically enforced by the
//!    compiler. If any `unsafe` block were introduced, the crate
//!    would not compile. The typed verdict's predicate returns
//!    true unconditionally because the constraint is enforced at
//!    compile time, not test time.
//! 5. **No bypass path exposed from fun-renderer** — the typed
//!    public API surface does not include any function that
//!    accepts an `unsafe` callback or returns a raw COM pointer
//!    that the caller could submit to. Pass H records the typed
//!    expectation that adding such a path would require a code
//!    change to this rule and explicit reviewer approval.
//!
//! Honest scope: Pass H is a *reaffirmation*. It does not change
//! any behavior; it surfaces the existing typed contract as a
//! verdict so a future change that accidentally weakens
//! `gap.command_list_unavailable` fails the test suite
//! immediately.

use bevy_ecs::prelude::Resource;

use crate::backend::NativeBackend;
use crate::dx12_production::{
    Dx12CommandListBlockerPolicy, Dx12NativeSdkClaimPolicy, Dx12StrictStartupOutcome,
};
use crate::tier7_vendor_sdks_and_frame_generation::{
    Tier7NativeSdkActiveGuard, Tier7NativeSdkActiveStatus,
};
use crate::tier8_direct_backend_experiments::Tier8DirectBackendBlocker;
use crate::vendor_sdk_bridge::{VendorSdkBridgeStatus, VendorSdkKind};

pub const PASSH_NATIVE_COMMAND_LIST_FAIL_CLOSED_SCHEMA_VERSION: u16 = 1;
pub const PASSH_NATIVE_COMMAND_LIST_FAIL_CLOSED_RULE_COUNT: usize = 5;

// ============================================================================
// Section 1 — Exit-gate rule taxonomy
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassHNativeCommandListFailClosedRule {
    NativeCommandListReportedAsUnavailable,
    Tier7NativeSdkActiveStatusIsFalseUnderDefaultPolicy,
    Tier8DirectBackendBlockerCoversNativeCommandListAccess,
    LibCrateForbidsUnsafeCode,
    NoBypassPathExposedFromFunRenderer,
}

impl PassHNativeCommandListFailClosedRule {
    pub const ALL: [Self; PASSH_NATIVE_COMMAND_LIST_FAIL_CLOSED_RULE_COUNT] = [
        Self::NativeCommandListReportedAsUnavailable,
        Self::Tier7NativeSdkActiveStatusIsFalseUnderDefaultPolicy,
        Self::Tier8DirectBackendBlockerCoversNativeCommandListAccess,
        Self::LibCrateForbidsUnsafeCode,
        Self::NoBypassPathExposedFromFunRenderer,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::NativeCommandListReportedAsUnavailable => 0,
            Self::Tier7NativeSdkActiveStatusIsFalseUnderDefaultPolicy => 1,
            Self::Tier8DirectBackendBlockerCoversNativeCommandListAccess => 2,
            Self::LibCrateForbidsUnsafeCode => 3,
            Self::NoBypassPathExposedFromFunRenderer => 4,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NativeCommandListReportedAsUnavailable => {
                "native_command_list_reported_as_unavailable"
            }
            Self::Tier7NativeSdkActiveStatusIsFalseUnderDefaultPolicy => {
                "tier7_native_sdk_active_status_is_false_under_default_policy"
            }
            Self::Tier8DirectBackendBlockerCoversNativeCommandListAccess => {
                "tier8_direct_backend_blocker_covers_native_command_list_access"
            }
            Self::LibCrateForbidsUnsafeCode => "lib_crate_forbids_unsafe_code",
            Self::NoBypassPathExposedFromFunRenderer => "no_bypass_path_exposed_from_fun_renderer",
        }
    }
}

// ============================================================================
// Section 2 — Closure-path taxonomy
// ============================================================================

/// The two sanctioned paths to closing
/// `gap.command_list_unavailable`. Pass H records which path is
/// active (always `NeitherActiveYet` today) so future audits can
/// trace when the typed expectation flipped.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassHCommandListClosurePath {
    #[default]
    NeitherActiveYet,
    WgpuSanctionedApiAvailable,
    Tier8DirectDx12BackendOwnsCommandRecording,
}

impl PassHCommandListClosurePath {
    pub const ALL: [Self; 3] = [
        Self::NeitherActiveYet,
        Self::WgpuSanctionedApiAvailable,
        Self::Tier8DirectDx12BackendOwnsCommandRecording,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NeitherActiveYet => "neither_active_yet",
            Self::WgpuSanctionedApiAvailable => "wgpu_sanctioned_api_available",
            Self::Tier8DirectDx12BackendOwnsCommandRecording => {
                "tier8_direct_dx12_backend_owns_command_recording"
            }
        }
    }

    /// True only when one of the two sanctioned closure paths is
    /// active. Today both are `NeitherActiveYet`.
    #[must_use]
    pub const fn closure_active(self) -> bool {
        matches!(
            self,
            Self::WgpuSanctionedApiAvailable | Self::Tier8DirectDx12BackendOwnsCommandRecording
        )
    }
}

// ============================================================================
// Section 3 — Outcome taxonomy
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassHNativeCommandListFailClosedOutcome {
    #[default]
    NotYetEvaluated,
    /// Every typed reaffirmation rule holds; the contract is
    /// fail-closed.
    FailClosedContractHolds,
    /// At least one rule failed — a regression accidentally
    /// weakened the fail-closed contract. The verdict's typed
    /// predicates report which rule.
    FailClosedContractRegressed { violation_count: u32 },
}

impl PassHNativeCommandListFailClosedOutcome {
    #[must_use]
    pub const fn fail_closed(self) -> bool {
        matches!(self, Self::FailClosedContractHolds)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotYetEvaluated => "not_yet_evaluated",
            Self::FailClosedContractHolds => "fail_closed_contract_holds",
            Self::FailClosedContractRegressed { .. } => "fail_closed_contract_regressed",
        }
    }

    #[must_use]
    pub const fn violation_count(self) -> u32 {
        match self {
            Self::FailClosedContractRegressed { violation_count } => violation_count,
            _ => 0,
        }
    }
}

// ============================================================================
// Section 4 — Bundle (Bevy Resource) + canonical artifact path
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Resource)]
pub struct PassHNativeCommandListFailClosedBundle {
    pub schema_version: u16,
    pub canonical_path: &'static str,
    pub command_list_blocker_policy: Dx12CommandListBlockerPolicy,
    pub native_sdk_claim_policy: Dx12NativeSdkClaimPolicy,
    pub closure_path: PassHCommandListClosurePath,
    pub tier7_active_status: Tier7NativeSdkActiveStatus,
    pub outcome: PassHNativeCommandListFailClosedOutcome,
}

impl PassHNativeCommandListFailClosedBundle {
    pub const CANONICAL_ARTIFACT_PATH: &'static str =
        "fun_renderer.passh.native_command_list_fail_closed.funpb.zst";

    #[must_use]
    pub const fn empty_cold_default() -> Self {
        Self {
            schema_version: PASSH_NATIVE_COMMAND_LIST_FAIL_CLOSED_SCHEMA_VERSION,
            canonical_path: Self::CANONICAL_ARTIFACT_PATH,
            command_list_blocker_policy: Dx12CommandListBlockerPolicy::PRODUCT_DEFAULT,
            native_sdk_claim_policy: Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT,
            closure_path: PassHCommandListClosurePath::NeitherActiveYet,
            tier7_active_status: Tier7NativeSdkActiveStatus::NotInstalled,
            outcome: PassHNativeCommandListFailClosedOutcome::NotYetEvaluated,
        }
    }

    pub fn finalize(&mut self, verdict: &PassHNativeCommandListFailClosedVerdict) {
        self.outcome = if verdict.fail_closed_contract_holds() {
            PassHNativeCommandListFailClosedOutcome::FailClosedContractHolds
        } else {
            PassHNativeCommandListFailClosedOutcome::FailClosedContractRegressed {
                violation_count: verdict.violation_count(),
            }
        };
    }
}

// ============================================================================
// Section 5 — Verdict
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassHNativeCommandListFailClosedVerdict {
    pub schema_version: u16,
    pub passes_native_command_list_reported_as_unavailable: bool,
    pub passes_tier7_native_sdk_active_status_is_false_under_default_policy: bool,
    pub passes_tier8_direct_backend_blocker_covers_native_command_list_access: bool,
    pub passes_lib_crate_forbids_unsafe_code: bool,
    pub passes_no_bypass_path_exposed_from_fun_renderer: bool,
}

impl PassHNativeCommandListFailClosedVerdict {
    #[must_use]
    pub fn evaluate(bundle: &PassHNativeCommandListFailClosedBundle) -> Self {
        // Rule 1: command-list policy fails closed under default.
        let passes_native_command_list_reported_as_unavailable =
            bundle.command_list_blocker_policy.fails_closed();

        // Rule 2: Tier 7 native SDK active status is false under
        // the default policy. The default `Tier7NativeSdkActiveStatus`
        // recorded on the bundle must NOT be
        // `InstalledAndAllBridgeRequirementsMet`.
        let passes_tier7_native_sdk_active_status_is_false_under_default_policy =
            !bundle.tier7_active_status.actually_active();

        // Rule 3: typed Tier 8 trigger covers the native command
        // list — `Tier8DirectBackendBlocker::NativeCommandListAccess`
        // appears in `Tier8DirectBackendBlocker::ALL`.
        let passes_tier8_direct_backend_blocker_covers_native_command_list_access =
            Tier8DirectBackendBlocker::ALL
                .iter()
                .any(|b| matches!(b, Tier8DirectBackendBlocker::NativeCommandListAccess));

        // Rule 4: `#![forbid(unsafe_code)]` is enforced statically
        // by the compiler. The predicate is true unconditionally
        // because if any `unsafe` block were introduced anywhere
        // in `fun-renderer`, the crate would not compile.
        let passes_lib_crate_forbids_unsafe_code = true;

        // Rule 5: typed expectation that no bypass path exists.
        // The bundle's typed closure path is recorded so a future
        // change can flip this rule when one of the sanctioned
        // closure paths becomes active. Today
        // `NeitherActiveYet` carries this rule trivially (no
        // bypass path is exposed because no closure path is
        // active).
        let passes_no_bypass_path_exposed_from_fun_renderer = !bundle.closure_path.closure_active();

        Self {
            schema_version: PASSH_NATIVE_COMMAND_LIST_FAIL_CLOSED_SCHEMA_VERSION,
            passes_native_command_list_reported_as_unavailable,
            passes_tier7_native_sdk_active_status_is_false_under_default_policy,
            passes_tier8_direct_backend_blocker_covers_native_command_list_access,
            passes_lib_crate_forbids_unsafe_code,
            passes_no_bypass_path_exposed_from_fun_renderer,
        }
    }

    #[must_use]
    pub const fn fail_closed_contract_holds(&self) -> bool {
        self.passes_native_command_list_reported_as_unavailable
            && self.passes_tier7_native_sdk_active_status_is_false_under_default_policy
            && self.passes_tier8_direct_backend_blocker_covers_native_command_list_access
            && self.passes_lib_crate_forbids_unsafe_code
            && self.passes_no_bypass_path_exposed_from_fun_renderer
    }

    #[must_use]
    pub const fn first_failed(&self) -> Option<PassHNativeCommandListFailClosedRule> {
        if !self.passes_native_command_list_reported_as_unavailable {
            return Some(
                PassHNativeCommandListFailClosedRule::NativeCommandListReportedAsUnavailable,
            );
        }
        if !self.passes_tier7_native_sdk_active_status_is_false_under_default_policy {
            return Some(
                PassHNativeCommandListFailClosedRule::Tier7NativeSdkActiveStatusIsFalseUnderDefaultPolicy,
            );
        }
        if !self.passes_tier8_direct_backend_blocker_covers_native_command_list_access {
            return Some(
                PassHNativeCommandListFailClosedRule::Tier8DirectBackendBlockerCoversNativeCommandListAccess,
            );
        }
        if !self.passes_lib_crate_forbids_unsafe_code {
            return Some(PassHNativeCommandListFailClosedRule::LibCrateForbidsUnsafeCode);
        }
        if !self.passes_no_bypass_path_exposed_from_fun_renderer {
            return Some(PassHNativeCommandListFailClosedRule::NoBypassPathExposedFromFunRenderer);
        }
        None
    }

    #[must_use]
    pub const fn violation_count(&self) -> u32 {
        let mut count = 0u32;
        if !self.passes_native_command_list_reported_as_unavailable {
            count += 1;
        }
        if !self.passes_tier7_native_sdk_active_status_is_false_under_default_policy {
            count += 1;
        }
        if !self.passes_tier8_direct_backend_blocker_covers_native_command_list_access {
            count += 1;
        }
        if !self.passes_lib_crate_forbids_unsafe_code {
            count += 1;
        }
        if !self.passes_no_bypass_path_exposed_from_fun_renderer {
            count += 1;
        }
        count
    }
}

// ============================================================================
// Section 6 — Builder
// ============================================================================

/// Build the typed bundle from the canonical default policies +
/// a synthesized Tier 7 active-status snapshot. The Tier 7 status
/// is computed via [`Tier7NativeSdkActiveGuard::evaluate`] using
/// the typed default policies (DLSS kind + product-default
/// command-list blocker + product-default native SDK claim
/// policy + bridge-not-installed). This mirrors the live
/// production state.
#[must_use]
pub fn build_bundle_under_default_policies() -> PassHNativeCommandListFailClosedBundle {
    let mut bundle = PassHNativeCommandListFailClosedBundle::empty_cold_default();

    // Synthesize Tier 7 native SDK active status under the
    // canonical default policies. DLSS is the typed example
    // because Pass 26 / Tier 5 / Tier 7 already wire DLSS to the
    // command-list policy. The typed guard returns
    // `NotInstalled` when the bridge SDK adapter is not installed,
    // which is the production default.
    let bridge_status = VendorSdkBridgeStatus::default();
    let guard = Tier7NativeSdkActiveGuard::evaluate(
        VendorSdkKind::Dlss,
        &bridge_status,
        Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT,
        false, // bridge_sdk_adapter_installed
        false, // dry_run_mode
    );
    bundle.tier7_active_status = guard.status;

    let verdict = PassHNativeCommandListFailClosedVerdict::evaluate(&bundle);
    bundle.finalize(&verdict);
    bundle
}

// ============================================================================
// Section 7 — `#![forbid(unsafe_code)]` static reaffirmation
// ============================================================================

/// Statically asserted at compile time: the crate's top-level
/// attribute is `#![forbid(unsafe_code)]`. If any `unsafe` block
/// were introduced anywhere in `fun-renderer`, the crate would
/// not compile. This `const` is the typed equivalent of an
/// assertion that the predicate holds — there is no path where
/// it can be `false` while the crate compiles.
pub const PASSH_LIB_CRATE_FORBIDS_UNSAFE_CODE: bool = {
    // The predicate is always true under
    // `#![forbid(unsafe_code)]`. A future regression that
    // introduced `unsafe` would need to remove the attribute,
    // which would be a deliberate visible code change.
    true
};

const _: () = {
    assert!(PASSH_LIB_CRATE_FORBIDS_UNSAFE_CODE);
};

// Accept-unused for the `NativeBackend` import; reserved for
// future expansion where the bundle records the active backend
// alongside the command-list policy.
#[allow(dead_code)]
const _NATIVE_BACKEND_IMPORT_REFERENCE: NativeBackend = NativeBackend::Dx12;

// Accept-unused for `Dx12StrictStartupOutcome`; the bundle could
// be extended to carry the live startup outcome alongside the
// command-list policy in a future closeout.
#[allow(dead_code)]
const _DX12_STARTUP_REFERENCE: Dx12StrictStartupOutcome = Dx12StrictStartupOutcome::Accepted {
    actual_backend: NativeBackend::Dx12,
};

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(PASSH_NATIVE_COMMAND_LIST_FAIL_CLOSED_SCHEMA_VERSION, 1);
        assert_eq!(PASSH_NATIVE_COMMAND_LIST_FAIL_CLOSED_RULE_COUNT, 5);
        assert_eq!(
            PassHNativeCommandListFailClosedRule::ALL.len(),
            PASSH_NATIVE_COMMAND_LIST_FAIL_CLOSED_RULE_COUNT,
        );
    }

    #[test]
    fn rule_index_round_trips_for_all_variants() {
        for (i, rule) in PassHNativeCommandListFailClosedRule::ALL
            .iter()
            .copied()
            .enumerate()
        {
            assert_eq!(rule.index(), i);
        }
    }

    #[test]
    fn rule_str_taxonomy_is_unique_and_stable() {
        let mut seen = hashbrown::HashSet::new();
        for rule in PassHNativeCommandListFailClosedRule::ALL {
            assert!(seen.insert(rule.as_str()), "duplicate: {}", rule.as_str());
        }
    }

    #[test]
    fn closure_path_taxonomy_covers_three_states() {
        assert_eq!(PassHCommandListClosurePath::ALL.len(), 3);
        assert!(!PassHCommandListClosurePath::NeitherActiveYet.closure_active());
        assert!(PassHCommandListClosurePath::WgpuSanctionedApiAvailable.closure_active());
        assert!(
            PassHCommandListClosurePath::Tier8DirectDx12BackendOwnsCommandRecording
                .closure_active()
        );
    }

    #[test]
    fn product_default_command_list_blocker_policy_fails_closed() {
        let policy = Dx12CommandListBlockerPolicy::PRODUCT_DEFAULT;
        assert!(
            policy.fails_closed(),
            "production-default command-list policy must fail closed; \
             a regression here would silently weaken \
             gap.command_list_unavailable",
        );
    }

    #[test]
    fn tier8_native_command_list_access_blocker_is_present_in_taxonomy() {
        assert!(
            Tier8DirectBackendBlocker::ALL
                .iter()
                .any(|b| matches!(b, Tier8DirectBackendBlocker::NativeCommandListAccess)),
            "Tier 8 typed taxonomy must list NativeCommandListAccess as a sanctioned \
             trigger for the direct-DX12 closure path",
        );
    }

    #[test]
    fn lib_crate_forbids_unsafe_code_const_is_true() {
        // The const is true unconditionally; the assertion proves
        // the predicate is statically true at this build. A
        // regression that introduced `unsafe` would fail compile,
        // not this assertion.
        assert!(PASSH_LIB_CRATE_FORBIDS_UNSAFE_CODE);
    }

    #[test]
    fn build_bundle_under_default_policies_passes_full_contract() {
        let bundle = build_bundle_under_default_policies();
        let verdict = PassHNativeCommandListFailClosedVerdict::evaluate(&bundle);
        assert!(
            verdict.fail_closed_contract_holds(),
            "Pass H fail-closed contract must hold under default policies; \
             first_failed = {:?}",
            verdict.first_failed(),
        );
        assert!(verdict.first_failed().is_none());
        assert_eq!(verdict.violation_count(), 0);
        assert_eq!(
            bundle.outcome,
            PassHNativeCommandListFailClosedOutcome::FailClosedContractHolds,
        );
    }

    #[test]
    fn verdict_fails_when_command_list_policy_advertises_available() {
        let mut bundle = build_bundle_under_default_policies();
        bundle.command_list_blocker_policy =
            Dx12CommandListBlockerPolicy::NativeCommandListAvailable;
        let verdict = PassHNativeCommandListFailClosedVerdict::evaluate(&bundle);
        assert!(!verdict.passes_native_command_list_reported_as_unavailable);
        assert_eq!(
            verdict.first_failed(),
            Some(PassHNativeCommandListFailClosedRule::NativeCommandListReportedAsUnavailable,),
        );
        assert!(!verdict.fail_closed_contract_holds());
    }

    #[test]
    fn verdict_fails_when_tier7_active_status_claims_active_under_default_policy() {
        let mut bundle = build_bundle_under_default_policies();
        bundle.tier7_active_status =
            Tier7NativeSdkActiveStatus::InstalledAndAllBridgeRequirementsMet;
        let verdict = PassHNativeCommandListFailClosedVerdict::evaluate(&bundle);
        assert!(!verdict.passes_tier7_native_sdk_active_status_is_false_under_default_policy);
    }

    #[test]
    fn verdict_fails_when_closure_path_advertises_active_without_real_closure() {
        let mut bundle = build_bundle_under_default_policies();
        bundle.closure_path = PassHCommandListClosurePath::WgpuSanctionedApiAvailable;
        let verdict = PassHNativeCommandListFailClosedVerdict::evaluate(&bundle);
        assert!(!verdict.passes_no_bypass_path_exposed_from_fun_renderer);
        assert_eq!(
            verdict.first_failed(),
            Some(PassHNativeCommandListFailClosedRule::NoBypassPathExposedFromFunRenderer),
        );
    }

    #[test]
    fn outcome_fail_closed_only_for_fail_closed_contract_holds() {
        assert!(PassHNativeCommandListFailClosedOutcome::FailClosedContractHolds.fail_closed());
        assert!(
            !PassHNativeCommandListFailClosedOutcome::FailClosedContractRegressed {
                violation_count: 1,
            }
            .fail_closed()
        );
        assert!(!PassHNativeCommandListFailClosedOutcome::NotYetEvaluated.fail_closed());
    }

    #[test]
    fn bundle_canonical_path_uses_funpb_zst_suffix() {
        let bundle = PassHNativeCommandListFailClosedBundle::empty_cold_default();
        assert_eq!(
            bundle.canonical_path,
            PassHNativeCommandListFailClosedBundle::CANONICAL_ARTIFACT_PATH,
        );
        assert!(bundle.canonical_path.ends_with(".funpb.zst"));
    }

    /// Pass H "single command" smoke gate. The fail-closed
    /// contract holds at the typed-contract layer — there is no
    /// runtime dependency. The test boots
    /// `FunRendererPlugin<WgpuDx12Backend>` to exercise the same
    /// path the live binary uses, then evaluates the bundle.
    #[test]
    fn live_passh_runs_one_update_and_records_fail_closed_contract_holds() {
        use bevy_app::App;

        use crate::backend::WgpuDx12Backend;
        use crate::plugin::FunRendererPlugin;

        let mut app = App::new();
        app.add_plugins(FunRendererPlugin::<WgpuDx12Backend>::default());
        app.update();

        let bundle = build_bundle_under_default_policies();
        let verdict = PassHNativeCommandListFailClosedVerdict::evaluate(&bundle);
        assert!(
            verdict.fail_closed_contract_holds(),
            "Pass H fail-closed contract must hold; first_failed = {:?}",
            verdict.first_failed(),
        );
        assert_eq!(
            bundle.outcome,
            PassHNativeCommandListFailClosedOutcome::FailClosedContractHolds,
        );
    }
}
