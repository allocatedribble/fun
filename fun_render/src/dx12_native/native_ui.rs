use crate::{
    FUN_RENDER_DEBUG_OVERLAY_STAGE, FUN_RENDER_HUD_UI_STAGE, FUN_RENDER_NATIVE_UI_STAGE,
    FUN_RENDER_NATIVE_UI_Z_INDEX, FunRenderCompositionStage,
};

pub const DX12_NATIVE_UI_TRANSPORT_SCHEMA_VERSION: u16 = 1;
pub const DX12_NATIVE_UI_SHARED_TEXTURE_RING_DEPTH_DEFAULT: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dx12NativeUiTransportPath {
    D3d11On12SharedTextureRing,
    SharedTextureRing,
    CpuDirtyRectFallback,
    Disabled,
}

impl Dx12NativeUiTransportPath {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::D3d11On12SharedTextureRing => "d3d11on12_shared_texture_ring",
            Self::SharedTextureRing => "shared_texture_ring",
            Self::CpuDirtyRectFallback => "cpu_dirty_rect_fallback",
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dx12NativeUiTransportPolicy {
    pub schema_version: u16,
    pub preferred_path: Dx12NativeUiTransportPath,
    pub shared_texture_ring_depth: u8,
    pub normal_frame_blocking_wait_allowed: bool,
    pub copy_through_dx12_native_boundary: bool,
    pub cpu_fallback_dirty_rect_only: bool,
    pub cpu_fallback_throttle_when_unchanged: bool,
    pub cpu_fallback_throttle_when_world_over_budget: bool,
    pub native_ui_uploads_count_as_world_uploads: bool,
    pub composition_stage: FunRenderCompositionStage,
    pub composition_z_index: i32,
}

impl Dx12NativeUiTransportPolicy {
    pub const DEFAULT: Self = Self {
        schema_version: DX12_NATIVE_UI_TRANSPORT_SCHEMA_VERSION,
        preferred_path: Dx12NativeUiTransportPath::D3d11On12SharedTextureRing,
        shared_texture_ring_depth: DX12_NATIVE_UI_SHARED_TEXTURE_RING_DEPTH_DEFAULT,
        normal_frame_blocking_wait_allowed: false,
        copy_through_dx12_native_boundary: true,
        cpu_fallback_dirty_rect_only: true,
        cpu_fallback_throttle_when_unchanged: true,
        cpu_fallback_throttle_when_world_over_budget: true,
        native_ui_uploads_count_as_world_uploads: false,
        composition_stage: FUN_RENDER_NATIVE_UI_STAGE,
        composition_z_index: FUN_RENDER_NATIVE_UI_Z_INDEX,
    };

    #[must_use]
    pub const fn preferred_path_label(self) -> &'static str {
        self.preferred_path.as_str()
    }

    #[must_use]
    pub const fn keeps_native_ui_after_world_postprocess(self) -> bool {
        self.composition_stage.order_key() > FunRenderCompositionStage::PostProcessing.order_key()
            && self.composition_stage.order_key() <= FUN_RENDER_HUD_UI_STAGE.order_key()
            && self.composition_stage.order_key() < FUN_RENDER_DEBUG_OVERLAY_STAGE.order_key()
            && self.composition_z_index >= FUN_RENDER_NATIVE_UI_Z_INDEX
    }

    #[must_use]
    pub const fn keeps_native_ui_out_of_temporal_reconstruction(self) -> bool {
        !self.composition_stage.feeds_dlss_input_color()
            && !self.composition_stage.feeds_dlss_depth_or_motion_vectors()
            && !self.composition_stage.feeds_dlss_rr_guide_buffers()
            && !self.composition_stage.feeds_temporal_reconstruction()
    }

    #[must_use]
    pub const fn keeps_native_ui_out_of_world_upload_accounting(self) -> bool {
        !self.native_ui_uploads_count_as_world_uploads
    }

    #[must_use]
    pub const fn is_nonblocking_gpu_transport(self) -> bool {
        matches!(
            self.preferred_path,
            Dx12NativeUiTransportPath::D3d11On12SharedTextureRing
                | Dx12NativeUiTransportPath::SharedTextureRing
        ) && !self.normal_frame_blocking_wait_allowed
    }
}

impl Default for Dx12NativeUiTransportPolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[must_use]
pub const fn dx12_native_ui_transport_policy() -> Dx12NativeUiTransportPolicy {
    Dx12NativeUiTransportPolicy::DEFAULT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dx12_native_ui_prefers_shared_texture_ring_without_blocking_wait() {
        let policy = dx12_native_ui_transport_policy();

        assert_eq!(
            policy.preferred_path,
            Dx12NativeUiTransportPath::D3d11On12SharedTextureRing
        );
        assert!(policy.shared_texture_ring_depth >= 3);
        assert!(policy.copy_through_dx12_native_boundary);
        assert!(policy.is_nonblocking_gpu_transport());
    }

    #[test]
    fn dx12_native_ui_cpu_fallback_is_dirty_rect_and_native_ui_accounted() {
        let policy = dx12_native_ui_transport_policy();

        assert!(policy.cpu_fallback_dirty_rect_only);
        assert!(policy.cpu_fallback_throttle_when_unchanged);
        assert!(policy.cpu_fallback_throttle_when_world_over_budget);
        assert!(policy.keeps_native_ui_out_of_world_upload_accounting());
    }

    #[test]
    fn dx12_native_ui_composes_after_postprocess_and_before_debug() {
        let policy = dx12_native_ui_transport_policy();

        assert!(policy.keeps_native_ui_after_world_postprocess());
        assert!(policy.composition_stage.order_key() < FUN_RENDER_HUD_UI_STAGE.order_key());
        assert!(policy.keeps_native_ui_out_of_temporal_reconstruction());
    }
}
