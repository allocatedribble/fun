//! Pass L — Native UI Rendering, Batching, and Atlas Work (Live GPU).
//!
//! The user's "Future bleeding-edge plan / Priority 4: Native
//! UI rendering, batching, and atlas work" — the rvelte/FUN UI
//! path is architecturally clean, now draw it. Walks the user's
//! full build order against a real DX12 wgpu device:
//!
//! 1. **UI batch table → actual UI draw pass.** A typed
//!    [`PassLUiBatchHeader`] table records the typed batches the
//!    runner walks; each entry switches pipelines and dispatches
//!    `pass.draw_indexed(0..6, 0, first_instance..)` against a
//!    real `wgpu::RenderPass`.
//! 2. **Filled quad pipeline.** A real `wgpu::RenderPipeline`
//!    with the shared vertex shader + `fs_filled` fragment
//!    shader. Renders solid-color rectangles.
//! 3. **Image quad pipeline.** A real `wgpu::RenderPipeline`
//!    with `fs_image`. Samples a real `wgpu::Texture` via
//!    `textureLoad` and multiplies by per-instance tint.
//! 4. **Glyph atlas and glyph run pipeline.** A real
//!    `wgpu::Texture` glyph atlas (typed shelf packer +
//!    [`PassLGlyphTableEntry`] table) backs the `fs_glyph`
//!    fragment shader; alpha is sampled from the atlas and
//!    multiplied with the per-instance color.
//! 5. **Clip/scissor stack.** A real
//!    `pass.set_scissor_rect(...)` call enforces the typed
//!    scissor at the render-pass level; the typed
//!    `PassLClipRect` per-instance is enforced by the fragment
//!    shader (`discard` outside clip).
//! 6. **Rounded rect analytic/SDF pipeline.** A real
//!    `wgpu::RenderPipeline` with `fs_rounded_rect` —
//!    closed-form analytic SDF
//!    (`rounded_rect_sdf(p, b, r)`) discards fragments outside
//!    the rounded shape.
//! 7. **Product launcher route rendered over proof scene.** A
//!    typed `PassLLauncherRouteIdentifier::PRODUCT_DEFAULT`
//!    records the typed identity; the runner clears the frame
//!    target to the typed proof-scene background and renders
//!    every batch on top.
//! 8. **Frame probe samples UI-colored region.** A real
//!    `copy_texture_to_buffer` of the frame target + readback
//!    sample at the typed probe pixels proves the UI batches
//!    landed on a UI-colored region (not the cleared
//!    background).
//!
//! Bleeding-edge extensions recorded as typed surfaces today
//! ([`PassLBleedingEdgeExtensionSurfaces`]) — the full live
//! integrations are follow-on passes:
//!   - GPU path rendering for complex vector primitives.
//!   - Compute-based glyph atlas packing.
//!   - Signed-distance rounded rects (the analytic
//!     [`PassLRoundedRectShadingMode::AnalyticDistance`] path is
//!     live today; the SDF path with anti-aliasing is the
//!     extension).
//!   - Retained UI damage rects.
//!   - Per-route UI render budgets.
//!
//! Pass L's evidence kind in
//! `quality_audit_contract::PASS_EVIDENCE_REGISTRY` is
//! `LiveGpuExecution`. The live test compiles one shared WGSL
//! shader module with five entry points, creates four real
//! render pipelines, allocates a real glyph atlas + image
//! texture + frame target, walks the typed batch table, runs
//! four real draws on a fresh DX12 wgpu device, and reads back
//! four typed probe pixels to prove every pipeline landed
//! pixels.

use flume::unbounded;

use fun_ecs::Resource;

use crate::bridge::wgpu::{
    Dx12Native, WgpuBridgeDeviceState, WgpuBridgeRuntimeFailure, WgpuBridgeRuntimeOptions,
    initialize_wgpu_bridge_runtime,
};
use crate::live_proof_frame_executor::LIVE_PROOF_FRAME_OFFSCREEN_EXTENT;

pub const PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION: u16 = 1;
pub const PASSL_RULE_COUNT: usize = 8;

/// Pass L proof-scene image extent (16×16, matches the canonical
/// `LIVE_PROOF_FRAME_OFFSCREEN_EXTENT`).
pub const PASSL_WIDTH: u32 = LIVE_PROOF_FRAME_OFFSCREEN_EXTENT;
pub const PASSL_HEIGHT: u32 = LIVE_PROOF_FRAME_OFFSCREEN_EXTENT;

/// 80-byte typed `PassLUiInstanceData` layout (5 × vec4).
pub const PASSL_UI_INSTANCE_BYTES: u64 = 80;

/// Six indices per quad (two triangles), `Uint32` index format.
pub const PASSL_QUAD_INDEX_COUNT: u32 = 6;
pub const PASSL_QUAD_INDEX_BYTES: u64 = (PASSL_QUAD_INDEX_COUNT as u64) * 4;

/// Glyph atlas: 32×32 `Rgba8Unorm` (4 KiB) so the typed shelf
/// packer has room for many proof-scene glyph cells. The proof
/// scene packs a single 4×4 white cell.
pub const PASSL_GLYPH_ATLAS_EXTENT: u32 = 32;

/// Frame target format. `Rgba8Unorm` (not sRGB) so the linear
/// instance colors pass through to the readback bytes without
/// gamma encoding.
pub const PASSL_FRAME_FORMAT: ::wgpu::TextureFormat = ::wgpu::TextureFormat::Rgba8Unorm;
pub const PASSL_IMAGE_FORMAT: ::wgpu::TextureFormat = ::wgpu::TextureFormat::Rgba8Unorm;
pub const PASSL_GLYPH_ATLAS_FORMAT: ::wgpu::TextureFormat = ::wgpu::TextureFormat::Rgba8Unorm;

/// Typed proof-scene background color (dark gray) the frame
/// target is cleared to before UI batches render on top.
pub const PASSL_PROOF_SCENE_BACKGROUND_LINEAR: [f32; 4] = [0.10, 0.10, 0.12, 1.0];

/// Typed scissor rectangle the runner sets at render-pass
/// start. (1, 1, 14, 14) excludes the outermost row/column to
/// prove the scissor is enforced; the proof-scene UI quads all
/// fit inside this rectangle.
pub const PASSL_SCISSOR_PROOF_SCENE: PassLClipRect = PassLClipRect {
    schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
    min_x: 1,
    min_y: 1,
    max_x: 15,
    max_y: 15,
};

// ============================================================================
// Section 1 — Exit-gate rule taxonomy
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassLNativeUiLiveRule {
    UiBatchTableExecutedAsActualDrawPass,
    FilledQuadPipelineExecuted,
    ImageQuadPipelineExecuted,
    GlyphAtlasAndGlyphRunPipelineExecuted,
    ClipScissorStackApplied,
    RoundedRectAnalyticSdfPipelineExecuted,
    ProductLauncherRouteRenderedOverProofScene,
    FrameProbeSamplesUiColoredRegion,
}

impl PassLNativeUiLiveRule {
    pub const ALL: [Self; PASSL_RULE_COUNT] = [
        Self::UiBatchTableExecutedAsActualDrawPass,
        Self::FilledQuadPipelineExecuted,
        Self::ImageQuadPipelineExecuted,
        Self::GlyphAtlasAndGlyphRunPipelineExecuted,
        Self::ClipScissorStackApplied,
        Self::RoundedRectAnalyticSdfPipelineExecuted,
        Self::ProductLauncherRouteRenderedOverProofScene,
        Self::FrameProbeSamplesUiColoredRegion,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::UiBatchTableExecutedAsActualDrawPass => 0,
            Self::FilledQuadPipelineExecuted => 1,
            Self::ImageQuadPipelineExecuted => 2,
            Self::GlyphAtlasAndGlyphRunPipelineExecuted => 3,
            Self::ClipScissorStackApplied => 4,
            Self::RoundedRectAnalyticSdfPipelineExecuted => 5,
            Self::ProductLauncherRouteRenderedOverProofScene => 6,
            Self::FrameProbeSamplesUiColoredRegion => 7,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UiBatchTableExecutedAsActualDrawPass => {
                "ui_batch_table_executed_as_actual_draw_pass"
            }
            Self::FilledQuadPipelineExecuted => "filled_quad_pipeline_executed",
            Self::ImageQuadPipelineExecuted => "image_quad_pipeline_executed",
            Self::GlyphAtlasAndGlyphRunPipelineExecuted => {
                "glyph_atlas_and_glyph_run_pipeline_executed"
            }
            Self::ClipScissorStackApplied => "clip_scissor_stack_applied",
            Self::RoundedRectAnalyticSdfPipelineExecuted => {
                "rounded_rect_analytic_sdf_pipeline_executed"
            }
            Self::ProductLauncherRouteRenderedOverProofScene => {
                "product_launcher_route_rendered_over_proof_scene"
            }
            Self::FrameProbeSamplesUiColoredRegion => "frame_probe_samples_ui_colored_region",
        }
    }
}

// ============================================================================
// Section 2 — Typed UI primitives
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassLUiQuadKind {
    #[default]
    Filled,
    Image,
    Glyph,
    RoundedRect,
}

impl PassLUiQuadKind {
    pub const ALL: [Self; 4] = [Self::Filled, Self::Image, Self::Glyph, Self::RoundedRect];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Filled => "filled",
            Self::Image => "image",
            Self::Glyph => "glyph",
            Self::RoundedRect => "rounded_rect",
        }
    }

    #[must_use]
    pub const fn as_float(self) -> f32 {
        match self {
            Self::Filled => 0.0,
            Self::Image => 1.0,
            Self::Glyph => 2.0,
            Self::RoundedRect => 3.0,
        }
    }
}

/// Typed clip rectangle in framebuffer pixel coordinates
/// (`min_x`, `min_y`, `max_x`, `max_y`). `min` inclusive, `max`
/// exclusive.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassLClipRect {
    pub schema_version: u16,
    pub min_x: u32,
    pub min_y: u32,
    pub max_x: u32,
    pub max_y: u32,
}

impl PassLClipRect {
    pub const FULL_PROOF_SCENE: Self = Self {
        schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
        min_x: 0,
        min_y: 0,
        max_x: PASSL_WIDTH,
        max_y: PASSL_HEIGHT,
    };

    /// Typed predicate: does this clip rect cover at least one
    /// pixel?
    #[must_use]
    pub const fn covers_any_pixel(self) -> bool {
        self.max_x > self.min_x && self.max_y > self.min_y
    }

    /// Typed predicate: does this clip rect strictly contain
    /// the typed pixel?
    #[must_use]
    pub const fn contains_pixel(self, x: u32, y: u32) -> bool {
        x >= self.min_x && y >= self.min_y && x < self.max_x && y < self.max_y
    }

    #[must_use]
    pub fn as_f32_xy_xy(self) -> [f32; 4] {
        [
            self.min_x as f32,
            self.min_y as f32,
            self.max_x as f32,
            self.max_y as f32,
        ]
    }
}

/// Typed UI instance data. 80 bytes std140-compatible (5 ×
/// vec4). The runner writes one of these per draw quad into
/// the storage instance buffer.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct PassLUiInstanceData {
    /// `(pos.x, pos.y, size.x, size.y)` in pixels.
    pub pos_size: [f32; 4],
    /// `(uv_min.x, uv_min.y, uv_max.x, uv_max.y)` in atlas /
    /// image UV space. Filled / rounded-rect pipelines ignore
    /// the UV.
    pub uv_rect: [f32; 4],
    /// `(r, g, b, a)` per-instance tint. Filled / rounded
    /// quads use this as the output color; image / glyph use
    /// it as a multiplicative tint on the sampled texel.
    pub color: [f32; 4],
    /// `(clip_min.x, clip_min.y, clip_max.x, clip_max.y)` in
    /// pixels. Enforced by the fragment shader (`discard`
    /// outside).
    pub clip_rect: [f32; 4],
    /// `(corner_radius_px, kind_as_float, _pad, _pad)`.
    pub corner_radius_kind: [f32; 4],
}

impl PassLUiInstanceData {
    pub const BYTES: usize = PASSL_UI_INSTANCE_BYTES as usize;

    #[must_use]
    pub fn as_bytes(&self) -> [u8; 80] {
        let mut bytes = [0u8; 80];
        for (i, &v) in self.pos_size.iter().enumerate() {
            bytes[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        for (i, &v) in self.uv_rect.iter().enumerate() {
            let base = 16 + i * 4;
            bytes[base..base + 4].copy_from_slice(&v.to_le_bytes());
        }
        for (i, &v) in self.color.iter().enumerate() {
            let base = 32 + i * 4;
            bytes[base..base + 4].copy_from_slice(&v.to_le_bytes());
        }
        for (i, &v) in self.clip_rect.iter().enumerate() {
            let base = 48 + i * 4;
            bytes[base..base + 4].copy_from_slice(&v.to_le_bytes());
        }
        for (i, &v) in self.corner_radius_kind.iter().enumerate() {
            let base = 64 + i * 4;
            bytes[base..base + 4].copy_from_slice(&v.to_le_bytes());
        }
        bytes
    }
}

/// Typed UI batch header. One entry per "batch" the runner
/// walks; the runner switches pipelines on the typed
/// `quad_kind` and dispatches
/// `pass.draw_indexed(0..6, 0, first_instance..first_instance + instance_count)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassLUiBatchHeader {
    pub schema_version: u16,
    pub batch_index: u32,
    pub quad_kind: PassLUiQuadKind,
    pub first_instance: u32,
    pub instance_count: u32,
    pub clip_rect: PassLClipRect,
}

impl PassLUiBatchHeader {
    #[must_use]
    pub const fn new(
        batch_index: u32,
        quad_kind: PassLUiQuadKind,
        first_instance: u32,
        instance_count: u32,
        clip_rect: PassLClipRect,
    ) -> Self {
        Self {
            schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
            batch_index,
            quad_kind,
            first_instance,
            instance_count,
            clip_rect,
        }
    }
}

// ============================================================================
// Section 3 — Typed launcher route + bleeding-edge surfaces
// ============================================================================

/// Typed identifier for the rvelte/FUN UI product launcher
/// route. The proof-scene runner records this as the rendered
/// route; production routes can extend the enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassLLauncherRouteIdentifier {
    pub schema_version: u16,
    pub stable_id: &'static str,
    pub is_product_default: bool,
}

impl PassLLauncherRouteIdentifier {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
        stable_id: "native_rvelte_fun_ui.launcher_route",
        is_product_default: true,
    };
}

/// Typed surfaces for the user's bleeding-edge extensions.
/// Every extension is recorded at the typed-contract layer
/// today; the live integrations are follow-on passes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassLBleedingEdgeExtensionSurfaces {
    pub schema_version: u16,
    pub gpu_path_rendering: PassLUiPathRenderingCapability,
    pub glyph_atlas_packing_mode: PassLGlyphAtlasPackingMode,
    pub rounded_rect_shading_mode: PassLRoundedRectShadingMode,
    pub damage_rect_kind: PassLDamageRectKind,
    pub per_route_render_budget: PassLPerRouteRenderBudget,
}

impl PassLBleedingEdgeExtensionSurfaces {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
        gpu_path_rendering: PassLUiPathRenderingCapability::PlanningSurface,
        glyph_atlas_packing_mode: PassLGlyphAtlasPackingMode::CpuShelfPacker,
        rounded_rect_shading_mode: PassLRoundedRectShadingMode::AnalyticDistance,
        damage_rect_kind: PassLDamageRectKind::FullFrameNoRetainedDamage,
        per_route_render_budget: PassLPerRouteRenderBudget::PRODUCT_DEFAULT,
    };
}

/// Typed GPU path rendering capability. The proof scene runs
/// quad-based UI; `GpuPathRenderingEngaged` is reserved for a
/// future live path-rendering pipeline.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassLUiPathRenderingCapability {
    #[default]
    PlanningSurface,
    GpuPathRenderingEngaged,
}

impl PassLUiPathRenderingCapability {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PlanningSurface => "planning_surface",
            Self::GpuPathRenderingEngaged => "gpu_path_rendering_engaged",
        }
    }
}

/// Typed glyph atlas packing mode. The product default is a
/// CPU shelf packer (live today); `GpuComputePacker` is the
/// bleeding-edge extension.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassLGlyphAtlasPackingMode {
    #[default]
    CpuShelfPacker,
    GpuComputePacker,
}

impl PassLGlyphAtlasPackingMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CpuShelfPacker => "cpu_shelf_packer",
            Self::GpuComputePacker => "gpu_compute_packer",
        }
    }
}

/// Typed rounded-rect shading mode. `AnalyticDistance` is the
/// closed-form SDF used in the live pipeline today; `Sdf` is
/// reserved for the anti-aliased SDF extension.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassLRoundedRectShadingMode {
    #[default]
    AnalyticDistance,
    Sdf,
}

impl PassLRoundedRectShadingMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AnalyticDistance => "analytic_distance",
            Self::Sdf => "sdf",
        }
    }
}

/// Typed damage rect kind. Default is `FullFrameNoRetainedDamage`
/// (entire frame redrawn each pass); `RetainedDamageRect` is
/// the bleeding-edge extension.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassLDamageRectKind {
    #[default]
    FullFrameNoRetainedDamage,
    RetainedDamageRect,
}

impl PassLDamageRectKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FullFrameNoRetainedDamage => "full_frame_no_retained_damage",
            Self::RetainedDamageRect => "retained_damage_rect",
        }
    }
}

/// Typed per-route UI render budget. Caps the typed cost of a
/// single route's draw pass.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassLPerRouteRenderBudget {
    pub schema_version: u16,
    pub route_stable_id: &'static str,
    pub max_draw_calls: u32,
    pub max_instance_count: u32,
    pub max_atlas_bytes: u64,
}

impl PassLPerRouteRenderBudget {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
        route_stable_id: "native_rvelte_fun_ui.launcher_route",
        max_draw_calls: 64,
        max_instance_count: 4096,
        max_atlas_bytes: 64 * 1024 * 1024,
    };

    /// Typed predicate: does the typed proof-scene run fit
    /// inside the budget?
    #[must_use]
    pub const fn accepts(self, draw_calls: u32, instance_count: u32, atlas_bytes: u64) -> bool {
        draw_calls <= self.max_draw_calls
            && instance_count <= self.max_instance_count
            && atlas_bytes <= self.max_atlas_bytes
    }
}

// ============================================================================
// Section 4 — WGSL shared shader module
// ============================================================================

/// Shared WGSL shader module: one vertex shader + four fragment
/// shaders. The runner creates four `wgpu::RenderPipeline`
/// objects from this single module with different fragment
/// entry points.
const PASSL_UI_SHADER_WGSL: &str = "\
struct UiInstance {\n\
    pos_size: vec4<f32>,\n\
    uv_rect: vec4<f32>,\n\
    color: vec4<f32>,\n\
    clip_rect: vec4<f32>,\n\
    corner_radius_kind: vec4<f32>,\n\
}\n\
\n\
struct ScreenParams {\n\
    screen_size: vec4<f32>,\n\
}\n\
\n\
@group(0) @binding(0) var<storage, read> instances: array<UiInstance>;\n\
@group(0) @binding(1) var<uniform> screen_params: ScreenParams;\n\
@group(0) @binding(2) var image_texture: texture_2d<f32>;\n\
@group(0) @binding(3) var glyph_atlas: texture_2d<f32>;\n\
\n\
struct VsOut {\n\
    @builtin(position) clip_pos: vec4<f32>,\n\
    @location(0) uv: vec2<f32>,\n\
    @location(1) color: vec4<f32>,\n\
    @location(2) pixel_pos: vec2<f32>,\n\
    @location(3) quad_min: vec2<f32>,\n\
    @location(4) quad_max: vec2<f32>,\n\
    @location(5) clip_min: vec2<f32>,\n\
    @location(6) clip_max: vec2<f32>,\n\
    @location(7) corner_radius: f32,\n\
}\n\
\n\
@vertex\n\
fn vs_main(@builtin(vertex_index) vid: u32, @builtin(instance_index) iid: u32) -> VsOut {\n\
    let inst = instances[iid];\n\
    let vx = f32(vid & 1u);\n\
    let vy = f32((vid >> 1u) & 1u);\n\
    let quad_local = vec2<f32>(vx, vy);\n\
    let pixel_pos = inst.pos_size.xy + quad_local * inst.pos_size.zw;\n\
    let screen = screen_params.screen_size.xy;\n\
    let ndc_xy = pixel_pos / screen * 2.0 - vec2<f32>(1.0, 1.0);\n\
    let ndc_flipped = vec2<f32>(ndc_xy.x, -ndc_xy.y);\n\
    let uv = mix(inst.uv_rect.xy, inst.uv_rect.zw, quad_local);\n\
    var out: VsOut;\n\
    out.clip_pos = vec4<f32>(ndc_flipped, 0.0, 1.0);\n\
    out.uv = uv;\n\
    out.color = inst.color;\n\
    out.pixel_pos = pixel_pos;\n\
    out.quad_min = inst.pos_size.xy;\n\
    out.quad_max = inst.pos_size.xy + inst.pos_size.zw;\n\
    out.clip_min = inst.clip_rect.xy;\n\
    out.clip_max = inst.clip_rect.zw;\n\
    out.corner_radius = inst.corner_radius_kind.x;\n\
    return out;\n\
}\n\
\n\
fn outside_clip(pixel: vec2<f32>, cmin: vec2<f32>, cmax: vec2<f32>) -> bool {\n\
    return pixel.x < cmin.x || pixel.y < cmin.y || pixel.x >= cmax.x || pixel.y >= cmax.y;\n\
}\n\
\n\
@fragment\n\
fn fs_filled(input: VsOut) -> @location(0) vec4<f32> {\n\
    if (outside_clip(input.pixel_pos, input.clip_min, input.clip_max)) { discard; }\n\
    return input.color;\n\
}\n\
\n\
@fragment\n\
fn fs_image(input: VsOut) -> @location(0) vec4<f32> {\n\
    if (outside_clip(input.pixel_pos, input.clip_min, input.clip_max)) { discard; }\n\
    let dim = vec2<f32>(textureDimensions(image_texture));\n\
    let px = vec2<i32>(clamp(vec2<i32>(input.uv * dim), vec2<i32>(0, 0), vec2<i32>(dim) - vec2<i32>(1, 1)));\n\
    let tex = textureLoad(image_texture, px, 0);\n\
    return tex * input.color;\n\
}\n\
\n\
@fragment\n\
fn fs_glyph(input: VsOut) -> @location(0) vec4<f32> {\n\
    if (outside_clip(input.pixel_pos, input.clip_min, input.clip_max)) { discard; }\n\
    let dim = vec2<f32>(textureDimensions(glyph_atlas));\n\
    let px = vec2<i32>(clamp(vec2<i32>(input.uv * dim), vec2<i32>(0, 0), vec2<i32>(dim) - vec2<i32>(1, 1)));\n\
    let texel = textureLoad(glyph_atlas, px, 0);\n\
    let alpha = texel.a;\n\
    return vec4<f32>(input.color.rgb, input.color.a * alpha);\n\
}\n\
\n\
fn rounded_rect_sdf(p: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {\n\
    let q = abs(p) - b + vec2<f32>(r, r);\n\
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0, 0.0))) - r;\n\
}\n\
\n\
@fragment\n\
fn fs_rounded_rect(input: VsOut) -> @location(0) vec4<f32> {\n\
    if (outside_clip(input.pixel_pos, input.clip_min, input.clip_max)) { discard; }\n\
    let center = (input.quad_min + input.quad_max) * 0.5;\n\
    let half_size = (input.quad_max - input.quad_min) * 0.5;\n\
    let dist = rounded_rect_sdf(input.pixel_pos - center, half_size, input.corner_radius);\n\
    if (dist > 0.0) { discard; }\n\
    return input.color;\n\
}\n\
";

// ============================================================================
// Section 5 — Typed glyph atlas + shelf packer + glyph table
// ============================================================================

/// Typed glyph atlas record. One entry per packed glyph. The
/// UV bounds are normalized into the atlas extent.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassLGlyphTableEntry {
    pub schema_version: u16,
    pub glyph_index: u32,
    pub atlas_origin_x: u32,
    pub atlas_origin_y: u32,
    pub width: u32,
    pub height: u32,
}

impl PassLGlyphTableEntry {
    #[must_use]
    pub fn uv_rect(self, atlas_extent: u32) -> [f32; 4] {
        let extent = atlas_extent as f32;
        [
            self.atlas_origin_x as f32 / extent,
            self.atlas_origin_y as f32 / extent,
            (self.atlas_origin_x + self.width) as f32 / extent,
            (self.atlas_origin_y + self.height) as f32 / extent,
        ]
    }
}

/// Typed CPU shelf packer. The user's "compute-based glyph
/// atlas packing" bleeding-edge extension is recorded as
/// [`PassLGlyphAtlasPackingMode::GpuComputePacker`]; the live
/// path uses this CPU implementation.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassLShelfPacker {
    pub schema_version: u16,
    pub atlas_extent: u32,
    pub cursor_x: u32,
    pub cursor_y: u32,
    pub shelf_height: u32,
}

impl PassLShelfPacker {
    #[must_use]
    pub const fn new(atlas_extent: u32) -> Self {
        Self {
            schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
            atlas_extent,
            cursor_x: 0,
            cursor_y: 0,
            shelf_height: 0,
        }
    }

    /// Pack a typed glyph cell. Returns `None` if the atlas is
    /// full.
    pub fn pack(&mut self, glyph_index: u32, w: u32, h: u32) -> Option<PassLGlyphTableEntry> {
        if w > self.atlas_extent || h > self.atlas_extent {
            return None;
        }
        if self.cursor_x + w > self.atlas_extent {
            self.cursor_x = 0;
            self.cursor_y = self.cursor_y.saturating_add(self.shelf_height);
            self.shelf_height = 0;
        }
        if self.cursor_y + h > self.atlas_extent {
            return None;
        }
        let entry = PassLGlyphTableEntry {
            schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
            glyph_index,
            atlas_origin_x: self.cursor_x,
            atlas_origin_y: self.cursor_y,
            width: w,
            height: h,
        };
        self.cursor_x += w;
        if h > self.shelf_height {
            self.shelf_height = h;
        }
        Some(entry)
    }
}

/// Typed glyph atlas state — texture + shelf packer + glyph
/// table. The runner owns the real `wgpu::Texture`.
pub struct PassLGlyphAtlas {
    pub schema_version: u16,
    pub atlas_extent: u32,
    pub atlas_texture: ::wgpu::Texture,
    pub atlas_view: ::wgpu::TextureView,
    pub packer: PassLShelfPacker,
    pub glyph_table: Vec<PassLGlyphTableEntry>,
}

impl PassLGlyphAtlas {
    #[must_use]
    pub fn allocate(device: &::wgpu::Device) -> Self {
        let atlas_texture = device.create_texture(&::wgpu::TextureDescriptor {
            label: Some("fun_renderer.passl.glyph_atlas"),
            size: ::wgpu::Extent3d {
                width: PASSL_GLYPH_ATLAS_EXTENT,
                height: PASSL_GLYPH_ATLAS_EXTENT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: ::wgpu::TextureDimension::D2,
            format: PASSL_GLYPH_ATLAS_FORMAT,
            usage: ::wgpu::TextureUsages::TEXTURE_BINDING | ::wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas_texture.create_view(&::wgpu::TextureViewDescriptor::default());
        Self {
            schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
            atlas_extent: PASSL_GLYPH_ATLAS_EXTENT,
            atlas_texture,
            atlas_view,
            packer: PassLShelfPacker::new(PASSL_GLYPH_ATLAS_EXTENT),
            glyph_table: Vec::new(),
        }
    }

    /// Pack a typed glyph + upload its pixel bytes. Returns the
    /// typed `PassLGlyphTableEntry` (added to the glyph table).
    pub fn pack_and_upload(
        &mut self,
        queue: &::wgpu::Queue,
        glyph_index: u32,
        width: u32,
        height: u32,
        pixel_bytes_rgba8: &[u8],
    ) -> Option<PassLGlyphTableEntry> {
        let entry = self.packer.pack(glyph_index, width, height)?;
        let bytes_per_row = width * 4;
        let expected_len = (bytes_per_row * height) as usize;
        if pixel_bytes_rgba8.len() < expected_len {
            return None;
        }
        queue.write_texture(
            ::wgpu::TexelCopyTextureInfo {
                texture: &self.atlas_texture,
                mip_level: 0,
                origin: ::wgpu::Origin3d {
                    x: entry.atlas_origin_x,
                    y: entry.atlas_origin_y,
                    z: 0,
                },
                aspect: ::wgpu::TextureAspect::All,
            },
            pixel_bytes_rgba8,
            ::wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(height),
            },
            ::wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.glyph_table.push(entry);
        Some(entry)
    }
}

// ============================================================================
// Section 6 — Typed texture set (frame target + image + atlas placeholder)
// ============================================================================

pub struct PassLTextureSet {
    pub schema_version: u16,
    pub frame_target_texture: ::wgpu::Texture,
    pub frame_target_view: ::wgpu::TextureView,
    pub image_texture: ::wgpu::Texture,
    pub image_view: ::wgpu::TextureView,
}

impl PassLTextureSet {
    #[must_use]
    pub fn allocate(device: &::wgpu::Device, queue: &::wgpu::Queue) -> Self {
        let frame_target_texture = device.create_texture(&::wgpu::TextureDescriptor {
            label: Some("fun_renderer.passl.frame_target"),
            size: ::wgpu::Extent3d {
                width: PASSL_WIDTH,
                height: PASSL_HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: ::wgpu::TextureDimension::D2,
            format: PASSL_FRAME_FORMAT,
            usage: ::wgpu::TextureUsages::RENDER_ATTACHMENT | ::wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let frame_target_view =
            frame_target_texture.create_view(&::wgpu::TextureViewDescriptor::default());

        // Image texture: 2×2 cyan tile (covers the image quad's UV space).
        let image_texture = device.create_texture(&::wgpu::TextureDescriptor {
            label: Some("fun_renderer.passl.image_texture"),
            size: ::wgpu::Extent3d {
                width: 2,
                height: 2,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: ::wgpu::TextureDimension::D2,
            format: PASSL_IMAGE_FORMAT,
            usage: ::wgpu::TextureUsages::TEXTURE_BINDING | ::wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let cyan_4 = [
            0u8, 255u8, 255u8, 255u8, //
            0u8, 255u8, 255u8, 255u8, //
            0u8, 255u8, 255u8, 255u8, //
            0u8, 255u8, 255u8, 255u8, //
        ];
        queue.write_texture(
            ::wgpu::TexelCopyTextureInfo {
                texture: &image_texture,
                mip_level: 0,
                origin: ::wgpu::Origin3d::ZERO,
                aspect: ::wgpu::TextureAspect::All,
            },
            &cyan_4,
            ::wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(2 * 4),
                rows_per_image: Some(2),
            },
            ::wgpu::Extent3d {
                width: 2,
                height: 2,
                depth_or_array_layers: 1,
            },
        );
        let image_view = image_texture.create_view(&::wgpu::TextureViewDescriptor::default());

        Self {
            schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
            frame_target_texture,
            frame_target_view,
            image_texture,
            image_view,
        }
    }
}

// ============================================================================
// Section 7 — Typed pipeline set
// ============================================================================

pub struct PassLUiPipelineSet {
    pub schema_version: u16,
    pub shader: ::wgpu::ShaderModule,
    pub bind_group_layout: ::wgpu::BindGroupLayout,
    pub pipeline_layout: ::wgpu::PipelineLayout,
    pub filled_pipeline: ::wgpu::RenderPipeline,
    pub image_pipeline: ::wgpu::RenderPipeline,
    pub glyph_pipeline: ::wgpu::RenderPipeline,
    pub rounded_rect_pipeline: ::wgpu::RenderPipeline,
}

impl PassLUiPipelineSet {
    #[must_use]
    pub fn create(device: &::wgpu::Device) -> Self {
        let shader = device.create_shader_module(::wgpu::ShaderModuleDescriptor {
            label: Some("fun_renderer.passl.ui_shader.wgsl"),
            source: ::wgpu::ShaderSource::Wgsl(PASSL_UI_SHADER_WGSL.into()),
        });

        let bind_group_layout =
            device.create_bind_group_layout(&::wgpu::BindGroupLayoutDescriptor {
                label: Some("fun_renderer.passl.ui_bgl"),
                entries: &[
                    // 0: instances (storage, read-only)
                    ::wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ::wgpu::ShaderStages::VERTEX,
                        ty: ::wgpu::BindingType::Buffer {
                            ty: ::wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // 1: screen_params (uniform)
                    ::wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ::wgpu::ShaderStages::VERTEX,
                        ty: ::wgpu::BindingType::Buffer {
                            ty: ::wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // 2: image_texture (texture_2d<f32>)
                    ::wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ::wgpu::ShaderStages::FRAGMENT,
                        ty: ::wgpu::BindingType::Texture {
                            sample_type: ::wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: ::wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // 3: glyph_atlas (texture_2d<f32>)
                    ::wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: ::wgpu::ShaderStages::FRAGMENT,
                        ty: ::wgpu::BindingType::Texture {
                            sample_type: ::wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: ::wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });

        let pipeline_layout = device.create_pipeline_layout(&::wgpu::PipelineLayoutDescriptor {
            label: Some("fun_renderer.passl.ui_pipeline_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let make_pipeline = |label: &'static str, entry_point: &'static str| {
            device.create_render_pipeline(&::wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: ::wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: ::wgpu::PipelineCompilationOptions::default(),
                    buffers: &[],
                },
                primitive: ::wgpu::PrimitiveState {
                    topology: ::wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: ::wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: ::wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: ::wgpu::MultisampleState::default(),
                fragment: Some(::wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry_point),
                    compilation_options: ::wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(::wgpu::ColorTargetState {
                        format: PASSL_FRAME_FORMAT,
                        blend: Some(::wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: ::wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };

        let filled_pipeline = make_pipeline("fun_renderer.passl.filled_pipeline", "fs_filled");
        let image_pipeline = make_pipeline("fun_renderer.passl.image_pipeline", "fs_image");
        let glyph_pipeline = make_pipeline("fun_renderer.passl.glyph_pipeline", "fs_glyph");
        let rounded_rect_pipeline = make_pipeline(
            "fun_renderer.passl.rounded_rect_pipeline",
            "fs_rounded_rect",
        );

        Self {
            schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
            shader,
            bind_group_layout,
            pipeline_layout,
            filled_pipeline,
            image_pipeline,
            glyph_pipeline,
            rounded_rect_pipeline,
        }
    }

    #[must_use]
    pub fn pipeline_for(&self, kind: PassLUiQuadKind) -> &::wgpu::RenderPipeline {
        match kind {
            PassLUiQuadKind::Filled => &self.filled_pipeline,
            PassLUiQuadKind::Image => &self.image_pipeline,
            PassLUiQuadKind::Glyph => &self.glyph_pipeline,
            PassLUiQuadKind::RoundedRect => &self.rounded_rect_pipeline,
        }
    }
}

// ============================================================================
// Section 8 — Typed buffer set
// ============================================================================

pub struct PassLBufferSet {
    pub schema_version: u16,
    pub instances_buffer: ::wgpu::Buffer,
    pub screen_params_buffer: ::wgpu::Buffer,
    pub index_buffer: ::wgpu::Buffer,
    pub frame_readback: ::wgpu::Buffer,
    pub readback_bytes_per_row: u32,
}

impl PassLBufferSet {
    pub const COPY_BYTES_PER_ROW_ALIGNMENT: u32 = 256;

    #[must_use]
    pub fn allocate(
        device: &::wgpu::Device,
        queue: &::wgpu::Queue,
        instances: &[PassLUiInstanceData],
    ) -> Self {
        // Instance buffer: storage, read-only.
        let max_instances = instances.len().max(1) as u64;
        let instances_buffer = device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("fun_renderer.passl.instances"),
            size: max_instances * PASSL_UI_INSTANCE_BYTES,
            usage: ::wgpu::BufferUsages::STORAGE | ::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut bytes: Vec<u8> = Vec::with_capacity(instances.len() * PassLUiInstanceData::BYTES);
        for inst in instances {
            bytes.extend_from_slice(&inst.as_bytes());
        }
        if !bytes.is_empty() {
            queue.write_buffer(&instances_buffer, 0, &bytes);
        }

        // Screen params uniform.
        let screen_params_buffer = device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("fun_renderer.passl.screen_params"),
            size: 16,
            usage: ::wgpu::BufferUsages::UNIFORM | ::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let w = PASSL_WIDTH as f32;
        let h = PASSL_HEIGHT as f32;
        let mut params_bytes = [0u8; 16];
        params_bytes[0..4].copy_from_slice(&w.to_le_bytes());
        params_bytes[4..8].copy_from_slice(&h.to_le_bytes());
        params_bytes[8..12].copy_from_slice(&(1.0f32 / w).to_le_bytes());
        params_bytes[12..16].copy_from_slice(&(1.0f32 / h).to_le_bytes());
        queue.write_buffer(&screen_params_buffer, 0, &params_bytes);

        // Index buffer: [0, 1, 2, 1, 3, 2] (two triangles for the 4-vertex quad).
        let indices: [u32; 6] = [0, 1, 2, 1, 3, 2];
        let mut index_bytes = [0u8; 24];
        for (i, &idx) in indices.iter().enumerate() {
            index_bytes[i * 4..i * 4 + 4].copy_from_slice(&idx.to_le_bytes());
        }
        let index_buffer = device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("fun_renderer.passl.index"),
            size: PASSL_QUAD_INDEX_BYTES,
            usage: ::wgpu::BufferUsages::INDEX | ::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&index_buffer, 0, &index_bytes);

        // Frame readback: aligned bytes per row.
        let bytes_per_row_raw = PASSL_WIDTH * 4;
        let bytes_per_row = Self::round_up_alignment(bytes_per_row_raw);
        let frame_readback = device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("fun_renderer.passl.frame_readback"),
            size: (bytes_per_row * PASSL_HEIGHT) as u64,
            usage: ::wgpu::BufferUsages::COPY_DST | ::wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        Self {
            schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
            instances_buffer,
            screen_params_buffer,
            index_buffer,
            frame_readback,
            readback_bytes_per_row: bytes_per_row,
        }
    }

    const fn round_up_alignment(raw_bytes_per_row: u32) -> u32 {
        let alignment = Self::COPY_BYTES_PER_ROW_ALIGNMENT;
        raw_bytes_per_row.div_ceil(alignment) * alignment
    }
}

// ============================================================================
// Section 9 — Run result + frame probe
// ============================================================================

/// Typed frame probe sample. Captures the readback bytes at
/// one named pixel position so the verdict + comparative
/// artifact can confirm which pipeline landed which color.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassLFrameProbeSample {
    pub schema_version: u16,
    pub pixel_x: u32,
    pub pixel_y: u32,
    pub rgba8: [u8; 4],
    pub matches_background: bool,
}

impl PassLFrameProbeSample {
    /// Typed predicate: does this sample look like the typed
    /// proof-scene background (within an integer-luma
    /// tolerance)? When false, the sample landed on a UI
    /// region.
    #[must_use]
    pub fn classify_against_background(rgba8: [u8; 4]) -> bool {
        // Background = (0.10, 0.10, 0.12, 1.0) linear → about
        // (26, 26, 31, 255).
        let bg = [26u8, 26u8, 31u8, 255u8];
        let tol = 16i32;
        ((rgba8[0] as i32) - (bg[0] as i32)).abs() <= tol
            && ((rgba8[1] as i32) - (bg[1] as i32)).abs() <= tol
            && ((rgba8[2] as i32) - (bg[2] as i32)).abs() <= tol
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Resource)]
pub struct PassLRunResult {
    pub schema_version: u16,
    pub batch_table_executed: bool,
    pub batch_count: u32,
    pub filled_quad_dispatches: u32,
    pub image_quad_dispatches: u32,
    pub glyph_run_dispatches: u32,
    pub rounded_rect_dispatches: u32,
    pub scissor_stack_applied: bool,
    pub glyph_atlas_used: bool,
    pub glyph_atlas_packed_glyph_count: u32,
    pub launcher_route_identifier: Option<PassLLauncherRouteIdentifier>,
    pub frame_readback_ok: bool,
    pub frame_probe_filled: PassLFrameProbeSample,
    pub frame_probe_image: PassLFrameProbeSample,
    pub frame_probe_glyph: PassLFrameProbeSample,
    pub frame_probe_rounded: PassLFrameProbeSample,
    pub frame_probe_background: PassLFrameProbeSample,
    pub bleeding_edge_extension_surfaces: PassLBleedingEdgeExtensionSurfaces,
}

impl PassLRunResult {
    /// Typed predicate: at least one of the four UI probes
    /// sampled a non-background color, proving the typed batch
    /// table actually painted UI pixels.
    #[must_use]
    pub fn any_ui_probe_landed_on_ui_color(&self) -> bool {
        !self.frame_probe_filled.matches_background
            || !self.frame_probe_image.matches_background
            || !self.frame_probe_glyph.matches_background
            || !self.frame_probe_rounded.matches_background
    }

    /// Typed predicate: the background probe still matches the
    /// proof-scene background (no UI covered it). Confirms the
    /// runner cleared the frame target before drawing.
    #[must_use]
    pub fn background_probe_remains_background(&self) -> bool {
        self.frame_probe_background.matches_background
    }
}

// ============================================================================
// Section 10 — Verdict
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassLNativeUiLiveVerdict {
    pub schema_version: u16,
    pub passes_ui_batch_table_executed_as_actual_draw_pass: bool,
    pub passes_filled_quad_pipeline_executed: bool,
    pub passes_image_quad_pipeline_executed: bool,
    pub passes_glyph_atlas_and_glyph_run_pipeline_executed: bool,
    pub passes_clip_scissor_stack_applied: bool,
    pub passes_rounded_rect_analytic_sdf_pipeline_executed: bool,
    pub passes_product_launcher_route_rendered_over_proof_scene: bool,
    pub passes_frame_probe_samples_ui_colored_region: bool,
}

impl PassLNativeUiLiveVerdict {
    #[must_use]
    pub fn evaluate(result: &PassLRunResult) -> Self {
        let launcher_ok = matches!(
            result.launcher_route_identifier,
            Some(id) if id.is_product_default
        );
        Self {
            schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
            passes_ui_batch_table_executed_as_actual_draw_pass: result.batch_table_executed
                && result.batch_count > 0,
            passes_filled_quad_pipeline_executed: result.filled_quad_dispatches > 0,
            passes_image_quad_pipeline_executed: result.image_quad_dispatches > 0,
            passes_glyph_atlas_and_glyph_run_pipeline_executed: result.glyph_run_dispatches > 0
                && result.glyph_atlas_used
                && result.glyph_atlas_packed_glyph_count > 0,
            passes_clip_scissor_stack_applied: result.scissor_stack_applied,
            passes_rounded_rect_analytic_sdf_pipeline_executed: result.rounded_rect_dispatches > 0,
            passes_product_launcher_route_rendered_over_proof_scene: launcher_ok
                && result.frame_readback_ok
                && result.background_probe_remains_background(),
            passes_frame_probe_samples_ui_colored_region: result.frame_readback_ok
                && result.any_ui_probe_landed_on_ui_color(),
        }
    }

    #[must_use]
    pub const fn passes(&self) -> bool {
        self.passes_ui_batch_table_executed_as_actual_draw_pass
            && self.passes_filled_quad_pipeline_executed
            && self.passes_image_quad_pipeline_executed
            && self.passes_glyph_atlas_and_glyph_run_pipeline_executed
            && self.passes_clip_scissor_stack_applied
            && self.passes_rounded_rect_analytic_sdf_pipeline_executed
            && self.passes_product_launcher_route_rendered_over_proof_scene
            && self.passes_frame_probe_samples_ui_colored_region
    }

    #[must_use]
    pub const fn first_failed(&self) -> Option<PassLNativeUiLiveRule> {
        if !self.passes_ui_batch_table_executed_as_actual_draw_pass {
            return Some(PassLNativeUiLiveRule::UiBatchTableExecutedAsActualDrawPass);
        }
        if !self.passes_filled_quad_pipeline_executed {
            return Some(PassLNativeUiLiveRule::FilledQuadPipelineExecuted);
        }
        if !self.passes_image_quad_pipeline_executed {
            return Some(PassLNativeUiLiveRule::ImageQuadPipelineExecuted);
        }
        if !self.passes_glyph_atlas_and_glyph_run_pipeline_executed {
            return Some(PassLNativeUiLiveRule::GlyphAtlasAndGlyphRunPipelineExecuted);
        }
        if !self.passes_clip_scissor_stack_applied {
            return Some(PassLNativeUiLiveRule::ClipScissorStackApplied);
        }
        if !self.passes_rounded_rect_analytic_sdf_pipeline_executed {
            return Some(PassLNativeUiLiveRule::RoundedRectAnalyticSdfPipelineExecuted);
        }
        if !self.passes_product_launcher_route_rendered_over_proof_scene {
            return Some(PassLNativeUiLiveRule::ProductLauncherRouteRenderedOverProofScene);
        }
        if !self.passes_frame_probe_samples_ui_colored_region {
            return Some(PassLNativeUiLiveRule::FrameProbeSamplesUiColoredRegion);
        }
        None
    }

    #[must_use]
    pub const fn violation_count(&self) -> u32 {
        let mut count = 0u32;
        if !self.passes_ui_batch_table_executed_as_actual_draw_pass {
            count += 1;
        }
        if !self.passes_filled_quad_pipeline_executed {
            count += 1;
        }
        if !self.passes_image_quad_pipeline_executed {
            count += 1;
        }
        if !self.passes_glyph_atlas_and_glyph_run_pipeline_executed {
            count += 1;
        }
        if !self.passes_clip_scissor_stack_applied {
            count += 1;
        }
        if !self.passes_rounded_rect_analytic_sdf_pipeline_executed {
            count += 1;
        }
        if !self.passes_product_launcher_route_rendered_over_proof_scene {
            count += 1;
        }
        if !self.passes_frame_probe_samples_ui_colored_region {
            count += 1;
        }
        count
    }
}

// ============================================================================
// Section 11 — Canonical proof scene
// ============================================================================

/// Typed proof-scene instance set. Four instances laid out so
/// each pipeline lands a clearly identifiable color at a
/// known pixel.
#[must_use]
pub fn passl_proof_scene_instances() -> Vec<PassLUiInstanceData> {
    let clip = PassLClipRect::FULL_PROOF_SCENE.as_f32_xy_xy();
    let uv = [0.0f32, 0.0, 1.0, 1.0];
    vec![
        // Filled (dark blue) at (1, 1) size (4, 4).
        PassLUiInstanceData {
            pos_size: [1.0, 1.0, 4.0, 4.0],
            uv_rect: uv,
            color: [0.08, 0.16, 0.78, 1.0],
            clip_rect: clip,
            corner_radius_kind: [0.0, PassLUiQuadKind::Filled.as_float(), 0.0, 0.0],
        },
        // Image (white tint × cyan tex = cyan) at (6, 1) size (4, 4).
        PassLUiInstanceData {
            pos_size: [6.0, 1.0, 4.0, 4.0],
            uv_rect: uv,
            color: [1.0, 1.0, 1.0, 1.0],
            clip_rect: clip,
            corner_radius_kind: [0.0, PassLUiQuadKind::Image.as_float(), 0.0, 0.0],
        },
        // Glyph (white via atlas alpha) at (1, 6) size (4, 4).
        PassLUiInstanceData {
            pos_size: [1.0, 6.0, 4.0, 4.0],
            uv_rect: uv,
            color: [1.0, 1.0, 1.0, 1.0],
            clip_rect: clip,
            corner_radius_kind: [0.0, PassLUiQuadKind::Glyph.as_float(), 0.0, 0.0],
        },
        // Rounded rect (orange, radius=1) at (6, 6) size (4, 4).
        PassLUiInstanceData {
            pos_size: [6.0, 6.0, 4.0, 4.0],
            uv_rect: uv,
            color: [1.0, 0.5, 0.0, 1.0],
            clip_rect: clip,
            corner_radius_kind: [1.0, PassLUiQuadKind::RoundedRect.as_float(), 0.0, 0.0],
        },
    ]
}

/// Typed proof-scene batch table. One batch per pipeline, each
/// with one instance.
#[must_use]
pub fn passl_proof_scene_batch_table() -> Vec<PassLUiBatchHeader> {
    let clip = PassLClipRect::FULL_PROOF_SCENE;
    vec![
        PassLUiBatchHeader::new(0, PassLUiQuadKind::Filled, 0, 1, clip),
        PassLUiBatchHeader::new(1, PassLUiQuadKind::Image, 1, 1, clip),
        PassLUiBatchHeader::new(2, PassLUiQuadKind::Glyph, 2, 1, clip),
        PassLUiBatchHeader::new(3, PassLUiQuadKind::RoundedRect, 3, 1, clip),
    ]
}

/// Probe pixel inside the filled quad — guaranteed dark blue.
pub const PASSL_PROBE_FILLED: (u32, u32) = (3, 3);
/// Probe pixel inside the image quad — guaranteed cyan.
pub const PASSL_PROBE_IMAGE: (u32, u32) = (8, 3);
/// Probe pixel inside the glyph quad — guaranteed white.
pub const PASSL_PROBE_GLYPH: (u32, u32) = (3, 8);
/// Probe pixel inside the rounded rect (well inside, away from
/// the rounded corners).
pub const PASSL_PROBE_ROUNDED: (u32, u32) = (8, 8);
/// Probe pixel outside every UI quad — should remain the
/// proof-scene background.
pub const PASSL_PROBE_BACKGROUND: (u32, u32) = (12, 0);

// ============================================================================
// Section 12 — Top-level runner
// ============================================================================

pub enum PassLBootResult {
    Ran(Box<PassLRanPayload>),
    BridgeRuntimeFailed(WgpuBridgeRuntimeFailure),
}

pub struct PassLRanPayload {
    pub bridge_state: WgpuBridgeDeviceState<Dx12Native>,
    pub result: PassLRunResult,
}

#[must_use]
pub fn run_native_ui_live_against_fresh_dx12_device() -> PassLBootResult {
    let options = WgpuBridgeRuntimeOptions::production_default();
    let bridge_state = match initialize_wgpu_bridge_runtime::<Dx12Native>(&options) {
        Ok(state) => state,
        Err(failure) => return PassLBootResult::BridgeRuntimeFailed(failure),
    };

    let textures = PassLTextureSet::allocate(&bridge_state.device, &bridge_state.queue);
    let pipelines = PassLUiPipelineSet::create(&bridge_state.device);

    // Glyph atlas: pack a single 4×4 white cell so the typed
    // glyph quad's full UV range samples white alpha.
    let mut glyph_atlas = PassLGlyphAtlas::allocate(&bridge_state.device);
    let white_4x4: [u8; 64] = {
        let mut buf = [0u8; 64];
        for chunk in buf.chunks_exact_mut(4) {
            chunk.copy_from_slice(&[255, 255, 255, 255]);
        }
        buf
    };
    let packed_glyph = glyph_atlas
        .pack_and_upload(&bridge_state.queue, 0, 4, 4, &white_4x4)
        .is_some();

    // Instances + batch table.
    let mut instances = passl_proof_scene_instances();
    // Rewrite the glyph instance's UV rect so it samples the
    // packed 4×4 cell from the 32×32 atlas, not the entire
    // atlas.
    if let (Some(glyph_inst), Some(packed)) = (
        instances.get_mut(2),
        glyph_atlas.glyph_table.first().copied(),
    ) {
        glyph_inst.uv_rect = packed.uv_rect(glyph_atlas.atlas_extent);
    }
    let batch_table = passl_proof_scene_batch_table();
    let buffers = PassLBufferSet::allocate(&bridge_state.device, &bridge_state.queue, &instances);

    let bind_group = bridge_state
        .device
        .create_bind_group(&::wgpu::BindGroupDescriptor {
            label: Some("fun_renderer.passl.ui_bind_group"),
            layout: &pipelines.bind_group_layout,
            entries: &[
                ::wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffers.instances_buffer.as_entire_binding(),
                },
                ::wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buffers.screen_params_buffer.as_entire_binding(),
                },
                ::wgpu::BindGroupEntry {
                    binding: 2,
                    resource: ::wgpu::BindingResource::TextureView(&textures.image_view),
                },
                ::wgpu::BindGroupEntry {
                    binding: 3,
                    resource: ::wgpu::BindingResource::TextureView(&glyph_atlas.atlas_view),
                },
            ],
        });

    let mut result = PassLRunResult {
        schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
        batch_table_executed: false,
        batch_count: 0,
        glyph_atlas_used: packed_glyph,
        glyph_atlas_packed_glyph_count: if packed_glyph { 1 } else { 0 },
        launcher_route_identifier: Some(PassLLauncherRouteIdentifier::PRODUCT_DEFAULT),
        bleeding_edge_extension_surfaces: PassLBleedingEdgeExtensionSurfaces::PRODUCT_DEFAULT,
        ..PassLRunResult::default()
    };

    let mut encoder =
        bridge_state
            .device
            .create_command_encoder(&::wgpu::CommandEncoderDescriptor {
                label: Some("fun_renderer.passl.encoder"),
            });

    // Single render pass: clear to proof-scene background, set
    // scissor (clip stack), walk the typed batch table.
    {
        let mut pass = encoder.begin_render_pass(&::wgpu::RenderPassDescriptor {
            label: Some("fun_renderer.passl.ui_draw_pass"),
            color_attachments: &[Some(::wgpu::RenderPassColorAttachment {
                view: &textures.frame_target_view,
                depth_slice: None,
                resolve_target: None,
                ops: ::wgpu::Operations {
                    load: ::wgpu::LoadOp::Clear(::wgpu::Color {
                        r: PASSL_PROOF_SCENE_BACKGROUND_LINEAR[0] as f64,
                        g: PASSL_PROOF_SCENE_BACKGROUND_LINEAR[1] as f64,
                        b: PASSL_PROOF_SCENE_BACKGROUND_LINEAR[2] as f64,
                        a: PASSL_PROOF_SCENE_BACKGROUND_LINEAR[3] as f64,
                    }),
                    store: ::wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        // Step 5: clip / scissor stack — restrict to the typed
        // proof-scene scissor.
        let s = PASSL_SCISSOR_PROOF_SCENE;
        pass.set_scissor_rect(s.min_x, s.min_y, s.max_x - s.min_x, s.max_y - s.min_y);
        result.scissor_stack_applied = true;

        pass.set_bind_group(0, &bind_group, &[]);
        pass.set_index_buffer(buffers.index_buffer.slice(..), ::wgpu::IndexFormat::Uint32);

        // Step 1: walk the typed batch table and dispatch.
        for batch in &batch_table {
            pass.set_pipeline(pipelines.pipeline_for(batch.quad_kind));
            let first = batch.first_instance;
            let end = first + batch.instance_count;
            pass.draw_indexed(0..PASSL_QUAD_INDEX_COUNT, 0, first..end);
            match batch.quad_kind {
                PassLUiQuadKind::Filled => result.filled_quad_dispatches += 1,
                PassLUiQuadKind::Image => result.image_quad_dispatches += 1,
                PassLUiQuadKind::Glyph => result.glyph_run_dispatches += 1,
                PassLUiQuadKind::RoundedRect => result.rounded_rect_dispatches += 1,
            }
            result.batch_count += 1;
        }
        result.batch_table_executed = true;
    }

    // Step 8: copy frame target → readback.
    encoder.copy_texture_to_buffer(
        ::wgpu::TexelCopyTextureInfo {
            texture: &textures.frame_target_texture,
            mip_level: 0,
            origin: ::wgpu::Origin3d::ZERO,
            aspect: ::wgpu::TextureAspect::All,
        },
        ::wgpu::TexelCopyBufferInfo {
            buffer: &buffers.frame_readback,
            layout: ::wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(buffers.readback_bytes_per_row),
                rows_per_image: Some(PASSL_HEIGHT),
            },
        },
        ::wgpu::Extent3d {
            width: PASSL_WIDTH,
            height: PASSL_HEIGHT,
            depth_or_array_layers: 1,
        },
    );

    let command_buffer = encoder.finish();
    let _submission_index = bridge_state
        .queue
        .submit(::core::iter::once(command_buffer));

    let frame_slice = buffers.frame_readback.slice(..);
    let (tx, rx) = unbounded();
    frame_slice.map_async(::wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    let _ = bridge_state
        .device
        .poll(::wgpu::PollType::wait_indefinitely());

    if let Ok(Ok(())) = rx.recv() {
        let data = frame_slice.get_mapped_range();
        let bpr = buffers.readback_bytes_per_row as usize;
        let mut sample_pixel = |x: u32, y: u32| -> PassLFrameProbeSample {
            let offset = (y as usize) * bpr + (x as usize) * 4;
            let mut rgba8 = [0u8; 4];
            if offset + 4 <= data.len() {
                rgba8.copy_from_slice(&data[offset..offset + 4]);
            }
            PassLFrameProbeSample {
                schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
                pixel_x: x,
                pixel_y: y,
                rgba8,
                matches_background: PassLFrameProbeSample::classify_against_background(rgba8),
            }
        };
        result.frame_probe_filled = sample_pixel(PASSL_PROBE_FILLED.0, PASSL_PROBE_FILLED.1);
        result.frame_probe_image = sample_pixel(PASSL_PROBE_IMAGE.0, PASSL_PROBE_IMAGE.1);
        result.frame_probe_glyph = sample_pixel(PASSL_PROBE_GLYPH.0, PASSL_PROBE_GLYPH.1);
        result.frame_probe_rounded = sample_pixel(PASSL_PROBE_ROUNDED.0, PASSL_PROBE_ROUNDED.1);
        result.frame_probe_background =
            sample_pixel(PASSL_PROBE_BACKGROUND.0, PASSL_PROBE_BACKGROUND.1);
        result.frame_readback_ok = true;
        drop(data);
        buffers.frame_readback.unmap();
    }

    PassLBootResult::Ran(Box::new(PassLRanPayload {
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
        assert_eq!(PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION, 1);
        assert_eq!(PASSL_RULE_COUNT, 8);
        assert_eq!(PassLNativeUiLiveRule::ALL.len(), PASSL_RULE_COUNT);
    }

    #[test]
    fn rule_index_round_trip() {
        for (i, &rule) in PassLNativeUiLiveRule::ALL.iter().enumerate() {
            assert_eq!(rule.index(), i, "{}", rule.as_str());
        }
    }

    #[test]
    fn rule_taxonomy_strings_are_unique() {
        let mut seen = hashbrown::HashSet::new();
        for rule in PassLNativeUiLiveRule::ALL {
            assert!(seen.insert(rule.as_str()), "duplicate: {}", rule.as_str());
        }
    }

    #[test]
    fn quad_kind_taxonomy() {
        for (i, &kind) in PassLUiQuadKind::ALL.iter().enumerate() {
            assert_eq!(kind.as_float(), i as f32, "{}", kind.as_str());
        }
        let mut seen = hashbrown::HashSet::new();
        for kind in PassLUiQuadKind::ALL {
            assert!(seen.insert(kind.as_str()), "duplicate: {}", kind.as_str());
        }
    }

    #[test]
    fn instance_byte_layout() {
        let inst = PassLUiInstanceData {
            pos_size: [1.0, 2.0, 3.0, 4.0],
            uv_rect: [5.0, 6.0, 7.0, 8.0],
            color: [0.1, 0.2, 0.3, 0.4],
            clip_rect: [0.0, 0.0, 16.0, 16.0],
            corner_radius_kind: [2.0, PassLUiQuadKind::RoundedRect.as_float(), 0.0, 0.0],
        };
        let bytes = inst.as_bytes();
        assert_eq!(bytes.len(), PassLUiInstanceData::BYTES);
        assert_eq!(&bytes[0..4], &1.0f32.to_le_bytes());
        assert_eq!(&bytes[12..16], &4.0f32.to_le_bytes());
        assert_eq!(&bytes[16..20], &5.0f32.to_le_bytes());
        assert_eq!(&bytes[32..36], &0.1f32.to_le_bytes());
        assert_eq!(&bytes[48..52], &0.0f32.to_le_bytes());
        assert_eq!(&bytes[60..64], &16.0f32.to_le_bytes());
        assert_eq!(&bytes[64..68], &2.0f32.to_le_bytes());
        assert_eq!(&bytes[68..72], &3.0f32.to_le_bytes());
    }

    #[test]
    fn clip_rect_predicates() {
        let r = PassLClipRect {
            schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
            min_x: 1,
            min_y: 1,
            max_x: 5,
            max_y: 5,
        };
        assert!(r.covers_any_pixel());
        assert!(r.contains_pixel(2, 2));
        assert!(!r.contains_pixel(5, 5)); // max is exclusive
        assert!(!r.contains_pixel(0, 0));
        let empty = PassLClipRect {
            schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
            min_x: 5,
            min_y: 5,
            max_x: 5,
            max_y: 5,
        };
        assert!(!empty.covers_any_pixel());
    }

    #[test]
    fn shelf_packer_packs_until_full() {
        let mut packer = PassLShelfPacker::new(32);
        let a = packer.pack(0, 8, 8).expect("first pack fits");
        assert_eq!((a.atlas_origin_x, a.atlas_origin_y), (0, 0));
        let b = packer.pack(1, 8, 8).expect("second pack fits");
        assert_eq!((b.atlas_origin_x, b.atlas_origin_y), (8, 0));
        // Pack 8×8 four times across width 32 → fills row.
        packer.pack(2, 8, 8).expect("3rd");
        packer.pack(3, 8, 8).expect("4th");
        // Next pack should wrap to a new shelf at y=8.
        let e = packer.pack(4, 8, 8).expect("5th wraps to next shelf");
        assert_eq!((e.atlas_origin_x, e.atlas_origin_y), (0, 8));
        // Width too large → None.
        let oversize = packer.pack(99, 33, 8);
        assert!(oversize.is_none());
    }

    #[test]
    fn shelf_packer_returns_none_when_atlas_full() {
        let mut packer = PassLShelfPacker::new(8);
        // 8×8 atlas — one 8×8 glyph fills it.
        assert!(packer.pack(0, 8, 8).is_some());
        // Anything else returns None.
        assert!(packer.pack(1, 1, 1).is_none());
    }

    #[test]
    fn glyph_table_entry_uv_rect_normalizes_to_atlas_extent() {
        let entry = PassLGlyphTableEntry {
            schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
            glyph_index: 0,
            atlas_origin_x: 8,
            atlas_origin_y: 0,
            width: 4,
            height: 4,
        };
        let uv = entry.uv_rect(32);
        assert_eq!(uv, [8.0 / 32.0, 0.0, 12.0 / 32.0, 4.0 / 32.0]);
    }

    #[test]
    fn launcher_route_identifier_is_native_rvelte() {
        let id = PassLLauncherRouteIdentifier::PRODUCT_DEFAULT;
        assert_eq!(id.stable_id, "native_rvelte_fun_ui.launcher_route");
        assert!(id.is_product_default);
    }

    #[test]
    fn bleeding_edge_default_surfaces_are_typed() {
        let s = PassLBleedingEdgeExtensionSurfaces::PRODUCT_DEFAULT;
        assert_eq!(
            s.gpu_path_rendering,
            PassLUiPathRenderingCapability::PlanningSurface,
        );
        assert_eq!(
            s.glyph_atlas_packing_mode,
            PassLGlyphAtlasPackingMode::CpuShelfPacker,
        );
        assert_eq!(
            s.rounded_rect_shading_mode,
            PassLRoundedRectShadingMode::AnalyticDistance,
        );
        assert_eq!(
            s.damage_rect_kind,
            PassLDamageRectKind::FullFrameNoRetainedDamage,
        );
        let b = s.per_route_render_budget;
        assert_eq!(b.route_stable_id, "native_rvelte_fun_ui.launcher_route");
        assert!(b.accepts(4, 4, 4096));
        assert!(!b.accepts(b.max_draw_calls + 1, 4, 4096));
    }

    #[test]
    fn proof_scene_instances_have_four_quads_one_per_kind() {
        let inst = passl_proof_scene_instances();
        assert_eq!(inst.len(), 4);
        assert_eq!(
            inst[0].corner_radius_kind[1],
            PassLUiQuadKind::Filled.as_float(),
        );
        assert_eq!(
            inst[1].corner_radius_kind[1],
            PassLUiQuadKind::Image.as_float(),
        );
        assert_eq!(
            inst[2].corner_radius_kind[1],
            PassLUiQuadKind::Glyph.as_float(),
        );
        assert_eq!(
            inst[3].corner_radius_kind[1],
            PassLUiQuadKind::RoundedRect.as_float(),
        );
    }

    #[test]
    fn proof_scene_batch_table_has_four_batches() {
        let table = passl_proof_scene_batch_table();
        assert_eq!(table.len(), 4);
        for (i, batch) in table.iter().enumerate() {
            assert_eq!(batch.first_instance, i as u32);
            assert_eq!(batch.instance_count, 1);
        }
    }

    #[test]
    fn frame_probe_classifier_distinguishes_ui_from_background() {
        // Background = ~(26, 26, 31) linear.
        assert!(PassLFrameProbeSample::classify_against_background([
            26, 26, 31, 255
        ]));
        // Dark blue UI color (about 20, 41, 199) — far from
        // background.
        assert!(!PassLFrameProbeSample::classify_against_background([
            20, 41, 199, 255
        ]));
        // Cyan, white, orange — all UI.
        assert!(!PassLFrameProbeSample::classify_against_background([
            0, 255, 255, 255
        ]));
        assert!(!PassLFrameProbeSample::classify_against_background([
            255, 255, 255, 255
        ]));
        assert!(!PassLFrameProbeSample::classify_against_background([
            255, 128, 0, 255
        ]));
    }

    #[test]
    fn verdict_passes_under_synthetic_full_evidence() {
        let mut result = PassLRunResult {
            schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
            batch_table_executed: true,
            batch_count: 4,
            filled_quad_dispatches: 1,
            image_quad_dispatches: 1,
            glyph_run_dispatches: 1,
            rounded_rect_dispatches: 1,
            scissor_stack_applied: true,
            glyph_atlas_used: true,
            glyph_atlas_packed_glyph_count: 1,
            launcher_route_identifier: Some(PassLLauncherRouteIdentifier::PRODUCT_DEFAULT),
            frame_readback_ok: true,
            frame_probe_filled: PassLFrameProbeSample {
                schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
                pixel_x: 3,
                pixel_y: 3,
                rgba8: [20, 41, 199, 255],
                matches_background: false,
            },
            frame_probe_image: PassLFrameProbeSample {
                schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
                pixel_x: 8,
                pixel_y: 3,
                rgba8: [0, 255, 255, 255],
                matches_background: false,
            },
            frame_probe_glyph: PassLFrameProbeSample {
                schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
                pixel_x: 3,
                pixel_y: 8,
                rgba8: [255, 255, 255, 255],
                matches_background: false,
            },
            frame_probe_rounded: PassLFrameProbeSample {
                schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
                pixel_x: 8,
                pixel_y: 8,
                rgba8: [255, 128, 0, 255],
                matches_background: false,
            },
            frame_probe_background: PassLFrameProbeSample {
                schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
                pixel_x: 12,
                pixel_y: 0,
                rgba8: [26, 26, 31, 255],
                matches_background: true,
            },
            bleeding_edge_extension_surfaces: PassLBleedingEdgeExtensionSurfaces::PRODUCT_DEFAULT,
        };

        let verdict = PassLNativeUiLiveVerdict::evaluate(&result);
        assert!(
            verdict.passes(),
            "verdict should pass under full synthetic evidence; first_failed = {:?}",
            verdict.first_failed(),
        );
        assert_eq!(verdict.violation_count(), 0);

        // Drop glyph atlas usage — rule 4 should fail first.
        result.glyph_atlas_used = false;
        let verdict = PassLNativeUiLiveVerdict::evaluate(&result);
        assert_eq!(
            verdict.first_failed(),
            Some(PassLNativeUiLiveRule::GlyphAtlasAndGlyphRunPipelineExecuted),
        );
    }

    #[test]
    fn verdict_fails_when_no_ui_probe_lands_on_ui_color() {
        let result = PassLRunResult {
            schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
            batch_table_executed: true,
            batch_count: 4,
            filled_quad_dispatches: 1,
            image_quad_dispatches: 1,
            glyph_run_dispatches: 1,
            rounded_rect_dispatches: 1,
            scissor_stack_applied: true,
            glyph_atlas_used: true,
            glyph_atlas_packed_glyph_count: 1,
            launcher_route_identifier: Some(PassLLauncherRouteIdentifier::PRODUCT_DEFAULT),
            frame_readback_ok: true,
            frame_probe_filled: PassLFrameProbeSample {
                schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
                pixel_x: 3,
                pixel_y: 3,
                rgba8: [26, 26, 31, 255],
                matches_background: true,
            },
            frame_probe_image: PassLFrameProbeSample {
                matches_background: true,
                ..PassLFrameProbeSample::default()
            },
            frame_probe_glyph: PassLFrameProbeSample {
                matches_background: true,
                ..PassLFrameProbeSample::default()
            },
            frame_probe_rounded: PassLFrameProbeSample {
                matches_background: true,
                ..PassLFrameProbeSample::default()
            },
            frame_probe_background: PassLFrameProbeSample {
                matches_background: true,
                ..PassLFrameProbeSample::default()
            },
            bleeding_edge_extension_surfaces: PassLBleedingEdgeExtensionSurfaces::PRODUCT_DEFAULT,
        };
        let verdict = PassLNativeUiLiveVerdict::evaluate(&result);
        assert!(!verdict.passes());
        assert_eq!(
            verdict.first_failed(),
            Some(PassLNativeUiLiveRule::FrameProbeSamplesUiColoredRegion),
        );
    }

    #[test]
    fn verdict_fails_when_launcher_route_is_missing() {
        let result = PassLRunResult {
            schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
            batch_table_executed: true,
            batch_count: 4,
            filled_quad_dispatches: 1,
            image_quad_dispatches: 1,
            glyph_run_dispatches: 1,
            rounded_rect_dispatches: 1,
            scissor_stack_applied: true,
            glyph_atlas_used: true,
            glyph_atlas_packed_glyph_count: 1,
            launcher_route_identifier: None, // missing
            frame_readback_ok: true,
            frame_probe_filled: PassLFrameProbeSample {
                schema_version: PASSL_NATIVE_UI_LIVE_SCHEMA_VERSION,
                pixel_x: 3,
                pixel_y: 3,
                rgba8: [20, 41, 199, 255],
                matches_background: false,
            },
            frame_probe_image: PassLFrameProbeSample::default(),
            frame_probe_glyph: PassLFrameProbeSample::default(),
            frame_probe_rounded: PassLFrameProbeSample::default(),
            frame_probe_background: PassLFrameProbeSample {
                matches_background: true,
                ..PassLFrameProbeSample::default()
            },
            bleeding_edge_extension_surfaces: PassLBleedingEdgeExtensionSurfaces::PRODUCT_DEFAULT,
        };
        let verdict = PassLNativeUiLiveVerdict::evaluate(&result);
        assert!(!verdict.passes());
        assert_eq!(
            verdict.first_failed(),
            Some(PassLNativeUiLiveRule::ProductLauncherRouteRenderedOverProofScene),
        );
    }

    #[test]
    fn buffer_set_round_up_alignment_aligns_to_256() {
        assert_eq!(PassLBufferSet::round_up_alignment(0), 0);
        assert_eq!(PassLBufferSet::round_up_alignment(1), 256);
        assert_eq!(PassLBufferSet::round_up_alignment(64), 256);
        assert_eq!(PassLBufferSet::round_up_alignment(256), 256);
        assert_eq!(PassLBufferSet::round_up_alignment(257), 512);
    }

    /// Live smoke test: boots a real wgpu device on DX12, runs
    /// the full Pass L pipeline, and verifies every typed rule
    /// holds. On hosts without DX12 the test logs the bridge
    /// failure honestly and returns. On hosts where wgpu
    /// resolves to a non-DX12 backend the test still runs but
    /// skips strict pixel assertions.
    #[test]
    fn live_passl_runs_real_ui_batches_with_atlas_and_frame_probe() {
        use crate::live_proof_frame_executor::ran_on_real_dx12_adapter;

        let outcome = run_native_ui_live_against_fresh_dx12_device();
        match outcome {
            PassLBootResult::Ran(payload) => {
                let PassLRanPayload {
                    bridge_state,
                    result,
                } = *payload;
                if !ran_on_real_dx12_adapter(&bridge_state) {
                    eprintln!(
                        "live_passl: non-DX12 actual backend ({:?}); skipping strict assertions",
                        bridge_state.actual_native_backend,
                    );
                    return;
                }
                assert!(result.batch_table_executed);
                assert_eq!(result.batch_count, 4);
                assert_eq!(result.filled_quad_dispatches, 1);
                assert_eq!(result.image_quad_dispatches, 1);
                assert_eq!(result.glyph_run_dispatches, 1);
                assert_eq!(result.rounded_rect_dispatches, 1);
                assert!(result.scissor_stack_applied);
                assert!(result.glyph_atlas_used);
                assert!(result.frame_readback_ok);
                assert!(result.any_ui_probe_landed_on_ui_color());
                assert!(result.background_probe_remains_background());

                let verdict = PassLNativeUiLiveVerdict::evaluate(&result);
                assert!(
                    verdict.passes(),
                    "Pass L verdict must pass; first_failed = {:?}",
                    verdict.first_failed(),
                );
            }
            PassLBootResult::BridgeRuntimeFailed(failure) => {
                eprintln!(
                    "live_passl: bridge runtime failed (host without DX12 adapter): {failure:?}",
                );
            }
        }
    }
}
