use std::sync::atomic::{AtomicBool, Ordering};

use tracing::info;

/// D3D12 resource-state categories expected at the native DLSS injection point.
///
/// The first native shim pass should conservatively transition into these
/// states immediately before evaluation and restore the render graph's expected
/// output state afterward. Tighter wgpu graph-owned transitions can replace the
/// shim-owned transitions only after correctness is proven with validation-layer
/// captures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dx12NativeResourceState {
    ShaderResource,
    DepthRead,
    UnorderedAccess,
    RenderTarget,
}

impl Dx12NativeResourceState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ShaderResource => "shader_resource",
            Self::DepthRead => "depth_read",
            Self::UnorderedAccess => "unordered_access",
            Self::RenderTarget => "render_target",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dx12DlssResourceStateRules {
    pub input_color: Dx12NativeResourceState,
    pub depth: Dx12NativeResourceState,
    pub motion_vectors: Dx12NativeResourceState,
    pub output_color: Dx12NativeResourceState,
    pub exposure: Option<Dx12NativeResourceState>,
}

impl Dx12DlssResourceStateRules {
    pub const SUPER_RESOLUTION: Self = Self {
        input_color: Dx12NativeResourceState::ShaderResource,
        depth: Dx12NativeResourceState::DepthRead,
        motion_vectors: Dx12NativeResourceState::ShaderResource,
        output_color: Dx12NativeResourceState::UnorderedAccess,
        exposure: Some(Dx12NativeResourceState::ShaderResource),
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dx12DlssResourceStatePlan {
    pub before_evaluate: Dx12DlssResourceStateRules,
    pub restore_output_after_evaluate: Dx12NativeResourceState,
    pub transition_owner: &'static str,
}

impl Dx12DlssResourceStatePlan {
    pub const SUPER_RESOLUTION_FIRST_PASS: Self = Self {
        before_evaluate: Dx12DlssResourceStateRules::SUPER_RESOLUTION,
        restore_output_after_evaluate: Dx12NativeResourceState::RenderTarget,
        transition_owner: "native_shim_conservative",
    };
}

static STATE_PLAN_LOGGED: AtomicBool = AtomicBool::new(false);

pub fn log_dx12_dlss_resource_state_plan_once(plan: Dx12DlssResourceStatePlan) {
    if !STATE_PLAN_LOGGED.swap(true, Ordering::Relaxed) {
        info!(
            target: "fun::render::dx12_native",
            input_color = plan.before_evaluate.input_color.as_str(),
            depth = plan.before_evaluate.depth.as_str(),
            motion_vectors = plan.before_evaluate.motion_vectors.as_str(),
            output_color = plan.before_evaluate.output_color.as_str(),
            exposure = plan
                .before_evaluate
                .exposure
                .map(Dx12NativeResourceState::as_str)
                .unwrap_or("none"),
            restore_output_after_evaluate = plan.restore_output_after_evaluate.as_str(),
            transition_owner = plan.transition_owner,
            "FUN DX12 DLSS resource state plan"
        );
    }
}
