//! Pass C0 / C1 — typed cloud resource taxonomy.
//!
//! Names the typed GPU resources the typed cloud
//! executor will allocate when Pass C2+ wires the typed
//! cloud frame graph.  Today this module only exposes the
//! typed resource taxonomy (a typed enum of resource
//! kinds + a typed bundle); the typed `wgpu::Buffer` /
//! `wgpu::Texture` allocation lives in a future pass.
//!
//! The typed taxonomy is intentionally aligned with the
//! typed legacy `fun_render::sky::render::resources`
//! shape so the migration is a name-and-feature-flag
//! lift, not a typed redesign.

pub const FUN_RENDERER_CLOUD_RESOURCES_SCHEMA_VERSION: u16 = 1;
pub const FUN_RENDERER_CLOUD_RESOURCE_KIND_COUNT: usize = 10;

/// Typed Pass C0 / C1 cloud resource kind.  Names the
/// typed GPU resources the typed cloud pipeline allocates.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CloudResourceKind {
    /// Typed weather map — 2D R16F (or RGBA8 for typed
    /// debug overlay).  Sampled by the typed raymarch.
    #[default]
    WeatherMap,
    /// Typed cloud shape noise — 3D R8 noise texture.
    ShapeNoise,
    /// Typed cloud detail noise — 3D R8 detail texture.
    DetailNoise,
    /// Typed cloud color target — RGBA16F.  Output of the
    /// typed raymarch.
    CloudColor,
    /// Typed cloud transmittance target — R16F.  Output of
    /// the typed raymarch.
    CloudTransmittance,
    /// Typed temporal history target A.
    HistoryA,
    /// Typed temporal history target B.
    HistoryB,
    /// Typed cloud world-shadow mask target.  Pass C2+.
    WorldShadowMask,
    /// Typed cloud parameters uniform buffer.
    ParamsUniform,
    /// Typed cloud debug overlay texture (for the typed
    /// `CloudDebugOverlay` modes).
    DebugOverlay,
}

impl CloudResourceKind {
    pub const ALL: [Self; FUN_RENDERER_CLOUD_RESOURCE_KIND_COUNT] = [
        Self::WeatherMap,
        Self::ShapeNoise,
        Self::DetailNoise,
        Self::CloudColor,
        Self::CloudTransmittance,
        Self::HistoryA,
        Self::HistoryB,
        Self::WorldShadowMask,
        Self::ParamsUniform,
        Self::DebugOverlay,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WeatherMap => "cloud_weather_map",
            Self::ShapeNoise => "cloud_shape_noise",
            Self::DetailNoise => "cloud_detail_noise",
            Self::CloudColor => "cloud_color",
            Self::CloudTransmittance => "cloud_transmittance",
            Self::HistoryA => "cloud_history_a",
            Self::HistoryB => "cloud_history_b",
            Self::WorldShadowMask => "cloud_world_shadow_mask",
            Self::ParamsUniform => "cloud_params_uniform",
            Self::DebugOverlay => "cloud_debug_overlay",
        }
    }

    /// Typed predicate: is this resource persistent across
    /// frames (allocated once, reused)?  Typed history
    /// targets + the typed world-shadow mask + the typed
    /// noise textures are persistent.  Typed per-frame
    /// targets (color, transmittance, params, debug
    /// overlay) are frame-local.
    #[must_use]
    pub const fn is_persistent(self) -> bool {
        matches!(
            self,
            Self::ShapeNoise
                | Self::DetailNoise
                | Self::WeatherMap
                | Self::HistoryA
                | Self::HistoryB
                | Self::WorldShadowMask,
        )
    }
}

/// Typed Pass C0 / C1 cloud resource bundle.  A typed
/// const list of the typed resources the typed cloud
/// executor allocates at boot.  Used by the typed
/// `cloud_diagnostics` module to audit resource counts.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CloudResourceBundle {
    pub schema_version: u16,
}

impl CloudResourceBundle {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_RENDERER_CLOUD_RESOURCES_SCHEMA_VERSION,
    };

    /// Typed count of typed persistent resources in the
    /// typed bundle.
    #[must_use]
    pub fn persistent_resource_count(&self) -> u32 {
        CloudResourceKind::ALL
            .iter()
            .filter(|k| k.is_persistent())
            .count() as u32
    }

    /// Typed count of typed frame-local resources in the
    /// typed bundle.
    #[must_use]
    pub fn frame_local_resource_count(&self) -> u32 {
        CloudResourceKind::ALL
            .iter()
            .filter(|k| !k.is_persistent())
            .count() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_resource_kind_taxonomy_is_dense() {
        assert_eq!(
            CloudResourceKind::ALL.len(),
            FUN_RENDERER_CLOUD_RESOURCE_KIND_COUNT
        );
        let mut seen = hashbrown::HashSet::new();
        for kind in CloudResourceKind::ALL {
            assert!(seen.insert(kind.as_str()), "duplicate: {}", kind.as_str());
        }
    }

    #[test]
    fn cloud_resource_kind_persistence_split() {
        // Persistent: shape noise, detail noise, weather
        // map, history A, history B, world shadow mask.
        assert!(CloudResourceKind::ShapeNoise.is_persistent());
        assert!(CloudResourceKind::DetailNoise.is_persistent());
        assert!(CloudResourceKind::WeatherMap.is_persistent());
        assert!(CloudResourceKind::HistoryA.is_persistent());
        assert!(CloudResourceKind::HistoryB.is_persistent());
        assert!(CloudResourceKind::WorldShadowMask.is_persistent());
        // Frame-local: cloud color, transmittance, params,
        // debug overlay.
        assert!(!CloudResourceKind::CloudColor.is_persistent());
        assert!(!CloudResourceKind::CloudTransmittance.is_persistent());
        assert!(!CloudResourceKind::ParamsUniform.is_persistent());
        assert!(!CloudResourceKind::DebugOverlay.is_persistent());
    }

    #[test]
    fn cloud_resource_bundle_counts() {
        let b = CloudResourceBundle::PRODUCT_DEFAULT;
        assert_eq!(b.persistent_resource_count(), 6);
        assert_eq!(b.frame_local_resource_count(), 4);
        assert_eq!(
            b.persistent_resource_count() + b.frame_local_resource_count(),
            FUN_RENDERER_CLOUD_RESOURCE_KIND_COUNT as u32,
        );
    }
}
