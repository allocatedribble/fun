//! Pass V2.5 — typed renderer-side exposure pass plan.
//!
//! Translates the typed fun-lux Pass 5
//! `FunLuxExposureSettings` (5 typed exposure modes,
//! typed histogram config, typed adaptation speeds, 7
//! typed controls) into a typed renderer frame-graph
//! plan that registers the typed
//! `PostProcessExposureHistogram` + `PostProcessExposureAdapt`
//! roles when the typed mode requires them.
//!
//! The typed plan is consumed by the typed graph compiler
//! after the typed bloom passes are registered but before
//! the typed tonemap pass.  The typed order keys come
//! from [`crate::hdr_pipeline::hdr_pipeline_order_key`].

use fun_lux::{FunLuxExposureMode, FunLuxExposureSettings};

use crate::frame_graph::FrameGraphPassRole;

pub const FUN_RENDERER_EXPOSURE_PASS_SCHEMA_VERSION: u16 = 1;

/// Typed Pass V2.5 exposure pass plan.  Carries the typed
/// fun-lux exposure settings + a typed list of typed
/// renderer roles the typed graph compiler should
/// register this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxExposurePassPlan {
    pub schema_version: u16,
    pub settings: FunLuxExposureSettings,
}

impl LuxExposurePassPlan {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_RENDERER_EXPOSURE_PASS_SCHEMA_VERSION,
        settings: FunLuxExposureSettings::PRODUCT_DEFAULT,
    };

    pub const COLD_DEFAULT: Self = Self {
        schema_version: FUN_RENDERER_EXPOSURE_PASS_SCHEMA_VERSION,
        settings: FunLuxExposureSettings::COLD_DEFAULT,
    };

    /// Typed builder: derive a typed plan from typed
    /// fun-lux exposure settings.
    #[must_use]
    pub const fn from_settings(settings: FunLuxExposureSettings) -> Self {
        Self {
            schema_version: FUN_RENDERER_EXPOSURE_PASS_SCHEMA_VERSION,
            settings,
        }
    }

    /// Typed predicate: should the typed renderer register
    /// the typed histogram pass this frame?  Returns `true`
    /// only when the typed mode requires histogram input
    /// (Auto, CameraWeightedAuto, GameplayZoneOverride).
    #[must_use]
    pub const fn registers_histogram_pass(&self) -> bool {
        self.settings.mode.requires_histogram()
    }

    /// Typed predicate: should the typed renderer register
    /// the typed adapt pass this frame?  Returns `true`
    /// when the typed mode drives an auto-exposure path
    /// (same modes as the histogram).  Manual and
    /// CinematicLocked skip the adapt pass.
    #[must_use]
    pub const fn registers_adapt_pass(&self) -> bool {
        self.settings.drives_auto_exposure_pass()
    }

    /// Typed list of typed frame-graph roles this typed
    /// plan emits this frame.  The typed graph compiler
    /// walks this list to register typed passes in typed
    /// order.
    ///
    /// Returns at most two typed roles
    /// (`PostProcessExposureHistogram` +
    /// `PostProcessExposureAdapt`); the typed slice length
    /// reflects which typed passes are active.
    #[must_use]
    pub fn typed_roles(&self) -> &'static [FrameGraphPassRole] {
        if self.registers_histogram_pass() && self.registers_adapt_pass() {
            &[
                FrameGraphPassRole::PostProcessExposureHistogram,
                FrameGraphPassRole::PostProcessExposureAdapt,
            ]
        } else if self.registers_adapt_pass() {
            // Manual / CinematicLocked don't reach here —
            // both `requires_histogram` and
            // `drives_auto_exposure_pass` are gated on the
            // typed `Auto` family.  Keep the typed branch
            // for forward-compat.
            &[FrameGraphPassRole::PostProcessExposureAdapt]
        } else {
            &[]
        }
    }
}

// ============================================================================
// Tests — Pass V2.5 typed exposure pass plan
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_default_registers_both_exposure_passes() {
        let plan = LuxExposurePassPlan::PRODUCT_DEFAULT;
        assert!(plan.registers_histogram_pass());
        assert!(plan.registers_adapt_pass());
        let roles = plan.typed_roles();
        assert_eq!(roles.len(), 2);
        assert_eq!(roles[0], FrameGraphPassRole::PostProcessExposureHistogram);
        assert_eq!(roles[1], FrameGraphPassRole::PostProcessExposureAdapt);
    }

    #[test]
    fn manual_mode_skips_histogram_and_adapt() {
        let mut settings = FunLuxExposureSettings::PRODUCT_DEFAULT;
        settings.mode = FunLuxExposureMode::Manual;
        let plan = LuxExposurePassPlan::from_settings(settings);
        assert!(!plan.registers_histogram_pass());
        assert!(!plan.registers_adapt_pass());
        assert_eq!(plan.typed_roles().len(), 0);
    }

    #[test]
    fn cinematic_locked_mode_skips_histogram_and_adapt() {
        let mut settings = FunLuxExposureSettings::PRODUCT_DEFAULT;
        settings.mode = FunLuxExposureMode::CinematicLocked;
        let plan = LuxExposurePassPlan::from_settings(settings);
        assert!(!plan.registers_histogram_pass());
        assert!(!plan.registers_adapt_pass());
    }

    #[test]
    fn cold_default_uses_manual_mode_and_emits_no_roles() {
        let plan = LuxExposurePassPlan::COLD_DEFAULT;
        assert!(!plan.registers_histogram_pass());
        assert_eq!(plan.typed_roles().len(), 0);
    }
}
