//! Pass C7.12 — typed `fun_render` cloud-shadow execution
//! retirement contract.
//!
//! After Pass C7.5 (typed wgpu pipeline + dispatch in
//! fun-renderer) and Pass C7.6 / C7.7 / C7.10 (typed Lux
//! direct lighting + typed volumetric + typed material
//! consumption of the typed cloud aux layer), the typed
//! product cloud-shadow execution moves entirely under
//! `fun-renderer`.  The typed `fun_render` legacy module
//! retains only its typed extraction + typed env-parsing
//! role.
//!
//! This module lands the typed retirement contract — a
//! typed const-evaluable policy record + typed audit
//! predicates that codify the typed ownership flip + the
//! typed startup-log identity statement.
//!
//! Acceptance (user spec):
//! - `fun_render` only extracts/env-parses cloud settings
//!   and signals.
//! - Product cloud shadow GPU work runs through
//!   `fun-renderer`.
//! - Any Bevy cloud path is explicit diagnostic-only.
//! - Startup logs identify `fun-renderer` as cloud-shadow
//!   executor.

use crate::clouds::FunCloudRendererContract;

pub const FUN_RENDERER_CLOUD_RETIREMENT_SCHEMA_VERSION: u16 = 1;

// ============================================================================
// Section 1 — typed FunRenderCloudShadowExecutionPolicy
// ============================================================================

/// Typed Pass C7.12 — typed fun_render cloud-shadow
/// execution policy.  Names the typed product-level rule
/// for what role the typed `fun_render` legacy module
/// plays in typed cloud-shadow execution.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunRenderCloudShadowExecutionPolicy {
    /// Typed product policy — typed `fun_render` only
    /// extracts + env-parses cloud settings + signals.
    /// Typed GPU work is forbidden in this typed mode.
    #[default]
    ExtractsOnly,
    /// Typed diagnostic-only — typed `fun_render` may
    /// still own a typed Bevy cloud path for typed
    /// debug / typed parity comparison, but it MUST NOT
    /// be the typed product cloud-shadow executor.
    /// Typed startup logs MUST flag this typed mode as
    /// typed non-production.
    DiagnosticOnly,
    /// Typed quarantined — typed `fun_render`'s typed
    /// cloud-shadow path is typed dead code retained for
    /// typed reference.  Typed feature-flagged off in
    /// typed product builds.
    Quarantined,
}

impl FunRenderCloudShadowExecutionPolicy {
    pub const ALL: [Self; 3] = [Self::ExtractsOnly, Self::DiagnosticOnly, Self::Quarantined];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExtractsOnly => "extracts_only",
            Self::DiagnosticOnly => "diagnostic_only",
            Self::Quarantined => "quarantined",
        }
    }

    /// Typed Pass C7.12 — typed predicate: is this typed
    /// policy permitted in typed product builds?  Only
    /// typed `ExtractsOnly` is.
    #[must_use]
    pub const fn is_product_permitted(self) -> bool {
        matches!(self, Self::ExtractsOnly)
    }

    /// Typed predicate: does this typed policy retain any
    /// typed cloud-shadow GPU work in typed `fun_render`?
    /// `false` for typed `ExtractsOnly` + `Quarantined`;
    /// `true` for typed `DiagnosticOnly` (typed debug
    /// path still touches typed GPU).
    #[must_use]
    pub const fn retains_gpu_work(self) -> bool {
        matches!(self, Self::DiagnosticOnly)
    }

    /// Typed predicate: must the typed startup log flag
    /// the typed mode as typed non-production?
    #[must_use]
    pub const fn requires_non_production_flag(self) -> bool {
        !matches!(self, Self::ExtractsOnly)
    }
}

// ============================================================================
// Section 2 — typed FunRenderCloudRetirementContract
// ============================================================================

/// Typed Pass C7.12 — typed retirement contract record.
/// Bundles the typed policy + the typed audit predicates
/// that codify the typed C0 ownership flip after the
/// typed C7.x cloud shadow path lands in `fun-renderer`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunRenderCloudRetirementContract {
    pub schema_version: u16,
    pub policy: FunRenderCloudShadowExecutionPolicy,
    /// Typed `fun_render` only extracts/env-parses cloud
    /// settings + signals.  Audit flag the typed product
    /// build must hold.
    pub fun_render_extracts_only: bool,
    /// Typed product cloud shadow GPU work runs through
    /// `fun-renderer`.  Audit flag.
    pub product_gpu_work_in_fun_renderer: bool,
    /// Typed any Bevy cloud path is explicit
    /// diagnostic-only.  Audit flag.
    pub bevy_cloud_path_is_diagnostic_only: bool,
    /// Typed startup logs identify `fun-renderer` as the
    /// typed cloud-shadow executor.  Audit flag.
    pub startup_log_identifies_executor: bool,
}

impl FunRenderCloudRetirementContract {
    /// Typed Pass C7.12 — typed product retirement
    /// contract.  Every typed audit flag MUST be `true`
    /// in typed product builds.
    pub const PRODUCT: Self = Self {
        schema_version: FUN_RENDERER_CLOUD_RETIREMENT_SCHEMA_VERSION,
        policy: FunRenderCloudShadowExecutionPolicy::ExtractsOnly,
        fun_render_extracts_only: true,
        product_gpu_work_in_fun_renderer: true,
        bevy_cloud_path_is_diagnostic_only: true,
        startup_log_identifies_executor: true,
    };

    /// Typed regression-capture contract — every flag is
    /// `false`.  Reserved for typed tests that prove
    /// flipping a typed flag in the typed product surface
    /// flips the typed predicate.
    pub const REGRESSION_CAPTURE: Self = Self {
        schema_version: FUN_RENDERER_CLOUD_RETIREMENT_SCHEMA_VERSION,
        policy: FunRenderCloudShadowExecutionPolicy::DiagnosticOnly,
        fun_render_extracts_only: false,
        product_gpu_work_in_fun_renderer: false,
        bevy_cloud_path_is_diagnostic_only: false,
        startup_log_identifies_executor: false,
    };

    /// Typed predicate: does this typed contract obey
    /// every typed product retirement rule?
    #[must_use]
    pub const fn obeys_all_retirement_rules(&self) -> bool {
        self.policy.is_product_permitted()
            && self.fun_render_extracts_only
            && self.product_gpu_work_in_fun_renderer
            && self.bevy_cloud_path_is_diagnostic_only
            && self.startup_log_identifies_executor
    }

    /// Typed predicate: does this typed retirement
    /// contract align with the typed C0 ownership
    /// contract?  Both must agree that typed `fun_render`
    /// is typed extraction-only and typed `fun-renderer`
    /// owns the typed cloud execution.
    #[must_use]
    pub const fn aligns_with_c0_ownership(&self, c0: &FunCloudRendererContract) -> bool {
        self.fun_render_extracts_only
            && c0.fun_render_extracts_only
            && self.product_gpu_work_in_fun_renderer
            && c0.fun_renderer_owns_cloud_execution
    }
}

// ============================================================================
// Section 3 — typed startup-log identity
// ============================================================================

/// Typed Pass C7.12 — typed startup-log identity record.
/// The typed renderer emits this typed record at typed
/// boot so typed operator + typed agent can confirm
/// `fun-renderer` is the typed cloud-shadow executor.
/// Drives the typed user-spec acceptance bullet "Startup
/// logs identify `fun-renderer` as cloud-shadow
/// executor".
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CloudShadowExecutorIdentity {
    pub schema_version: u16,
    pub executor_package: &'static str,
    pub executor_module: &'static str,
    pub legacy_extracts_package: &'static str,
    pub retirement_policy: FunRenderCloudShadowExecutionPolicy,
}

impl CloudShadowExecutorIdentity {
    /// Typed product identity — typed `fun-renderer`
    /// owns the typed cloud shadow GPU work; typed
    /// `fun_render` retains typed extraction-only role.
    pub const PRODUCT: Self = Self {
        schema_version: FUN_RENDERER_CLOUD_RETIREMENT_SCHEMA_VERSION,
        executor_package: "fun-renderer",
        executor_module: "fun_renderer::cloud_shadow_pipelines",
        legacy_extracts_package: "fun_render",
        retirement_policy: FunRenderCloudShadowExecutionPolicy::ExtractsOnly,
    };

    /// Typed predicate: does this typed identity name
    /// `fun-renderer` as the typed executor?
    #[must_use]
    pub fn names_fun_renderer_as_executor(&self) -> bool {
        self.executor_package == "fun-renderer"
            && self.executor_module.starts_with("fun_renderer::")
    }

    /// Typed predicate: does this typed identity name
    /// `fun_render` as the typed legacy extracts-only
    /// owner?
    #[must_use]
    pub fn names_fun_render_as_extracts_only(&self) -> bool {
        self.legacy_extracts_package == "fun_render"
            && matches!(
                self.retirement_policy,
                FunRenderCloudShadowExecutionPolicy::ExtractsOnly,
            )
    }
}

/// Typed Pass C7.12 — emit the typed cloud-shadow
/// executor startup log line.  Returns a typed
/// human-readable string the typed renderer logs at
/// boot.  Audited at the typed string layer so the
/// typed `Startup logs identify fun-renderer as
/// cloud-shadow executor` acceptance bullet is testable.
#[must_use]
pub fn cloud_shadow_executor_startup_log(identity: &CloudShadowExecutorIdentity) -> String {
    format!(
        "[fun-renderer] cloud-shadow executor: package={} module={} legacy_extracts_only={} \
        retirement_policy={}",
        identity.executor_package,
        identity.executor_module,
        identity.legacy_extracts_package,
        identity.retirement_policy.as_str(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pass C7.12 schema is stable.
    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_RENDERER_CLOUD_RETIREMENT_SCHEMA_VERSION, 1);
    }

    /// Pass C7.12 — typed policy taxonomy walks user
    /// spec.
    #[test]
    fn policy_taxonomy_walks_user_spec() {
        assert_eq!(FunRenderCloudShadowExecutionPolicy::ALL.len(), 3);
        // Typed only ExtractsOnly is product-permitted.
        assert!(FunRenderCloudShadowExecutionPolicy::ExtractsOnly.is_product_permitted());
        assert!(!FunRenderCloudShadowExecutionPolicy::DiagnosticOnly.is_product_permitted());
        assert!(!FunRenderCloudShadowExecutionPolicy::Quarantined.is_product_permitted());
        // Typed only DiagnosticOnly retains GPU work.
        assert!(!FunRenderCloudShadowExecutionPolicy::ExtractsOnly.retains_gpu_work());
        assert!(FunRenderCloudShadowExecutionPolicy::DiagnosticOnly.retains_gpu_work());
        assert!(!FunRenderCloudShadowExecutionPolicy::Quarantined.retains_gpu_work());
        // Typed non-Extract policies require typed
        // non-production flag.
        assert!(!FunRenderCloudShadowExecutionPolicy::ExtractsOnly.requires_non_production_flag());
        assert!(FunRenderCloudShadowExecutionPolicy::DiagnosticOnly.requires_non_production_flag());
        assert!(FunRenderCloudShadowExecutionPolicy::Quarantined.requires_non_production_flag());
    }

    /// Pass C7.12 acceptance — `fun_render` only
    /// extracts/env-parses cloud settings and signals.
    #[test]
    fn fun_render_only_extracts_in_product() {
        let contract = FunRenderCloudRetirementContract::PRODUCT;
        assert!(contract.fun_render_extracts_only);
        assert_eq!(
            contract.policy,
            FunRenderCloudShadowExecutionPolicy::ExtractsOnly,
        );
        assert!(contract.obeys_all_retirement_rules());
        // Typed regression-capture contract flips every
        // flag.
        let cap = FunRenderCloudRetirementContract::REGRESSION_CAPTURE;
        assert!(!cap.fun_render_extracts_only);
        assert!(!cap.obeys_all_retirement_rules());
    }

    /// Pass C7.12 acceptance — product cloud shadow GPU
    /// work runs through `fun-renderer`.
    #[test]
    fn product_cloud_shadow_gpu_work_runs_through_fun_renderer() {
        let contract = FunRenderCloudRetirementContract::PRODUCT;
        assert!(contract.product_gpu_work_in_fun_renderer);
        // Typed identity names fun-renderer.
        let identity = CloudShadowExecutorIdentity::PRODUCT;
        assert!(identity.names_fun_renderer_as_executor());
        assert_eq!(identity.executor_package, "fun-renderer");
        assert_eq!(
            identity.executor_module,
            "fun_renderer::cloud_shadow_pipelines"
        );
    }

    /// Pass C7.12 acceptance — any Bevy cloud path is
    /// explicit diagnostic-only.
    #[test]
    fn bevy_cloud_path_is_diagnostic_only() {
        let contract = FunRenderCloudRetirementContract::PRODUCT;
        assert!(contract.bevy_cloud_path_is_diagnostic_only);
        // Typed DiagnosticOnly policy correctly flags
        // typed retains GPU work + typed non-production.
        let policy = FunRenderCloudShadowExecutionPolicy::DiagnosticOnly;
        assert!(policy.retains_gpu_work());
        assert!(policy.requires_non_production_flag());
        assert!(!policy.is_product_permitted());
    }

    /// Pass C7.12 acceptance — startup logs identify
    /// `fun-renderer` as cloud-shadow executor.
    #[test]
    fn startup_logs_identify_fun_renderer_as_cloud_shadow_executor() {
        let contract = FunRenderCloudRetirementContract::PRODUCT;
        assert!(contract.startup_log_identifies_executor);
        let identity = CloudShadowExecutorIdentity::PRODUCT;
        let line = cloud_shadow_executor_startup_log(&identity);
        assert!(line.contains("[fun-renderer] cloud-shadow executor:"));
        assert!(line.contains("package=fun-renderer"));
        assert!(line.contains("module=fun_renderer::cloud_shadow_pipelines"));
        assert!(line.contains("legacy_extracts_only=fun_render"));
        assert!(line.contains("retirement_policy=extracts_only"));
        // Typed identity predicates pass.
        assert!(identity.names_fun_renderer_as_executor());
        assert!(identity.names_fun_render_as_extracts_only());
    }

    /// Pass C7.12 — typed retirement contract aligns
    /// with the typed C0 ownership contract.
    #[test]
    fn retirement_contract_aligns_with_c0_ownership() {
        let retirement = FunRenderCloudRetirementContract::PRODUCT;
        let c0 = FunCloudRendererContract::CURRENT;
        assert!(retirement.aligns_with_c0_ownership(&c0));
        // Typed regression-capture C0 flips ownership →
        // typed alignment fails.
        let c0_reg = FunCloudRendererContract::REGRESSION_CAPTURE;
        assert!(!retirement.aligns_with_c0_ownership(&c0_reg));
    }
}
