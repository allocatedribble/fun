//! Typed `LuxFramePlan` diagnostics.
//!
//! Records typed metrics about a single frame plan build so
//! the renderer + observability harnesses can audit pass /
//! resource churn without parsing the plan itself.

pub const FUN_LUX_DIAGNOSTICS_SCHEMA_VERSION: u16 = 1;

/// Typed per-frame plan diagnostics. The renderer ingests
/// these alongside the typed `LuxFramePlan` to drive the
/// observability surface; the audit-harness tests assert on
/// the typed counters.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxFramePlanDiagnostics {
    pub schema_version: u16,
    /// Total number of scenes the planner walked.
    pub scenes_total: u32,
    /// Number of scenes the planner classified as visible.
    pub scenes_visible: u32,
    /// Number of scenes whose typed dirty-region list was
    /// non-empty.
    pub scenes_dirty: u32,
    /// Total number of typed pass requests across every
    /// scene plan.
    pub pass_count_total: u32,
    /// Total number of typed resource intents across every
    /// scene plan.
    pub resource_count_total: u32,
    /// Total number of typed dirty regions across every
    /// scene plan.
    pub dirty_region_count_total: u32,
    /// Plan build time in microseconds (recorded by the
    /// renderer-side harness when it submits the plan; the
    /// fun-lux planner itself records 0 because measurement
    /// is the renderer's responsibility).
    pub plan_build_time_us: u32,
    /// Typed predicate flag: did the planner emit a minimal
    /// plan (no scene work)?
    pub minimal_plan_no_scene_work: bool,
}

impl LuxFramePlanDiagnostics {
    pub const COLD_DEFAULT: Self = Self {
        schema_version: FUN_LUX_DIAGNOSTICS_SCHEMA_VERSION,
        scenes_total: 0,
        scenes_visible: 0,
        scenes_dirty: 0,
        pass_count_total: 0,
        resource_count_total: 0,
        dirty_region_count_total: 0,
        plan_build_time_us: 0,
        minimal_plan_no_scene_work: true,
    };

    /// Typed builder: marks the diagnostics as recording a
    /// minimal-plan-no-scene-work frame.
    #[must_use]
    pub const fn minimal_for(scenes_total: u32) -> Self {
        Self {
            schema_version: FUN_LUX_DIAGNOSTICS_SCHEMA_VERSION,
            scenes_total,
            scenes_visible: 0,
            scenes_dirty: 0,
            pass_count_total: 0,
            resource_count_total: 0,
            dirty_region_count_total: 0,
            plan_build_time_us: 0,
            minimal_plan_no_scene_work: true,
        }
    }

    /// Typed predicate: did the planner emit any work this
    /// frame (passes or resources)?
    #[must_use]
    pub const fn emitted_work(&self) -> bool {
        self.pass_count_total > 0 || self.resource_count_total > 0
    }

    /// Typed predicate: did the planner emit *only* minimal
    /// work — no scenes visible, no dirty regions, no passes,
    /// no resources?
    #[must_use]
    pub const fn is_strict_minimal(&self) -> bool {
        self.scenes_visible == 0
            && self.scenes_dirty == 0
            && self.pass_count_total == 0
            && self.resource_count_total == 0
            && self.dirty_region_count_total == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_LUX_DIAGNOSTICS_SCHEMA_VERSION, 1);
    }

    #[test]
    fn cold_default_diagnostics_record_zero_work() {
        let d = LuxFramePlanDiagnostics::COLD_DEFAULT;
        assert_eq!(d.scenes_total, 0);
        assert_eq!(d.pass_count_total, 0);
        assert!(!d.emitted_work());
        assert!(d.is_strict_minimal());
        assert!(d.minimal_plan_no_scene_work);
    }

    #[test]
    fn minimal_for_records_scene_count_without_emitting_work() {
        let d = LuxFramePlanDiagnostics::minimal_for(3);
        assert_eq!(d.scenes_total, 3);
        assert_eq!(d.scenes_visible, 0);
        assert_eq!(d.pass_count_total, 0);
        assert!(!d.emitted_work());
        assert!(d.is_strict_minimal());
        assert!(d.minimal_plan_no_scene_work);
    }

    #[test]
    fn emitted_work_predicate_responds_to_pass_or_resource_count() {
        let mut d = LuxFramePlanDiagnostics::COLD_DEFAULT;
        d.pass_count_total = 1;
        assert!(d.emitted_work());
        assert!(!d.is_strict_minimal());
        let mut d2 = LuxFramePlanDiagnostics::COLD_DEFAULT;
        d2.resource_count_total = 1;
        assert!(d2.emitted_work());
        assert!(!d2.is_strict_minimal());
    }
}
