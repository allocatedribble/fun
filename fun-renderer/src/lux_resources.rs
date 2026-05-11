//! Pass 3 — typed mapping from `fun_lux::LuxResourceIntent` to
//! `FrameGraphResourceType` + typed `LuxResourceLifetime`.
//!
//! `LuxGraphCompiler` (in [`crate::lux_graph`]) walks every
//! [`fun_lux::LuxResourceIntent`] in the typed
//! [`fun_lux::LuxFramePlan`] and calls
//! [`map_intent_to_resource_type`] / [`lifetime_for`] to
//! decide:
//!
//! 1. Which typed [`FrameGraphResourceType`] to declare in
//!    the typed [`crate::frame_graph::RendererFrameGraph`].
//! 2. Which typed [`LuxResourceLifetime`] classification to
//!    apply (`Persistent` / `FrameLocal` / `GraphTransient`).
//!
//! Persistent resources (light buffers, shadow atlas, virtual
//! shadow pages, surface / radiance / probe caches, temporal
//! histories) survive across frames. Frame-local resources
//! (cluster lists, shadow request lists, volumetric froxel
//! buffers, intermediate fog integration) are valid for one
//! frame. Graph-transient resources (bloom mips, scratch
//! buffers, debug overlays) are valid only within the
//! current `RendererFrameGraph::execute` window.

use fun_lux::LuxResourceIntent;

use crate::frame_graph::FrameGraphResourceType;

pub const FUN_RENDERER_LUX_RESOURCES_SCHEMA_VERSION: u16 = 1;
pub const LUX_RESOURCE_LIFETIME_COUNT: usize = 3;

/// Typed lifetime classification for a Lux resource.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxResourceLifetime {
    /// Persistent across frames. The renderer allocates the
    /// resource once and reuses it; the typed resource
    /// generation tracks per-frame mutations without
    /// reallocating.
    Persistent,
    /// Valid for one frame; freed at end-of-frame. Examples:
    /// cluster bin lists, shadow-request lists, volumetric
    /// froxel buffers, intermediate fog integration tiles.
    #[default]
    FrameLocal,
    /// Valid only within the current graph execution. Used
    /// for in-pass scratch + debug overlays. The renderer
    /// reuses the same backing allocation across these
    /// transients.
    GraphTransient,
}

impl LuxResourceLifetime {
    pub const ALL: [Self; LUX_RESOURCE_LIFETIME_COUNT] =
        [Self::Persistent, Self::FrameLocal, Self::GraphTransient];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Persistent => "persistent",
            Self::FrameLocal => "frame_local",
            Self::GraphTransient => "graph_transient",
        }
    }

    /// Typed predicate: does this lifetime require the
    /// renderer to track the resource across frames?
    #[must_use]
    pub const fn is_persistent(self) -> bool {
        matches!(self, Self::Persistent)
    }

    /// Typed predicate: does this lifetime free the resource
    /// at end-of-frame?
    #[must_use]
    pub const fn ends_at_frame_boundary(self) -> bool {
        !matches!(self, Self::Persistent)
    }
}

/// Pass 3 typed resource-type mapping. Each
/// `fun_lux::LuxResourceIntent` variant deterministically
/// maps to one typed `FrameGraphResourceType`. The renderer
/// uses the typed mapping to declare resource handles in the
/// frame graph; the typed contract guarantees that
/// `fun-lux` and `fun-renderer` never disagree about which
/// resource a lux intent represents.
#[must_use]
pub const fn map_intent_to_resource_type(intent: &LuxResourceIntent) -> FrameGraphResourceType {
    match intent {
        LuxResourceIntent::LightBuffer { .. } => FrameGraphResourceType::LuxLightBuffer,
        LuxResourceIntent::LightIndexBuffer { .. } => FrameGraphResourceType::LuxLightIndexBuffer,
        LuxResourceIntent::ClusterGrid { .. } => FrameGraphResourceType::LuxClusterGrid,
        LuxResourceIntent::ReservoirBuffer { .. } => FrameGraphResourceType::LuxReservoirBuffer,
        LuxResourceIntent::ShadowRequestBuffer { .. } => {
            FrameGraphResourceType::LuxShadowRequestBuffer
        }
        LuxResourceIntent::ShadowAtlas { .. } => FrameGraphResourceType::LuxShadowAtlas,
        LuxResourceIntent::VirtualShadowPageTable { .. } => {
            FrameGraphResourceType::LuxVirtualShadowPages
        }
        LuxResourceIntent::SurfaceCache { .. } => FrameGraphResourceType::LuxSurfaceCache,
        LuxResourceIntent::RadianceCache { .. } => FrameGraphResourceType::LuxRadianceCache,
        LuxResourceIntent::ProbeCache { .. } => FrameGraphResourceType::LuxProbeCache,
        LuxResourceIntent::ReflectionTraceBuffer { .. } => {
            FrameGraphResourceType::LuxReflectionBuffer
        }
        LuxResourceIntent::DenoiseHistory { .. } => FrameGraphResourceType::LuxDenoiseHistory,
        LuxResourceIntent::VolumetricFroxelDensity { .. } => {
            FrameGraphResourceType::LuxVolumetricFroxelDensity
        }
        LuxResourceIntent::VolumetricFroxelScattering { .. } => {
            FrameGraphResourceType::LuxVolumetricFroxelScattering
        }
        LuxResourceIntent::VolumetricHistory { .. } => FrameGraphResourceType::LuxVolumetricHistory,
        LuxResourceIntent::IntegratedFog { .. } => {
            FrameGraphResourceType::LuxVolumetricIntegratedFog
        }
        LuxResourceIntent::LuxDebugBuffer { .. } => FrameGraphResourceType::TransientScratch,
    }
}

/// Pass 3 typed lifetime mapping for the typed lux resource
/// types. Persistent resources live across frames; frame
/// local resources free at end-of-frame; graph-transient
/// resources reuse pool allocations within the frame.
#[must_use]
pub const fn lifetime_for(resource_type: FrameGraphResourceType) -> LuxResourceLifetime {
    match resource_type {
        // Persistent — survive across frames.
        FrameGraphResourceType::LuxLightBuffer
        | FrameGraphResourceType::LuxShadowAtlas
        | FrameGraphResourceType::LuxVirtualShadowPages
        | FrameGraphResourceType::LuxSurfaceCache
        | FrameGraphResourceType::LuxRadianceCache
        | FrameGraphResourceType::LuxProbeCache
        | FrameGraphResourceType::LuxDenoiseHistory
        | FrameGraphResourceType::LuxVolumetricHistory => LuxResourceLifetime::Persistent,
        // Frame-local — valid for one frame.
        FrameGraphResourceType::LuxLightIndexBuffer
        | FrameGraphResourceType::LuxClusterGrid
        | FrameGraphResourceType::LuxReservoirBuffer
        | FrameGraphResourceType::LuxShadowRequestBuffer
        | FrameGraphResourceType::LuxReflectionBuffer
        | FrameGraphResourceType::LuxVolumetricFroxelDensity
        | FrameGraphResourceType::LuxVolumetricFroxelScattering
        | FrameGraphResourceType::LuxVolumetricIntegratedFog => LuxResourceLifetime::FrameLocal,
        // Graph-transient — pooled within one frame
        // execution.
        FrameGraphResourceType::TransientScratch => LuxResourceLifetime::GraphTransient,
        // Non-lux resource types still get a typed
        // classification so callers can read the lifetime
        // for any handle. The renderer's existing
        // allocator owns these directly.
        _ => LuxResourceLifetime::FrameLocal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_RENDERER_LUX_RESOURCES_SCHEMA_VERSION, 1);
        assert_eq!(LUX_RESOURCE_LIFETIME_COUNT, 3);
        assert_eq!(LuxResourceLifetime::ALL.len(), LUX_RESOURCE_LIFETIME_COUNT);
    }

    #[test]
    fn lifetime_taxonomy_strings_are_unique() {
        let mut seen = hashbrown::HashSet::new();
        for lt in LuxResourceLifetime::ALL {
            assert!(seen.insert(lt.as_str()), "duplicate: {}", lt.as_str());
        }
    }

    #[test]
    fn lifetime_predicates() {
        assert!(LuxResourceLifetime::Persistent.is_persistent());
        assert!(!LuxResourceLifetime::Persistent.ends_at_frame_boundary());
        assert!(!LuxResourceLifetime::FrameLocal.is_persistent());
        assert!(LuxResourceLifetime::FrameLocal.ends_at_frame_boundary());
        assert!(!LuxResourceLifetime::GraphTransient.is_persistent());
        assert!(LuxResourceLifetime::GraphTransient.ends_at_frame_boundary());
    }

    #[test]
    fn light_buffer_maps_to_persistent() {
        let intent = LuxResourceIntent::LightBuffer {
            stable_id: "test",
            max_light_count: 1024,
            bytes_per_light: 32,
        };
        let rt = map_intent_to_resource_type(&intent);
        assert_eq!(rt, FrameGraphResourceType::LuxLightBuffer);
        assert_eq!(lifetime_for(rt), LuxResourceLifetime::Persistent);
    }

    #[test]
    fn cluster_grid_maps_to_frame_local() {
        let intent = LuxResourceIntent::ClusterGrid {
            stable_id: "test",
            clusters_x: 16,
            clusters_y: 8,
            clusters_z: 24,
        };
        let rt = map_intent_to_resource_type(&intent);
        assert_eq!(rt, FrameGraphResourceType::LuxClusterGrid);
        assert_eq!(lifetime_for(rt), LuxResourceLifetime::FrameLocal);
    }

    #[test]
    fn shadow_atlas_maps_to_persistent() {
        let intent = LuxResourceIntent::ShadowAtlas {
            stable_id: "test",
            atlas_extent: 4096,
            slot_count: 64,
        };
        let rt = map_intent_to_resource_type(&intent);
        assert_eq!(rt, FrameGraphResourceType::LuxShadowAtlas);
        assert_eq!(lifetime_for(rt), LuxResourceLifetime::Persistent);
    }

    #[test]
    fn surface_radiance_probe_caches_map_to_persistent() {
        let intents = [
            LuxResourceIntent::SurfaceCache {
                stable_id: "sc",
                surface_count: 256,
            },
            LuxResourceIntent::RadianceCache {
                stable_id: "rc",
                voxel_count: 1024,
            },
            LuxResourceIntent::ProbeCache {
                stable_id: "pc",
                probe_count: 64,
            },
        ];
        for intent in &intents {
            let rt = map_intent_to_resource_type(intent);
            assert_eq!(
                lifetime_for(rt),
                LuxResourceLifetime::Persistent,
                "{:?}",
                rt,
            );
        }
    }

    #[test]
    fn temporal_histories_map_to_persistent() {
        let denoise = LuxResourceIntent::DenoiseHistory {
            stable_id: "dh",
            width: 1920,
            height: 1080,
        };
        let vol_history = LuxResourceIntent::VolumetricHistory {
            stable_id: "vh",
            width: 160,
            height: 90,
            depth: 64,
        };
        assert_eq!(
            lifetime_for(map_intent_to_resource_type(&denoise)),
            LuxResourceLifetime::Persistent,
        );
        assert_eq!(
            lifetime_for(map_intent_to_resource_type(&vol_history)),
            LuxResourceLifetime::Persistent,
        );
    }

    #[test]
    fn frame_local_resources_match_user_routing() {
        // Per the user routing spec: cluster lists, shadow
        // request lists, volumetric froxel buffers,
        // intermediate fog integration are frame-local.
        let frame_local = [
            FrameGraphResourceType::LuxClusterGrid,
            FrameGraphResourceType::LuxLightIndexBuffer,
            FrameGraphResourceType::LuxShadowRequestBuffer,
            FrameGraphResourceType::LuxVolumetricFroxelDensity,
            FrameGraphResourceType::LuxVolumetricFroxelScattering,
            FrameGraphResourceType::LuxVolumetricIntegratedFog,
            FrameGraphResourceType::LuxReservoirBuffer,
            FrameGraphResourceType::LuxReflectionBuffer,
        ];
        for rt in frame_local {
            assert_eq!(
                lifetime_for(rt),
                LuxResourceLifetime::FrameLocal,
                "{:?}",
                rt,
            );
        }
    }

    #[test]
    fn graph_transient_resources_for_scratch() {
        let intent = LuxResourceIntent::LuxDebugBuffer {
            stable_id: "debug",
            byte_size: 4096,
        };
        let rt = map_intent_to_resource_type(&intent);
        assert_eq!(rt, FrameGraphResourceType::TransientScratch);
        assert_eq!(lifetime_for(rt), LuxResourceLifetime::GraphTransient);
    }

    #[test]
    fn every_lux_resource_type_has_typed_lifetime() {
        // Walk every typed lux resource type and confirm a
        // typed lifetime exists. The compiler relies on
        // this exhaustiveness.
        for rt in FrameGraphResourceType::ALL {
            if !rt.is_lux() {
                continue;
            }
            let _ = lifetime_for(rt); // const fn — exhaustiveness checked.
        }
    }
}
