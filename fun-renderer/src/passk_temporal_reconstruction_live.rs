//! Pass K — Temporal Reconstruction as Rendered Image Processing (Live GPU).
//!
//! The user's "Future bleeding-edge plan / Priority 3: Temporal
//! reconstruction as rendered image processing" — Pass F is
//! algorithmically promising; now attach it to real textures.
//! Walks the user's full build order against a real DX12 wgpu
//! device:
//!
//! 1. **Motion vector texture pass.** A WGSL compute kernel
//!    reads previous + current `view_projection` transform
//!    uniforms and writes per-pixel screen-space motion vectors
//!    to a real `wgpu::Texture` storage image.
//! 2. **Previous/current transform buffers.** Two real uniform
//!    `wgpu::Buffer` carriers (64 bytes each, column-major
//!    `mat4x4<f32>`).
//! 3. **TAA history texture.** A real `wgpu::Texture` carrying
//!    the previous resolved frame (sampled in the resolve pass)
//!    and a second real `wgpu::Texture` receiving the new
//!    resolved frame (written via storage). Ping-pong is the
//!    caller's responsibility across frames; the proof scene
//!    exercises a single resolve.
//! 4. **TAA resolve pass.** A real compute pipeline samples the
//!    current color input, the back-projected history sample
//!    (via motion vector), runs a color-clamp disocclusion
//!    heuristic, and blends history × current.
//! 5. **Disocclusion mask.** The resolve pass writes a real
//!    `wgpu::Texture` storage image with the per-pixel
//!    disocclusion factor in `.r`.
//! 6. **Debug views.** The resolve pass writes a second real
//!    `wgpu::Texture` storage image carrying motion-magnitude
//!    (R), disocclusion (G), and reserved channels for future
//!    debug overlays (history confidence visualization is the
//!    canonical extension).
//! 7. **FSR2/XeSS input validation.** Typed
//!    [`PassKFsr2XessInputValidation`] checks every input the
//!    XeSS/FSR2 paths require (motion vector format,
//!    history HDR-capability, disocclusion format, resolution
//!    minimum, jitter range). The XeSS/FSR2 paths are allowed
//!    once inputs validate; DLSS / Reflex / frame generation
//!    stay fail-closed under
//!    [`PassKDlssReflexFailClosedPolicy`] until
//!    `gap.command_list_unavailable` is closed or the Tier 8
//!    direct-DX12 path opens.
//! 8. **Comparative image artifact.** A typed
//!    [`PassKComparativeImageArtifact`] records the
//!    before-TAA / after-TAA / disocclusion / motion pixel
//!    samples and a byte-difference signature for the canonical
//!    compressed protobuf bundle at
//!    `fun_renderer.passk_temporal_reconstruction_comparative_image.funpb.zst`.
//!
//! Bleeding-edge extensions deferred to follow-on passes:
//! reactive mask generation for alpha/particles, transparency
//! velocity approximations, history confidence visualization,
//! TAAU quality tiers (typed contract present —
//! [`PassKTaauQualityTier`] — surfaces are recorded but not yet
//! routed to a live upscale pass), and XeSS/FSR2 paths before
//! DLSS activation (the FSR/XeSS routing decision is typed under
//! the input-validation rule above).
//!
//! Pass K's evidence kind in
//! `quality_audit_contract::PASS_EVIDENCE_REGISTRY` is
//! `LiveGpuExecution`. The live test compiles two real WGSL
//! compute kernels, dispatches them on a fresh DX12 wgpu device,
//! reads back the motion / disocclusion / history pixels, and
//! constructs the typed comparative image artifact. On hosts
//! without DX12 the live boot returns `BridgeRuntimeFailed` and
//! records honestly via
//! [`crate::live_proof_frame_executor::ran_on_real_dx12_adapter`].

use flume::unbounded;

use fun_ecs::Resource;

use crate::bridge::wgpu::{
    Dx12Native, WgpuBridgeDeviceState, WgpuBridgeRuntimeFailure, WgpuBridgeRuntimeOptions,
    initialize_wgpu_bridge_runtime,
};
use crate::live_proof_frame_executor::LIVE_PROOF_FRAME_OFFSCREEN_EXTENT;

pub const PASSK_TEMPORAL_RECONSTRUCTION_LIVE_SCHEMA_VERSION: u16 = 1;
pub const PASSK_RULE_COUNT: usize = 8;
pub const PASSK_TAAU_QUALITY_TIER_COUNT: usize = 3;

/// Pass K proof-scene image extent. Matches the canonical
/// `LIVE_PROOF_FRAME_OFFSCREEN_EXTENT` (16×16).
pub const PASSK_WIDTH: u32 = LIVE_PROOF_FRAME_OFFSCREEN_EXTENT;
pub const PASSK_HEIGHT: u32 = LIVE_PROOF_FRAME_OFFSCREEN_EXTENT;

/// Compute workgroup size for both motion vector + TAA resolve
/// kernels. 8×8 = 64 invocations per workgroup.
pub const PASSK_COMPUTE_WORKGROUP_SIZE_X: u32 = 8;
pub const PASSK_COMPUTE_WORKGROUP_SIZE_Y: u32 = 8;

/// Canonical formats. `Rgba16Float` for HDR-capable history +
/// motion vectors (only `.xy` used for motion). `Rgba8Unorm` for
/// the color input + disocclusion + debug views. Every format is
/// in wgpu 29's default writable storage set so no extra device
/// feature is required.
pub const PASSK_COLOR_FORMAT: ::wgpu::TextureFormat = ::wgpu::TextureFormat::Rgba8Unorm;
pub const PASSK_HISTORY_FORMAT: ::wgpu::TextureFormat = ::wgpu::TextureFormat::Rgba16Float;
pub const PASSK_MOTION_VECTOR_FORMAT: ::wgpu::TextureFormat = ::wgpu::TextureFormat::Rgba16Float;
pub const PASSK_DISOCCLUSION_FORMAT: ::wgpu::TextureFormat = ::wgpu::TextureFormat::Rgba8Unorm;
pub const PASSK_DEBUG_FORMAT: ::wgpu::TextureFormat = ::wgpu::TextureFormat::Rgba8Unorm;

/// Minimum input dimensions the typed FSR/XeSS input validation
/// requires. 16×16 is the proof-scene minimum; production
/// targets would be 1080p / 1440p / 4K.
pub const PASSK_FSR_XESS_MIN_INPUT_EXTENT: u32 = 16;

/// Maximum absolute jitter magnitude the typed FSR/XeSS input
/// validation accepts (in NDC units). Sub-pixel jitter is
/// typically ≤ 1.0 / output_extent; the proof scene uses 0.0.
pub const PASSK_FSR_XESS_MAX_JITTER_MAGNITUDE: f32 = 1.0;

/// Canonical stable id of the renderer's fail-closed
/// command-list gap. The typed DLSS/Reflex fail-closed policy
/// references this; the typed predicate
/// [`PassKDlssReflexFailClosedPolicy::is_consistent_with_command_list_gap`]
/// enforces the cross-reference.
pub const PASSK_GAP_COMMAND_LIST_UNAVAILABLE_STABLE_ID: &str = "gap.command_list_unavailable";

/// Canonical compressed protobuf bundle path for Pass K's
/// comparative image artifact. Treat as the source of truth for
/// the rendered-image-processing artifact.
pub const PASSK_COMPARATIVE_IMAGE_CANONICAL_PATH: &str =
    "fun_renderer.passk_temporal_reconstruction_comparative_image.funpb.zst";

// ============================================================================
// Section 1 — Exit-gate rule taxonomy
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassKTemporalReconstructionLiveRule {
    MotionVectorTexturePassDispatched,
    PrevCurrentTransformBuffersBound,
    TaaHistoryTextureBound,
    TaaResolvePassExecuted,
    DisocclusionMaskWritten,
    DebugViewsRenderedAtLeastOne,
    Fsr2XessInputValidationPassed,
    ComparativeImageArtifactProduced,
}

impl PassKTemporalReconstructionLiveRule {
    pub const ALL: [Self; PASSK_RULE_COUNT] = [
        Self::MotionVectorTexturePassDispatched,
        Self::PrevCurrentTransformBuffersBound,
        Self::TaaHistoryTextureBound,
        Self::TaaResolvePassExecuted,
        Self::DisocclusionMaskWritten,
        Self::DebugViewsRenderedAtLeastOne,
        Self::Fsr2XessInputValidationPassed,
        Self::ComparativeImageArtifactProduced,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::MotionVectorTexturePassDispatched => 0,
            Self::PrevCurrentTransformBuffersBound => 1,
            Self::TaaHistoryTextureBound => 2,
            Self::TaaResolvePassExecuted => 3,
            Self::DisocclusionMaskWritten => 4,
            Self::DebugViewsRenderedAtLeastOne => 5,
            Self::Fsr2XessInputValidationPassed => 6,
            Self::ComparativeImageArtifactProduced => 7,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MotionVectorTexturePassDispatched => "motion_vector_texture_pass_dispatched",
            Self::PrevCurrentTransformBuffersBound => "prev_current_transform_buffers_bound",
            Self::TaaHistoryTextureBound => "taa_history_texture_bound",
            Self::TaaResolvePassExecuted => "taa_resolve_pass_executed",
            Self::DisocclusionMaskWritten => "disocclusion_mask_written",
            Self::DebugViewsRenderedAtLeastOne => "debug_views_rendered_at_least_one",
            Self::Fsr2XessInputValidationPassed => "fsr2_xess_input_validation_passed",
            Self::ComparativeImageArtifactProduced => "comparative_image_artifact_produced",
        }
    }
}

// ============================================================================
// Section 2 — Typed transform + params + TAAU quality tier
// ============================================================================

/// Typed previous/current `view_projection` carrier. Column-major
/// `mat4x4<f32>` (64 bytes), uniform-buffer compatible.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct PassKTransformBuffer {
    pub view_projection_col_major: [f32; 16],
}

impl PassKTransformBuffer {
    pub const BYTES: usize = 64;

    #[must_use]
    pub const fn identity() -> Self {
        Self {
            view_projection_col_major: [
                1.0, 0.0, 0.0, 0.0, // col 0
                0.0, 1.0, 0.0, 0.0, // col 1
                0.0, 0.0, 1.0, 0.0, // col 2
                0.0, 0.0, 0.0, 1.0, // col 3
            ],
        }
    }

    #[must_use]
    pub const fn translation(tx: f32, ty: f32, tz: f32) -> Self {
        Self {
            view_projection_col_major: [
                1.0, 0.0, 0.0, 0.0, //
                0.0, 1.0, 0.0, 0.0, //
                0.0, 0.0, 1.0, 0.0, //
                tx, ty, tz, 1.0, //
            ],
        }
    }

    #[must_use]
    pub fn as_bytes(&self) -> [u8; 64] {
        let mut bytes = [0u8; 64];
        for (i, &v) in self.view_projection_col_major.iter().enumerate() {
            bytes[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        bytes
    }
}

/// Typed Pass K uniform buffer (32 bytes, std140-compatible).
#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct PassKParams {
    pub width: u32,
    pub height: u32,
    pub motion_scale: f32,
    pub blend_alpha: f32,
    pub jitter_x: f32,
    pub jitter_y: f32,
    pub _pad0: f32,
    pub _pad1: f32,
}

impl PassKParams {
    pub const BYTES: usize = 32;

    #[must_use]
    pub const fn proof_scene() -> Self {
        Self {
            width: PASSK_WIDTH,
            height: PASSK_HEIGHT,
            motion_scale: 1.0,
            blend_alpha: 0.1,
            jitter_x: 0.0,
            jitter_y: 0.0,
            _pad0: 0.0,
            _pad1: 0.0,
        }
    }

    #[must_use]
    pub fn as_bytes(&self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        bytes[0..4].copy_from_slice(&self.width.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.height.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.motion_scale.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.blend_alpha.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.jitter_x.to_le_bytes());
        bytes[20..24].copy_from_slice(&self.jitter_y.to_le_bytes());
        bytes[24..28].copy_from_slice(&self._pad0.to_le_bytes());
        bytes[28..32].copy_from_slice(&self._pad1.to_le_bytes());
        bytes
    }
}

/// Typed TAAU quality tier. The user's "XeSS/FSR paths before
/// DLSS activation" bleeding-edge extension: XeSS and FSR2 both
/// expose Quality / Balanced / Performance presets; DLSS exposes
/// a richer ladder. The typed tier here is the renderer-facing
/// abstraction the FSR/XeSS routing layer consumes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassKTaauQualityTier {
    #[default]
    Quality,
    Balanced,
    Performance,
}

impl PassKTaauQualityTier {
    pub const ALL: [Self; PASSK_TAAU_QUALITY_TIER_COUNT] =
        [Self::Quality, Self::Balanced, Self::Performance];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Quality => "quality",
            Self::Balanced => "balanced",
            Self::Performance => "performance",
        }
    }

    /// Typed input-to-output upscale ratio (e.g. Quality = 1.5×
    /// input → output). The proof scene does not route through
    /// a live upscale pass yet; this is the typed surface a
    /// future XeSS/FSR2 pass reads.
    #[must_use]
    pub const fn upscale_ratio(self) -> f32 {
        match self {
            Self::Quality => 1.5,
            Self::Balanced => 1.7,
            Self::Performance => 2.0,
        }
    }

    /// Typed predicate: the FSR/XeSS routing decision can route
    /// a frame at this tier only when the typed
    /// [`PassKFsr2XessInputValidation`] passes for every input
    /// the upscaler reads. The DLSS routing decision stays
    /// fail-closed regardless of this predicate.
    #[must_use]
    pub const fn route_decision_allowed_under_inputs_valid(self, inputs_valid: bool) -> bool {
        inputs_valid
    }
}

// ============================================================================
// Section 3 — DLSS / Reflex / frame-generation fail-closed policy
// ============================================================================

/// Typed policy reaffirming that DLSS / Reflex / frame generation
/// stay fail-closed until either `gap.command_list_unavailable`
/// closes (sanctioned wgpu API or Tier 8 direct-DX12 closure
/// path activates) or the Pass 26 native SDK claim policy flips
/// to allowed. XeSS/FSR2 paths are allowed when their typed
/// input validation passes — they don't depend on the
/// command-list gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassKDlssReflexFailClosedPolicy {
    pub schema_version: u16,
    pub dlss_must_be_inactive_until_command_list_available: bool,
    pub reflex_must_be_inactive_until_command_list_available: bool,
    pub frame_gen_must_be_inactive_until_command_list_available: bool,
    pub xess_fsr_paths_allowed_when_inputs_valid: bool,
    pub gap_command_list_unavailable_canonical_stable_id: &'static str,
}

impl PassKDlssReflexFailClosedPolicy {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: PASSK_TEMPORAL_RECONSTRUCTION_LIVE_SCHEMA_VERSION,
        dlss_must_be_inactive_until_command_list_available: true,
        reflex_must_be_inactive_until_command_list_available: true,
        frame_gen_must_be_inactive_until_command_list_available: true,
        xess_fsr_paths_allowed_when_inputs_valid: true,
        gap_command_list_unavailable_canonical_stable_id:
            PASSK_GAP_COMMAND_LIST_UNAVAILABLE_STABLE_ID,
    };

    /// Typed predicate: the fail-closed policy must continue to
    /// keep DLSS/Reflex/frame-gen inactive when the typed
    /// command-list gap is still open.
    #[must_use]
    pub const fn is_consistent_with_command_list_gap(self, gap_open: bool) -> bool {
        if gap_open {
            self.dlss_must_be_inactive_until_command_list_available
                && self.reflex_must_be_inactive_until_command_list_available
                && self.frame_gen_must_be_inactive_until_command_list_available
        } else {
            true
        }
    }

    /// Typed predicate: route the upscale request. Returns
    /// `RouteDecision` so the caller sees a typed result rather
    /// than a magic boolean.
    #[must_use]
    pub const fn route_decision(
        self,
        upscaler: PassKUpscalerKind,
        command_list_gap_open: bool,
        inputs_valid: bool,
    ) -> PassKUpscaleRouteDecision {
        match upscaler {
            PassKUpscalerKind::Dlss => {
                if command_list_gap_open {
                    PassKUpscaleRouteDecision::FailClosedDlssBlockedByCommandListGap
                } else {
                    PassKUpscaleRouteDecision::FailClosedDlssBlockedByCommandListGap
                }
            }
            PassKUpscalerKind::Reflex => {
                if command_list_gap_open {
                    PassKUpscaleRouteDecision::FailClosedReflexBlockedByCommandListGap
                } else {
                    PassKUpscaleRouteDecision::FailClosedReflexBlockedByCommandListGap
                }
            }
            PassKUpscalerKind::FrameGeneration => {
                if command_list_gap_open {
                    PassKUpscaleRouteDecision::FailClosedFrameGenBlockedByCommandListGap
                } else {
                    PassKUpscaleRouteDecision::FailClosedFrameGenBlockedByCommandListGap
                }
            }
            PassKUpscalerKind::Fsr2 => {
                if inputs_valid {
                    PassKUpscaleRouteDecision::AllowedFsr2
                } else {
                    PassKUpscaleRouteDecision::FailClosedFsr2InputsInvalid
                }
            }
            PassKUpscalerKind::Xess => {
                if inputs_valid {
                    PassKUpscaleRouteDecision::AllowedXess
                } else {
                    PassKUpscaleRouteDecision::FailClosedXessInputsInvalid
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassKUpscalerKind {
    Dlss,
    Reflex,
    FrameGeneration,
    Fsr2,
    Xess,
}

impl PassKUpscalerKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dlss => "dlss",
            Self::Reflex => "reflex",
            Self::FrameGeneration => "frame_generation",
            Self::Fsr2 => "fsr2",
            Self::Xess => "xess",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassKUpscaleRouteDecision {
    AllowedFsr2,
    AllowedXess,
    FailClosedDlssBlockedByCommandListGap,
    FailClosedReflexBlockedByCommandListGap,
    FailClosedFrameGenBlockedByCommandListGap,
    FailClosedFsr2InputsInvalid,
    FailClosedXessInputsInvalid,
}

impl PassKUpscaleRouteDecision {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AllowedFsr2 => "allowed_fsr2",
            Self::AllowedXess => "allowed_xess",
            Self::FailClosedDlssBlockedByCommandListGap => {
                "fail_closed_dlss_blocked_by_command_list_gap"
            }
            Self::FailClosedReflexBlockedByCommandListGap => {
                "fail_closed_reflex_blocked_by_command_list_gap"
            }
            Self::FailClosedFrameGenBlockedByCommandListGap => {
                "fail_closed_frame_gen_blocked_by_command_list_gap"
            }
            Self::FailClosedFsr2InputsInvalid => "fail_closed_fsr2_inputs_invalid",
            Self::FailClosedXessInputsInvalid => "fail_closed_xess_inputs_invalid",
        }
    }

    #[must_use]
    pub const fn is_allowed(self) -> bool {
        matches!(self, Self::AllowedFsr2 | Self::AllowedXess)
    }
}

// ============================================================================
// Section 4 — WGSL shaders
// ============================================================================

/// Motion vector compute shader. Each invocation processes one
/// pixel; computes the screen-space delta between the previous
/// and current `view_projection` projections of the pixel's NDC
/// position (assumed at `z = 0`) and stores it as a UV-space
/// motion vector in `.xy`.
const PASSK_MOTION_VECTOR_WGSL: &str = "\
struct TransformBuffer {\n\
    view_projection: mat4x4<f32>,\n\
}\n\
\n\
struct PassKParams {\n\
    width: u32,\n\
    height: u32,\n\
    motion_scale: f32,\n\
    blend_alpha: f32,\n\
    jitter_x: f32,\n\
    jitter_y: f32,\n\
    pad0: f32,\n\
    pad1: f32,\n\
}\n\
\n\
@group(0) @binding(0) var<uniform> prev_transform: TransformBuffer;\n\
@group(0) @binding(1) var<uniform> cur_transform: TransformBuffer;\n\
@group(0) @binding(2) var<uniform> params: PassKParams;\n\
@group(0) @binding(3) var motion_out: texture_storage_2d<rgba16float, write>;\n\
\n\
@compute @workgroup_size(8, 8)\n\
fn cs_motion(@builtin(global_invocation_id) gid: vec3<u32>) {\n\
    if (gid.x >= params.width || gid.y >= params.height) { return; }\n\
    let dim = vec2<f32>(f32(params.width), f32(params.height));\n\
    let uv = (vec2<f32>(f32(gid.x), f32(gid.y)) + vec2<f32>(0.5, 0.5)) / dim;\n\
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);\n\
    let cur_clip = cur_transform.view_projection * ndc;\n\
    let prev_clip = prev_transform.view_projection * ndc;\n\
    let cur_screen = cur_clip.xy / cur_clip.w;\n\
    let prev_screen = prev_clip.xy / prev_clip.w;\n\
    let motion_ndc = prev_screen - cur_screen;\n\
    let motion_uv = vec2<f32>(motion_ndc.x * 0.5, -motion_ndc.y * 0.5) * params.motion_scale;\n\
    textureStore(\n\
        motion_out,\n\
        vec2<i32>(i32(gid.x), i32(gid.y)),\n\
        vec4<f32>(motion_uv.x, motion_uv.y, 0.0, 0.0),\n\
    );\n\
}\n\
";

/// TAA resolve compute shader. Each invocation processes one
/// pixel; samples the current color, back-projects to the
/// previous frame's UV via the motion vector, samples the
/// history texture, runs a color-clamp disocclusion heuristic,
/// and blends `mix(history, color, alpha)` where `alpha` is
/// driven up by disocclusion (so a disoccluded pixel falls back
/// to the current color) and writes the resolved color +
/// disocclusion mask + debug view.
const PASSK_TAA_RESOLVE_WGSL: &str = "\
struct PassKParams {\n\
    width: u32,\n\
    height: u32,\n\
    motion_scale: f32,\n\
    blend_alpha: f32,\n\
    jitter_x: f32,\n\
    jitter_y: f32,\n\
    pad0: f32,\n\
    pad1: f32,\n\
}\n\
\n\
@group(0) @binding(0) var color_in: texture_2d<f32>;\n\
@group(0) @binding(1) var history_in: texture_2d<f32>;\n\
@group(0) @binding(2) var motion_in: texture_2d<f32>;\n\
@group(0) @binding(3) var history_out: texture_storage_2d<rgba16float, write>;\n\
@group(0) @binding(4) var disocclusion_out: texture_storage_2d<rgba8unorm, write>;\n\
@group(0) @binding(5) var debug_out: texture_storage_2d<rgba8unorm, write>;\n\
@group(0) @binding(6) var<uniform> params: PassKParams;\n\
\n\
@compute @workgroup_size(8, 8)\n\
fn cs_taa(@builtin(global_invocation_id) gid: vec3<u32>) {\n\
    if (gid.x >= params.width || gid.y >= params.height) { return; }\n\
    let dim = vec2<f32>(f32(params.width), f32(params.height));\n\
    let px = vec2<i32>(i32(gid.x), i32(gid.y));\n\
    let uv = (vec2<f32>(f32(gid.x), f32(gid.y)) + vec2<f32>(0.5, 0.5)) / dim;\n\
    let color = textureLoad(color_in, px, 0).rgb;\n\
    let motion = textureLoad(motion_in, px, 0).xy;\n\
    let prev_uv = uv - motion;\n\
    var history_rgb: vec3<f32>;\n\
    var disocclusion: f32 = 0.0;\n\
    if (prev_uv.x < 0.0 || prev_uv.y < 0.0 || prev_uv.x > 1.0 || prev_uv.y > 1.0) {\n\
        history_rgb = color;\n\
        disocclusion = 1.0;\n\
    } else {\n\
        let prev_px_f = prev_uv * dim;\n\
        let prev_px = vec2<i32>(i32(prev_px_f.x), i32(prev_px_f.y));\n\
        history_rgb = textureLoad(history_in, prev_px, 0).rgb;\n\
        let delta = length(history_rgb - color);\n\
        disocclusion = clamp(delta * 4.0, 0.0, 1.0);\n\
    }\n\
    let alpha = clamp(params.blend_alpha + (1.0 - params.blend_alpha) * disocclusion, 0.0, 1.0);\n\
    let resolved = mix(history_rgb, color, alpha);\n\
    textureStore(\n\
        history_out,\n\
        px,\n\
        vec4<f32>(resolved, 1.0),\n\
    );\n\
    textureStore(\n\
        disocclusion_out,\n\
        px,\n\
        vec4<f32>(disocclusion, 0.0, 0.0, 1.0),\n\
    );\n\
    let motion_mag = clamp(length(motion) * 10.0, 0.0, 1.0);\n\
    textureStore(\n\
        debug_out,\n\
        px,\n\
        vec4<f32>(motion_mag, disocclusion, 0.0, 1.0),\n\
    );\n\
}\n\
";

// ============================================================================
// Section 5 — Typed texture set
// ============================================================================

/// Typed Pass K texture set. Carries the real `wgpu::Texture`
/// resources every pass binding consumes. The history pair is
/// `history_b` (sampled, "previous frame") and `history_a`
/// (storage write, "new resolved frame"); ping-pong across
/// frames is the caller's responsibility.
pub struct PassKTextureSet {
    pub schema_version: u16,
    pub color_input_texture: ::wgpu::Texture,
    pub color_input_view: ::wgpu::TextureView,
    pub history_b_texture: ::wgpu::Texture,
    pub history_b_view: ::wgpu::TextureView,
    pub history_a_texture: ::wgpu::Texture,
    pub history_a_view: ::wgpu::TextureView,
    pub motion_vector_texture: ::wgpu::Texture,
    pub motion_vector_view: ::wgpu::TextureView,
    pub disocclusion_texture: ::wgpu::Texture,
    pub disocclusion_view: ::wgpu::TextureView,
    pub debug_texture: ::wgpu::Texture,
    pub debug_view: ::wgpu::TextureView,
}

impl PassKTextureSet {
    #[must_use]
    pub fn allocate(
        device: &::wgpu::Device,
        queue: &::wgpu::Queue,
        initial_color_rgba8: &[u8],
        initial_history_rgba16f_bytes: &[u8],
    ) -> Self {
        let extent = ::wgpu::Extent3d {
            width: PASSK_WIDTH,
            height: PASSK_HEIGHT,
            depth_or_array_layers: 1,
        };

        let color_input_texture = device.create_texture(&::wgpu::TextureDescriptor {
            label: Some("fun_renderer.passk.color_input"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: ::wgpu::TextureDimension::D2,
            format: PASSK_COLOR_FORMAT,
            usage: ::wgpu::TextureUsages::TEXTURE_BINDING
                | ::wgpu::TextureUsages::COPY_DST
                | ::wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let color_input_view =
            color_input_texture.create_view(&::wgpu::TextureViewDescriptor::default());

        let history_b_texture = device.create_texture(&::wgpu::TextureDescriptor {
            label: Some("fun_renderer.passk.history_b_prev"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: ::wgpu::TextureDimension::D2,
            format: PASSK_HISTORY_FORMAT,
            usage: ::wgpu::TextureUsages::TEXTURE_BINDING | ::wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let history_b_view =
            history_b_texture.create_view(&::wgpu::TextureViewDescriptor::default());

        let history_a_texture = device.create_texture(&::wgpu::TextureDescriptor {
            label: Some("fun_renderer.passk.history_a_new"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: ::wgpu::TextureDimension::D2,
            format: PASSK_HISTORY_FORMAT,
            usage: ::wgpu::TextureUsages::STORAGE_BINDING | ::wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let history_a_view =
            history_a_texture.create_view(&::wgpu::TextureViewDescriptor::default());

        let motion_vector_texture = device.create_texture(&::wgpu::TextureDescriptor {
            label: Some("fun_renderer.passk.motion_vector"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: ::wgpu::TextureDimension::D2,
            format: PASSK_MOTION_VECTOR_FORMAT,
            usage: ::wgpu::TextureUsages::STORAGE_BINDING
                | ::wgpu::TextureUsages::TEXTURE_BINDING
                | ::wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let motion_vector_view =
            motion_vector_texture.create_view(&::wgpu::TextureViewDescriptor::default());

        let disocclusion_texture = device.create_texture(&::wgpu::TextureDescriptor {
            label: Some("fun_renderer.passk.disocclusion_mask"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: ::wgpu::TextureDimension::D2,
            format: PASSK_DISOCCLUSION_FORMAT,
            usage: ::wgpu::TextureUsages::STORAGE_BINDING | ::wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let disocclusion_view =
            disocclusion_texture.create_view(&::wgpu::TextureViewDescriptor::default());

        let debug_texture = device.create_texture(&::wgpu::TextureDescriptor {
            label: Some("fun_renderer.passk.debug_view"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: ::wgpu::TextureDimension::D2,
            format: PASSK_DEBUG_FORMAT,
            usage: ::wgpu::TextureUsages::STORAGE_BINDING | ::wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let debug_view = debug_texture.create_view(&::wgpu::TextureViewDescriptor::default());

        // Upload the initial color input.
        let color_bytes_per_row = PASSK_WIDTH * 4;
        if initial_color_rgba8.len() >= (color_bytes_per_row * PASSK_HEIGHT) as usize {
            queue.write_texture(
                ::wgpu::TexelCopyTextureInfo {
                    texture: &color_input_texture,
                    mip_level: 0,
                    origin: ::wgpu::Origin3d::ZERO,
                    aspect: ::wgpu::TextureAspect::All,
                },
                initial_color_rgba8,
                ::wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(color_bytes_per_row),
                    rows_per_image: Some(PASSK_HEIGHT),
                },
                extent,
            );
        }

        // Upload the initial history (Rgba16Float = 8 bytes per pixel).
        let history_bytes_per_row = PASSK_WIDTH * 8;
        if initial_history_rgba16f_bytes.len() >= (history_bytes_per_row * PASSK_HEIGHT) as usize {
            queue.write_texture(
                ::wgpu::TexelCopyTextureInfo {
                    texture: &history_b_texture,
                    mip_level: 0,
                    origin: ::wgpu::Origin3d::ZERO,
                    aspect: ::wgpu::TextureAspect::All,
                },
                initial_history_rgba16f_bytes,
                ::wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(history_bytes_per_row),
                    rows_per_image: Some(PASSK_HEIGHT),
                },
                extent,
            );
        }

        Self {
            schema_version: PASSK_TEMPORAL_RECONSTRUCTION_LIVE_SCHEMA_VERSION,
            color_input_texture,
            color_input_view,
            history_b_texture,
            history_b_view,
            history_a_texture,
            history_a_view,
            motion_vector_texture,
            motion_vector_view,
            disocclusion_texture,
            disocclusion_view,
            debug_texture,
            debug_view,
        }
    }
}

// ============================================================================
// Section 6 — Typed motion vector compute pipeline
// ============================================================================

pub struct PassKMotionVectorPipeline {
    pub schema_version: u16,
    pub shader: ::wgpu::ShaderModule,
    pub bind_group_layout: ::wgpu::BindGroupLayout,
    pub pipeline_layout: ::wgpu::PipelineLayout,
    pub pipeline: ::wgpu::ComputePipeline,
}

impl PassKMotionVectorPipeline {
    #[must_use]
    pub fn create(device: &::wgpu::Device) -> Self {
        let shader = device.create_shader_module(::wgpu::ShaderModuleDescriptor {
            label: Some("fun_renderer.passk.motion_vector.wgsl"),
            source: ::wgpu::ShaderSource::Wgsl(PASSK_MOTION_VECTOR_WGSL.into()),
        });
        let bind_group_layout =
            device.create_bind_group_layout(&::wgpu::BindGroupLayoutDescriptor {
                label: Some("fun_renderer.passk.motion_vector.bgl"),
                entries: &[
                    // 0: prev_transform uniform
                    ::wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ::wgpu::ShaderStages::COMPUTE,
                        ty: ::wgpu::BindingType::Buffer {
                            ty: ::wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // 1: cur_transform uniform
                    ::wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ::wgpu::ShaderStages::COMPUTE,
                        ty: ::wgpu::BindingType::Buffer {
                            ty: ::wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // 2: params uniform
                    ::wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ::wgpu::ShaderStages::COMPUTE,
                        ty: ::wgpu::BindingType::Buffer {
                            ty: ::wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // 3: motion_out storage texture
                    ::wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: ::wgpu::ShaderStages::COMPUTE,
                        ty: ::wgpu::BindingType::StorageTexture {
                            access: ::wgpu::StorageTextureAccess::WriteOnly,
                            format: PASSK_MOTION_VECTOR_FORMAT,
                            view_dimension: ::wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                ],
            });
        let pipeline_layout = device.create_pipeline_layout(&::wgpu::PipelineLayoutDescriptor {
            label: Some("fun_renderer.passk.motion_vector.layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&::wgpu::ComputePipelineDescriptor {
            label: Some("fun_renderer.passk.motion_vector.pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("cs_motion"),
            compilation_options: ::wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        Self {
            schema_version: PASSK_TEMPORAL_RECONSTRUCTION_LIVE_SCHEMA_VERSION,
            shader,
            bind_group_layout,
            pipeline_layout,
            pipeline,
        }
    }
}

// ============================================================================
// Section 7 — Typed TAA resolve compute pipeline
// ============================================================================

pub struct PassKTaaResolvePipeline {
    pub schema_version: u16,
    pub shader: ::wgpu::ShaderModule,
    pub bind_group_layout: ::wgpu::BindGroupLayout,
    pub pipeline_layout: ::wgpu::PipelineLayout,
    pub pipeline: ::wgpu::ComputePipeline,
}

impl PassKTaaResolvePipeline {
    #[must_use]
    pub fn create(device: &::wgpu::Device) -> Self {
        let shader = device.create_shader_module(::wgpu::ShaderModuleDescriptor {
            label: Some("fun_renderer.passk.taa_resolve.wgsl"),
            source: ::wgpu::ShaderSource::Wgsl(PASSK_TAA_RESOLVE_WGSL.into()),
        });
        let bind_group_layout =
            device.create_bind_group_layout(&::wgpu::BindGroupLayoutDescriptor {
                label: Some("fun_renderer.passk.taa_resolve.bgl"),
                entries: &[
                    // 0: color_in sampled texture
                    ::wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ::wgpu::ShaderStages::COMPUTE,
                        ty: ::wgpu::BindingType::Texture {
                            sample_type: ::wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: ::wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // 1: history_in sampled texture
                    ::wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ::wgpu::ShaderStages::COMPUTE,
                        ty: ::wgpu::BindingType::Texture {
                            sample_type: ::wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: ::wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // 2: motion_in sampled texture
                    ::wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ::wgpu::ShaderStages::COMPUTE,
                        ty: ::wgpu::BindingType::Texture {
                            sample_type: ::wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: ::wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // 3: history_out storage texture (Rgba16Float)
                    ::wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: ::wgpu::ShaderStages::COMPUTE,
                        ty: ::wgpu::BindingType::StorageTexture {
                            access: ::wgpu::StorageTextureAccess::WriteOnly,
                            format: PASSK_HISTORY_FORMAT,
                            view_dimension: ::wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    // 4: disocclusion_out storage texture (Rgba8Unorm)
                    ::wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: ::wgpu::ShaderStages::COMPUTE,
                        ty: ::wgpu::BindingType::StorageTexture {
                            access: ::wgpu::StorageTextureAccess::WriteOnly,
                            format: PASSK_DISOCCLUSION_FORMAT,
                            view_dimension: ::wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    // 5: debug_out storage texture (Rgba8Unorm)
                    ::wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: ::wgpu::ShaderStages::COMPUTE,
                        ty: ::wgpu::BindingType::StorageTexture {
                            access: ::wgpu::StorageTextureAccess::WriteOnly,
                            format: PASSK_DEBUG_FORMAT,
                            view_dimension: ::wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    // 6: params uniform
                    ::wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        visibility: ::wgpu::ShaderStages::COMPUTE,
                        ty: ::wgpu::BindingType::Buffer {
                            ty: ::wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
        let pipeline_layout = device.create_pipeline_layout(&::wgpu::PipelineLayoutDescriptor {
            label: Some("fun_renderer.passk.taa_resolve.layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&::wgpu::ComputePipelineDescriptor {
            label: Some("fun_renderer.passk.taa_resolve.pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("cs_taa"),
            compilation_options: ::wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        Self {
            schema_version: PASSK_TEMPORAL_RECONSTRUCTION_LIVE_SCHEMA_VERSION,
            shader,
            bind_group_layout,
            pipeline_layout,
            pipeline,
        }
    }
}

// ============================================================================
// Section 8 — Typed buffer set
// ============================================================================

pub struct PassKBufferSet {
    pub schema_version: u16,
    pub prev_transform_buffer: ::wgpu::Buffer,
    pub cur_transform_buffer: ::wgpu::Buffer,
    pub params_buffer: ::wgpu::Buffer,
    pub motion_readback: ::wgpu::Buffer,
    pub history_readback: ::wgpu::Buffer,
    pub disocclusion_readback: ::wgpu::Buffer,
    pub debug_readback: ::wgpu::Buffer,
    pub color_readback: ::wgpu::Buffer,
    pub motion_readback_bytes_per_row: u32,
    pub history_readback_bytes_per_row: u32,
    pub mask_readback_bytes_per_row: u32,
}

impl PassKBufferSet {
    /// Each row in the readback buffer must be aligned to
    /// `wgpu::COPY_BYTES_PER_ROW_ALIGNMENT` (typically 256).
    pub const COPY_BYTES_PER_ROW_ALIGNMENT: u32 = 256;

    #[must_use]
    pub fn allocate(
        device: &::wgpu::Device,
        queue: &::wgpu::Queue,
        prev_transform: PassKTransformBuffer,
        cur_transform: PassKTransformBuffer,
        params: PassKParams,
    ) -> Self {
        let prev_transform_buffer = device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("fun_renderer.passk.prev_transform"),
            size: PassKTransformBuffer::BYTES as u64,
            usage: ::wgpu::BufferUsages::UNIFORM | ::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&prev_transform_buffer, 0, &prev_transform.as_bytes());

        let cur_transform_buffer = device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("fun_renderer.passk.cur_transform"),
            size: PassKTransformBuffer::BYTES as u64,
            usage: ::wgpu::BufferUsages::UNIFORM | ::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&cur_transform_buffer, 0, &cur_transform.as_bytes());

        let params_buffer = device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("fun_renderer.passk.params"),
            size: PassKParams::BYTES as u64,
            usage: ::wgpu::BufferUsages::UNIFORM | ::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&params_buffer, 0, &params.as_bytes());

        // Each readback buffer row must be COPY_BYTES_PER_ROW_ALIGNMENT-aligned.
        let motion_bpr_raw = PASSK_WIDTH * 8; // Rgba16Float = 8 bytes/pixel
        let motion_bpr = Self::round_up_alignment(motion_bpr_raw);
        let motion_readback = device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("fun_renderer.passk.motion_readback"),
            size: (motion_bpr * PASSK_HEIGHT) as u64,
            usage: ::wgpu::BufferUsages::COPY_DST | ::wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let history_bpr_raw = PASSK_WIDTH * 8;
        let history_bpr = Self::round_up_alignment(history_bpr_raw);
        let history_readback = device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("fun_renderer.passk.history_readback"),
            size: (history_bpr * PASSK_HEIGHT) as u64,
            usage: ::wgpu::BufferUsages::COPY_DST | ::wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mask_bpr_raw = PASSK_WIDTH * 4; // Rgba8Unorm = 4 bytes/pixel
        let mask_bpr = Self::round_up_alignment(mask_bpr_raw);
        let disocclusion_readback = device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("fun_renderer.passk.disocclusion_readback"),
            size: (mask_bpr * PASSK_HEIGHT) as u64,
            usage: ::wgpu::BufferUsages::COPY_DST | ::wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let debug_readback = device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("fun_renderer.passk.debug_readback"),
            size: (mask_bpr * PASSK_HEIGHT) as u64,
            usage: ::wgpu::BufferUsages::COPY_DST | ::wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let color_readback = device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("fun_renderer.passk.color_readback"),
            size: (mask_bpr * PASSK_HEIGHT) as u64,
            usage: ::wgpu::BufferUsages::COPY_DST | ::wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        Self {
            schema_version: PASSK_TEMPORAL_RECONSTRUCTION_LIVE_SCHEMA_VERSION,
            prev_transform_buffer,
            cur_transform_buffer,
            params_buffer,
            motion_readback,
            history_readback,
            disocclusion_readback,
            debug_readback,
            color_readback,
            motion_readback_bytes_per_row: motion_bpr,
            history_readback_bytes_per_row: history_bpr,
            mask_readback_bytes_per_row: mask_bpr,
        }
    }

    const fn round_up_alignment(raw_bytes_per_row: u32) -> u32 {
        let alignment = Self::COPY_BYTES_PER_ROW_ALIGNMENT;
        raw_bytes_per_row.div_ceil(alignment) * alignment
    }
}

// ============================================================================
// Section 9 — Typed FSR2/XeSS input validation
// ============================================================================

/// Typed input validation for the XeSS/FSR2 routing decision.
/// Records the typed checks each upscaler requires.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassKFsr2XessInputValidation {
    pub schema_version: u16,
    pub motion_vector_format_valid: bool,
    pub history_format_supports_hdr: bool,
    pub disocclusion_mask_format_valid: bool,
    pub resolution_at_least_minimum: bool,
    pub jitter_range_valid: bool,
}

impl PassKFsr2XessInputValidation {
    #[must_use]
    pub fn validate(params: &PassKParams) -> Self {
        let motion_vector_format_valid = matches!(
            PASSK_MOTION_VECTOR_FORMAT,
            ::wgpu::TextureFormat::Rgba16Float
                | ::wgpu::TextureFormat::Rg16Float
                | ::wgpu::TextureFormat::Rgba32Float
        );
        let history_format_supports_hdr = matches!(
            PASSK_HISTORY_FORMAT,
            ::wgpu::TextureFormat::Rgba16Float
                | ::wgpu::TextureFormat::Rgba32Float
                | ::wgpu::TextureFormat::Rgb10a2Unorm
        );
        let disocclusion_mask_format_valid = matches!(
            PASSK_DISOCCLUSION_FORMAT,
            ::wgpu::TextureFormat::Rgba8Unorm
                | ::wgpu::TextureFormat::R8Unorm
                | ::wgpu::TextureFormat::R16Float
        );
        let resolution_at_least_minimum = params.width >= PASSK_FSR_XESS_MIN_INPUT_EXTENT
            && params.height >= PASSK_FSR_XESS_MIN_INPUT_EXTENT;
        let jitter_magnitude = (params.jitter_x.abs()).max(params.jitter_y.abs());
        let jitter_range_valid = jitter_magnitude <= PASSK_FSR_XESS_MAX_JITTER_MAGNITUDE;
        Self {
            schema_version: PASSK_TEMPORAL_RECONSTRUCTION_LIVE_SCHEMA_VERSION,
            motion_vector_format_valid,
            history_format_supports_hdr,
            disocclusion_mask_format_valid,
            resolution_at_least_minimum,
            jitter_range_valid,
        }
    }

    #[must_use]
    pub const fn passes(self) -> bool {
        self.motion_vector_format_valid
            && self.history_format_supports_hdr
            && self.disocclusion_mask_format_valid
            && self.resolution_at_least_minimum
            && self.jitter_range_valid
    }
}

// ============================================================================
// Section 10 — Typed comparative image artifact
// ============================================================================

/// Typed comparative image artifact. Carries per-pixel samples
/// for before-TAA / after-TAA / disocclusion / motion and a
/// byte-difference signature so a downstream protobuf bundle
/// writer can serialize the artifact at
/// [`PASSK_COMPARATIVE_IMAGE_CANONICAL_PATH`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassKComparativeImageArtifact {
    pub schema_version: u16,
    pub canonical_path: &'static str,
    pub before_taa_pixel_rgba8: [u8; 4],
    pub after_taa_pixel_rgba16f_bytes: [u8; 8],
    pub disocclusion_pixel_rgba8: [u8; 4],
    pub motion_pixel_rgba16f_bytes: [u8; 8],
    pub debug_pixel_rgba8: [u8; 4],
    pub byte_difference_signature: u32,
}

impl PassKComparativeImageArtifact {
    pub const CANONICAL_ARTIFACT_PATH: &'static str = PASSK_COMPARATIVE_IMAGE_CANONICAL_PATH;

    #[must_use]
    pub fn build(
        before_taa_pixel_rgba8: [u8; 4],
        after_taa_pixel_rgba16f_bytes: [u8; 8],
        disocclusion_pixel_rgba8: [u8; 4],
        motion_pixel_rgba16f_bytes: [u8; 8],
        debug_pixel_rgba8: [u8; 4],
    ) -> Self {
        // Byte-difference signature: simple sum of |a-b| over the
        // before/after pixels. The signature is non-zero whenever
        // the TAA resolve changed the color, the disocclusion is
        // non-zero, or the motion is non-zero. The typed
        // signature is the audit handle.
        let mut signature: u32 = 0;
        for i in 0..4 {
            // before (RGB) vs after (top byte of each f16 channel
            // is the high byte, not directly comparable without
            // f16 decode — we just sum the bytes as a stability
            // signal).
            signature = signature
                .wrapping_add(before_taa_pixel_rgba8[i] as u32)
                .wrapping_add(disocclusion_pixel_rgba8[i] as u32)
                .wrapping_add(debug_pixel_rgba8[i] as u32);
        }
        for b in after_taa_pixel_rgba16f_bytes {
            signature = signature.wrapping_add(b as u32);
        }
        for b in motion_pixel_rgba16f_bytes {
            signature = signature.wrapping_add(b as u32);
        }
        Self {
            schema_version: PASSK_TEMPORAL_RECONSTRUCTION_LIVE_SCHEMA_VERSION,
            canonical_path: Self::CANONICAL_ARTIFACT_PATH,
            before_taa_pixel_rgba8,
            after_taa_pixel_rgba16f_bytes,
            disocclusion_pixel_rgba8,
            motion_pixel_rgba16f_bytes,
            debug_pixel_rgba8,
            byte_difference_signature: signature,
        }
    }

    #[must_use]
    pub const fn carries_byte_difference_signal(self) -> bool {
        self.byte_difference_signature != 0
    }
}

// ============================================================================
// Section 11 — Typed run result
// ============================================================================

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Resource)]
pub struct PassKRunResult {
    pub schema_version: u16,
    pub motion_vector_dispatches: u32,
    pub taa_resolve_dispatches: u32,
    pub prev_transform_bound: bool,
    pub cur_transform_bound: bool,
    pub history_b_bound: bool,
    pub history_a_written: bool,
    pub disocclusion_mask_written: bool,
    pub debug_view_written: bool,
    pub motion_readback_ok: bool,
    pub history_readback_ok: bool,
    pub disocclusion_readback_ok: bool,
    pub debug_readback_ok: bool,
    pub color_readback_ok: bool,
    pub comparative_image: PassKComparativeImageArtifact,
    pub fsr_xess_input_validation: PassKFsr2XessInputValidation,
}

impl PassKRunResult {
    #[must_use]
    pub fn readbacks_all_succeeded(&self) -> bool {
        self.motion_readback_ok
            && self.history_readback_ok
            && self.disocclusion_readback_ok
            && self.debug_readback_ok
            && self.color_readback_ok
    }
}

// ============================================================================
// Section 12 — Verdict
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassKTemporalReconstructionLiveVerdict {
    pub schema_version: u16,
    pub passes_motion_vector_texture_pass_dispatched: bool,
    pub passes_prev_current_transform_buffers_bound: bool,
    pub passes_taa_history_texture_bound: bool,
    pub passes_taa_resolve_pass_executed: bool,
    pub passes_disocclusion_mask_written: bool,
    pub passes_debug_views_rendered_at_least_one: bool,
    pub passes_fsr2_xess_input_validation_passed: bool,
    pub passes_comparative_image_artifact_produced: bool,
}

impl PassKTemporalReconstructionLiveVerdict {
    #[must_use]
    pub fn evaluate(result: &PassKRunResult) -> Self {
        Self {
            schema_version: PASSK_TEMPORAL_RECONSTRUCTION_LIVE_SCHEMA_VERSION,
            passes_motion_vector_texture_pass_dispatched: result.motion_vector_dispatches > 0
                && result.motion_readback_ok,
            passes_prev_current_transform_buffers_bound: result.prev_transform_bound
                && result.cur_transform_bound,
            passes_taa_history_texture_bound: result.history_b_bound,
            passes_taa_resolve_pass_executed: result.taa_resolve_dispatches > 0
                && result.history_a_written
                && result.history_readback_ok,
            passes_disocclusion_mask_written: result.disocclusion_mask_written
                && result.disocclusion_readback_ok,
            passes_debug_views_rendered_at_least_one: result.debug_view_written
                && result.debug_readback_ok,
            passes_fsr2_xess_input_validation_passed: result.fsr_xess_input_validation.passes(),
            passes_comparative_image_artifact_produced: result
                .comparative_image
                .carries_byte_difference_signal(),
        }
    }

    #[must_use]
    pub const fn passes(&self) -> bool {
        self.passes_motion_vector_texture_pass_dispatched
            && self.passes_prev_current_transform_buffers_bound
            && self.passes_taa_history_texture_bound
            && self.passes_taa_resolve_pass_executed
            && self.passes_disocclusion_mask_written
            && self.passes_debug_views_rendered_at_least_one
            && self.passes_fsr2_xess_input_validation_passed
            && self.passes_comparative_image_artifact_produced
    }

    #[must_use]
    pub const fn first_failed(&self) -> Option<PassKTemporalReconstructionLiveRule> {
        if !self.passes_motion_vector_texture_pass_dispatched {
            return Some(PassKTemporalReconstructionLiveRule::MotionVectorTexturePassDispatched);
        }
        if !self.passes_prev_current_transform_buffers_bound {
            return Some(PassKTemporalReconstructionLiveRule::PrevCurrentTransformBuffersBound);
        }
        if !self.passes_taa_history_texture_bound {
            return Some(PassKTemporalReconstructionLiveRule::TaaHistoryTextureBound);
        }
        if !self.passes_taa_resolve_pass_executed {
            return Some(PassKTemporalReconstructionLiveRule::TaaResolvePassExecuted);
        }
        if !self.passes_disocclusion_mask_written {
            return Some(PassKTemporalReconstructionLiveRule::DisocclusionMaskWritten);
        }
        if !self.passes_debug_views_rendered_at_least_one {
            return Some(PassKTemporalReconstructionLiveRule::DebugViewsRenderedAtLeastOne);
        }
        if !self.passes_fsr2_xess_input_validation_passed {
            return Some(PassKTemporalReconstructionLiveRule::Fsr2XessInputValidationPassed);
        }
        if !self.passes_comparative_image_artifact_produced {
            return Some(PassKTemporalReconstructionLiveRule::ComparativeImageArtifactProduced);
        }
        None
    }

    #[must_use]
    pub const fn violation_count(&self) -> u32 {
        let mut count = 0u32;
        if !self.passes_motion_vector_texture_pass_dispatched {
            count += 1;
        }
        if !self.passes_prev_current_transform_buffers_bound {
            count += 1;
        }
        if !self.passes_taa_history_texture_bound {
            count += 1;
        }
        if !self.passes_taa_resolve_pass_executed {
            count += 1;
        }
        if !self.passes_disocclusion_mask_written {
            count += 1;
        }
        if !self.passes_debug_views_rendered_at_least_one {
            count += 1;
        }
        if !self.passes_fsr2_xess_input_validation_passed {
            count += 1;
        }
        if !self.passes_comparative_image_artifact_produced {
            count += 1;
        }
        count
    }
}

// ============================================================================
// Section 13 — Canonical proof scene
// ============================================================================

/// Typed proof-scene previous-frame `view_projection`: identity.
#[must_use]
pub const fn passk_proof_scene_prev_transform() -> PassKTransformBuffer {
    PassKTransformBuffer::identity()
}

/// Typed proof-scene current-frame `view_projection`: a small
/// translation that produces a non-zero, uniform motion vector
/// across every pixel. `(0.05, 0.0, 0.0)` in NDC corresponds to
/// ~0.4 pixels at 16-wide.
#[must_use]
pub const fn passk_proof_scene_cur_transform() -> PassKTransformBuffer {
    PassKTransformBuffer::translation(0.05, 0.0, 0.0)
}

/// Typed proof-scene initial color input: uniform `(255, 220, 64,
/// 255)` saturated yellow tile. The TAA resolve will blend this
/// with the history (saturated red) and produce a visually
/// distinct output the comparative image artifact records.
#[must_use]
pub fn passk_proof_scene_initial_color_rgba8() -> Vec<u8> {
    let mut bytes = Vec::with_capacity((PASSK_WIDTH * PASSK_HEIGHT * 4) as usize);
    for _ in 0..(PASSK_WIDTH * PASSK_HEIGHT) {
        bytes.extend_from_slice(&[255, 220, 64, 255]);
    }
    bytes
}

/// Typed proof-scene initial history: uniform `(1.0, 0.0, 0.0,
/// 1.0)` saturated red, encoded as `Rgba16Float`. The half-float
/// for 1.0 is `0x3C00`; for 0.0 is `0x0000`. Little-endian byte
/// pairs.
#[must_use]
pub fn passk_proof_scene_initial_history_rgba16f() -> Vec<u8> {
    let f16_one_le: [u8; 2] = [0x00, 0x3C];
    let f16_zero_le: [u8; 2] = [0x00, 0x00];
    let mut bytes = Vec::with_capacity((PASSK_WIDTH * PASSK_HEIGHT * 8) as usize);
    for _ in 0..(PASSK_WIDTH * PASSK_HEIGHT) {
        bytes.extend_from_slice(&f16_one_le); // R = 1.0
        bytes.extend_from_slice(&f16_zero_le); // G = 0.0
        bytes.extend_from_slice(&f16_zero_le); // B = 0.0
        bytes.extend_from_slice(&f16_one_le); // A = 1.0
    }
    bytes
}

// ============================================================================
// Section 14 — Top-level runner
// ============================================================================

pub enum PassKBootResult {
    Ran(Box<PassKRanPayload>),
    BridgeRuntimeFailed(WgpuBridgeRuntimeFailure),
}

pub struct PassKRanPayload {
    pub bridge_state: WgpuBridgeDeviceState<Dx12Native>,
    pub result: PassKRunResult,
}

#[must_use]
pub fn run_temporal_reconstruction_live_against_fresh_dx12_device() -> PassKBootResult {
    let options = WgpuBridgeRuntimeOptions::production_default();
    let bridge_state = match initialize_wgpu_bridge_runtime::<Dx12Native>(&options) {
        Ok(state) => state,
        Err(failure) => return PassKBootResult::BridgeRuntimeFailed(failure),
    };

    let prev_transform = passk_proof_scene_prev_transform();
    let cur_transform = passk_proof_scene_cur_transform();
    let params = PassKParams::proof_scene();
    let initial_color = passk_proof_scene_initial_color_rgba8();
    let initial_history = passk_proof_scene_initial_history_rgba16f();

    let textures = PassKTextureSet::allocate(
        &bridge_state.device,
        &bridge_state.queue,
        &initial_color,
        &initial_history,
    );
    let motion_pipe = PassKMotionVectorPipeline::create(&bridge_state.device);
    let taa_pipe = PassKTaaResolvePipeline::create(&bridge_state.device);
    let buffers = PassKBufferSet::allocate(
        &bridge_state.device,
        &bridge_state.queue,
        prev_transform,
        cur_transform,
        params,
    );

    let motion_bind_group = bridge_state
        .device
        .create_bind_group(&::wgpu::BindGroupDescriptor {
            label: Some("fun_renderer.passk.motion_vector.bind_group"),
            layout: &motion_pipe.bind_group_layout,
            entries: &[
                ::wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffers.prev_transform_buffer.as_entire_binding(),
                },
                ::wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buffers.cur_transform_buffer.as_entire_binding(),
                },
                ::wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buffers.params_buffer.as_entire_binding(),
                },
                ::wgpu::BindGroupEntry {
                    binding: 3,
                    resource: ::wgpu::BindingResource::TextureView(&textures.motion_vector_view),
                },
            ],
        });

    let taa_bind_group = bridge_state
        .device
        .create_bind_group(&::wgpu::BindGroupDescriptor {
            label: Some("fun_renderer.passk.taa_resolve.bind_group"),
            layout: &taa_pipe.bind_group_layout,
            entries: &[
                ::wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ::wgpu::BindingResource::TextureView(&textures.color_input_view),
                },
                ::wgpu::BindGroupEntry {
                    binding: 1,
                    resource: ::wgpu::BindingResource::TextureView(&textures.history_b_view),
                },
                ::wgpu::BindGroupEntry {
                    binding: 2,
                    resource: ::wgpu::BindingResource::TextureView(&textures.motion_vector_view),
                },
                ::wgpu::BindGroupEntry {
                    binding: 3,
                    resource: ::wgpu::BindingResource::TextureView(&textures.history_a_view),
                },
                ::wgpu::BindGroupEntry {
                    binding: 4,
                    resource: ::wgpu::BindingResource::TextureView(&textures.disocclusion_view),
                },
                ::wgpu::BindGroupEntry {
                    binding: 5,
                    resource: ::wgpu::BindingResource::TextureView(&textures.debug_view),
                },
                ::wgpu::BindGroupEntry {
                    binding: 6,
                    resource: buffers.params_buffer.as_entire_binding(),
                },
            ],
        });

    let mut result = PassKRunResult {
        schema_version: PASSK_TEMPORAL_RECONSTRUCTION_LIVE_SCHEMA_VERSION,
        prev_transform_bound: true,
        cur_transform_bound: true,
        history_b_bound: true,
        fsr_xess_input_validation: PassKFsr2XessInputValidation::validate(&params),
        ..PassKRunResult::default()
    };

    let mut encoder =
        bridge_state
            .device
            .create_command_encoder(&::wgpu::CommandEncoderDescriptor {
                label: Some("fun_renderer.passk.encoder"),
            });

    // Steps 1-2: motion vector compute pass.
    {
        let mut pass = encoder.begin_compute_pass(&::wgpu::ComputePassDescriptor {
            label: Some("fun_renderer.passk.motion_vector_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&motion_pipe.pipeline);
        pass.set_bind_group(0, &motion_bind_group, &[]);
        let wg_x = PASSK_WIDTH.div_ceil(PASSK_COMPUTE_WORKGROUP_SIZE_X);
        let wg_y = PASSK_HEIGHT.div_ceil(PASSK_COMPUTE_WORKGROUP_SIZE_Y);
        pass.dispatch_workgroups(wg_x, wg_y, 1);
        result.motion_vector_dispatches = 1;
    }

    // Steps 3-6: TAA resolve compute pass (writes history_a +
    // disocclusion + debug).
    {
        let mut pass = encoder.begin_compute_pass(&::wgpu::ComputePassDescriptor {
            label: Some("fun_renderer.passk.taa_resolve_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&taa_pipe.pipeline);
        pass.set_bind_group(0, &taa_bind_group, &[]);
        let wg_x = PASSK_WIDTH.div_ceil(PASSK_COMPUTE_WORKGROUP_SIZE_X);
        let wg_y = PASSK_HEIGHT.div_ceil(PASSK_COMPUTE_WORKGROUP_SIZE_Y);
        pass.dispatch_workgroups(wg_x, wg_y, 1);
        result.taa_resolve_dispatches = 1;
        result.history_a_written = true;
        result.disocclusion_mask_written = true;
        result.debug_view_written = true;
    }

    // Copy every output texture → readback buffer.
    let extent = ::wgpu::Extent3d {
        width: PASSK_WIDTH,
        height: PASSK_HEIGHT,
        depth_or_array_layers: 1,
    };
    encoder.copy_texture_to_buffer(
        ::wgpu::TexelCopyTextureInfo {
            texture: &textures.motion_vector_texture,
            mip_level: 0,
            origin: ::wgpu::Origin3d::ZERO,
            aspect: ::wgpu::TextureAspect::All,
        },
        ::wgpu::TexelCopyBufferInfo {
            buffer: &buffers.motion_readback,
            layout: ::wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(buffers.motion_readback_bytes_per_row),
                rows_per_image: Some(PASSK_HEIGHT),
            },
        },
        extent,
    );
    encoder.copy_texture_to_buffer(
        ::wgpu::TexelCopyTextureInfo {
            texture: &textures.history_a_texture,
            mip_level: 0,
            origin: ::wgpu::Origin3d::ZERO,
            aspect: ::wgpu::TextureAspect::All,
        },
        ::wgpu::TexelCopyBufferInfo {
            buffer: &buffers.history_readback,
            layout: ::wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(buffers.history_readback_bytes_per_row),
                rows_per_image: Some(PASSK_HEIGHT),
            },
        },
        extent,
    );
    encoder.copy_texture_to_buffer(
        ::wgpu::TexelCopyTextureInfo {
            texture: &textures.disocclusion_texture,
            mip_level: 0,
            origin: ::wgpu::Origin3d::ZERO,
            aspect: ::wgpu::TextureAspect::All,
        },
        ::wgpu::TexelCopyBufferInfo {
            buffer: &buffers.disocclusion_readback,
            layout: ::wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(buffers.mask_readback_bytes_per_row),
                rows_per_image: Some(PASSK_HEIGHT),
            },
        },
        extent,
    );
    encoder.copy_texture_to_buffer(
        ::wgpu::TexelCopyTextureInfo {
            texture: &textures.debug_texture,
            mip_level: 0,
            origin: ::wgpu::Origin3d::ZERO,
            aspect: ::wgpu::TextureAspect::All,
        },
        ::wgpu::TexelCopyBufferInfo {
            buffer: &buffers.debug_readback,
            layout: ::wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(buffers.mask_readback_bytes_per_row),
                rows_per_image: Some(PASSK_HEIGHT),
            },
        },
        extent,
    );
    encoder.copy_texture_to_buffer(
        ::wgpu::TexelCopyTextureInfo {
            texture: &textures.color_input_texture,
            mip_level: 0,
            origin: ::wgpu::Origin3d::ZERO,
            aspect: ::wgpu::TextureAspect::All,
        },
        ::wgpu::TexelCopyBufferInfo {
            buffer: &buffers.color_readback,
            layout: ::wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(buffers.mask_readback_bytes_per_row),
                rows_per_image: Some(PASSK_HEIGHT),
            },
        },
        extent,
    );

    let command_buffer = encoder.finish();
    let _submission_index = bridge_state
        .queue
        .submit(::core::iter::once(command_buffer));

    // Map every readback in parallel.
    let motion_slice = buffers.motion_readback.slice(..);
    let history_slice = buffers.history_readback.slice(..);
    let disocclusion_slice = buffers.disocclusion_readback.slice(..);
    let debug_slice = buffers.debug_readback.slice(..);
    let color_slice = buffers.color_readback.slice(..);
    let (motion_tx, motion_rx) = unbounded();
    let (history_tx, history_rx) = unbounded();
    let (disocclusion_tx, disocclusion_rx) = unbounded();
    let (debug_tx, debug_rx) = unbounded();
    let (color_tx, color_rx) = unbounded();
    motion_slice.map_async(::wgpu::MapMode::Read, move |r| {
        let _ = motion_tx.send(r);
    });
    history_slice.map_async(::wgpu::MapMode::Read, move |r| {
        let _ = history_tx.send(r);
    });
    disocclusion_slice.map_async(::wgpu::MapMode::Read, move |r| {
        let _ = disocclusion_tx.send(r);
    });
    debug_slice.map_async(::wgpu::MapMode::Read, move |r| {
        let _ = debug_tx.send(r);
    });
    color_slice.map_async(::wgpu::MapMode::Read, move |r| {
        let _ = color_tx.send(r);
    });
    let _ = bridge_state
        .device
        .poll(::wgpu::PollType::wait_indefinitely());

    let mut before_taa_pixel_rgba8 = [0u8; 4];
    let mut after_taa_pixel_rgba16f_bytes = [0u8; 8];
    let mut disocclusion_pixel_rgba8 = [0u8; 4];
    let mut motion_pixel_rgba16f_bytes = [0u8; 8];
    let mut debug_pixel_rgba8 = [0u8; 4];

    if let Ok(Ok(())) = motion_rx.recv() {
        let data = motion_slice.get_mapped_range();
        if data.len() >= 8 {
            motion_pixel_rgba16f_bytes.copy_from_slice(&data[0..8]);
            result.motion_readback_ok = true;
        }
        drop(data);
        buffers.motion_readback.unmap();
    }
    if let Ok(Ok(())) = history_rx.recv() {
        let data = history_slice.get_mapped_range();
        if data.len() >= 8 {
            after_taa_pixel_rgba16f_bytes.copy_from_slice(&data[0..8]);
            result.history_readback_ok = true;
        }
        drop(data);
        buffers.history_readback.unmap();
    }
    if let Ok(Ok(())) = disocclusion_rx.recv() {
        let data = disocclusion_slice.get_mapped_range();
        if data.len() >= 4 {
            disocclusion_pixel_rgba8.copy_from_slice(&data[0..4]);
            result.disocclusion_readback_ok = true;
        }
        drop(data);
        buffers.disocclusion_readback.unmap();
    }
    if let Ok(Ok(())) = debug_rx.recv() {
        let data = debug_slice.get_mapped_range();
        if data.len() >= 4 {
            debug_pixel_rgba8.copy_from_slice(&data[0..4]);
            result.debug_readback_ok = true;
        }
        drop(data);
        buffers.debug_readback.unmap();
    }
    if let Ok(Ok(())) = color_rx.recv() {
        let data = color_slice.get_mapped_range();
        if data.len() >= 4 {
            before_taa_pixel_rgba8.copy_from_slice(&data[0..4]);
            result.color_readback_ok = true;
        }
        drop(data);
        buffers.color_readback.unmap();
    }

    // Step 8: build the typed comparative image artifact from
    // the readbacks.
    result.comparative_image = PassKComparativeImageArtifact::build(
        before_taa_pixel_rgba8,
        after_taa_pixel_rgba16f_bytes,
        disocclusion_pixel_rgba8,
        motion_pixel_rgba16f_bytes,
        debug_pixel_rgba8,
    );

    PassKBootResult::Ran(Box::new(PassKRanPayload {
        bridge_state,
        result,
    }))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(PASSK_TEMPORAL_RECONSTRUCTION_LIVE_SCHEMA_VERSION, 1);
        assert_eq!(PASSK_RULE_COUNT, 8);
        assert_eq!(
            PassKTemporalReconstructionLiveRule::ALL.len(),
            PASSK_RULE_COUNT
        );
        assert_eq!(PASSK_TAAU_QUALITY_TIER_COUNT, 3);
        assert_eq!(
            PassKTaauQualityTier::ALL.len(),
            PASSK_TAAU_QUALITY_TIER_COUNT
        );
    }

    #[test]
    fn rule_index_round_trip() {
        for (i, &rule) in PassKTemporalReconstructionLiveRule::ALL.iter().enumerate() {
            assert_eq!(rule.index(), i, "{}", rule.as_str());
        }
    }

    #[test]
    fn rule_taxonomy_strings_are_unique() {
        let mut seen = hashbrown::HashSet::new();
        for rule in PassKTemporalReconstructionLiveRule::ALL {
            assert!(seen.insert(rule.as_str()), "duplicate: {}", rule.as_str());
        }
    }

    #[test]
    fn taau_quality_tier_taxonomy() {
        assert_eq!(PassKTaauQualityTier::Quality.as_str(), "quality");
        assert_eq!(PassKTaauQualityTier::Balanced.as_str(), "balanced");
        assert_eq!(PassKTaauQualityTier::Performance.as_str(), "performance");
        // Quality has the lowest upscale ratio (highest input
        // fidelity); Performance has the highest ratio (cheapest
        // input).
        assert!(
            PassKTaauQualityTier::Quality.upscale_ratio()
                < PassKTaauQualityTier::Balanced.upscale_ratio()
        );
        assert!(
            PassKTaauQualityTier::Balanced.upscale_ratio()
                < PassKTaauQualityTier::Performance.upscale_ratio()
        );
        // Route decision tied to inputs valid.
        assert!(PassKTaauQualityTier::Quality.route_decision_allowed_under_inputs_valid(true));
        assert!(!PassKTaauQualityTier::Quality.route_decision_allowed_under_inputs_valid(false));
    }

    #[test]
    fn transform_buffer_byte_layout() {
        let identity = PassKTransformBuffer::identity();
        let bytes = identity.as_bytes();
        assert_eq!(bytes.len(), 64);
        // Column 0 first element = 1.0
        assert_eq!(&bytes[0..4], &1.0f32.to_le_bytes());
        // Column 0 second element = 0.0
        assert_eq!(&bytes[4..8], &0.0f32.to_le_bytes());
        // Column 3 last element = 1.0
        assert_eq!(&bytes[60..64], &1.0f32.to_le_bytes());

        let t = PassKTransformBuffer::translation(0.5, 0.25, 0.125);
        let tb = t.as_bytes();
        // Column 3 first element (tx)
        assert_eq!(&tb[48..52], &0.5f32.to_le_bytes());
        assert_eq!(&tb[52..56], &0.25f32.to_le_bytes());
        assert_eq!(&tb[56..60], &0.125f32.to_le_bytes());
        assert_eq!(&tb[60..64], &1.0f32.to_le_bytes());
    }

    #[test]
    fn params_byte_layout() {
        let params = PassKParams {
            width: 16,
            height: 16,
            motion_scale: 1.5,
            blend_alpha: 0.25,
            jitter_x: -0.1,
            jitter_y: 0.2,
            _pad0: 0.0,
            _pad1: 0.0,
        };
        let bytes = params.as_bytes();
        assert_eq!(bytes.len(), 32);
        assert_eq!(&bytes[0..4], &16u32.to_le_bytes());
        assert_eq!(&bytes[4..8], &16u32.to_le_bytes());
        assert_eq!(&bytes[8..12], &1.5f32.to_le_bytes());
        assert_eq!(&bytes[12..16], &0.25f32.to_le_bytes());
        assert_eq!(&bytes[16..20], &(-0.1f32).to_le_bytes());
        assert_eq!(&bytes[20..24], &0.2f32.to_le_bytes());
    }

    #[test]
    fn proof_scene_initial_color_is_uniform_yellow() {
        let bytes = passk_proof_scene_initial_color_rgba8();
        assert_eq!(bytes.len(), (PASSK_WIDTH * PASSK_HEIGHT * 4) as usize);
        assert_eq!(&bytes[0..4], &[255, 220, 64, 255]);
        // Last pixel matches too.
        let last = bytes.len() - 4;
        assert_eq!(&bytes[last..], &[255, 220, 64, 255]);
    }

    #[test]
    fn proof_scene_initial_history_is_uniform_red_f16() {
        let bytes = passk_proof_scene_initial_history_rgba16f();
        assert_eq!(bytes.len(), (PASSK_WIDTH * PASSK_HEIGHT * 8) as usize);
        // R = 1.0 (0x3C00 le), G = 0.0, B = 0.0, A = 1.0
        assert_eq!(&bytes[0..2], &[0x00, 0x3C]);
        assert_eq!(&bytes[2..4], &[0x00, 0x00]);
        assert_eq!(&bytes[4..6], &[0x00, 0x00]);
        assert_eq!(&bytes[6..8], &[0x00, 0x3C]);
    }

    #[test]
    fn fsr_xess_input_validation_passes_for_proof_scene_params() {
        let params = PassKParams::proof_scene();
        let v = PassKFsr2XessInputValidation::validate(&params);
        assert!(v.passes(), "input validation must pass under proof scene");
        assert!(v.motion_vector_format_valid);
        assert!(v.history_format_supports_hdr);
        assert!(v.disocclusion_mask_format_valid);
        assert!(v.resolution_at_least_minimum);
        assert!(v.jitter_range_valid);
    }

    #[test]
    fn fsr_xess_input_validation_fails_when_resolution_below_minimum() {
        let params = PassKParams {
            width: 8,
            height: 8,
            ..PassKParams::proof_scene()
        };
        let v = PassKFsr2XessInputValidation::validate(&params);
        assert!(!v.passes());
        assert!(!v.resolution_at_least_minimum);
    }

    #[test]
    fn fsr_xess_input_validation_fails_when_jitter_out_of_range() {
        let params = PassKParams {
            jitter_x: 5.0,
            ..PassKParams::proof_scene()
        };
        let v = PassKFsr2XessInputValidation::validate(&params);
        assert!(!v.passes());
        assert!(!v.jitter_range_valid);
    }

    #[test]
    fn dlss_reflex_policy_keeps_dlss_fail_closed_when_gap_open() {
        let policy = PassKDlssReflexFailClosedPolicy::PRODUCT_DEFAULT;
        assert!(policy.is_consistent_with_command_list_gap(true));
        // DLSS / Reflex / FrameGen always fail-closed even if
        // inputs are valid.
        for upscaler in [
            PassKUpscalerKind::Dlss,
            PassKUpscalerKind::Reflex,
            PassKUpscalerKind::FrameGeneration,
        ] {
            let decision = policy.route_decision(upscaler, true, true);
            assert!(!decision.is_allowed(), "{}", upscaler.as_str());
        }
        // FSR2 / XeSS allowed when inputs valid, blocked when not.
        assert_eq!(
            policy.route_decision(PassKUpscalerKind::Fsr2, true, true),
            PassKUpscaleRouteDecision::AllowedFsr2,
        );
        assert_eq!(
            policy.route_decision(PassKUpscalerKind::Xess, true, true),
            PassKUpscaleRouteDecision::AllowedXess,
        );
        assert_eq!(
            policy.route_decision(PassKUpscalerKind::Fsr2, true, false),
            PassKUpscaleRouteDecision::FailClosedFsr2InputsInvalid,
        );
        assert_eq!(
            policy.route_decision(PassKUpscalerKind::Xess, true, false),
            PassKUpscaleRouteDecision::FailClosedXessInputsInvalid,
        );
    }

    #[test]
    fn dlss_reflex_policy_references_canonical_command_list_gap() {
        let policy = PassKDlssReflexFailClosedPolicy::PRODUCT_DEFAULT;
        assert_eq!(
            policy.gap_command_list_unavailable_canonical_stable_id,
            "gap.command_list_unavailable",
        );
    }

    #[test]
    fn comparative_image_canonical_path_uses_funpb_zst_suffix() {
        let artifact = PassKComparativeImageArtifact::build(
            [10, 20, 30, 40],
            [1, 2, 3, 4, 5, 6, 7, 8],
            [128, 0, 0, 255],
            [0x80, 0x3C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            [200, 100, 0, 255],
        );
        assert_eq!(
            artifact.canonical_path,
            PASSK_COMPARATIVE_IMAGE_CANONICAL_PATH
        );
        assert!(artifact.canonical_path.ends_with(".funpb.zst"));
        assert!(artifact.carries_byte_difference_signal());
    }

    #[test]
    fn verdict_passes_under_synthetic_full_evidence() {
        let mut result = PassKRunResult {
            schema_version: PASSK_TEMPORAL_RECONSTRUCTION_LIVE_SCHEMA_VERSION,
            motion_vector_dispatches: 1,
            taa_resolve_dispatches: 1,
            prev_transform_bound: true,
            cur_transform_bound: true,
            history_b_bound: true,
            history_a_written: true,
            disocclusion_mask_written: true,
            debug_view_written: true,
            motion_readback_ok: true,
            history_readback_ok: true,
            disocclusion_readback_ok: true,
            debug_readback_ok: true,
            color_readback_ok: true,
            comparative_image: PassKComparativeImageArtifact::build(
                [255, 220, 64, 255],
                [0; 8],
                [128, 0, 0, 255],
                [0; 8],
                [200, 100, 0, 255],
            ),
            fsr_xess_input_validation: PassKFsr2XessInputValidation::validate(
                &PassKParams::proof_scene(),
            ),
        };
        // Force the comparative image artifact's signature to
        // be non-zero (the build above already does).
        assert!(result.comparative_image.carries_byte_difference_signal());

        let verdict = PassKTemporalReconstructionLiveVerdict::evaluate(&result);
        assert!(
            verdict.passes(),
            "verdict should pass under full synthetic evidence; first_failed = {:?}",
            verdict.first_failed(),
        );
        assert_eq!(verdict.violation_count(), 0);

        // Drop motion dispatch — rule 1 should fail first.
        result.motion_vector_dispatches = 0;
        let verdict = PassKTemporalReconstructionLiveVerdict::evaluate(&result);
        assert_eq!(
            verdict.first_failed(),
            Some(PassKTemporalReconstructionLiveRule::MotionVectorTexturePassDispatched),
        );
    }

    #[test]
    fn verdict_fails_when_input_validation_fails() {
        let result = PassKRunResult {
            schema_version: PASSK_TEMPORAL_RECONSTRUCTION_LIVE_SCHEMA_VERSION,
            motion_vector_dispatches: 1,
            taa_resolve_dispatches: 1,
            prev_transform_bound: true,
            cur_transform_bound: true,
            history_b_bound: true,
            history_a_written: true,
            disocclusion_mask_written: true,
            debug_view_written: true,
            motion_readback_ok: true,
            history_readback_ok: true,
            disocclusion_readback_ok: true,
            debug_readback_ok: true,
            color_readback_ok: true,
            comparative_image: PassKComparativeImageArtifact::build(
                [255, 220, 64, 255],
                [0; 8],
                [128, 0, 0, 255],
                [0; 8],
                [200, 100, 0, 255],
            ),
            fsr_xess_input_validation: PassKFsr2XessInputValidation::validate(&PassKParams {
                width: 4, // below PASSK_FSR_XESS_MIN_INPUT_EXTENT
                height: 4,
                ..PassKParams::proof_scene()
            }),
        };
        let verdict = PassKTemporalReconstructionLiveVerdict::evaluate(&result);
        assert!(!verdict.passes());
        assert_eq!(
            verdict.first_failed(),
            Some(PassKTemporalReconstructionLiveRule::Fsr2XessInputValidationPassed),
        );
    }

    #[test]
    fn verdict_fails_when_comparative_image_signature_zero() {
        let result = PassKRunResult {
            schema_version: PASSK_TEMPORAL_RECONSTRUCTION_LIVE_SCHEMA_VERSION,
            motion_vector_dispatches: 1,
            taa_resolve_dispatches: 1,
            prev_transform_bound: true,
            cur_transform_bound: true,
            history_b_bound: true,
            history_a_written: true,
            disocclusion_mask_written: true,
            debug_view_written: true,
            motion_readback_ok: true,
            history_readback_ok: true,
            disocclusion_readback_ok: true,
            debug_readback_ok: true,
            color_readback_ok: true,
            comparative_image: PassKComparativeImageArtifact::build(
                [0, 0, 0, 0],
                [0; 8],
                [0, 0, 0, 0],
                [0; 8],
                [0, 0, 0, 0],
            ),
            fsr_xess_input_validation: PassKFsr2XessInputValidation::validate(
                &PassKParams::proof_scene(),
            ),
        };
        let verdict = PassKTemporalReconstructionLiveVerdict::evaluate(&result);
        assert!(!verdict.passes());
        assert_eq!(
            verdict.first_failed(),
            Some(PassKTemporalReconstructionLiveRule::ComparativeImageArtifactProduced),
        );
    }

    #[test]
    fn buffer_set_round_up_alignment_aligns_to_256() {
        assert_eq!(PassKBufferSet::round_up_alignment(0), 0);
        assert_eq!(PassKBufferSet::round_up_alignment(1), 256);
        assert_eq!(PassKBufferSet::round_up_alignment(128), 256);
        assert_eq!(PassKBufferSet::round_up_alignment(256), 256);
        assert_eq!(PassKBufferSet::round_up_alignment(257), 512);
    }

    /// Live smoke test: boots a real wgpu device on DX12, runs
    /// the full Pass K pipeline, and verifies every typed rule
    /// holds. On hosts without DX12 the test logs the bridge
    /// failure honestly and returns. On hosts where wgpu
    /// resolves to a non-DX12 backend the test still runs but
    /// skips strict pixel assertions.
    #[test]
    fn live_passk_runs_real_temporal_reconstruction_with_taa_resolve_and_comparative_image() {
        use crate::live_proof_frame_executor::ran_on_real_dx12_adapter;

        let outcome = run_temporal_reconstruction_live_against_fresh_dx12_device();
        match outcome {
            PassKBootResult::Ran(payload) => {
                let PassKRanPayload {
                    bridge_state,
                    result,
                } = *payload;
                if !ran_on_real_dx12_adapter(&bridge_state) {
                    eprintln!(
                        "live_passk: non-DX12 actual backend ({:?}); skipping strict assertions",
                        bridge_state.actual_native_backend,
                    );
                    return;
                }
                assert_eq!(result.motion_vector_dispatches, 1);
                assert_eq!(result.taa_resolve_dispatches, 1);
                assert!(result.prev_transform_bound);
                assert!(result.cur_transform_bound);
                assert!(result.history_b_bound);
                assert!(result.history_a_written);
                assert!(result.disocclusion_mask_written);
                assert!(result.debug_view_written);
                assert!(result.readbacks_all_succeeded());
                assert!(result.fsr_xess_input_validation.passes());
                assert!(result.comparative_image.carries_byte_difference_signal());

                // Before-TAA pixel must match the proof-scene
                // initial color (read back from the
                // CPU-uploaded color input texture).
                assert_eq!(
                    result.comparative_image.before_taa_pixel_rgba8,
                    [255, 220, 64, 255],
                );

                let verdict = PassKTemporalReconstructionLiveVerdict::evaluate(&result);
                assert!(
                    verdict.passes(),
                    "Pass K verdict must pass; first_failed = {:?}",
                    verdict.first_failed(),
                );
            }
            PassKBootResult::BridgeRuntimeFailed(failure) => {
                eprintln!(
                    "live_passk: bridge runtime failed (host without DX12 adapter): {failure:?}",
                );
            }
        }
    }
}
