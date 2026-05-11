//! Pass C0 / C1 — typed cloud pass taxonomy.
//!
//! Names the typed cloud frame-graph passes the typed
//! cloud executor will dispatch when Pass C2+ wires the
//! typed cloud frame graph.  Today this module only
//! exposes the typed pass taxonomy + the typed
//! order-key helper; the actual GPU dispatch lives in a
//! future pass.

pub const FUN_RENDERER_CLOUD_PASSES_SCHEMA_VERSION: u16 = 1;

/// Typed Pass C0 / C1 cloud pass role.  Mirrors the typed
/// legacy `fun_render::sky::render` pass shape and adds
/// the typed world-shadow + Lux-lighting hooks the user
/// audit calls out.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CloudPassRole {
    /// Typed weather-map update pass (per-frame profile
    /// blend).
    #[default]
    WeatherUpdate,
    /// Typed cloud raymarch compute pass.
    Raymarch,
    /// Typed temporal reconstruction resolve.
    TemporalResolve,
    /// Typed cloud → HDR scene composite (depth-aware so
    /// foreground geometry occludes clouds).
    Composite,
    /// Pass C2+ — typed cloud world-shadow projection.
    WorldShadowProjection,
    /// Pass C3+ — typed bridge that feeds the typed cloud
    /// transmittance / scattering into the typed Lux
    /// volumetric froxel.
    LuxVolumetricBridge,
    /// Typed debug overlay composite.
    DebugOverlay,
}

impl CloudPassRole {
    pub const ALL: [Self; 7] = [
        Self::WeatherUpdate,
        Self::Raymarch,
        Self::TemporalResolve,
        Self::Composite,
        Self::WorldShadowProjection,
        Self::LuxVolumetricBridge,
        Self::DebugOverlay,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WeatherUpdate => "cloud_weather_update",
            Self::Raymarch => "cloud_raymarch",
            Self::TemporalResolve => "cloud_temporal_resolve",
            Self::Composite => "cloud_composite",
            Self::WorldShadowProjection => "cloud_world_shadow_projection",
            Self::LuxVolumetricBridge => "cloud_lux_volumetric_bridge",
            Self::DebugOverlay => "cloud_debug_overlay",
        }
    }

    /// Typed order key the typed cloud executor uses to
    /// schedule typed passes.  Cloud passes run AFTER the
    /// typed lighting + volumetric work (so they can
    /// sample typed Lux output) but BEFORE the typed
    /// composite into HDR scene color.
    ///
    ///   WeatherUpdate          (50)
    ///     → WorldShadowProjection (75) — Pass C2+
    ///     → Raymarch              (100)
    ///     → TemporalResolve       (200)
    ///     → LuxVolumetricBridge   (250) — Pass C3+
    ///     → Composite             (300)
    ///     → DebugOverlay          (400)
    #[must_use]
    pub const fn order_key(self) -> u16 {
        match self {
            Self::WeatherUpdate => 50,
            Self::WorldShadowProjection => 75,
            Self::Raymarch => 100,
            Self::TemporalResolve => 200,
            Self::LuxVolumetricBridge => 250,
            Self::Composite => 300,
            Self::DebugOverlay => 400,
        }
    }
}

/// Typed Pass C0 / C1 const predicate: the typed cloud
/// composite runs BEFORE the typed debug overlay pass.
#[must_use]
pub const fn cloud_composite_runs_before_debug_overlay() -> bool {
    CloudPassRole::Composite.order_key() < CloudPassRole::DebugOverlay.order_key()
}

/// Typed Pass C0 / C1 const predicate: the typed cloud
/// raymarch runs AFTER the typed weather update.
#[must_use]
pub const fn cloud_raymarch_runs_after_weather_update() -> bool {
    CloudPassRole::Raymarch.order_key() > CloudPassRole::WeatherUpdate.order_key()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_pass_role_taxonomy_is_dense() {
        assert_eq!(CloudPassRole::ALL.len(), 7);
        let mut seen = std::collections::HashSet::new();
        for r in CloudPassRole::ALL {
            assert!(seen.insert(r.as_str()), "duplicate: {}", r.as_str());
            // Every typed name starts with "cloud_".
            assert!(r.as_str().starts_with("cloud_"));
        }
    }

    #[test]
    fn cloud_pass_role_order_is_monotonic_along_user_chain() {
        let chain = [
            CloudPassRole::WeatherUpdate,
            CloudPassRole::WorldShadowProjection,
            CloudPassRole::Raymarch,
            CloudPassRole::TemporalResolve,
            CloudPassRole::LuxVolumetricBridge,
            CloudPassRole::Composite,
            CloudPassRole::DebugOverlay,
        ];
        let mut prev = 0u16;
        for r in chain {
            let k = r.order_key();
            assert!(k >= prev, "{:?} key {} < prev {}", r, k, prev);
            prev = k;
        }
    }

    #[test]
    fn cloud_composite_runs_before_debug_overlay_predicate() {
        assert!(cloud_composite_runs_before_debug_overlay());
    }

    #[test]
    fn cloud_raymarch_runs_after_weather_update_predicate() {
        assert!(cloud_raymarch_runs_after_weather_update());
    }
}
