//! Pass V2.6 — typed first volumetric execution chain.
//!
//! Pairs the typed fun-lux `LuxVolumetricSettings` policy
//! (the typed canonical surface from `fog_volumetric`)
//! with the typed renderer frame-graph roles to ship the
//! first typed volumetric execution chain:
//!
//! 1. `LuxVolumetricFogInject`   (typed fog density inject)
//! 2. `LuxVolumetricLightInject` (typed light scattering inject)
//! 3. `LuxVolumetricIntegrate`   (typed view-ray integration)
//! 4. `LuxVolumetricComposite`   (typed composite into HDR)
//!
//! Temporal reprojection (`LuxVolumetricTemporalReproject`)
//! is intentionally deferred until after the first typed
//! visible fog renders.  The typed Pass 8 surface tracks
//! the typed reset reasons and the typed temporal stability
//! settings; V2.6 only audits the typed first execution
//! chain.
//!
//! The typed executor module is a pure typed contract — it
//! exposes the typed chain as a typed slice, exposes typed
//! ordering predicates against the Pass V2.5 HDR pipeline,
//! and exposes a typed debug-section emitter the bridge can
//! append to the typed
//! `RendererFrameGraphDebugArtifact.content`.

use fun_lux::LuxVolumetricSettings;

use crate::frame_graph::FrameGraphPassRole;
use crate::hdr_pipeline::hdr_pipeline_order_key;

pub const FUN_RENDERER_LUX_VOLUMETRIC_EXECUTOR_SCHEMA_VERSION: u16 = 1;

/// Typed Pass V2.6 first volumetric execution chain.  Four
/// typed `FrameGraphPassRole` variants in typed order.
///
/// The typed chain is `&'static` so callers can iterate it
/// without copying.
pub const LUX_VOLUMETRIC_FIRST_EXECUTION_CHAIN: &[FrameGraphPassRole] = &[
    FrameGraphPassRole::LuxVolumetricFogInject,
    FrameGraphPassRole::LuxVolumetricLightInject,
    FrameGraphPassRole::LuxVolumetricIntegrate,
    FrameGraphPassRole::LuxVolumetricComposite,
];

/// Typed Pass V2.6 executor plan.  Carries the typed
/// `LuxVolumetricSettings` + a typed flag indicating
/// whether the typed first visible fog has been
/// composited.  The typed temporal reprojection pass is
/// gated on the typed flag (deferred until after first
/// visible fog per the V2.6 spec).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxVolumetricExecutorPlan {
    pub schema_version: u16,
    pub settings: LuxVolumetricSettings,
    /// Typed flag — has the typed first visible fog
    /// composite already run?  V2.6 emits the typed
    /// first execution chain on every frame the typed
    /// pipeline is active; the typed temporal
    /// reprojection pass is deferred until this flag
    /// flips to `true`.
    pub first_visible_fog_composited: bool,
}

impl LuxVolumetricExecutorPlan {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_RENDERER_LUX_VOLUMETRIC_EXECUTOR_SCHEMA_VERSION,
        settings: LuxVolumetricSettings::PRODUCT_DEFAULT,
        first_visible_fog_composited: false,
    };

    pub const COLD_DEFAULT: Self = Self {
        schema_version: FUN_RENDERER_LUX_VOLUMETRIC_EXECUTOR_SCHEMA_VERSION,
        settings: LuxVolumetricSettings::COLD_DEFAULT,
        first_visible_fog_composited: false,
    };

    /// Typed builder: derive a typed plan from typed
    /// fun-lux settings.
    #[must_use]
    pub const fn from_settings(settings: LuxVolumetricSettings) -> Self {
        Self {
            schema_version: FUN_RENDERER_LUX_VOLUMETRIC_EXECUTOR_SCHEMA_VERSION,
            settings,
            first_visible_fog_composited: false,
        }
    }

    /// Typed predicate: should the typed first-execution
    /// chain run this frame?
    #[must_use]
    pub const fn registers_first_execution_chain(&self) -> bool {
        self.settings.is_active()
    }

    /// Typed predicate: should the typed temporal
    /// reprojection pass run this frame?  V2.6 defers
    /// this until after first visible fog composites.
    #[must_use]
    pub const fn registers_temporal_reprojection(&self) -> bool {
        self.settings.is_active() && self.first_visible_fog_composited
    }

    /// Typed slice of typed `FrameGraphPassRole` variants
    /// this plan emits this frame.  Returns the typed
    /// first-execution chain when active, plus
    /// `LuxVolumetricTemporalReproject` once the typed
    /// temporal gate fires.
    #[must_use]
    pub fn typed_roles(&self) -> &'static [FrameGraphPassRole] {
        if self.registers_temporal_reprojection() {
            // First-execution + temporal reprojection.  The
            // typed temporal reproject runs BEFORE the
            // typed integrate step in the typed Pass 8
            // ordering (order_key 700 < 800).  V2.6 leaves
            // the typed temporal slot reserved; the typed
            // chain returned here keeps the typed
            // first-execution order so a future pass can
            // splice in temporal reproject without
            // re-deriving the typed chain.
            &[
                FrameGraphPassRole::LuxVolumetricFogInject,
                FrameGraphPassRole::LuxVolumetricLightInject,
                FrameGraphPassRole::LuxVolumetricTemporalReproject,
                FrameGraphPassRole::LuxVolumetricIntegrate,
                FrameGraphPassRole::LuxVolumetricComposite,
            ]
        } else if self.registers_first_execution_chain() {
            LUX_VOLUMETRIC_FIRST_EXECUTION_CHAIN
        } else {
            &[]
        }
    }
}

/// Typed Pass V2.6 typed predicate — the typed volumetric
/// composite runs BEFORE bloom + tonemap.  Encoded by
/// looking up the typed Pass V2.5 HDR order keys:
///   LuxVolumetricComposite (150) — note: Pass V2.5
///   `hdr_pipeline_order_key` puts it at 150 so it
///   runs before bloom prefilter (200).
///   PostProcessBloomPrefilter (200)
///   PostProcessToneMapping (500)
#[must_use]
pub const fn volumetric_composite_runs_before_bloom_and_tonemap() -> bool {
    let composite = match hdr_pipeline_order_key(FrameGraphPassRole::LuxVolumetricComposite) {
        Some(k) => k,
        None => return false,
    };
    let bloom_prefilter =
        match hdr_pipeline_order_key(FrameGraphPassRole::PostProcessBloomPrefilter) {
            Some(k) => k,
            None => return false,
        };
    let tonemap = match hdr_pipeline_order_key(FrameGraphPassRole::PostProcessToneMapping) {
        Some(k) => k,
        None => return false,
    };
    composite < bloom_prefilter && composite < tonemap
}

/// Typed Pass V2.6 debug section.  Returns the typed
/// "Volumetric Pipeline" multi-line section the typed
/// debug artifact carries when V2.6 is wired.
#[must_use]
pub fn volumetric_pipeline_debug_section(plan: &LuxVolumetricExecutorPlan) -> String {
    use core::fmt::Write as _;
    let mut content = String::new();
    let _ = writeln!(content, "Volumetric Pipeline");
    let _ = writeln!(content, "-------------------");
    let _ = writeln!(content, "active: {}", plan.settings.is_active());
    let _ = writeln!(content, "quality: {}", plan.settings.quality.as_str());
    let _ = writeln!(
        content,
        "first_visible_fog_composited: {}",
        plan.first_visible_fog_composited,
    );
    let _ = writeln!(content, "first_execution_chain:");
    for role in plan.typed_roles() {
        let key = hdr_pipeline_order_key(*role).unwrap_or(0);
        let _ = writeln!(content, "  {} = order_key {}", role.as_str(), key);
    }
    content
}

// ============================================================================
// Tests — Pass V2.6 typed acceptance
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use fun_lux::{LuxDirtyFlags, LuxFramePlanner, LuxSceneChangeSignal, LuxSceneId};

    use crate::frame_graph::RendererFrameGraph;
    use crate::lux_graph::LuxGraphCompiler;
    use crate::resource::RendererResourceRegistry;

    /// Pass V2.6 acceptance — the typed canonical
    /// volumetric settings surface is single-source.  The
    /// typed `LuxVolumetricSettings` reached via
    /// `fun_lux::LuxVolumetricSettings` (crate-root
    /// re-export) and `fun_lux::fog_volumetric::LuxVolumetricSettings`
    /// resolve to the same typed item; the typed
    /// `fun_lux::volumetric` shim re-exports the same
    /// typed name through its typed migration aliases.
    #[test]
    fn canonical_volumetric_settings_surface_is_single_source() {
        // The typed `LuxVolumetricSettings` reachable via
        // both paths is the same typed item — the typed
        // `from_settings` typed builder accepts either
        // import path interchangeably.
        let from_canonical: LuxVolumetricSettings = fun_lux::LuxVolumetricSettings::PRODUCT_DEFAULT;
        let from_module: LuxVolumetricSettings =
            fun_lux::fog_volumetric::LuxVolumetricSettings::PRODUCT_DEFAULT;
        let from_shim: LuxVolumetricSettings =
            fun_lux::volumetric::LuxVolumetricSettings::PRODUCT_DEFAULT;
        assert_eq!(from_canonical, from_module);
        assert_eq!(from_canonical, from_shim);
        // Round-trips through `LuxVolumetricExecutorPlan`.
        let plan = LuxVolumetricExecutorPlan::from_settings(from_canonical);
        assert_eq!(plan.settings, from_module);
    }

    /// Pass V2.6 acceptance — the typed volumetric graph
    /// roles compile cleanly from a typed `LuxFramePlan`.
    /// Bypasses the bridge and exercises
    /// `LuxGraphCompiler::compile_lux_plan` directly with a
    /// typed light-changed signal so the typed planner
    /// emits the typed volumetric chain.
    #[test]
    fn volumetric_graph_roles_compile_from_lux_plan() {
        let mut planner = LuxFramePlanner::product_default();
        planner.current_light_count = 16;
        let plan = planner.build_frame_plan(
            1,
            &[LuxSceneChangeSignal::light_changed(LuxSceneId::PROOF_SCENE, 1)],
        );
        let mut graph = RendererFrameGraph::default();
        let mut resources = RendererResourceRegistry::default();
        let report = LuxGraphCompiler::compile_lux_plan(&mut graph, &mut resources, &plan);

        // The typed planner emits the typed volumetric chain
        // under product-default scheduling (volumetric is
        // EveryFrame).  Confirm the typed compile saw at
        // least one of the typed four V2.6 roles.
        let roles: Vec<FrameGraphPassRole> =
            graph.passes().iter().map(|p| p.descriptor.role).collect();
        let any_volumetric = roles.iter().any(|r| {
            matches!(
                r,
                FrameGraphPassRole::LuxVolumetricFogInject
                    | FrameGraphPassRole::LuxVolumetricLightInject
                    | FrameGraphPassRole::LuxVolumetricIntegrate
                    | FrameGraphPassRole::LuxVolumetricComposite,
            )
        });
        assert!(
            any_volumetric,
            "compile must emit at least one typed V2.6 volumetric role; roles = {:?}",
            roles,
        );
        // Compile cleanly — no typed failures.
        assert!(
            report.failures.is_empty(),
            "volumetric compile must succeed: {:?}",
            report.failures,
        );
    }

    /// Pass V2.6 acceptance — the typed volumetric
    /// composite runs BEFORE bloom + tonemap.  Verified
    /// through the typed Pass V2.5 `hdr_pipeline_order_key`
    /// table.
    #[test]
    fn volumetric_composite_runs_before_bloom_and_tonemap_predicate_holds() {
        assert!(volumetric_composite_runs_before_bloom_and_tonemap());
        let composite =
            hdr_pipeline_order_key(FrameGraphPassRole::LuxVolumetricComposite).unwrap();
        let bloom_prefilter =
            hdr_pipeline_order_key(FrameGraphPassRole::PostProcessBloomPrefilter).unwrap();
        let tonemap =
            hdr_pipeline_order_key(FrameGraphPassRole::PostProcessToneMapping).unwrap();
        assert!(composite < bloom_prefilter);
        assert!(composite < tonemap);
    }

    /// Pass V2.6 acceptance — a typed fog-only dirty change
    /// does NOT rebuild the typed direct-light cluster
    /// grid.  Encoded by the typed Pass 2
    /// `LuxDirtyFlags::VOLUMETRIC.invalidates_direct_light_clusters`
    /// predicate (returns `false` for VOLUMETRIC alone).
    #[test]
    fn fog_only_change_does_not_rebuild_light_clusters() {
        let fog_only = LuxDirtyFlags::VOLUMETRIC;
        assert!(!fog_only.invalidates_direct_light_clusters());
        assert!(fog_only.touches_volumetric_only());
        // Color-only also does not rebuild clusters (sanity
        // check from the typed Pass 2 contract).
        let color_only = LuxDirtyFlags::COLOR;
        assert!(!color_only.invalidates_direct_light_clusters());
    }

    /// Pass V2.6 — the typed first execution chain has
    /// exactly four typed roles in typed order.
    #[test]
    fn first_execution_chain_has_four_roles_in_order() {
        assert_eq!(LUX_VOLUMETRIC_FIRST_EXECUTION_CHAIN.len(), 4);
        assert_eq!(
            LUX_VOLUMETRIC_FIRST_EXECUTION_CHAIN[0],
            FrameGraphPassRole::LuxVolumetricFogInject,
        );
        assert_eq!(
            LUX_VOLUMETRIC_FIRST_EXECUTION_CHAIN[1],
            FrameGraphPassRole::LuxVolumetricLightInject,
        );
        assert_eq!(
            LUX_VOLUMETRIC_FIRST_EXECUTION_CHAIN[2],
            FrameGraphPassRole::LuxVolumetricIntegrate,
        );
        assert_eq!(
            LUX_VOLUMETRIC_FIRST_EXECUTION_CHAIN[3],
            FrameGraphPassRole::LuxVolumetricComposite,
        );
    }

    /// Pass V2.6 — temporal reprojection is deferred until
    /// after the typed first visible fog composites.
    #[test]
    fn temporal_reprojection_deferred_until_first_visible_fog_composites() {
        let mut plan = LuxVolumetricExecutorPlan::PRODUCT_DEFAULT;
        assert!(plan.registers_first_execution_chain());
        assert!(!plan.registers_temporal_reprojection());
        let roles = plan.typed_roles();
        assert!(!roles.contains(&FrameGraphPassRole::LuxVolumetricTemporalReproject));

        plan.first_visible_fog_composited = true;
        assert!(plan.registers_temporal_reprojection());
        let roles = plan.typed_roles();
        assert!(roles.contains(&FrameGraphPassRole::LuxVolumetricTemporalReproject));
    }

    /// Pass V2.6 — cold-default plan emits zero typed
    /// roles.
    #[test]
    fn cold_default_emits_no_roles() {
        let plan = LuxVolumetricExecutorPlan::COLD_DEFAULT;
        assert!(!plan.registers_first_execution_chain());
        assert_eq!(plan.typed_roles().len(), 0);
    }

    #[test]
    fn debug_section_emits_typed_layout() {
        let plan = LuxVolumetricExecutorPlan::PRODUCT_DEFAULT;
        let section = volumetric_pipeline_debug_section(&plan);
        assert!(section.contains("Volumetric Pipeline"));
        assert!(section.contains("active: true"));
        assert!(section.contains("first_visible_fog_composited:"));
        assert!(section.contains("first_execution_chain:"));
        assert!(section.contains("lux_volumetric_fog_inject"));
        assert!(section.contains("lux_volumetric_composite"));
    }
}
