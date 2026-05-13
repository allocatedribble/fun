//! Pass 28 — Upscaling, Motion Vectors, and Vendor SDKs.
//!
//! Prepare premium presentation features without violating bridge
//! boundaries. The existing `upscaling.rs` and `frame_generation.rs`
//! modules carry the planning surface. Pass 28 adds the typed
//! runtime contracts:
//!
//! `MotionVectorPassPlan` is the typed velocity-buffer pass,
//! producing pixel-space motion deltas from previous/current world
//! transforms and previous/current camera matrices, with a typed
//! debug-view selector.
//!
//! `VendorSdkStatus` is the per-vendor (DLSS / FSR2 / FSR3 / XeSS /
//! Reflex) typed status enum. `DlssTruth` enforces the Pass 26
//! contract: DLSS cannot become active while the native command-list
//! path is unavailable.
//!
//! `UpscalerInputValidation` runs the typed input contract for
//! every upscaler kind (Native / TaaUpscale / DLSS / FSR / XeSS):
//! color, depth, motion vectors, exposure, reactive mask, jitter,
//! render resolution, display resolution.
//!
//! `HudLessSceneColorPolicy` declares the typed UI/scene
//! separation: frame generation interpolates the HUD-less scene
//! color, and the UI is composited on top of the interpolated
//! frame. `PresentPacingPolicy` is the typed pacing rule;
//! `LatencyPolicy` records the typed budget the operator selected
//! (PowerSaver / Balanced / LatencyOptimised / Reflex).
//!
//! Design rules: premium features stay above ECS/graph and below
//! bridge-native interop where required. No stubbed SDK claims
//! success — every SDK has a typed `VendorSdkStatus::Active` only
//! when every required input is bound, the SDK is linked, the
//! adapter is supported, and the command-list path is available.
//! UI/scene separation is preserved by routing UI composition
//! through `HudLessSceneColorPolicy` rather than the upscaler input
//! color.

use fun_ecs::Resource;

use crate::component_api::{RenderExtent2d, RenderStableId, RenderVec2, RenderVec3};
use crate::dx12_production::Dx12NativeSdkClaimPolicy;

pub const VENDOR_SDK_BRIDGE_SCHEMA_VERSION: u16 = 1;

pub const UPSCALER_KIND_COUNT: usize = 5;
pub const VENDOR_SDK_KIND_COUNT: usize = 5;
pub const UPSCALER_INPUT_KIND_COUNT: usize = 8;
pub const DLSS_STATUS_COUNT: usize = 7;
pub const MOTION_VECTOR_DEBUG_VIEW_COUNT: usize = 4;

// ============================================================================
// Section 1 — Motion vector pass plan
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct MotionVectorTransform {
    pub world_matrix: [f32; 16],
    pub frame_index: u64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct MotionVectorCameraMatrices {
    pub view: [f32; 16],
    pub projection: [f32; 16],
    pub view_projection: [f32; 16],
    pub jitter: RenderVec2,
    pub frame_index: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionVectorPerObjectInput {
    pub object_id: RenderStableId,
    pub previous_transform: MotionVectorTransform,
    pub current_transform: MotionVectorTransform,
    pub world_position: RenderVec3,
    pub static_geometry: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MotionVectorEncoding {
    #[default]
    PixelDelta,
    Ndc,
    UnitSphere,
    QuantisedSnorm16,
}

impl MotionVectorEncoding {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PixelDelta => "pixel_delta",
            Self::Ndc => "ndc",
            Self::UnitSphere => "unit_sphere",
            Self::QuantisedSnorm16 => "quantised_snorm16",
        }
    }

    #[must_use]
    pub const fn supports_dlss(self) -> bool {
        matches!(self, Self::PixelDelta | Self::Ndc)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MotionVectorDebugView {
    #[default]
    None,
    Magnitude,
    Direction,
    StaticVsDynamic,
}

impl MotionVectorDebugView {
    pub const ALL: [Self; MOTION_VECTOR_DEBUG_VIEW_COUNT] = [
        Self::None,
        Self::Magnitude,
        Self::Direction,
        Self::StaticVsDynamic,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Magnitude => "magnitude",
            Self::Direction => "direction",
            Self::StaticVsDynamic => "static_vs_dynamic",
        }
    }

    #[must_use]
    pub const fn is_active(self) -> bool {
        !matches!(self, Self::None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MotionVectorPassPlan {
    pub schema_version: u16,
    pub view_id: RenderStableId,
    pub render_resolution: RenderExtent2d,
    pub encoding: MotionVectorEncoding,
    pub debug_view: MotionVectorDebugView,
    pub previous_camera_available: bool,
    pub jitter_subtracted_from_motion: bool,
    pub static_geometry_zero_motion: bool,
    pub workgroup_size_x: u8,
    pub workgroup_size_y: u8,
}

impl MotionVectorPassPlan {
    #[must_use]
    pub fn build(
        view_id: RenderStableId,
        render_resolution: RenderExtent2d,
        previous_camera_available: bool,
    ) -> Self {
        Self {
            schema_version: VENDOR_SDK_BRIDGE_SCHEMA_VERSION,
            view_id,
            render_resolution,
            encoding: MotionVectorEncoding::PixelDelta,
            debug_view: MotionVectorDebugView::None,
            previous_camera_available,
            jitter_subtracted_from_motion: true,
            static_geometry_zero_motion: true,
            workgroup_size_x: 8,
            workgroup_size_y: 8,
        }
    }

    #[must_use]
    pub const fn dispatch_x(&self) -> u32 {
        self.render_resolution
            .width
            .div_ceil(self.workgroup_size_x as u32)
    }

    #[must_use]
    pub const fn dispatch_y(&self) -> u32 {
        self.render_resolution
            .height
            .div_ceil(self.workgroup_size_y as u32)
    }

    #[must_use]
    pub const fn requires_previous_frame(&self) -> bool {
        true
    }

    #[must_use]
    pub fn supports_dlss_handoff(&self) -> bool {
        self.encoding.supports_dlss()
            && self.previous_camera_available
            && self.jitter_subtracted_from_motion
    }
}

#[derive(Debug, Default, Clone, PartialEq, Resource)]
pub struct MotionVectorFrameInputs {
    pub schema_version: u16,
    pub view_id: RenderStableId,
    pub previous_camera: MotionVectorCameraMatrices,
    pub current_camera: MotionVectorCameraMatrices,
    pub objects: Vec<MotionVectorPerObjectInput>,
    pub previous_frame_present: bool,
}

impl MotionVectorFrameInputs {
    pub fn clear(&mut self) {
        self.objects.clear();
        self.previous_frame_present = false;
    }

    pub fn record(&mut self, object: MotionVectorPerObjectInput) {
        self.objects.push(object);
    }

    #[must_use]
    pub fn total(&self) -> u32 {
        self.objects.len() as u32
    }

    #[must_use]
    pub fn dynamic_object_count(&self) -> u32 {
        self.objects.iter().filter(|o| !o.static_geometry).count() as u32
    }
}

// ============================================================================
// Section 2 — Upscaler kind + input validation
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UpscalerKind {
    #[default]
    Native,
    TaaUpscale,
    Dlss,
    Fsr,
    XeSs,
}

impl UpscalerKind {
    pub const ALL: [Self; UPSCALER_KIND_COUNT] = [
        Self::Native,
        Self::TaaUpscale,
        Self::Dlss,
        Self::Fsr,
        Self::XeSs,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Native => 0,
            Self::TaaUpscale => 1,
            Self::Dlss => 2,
            Self::Fsr => 3,
            Self::XeSs => 4,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::TaaUpscale => "taa_upscale",
            Self::Dlss => "dlss",
            Self::Fsr => "fsr",
            Self::XeSs => "xess",
        }
    }

    #[must_use]
    pub const fn requires_motion_vectors(self) -> bool {
        matches!(self, Self::TaaUpscale | Self::Dlss | Self::Fsr | Self::XeSs)
    }

    #[must_use]
    pub const fn requires_jitter(self) -> bool {
        matches!(self, Self::TaaUpscale | Self::Dlss | Self::Fsr | Self::XeSs)
    }

    #[must_use]
    pub const fn requires_reactive_mask(self) -> bool {
        matches!(self, Self::Fsr)
    }

    #[must_use]
    pub const fn requires_native_command_list(self) -> bool {
        matches!(self, Self::Dlss)
    }

    #[must_use]
    pub const fn associated_vendor_sdk(self) -> Option<VendorSdkKind> {
        match self {
            Self::Dlss => Some(VendorSdkKind::Dlss),
            Self::Fsr => Some(VendorSdkKind::Fsr2),
            Self::XeSs => Some(VendorSdkKind::XeSs),
            Self::Native | Self::TaaUpscale => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UpscalerInputKind {
    Color,
    Depth,
    MotionVectors,
    Exposure,
    ReactiveMask,
    Jitter,
    RenderResolution,
    DisplayResolution,
}

impl UpscalerInputKind {
    pub const ALL: [Self; UPSCALER_INPUT_KIND_COUNT] = [
        Self::Color,
        Self::Depth,
        Self::MotionVectors,
        Self::Exposure,
        Self::ReactiveMask,
        Self::Jitter,
        Self::RenderResolution,
        Self::DisplayResolution,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Color => 0,
            Self::Depth => 1,
            Self::MotionVectors => 2,
            Self::Exposure => 3,
            Self::ReactiveMask => 4,
            Self::Jitter => 5,
            Self::RenderResolution => 6,
            Self::DisplayResolution => 7,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Color => "color",
            Self::Depth => "depth",
            Self::MotionVectors => "motion_vectors",
            Self::Exposure => "exposure",
            Self::ReactiveMask => "reactive_mask",
            Self::Jitter => "jitter",
            Self::RenderResolution => "render_resolution",
            Self::DisplayResolution => "display_resolution",
        }
    }

    #[must_use]
    pub const fn required_for(self, kind: UpscalerKind) -> bool {
        match self {
            // Color, depth, render_res, display_res are always required.
            Self::Color | Self::Depth | Self::RenderResolution | Self::DisplayResolution => true,
            // Exposure is required for tonemapping-aware upscalers.
            Self::Exposure => matches!(
                kind,
                UpscalerKind::Dlss | UpscalerKind::Fsr | UpscalerKind::XeSs,
            ),
            Self::MotionVectors => kind.requires_motion_vectors(),
            Self::Jitter => kind.requires_jitter(),
            Self::ReactiveMask => kind.requires_reactive_mask(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UpscalerInputBindings {
    pub schema_version: u16,
    pub kind: UpscalerKind,
    pub bindings_present: [bool; UPSCALER_INPUT_KIND_COUNT],
}

impl UpscalerInputBindings {
    #[must_use]
    pub fn for_kind(kind: UpscalerKind) -> Self {
        Self {
            schema_version: VENDOR_SDK_BRIDGE_SCHEMA_VERSION,
            kind,
            bindings_present: [false; UPSCALER_INPUT_KIND_COUNT],
        }
    }

    pub fn mark_present(&mut self, input: UpscalerInputKind) {
        self.bindings_present[input.index()] = true;
    }

    pub fn mark_missing(&mut self, input: UpscalerInputKind) {
        self.bindings_present[input.index()] = false;
    }

    #[must_use]
    pub fn is_present(&self, input: UpscalerInputKind) -> bool {
        self.bindings_present[input.index()]
    }

    pub fn missing_inputs(&self) -> impl Iterator<Item = UpscalerInputKind> + '_ {
        UpscalerInputKind::ALL
            .into_iter()
            .filter(|input| input.required_for(self.kind) && !self.is_present(*input))
    }

    #[must_use]
    pub fn missing_input_count(&self) -> u8 {
        self.missing_inputs().count() as u8
    }

    #[must_use]
    pub fn validates(&self) -> bool {
        self.missing_input_count() == 0
    }
}

// ============================================================================
// Section 3 — Vendor SDK bridge status model + DLSS truth
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VendorSdkKind {
    #[default]
    Dlss,
    Fsr2,
    Fsr3,
    XeSs,
    Reflex,
}

impl VendorSdkKind {
    pub const ALL: [Self; VENDOR_SDK_KIND_COUNT] =
        [Self::Dlss, Self::Fsr2, Self::Fsr3, Self::XeSs, Self::Reflex];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Dlss => 0,
            Self::Fsr2 => 1,
            Self::Fsr3 => 2,
            Self::XeSs => 3,
            Self::Reflex => 4,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dlss => "dlss",
            Self::Fsr2 => "fsr2",
            Self::Fsr3 => "fsr3",
            Self::XeSs => "xess",
            Self::Reflex => "reflex",
        }
    }

    #[must_use]
    pub const fn requires_native_command_list(self) -> bool {
        matches!(self, Self::Dlss | Self::Reflex)
    }

    #[must_use]
    pub const fn requires_specific_vendor_adapter(self) -> bool {
        matches!(self, Self::Dlss | Self::Reflex)
    }
}

/// Per-vendor SDK status. Mirrors the contract in the Pass 28 spec
/// for DLSS truth — every SDK has a typed reason it is *not* active
/// so the operator and diagnostics can name the gate.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VendorSdkStatus {
    #[default]
    Disabled,
    SdkNotLinked,
    UnsupportedAdapter,
    NativeCommandListUnavailable,
    MissingInputs,
    Active,
    FailedRuntime,
}

impl VendorSdkStatus {
    pub const ALL: [Self; DLSS_STATUS_COUNT] = [
        Self::Disabled,
        Self::SdkNotLinked,
        Self::UnsupportedAdapter,
        Self::NativeCommandListUnavailable,
        Self::MissingInputs,
        Self::Active,
        Self::FailedRuntime,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::SdkNotLinked => "sdk_not_linked",
            Self::UnsupportedAdapter => "unsupported_adapter",
            Self::NativeCommandListUnavailable => "native_command_list_unavailable",
            Self::MissingInputs => "missing_inputs",
            Self::Active => "active",
            Self::FailedRuntime => "failed_runtime",
        }
    }

    #[must_use]
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }

    #[must_use]
    pub const fn is_blocked(self) -> bool {
        !matches!(self, Self::Active)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VendorSdkBridgeRecord {
    pub schema_version: u16,
    pub kind: VendorSdkKind,
    pub status: VendorSdkStatus,
    pub sdk_linked: bool,
    pub adapter_supported: bool,
    pub command_list_path_available: bool,
    pub inputs_validated: bool,
    pub last_runtime_failure_count: u32,
}

impl VendorSdkBridgeRecord {
    #[must_use]
    pub const fn new(kind: VendorSdkKind) -> Self {
        Self {
            schema_version: VENDOR_SDK_BRIDGE_SCHEMA_VERSION,
            kind,
            status: VendorSdkStatus::Disabled,
            sdk_linked: false,
            adapter_supported: false,
            command_list_path_available: false,
            inputs_validated: false,
            last_runtime_failure_count: 0,
        }
    }

    /// Resolve the SDK status from current bridge state. The
    /// resolution order is fixed so diagnostics name the *first*
    /// reason the SDK is gated, matching the Pass 28 contract.
    pub fn resolve_status(&mut self) {
        if !self.sdk_linked {
            self.status = VendorSdkStatus::SdkNotLinked;
            return;
        }
        if !self.adapter_supported {
            self.status = VendorSdkStatus::UnsupportedAdapter;
            return;
        }
        if self.kind.requires_native_command_list() && !self.command_list_path_available {
            self.status = VendorSdkStatus::NativeCommandListUnavailable;
            return;
        }
        if !self.inputs_validated {
            self.status = VendorSdkStatus::MissingInputs;
            return;
        }
        if self.last_runtime_failure_count > 0 {
            self.status = VendorSdkStatus::FailedRuntime;
            return;
        }
        self.status = VendorSdkStatus::Active;
    }

    pub fn record_runtime_failure(&mut self) {
        self.last_runtime_failure_count = self.last_runtime_failure_count.saturating_add(1);
        self.status = VendorSdkStatus::FailedRuntime;
    }

    pub fn clear_runtime_failures(&mut self) {
        self.last_runtime_failure_count = 0;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Resource)]
pub struct VendorSdkBridgeStatus {
    pub schema_version: u16,
    pub records: [VendorSdkBridgeRecord; VENDOR_SDK_KIND_COUNT],
}

impl VendorSdkBridgeStatus {
    #[must_use]
    pub fn new() -> Self {
        let records = [
            VendorSdkBridgeRecord::new(VendorSdkKind::Dlss),
            VendorSdkBridgeRecord::new(VendorSdkKind::Fsr2),
            VendorSdkBridgeRecord::new(VendorSdkKind::Fsr3),
            VendorSdkBridgeRecord::new(VendorSdkKind::XeSs),
            VendorSdkBridgeRecord::new(VendorSdkKind::Reflex),
        ];
        Self {
            schema_version: VENDOR_SDK_BRIDGE_SCHEMA_VERSION,
            records,
        }
    }

    #[must_use]
    pub fn record_for(&self, kind: VendorSdkKind) -> VendorSdkBridgeRecord {
        self.records[kind.index()]
    }

    pub fn record_for_mut(&mut self, kind: VendorSdkKind) -> &mut VendorSdkBridgeRecord {
        &mut self.records[kind.index()]
    }

    pub fn resolve_all(&mut self) {
        for record in &mut self.records {
            record.resolve_status();
        }
    }

    /// Apply the Pass 26 `Dx12NativeSdkClaimPolicy` to every SDK
    /// that requires native command list access. The policy is the
    /// single authoritative source — DLSS / Reflex cannot become
    /// active while the command-list path is unavailable.
    pub fn apply_dx12_native_sdk_policy(&mut self, policy: Dx12NativeSdkClaimPolicy) {
        let command_list_available = policy.allows_native_command_list_use();
        for record in &mut self.records {
            if record.kind.requires_native_command_list() {
                record.command_list_path_available = command_list_available;
            }
        }
    }

    #[must_use]
    pub fn count_blocked(&self) -> u32 {
        self.records
            .iter()
            .filter(|r| r.status.is_blocked() && !matches!(r.status, VendorSdkStatus::Disabled))
            .count() as u32
    }

    #[must_use]
    pub fn dlss_truth_holds(&self, command_list_available: bool) -> bool {
        let dlss = self.record_for(VendorSdkKind::Dlss);
        if command_list_available {
            true
        } else {
            !dlss.status.is_active()
        }
    }
}

impl Default for VendorSdkBridgeStatus {
    fn default() -> Self {
        Self::new()
    }
}

/// Typed DLSS truth check that consumes the Pass 26 native SDK
/// claim policy. The renderer asserts this every frame so a
/// stubbed SDK cannot claim success.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DlssTruth {
    pub schema_version: u16,
    pub command_list_available: bool,
    pub current_status: VendorSdkStatus,
    pub holds: bool,
}

impl DlssTruth {
    #[must_use]
    pub fn evaluate(
        bridge_status: &VendorSdkBridgeStatus,
        policy: Dx12NativeSdkClaimPolicy,
    ) -> Self {
        let command_list_available = policy.allows_native_command_list_use();
        let dlss_status = bridge_status.record_for(VendorSdkKind::Dlss).status;
        let holds = if command_list_available {
            true
        } else {
            !matches!(dlss_status, VendorSdkStatus::Active)
        };
        Self {
            schema_version: VENDOR_SDK_BRIDGE_SCHEMA_VERSION,
            command_list_available,
            current_status: dlss_status,
            holds,
        }
    }
}

// ============================================================================
// Section 4 — HUD-less scene color + present pacing + latency policy
// ============================================================================

/// Typed UI/scene separation for frame generation. Frame
/// generation interpolates the *HUD-less* scene color; the UI is
/// composited on top of the interpolated frame via the
/// `FinalOutputTransform` Pass-23 contract.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HudLessSceneColorPolicy {
    #[default]
    HudLessForFrameGeneration,
    HudIncludedDebugOnly,
    SceneOnlyNoUi,
}

impl HudLessSceneColorPolicy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HudLessForFrameGeneration => "hud_less_for_frame_generation",
            Self::HudIncludedDebugOnly => "hud_included_debug_only",
            Self::SceneOnlyNoUi => "scene_only_no_ui",
        }
    }

    #[must_use]
    pub const fn requires_separate_ui_layer(self) -> bool {
        matches!(self, Self::HudLessForFrameGeneration | Self::SceneOnlyNoUi)
    }

    #[must_use]
    pub const fn frame_generation_safe(self) -> bool {
        matches!(self, Self::HudLessForFrameGeneration)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PresentPacingMode {
    #[default]
    SyncToVblank,
    Mailbox,
    Immediate,
    Reflex,
    FixedTargetFps(u16),
}

impl PresentPacingMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SyncToVblank => "sync_to_vblank",
            Self::Mailbox => "mailbox",
            Self::Immediate => "immediate",
            Self::Reflex => "reflex",
            Self::FixedTargetFps(_) => "fixed_target_fps",
        }
    }

    #[must_use]
    pub const fn allows_frame_generation(self) -> bool {
        matches!(
            self,
            Self::Mailbox | Self::Immediate | Self::Reflex | Self::FixedTargetFps(_)
        )
    }

    #[must_use]
    pub const fn target_fps(self) -> Option<u16> {
        match self {
            Self::FixedTargetFps(fps) => Some(fps),
            _ => None,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LatencyPolicy {
    #[default]
    Balanced,
    PowerSaver,
    LatencyOptimised,
    ReflexLowLatency,
    ReflexLowLatencyBoost,
}

impl LatencyPolicy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
            Self::PowerSaver => "power_saver",
            Self::LatencyOptimised => "latency_optimised",
            Self::ReflexLowLatency => "reflex_low_latency",
            Self::ReflexLowLatencyBoost => "reflex_low_latency_boost",
        }
    }

    #[must_use]
    pub const fn requires_reflex_sdk(self) -> bool {
        matches!(self, Self::ReflexLowLatency | Self::ReflexLowLatencyBoost)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PresentPacingPolicy {
    pub schema_version: u16,
    pub pacing_mode: PresentPacingMode,
    pub latency_policy: LatencyPolicy,
    pub hud_less: HudLessSceneColorPolicy,
}

impl PresentPacingPolicy {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: VENDOR_SDK_BRIDGE_SCHEMA_VERSION,
        pacing_mode: PresentPacingMode::SyncToVblank,
        latency_policy: LatencyPolicy::Balanced,
        hud_less: HudLessSceneColorPolicy::HudLessForFrameGeneration,
    };

    #[must_use]
    pub const fn frame_generation_compatible(&self) -> bool {
        self.pacing_mode.allows_frame_generation() && self.hud_less.frame_generation_safe()
    }

    #[must_use]
    pub const fn requires_reflex(&self) -> bool {
        self.latency_policy.requires_reflex_sdk()
            || matches!(self.pacing_mode, PresentPacingMode::Reflex)
    }
}

impl Default for PresentPacingPolicy {
    fn default() -> Self {
        Self::PRODUCT_DEFAULT
    }
}

// ============================================================================
// Section 5 — Frame artifact tying motion vectors + upscaler + SDK + pacing
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Resource)]
pub struct UpscalerVendorSdkPlan {
    pub schema_version: u16,
    pub view_id: RenderStableId,
    pub upscaler_kind: UpscalerKind,
    pub bindings: UpscalerInputBindings,
    pub vendor_sdk_status: VendorSdkBridgeStatus,
    pub motion_vector_pass: MotionVectorPassPlan,
    pub present_pacing: PresentPacingPolicy,
    pub dlss_truth: DlssTruth,
}

impl UpscalerVendorSdkPlan {
    #[must_use]
    pub fn assemble(
        view_id: RenderStableId,
        upscaler_kind: UpscalerKind,
        motion_vector_pass: MotionVectorPassPlan,
        bindings: UpscalerInputBindings,
        vendor_sdk_status: VendorSdkBridgeStatus,
        present_pacing: PresentPacingPolicy,
        dlss_native_sdk_policy: Dx12NativeSdkClaimPolicy,
    ) -> Self {
        let dlss_truth = DlssTruth::evaluate(&vendor_sdk_status, dlss_native_sdk_policy);
        Self {
            schema_version: VENDOR_SDK_BRIDGE_SCHEMA_VERSION,
            view_id,
            upscaler_kind,
            bindings,
            vendor_sdk_status,
            motion_vector_pass,
            present_pacing,
            dlss_truth,
        }
    }

    #[must_use]
    pub fn upscaler_can_run(&self) -> bool {
        self.bindings.validates()
            && (!self.upscaler_kind.requires_native_command_list()
                || self.dlss_truth.command_list_available)
    }

    #[must_use]
    pub fn frame_generation_safe(&self) -> bool {
        self.present_pacing.frame_generation_compatible()
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Resource)]
pub struct UpscalerVendorSdkDiagnostics {
    pub schema_version: u16,
    pub view_id: RenderStableId,
    pub upscaler_kind: UpscalerKind,
    pub bindings_validated: bool,
    pub missing_input_count: u8,
    pub blocked_vendor_sdk_count: u32,
    pub dlss_truth_holds: bool,
    pub frame_generation_safe: bool,
    pub motion_vectors_supports_dlss_handoff: bool,
    pub motion_vector_dispatch_x: u32,
    pub motion_vector_dispatch_y: u32,
}

impl UpscalerVendorSdkDiagnostics {
    #[must_use]
    pub fn from_plan(plan: &UpscalerVendorSdkPlan) -> Self {
        Self {
            schema_version: VENDOR_SDK_BRIDGE_SCHEMA_VERSION,
            view_id: plan.view_id,
            upscaler_kind: plan.upscaler_kind,
            bindings_validated: plan.bindings.validates(),
            missing_input_count: plan.bindings.missing_input_count(),
            blocked_vendor_sdk_count: plan.vendor_sdk_status.count_blocked(),
            dlss_truth_holds: plan.dlss_truth.holds,
            frame_generation_safe: plan.frame_generation_safe(),
            motion_vectors_supports_dlss_handoff: plan.motion_vector_pass.supports_dlss_handoff(),
            motion_vector_dispatch_x: plan.motion_vector_pass.dispatch_x(),
            motion_vector_dispatch_y: plan.motion_vector_pass.dispatch_y(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn validated_bindings_for(kind: UpscalerKind) -> UpscalerInputBindings {
        let mut bindings = UpscalerInputBindings::for_kind(kind);
        for input in UpscalerInputKind::ALL {
            if input.required_for(kind) {
                bindings.mark_present(input);
            }
        }
        bindings
    }

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(VENDOR_SDK_BRIDGE_SCHEMA_VERSION, 1);
        assert_eq!(UPSCALER_KIND_COUNT, 5);
        assert_eq!(VENDOR_SDK_KIND_COUNT, 5);
        assert_eq!(UPSCALER_INPUT_KIND_COUNT, 8);
        assert_eq!(DLSS_STATUS_COUNT, 7);
        assert_eq!(MOTION_VECTOR_DEBUG_VIEW_COUNT, 4);
    }

    #[test]
    fn motion_vector_pass_default_supports_dlss_handoff() {
        let plan = MotionVectorPassPlan::build(
            RenderStableId::new(1),
            RenderExtent2d::new(1280, 720),
            true,
        );
        assert!(plan.supports_dlss_handoff());
        assert_eq!(plan.encoding, MotionVectorEncoding::PixelDelta);
    }

    #[test]
    fn motion_vector_pass_without_previous_camera_blocks_dlss() {
        let plan = MotionVectorPassPlan::build(
            RenderStableId::new(1),
            RenderExtent2d::new(1280, 720),
            false,
        );
        assert!(!plan.supports_dlss_handoff());
    }

    #[test]
    fn motion_vector_pass_dispatch_covers_full_render_resolution() {
        let plan = MotionVectorPassPlan::build(
            RenderStableId::new(1),
            RenderExtent2d::new(1920, 1080),
            true,
        );
        assert_eq!(plan.dispatch_x(), 240); // 1920 / 8
        assert_eq!(plan.dispatch_y(), 135); // 1080 / 8
    }

    #[test]
    fn motion_vector_encoding_dlss_supports_subset() {
        assert!(MotionVectorEncoding::PixelDelta.supports_dlss());
        assert!(MotionVectorEncoding::Ndc.supports_dlss());
        assert!(!MotionVectorEncoding::UnitSphere.supports_dlss());
        assert!(!MotionVectorEncoding::QuantisedSnorm16.supports_dlss());
    }

    #[test]
    fn motion_vector_debug_view_active_classification() {
        assert!(!MotionVectorDebugView::None.is_active());
        assert!(MotionVectorDebugView::Magnitude.is_active());
        assert!(MotionVectorDebugView::Direction.is_active());
        assert!(MotionVectorDebugView::StaticVsDynamic.is_active());
    }

    #[test]
    fn motion_vector_frame_inputs_count_dynamic_objects() {
        let mut inputs = MotionVectorFrameInputs::default();
        inputs.record(MotionVectorPerObjectInput {
            object_id: RenderStableId::new(1),
            previous_transform: MotionVectorTransform::default(),
            current_transform: MotionVectorTransform::default(),
            world_position: RenderVec3::ZERO,
            static_geometry: true,
        });
        inputs.record(MotionVectorPerObjectInput {
            object_id: RenderStableId::new(2),
            previous_transform: MotionVectorTransform::default(),
            current_transform: MotionVectorTransform::default(),
            world_position: RenderVec3::ZERO,
            static_geometry: false,
        });
        assert_eq!(inputs.total(), 2);
        assert_eq!(inputs.dynamic_object_count(), 1);
    }

    #[test]
    fn upscaler_kind_required_inputs_match_pass_28_contract() {
        // Native: only color/depth/render/display
        assert!(UpscalerInputKind::Color.required_for(UpscalerKind::Native));
        assert!(UpscalerInputKind::Depth.required_for(UpscalerKind::Native));
        assert!(!UpscalerInputKind::MotionVectors.required_for(UpscalerKind::Native));
        assert!(!UpscalerInputKind::Jitter.required_for(UpscalerKind::Native));
        assert!(!UpscalerInputKind::ReactiveMask.required_for(UpscalerKind::Native));
        assert!(!UpscalerInputKind::Exposure.required_for(UpscalerKind::Native));
        // DLSS: needs motion + jitter + exposure + render + display
        assert!(UpscalerInputKind::MotionVectors.required_for(UpscalerKind::Dlss));
        assert!(UpscalerInputKind::Jitter.required_for(UpscalerKind::Dlss));
        assert!(UpscalerInputKind::Exposure.required_for(UpscalerKind::Dlss));
        assert!(!UpscalerInputKind::ReactiveMask.required_for(UpscalerKind::Dlss));
        // FSR: also requires reactive mask
        assert!(UpscalerInputKind::ReactiveMask.required_for(UpscalerKind::Fsr));
        assert!(UpscalerInputKind::MotionVectors.required_for(UpscalerKind::Fsr));
        // XeSS: needs motion + jitter + exposure
        assert!(UpscalerInputKind::MotionVectors.required_for(UpscalerKind::XeSs));
        assert!(UpscalerInputKind::Jitter.required_for(UpscalerKind::XeSs));
    }

    #[test]
    fn upscaler_input_bindings_validate_when_all_required_inputs_present() {
        let bindings = validated_bindings_for(UpscalerKind::Dlss);
        assert!(bindings.validates());
        assert_eq!(bindings.missing_input_count(), 0);
    }

    #[test]
    fn upscaler_input_bindings_fail_when_required_input_missing() {
        let mut bindings = validated_bindings_for(UpscalerKind::Dlss);
        bindings.mark_missing(UpscalerInputKind::Jitter);
        assert!(!bindings.validates());
        assert!(
            bindings
                .missing_inputs()
                .any(|input| matches!(input, UpscalerInputKind::Jitter))
        );
    }

    #[test]
    fn vendor_sdk_kind_routes_to_upscaler_kind() {
        assert_eq!(
            UpscalerKind::Dlss.associated_vendor_sdk(),
            Some(VendorSdkKind::Dlss),
        );
        assert_eq!(
            UpscalerKind::Fsr.associated_vendor_sdk(),
            Some(VendorSdkKind::Fsr2),
        );
        assert_eq!(
            UpscalerKind::XeSs.associated_vendor_sdk(),
            Some(VendorSdkKind::XeSs),
        );
        assert_eq!(UpscalerKind::Native.associated_vendor_sdk(), None);
        assert_eq!(UpscalerKind::TaaUpscale.associated_vendor_sdk(), None);
    }

    #[test]
    fn vendor_sdk_status_disabled_default_is_blocked() {
        let record = VendorSdkBridgeRecord::new(VendorSdkKind::Dlss);
        assert!(record.status.is_blocked());
        assert!(matches!(record.status, VendorSdkStatus::Disabled));
    }

    #[test]
    fn vendor_sdk_resolve_status_routes_through_each_gate_in_order() {
        let mut record = VendorSdkBridgeRecord::new(VendorSdkKind::Dlss);
        record.resolve_status();
        assert_eq!(record.status, VendorSdkStatus::SdkNotLinked);

        record.sdk_linked = true;
        record.resolve_status();
        assert_eq!(record.status, VendorSdkStatus::UnsupportedAdapter);

        record.adapter_supported = true;
        record.resolve_status();
        assert_eq!(record.status, VendorSdkStatus::NativeCommandListUnavailable);

        record.command_list_path_available = true;
        record.resolve_status();
        assert_eq!(record.status, VendorSdkStatus::MissingInputs);

        record.inputs_validated = true;
        record.resolve_status();
        assert_eq!(record.status, VendorSdkStatus::Active);

        record.record_runtime_failure();
        assert_eq!(record.status, VendorSdkStatus::FailedRuntime);
    }

    #[test]
    fn vendor_sdk_resolve_status_fsr_does_not_require_command_list() {
        let mut record = VendorSdkBridgeRecord::new(VendorSdkKind::Fsr2);
        record.sdk_linked = true;
        record.adapter_supported = true;
        // command_list_path_available stays false
        record.inputs_validated = true;
        record.resolve_status();
        assert_eq!(record.status, VendorSdkStatus::Active);
    }

    #[test]
    fn vendor_sdk_bridge_apply_dx12_policy_blocks_dlss_when_command_list_unavailable() {
        let mut bridge = VendorSdkBridgeStatus::default();
        let dlss = bridge.record_for_mut(VendorSdkKind::Dlss);
        dlss.sdk_linked = true;
        dlss.adapter_supported = true;
        dlss.inputs_validated = true;
        bridge.apply_dx12_native_sdk_policy(Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT);
        bridge.resolve_all();
        assert_eq!(
            bridge.record_for(VendorSdkKind::Dlss).status,
            VendorSdkStatus::NativeCommandListUnavailable,
        );
    }

    #[test]
    fn vendor_sdk_bridge_apply_dx12_policy_unblocks_dlss_when_sanctioned() {
        let mut bridge = VendorSdkBridgeStatus::default();
        let dlss = bridge.record_for_mut(VendorSdkKind::Dlss);
        dlss.sdk_linked = true;
        dlss.adapter_supported = true;
        dlss.inputs_validated = true;
        let policy = Dx12NativeSdkClaimPolicy::from_capabilities(true, false);
        bridge.apply_dx12_native_sdk_policy(policy);
        bridge.resolve_all();
        assert_eq!(
            bridge.record_for(VendorSdkKind::Dlss).status,
            VendorSdkStatus::Active,
        );
    }

    #[test]
    fn dlss_truth_holds_when_command_list_available() {
        let mut bridge = VendorSdkBridgeStatus::default();
        let dlss = bridge.record_for_mut(VendorSdkKind::Dlss);
        dlss.sdk_linked = true;
        dlss.adapter_supported = true;
        dlss.inputs_validated = true;
        let policy = Dx12NativeSdkClaimPolicy::from_capabilities(true, false);
        bridge.apply_dx12_native_sdk_policy(policy);
        bridge.resolve_all();
        let truth = DlssTruth::evaluate(&bridge, policy);
        assert!(truth.holds);
        assert!(truth.command_list_available);
        assert_eq!(truth.current_status, VendorSdkStatus::Active);
    }

    #[test]
    fn dlss_truth_holds_when_command_list_unavailable_and_dlss_not_active() {
        let bridge = VendorSdkBridgeStatus::default();
        let policy = Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT;
        let truth = DlssTruth::evaluate(&bridge, policy);
        assert!(truth.holds);
        assert!(!truth.command_list_available);
    }

    #[test]
    fn hud_less_policy_default_is_frame_generation_safe() {
        let policy = HudLessSceneColorPolicy::default();
        assert_eq!(policy, HudLessSceneColorPolicy::HudLessForFrameGeneration);
        assert!(policy.frame_generation_safe());
        assert!(policy.requires_separate_ui_layer());
    }

    #[test]
    fn hud_included_debug_only_blocks_frame_generation() {
        assert!(!HudLessSceneColorPolicy::HudIncludedDebugOnly.frame_generation_safe());
    }

    #[test]
    fn present_pacing_default_is_sync_to_vblank_with_balanced_latency() {
        let policy = PresentPacingPolicy::default();
        assert_eq!(policy.pacing_mode, PresentPacingMode::SyncToVblank);
        assert_eq!(policy.latency_policy, LatencyPolicy::Balanced);
        assert!(!policy.frame_generation_compatible());
    }

    #[test]
    fn present_pacing_mailbox_with_hud_less_supports_frame_generation() {
        let policy = PresentPacingPolicy {
            schema_version: VENDOR_SDK_BRIDGE_SCHEMA_VERSION,
            pacing_mode: PresentPacingMode::Mailbox,
            latency_policy: LatencyPolicy::Balanced,
            hud_less: HudLessSceneColorPolicy::HudLessForFrameGeneration,
        };
        assert!(policy.frame_generation_compatible());
    }

    #[test]
    fn latency_reflex_requires_reflex_sdk() {
        assert!(LatencyPolicy::ReflexLowLatency.requires_reflex_sdk());
        assert!(LatencyPolicy::ReflexLowLatencyBoost.requires_reflex_sdk());
        assert!(!LatencyPolicy::Balanced.requires_reflex_sdk());
        assert!(!LatencyPolicy::PowerSaver.requires_reflex_sdk());
    }

    #[test]
    fn fixed_target_fps_pacing_records_target() {
        let mode = PresentPacingMode::FixedTargetFps(120);
        assert_eq!(mode.target_fps(), Some(120));
        assert_eq!(PresentPacingMode::SyncToVblank.target_fps(), None);
    }

    #[test]
    fn upscaler_vendor_sdk_plan_with_dlss_unavailable_command_list_does_not_run() {
        let plan = UpscalerVendorSdkPlan::assemble(
            RenderStableId::new(1),
            UpscalerKind::Dlss,
            MotionVectorPassPlan::build(
                RenderStableId::new(1),
                RenderExtent2d::new(1280, 720),
                true,
            ),
            validated_bindings_for(UpscalerKind::Dlss),
            VendorSdkBridgeStatus::default(),
            PresentPacingPolicy::default(),
            Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT,
        );
        assert!(!plan.upscaler_can_run());
        assert!(!plan.dlss_truth.command_list_available);
    }

    #[test]
    fn upscaler_vendor_sdk_plan_with_native_kind_runs_without_command_list() {
        let plan = UpscalerVendorSdkPlan::assemble(
            RenderStableId::new(1),
            UpscalerKind::Native,
            MotionVectorPassPlan::build(
                RenderStableId::new(1),
                RenderExtent2d::new(1280, 720),
                false,
            ),
            validated_bindings_for(UpscalerKind::Native),
            VendorSdkBridgeStatus::default(),
            PresentPacingPolicy::default(),
            Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT,
        );
        assert!(plan.upscaler_can_run());
    }

    #[test]
    fn upscaler_vendor_sdk_plan_with_missing_inputs_blocks_run() {
        let mut bindings = UpscalerInputBindings::for_kind(UpscalerKind::Fsr);
        bindings.mark_present(UpscalerInputKind::Color);
        let plan = UpscalerVendorSdkPlan::assemble(
            RenderStableId::new(1),
            UpscalerKind::Fsr,
            MotionVectorPassPlan::build(
                RenderStableId::new(1),
                RenderExtent2d::new(1280, 720),
                true,
            ),
            bindings,
            VendorSdkBridgeStatus::default(),
            PresentPacingPolicy::default(),
            Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT,
        );
        assert!(!plan.upscaler_can_run());
    }

    #[test]
    fn upscaler_diagnostics_summarise_plan_state() {
        let plan = UpscalerVendorSdkPlan::assemble(
            RenderStableId::new(7),
            UpscalerKind::Dlss,
            MotionVectorPassPlan::build(
                RenderStableId::new(7),
                RenderExtent2d::new(1920, 1080),
                true,
            ),
            validated_bindings_for(UpscalerKind::Dlss),
            VendorSdkBridgeStatus::default(),
            PresentPacingPolicy::default(),
            Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT,
        );
        let diag = UpscalerVendorSdkDiagnostics::from_plan(&plan);
        assert_eq!(diag.upscaler_kind, UpscalerKind::Dlss);
        assert!(diag.bindings_validated);
        assert_eq!(diag.missing_input_count, 0);
        assert!(diag.dlss_truth_holds);
        assert!(diag.motion_vectors_supports_dlss_handoff);
        assert_eq!(diag.motion_vector_dispatch_x, 240);
        assert_eq!(diag.motion_vector_dispatch_y, 135);
    }

    #[test]
    fn vendor_sdk_kind_native_command_list_classification() {
        assert!(VendorSdkKind::Dlss.requires_native_command_list());
        assert!(VendorSdkKind::Reflex.requires_native_command_list());
        assert!(!VendorSdkKind::Fsr2.requires_native_command_list());
        assert!(!VendorSdkKind::XeSs.requires_native_command_list());
    }

    #[test]
    fn vendor_sdk_status_strings_match_pass_28_contract() {
        assert_eq!(VendorSdkStatus::Disabled.as_str(), "disabled");
        assert_eq!(VendorSdkStatus::SdkNotLinked.as_str(), "sdk_not_linked");
        assert_eq!(
            VendorSdkStatus::UnsupportedAdapter.as_str(),
            "unsupported_adapter",
        );
        assert_eq!(
            VendorSdkStatus::NativeCommandListUnavailable.as_str(),
            "native_command_list_unavailable",
        );
        assert_eq!(VendorSdkStatus::MissingInputs.as_str(), "missing_inputs");
        assert_eq!(VendorSdkStatus::Active.as_str(), "active");
        assert_eq!(VendorSdkStatus::FailedRuntime.as_str(), "failed_runtime");
    }
}
