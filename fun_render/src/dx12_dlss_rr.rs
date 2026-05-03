use bevy::prelude::*;
use tracing::{info, warn};

use crate::{ClientRenderConfig, Dx12NativeDlssSrSupport};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum Dx12NativeDlssRrGateRejection {
    RuntimeDisabled,
    SuperResolutionUnavailable,
    RayReconstructionUnsupported,
    SolariGuideResourcesInvalid,
    ScenePathUnsupported,
}

impl Dx12NativeDlssRrGateRejection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RuntimeDisabled => "runtime_disabled",
            Self::SuperResolutionUnavailable => "super_resolution_unavailable",
            Self::RayReconstructionUnsupported => "ray_reconstruction_unsupported",
            Self::SolariGuideResourcesInvalid => "solari_guide_resources_invalid",
            Self::ScenePathUnsupported => "scene_path_unsupported",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource, Reflect)]
#[reflect(Resource, Default, Clone)]
pub struct Dx12NativeDlssRrSupport {
    pub solari_guide_resources_valid: bool,
    pub current_scene_path_supports_guides: bool,
}

impl Default for Dx12NativeDlssRrSupport {
    fn default() -> Self {
        Self::unsupported()
    }
}

impl Dx12NativeDlssRrSupport {
    pub const fn unsupported() -> Self {
        Self {
            solari_guide_resources_valid: false,
            current_scene_path_supports_guides: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource, Reflect)]
#[reflect(Resource, Default, Clone)]
pub struct Dx12NativeDlssRrStatus {
    pub enabled: bool,
    pub rejection: Option<Dx12NativeDlssRrGateRejection>,
}

impl Default for Dx12NativeDlssRrStatus {
    fn default() -> Self {
        Self {
            enabled: false,
            rejection: Some(Dx12NativeDlssRrGateRejection::RuntimeDisabled),
        }
    }
}

impl Dx12NativeDlssRrStatus {
    pub fn evaluate(
        config: ClientRenderConfig,
        sr_support: Dx12NativeDlssSrSupport,
        rr_support: Dx12NativeDlssRrSupport,
    ) -> Self {
        if !config.native_dlss.allow_ray_reconstruction {
            return Self {
                enabled: false,
                rejection: Some(Dx12NativeDlssRrGateRejection::RuntimeDisabled),
            };
        }
        if !sr_support.super_resolution_ready(config.native_dlss) {
            return Self {
                enabled: false,
                rejection: Some(Dx12NativeDlssRrGateRejection::SuperResolutionUnavailable),
            };
        }
        if !sr_support.ray_reconstruction_supported {
            return Self {
                enabled: false,
                rejection: Some(Dx12NativeDlssRrGateRejection::RayReconstructionUnsupported),
            };
        }
        if !rr_support.solari_guide_resources_valid {
            return Self {
                enabled: false,
                rejection: Some(Dx12NativeDlssRrGateRejection::SolariGuideResourcesInvalid),
            };
        }
        if !rr_support.current_scene_path_supports_guides {
            return Self {
                enabled: false,
                rejection: Some(Dx12NativeDlssRrGateRejection::ScenePathUnsupported),
            };
        }
        Self {
            enabled: true,
            rejection: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dx12NativeDlssRrGuideSurfaceStatus {
    Present,
    Missing,
    PendingNativeBridge,
}

impl Dx12NativeDlssRrGuideSurfaceStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Missing => "missing",
            Self::PendingNativeBridge => "pending_native_bridge",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dx12NativeDlssRrGuideSurfaceSpec {
    pub id: &'static str,
    pub required: bool,
    pub status: Dx12NativeDlssRrGuideSurfaceStatus,
    pub format: &'static str,
    pub resolution: &'static str,
    pub coordinate_convention: &'static str,
    pub lifetime: &'static str,
    pub resource_state: &'static str,
    pub camera_association: &'static str,
    pub history_reset: &'static str,
}

pub const SOLARI_RR_GUIDE_SURFACE_AUDIT: &[Dx12NativeDlssRrGuideSurfaceSpec] = &[
    Dx12NativeDlssRrGuideSurfaceSpec {
        id: "input_color",
        required: true,
        status: Dx12NativeDlssRrGuideSurfaceStatus::PendingNativeBridge,
        format: "view_target_main_texture_format",
        resolution: "native_dlss_input_resolution",
        coordinate_convention: "jittered_hdr_scene_color_before_bloom_and_ui",
        lifetime: "view_target_post_process_source_for_current_view",
        resource_state: "shader_resource",
        camera_association: "same_view_entity_as_dlss_rr_camera",
        history_reset: "dlss_history_reset",
    },
    Dx12NativeDlssRrGuideSurfaceSpec {
        id: "depth",
        required: true,
        status: Dx12NativeDlssRrGuideSurfaceStatus::Present,
        format: "Depth32Float",
        resolution: "native_dlss_input_resolution",
        coordinate_convention: "bevy_reversed_z_infinite_far_non_linear",
        lifetime: "view_prepass_depth_for_current_view",
        resource_state: "depth_read_or_shader_resource",
        camera_association: "same_view_entity_as_dlss_rr_camera",
        history_reset: "dlss_history_reset",
    },
    Dx12NativeDlssRrGuideSurfaceSpec {
        id: "motion_vectors",
        required: true,
        status: Dx12NativeDlssRrGuideSurfaceStatus::Present,
        format: "Rg16Float",
        resolution: "native_dlss_input_resolution",
        coordinate_convention: "current_minus_previous_normalized_uv_jitter_excluded",
        lifetime: "view_prepass_motion_vectors_for_current_view",
        resource_state: "shader_resource",
        camera_association: "same_view_entity_as_dlss_rr_camera",
        history_reset: "dlss_history_reset",
    },
    Dx12NativeDlssRrGuideSurfaceSpec {
        id: "diffuse_albedo",
        required: true,
        status: Dx12NativeDlssRrGuideSurfaceStatus::Present,
        format: "Rgba8Unorm",
        resolution: "native_dlss_input_resolution",
        coordinate_convention: "solari_resolved_diffuse_albedo_linear_rgb",
        lifetime: "ViewDlssRayReconstructionTextures",
        resource_state: "storage_write_then_shader_resource",
        camera_association: "same_view_entity_as_solari_lighting",
        history_reset: "solari_reset_and_dlss_history_reset",
    },
    Dx12NativeDlssRrGuideSurfaceSpec {
        id: "specular_albedo",
        required: true,
        status: Dx12NativeDlssRrGuideSurfaceStatus::Present,
        format: "Rgba8Unorm",
        resolution: "native_dlss_input_resolution",
        coordinate_convention: "solari_env_brdf_specular_albedo_linear_rgb",
        lifetime: "ViewDlssRayReconstructionTextures",
        resource_state: "storage_write_then_shader_resource",
        camera_association: "same_view_entity_as_solari_lighting",
        history_reset: "solari_reset_and_dlss_history_reset",
    },
    Dx12NativeDlssRrGuideSurfaceSpec {
        id: "normal_roughness",
        required: true,
        status: Dx12NativeDlssRrGuideSurfaceStatus::Present,
        format: "Rgba16Float",
        resolution: "native_dlss_input_resolution",
        coordinate_convention: "world_normal_xyz_roughness_w",
        lifetime: "ViewDlssRayReconstructionTextures",
        resource_state: "storage_write_then_shader_resource",
        camera_association: "same_view_entity_as_solari_lighting",
        history_reset: "solari_reset_and_dlss_history_reset",
    },
    Dx12NativeDlssRrGuideSurfaceSpec {
        id: "specular_motion_vectors",
        required: true,
        status: Dx12NativeDlssRrGuideSurfaceStatus::Present,
        format: "Rg16Float",
        resolution: "native_dlss_input_resolution",
        coordinate_convention: "specular_virtual_position_motion_normalized_uv",
        lifetime: "ViewDlssRayReconstructionTextures",
        resource_state: "storage_write_then_shader_resource",
        camera_association: "same_view_entity_as_solari_lighting",
        history_reset: "solari_reset_and_dlss_history_reset",
    },
    Dx12NativeDlssRrGuideSurfaceSpec {
        id: "bias",
        required: true,
        status: Dx12NativeDlssRrGuideSurfaceStatus::Present,
        format: "Rgba8Unorm",
        resolution: "native_dlss_input_resolution",
        coordinate_convention: "current_color_bias_scalar_replicated",
        lifetime: "ViewDlssRayReconstructionTextures",
        resource_state: "storage_write_then_shader_resource",
        camera_association: "same_view_entity_as_solari_lighting",
        history_reset: "solari_reset_and_dlss_history_reset",
    },
    Dx12NativeDlssRrGuideSurfaceSpec {
        id: "exposure",
        required: false,
        status: Dx12NativeDlssRrGuideSurfaceStatus::PendingNativeBridge,
        format: "sdk_optional",
        resolution: "1x1_or_view_dependent",
        coordinate_convention: "bevy_view_exposure",
        lifetime: "view_uniform_or_future_exposure_texture",
        resource_state: "shader_resource",
        camera_association: "same_view_entity_as_dlss_rr_camera",
        history_reset: "not_history_bearing",
    },
    Dx12NativeDlssRrGuideSurfaceSpec {
        id: "ray_distance",
        required: false,
        status: Dx12NativeDlssRrGuideSurfaceStatus::Missing,
        format: "not_allocated",
        resolution: "native_dlss_input_resolution",
        coordinate_convention: "world_space_ray_t",
        lifetime: "future_solari_ray_distance_guide",
        resource_state: "shader_resource",
        camera_association: "same_view_entity_as_solari_lighting",
        history_reset: "solari_reset_and_dlss_history_reset",
    },
    Dx12NativeDlssRrGuideSurfaceSpec {
        id: "reactive_mask",
        required: false,
        status: Dx12NativeDlssRrGuideSurfaceStatus::Missing,
        format: "not_allocated",
        resolution: "native_dlss_input_resolution",
        coordinate_convention: "sdk_reactive_mask",
        lifetime: "future_transparency_or_particle_guide",
        resource_state: "shader_resource",
        camera_association: "same_view_entity_as_dlss_rr_camera",
        history_reset: "dlss_history_reset",
    },
];

pub const fn solari_rr_guide_surface_audit() -> &'static [Dx12NativeDlssRrGuideSurfaceSpec] {
    SOLARI_RR_GUIDE_SURFACE_AUDIT
}

pub fn install_dx12_native_dlss_rr(app: &mut App) {
    app.register_type::<Dx12NativeDlssRrSupport>()
        .register_type::<Dx12NativeDlssRrStatus>()
        .register_type::<Dx12NativeDlssRrGateRejection>()
        .init_resource::<Dx12NativeDlssRrSupport>()
        .init_resource::<Dx12NativeDlssRrStatus>()
        .add_systems(Update, update_dx12_native_dlss_rr_status);
}

fn update_dx12_native_dlss_rr_status(
    config: Res<ClientRenderConfig>,
    sr_support: Res<Dx12NativeDlssSrSupport>,
    rr_support: Res<Dx12NativeDlssRrSupport>,
    mut status: ResMut<Dx12NativeDlssRrStatus>,
) {
    let next = Dx12NativeDlssRrStatus::evaluate(*config, *sr_support, *rr_support);
    if *status == next {
        return;
    }

    if next.enabled {
        info!(
            target: "fun::render::dlss",
            "FUN DX12 DLSS Ray Reconstruction gate enabled"
        );
    } else if config.native_dlss.allow_ray_reconstruction {
        warn!(
            target: "fun::render::dlss",
            rejection = next
                .rejection
                .map(Dx12NativeDlssRrGateRejection::as_str)
                .unwrap_or("none"),
            "FUN DX12 DLSS Ray Reconstruction gate rejected the request"
        );
    }
    *status = next;
}

#[cfg(test)]
mod tests {
    use super::{
        Dx12NativeDlssRrGateRejection, Dx12NativeDlssRrGuideSurfaceStatus, Dx12NativeDlssRrStatus,
        Dx12NativeDlssRrSupport, solari_rr_guide_surface_audit,
    };
    use crate::{ClientRenderConfig, Dx12NativeDlssSrSupport, NativeDlssConfig, NativeDlssMode};

    fn rr_config(allow_ray_reconstruction: bool) -> ClientRenderConfig {
        ClientRenderConfig {
            native_dlss: NativeDlssConfig {
                enabled: true,
                mode: NativeDlssMode::Quality,
                sharpness: NativeDlssConfig::DEFAULT_SHARPNESS,
                allow_ray_reconstruction,
                force_reset_next_frame: false,
                debug_overlay: false,
            },
            dlss_rr_enabled: allow_ray_reconstruction,
            ..ClientRenderConfig::from_env()
        }
    }

    fn sr_support(rr_supported: bool) -> Dx12NativeDlssSrSupport {
        Dx12NativeDlssSrSupport {
            runtime_found: true,
            native_handle_extraction_available: true,
            super_resolution_supported: true,
            ray_reconstruction_supported: rr_supported,
            driver_needs_update: false,
        }
    }

    #[test]
    fn rr_gate_defaults_closed_until_explicit_runtime_gate() {
        let status = Dx12NativeDlssRrStatus::evaluate(
            rr_config(false),
            sr_support(true),
            Dx12NativeDlssRrSupport {
                solari_guide_resources_valid: true,
                current_scene_path_supports_guides: true,
            },
        );

        assert!(!status.enabled);
        assert_eq!(
            status.rejection,
            Some(Dx12NativeDlssRrGateRejection::RuntimeDisabled)
        );
    }

    #[test]
    fn rr_gate_requires_sr_rr_guides_and_scene_path() {
        let config = rr_config(true);
        assert_eq!(
            Dx12NativeDlssRrStatus::evaluate(
                config,
                Dx12NativeDlssSrSupport::unsupported(),
                Dx12NativeDlssRrSupport {
                    solari_guide_resources_valid: true,
                    current_scene_path_supports_guides: true,
                },
            )
            .rejection,
            Some(Dx12NativeDlssRrGateRejection::SuperResolutionUnavailable)
        );
        assert_eq!(
            Dx12NativeDlssRrStatus::evaluate(
                config,
                sr_support(false),
                Dx12NativeDlssRrSupport {
                    solari_guide_resources_valid: true,
                    current_scene_path_supports_guides: true,
                },
            )
            .rejection,
            Some(Dx12NativeDlssRrGateRejection::RayReconstructionUnsupported)
        );
        assert_eq!(
            Dx12NativeDlssRrStatus::evaluate(
                config,
                sr_support(true),
                Dx12NativeDlssRrSupport {
                    solari_guide_resources_valid: false,
                    current_scene_path_supports_guides: true,
                },
            )
            .rejection,
            Some(Dx12NativeDlssRrGateRejection::SolariGuideResourcesInvalid)
        );
        assert!(
            Dx12NativeDlssRrStatus::evaluate(
                config,
                sr_support(true),
                Dx12NativeDlssRrSupport {
                    solari_guide_resources_valid: true,
                    current_scene_path_supports_guides: true,
                },
            )
            .enabled
        );
    }

    #[test]
    fn solari_rr_guide_audit_records_missing_optional_surfaces() {
        let audit = solari_rr_guide_surface_audit();
        assert!(audit.iter().any(|surface| surface.id == "normal_roughness"
            && surface.format == "Rgba16Float"
            && surface.required));
        assert!(audit.iter().any(|surface| surface.id == "ray_distance"
            && surface.status == Dx12NativeDlssRrGuideSurfaceStatus::Missing));
        assert!(audit.iter().all(|surface| !surface.required
            || surface.status != Dx12NativeDlssRrGuideSurfaceStatus::Missing));
    }
}
