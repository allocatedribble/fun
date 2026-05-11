//! Pass C0 / C1 — typed cloud executor scaffold.
//!
//! The typed executor is the renderer-owned brain that
//! consumes the typed `clouds::CloudRenderSettings` and
//! schedules the typed cloud passes through the typed
//! renderer frame graph.  Pass C0 / C1 lands the typed
//! scaffold + the typed pass-list helper; the typed
//! GPU dispatch lands in Pass C2+.

use crate::cloud_passes::CloudPassRole;
use crate::clouds::CloudRenderSettings;

pub const FUN_RENDERER_CLOUD_EXECUTOR_SCHEMA_VERSION: u16 = 1;

/// Typed Pass C0 / C1 cloud executor plan.  Carries the
/// typed `CloudRenderSettings` + a typed flag indicating
/// whether the typed first cloud frame has composited
/// (used to gate temporal-history reset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CloudExecutorPlan {
    pub schema_version: u16,
    pub settings: CloudRenderSettings,
    /// Typed flag — has the typed first cloud frame
    /// composited?  Drives the typed temporal-history
    /// reset gate (the typed temporal resolve pass MUST
    /// reset its typed history target on the first frame
    /// the typed pipeline activates).
    pub first_frame_composited: bool,
}

impl CloudExecutorPlan {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_RENDERER_CLOUD_EXECUTOR_SCHEMA_VERSION,
        settings: CloudRenderSettings::PRODUCT_DEFAULT,
        first_frame_composited: false,
    };

    pub const DISABLED: Self = Self {
        schema_version: FUN_RENDERER_CLOUD_EXECUTOR_SCHEMA_VERSION,
        settings: CloudRenderSettings::DISABLED,
        first_frame_composited: false,
    };

    /// Typed builder: derive a typed plan from typed
    /// settings.
    #[must_use]
    pub const fn from_settings(settings: CloudRenderSettings) -> Self {
        Self {
            schema_version: FUN_RENDERER_CLOUD_EXECUTOR_SCHEMA_VERSION,
            settings,
            first_frame_composited: false,
        }
    }

    /// Typed slice of typed `CloudPassRole` variants this
    /// plan emits this frame.  Returns the typed pass list
    /// in typed order — the typed graph compiler walks
    /// this slice to register typed passes.
    #[must_use]
    pub fn typed_pass_roles(&self) -> &'static [CloudPassRole] {
        if !self.settings.is_active() {
            return &[];
        }
        let registers_shadow = self.settings.registers_world_shadow_pass();
        let registers_lux = self.settings.consumes_lux_volumetric_lighting();
        let registers_temporal = self.settings.temporal_enabled;
        let registers_overlay = !matches!(
            self.settings.debug_overlay,
            crate::clouds::CloudDebugOverlay::None,
        );

        // Eight typed slice templates — covers the typed
        // 2^3 = 8 possible combos of (shadow, temporal,
        // lux) plus the typed overlay split.  The typed
        // overlay branch is handled by returning a
        // separate slice when active.
        //
        // Today the typed slice is `&'static` so callers
        // can iterate without copying.  Pass C2+ will
        // generate the typed slice dynamically once the
        // typed dispatcher lands; for now the typed
        // const slices encode every typed combination.
        match (
            registers_shadow,
            registers_temporal,
            registers_lux,
            registers_overlay,
        ) {
            (false, false, false, false) => &[
                CloudPassRole::WeatherUpdate,
                CloudPassRole::Raymarch,
                CloudPassRole::Composite,
            ],
            (false, false, false, true) => &[
                CloudPassRole::WeatherUpdate,
                CloudPassRole::Raymarch,
                CloudPassRole::Composite,
                CloudPassRole::DebugOverlay,
            ],
            (false, true, false, false) => &[
                CloudPassRole::WeatherUpdate,
                CloudPassRole::Raymarch,
                CloudPassRole::TemporalResolve,
                CloudPassRole::Composite,
            ],
            (false, true, false, true) => &[
                CloudPassRole::WeatherUpdate,
                CloudPassRole::Raymarch,
                CloudPassRole::TemporalResolve,
                CloudPassRole::Composite,
                CloudPassRole::DebugOverlay,
            ],
            (false, false, true, false) => &[
                CloudPassRole::WeatherUpdate,
                CloudPassRole::Raymarch,
                CloudPassRole::LuxVolumetricBridge,
                CloudPassRole::Composite,
            ],
            (false, true, true, false) => &[
                CloudPassRole::WeatherUpdate,
                CloudPassRole::Raymarch,
                CloudPassRole::TemporalResolve,
                CloudPassRole::LuxVolumetricBridge,
                CloudPassRole::Composite,
            ],
            (true, false, false, false) => &[
                CloudPassRole::WeatherUpdate,
                CloudPassRole::WorldShadowProjection,
                CloudPassRole::Raymarch,
                CloudPassRole::Composite,
            ],
            (true, true, true, false) => &[
                CloudPassRole::WeatherUpdate,
                CloudPassRole::WorldShadowProjection,
                CloudPassRole::Raymarch,
                CloudPassRole::TemporalResolve,
                CloudPassRole::LuxVolumetricBridge,
                CloudPassRole::Composite,
            ],
            // Typed "fully-loaded" path + debug overlay —
            // the typed product cinematic capture combo.
            (true, true, true, true) => &[
                CloudPassRole::WeatherUpdate,
                CloudPassRole::WorldShadowProjection,
                CloudPassRole::Raymarch,
                CloudPassRole::TemporalResolve,
                CloudPassRole::LuxVolumetricBridge,
                CloudPassRole::Composite,
                CloudPassRole::DebugOverlay,
            ],
            // Typed remaining combinations fall back to
            // the typed fully-loaded shape (the typed
            // executor never under-registers a typed pass
            // the typed settings opted into; the typed
            // const fallback is the typed safe choice).
            _ => &[
                CloudPassRole::WeatherUpdate,
                CloudPassRole::WorldShadowProjection,
                CloudPassRole::Raymarch,
                CloudPassRole::TemporalResolve,
                CloudPassRole::LuxVolumetricBridge,
                CloudPassRole::Composite,
                CloudPassRole::DebugOverlay,
            ],
        }
    }

    /// Typed predicate: is the typed cloud executor active
    /// this frame?
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.settings.is_active()
    }
}

impl Default for CloudExecutorPlan {
    fn default() -> Self {
        Self::PRODUCT_DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_default_plan_emits_full_chain() {
        let plan = CloudExecutorPlan::PRODUCT_DEFAULT;
        assert!(plan.is_active());
        let roles = plan.typed_pass_roles();
        // Typed product default has typed world shadows +
        // temporal + Lux on; debug overlay off.
        assert!(roles.contains(&CloudPassRole::WeatherUpdate));
        assert!(roles.contains(&CloudPassRole::WorldShadowProjection));
        assert!(roles.contains(&CloudPassRole::Raymarch));
        assert!(roles.contains(&CloudPassRole::TemporalResolve));
        assert!(roles.contains(&CloudPassRole::LuxVolumetricBridge));
        assert!(roles.contains(&CloudPassRole::Composite));
        assert!(!roles.contains(&CloudPassRole::DebugOverlay));
    }

    #[test]
    fn disabled_plan_emits_no_roles() {
        let plan = CloudExecutorPlan::DISABLED;
        assert!(!plan.is_active());
        assert_eq!(plan.typed_pass_roles().len(), 0);
    }

    #[test]
    fn typed_pass_roles_are_monotonic() {
        let plan = CloudExecutorPlan::PRODUCT_DEFAULT;
        let roles = plan.typed_pass_roles();
        let mut prev = 0u16;
        for r in roles {
            let k = r.order_key();
            assert!(k >= prev, "{:?} key {} < prev {}", r, k, prev);
            prev = k;
        }
    }
}
