//! Pass C0 / C1 — typed cloud diagnostics.
//!
//! Emits the typed "Cloud Pipeline" debug section the
//! typed renderer frame-graph debug artifact carries when
//! the typed cloud executor is wired.  Pass C2+ will
//! extend this with typed per-pass GPU timings + typed
//! shadow-readback audits.

use crate::cloud_executor::CloudExecutorPlan;
use crate::cloud_shadow_passes::CloudShadowGraphDiagnostics;
use crate::clouds::FunCloudRendererContract;

pub const FUN_RENDERER_CLOUD_DIAGNOSTICS_SCHEMA_VERSION: u16 = 1;

/// Typed Pass C0 / C1 cloud diagnostics report.  The
/// typed cloud executor fills these typed counters after
/// it walks the typed pass list.  Today this report is
/// minimal — Pass C2+ will add typed GPU timing and
/// typed readback counters.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CloudFrameDiagnostics {
    pub schema_version: u16,
    pub typed_pass_count: u16,
    pub world_shadow_pass_registered: bool,
    pub lux_volumetric_bridge_registered: bool,
    pub debug_overlay_registered: bool,
    pub temporal_resolve_registered: bool,
}

impl CloudFrameDiagnostics {
    pub const COLD_DEFAULT: Self = Self {
        schema_version: FUN_RENDERER_CLOUD_DIAGNOSTICS_SCHEMA_VERSION,
        typed_pass_count: 0,
        world_shadow_pass_registered: false,
        lux_volumetric_bridge_registered: false,
        debug_overlay_registered: false,
        temporal_resolve_registered: false,
    };

    /// Typed builder: derive a typed diagnostics record
    /// from a typed executor plan.
    #[must_use]
    pub fn from_plan(plan: &CloudExecutorPlan) -> Self {
        use crate::cloud_passes::CloudPassRole;
        let roles = plan.typed_pass_roles();
        let typed_pass_count = u16::try_from(roles.len()).unwrap_or(u16::MAX);
        Self {
            schema_version: FUN_RENDERER_CLOUD_DIAGNOSTICS_SCHEMA_VERSION,
            typed_pass_count,
            world_shadow_pass_registered: roles.contains(&CloudPassRole::WorldShadowProjection),
            lux_volumetric_bridge_registered: roles.contains(&CloudPassRole::LuxVolumetricBridge),
            debug_overlay_registered: roles.contains(&CloudPassRole::DebugOverlay),
            temporal_resolve_registered: roles.contains(&CloudPassRole::TemporalResolve),
        }
    }

    /// Typed predicate: did the typed cloud pipeline run
    /// this frame (at least one typed pass registered)?
    #[must_use]
    pub const fn cloud_pipeline_ran(&self) -> bool {
        self.typed_pass_count > 0
    }
}

/// Typed Pass C0 / C1 cloud debug section.  Returns the
/// typed "Cloud Pipeline" multi-line section the typed
/// debug artifact carries.
#[must_use]
pub fn cloud_pipeline_debug_section(
    plan: &CloudExecutorPlan,
    diagnostics: &CloudFrameDiagnostics,
) -> String {
    use core::fmt::Write as _;
    let mut content = String::new();
    let _ = writeln!(content, "Cloud Pipeline");
    let _ = writeln!(content, "--------------");
    let _ = writeln!(content, "active: {}", plan.is_active());
    let _ = writeln!(content, "quality: {}", plan.settings.quality.as_str());
    let _ = writeln!(
        content,
        "internal_scale: {}",
        plan.settings.internal_scale.as_str(),
    );
    let _ = writeln!(content, "profile: {}", plan.settings.profile_id.as_str(),);
    let _ = writeln!(content, "temporal: {}", plan.settings.temporal_enabled);
    let _ = writeln!(
        content,
        "world_shadows: {}",
        plan.settings.registers_world_shadow_pass(),
    );
    let _ = writeln!(
        content,
        "lux_lighting: {}",
        plan.settings.consumes_lux_volumetric_lighting(),
    );
    let _ = writeln!(
        content,
        "debug_overlay: {}",
        plan.settings.debug_overlay.as_str(),
    );
    let _ = writeln!(
        content,
        "typed_pass_count: {}",
        diagnostics.typed_pass_count
    );
    let _ = writeln!(content, "pass_chain:");
    for role in plan.typed_pass_roles() {
        let _ = writeln!(
            content,
            "  {} = order_key {}",
            role.as_str(),
            role.order_key(),
        );
    }
    // Pass C0 cloud ownership contract footer — anchors
    // the typed debug section to the typed contract.
    let contract = FunCloudRendererContract::CURRENT;
    let _ = writeln!(
        content,
        "ownership_contract: fun_renderer_owns={} fun_render_extracts_only={} \
        clouds_use_renderer_frame_graph={} clouds_consume_lux={} \
        clouds_cast_world_shadows={}",
        contract.fun_renderer_owns_cloud_execution,
        contract.fun_render_extracts_only,
        contract.clouds_use_renderer_frame_graph,
        contract.clouds_can_consume_lux_volumetric_lighting,
        contract.clouds_can_cast_world_shadows,
    );
    content
}

/// Pass C7.4.5 cloud shadow debug section.  Returns the
/// typed "Cloud Shadow Pipeline" multi-line section the
/// typed debug artifact emits when the typed cloud shadow
/// chain is recorded.  Drives the typed "Project, filter,
/// register passes appear in graph diagnostics" acceptance
/// bullet.
#[must_use]
pub fn cloud_shadow_pipeline_debug_section(diagnostics: &CloudShadowGraphDiagnostics) -> String {
    use core::fmt::Write as _;
    let mut content = String::new();
    let _ = writeln!(content, "Cloud Shadow Pipeline");
    let _ = writeln!(content, "---------------------");
    let _ = writeln!(
        content,
        "frame_delay_mode: {}",
        diagnostics.frame_delay_mode.as_str(),
    );
    let _ = writeln!(
        content,
        "full_chain_ran: {}",
        diagnostics.full_chain_ran_this_frame(),
    );
    let _ = writeln!(
        content,
        "every_pass_declares_resource_interaction: {}",
        diagnostics.every_pass_declares_resource_interaction(),
    );
    let _ = writeln!(content, "pass_chain:");
    for record in diagnostics.as_slice() {
        let _ = writeln!(
            content,
            "  {} = order_key {} registers={} projection_live={} reads={} writes={}",
            record.role.as_str(),
            record.order_key,
            record.registers_pass,
            record.projection_is_live,
            record.reads.len(),
            record.writes.len(),
        );
    }
    content
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_from_product_plan_reports_full_chain() {
        let plan = CloudExecutorPlan::PRODUCT_DEFAULT;
        let d = CloudFrameDiagnostics::from_plan(&plan);
        assert!(d.cloud_pipeline_ran());
        assert!(d.world_shadow_pass_registered);
        assert!(d.lux_volumetric_bridge_registered);
        assert!(d.temporal_resolve_registered);
        assert!(!d.debug_overlay_registered);
        assert!(d.typed_pass_count >= 6);
    }

    #[test]
    fn diagnostics_from_disabled_plan_reports_zero_passes() {
        let plan = CloudExecutorPlan::DISABLED;
        let d = CloudFrameDiagnostics::from_plan(&plan);
        assert!(!d.cloud_pipeline_ran());
        assert_eq!(d.typed_pass_count, 0);
    }

    #[test]
    fn cloud_pipeline_debug_section_emits_typed_layout() {
        let plan = CloudExecutorPlan::PRODUCT_DEFAULT;
        let d = CloudFrameDiagnostics::from_plan(&plan);
        let section = cloud_pipeline_debug_section(&plan, &d);
        assert!(section.contains("Cloud Pipeline"));
        assert!(section.contains("active: true"));
        assert!(section.contains("quality: balanced"));
        assert!(section.contains("profile: scattered"));
        assert!(section.contains("world_shadows: true"));
        assert!(section.contains("lux_lighting: true"));
        assert!(section.contains("pass_chain:"));
        assert!(section.contains("cloud_raymarch"));
        assert!(section.contains("cloud_composite"));
        assert!(section.contains("ownership_contract:"));
        assert!(section.contains("fun_renderer_owns=true"));
    }

    /// Pass C7.4.5 acceptance — typed cloud shadow pipeline
    /// debug section emits every typed cloud-shadow pass
    /// (project, filter, register-layer) with the typed
    /// order key + registers / projection flags + reads /
    /// writes counts.  Confirms the typed "Project, filter,
    /// register passes appear in graph diagnostics"
    /// acceptance bullet at the typed user-visible string
    /// layer.
    #[test]
    fn cloud_shadow_pipeline_debug_section_emits_every_pass() {
        use crate::cloud_shadow::{CloudShadowFrameDelayMode, CloudShadowProjectionConstants};
        use crate::cloud_shadow_passes::CloudShadowGraphDiagnostics;
        use crate::clouds::{CloudRenderSettings, CloudWeatherProfileId};
        use fun_lux::LuxLightId;

        let settings = CloudRenderSettings::PRODUCT_DEFAULT;
        let constants = CloudShadowProjectionConstants::from_inputs(
            &settings,
            CloudWeatherProfileId::Scattered,
            LuxLightId::new(7),
            [0.0, 1.0, 0.0],
            0,
        );
        let diagnostics = CloudShadowGraphDiagnostics::from_inputs(
            &settings,
            &constants,
            CloudShadowFrameDelayMode::OneFrameDelayed,
        );
        let section = cloud_shadow_pipeline_debug_section(&diagnostics);
        assert!(section.contains("Cloud Shadow Pipeline"));
        assert!(section.contains("frame_delay_mode: one_frame_delayed"));
        assert!(section.contains("full_chain_ran: true"));
        assert!(section.contains("pass_chain:"));
        assert!(section.contains("lux_cloud_shadow_project"));
        assert!(section.contains("lux_cloud_shadow_filter"));
        assert!(section.contains("lux_cloud_shadow_register_layer"));
        assert!(section.contains("registers=true"));
        assert!(section.contains("projection_live=true"));
        assert!(section.contains("order_key 140"));
        assert!(section.contains("order_key 145"));
        assert!(section.contains("order_key 150"));
    }
}
