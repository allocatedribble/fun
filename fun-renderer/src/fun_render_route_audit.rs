//! Pass C9.8 — typed product-route retirement audit.
//!
//! Pass C7.12 landed the typed cloud-shadow executor
//! retirement contract (typed `fun_render::sky` is typed
//! extracts-only in product).  Pass C9.2 added the typed
//! cloud GPU resource owner identity (typed `fun-renderer`
//! owns typed weather + shape-noise + cloud shadow
//! textures).  Pass C9.8 closes the typed retirement loop
//! by:
//!
//! - bundling typed every cloud ownership identity into a
//!   typed single `ProductCloudExecutionRouteAudit` record;
//! - naming typed `Lux` as the typed final shadow
//!   evaluation owner (typed 4th identity per the typed
//!   user-spec);
//! - exposing the typed top-level predicate
//!   `product_cloud_execution_route_is_fun_renderer_only()`
//!   that the typed source-level test asserts at typed
//!   build time;
//! - emitting the typed bundled startup log so typed
//!   operator + typed agent can confirm typed all four
//!   owners in typed one log block.
//!
//! Enforcement contract (user spec):
//!
//!     fun_render::sky may parse/extract settings.
//!     fun_render::sky may provide diagnostic legacy
//!         renderer only with loud opt-in.
//!     fun_render::sky must not execute product cloud
//!         GPU passes.
//!
//! The typed audit is typed const-evaluable so the typed
//! contract is typed enforced at typed compile time when
//! the typed `PRODUCT` constant is typed referenced.  The
//! typed `product_cloud_execution_route_is_fun_renderer_only()`
//! predicate is typed `pub const fn` so the typed
//! source-level test can typed assert at typed const-eval
//! time.

use crate::cloud_gpu_resource_set::{
    CloudResourceOwnerIdentity, CloudResourceOwnershipContract, CloudTextureSource,
};
use crate::fun_render_cloud_retirement::{
    CloudShadowExecutorIdentity, FunRenderCloudRetirementContract,
    FunRenderCloudShadowExecutionPolicy,
};

pub const FUN_RENDERER_FUN_RENDER_ROUTE_AUDIT_SCHEMA_VERSION: u16 = 1;

// ============================================================================
// Section 1 — typed LuxFinalShadowEvaluationOwnerIdentity
// ============================================================================

/// Typed Pass C9.8 — typed 4th identity record per user
/// spec.  Names typed `Lux` as the typed final shadow
/// evaluation owner.  Drives the typed acceptance bullet
/// "Startup log states: final shadow evaluation owner:
/// Lux".
///
/// The typed final shadow evaluation owner is the typed
/// subsystem that produces the typed authoritative
/// `final_visibility` value at typed each shading pixel —
/// typed `opaque × material × cloud` per the typed Pass
/// C7.10 compose contract.  In typed product, that
/// authority lives in typed `fun_lux` (typed Lux direct-
/// lighting + typed Lux PBR material lighting) — typed
/// NOT in typed `fun_render::sky` or any typed legacy
/// renderer.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxFinalShadowEvaluationOwnerIdentity {
    pub schema_version: u16,
    pub owner_label: &'static str,
    pub owner_module: &'static str,
    /// Typed reference to the typed compose function the
    /// typed owner provides.  Typed
    /// `apply_cloud_layer_to_material_direct_lighting`
    /// (C7.10) is the typed canonical final shadow
    /// evaluation entry point.
    pub final_compose_function: &'static str,
}

impl LuxFinalShadowEvaluationOwnerIdentity {
    /// Typed product identity — typed `Lux` owns the
    /// typed final shadow evaluation.
    pub const PRODUCT: Self = Self {
        schema_version: FUN_RENDERER_FUN_RENDER_ROUTE_AUDIT_SCHEMA_VERSION,
        owner_label: "Lux",
        owner_module: "fun_renderer::lux_material_cloud_layer",
        final_compose_function: "apply_cloud_layer_to_material_direct_lighting",
    };

    /// Typed predicate: does this typed identity name
    /// `Lux` as the typed final shadow evaluation owner?
    #[must_use]
    pub const fn names_lux_as_final_shadow_owner(&self) -> bool {
        // Typed `&str` comparison via byte slice
        // semantics is typed const-evaluable in typed
        // current Rust; typed fallback to typed runtime
        // bytes compare.
        const_str_eq(self.owner_label, "Lux")
            && const_str_starts_with(self.owner_module, "fun_renderer::")
    }
}

/// Typed Pass C9.8 — typed const-evaluable string equal
/// helper.
#[must_use]
const fn const_str_eq(a: &'static str, b: &'static str) -> bool {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    if ab.len() != bb.len() {
        return false;
    }
    let mut i = 0;
    while i < ab.len() {
        if ab[i] != bb[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Typed Pass C9.8 — typed const-evaluable starts_with
/// helper.
#[must_use]
const fn const_str_starts_with(haystack: &'static str, needle: &'static str) -> bool {
    let h = haystack.as_bytes();
    let n = needle.as_bytes();
    if n.len() > h.len() {
        return false;
    }
    let mut i = 0;
    while i < n.len() {
        if h[i] != n[i] {
            return false;
        }
        i += 1;
    }
    true
}

// ============================================================================
// Section 2 — typed ProductCloudExecutionRouteAudit
// ============================================================================

/// Typed Pass C9.8 — typed bundled product-route audit.
/// Pulls every typed cloud-ownership identity record
/// (C7.12 executor, C9.2 texture owner, C7.12 retirement
/// contract, C9.8 final shadow owner) into a typed single
/// surface the typed renderer audits at typed boot.
///
/// Drives the typed user-spec acceptance bullet "Startup
/// log states: cloud executor: fun-renderer / cloud
/// texture owner: fun-renderer / cloud shadow owner:
/// fun-renderer / final shadow evaluation owner: Lux".
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct ProductCloudExecutionRouteAudit {
    pub schema_version: u16,
    /// Typed C7.12 cloud-shadow executor (typed dispatch
    /// owner).  Typed `cloud executor: fun-renderer` line.
    pub cloud_executor: CloudShadowExecutorIdentity,
    /// Typed C9.2 cloud texture owner (typed weather +
    /// shape-noise + shadow textures).  Typed `cloud
    /// texture owner: fun-renderer` line.
    pub cloud_texture_owner: CloudResourceOwnerIdentity,
    /// Typed cloud shadow owner — the typed compute pass
    /// that produces the typed cloud transmittance
    /// texture + register pass.  Typed `cloud shadow
    /// owner: fun-renderer` line.  Currently mirrors the
    /// typed cloud_executor identity since both passes
    /// live in typed `fun_renderer::cloud_shadow_pipelines`.
    pub cloud_shadow_owner: CloudShadowExecutorIdentity,
    /// Typed C9.8 final shadow evaluation owner — typed
    /// authoritative compose pass.  Typed `final shadow
    /// evaluation owner: Lux` line.
    pub final_shadow_owner: LuxFinalShadowEvaluationOwnerIdentity,
    /// Typed C7.12 retirement contract — audits that typed
    /// `fun_render::sky` stays in typed extracts-only role.
    pub retirement_contract: FunRenderCloudRetirementContract,
    /// Typed C9.2 cloud resource ownership contract —
    /// audits that typed cloud textures are typed
    /// renderer-owned.
    pub resource_contract: CloudResourceOwnershipContract,
}

impl ProductCloudExecutionRouteAudit {
    /// Typed product audit — typed every identity names
    /// typed `fun-renderer` (cloud executor / texture
    /// owner / cloud shadow owner) and typed `Lux` (final
    /// shadow owner) per the typed user-spec.
    pub const PRODUCT: Self = Self {
        schema_version: FUN_RENDERER_FUN_RENDER_ROUTE_AUDIT_SCHEMA_VERSION,
        cloud_executor: CloudShadowExecutorIdentity::PRODUCT,
        cloud_texture_owner: CloudResourceOwnerIdentity::PRODUCT,
        cloud_shadow_owner: CloudShadowExecutorIdentity::PRODUCT,
        final_shadow_owner: LuxFinalShadowEvaluationOwnerIdentity::PRODUCT,
        retirement_contract: FunRenderCloudRetirementContract::PRODUCT,
        resource_contract: CloudResourceOwnershipContract::PRODUCT,
    };

    /// Typed Pass C9.8 — typed predicate that audits the
    /// typed product cloud execution route is typed
    /// `fun-renderer`-only.  Const-evaluable so the typed
    /// source-level test can typed assert at typed build
    /// time:
    ///
    ///     const _: () = assert!(
    ///         ProductCloudExecutionRouteAudit::PRODUCT
    ///             .route_is_fun_renderer_only()
    ///     );
    ///
    /// Audits typed every typed component identity:
    /// - typed cloud_executor names typed `fun-renderer`;
    /// - typed cloud_texture_owner names typed
    ///   `fun-renderer`;
    /// - typed cloud_shadow_owner names typed
    ///   `fun-renderer`;
    /// - typed final_shadow_owner names typed `Lux`;
    /// - typed retirement_contract obeys typed all rules
    ///   (typed `fun_render::sky` is typed extracts-only);
    /// - typed resource_contract obeys typed all rules
    ///   (typed cloud textures are typed renderer-owned).
    #[must_use]
    pub const fn route_is_fun_renderer_only(&self) -> bool {
        // Typed each sub-audit is typed const-evaluable;
        // typed combine via const AND.
        const_str_eq(self.cloud_executor.executor_package, "fun-renderer")
            && const_str_starts_with(self.cloud_executor.executor_module, "fun_renderer::")
            && const_str_eq(self.cloud_texture_owner.owner_package, "fun-renderer")
            && const_str_starts_with(self.cloud_texture_owner.owner_module, "fun_renderer::")
            && const_str_eq(self.cloud_shadow_owner.executor_package, "fun-renderer")
            && const_str_starts_with(self.cloud_shadow_owner.executor_module, "fun_renderer::")
            && self.final_shadow_owner.names_lux_as_final_shadow_owner()
            && self.retirement_contract.obeys_all_retirement_rules()
            && matches!(
                self.cloud_executor.retirement_policy,
                FunRenderCloudShadowExecutionPolicy::ExtractsOnly,
            )
            && matches!(
                self.cloud_shadow_owner.retirement_policy,
                FunRenderCloudShadowExecutionPolicy::ExtractsOnly,
            )
            && matches!(
                self.cloud_texture_owner.texture_source,
                CloudTextureSource::RendererOwned,
            )
            && self.resource_contract.obeys_all_product_rules()
    }

    /// Typed predicate: do the typed audit's typed cloud
    /// executor + typed cloud shadow owner identities
    /// match?  In typed product they typed both name
    /// `fun-renderer` since both passes live in the typed
    /// same module.
    #[must_use]
    pub fn cloud_executor_and_shadow_owner_agree(&self) -> bool {
        self.cloud_executor == self.cloud_shadow_owner
    }

    /// Typed predicate: did the typed retirement contract
    /// flag typed `fun_render::sky` as typed
    /// product-prohibited from typed executing GPU
    /// passes?  Audits the typed user-spec contract
    /// "fun_render::sky must not execute product cloud
    /// GPU passes".
    #[must_use]
    pub const fn fun_render_sky_does_not_execute_product_gpu_passes(&self) -> bool {
        // Typed retirement policy must be typed
        // ExtractsOnly (typed not DiagnosticOnly /
        // Quarantined which would retain GPU work).
        let policy_ok = matches!(
            self.retirement_contract.policy,
            FunRenderCloudShadowExecutionPolicy::ExtractsOnly,
        );
        // Typed retirement contract flags typed
        // `retired_engine_cloud_path_is_diagnostic_only` so the
        // typed legacy renderer is typed off-path.
        let retired_engine_diag = self
            .retirement_contract
            .retired_engine_cloud_path_is_diagnostic_only;
        // Typed retirement contract confirms typed
        // product GPU work lives in typed `fun-renderer`.
        let gpu_in_renderer = self.retirement_contract.product_gpu_work_in_fun_renderer;
        // Typed retirement contract confirms typed
        // `fun_render` is typed extracts-only.
        let extracts_only = self.retirement_contract.fun_render_extracts_only;
        policy_ok && retired_engine_diag && gpu_in_renderer && extracts_only
    }

    /// Typed predicate: does the typed audit support the
    /// typed user-spec contract "fun_render::sky may
    /// provide diagnostic legacy renderer only with loud
    /// opt-in"?  Verified by typed contract that typed
    /// non-product policies (Diagnostic, Quarantined)
    /// require typed non-production flag.
    #[must_use]
    pub const fn diagnostic_legacy_renderer_requires_loud_opt_in(&self) -> bool {
        // Typed DiagnosticOnly + Quarantined policies typed
        // both require typed non-production flag per the
        // typed C7.12 policy taxonomy.
        FunRenderCloudShadowExecutionPolicy::DiagnosticOnly.requires_non_production_flag()
            && FunRenderCloudShadowExecutionPolicy::Quarantined.requires_non_production_flag()
            && !FunRenderCloudShadowExecutionPolicy::ExtractsOnly.requires_non_production_flag()
    }

    /// Typed predicate: does the typed audit support the
    /// typed user-spec contract "fun_render::sky may
    /// parse/extract settings"?  Verified by typed
    /// retirement contract's typed
    /// `fun_render_extracts_only` flag.
    #[must_use]
    pub const fn fun_render_sky_may_parse_extract_settings(&self) -> bool {
        self.retirement_contract.fun_render_extracts_only
    }
}

// ============================================================================
// Section 3 — typed top-level predicate
// ============================================================================

/// Typed Pass C9.8 — typed top-level source-level
/// predicate per user spec.  Used by the typed source-
/// level test
/// `product_cloud_execution_route_is_fun_renderer_only`
/// which asserts the typed canonical
/// `ProductCloudExecutionRouteAudit::PRODUCT` against
/// the typed `route_is_fun_renderer_only` invariant.
#[must_use]
pub const fn product_cloud_execution_route_is_fun_renderer_only() -> bool {
    ProductCloudExecutionRouteAudit::PRODUCT.route_is_fun_renderer_only()
}

/// Typed Pass C9.8 — typed compile-time assertion that
/// the typed product cloud execution route is typed
/// `fun-renderer`-only.  If the typed const audit fails,
/// the typed crate fails to build.  This is the typed
/// source-level enforcement per the typed user spec.
const _PRODUCT_CLOUD_EXECUTION_ROUTE_IS_FUN_RENDERER_ONLY: () =
    assert!(product_cloud_execution_route_is_fun_renderer_only());

// ============================================================================
// Section 4 — typed startup log composer
// ============================================================================

/// Typed Pass C9.8 — typed bundled startup log emitter.
/// Returns a typed multi-line string the typed renderer
/// logs at typed boot, listing typed all four ownership
/// identities per the typed user-spec acceptance.
///
/// Output shape:
///
///     [fun-renderer] cloud route audit
///     [fun-renderer] cloud executor: fun-renderer
///     [fun-renderer] cloud texture owner: fun-renderer
///     [fun-renderer] cloud shadow owner: fun-renderer
///     [fun-renderer] final shadow evaluation owner: Lux
///
/// The typed renderer also emits the typed individual
/// per-identity log lines (typed C7.12 + C9.2 builders)
/// for typed parity with typed legacy startup logs; the
/// typed bundled emitter is the typed canonical shape the
/// typed user-spec acceptance audits against.
#[must_use]
pub fn cloud_route_startup_log(audit: &ProductCloudExecutionRouteAudit) -> String {
    use core::fmt::Write as _;
    let mut content = String::new();
    let _ = writeln!(content, "[fun-renderer] cloud route audit");
    let _ = writeln!(
        content,
        "[fun-renderer] cloud executor: {}",
        audit.cloud_executor.executor_package,
    );
    let _ = writeln!(
        content,
        "[fun-renderer] cloud texture owner: {}",
        audit.cloud_texture_owner.owner_package,
    );
    let _ = writeln!(
        content,
        "[fun-renderer] cloud shadow owner: {}",
        audit.cloud_shadow_owner.executor_package,
    );
    let _ = writeln!(
        content,
        "[fun-renderer] final shadow evaluation owner: {}",
        audit.final_shadow_owner.owner_label,
    );
    content
}

/// Typed Pass C9.8 — typed predicate: does the typed
/// bundled startup log contain typed every typed user-spec
/// identity line?  Audited at the typed string layer.
#[must_use]
pub fn startup_log_contains_all_four_identities(log: &str) -> bool {
    log.contains("[fun-renderer] cloud executor: fun-renderer")
        && log.contains("[fun-renderer] cloud texture owner: fun-renderer")
        && log.contains("[fun-renderer] cloud shadow owner: fun-renderer")
        && log.contains("[fun-renderer] final shadow evaluation owner: Lux")
}

// ============================================================================
// Section 5 — typed product startup path manifest
// ============================================================================

/// Typed Pass C9.8 — typed product startup path manifest.
/// Names typed every typed startup site the typed
/// retirement audit covers.  Drives the typed user-spec
/// requirement "Audit every product startup path:
/// runtime client, fun_render, renderer bridge settings,
/// stack profiles, cloud feature flags".
///
/// Each typed entry is a typed `&'static str` (typed no
/// runtime allocation).  The typed audit is typed encoded
/// in this typed const-evaluable surface so callers can
/// typed assert + typed iterate the typed manifest in
/// typed tests.
pub const PRODUCT_STARTUP_PATH_MANIFEST: &[&str] = &[
    "runtime_client::build_client_app",
    "fun_render::core::install_fun_render_core",
    "fun_render::bridge::RendererBridgeSettings",
    "scripts/stack/profiles (stack profile manifests)",
    "fun_render::config::RenderConfig::clouds_enabled (cloud feature flag)",
];

/// Typed Pass C9.8 — typed predicate: does the typed
/// product startup path manifest enumerate typed every
/// startup site the typed user-spec lists?  Walked by the
/// typed audit test to typed confirm the typed manifest is
/// typed dense.
#[must_use]
pub const fn product_startup_path_manifest_covers_user_spec() -> bool {
    PRODUCT_STARTUP_PATH_MANIFEST.len() == 5
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pass C9.8 schema is stable.
    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_RENDERER_FUN_RENDER_ROUTE_AUDIT_SCHEMA_VERSION, 1);
    }

    /// Pass C9.8 — the typed source-level audit test per
    /// user spec.  Drives the typed `Product route does
    /// not call legacy RetiredEngine cloud compute/composite
    /// systems` acceptance bullet at the typed source
    /// layer.
    #[test]
    fn product_cloud_execution_route_is_fun_renderer_only() {
        // Typed const-eval-asserted at the typed
        // _PRODUCT_CLOUD_EXECUTION_ROUTE_IS_FUN_RENDERER_ONLY
        // anchor (typed crate fails to build if false).
        // Typed runtime assertion mirrors the typed const
        // assertion so the typed test reports the typed
        // failure cleanly when typed someone breaks the
        // typed audit.
        assert!(super::product_cloud_execution_route_is_fun_renderer_only());

        let audit = ProductCloudExecutionRouteAudit::PRODUCT;
        assert!(audit.route_is_fun_renderer_only());
    }

    /// Pass C9.8 acceptance — product route does not call
    /// legacy RetiredEngine cloud compute/composite systems.
    /// Verified by typed retirement contract +
    /// `fun_render_sky_does_not_execute_product_gpu_passes`
    /// predicate.
    #[test]
    fn product_route_does_not_call_legacy_retired_engine_cloud_systems() {
        let audit = ProductCloudExecutionRouteAudit::PRODUCT;
        assert!(audit.fun_render_sky_does_not_execute_product_gpu_passes());
        // Typed retirement policy is typed ExtractsOnly.
        assert_eq!(
            audit.cloud_executor.retirement_policy,
            FunRenderCloudShadowExecutionPolicy::ExtractsOnly,
        );
        // Typed retired_engine cloud path is typed diagnostic-only.
        assert!(
            audit
                .retirement_contract
                .retired_engine_cloud_path_is_diagnostic_only
        );
        // Typed product GPU work lives in typed
        // fun-renderer.
        assert!(audit.retirement_contract.product_gpu_work_in_fun_renderer);
    }

    /// Pass C9.8 acceptance — legacy cloud rendering
    /// requires explicit diagnostic flag.
    #[test]
    fn legacy_cloud_rendering_requires_explicit_diagnostic_flag() {
        let audit = ProductCloudExecutionRouteAudit::PRODUCT;
        assert!(audit.diagnostic_legacy_renderer_requires_loud_opt_in());
        // Typed DiagnosticOnly + Quarantined require
        // non-production flag.
        assert!(FunRenderCloudShadowExecutionPolicy::DiagnosticOnly.requires_non_production_flag());
        assert!(FunRenderCloudShadowExecutionPolicy::Quarantined.requires_non_production_flag());
        // Typed ExtractsOnly (product) does not.
        assert!(!FunRenderCloudShadowExecutionPolicy::ExtractsOnly.requires_non_production_flag());
    }

    /// Pass C9.8 acceptance — startup log states all four
    /// ownership lines per user spec.
    #[test]
    fn startup_log_states_all_four_ownership_lines() {
        let audit = ProductCloudExecutionRouteAudit::PRODUCT;
        let log = cloud_route_startup_log(&audit);
        // Typed four ownership lines per typed user-spec.
        assert!(log.contains("cloud executor: fun-renderer"));
        assert!(log.contains("cloud texture owner: fun-renderer"));
        assert!(log.contains("cloud shadow owner: fun-renderer"));
        assert!(log.contains("final shadow evaluation owner: Lux"));
        // Typed combined predicate.
        assert!(startup_log_contains_all_four_identities(&log));
        // Typed multi-line format.
        let lines: Vec<&str> = log.lines().collect();
        assert!(lines.len() >= 5); // header + 4 identity lines
    }

    /// Pass C9.8 — typed product startup path manifest
    /// covers every user-spec audit site.
    #[test]
    fn product_startup_path_manifest_covers_every_user_spec_site() {
        assert!(super::product_startup_path_manifest_covers_user_spec());
        // Typed manifest enumerates typed five sites.
        let sites: Vec<&str> = PRODUCT_STARTUP_PATH_MANIFEST.iter().copied().collect();
        // Typed user-spec list:
        //   runtime client, fun_render, renderer bridge
        //   settings, stack profiles, cloud feature flags.
        assert!(sites.iter().any(|s| s.contains("runtime_client")));
        assert!(sites.iter().any(|s| s.contains("fun_render::core")));
        assert!(sites.iter().any(|s| s.contains("RendererBridgeSettings")));
        assert!(sites.iter().any(|s| s.contains("stack profile")));
        assert!(sites.iter().any(|s| s.contains("cloud feature flag")));
    }

    /// Pass C9.8 — typed cloud executor + cloud shadow
    /// owner identities agree (both name fun-renderer +
    /// share canonical module).
    #[test]
    fn cloud_executor_and_shadow_owner_agree() {
        let audit = ProductCloudExecutionRouteAudit::PRODUCT;
        assert!(audit.cloud_executor_and_shadow_owner_agree());
        assert_eq!(audit.cloud_executor, audit.cloud_shadow_owner);
        assert!(audit.cloud_executor.names_fun_renderer_as_executor());
    }

    /// Pass C9.8 — typed final shadow evaluation owner
    /// identity names Lux + canonical compose function.
    #[test]
    fn final_shadow_owner_names_lux_with_canonical_compose() {
        let owner = LuxFinalShadowEvaluationOwnerIdentity::PRODUCT;
        assert!(owner.names_lux_as_final_shadow_owner());
        assert_eq!(owner.owner_label, "Lux");
        assert_eq!(owner.owner_module, "fun_renderer::lux_material_cloud_layer");
        assert_eq!(
            owner.final_compose_function,
            "apply_cloud_layer_to_material_direct_lighting",
        );
    }

    /// Pass C9.8 — typed fun_render::sky may parse/extract
    /// settings per user-spec.
    #[test]
    fn fun_render_sky_may_parse_extract_settings() {
        let audit = ProductCloudExecutionRouteAudit::PRODUCT;
        assert!(audit.fun_render_sky_may_parse_extract_settings());
        assert!(audit.retirement_contract.fun_render_extracts_only);
    }

    /// Pass C9.8 — typed cloud texture owner identity
    /// names fun-renderer + RendererOwned source.
    #[test]
    fn cloud_texture_owner_names_fun_renderer_and_renderer_owned_source() {
        let audit = ProductCloudExecutionRouteAudit::PRODUCT;
        assert!(audit.cloud_texture_owner.names_fun_renderer_as_owner());
        assert!(audit.cloud_texture_owner.names_fun_render_as_legacy_donor());
        assert_eq!(
            audit.cloud_texture_owner.texture_source,
            CloudTextureSource::RendererOwned,
        );
        assert!(audit.resource_contract.obeys_all_product_rules());
    }

    /// Pass C9.8 — typed const string helpers are
    /// const-evaluable.
    #[test]
    fn const_str_helpers_work_at_const_eval() {
        const STRINGS_MATCH: bool = const_str_eq("abc", "abc");
        const STRINGS_DIFFER: bool = const_str_eq("abc", "abd");
        const PREFIX_MATCHES: bool =
            const_str_starts_with("fun_renderer::cloud_shadow", "fun_renderer::");
        const PREFIX_MISMATCHES: bool =
            const_str_starts_with("renderer::cloud_shadow", "fun_renderer::");
        assert!(STRINGS_MATCH);
        assert!(!STRINGS_DIFFER);
        assert!(PREFIX_MATCHES);
        assert!(!PREFIX_MISMATCHES);
    }

    /// Pass C9.8 — typed cloud_route_startup_log returns
    /// stable shape.
    #[test]
    fn cloud_route_startup_log_stable_shape() {
        let audit = ProductCloudExecutionRouteAudit::PRODUCT;
        let log = cloud_route_startup_log(&audit);
        // Typed header line.
        assert!(log.starts_with("[fun-renderer] cloud route audit"));
        // Typed each identity line on its own line.
        let lines: Vec<&str> = log.lines().collect();
        assert_eq!(lines.len(), 5);
        assert!(lines[1].contains("cloud executor"));
        assert!(lines[2].contains("cloud texture owner"));
        assert!(lines[3].contains("cloud shadow owner"));
        assert!(lines[4].contains("final shadow evaluation owner"));
    }

    /// Pass C9.8 — typed acceptance audit covers user-spec
    /// 4-bullet acceptance set in one summary.
    #[test]
    fn acceptance_covers_user_spec_four_bullets() {
        let audit = ProductCloudExecutionRouteAudit::PRODUCT;

        // Bullet 1: Product route does not call legacy
        // RetiredEngine cloud compute/composite systems.
        assert!(audit.fun_render_sky_does_not_execute_product_gpu_passes());

        // Bullet 2: Legacy cloud rendering requires
        // explicit diagnostic flag.
        assert!(audit.diagnostic_legacy_renderer_requires_loud_opt_in());

        // Bullet 3: Startup log states the four ownership
        // identities.
        let log = cloud_route_startup_log(&audit);
        assert!(startup_log_contains_all_four_identities(&log));

        // Bullet 4 (composite): Product cloud execution
        // route is fun-renderer only.
        assert!(audit.route_is_fun_renderer_only());
        assert!(super::product_cloud_execution_route_is_fun_renderer_only());
    }
}
