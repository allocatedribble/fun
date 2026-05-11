//! Pass V2.5 — typed renderer-side tonemap pass plan.
//!
//! Translates the typed fun-lux Pass 5
//! `FunLuxTonemapSettings` (`FunLuxTonemapMode` +
//! `FunLuxCustomTonemapCurve` with 7 typed dials) into a
//! typed renderer frame-graph plan that registers the
//! typed `PostProcessToneMapping` role.
//!
//! The typed plan ships alongside
//! [`crate::hdr_pipeline::LuxHdrPipelinePlan`] and the
//! typed [`crate::exposure_pass::LuxExposurePassPlan`];
//! together they fully describe the typed HDR-post path
//! the typed renderer registers each frame.
//!
//! Tonemap MUST run after bloom + HDR composite + exposure
//! and BEFORE display output encoding.  The typed
//! `crate::hdr_pipeline::tonemap_runs_after_bloom_and_hdr_composite`
//! + `final_output_runs_after_tonemap` const predicates
//! enforce this at the type-system layer.

use fun_lux::FunLuxTonemapSettings;

use crate::frame_graph::FrameGraphPassRole;

pub const FUN_RENDERER_TONEMAP_PASS_SCHEMA_VERSION: u16 = 1;

/// Typed Pass V2.5 tonemap pass plan.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LuxTonemapPassPlan {
    pub schema_version: u16,
    pub settings: FunLuxTonemapSettings,
}

impl LuxTonemapPassPlan {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_RENDERER_TONEMAP_PASS_SCHEMA_VERSION,
        settings: FunLuxTonemapSettings::PRODUCT_DEFAULT,
    };

    /// Typed debug default — neutral Reinhard.  Useful for
    /// proving HDR accumulation + bloom paths without
    /// tonemap-induced artifacts.
    pub const DEBUG_NEUTRAL: Self = Self {
        schema_version: FUN_RENDERER_TONEMAP_PASS_SCHEMA_VERSION,
        settings: FunLuxTonemapSettings::DEBUG_NEUTRAL,
    };

    /// Typed cinematic default — ACES filmic with product
    /// custom curve.
    pub const CINEMATIC_ACES: Self = Self {
        schema_version: FUN_RENDERER_TONEMAP_PASS_SCHEMA_VERSION,
        settings: FunLuxTonemapSettings::CINEMATIC_ACES,
    };

    /// Typed builder: derive a typed plan from typed
    /// fun-lux tonemap settings.
    #[must_use]
    pub const fn from_settings(settings: FunLuxTonemapSettings) -> Self {
        Self {
            schema_version: FUN_RENDERER_TONEMAP_PASS_SCHEMA_VERSION,
            settings,
        }
    }

    /// Typed predicate: should the typed renderer register
    /// the typed tonemap pass this frame?  Every typed
    /// `FunLuxTonemapMode` variant (`NeutralReinhard`,
    /// `AcesFilmic`, `CustomFunLux`) registers the typed
    /// tonemap pass — fun-lux does not model a typed
    /// linear-passthrough mode at this layer
    /// (HDR-direct displays would handle that through the
    /// typed display output encoding stage, not by
    /// skipping tonemap).
    #[must_use]
    pub const fn registers_tonemap_pass(&self) -> bool {
        // Returns `true` for every typed mode today.  Kept
        // as a typed predicate so a future passthrough mode
        // (or a typed `Disabled` variant) can land without
        // changing callers.
        let _ = self.settings.mode;
        true
    }

    /// Typed list of typed frame-graph roles this typed
    /// plan emits this frame.
    #[must_use]
    pub fn typed_roles(&self) -> &'static [FrameGraphPassRole] {
        if self.registers_tonemap_pass() {
            &[FrameGraphPassRole::PostProcessToneMapping]
        } else {
            &[]
        }
    }
}

// ============================================================================
// Tests — Pass V2.5 typed tonemap pass plan
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_default_registers_tonemap_pass() {
        let plan = LuxTonemapPassPlan::PRODUCT_DEFAULT;
        assert!(plan.registers_tonemap_pass());
        let roles = plan.typed_roles();
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0], FrameGraphPassRole::PostProcessToneMapping);
    }

    #[test]
    fn debug_neutral_registers_tonemap_pass() {
        let plan = LuxTonemapPassPlan::DEBUG_NEUTRAL;
        assert!(plan.registers_tonemap_pass());
    }

    #[test]
    fn cinematic_aces_registers_tonemap_pass() {
        let plan = LuxTonemapPassPlan::CINEMATIC_ACES;
        assert!(plan.registers_tonemap_pass());
    }

    #[test]
    fn every_typed_mode_registers_tonemap_pass() {
        use fun_lux::FunLuxTonemapMode;
        for mode in FunLuxTonemapMode::ALL {
            let mut settings = FunLuxTonemapSettings::PRODUCT_DEFAULT;
            settings.mode = mode;
            let plan = LuxTonemapPassPlan::from_settings(settings);
            assert!(
                plan.registers_tonemap_pass(),
                "{:?} should register tonemap",
                mode,
            );
            assert_eq!(plan.typed_roles().len(), 1);
        }
    }
}
