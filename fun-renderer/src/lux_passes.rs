//! Pass 3 — typed mapping from `fun_lux::LuxPassRequest` to
//! `FrameGraphPassRole` + typed read / write resource tables.
//!
//! `LuxGraphCompiler` (in [`crate::lux_graph`]) walks every
//! [`fun_lux::LuxPassRequest`] in the typed
//! [`fun_lux::LuxFramePlan`] and calls:
//!
//! - [`lux_role_for`] to pick the typed
//!   [`crate::frame_graph::FrameGraphPassRole`].
//! - [`typed_resource_inputs`] / [`typed_resource_outputs`]
//!   to enumerate the typed
//!   [`crate::frame_graph::FrameGraphResourceType`] reads +
//!   writes the pass declares.
//!
//! Every typed Lux pass MUST declare at least one resource
//! read OR write — the typed contract refuses empty passes
//! via [`crate::frame_graph::FrameGraphValidationFailureCode::LuxPassDeclaresNoReadsOrWrites`].

use fun_lux::LuxPassRequest;

use crate::frame_graph::{FrameGraphPassRole, FrameGraphResourceType};

pub const FUN_RENDERER_LUX_PASSES_SCHEMA_VERSION: u16 = 1;

/// Pass 3 typed pass-role mapping. Each
/// `fun_lux::LuxPassRequest` variant deterministically maps
/// to one typed `FrameGraphPassRole`.
#[must_use]
pub const fn lux_role_for(request: &LuxPassRequest) -> FrameGraphPassRole {
    match request {
        LuxPassRequest::UploadLightBuffers(_) => FrameGraphPassRole::LuxUploadLightBuffers,
        LuxPassRequest::ClusterLights(_) => FrameGraphPassRole::LuxClusterLights,
        LuxPassRequest::SelectReservoirs(_) => FrameGraphPassRole::LuxReservoirTemporalReuse,
        LuxPassRequest::BuildShadowRequests(_) => FrameGraphPassRole::LuxShadowRequests,
        LuxPassRequest::RenderVirtualShadowPages(_) => FrameGraphPassRole::LuxVirtualShadowPages,
        LuxPassRequest::FilterVirtualShadows(_) => FrameGraphPassRole::LuxVirtualShadowFilter,
        LuxPassRequest::VoxelShadowDemandMark(_) => FrameGraphPassRole::LuxVoxelShadowDemandMark,
        LuxPassRequest::VoxelShadowPageBuild(_) => FrameGraphPassRole::LuxVoxelShadowPageBuild,
        LuxPassRequest::VoxelSdfDistantShadowResolve(_) => {
            FrameGraphPassRole::LuxVoxelSdfDistantShadowResolve
        }
        LuxPassRequest::VoxelRadianceClipmapUpdate(_) => {
            FrameGraphPassRole::LuxVoxelRadianceClipmapUpdate
        }
        LuxPassRequest::VoxelCanopyTransmittanceInject(_) => {
            FrameGraphPassRole::LuxVoxelCanopyTransmittanceInject
        }
        LuxPassRequest::VoxelTerrainAoResolve(_) => FrameGraphPassRole::LuxVoxelTerrainAoResolve,
        LuxPassRequest::StormExtinctionInject(_) => FrameGraphPassRole::LuxStormExtinctionInject,
        LuxPassRequest::DirectLighting(_) => FrameGraphPassRole::LuxDirectLighting,
        LuxPassRequest::GiTrace(_) => FrameGraphPassRole::LuxGiTrace,
        LuxPassRequest::GiCacheUpdate(_) => FrameGraphPassRole::LuxGiCacheUpdate,
        LuxPassRequest::ReflectionTrace(_) => FrameGraphPassRole::LuxReflectionTrace,
        LuxPassRequest::Denoise(_) => FrameGraphPassRole::LuxDenoise,
        LuxPassRequest::VolumetricFogInject(_) => FrameGraphPassRole::LuxVolumetricFogInject,
        LuxPassRequest::VolumetricLightInject(_) => FrameGraphPassRole::LuxVolumetricLightInject,
        LuxPassRequest::VolumetricTemporalReproject(_) => {
            FrameGraphPassRole::LuxVolumetricTemporalReproject
        }
        LuxPassRequest::VolumetricIntegrate(_) => FrameGraphPassRole::LuxVolumetricIntegrate,
        LuxPassRequest::VolumetricComposite(_) => FrameGraphPassRole::LuxVolumetricComposite,
        LuxPassRequest::LuxDebugOverlay(_) => FrameGraphPassRole::LuxDebugOverlay,
    }
}

/// Pass 3 typed read tables. Each typed Lux pass role names
/// the typed `FrameGraphResourceType`s it reads. The
/// compiler binds these to typed
/// `FrameGraphResourceHandle`s declared in the frame graph.
///
/// `LuxUploadLightBuffers` is the only Lux role that
/// declares no reads (it's an upload pass — input is CPU
/// data via `queue.write_buffer`). Every other typed Lux
/// role declares at least one read OR write.
#[must_use]
pub const fn typed_resource_inputs(role: FrameGraphPassRole) -> &'static [FrameGraphResourceType] {
    match role {
        FrameGraphPassRole::LuxUploadLightBuffers => &[],
        FrameGraphPassRole::LuxClusterLights => &[FrameGraphResourceType::LuxLightBuffer],
        FrameGraphPassRole::LuxReservoirTemporalReuse => &[
            FrameGraphResourceType::LuxLightBuffer,
            FrameGraphResourceType::LuxLightIndexBuffer,
            FrameGraphResourceType::LuxClusterGrid,
        ],
        FrameGraphPassRole::LuxReservoirSpatialReuse => {
            &[FrameGraphResourceType::LuxReservoirBuffer]
        }
        FrameGraphPassRole::LuxShadowRequests => &[FrameGraphResourceType::LuxLightBuffer],
        FrameGraphPassRole::LuxVirtualShadowPages => {
            &[FrameGraphResourceType::LuxShadowRequestBuffer]
        }
        FrameGraphPassRole::LuxVirtualShadowFilter => {
            &[FrameGraphResourceType::LuxVirtualShadowPages]
        }
        FrameGraphPassRole::LuxVoxelShadowDemandMark => &[],
        FrameGraphPassRole::LuxVoxelShadowPageBuild => {
            &[FrameGraphResourceType::LuxVoxelShadowPageTable]
        }
        FrameGraphPassRole::LuxVoxelSdfDistantShadowResolve => {
            &[FrameGraphResourceType::LuxVoxelTerrainSdfPool]
        }
        FrameGraphPassRole::LuxVoxelRadianceClipmapUpdate => {
            &[FrameGraphResourceType::LuxVoxelTerrainRadianceClipmap]
        }
        FrameGraphPassRole::LuxVoxelCanopyTransmittanceInject => {
            &[FrameGraphResourceType::LuxVoxelCanopyOpacityClipmap]
        }
        FrameGraphPassRole::LuxVoxelTerrainAoResolve => {
            &[FrameGraphResourceType::LuxVoxelTerrainSdfPool]
        }
        FrameGraphPassRole::LuxStormExtinctionInject => {
            &[FrameGraphResourceType::LuxStormExtinctionClipmap]
        }
        FrameGraphPassRole::LuxDirectLighting => &[
            FrameGraphResourceType::LuxLightBuffer,
            FrameGraphResourceType::LuxLightIndexBuffer,
            FrameGraphResourceType::LuxClusterGrid,
            FrameGraphResourceType::LuxShadowAtlas,
        ],
        FrameGraphPassRole::LuxGiTrace => &[
            FrameGraphResourceType::LuxLightBuffer,
            FrameGraphResourceType::LuxRadianceCache,
        ],
        FrameGraphPassRole::LuxGiCacheUpdate => &[FrameGraphResourceType::LuxRadianceCache],
        FrameGraphPassRole::LuxReflectionTrace => &[
            FrameGraphResourceType::LuxLightBuffer,
            FrameGraphResourceType::LuxSurfaceCache,
        ],
        FrameGraphPassRole::LuxDenoise => &[FrameGraphResourceType::LuxDenoiseHistory],
        FrameGraphPassRole::LuxVolumetricFogInject => &[],
        FrameGraphPassRole::LuxVolumetricLightInject => &[
            FrameGraphResourceType::LuxLightBuffer,
            FrameGraphResourceType::LuxVolumetricFroxelDensity,
        ],
        FrameGraphPassRole::LuxVolumetricTemporalReproject => &[
            FrameGraphResourceType::LuxVolumetricFroxelScattering,
            FrameGraphResourceType::LuxVolumetricHistory,
        ],
        FrameGraphPassRole::LuxVolumetricIntegrate => {
            &[FrameGraphResourceType::LuxVolumetricFroxelScattering]
        }
        FrameGraphPassRole::LuxVolumetricComposite => {
            &[FrameGraphResourceType::LuxVolumetricIntegratedFog]
        }
        FrameGraphPassRole::LuxDebugOverlay => &[],
        _ => &[],
    }
}

/// Pass 3 typed write tables. Each typed Lux pass role
/// names the typed `FrameGraphResourceType`s it writes.
#[must_use]
pub const fn typed_resource_outputs(role: FrameGraphPassRole) -> &'static [FrameGraphResourceType] {
    match role {
        FrameGraphPassRole::LuxUploadLightBuffers => &[FrameGraphResourceType::LuxLightBuffer],
        FrameGraphPassRole::LuxClusterLights => &[
            FrameGraphResourceType::LuxClusterGrid,
            FrameGraphResourceType::LuxLightIndexBuffer,
        ],
        FrameGraphPassRole::LuxReservoirTemporalReuse => {
            &[FrameGraphResourceType::LuxReservoirBuffer]
        }
        FrameGraphPassRole::LuxReservoirSpatialReuse => {
            &[FrameGraphResourceType::LuxReservoirBuffer]
        }
        FrameGraphPassRole::LuxShadowRequests => &[FrameGraphResourceType::LuxShadowRequestBuffer],
        FrameGraphPassRole::LuxVirtualShadowPages => &[
            FrameGraphResourceType::LuxVirtualShadowPages,
            FrameGraphResourceType::LuxShadowAtlas,
        ],
        FrameGraphPassRole::LuxVirtualShadowFilter => {
            &[FrameGraphResourceType::LuxVirtualShadowPages]
        }
        FrameGraphPassRole::LuxVoxelShadowDemandMark => {
            &[FrameGraphResourceType::LuxVoxelShadowPageTable]
        }
        FrameGraphPassRole::LuxVoxelShadowPageBuild => &[
            FrameGraphResourceType::LuxVirtualShadowPages,
            FrameGraphResourceType::LuxShadowAtlas,
        ],
        FrameGraphPassRole::LuxVoxelSdfDistantShadowResolve => {
            &[FrameGraphResourceType::LuxVirtualShadowPages]
        }
        FrameGraphPassRole::LuxVoxelRadianceClipmapUpdate => {
            &[FrameGraphResourceType::LuxVoxelTerrainRadianceClipmap]
        }
        FrameGraphPassRole::LuxVoxelCanopyTransmittanceInject => {
            &[FrameGraphResourceType::LuxVolumetricFroxelScattering]
        }
        FrameGraphPassRole::LuxVoxelTerrainAoResolve => {
            &[FrameGraphResourceType::RenderResolutionSceneColor]
        }
        FrameGraphPassRole::LuxStormExtinctionInject => {
            &[FrameGraphResourceType::LuxVolumetricFroxelDensity]
        }
        FrameGraphPassRole::LuxDirectLighting => {
            &[FrameGraphResourceType::RenderResolutionSceneColor]
        }
        FrameGraphPassRole::LuxGiTrace => &[FrameGraphResourceType::RenderResolutionSceneColor],
        FrameGraphPassRole::LuxGiCacheUpdate => &[FrameGraphResourceType::LuxRadianceCache],
        FrameGraphPassRole::LuxReflectionTrace => &[FrameGraphResourceType::LuxReflectionBuffer],
        FrameGraphPassRole::LuxDenoise => &[
            FrameGraphResourceType::RenderResolutionSceneColor,
            FrameGraphResourceType::LuxDenoiseHistory,
        ],
        FrameGraphPassRole::LuxVolumetricFogInject => {
            &[FrameGraphResourceType::LuxVolumetricFroxelDensity]
        }
        FrameGraphPassRole::LuxVolumetricLightInject => {
            &[FrameGraphResourceType::LuxVolumetricFroxelScattering]
        }
        FrameGraphPassRole::LuxVolumetricTemporalReproject => &[
            FrameGraphResourceType::LuxVolumetricFroxelScattering,
            FrameGraphResourceType::LuxVolumetricHistory,
        ],
        FrameGraphPassRole::LuxVolumetricIntegrate => {
            &[FrameGraphResourceType::LuxVolumetricIntegratedFog]
        }
        FrameGraphPassRole::LuxVolumetricComposite => {
            &[FrameGraphResourceType::RenderResolutionSceneColor]
        }
        FrameGraphPassRole::LuxDebugOverlay => &[FrameGraphResourceType::TransientScratch],
        _ => &[],
    }
}

/// Typed predicate: does this typed Lux role declare at
/// least one read OR write? Used by the compiler's
/// validation step.
#[must_use]
pub const fn declares_any_resource_interaction(role: FrameGraphPassRole) -> bool {
    !typed_resource_inputs(role).is_empty() || !typed_resource_outputs(role).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fun_lux::{
        LuxPassCommon, LuxPassKind, LuxPassRequest, LuxQualityTier, UploadLightBuffersPass,
    };

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_RENDERER_LUX_PASSES_SCHEMA_VERSION, 1);
    }

    #[test]
    fn lux_role_for_walks_every_lux_pass_kind() {
        // Build one of every variant and confirm the role
        // returned is a Lux role.
        let common = LuxPassCommon::new("test", LuxQualityTier::Medium);
        let requests: Vec<LuxPassRequest> = vec![
            LuxPassRequest::UploadLightBuffers(UploadLightBuffersPass {
                common: common.clone(),
                light_count: 0,
                bytes_per_light: 0,
            }),
            LuxPassRequest::ClusterLights(fun_lux::ClusterLightsPass {
                common: common.clone(),
                clusters_x: 1,
                clusters_y: 1,
                clusters_z: 1,
                max_lights_per_cluster: 1,
            }),
            LuxPassRequest::SelectReservoirs(fun_lux::SelectReservoirsPass {
                common: common.clone(),
                reservoir_samples_per_pixel: 1,
                temporal_reuse_enabled: true,
                spatial_reuse_enabled: false,
            }),
            LuxPassRequest::BuildShadowRequests(fun_lux::BuildShadowRequestsPass {
                common: common.clone(),
                max_shadow_request_count: 1,
            }),
            LuxPassRequest::RenderVirtualShadowPages(fun_lux::RenderVirtualShadowPagesPass {
                common: common.clone(),
                max_pages_per_frame: 1,
            }),
            LuxPassRequest::FilterVirtualShadows(fun_lux::FilterVirtualShadowsPass {
                common: common.clone(),
                spatial_filter_kernel_radius: 1,
            }),
            LuxPassRequest::VoxelShadowDemandMark(fun_lux::VoxelShadowDemandMarkPass {
                common: common.clone(),
                dirty_row_count: 1,
            }),
            LuxPassRequest::VoxelShadowPageBuild(fun_lux::VoxelShadowPageBuildPass {
                common: common.clone(),
                max_pages_per_frame: 1,
            }),
            LuxPassRequest::VoxelSdfDistantShadowResolve(
                fun_lux::VoxelSdfDistantShadowResolvePass {
                    common: common.clone(),
                    sdf_page_count: 1,
                },
            ),
            LuxPassRequest::VoxelRadianceClipmapUpdate(fun_lux::VoxelRadianceClipmapUpdatePass {
                common: common.clone(),
                dirty_row_count: 1,
            }),
            LuxPassRequest::VoxelCanopyTransmittanceInject(
                fun_lux::VoxelCanopyTransmittanceInjectPass {
                    common: common.clone(),
                    opacity_page_count: 1,
                },
            ),
            LuxPassRequest::VoxelTerrainAoResolve(fun_lux::VoxelTerrainAoResolvePass {
                common: common.clone(),
                sdf_page_count: 1,
            }),
            LuxPassRequest::StormExtinctionInject(fun_lux::StormExtinctionInjectPass {
                common: common.clone(),
                dirty_row_count: 1,
            }),
            LuxPassRequest::DirectLighting(fun_lux::DirectLightingPass {
                common: common.clone(),
                uses_reservoirs: false,
                samples_virtual_shadow: false,
            }),
            LuxPassRequest::GiTrace(fun_lux::GiTracePass {
                common: common.clone(),
                max_bounces: 1,
                uses_radiance_cache: false,
            }),
            LuxPassRequest::GiCacheUpdate(fun_lux::GiCacheUpdatePass {
                common: common.clone(),
                voxel_resolution: 1,
            }),
            LuxPassRequest::ReflectionTrace(fun_lux::ReflectionTracePass {
                common: common.clone(),
                ray_budget_per_pixel: 1,
                uses_surface_cache: false,
            }),
            LuxPassRequest::Denoise(fun_lux::DenoisePass {
                common: common.clone(),
                temporal_history_frames: 1,
                spatial_kernel_radius: 1,
            }),
            LuxPassRequest::VolumetricFogInject(fun_lux::VolumetricFogInjectPass {
                common: common.clone(),
                froxel_width: 1,
                froxel_height: 1,
                froxel_depth: 1,
            }),
            LuxPassRequest::VolumetricLightInject(fun_lux::VolumetricLightInjectPass {
                common: common.clone(),
                light_count: 1,
            }),
            LuxPassRequest::VolumetricTemporalReproject(fun_lux::VolumetricTemporalReprojectPass {
                common: common.clone(),
                reproject_alpha_q8: 0,
            }),
            LuxPassRequest::VolumetricIntegrate(fun_lux::VolumetricIntegratePass {
                common: common.clone(),
                multi_tap: false,
            }),
            LuxPassRequest::VolumetricComposite(fun_lux::VolumetricCompositePass {
                common: common.clone(),
            }),
            LuxPassRequest::LuxDebugOverlay(fun_lux::LuxDebugOverlayPass {
                common,
                overlay_kind: fun_lux::LuxDebugOverlayKind::LightsPerCluster,
            }),
        ];
        assert_eq!(requests.len(), LuxPassKind::ALL.len());
        for r in &requests {
            let role = lux_role_for(r);
            assert!(role.is_lux(), "role {role:?} must be lux");
            assert!(role.lux_order_key().is_some());
        }
    }

    #[test]
    fn every_lux_role_has_typed_inputs_or_outputs() {
        // Every typed Lux role must declare at least one
        // typed read OR write (the typed contract refuses
        // empty passes).
        for role in [
            FrameGraphPassRole::LuxUploadLightBuffers,
            FrameGraphPassRole::LuxClusterLights,
            FrameGraphPassRole::LuxReservoirTemporalReuse,
            FrameGraphPassRole::LuxReservoirSpatialReuse,
            FrameGraphPassRole::LuxShadowRequests,
            FrameGraphPassRole::LuxVirtualShadowPages,
            FrameGraphPassRole::LuxVirtualShadowFilter,
            FrameGraphPassRole::LuxDirectLighting,
            FrameGraphPassRole::LuxGiTrace,
            FrameGraphPassRole::LuxGiCacheUpdate,
            FrameGraphPassRole::LuxReflectionTrace,
            FrameGraphPassRole::LuxDenoise,
            FrameGraphPassRole::LuxVolumetricFogInject,
            FrameGraphPassRole::LuxVolumetricLightInject,
            FrameGraphPassRole::LuxVolumetricTemporalReproject,
            FrameGraphPassRole::LuxVolumetricIntegrate,
            FrameGraphPassRole::LuxVolumetricComposite,
            FrameGraphPassRole::LuxDebugOverlay,
        ] {
            assert!(
                declares_any_resource_interaction(role),
                "lux role {role:?} must declare at least one read or write",
            );
        }
    }

    #[test]
    fn typed_ordering_satisfies_user_pass_order() {
        // The user spec: upload → cluster → shadow requests →
        // virtual shadow pages → direct lighting → GI/refl →
        // volumetric → debug overlay.
        let upload = FrameGraphPassRole::LuxUploadLightBuffers
            .lux_order_key()
            .unwrap();
        let cluster = FrameGraphPassRole::LuxClusterLights
            .lux_order_key()
            .unwrap();
        let shadow = FrameGraphPassRole::LuxShadowRequests
            .lux_order_key()
            .unwrap();
        let vpages = FrameGraphPassRole::LuxVirtualShadowPages
            .lux_order_key()
            .unwrap();
        let direct = FrameGraphPassRole::LuxDirectLighting
            .lux_order_key()
            .unwrap();
        let gi = FrameGraphPassRole::LuxGiTrace.lux_order_key().unwrap();
        let refl = FrameGraphPassRole::LuxReflectionTrace
            .lux_order_key()
            .unwrap();
        let fog = FrameGraphPassRole::LuxVolumetricFogInject
            .lux_order_key()
            .unwrap();
        let composite = FrameGraphPassRole::LuxVolumetricComposite
            .lux_order_key()
            .unwrap();
        let debug = FrameGraphPassRole::LuxDebugOverlay.lux_order_key().unwrap();
        assert!(upload < cluster);
        assert!(cluster < shadow);
        assert!(shadow < vpages);
        assert!(vpages < direct);
        assert!(direct < gi);
        assert!(direct < refl);
        assert!(refl < fog);
        assert!(fog < composite);
        assert!(composite < debug);
    }
}
