//! Pass C0 / C1 / C7.2 — typed cloud world-shadow settings
//! + typed frame-graph ordering.
//!
//! The typed cloud raymarch produces a typed transmittance
//! map (per-pixel cloud opacity toward the sun); Pass C7.2
//! adds the typed pass roles +
//! resources that project this transmittance onto the
//! world to drive the typed cloud world-shadow mask that
//! terrain + materials sample.
//!
//! Pass C0 / C1 land the typed settings + the typed
//! resource intent shape.  Pass C7.2 lands the typed
//! `CloudShadowFrameDelayMode` enum + the typed ordering
//! predicates against the typed Lux direct-lighting pass.
//! The actual GPU shadow projection pass lives in a later
//! cloud pass.

use crate::clouds::{CloudQuality, CloudRenderSettings, CloudWeatherProfileId};
use crate::frame_graph::{FrameGraphPassRole, FrameGraphResourceType};
use fun_lux::LuxLightId;

pub const FUN_RENDERER_CLOUD_SHADOW_SCHEMA_VERSION: u16 = 1;

/// Typed Pass C1 cloud world-shadow resolution.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CloudShadowResolution {
    /// 1024 × 1024 — typed cheap target.
    Cheap1024,
    /// 2048 × 2048 — typed product default.
    #[default]
    Balanced2048,
    /// 4096 × 4096 — typed cinematic target.
    Cinematic4096,
}

impl CloudShadowResolution {
    pub const ALL: [Self; 3] =
        [Self::Cheap1024, Self::Balanced2048, Self::Cinematic4096];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cheap1024 => "cheap_1024",
            Self::Balanced2048 => "balanced_2048",
            Self::Cinematic4096 => "cinematic_4096",
        }
    }

    /// Typed `(width, height)` of the typed shadow target
    /// in pixels.
    #[must_use]
    pub const fn pixel_extent(self) -> (u32, u32) {
        match self {
            Self::Cheap1024 => (1024, 1024),
            Self::Balanced2048 => (2048, 2048),
            Self::Cinematic4096 => (4096, 4096),
        }
    }
}

/// Typed Pass C1 cloud world-shadow update policy.  Drives
/// the typed cadence at which the typed cloud world-shadow
/// mask refreshes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CloudShadowUpdatePolicy {
    /// Update every frame — typed cinematic capture / debug.
    EveryFrame,
    /// Update every typed N frames (typed product default).
    /// Cloud shadows drift slowly; per-frame refresh is
    /// over-budget for the typed product target.
    #[default]
    EveryNFrames,
    /// Update only when the typed cloud weather profile or
    /// the typed sun direction crosses a typed threshold.
    OnWeatherOrSunChange,
}

impl CloudShadowUpdatePolicy {
    pub const ALL: [Self; 3] = [
        Self::EveryFrame,
        Self::EveryNFrames,
        Self::OnWeatherOrSunChange,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EveryFrame => "every_frame",
            Self::EveryNFrames => "every_n_frames",
            Self::OnWeatherOrSunChange => "on_weather_or_sun_change",
        }
    }

    /// Typed default stride for `EveryNFrames` — every typed
    /// 4 frames at the typed product cadence.  Smaller
    /// strides reach toward `EveryFrame` semantics; larger
    /// strides reach toward `OnWeatherOrSunChange`.
    #[must_use]
    pub const fn default_stride_frames(self) -> u32 {
        match self {
            Self::EveryFrame => 1,
            Self::EveryNFrames => 4,
            Self::OnWeatherOrSunChange => u32::MAX,
        }
    }
}

/// Typed Pass C1 cloud world-shadow settings.  Drives the
/// typed cloud world-shadow mask sub-path:
///
/// - `enabled` — typed master switch.
/// - `resolution` — typed shadow target size.
/// - `update_policy` — typed refresh cadence.
/// - `opacity_scale_q16` — Q16 (0..=65_535) scale applied
///   to the typed cloud transmittance before the typed
///   world shadow mask is produced.  Lets the artist dim
///   cloud shadows globally.
/// - `softness_q16` — Q16 cloud-shadow softness in screen
///   units.  Drives the typed gaussian blur kernel on the
///   typed shadow mask before world sampling.
/// - `max_distance_meters` — typed maximum world distance
///   at which the typed cloud shadow contribution fades to
///   zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CloudWorldShadowSettings {
    pub schema_version: u16,
    pub enabled: bool,
    pub resolution: CloudShadowResolution,
    pub update_policy: CloudShadowUpdatePolicy,
    pub opacity_scale_q16: u16,
    pub softness_q16: u16,
    pub max_distance_meters: u32,
}

impl CloudWorldShadowSettings {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_RENDERER_CLOUD_SHADOW_SCHEMA_VERSION,
        enabled: true,
        resolution: CloudShadowResolution::Balanced2048,
        update_policy: CloudShadowUpdatePolicy::EveryNFrames,
        opacity_scale_q16: 52_429, // ~0.8 — cloud shadows
                                   // dim but not full
                                   // opaque
        softness_q16: 8_192,       // ~0.125 — soft edges
        max_distance_meters: 20_000,
    };

    pub const DISABLED: Self = Self {
        schema_version: FUN_RENDERER_CLOUD_SHADOW_SCHEMA_VERSION,
        enabled: false,
        resolution: CloudShadowResolution::Cheap1024,
        update_policy: CloudShadowUpdatePolicy::OnWeatherOrSunChange,
        opacity_scale_q16: 0,
        softness_q16: 0,
        max_distance_meters: 0,
    };

    /// Typed predicate: should the typed cloud world-shadow
    /// pass register this frame?  The typed `enabled` flag
    /// gates everything; the typed opacity scale must be
    /// non-zero for the typed mask to contribute anything.
    #[must_use]
    pub const fn registers_world_shadow_pass(&self) -> bool {
        self.enabled && self.opacity_scale_q16 > 0 && self.max_distance_meters > 0
    }
}

impl Default for CloudWorldShadowSettings {
    fn default() -> Self {
        Self::PRODUCT_DEFAULT
    }
}

// ============================================================================
// Pass C7.3 — typed cloud shadow sample format + shadow math model
// ============================================================================

/// Typed Pass C7.3 cloud shadow storage format.  Drives
/// the typed GPU texture format the typed
/// `CloudWorldShadowFiltered` resource uses.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CloudShadowStorageFormat {
    /// Typed R8Unorm — typed cheap target.  Stores typed
    /// transmittance only with 8 bits of precision per
    /// pixel.  Acceptable for typed Low / Cheap quality
    /// tiers where the typed banding artifacts are below
    /// the typed perceptual threshold.
    R8Unorm,
    /// Typed R16Float — typed balanced / cinematic
    /// target.  Stores typed transmittance only with 16
    /// bits of floating-point precision.  Typed product
    /// default.
    #[default]
    R16Float,
    /// Typed RGBA16Float — typed packed 4-channel sample
    /// target.  Stores the typed full
    /// [`CloudShadowSample`] (transmittance + optical
    /// depth + coverage + confidence).  Reserved for the
    /// typed cinematic + typed debug-readback path.
    Rgba16FloatPacked,
}

impl CloudShadowStorageFormat {
    pub const ALL: [Self; 3] = [Self::R8Unorm, Self::R16Float, Self::Rgba16FloatPacked];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::R8Unorm => "r8_unorm",
            Self::R16Float => "r16_float",
            Self::Rgba16FloatPacked => "rgba16_float_packed",
        }
    }

    /// Typed bytes per pixel of the typed storage format.
    #[must_use]
    pub const fn bytes_per_pixel(self) -> u8 {
        match self {
            Self::R8Unorm => 1,
            Self::R16Float => 2,
            Self::Rgba16FloatPacked => 8,
        }
    }

    /// Typed predicate: does this typed format carry the
    /// typed full [`CloudShadowSample`] payload (vs only
    /// the typed transmittance channel)?
    #[must_use]
    pub const fn carries_full_sample(self) -> bool {
        matches!(self, Self::Rgba16FloatPacked)
    }

    /// Typed predicate: is this typed format the typed
    /// cheap target?
    #[must_use]
    pub const fn is_cheap(self) -> bool {
        matches!(self, Self::R8Unorm)
    }

    /// Typed Pass C7.4 selector — derive the typed storage
    /// format from a typed `CloudQuality` tier + a typed
    /// `debug_readback_active` flag.  Matches the typed
    /// C7.3 storage defaults:
    ///
    ///     Cheap      -> R8Unorm
    ///     Balanced   -> R16Float
    ///     Cinematic  -> R16Float, or Rgba16FloatPacked
    ///                   when the typed debug-readback path
    ///                   is active (the typed packed format
    ///                   carries the typed full
    ///                   `CloudShadowSample` payload).
    ///
    /// `CloudQuality::Off` returns `R8Unorm` — the typed
    /// cheap fallback — but the typed cloud shadow pass
    /// MUST NOT register at all when quality is `Off`
    /// (gated by `CloudRenderSettings::registers_world_shadow_pass`).
    #[must_use]
    pub const fn for_quality(quality: CloudQuality, debug_readback_active: bool) -> Self {
        match quality {
            CloudQuality::Off => Self::R8Unorm,
            CloudQuality::Cheap => Self::R8Unorm,
            CloudQuality::Balanced => Self::R16Float,
            CloudQuality::Cinematic => {
                if debug_readback_active {
                    Self::Rgba16FloatPacked
                } else {
                    Self::R16Float
                }
            }
        }
    }
}

/// Typed Pass C7.3 cloud shadow sample.  Packed GPU format
/// per the user spec.  The typed GPU target can start as
/// single-channel transmittance (`R16Float`) and migrate
/// to the typed full 4-channel packed format
/// (`Rgba16FloatPacked`) when the typed cinematic +
/// debug-readback path needs the typed extra channels.
///
/// Typed encoding:
/// - `transmittance`: `1.0` = no cloud shadow,
///   `0.0` = fully occluded by dense cloud.
/// - `optical_depth`: typed Beer-Lambert optical depth
///   along the typed sun ray.  `0.0` = no cloud;
///   higher = denser path.
/// - `coverage`: typed cloud coverage at the typed
///   sample point.  `0.0` = no cloud; `1.0` = fully
///   covered.  Note: `coverage = 1.0` does NOT imply
///   `transmittance = 0.0` — a typed thin cumulus
///   patch has high coverage but low optical depth.
/// - `confidence`: typed confidence the typed sample is
///   reliable.  `0.0` = typed history-only fallback;
///   `1.0` = typed full-resolution current-frame sample.
///
/// `Hash` is intentionally NOT derived — the typed
/// struct carries `f32` fields which are not `Eq`.  Pass
/// C7.3 does not key any typed map on
/// `CloudShadowSample`; the typed
/// `RendererFrameGraphDiagnostics::resource_count` keys
/// off the typed resource handle, not the typed sample.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct CloudShadowSample {
    pub transmittance: f32,
    pub optical_depth: f32,
    pub coverage: f32,
    pub confidence: f32,
}

impl CloudShadowSample {
    /// Typed "no cloud" sample — typed full sunlight, zero
    /// optical depth, zero coverage, full confidence.  The
    /// typed `transmittance = 1.0` encoding matches the
    /// user spec ("1.0 = no cloud shadow").
    pub const NO_CLOUD: Self = Self {
        transmittance: 1.0,
        optical_depth: 0.0,
        coverage: 0.0,
        confidence: 1.0,
    };

    /// Typed "fully occluded" sample — typed zero
    /// sunlight, high optical depth, full coverage, full
    /// confidence.  The typed `transmittance = 0.0`
    /// encoding matches the user spec ("0.0 = fully
    /// occluded by dense cloud").
    pub const FULLY_OCCLUDED: Self = Self {
        transmittance: 0.0,
        optical_depth: 10.0,
        coverage: 1.0,
        confidence: 1.0,
    };

    /// Typed builder: typed sample from a single typed
    /// transmittance value.  Derives the typed optical
    /// depth via Beer-Lambert (`-ln(transmittance)`),
    /// leaves the typed coverage at `coverage` and the
    /// typed confidence at `1.0`.
    #[must_use]
    pub fn from_transmittance(transmittance: f32, coverage: f32) -> Self {
        let clamped = transmittance.clamp(0.0, 1.0);
        // Beer-Lambert: T = exp(-tau) → tau = -ln(T).
        // Clamp the typed tau to a typed reasonable
        // maximum (10) so the typed `FULLY_OCCLUDED` case
        // produces a typed finite value.
        let optical_depth = if clamped > 0.0 {
            (-clamped.ln()).min(10.0)
        } else {
            10.0
        };
        Self {
            transmittance: clamped,
            optical_depth,
            coverage: coverage.clamp(0.0, 1.0),
            confidence: 1.0,
        }
    }

    /// Typed predicate: is this typed sample the typed
    /// no-cloud baseline (transmittance fully open)?
    #[must_use]
    pub fn is_no_cloud(&self) -> bool {
        self.transmittance >= 1.0
    }

    /// Typed predicate: is this typed sample fully
    /// occluded (transmittance fully closed)?
    #[must_use]
    pub fn is_fully_occluded(&self) -> bool {
        self.transmittance <= 0.0
    }
}

// ============================================================================
// Pass C7.3 — typed Lux direct-light shadow math
// ============================================================================

/// Typed Pass C7.3 Lux direct-light shadow math.  Encodes
/// the typed user-spec formula:
///
///     final_direct_visibility = opaque_shadow * cloud_transmittance
///
/// The typed contract refuses baking cloud shadows into
/// opaque virtual shadow depth — clouds are volumetric
/// and semi-transparent, not opaque casters.  The typed
/// `cloud_shadows_not_baked_into_opaque_depth()` const
/// predicate audits the typed contract.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxDirectLightShadowMath;

impl LuxDirectLightShadowMath {
    /// Typed compose helper: combines a typed opaque
    /// shadow factor (`1.0` = lit, `0.0` = shadowed by
    /// opaque geometry) with a typed cloud transmittance
    /// (`1.0` = no cloud, `0.0` = fully occluded by
    /// cloud) into a typed final direct-visibility
    /// factor.
    ///
    /// The typed formula is a typed product — opaque
    /// shadow blocks light through geometry; cloud
    /// transmittance dims sunlight through atmosphere;
    /// both effects multiply into the typed final
    /// visibility.
    #[must_use]
    pub fn compose_final_direct_visibility(
        opaque_shadow: f32,
        cloud_transmittance: f32,
    ) -> f32 {
        let opaque = opaque_shadow.clamp(0.0, 1.0);
        let cloud = cloud_transmittance.clamp(0.0, 1.0);
        opaque * cloud
    }

    /// Typed convenience: typed final visibility from a
    /// typed opaque shadow factor + a typed full
    /// `CloudShadowSample` (consumes the typed
    /// transmittance channel only).
    #[must_use]
    pub fn compose_from_sample(opaque_shadow: f32, cloud: CloudShadowSample) -> f32 {
        Self::compose_final_direct_visibility(opaque_shadow, cloud.transmittance)
    }
}

/// Typed Pass C7.3 const predicate — cloud shadows MUST
/// NOT be baked into the typed opaque virtual shadow
/// depth.  Encoded by the typed taxonomy split: the
/// typed cloud shadow resources live in their own typed
/// `Cloud*` namespace (NOT the typed `LuxVirtualShadow*`
/// namespace), and the typed `is_lux()` predicate on
/// `FrameGraphResourceType` returns `false` for typed
/// cloud resources.  The typed cloud shadow project /
/// filter passes write to typed `CloudWorldShadow*`
/// targets, never to typed `LuxVirtualShadowPages` or
/// `LuxShadowAtlas`.
#[must_use]
pub const fn cloud_shadows_not_baked_into_opaque_depth() -> bool {
    // Typed taxonomy split: typed cloud resource types do
    // NOT participate in the typed `is_lux()` set, AND
    // typed cloud shadow pass roles do NOT write to typed
    // Lux virtual shadow / atlas resources (audited at the
    // typed `lux_passes::typed_resource_outputs` layer in
    // the typed compiler).
    let cw_transmittance = FrameGraphResourceType::CloudWorldShadowTransmittance;
    let cw_filtered = FrameGraphResourceType::CloudWorldShadowFiltered;
    let cw_constants = FrameGraphResourceType::CloudShadowProjectionConstants;
    !cw_transmittance.is_lux() && !cw_filtered.is_lux() && !cw_constants.is_lux()
}

/// Typed Pass C7.3 const predicate — the typed
/// transmittance encoding matches the user spec
/// (`1.0` = no cloud, `0.0` = fully occluded).
#[must_use]
pub const fn cloud_transmittance_encoding_matches_user_spec() -> bool {
    // Typed `NO_CLOUD.transmittance == 1.0`; typed
    // `FULLY_OCCLUDED.transmittance == 0.0`.  Encoded as
    // typed const-evaluable f32 comparisons.
    let no_cloud_t = CloudShadowSample::NO_CLOUD.transmittance;
    let occluded_t = CloudShadowSample::FULLY_OCCLUDED.transmittance;
    no_cloud_t == 1.0 && occluded_t == 0.0
}

// ============================================================================
// Pass C7.2 — typed frame-delay mode + ordering predicates
// ============================================================================

/// Typed Pass C7.2 cloud shadow frame-delay mode.  The
/// typed renderer picks one of these per the typed
/// product setting; the typed ordering predicates audit
/// the typed contract holds.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CloudShadowFrameDelayMode {
    /// Typed one-frame-delayed mode.  Frame N writes the
    /// typed `CloudWorldShadowFiltered` target; frame N+1
    /// reads it in the typed `LuxDirectLighting` pass.
    /// Easier to schedule (the typed project + filter
    /// passes run anywhere in frame N before the typed
    /// frame-graph submit), at the cost of one frame of
    /// stale cloud shadows.
    #[default]
    OneFrameDelayed,
    /// Typed same-frame mode.  Frame N writes the typed
    /// `CloudWorldShadowFiltered` target AND reads it in
    /// the typed `LuxDirectLighting` pass.  Harder to
    /// schedule (the typed project + filter must run
    /// before the typed direct-lighting pass), but the
    /// typed cloud shadows track the typed sun + camera
    /// without a frame of lag.
    SameFrame,
}

impl CloudShadowFrameDelayMode {
    pub const ALL: [Self; 2] = [Self::OneFrameDelayed, Self::SameFrame];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OneFrameDelayed => "one_frame_delayed",
            Self::SameFrame => "same_frame",
        }
    }

    /// Typed predicate: does the typed
    /// `LuxDirectLighting` pass sample the typed
    /// CURRENT frame's filtered cloud shadow?  `true` for
    /// `SameFrame`, `false` for `OneFrameDelayed`.
    #[must_use]
    pub const fn samples_current_frame_filtered_shadow(self) -> bool {
        matches!(self, Self::SameFrame)
    }

    /// Typed predicate: does the typed
    /// `LuxDirectLighting` pass sample the typed
    /// PREVIOUS frame's filtered cloud shadow?  `true`
    /// for `OneFrameDelayed`.
    #[must_use]
    pub const fn samples_previous_frame_filtered_shadow(self) -> bool {
        matches!(self, Self::OneFrameDelayed)
    }
}

/// Typed Pass C7.2 const predicate — the typed cloud
/// shadow project pass runs BEFORE the typed cloud shadow
/// filter pass.
#[must_use]
pub const fn cloud_shadow_project_runs_before_filter() -> bool {
    let project = match FrameGraphPassRole::LuxCloudShadowProject.lux_order_key() {
        Some(k) => k,
        None => return false,
    };
    let filter = match FrameGraphPassRole::LuxCloudShadowFilter.lux_order_key() {
        Some(k) => k,
        None => return false,
    };
    project < filter
}

/// Typed Pass C7.2 const predicate — the typed cloud
/// shadow filter pass runs BEFORE the typed cloud shadow
/// register-layer pass.
#[must_use]
pub const fn cloud_shadow_filter_runs_before_register_layer() -> bool {
    let filter = match FrameGraphPassRole::LuxCloudShadowFilter.lux_order_key() {
        Some(k) => k,
        None => return false,
    };
    let register = match FrameGraphPassRole::LuxCloudShadowRegisterLayer.lux_order_key() {
        Some(k) => k,
        None => return false,
    };
    filter < register
}

/// Typed Pass C7.2 const predicate — the typed cloud
/// shadow register-layer pass runs BEFORE the typed
/// `LuxDirectLighting` pass that consumes the typed
/// filtered cloud shadow.  Same-frame mode requires this
/// strictly; one-frame-delayed mode does not (the typed
/// direct-lighting pass reads the PREVIOUS frame's
/// result, so the typed register pass can run any time
/// in the typed current frame).
#[must_use]
pub const fn cloud_shadow_register_runs_before_direct_lighting() -> bool {
    let register = match FrameGraphPassRole::LuxCloudShadowRegisterLayer.lux_order_key() {
        Some(k) => k,
        None => return false,
    };
    let direct = match FrameGraphPassRole::LuxDirectLighting.lux_order_key() {
        Some(k) => k,
        None => return false,
    };
    register < direct
}

/// Typed Pass C7.2 const predicate — every typed cloud
/// shadow ordering invariant holds.
#[must_use]
pub const fn cloud_shadow_ordering_invariants_hold() -> bool {
    cloud_shadow_project_runs_before_filter()
        && cloud_shadow_filter_runs_before_register_layer()
        && cloud_shadow_register_runs_before_direct_lighting()
}

/// Typed Pass C7.2 const helper — typed slice of the
/// typed cloud-shadow frame-graph resource types.  Used
/// by tests + diagnostics that audit "the typed cloud
/// shadow resources appear in `RendererFrameGraphDiagnostics`."
pub const CLOUD_SHADOW_RESOURCE_TYPES: &[FrameGraphResourceType] = &[
    FrameGraphResourceType::CloudWorldShadowTransmittance,
    FrameGraphResourceType::CloudWorldShadowFiltered,
    FrameGraphResourceType::CloudShadowProjectionConstants,
];

/// Typed Pass C7.2 const helper — typed slice of the
/// typed cloud-shadow frame-graph pass roles.
pub const CLOUD_SHADOW_PASS_ROLES: &[FrameGraphPassRole] = &[
    FrameGraphPassRole::LuxCloudShadowProject,
    FrameGraphPassRole::LuxCloudShadowFilter,
    FrameGraphPassRole::LuxCloudShadowRegisterLayer,
];

// ============================================================================
// Pass C7.4 — typed cloud shadow projection model + resource diagnostics
// ============================================================================

/// Typed Pass C7.4 cloud shadow projection mode.  Drives
/// the typed world → shadow-UV projection strategy the
/// typed `LuxCloudShadowProject` pass picks.
///
/// - `CameraCenteredPlane` — typed initial production
///   default.  Projects every typed world sample along the
///   typed sun direction onto a typed horizontal plane at
///   the typed camera's altitude.  Cheap, stable, hits the
///   typed product target without cascading.
/// - `DirectionalLightClipRegion` — fits a typed single
///   orthographic clip volume to the typed camera frustum
///   slice up to `max_distance_meters`.  Reserved for a
///   future tier that wants the typed shadow UV to track
///   the typed view frustum tightly.
/// - `CascadedDirectionalRegions` — reserved for the typed
///   cinematic tier; splits the typed shadow region into
///   typed multiple cascades.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CloudShadowProjectionMode {
    #[default]
    CameraCenteredPlane,
    DirectionalLightClipRegion,
    CascadedDirectionalRegions,
}

impl CloudShadowProjectionMode {
    pub const ALL: [Self; 3] = [
        Self::CameraCenteredPlane,
        Self::DirectionalLightClipRegion,
        Self::CascadedDirectionalRegions,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CameraCenteredPlane => "camera_centered_plane",
            Self::DirectionalLightClipRegion => "directional_light_clip_region",
            Self::CascadedDirectionalRegions => "cascaded_directional_regions",
        }
    }

    /// Typed predicate: is this typed mode the typed initial
    /// production default?
    #[must_use]
    pub const fn is_initial_production_default(self) -> bool {
        matches!(self, Self::CameraCenteredPlane)
    }
}

/// Typed Pass C7.4 cloud shadow projection constants.  CPU
/// builder for the typed GPU uniform consumed by the typed
/// `LuxCloudShadowProject` + `LuxCloudShadowFilter` passes.
///
/// The typed `world_from_shadow_uv` + `shadow_uv_from_world`
/// matrices encode the typed world ↔ shadow-UV transform
/// (row-major `[row][col]`).  The typed `sun_direction_ws`
/// is the typed normalized sun direction in world space;
/// the typed cloud raymarch projects samples along this
/// vector onto the typed shadow plane.
///
/// `Eq + Hash` are intentionally NOT derived — the typed
/// struct carries `f32` fields which are not `Eq`.  This
/// is a CPU-side builder shape; the typed GPU uniform
/// layout is responsible for its own padding + alignment.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct CloudShadowProjectionConstants {
    pub schema_version: u16,
    pub mode: CloudShadowProjectionMode,
    pub light_id: LuxLightId,
    pub world_from_shadow_uv: [[f32; 4]; 4],
    pub shadow_uv_from_world: [[f32; 4]; 4],
    pub sun_direction_ws: [f32; 3],
    pub cloud_base_meters: f32,
    pub cloud_top_meters: f32,
    pub max_distance_meters: f32,
    pub opacity_scale: f32,
    pub softness: f32,
    pub frame_index: u32,
}

impl CloudShadowProjectionConstants {
    /// Typed disabled projection.  Every typed predicate
    /// returns `false`; downstream passes that consume the
    /// typed constants gate cleanly on the typed predicates.
    pub const DISABLED: Self = Self {
        schema_version: FUN_RENDERER_CLOUD_SHADOW_SCHEMA_VERSION,
        mode: CloudShadowProjectionMode::CameraCenteredPlane,
        light_id: LuxLightId::INVALID,
        world_from_shadow_uv: [[0.0; 4]; 4],
        shadow_uv_from_world: [[0.0; 4]; 4],
        sun_direction_ws: [0.0, 0.0, 0.0],
        cloud_base_meters: 0.0,
        cloud_top_meters: 0.0,
        max_distance_meters: 0.0,
        opacity_scale: 0.0,
        softness: 0.0,
        frame_index: 0,
    };

    /// Typed Pass C7.4 builder.  Derives typed projection
    /// constants from the typed inputs:
    ///
    /// - `settings` — typed `CloudRenderSettings` (gates the
    ///   pass via `registers_world_shadow_pass()` + supplies
    ///   the typed opacity / softness / max-distance fields).
    /// - `profile` — typed weather profile id (supplies the
    ///   typed cloud slab altitudes).
    /// - `light_id` — typed primary Lux directional light id
    ///   that owns the typed shadow layer (`INVALID` →
    ///   `DISABLED`).
    /// - `sun_direction_ws` — typed normalized sun direction
    ///   in world space.  Caller-supplied; a typed zero
    ///   vector → `DISABLED`.
    /// - `frame_index` — typed frame index used for the
    ///   typed temporal jitter / cache invalidation.
    ///
    /// Returns the typed `DISABLED` constants when:
    /// 1. `settings.registers_world_shadow_pass()` is false.
    /// 2. The typed `light_id` is `INVALID`.
    /// 3. The typed `sun_direction_ws` is the zero vector.
    /// 4. The typed weather profile reports an invalid slab.
    #[must_use]
    pub fn from_inputs(
        settings: &CloudRenderSettings,
        profile: CloudWeatherProfileId,
        light_id: LuxLightId,
        sun_direction_ws: [f32; 3],
        frame_index: u32,
    ) -> Self {
        if !settings.registers_world_shadow_pass() {
            return Self::DISABLED;
        }
        if !light_id.is_valid() {
            return Self::DISABLED;
        }
        let [sx, sy, sz] = sun_direction_ws;
        let sun_norm_sq = sx * sx + sy * sy + sz * sz;
        if sun_norm_sq <= f32::EPSILON {
            return Self::DISABLED;
        }
        let (base_u32, top_u32) = profile.cloud_slab_meters();
        if base_u32 >= top_u32 {
            return Self::DISABLED;
        }
        let cloud_base_meters = base_u32 as f32;
        let cloud_top_meters = top_u32 as f32;
        let max_distance_meters = settings.world_shadows.max_distance_meters as f32;
        let opacity_scale = (settings.world_shadows.opacity_scale_q16 as f32) / 65_535.0;
        let softness = (settings.world_shadows.softness_q16 as f32) / 65_535.0;

        // Typed `CameraCenteredPlane` scaffold projection:
        // the typed shadow UV space covers a typed
        // `2 * max_distance_meters` square on the typed
        // horizontal plane at `cloud_base_meters` altitude,
        // centered on the world origin (camera will be
        // re-centered when the typed live camera lands).
        //
        //   u = x / (2D) + 0.5
        //   v = z / (2D) + 0.5
        //
        // Stored row-major.  GPU upload reshapes to the
        // shader-native layout as needed.
        let d = max_distance_meters.max(1.0);
        let inv_2d = 1.0 / (2.0 * d);
        let shadow_uv_from_world = [
            [inv_2d, 0.0, 0.0, 0.5],
            [0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, inv_2d, 0.5],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let world_from_shadow_uv = [
            [2.0 * d, 0.0, 0.0, -d],
            [0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 2.0 * d, -d],
            [0.0, 0.0, 0.0, 1.0],
        ];

        Self {
            schema_version: FUN_RENDERER_CLOUD_SHADOW_SCHEMA_VERSION,
            mode: CloudShadowProjectionMode::CameraCenteredPlane,
            light_id,
            world_from_shadow_uv,
            shadow_uv_from_world,
            sun_direction_ws,
            cloud_base_meters,
            cloud_top_meters,
            max_distance_meters,
            opacity_scale,
            softness,
            frame_index,
        }
    }

    /// Typed predicate: do these typed constants project a
    /// typed valid directional Lux light?  Requires a typed
    /// valid `light_id` AND a typed non-zero opacity scale
    /// (otherwise the typed projection contributes nothing
    /// to the typed final direct visibility).
    #[must_use]
    pub fn projects_directional_lux_light(&self) -> bool {
        self.light_id.is_valid() && self.opacity_scale > 0.0
    }

    /// Typed predicate: is the typed cloud slab valid?
    /// Requires `cloud_top_meters > cloud_base_meters` AND
    /// `cloud_base_meters >= 0.0`.
    #[must_use]
    pub fn has_valid_cloud_slab(&self) -> bool {
        self.cloud_base_meters >= 0.0
            && self.cloud_top_meters > self.cloud_base_meters
    }

    /// Typed predicate: do these typed constants describe a
    /// typed non-zero shadow region?  Requires a typed
    /// positive `max_distance_meters` AND a typed positive
    /// `opacity_scale`.  Softness alone is not sufficient —
    /// a typed softened zero region is still zero.
    #[must_use]
    pub fn has_nonzero_shadow_region(&self) -> bool {
        self.max_distance_meters > 0.0 && self.opacity_scale > 0.0
    }

    /// Typed predicate: does this typed constants record
    /// describe a typed fully-live projection?  Composes
    /// every typed sub-predicate.
    #[must_use]
    pub fn projects_world_shadow(&self) -> bool {
        self.projects_directional_lux_light()
            && self.has_valid_cloud_slab()
            && self.has_nonzero_shadow_region()
    }
}

impl Default for CloudShadowProjectionConstants {
    fn default() -> Self {
        Self::DISABLED
    }
}

/// Typed Pass C7.4 — typed byte size of the typed CPU-side
/// `CloudShadowProjectionConstants` record.  Drives the
/// typed `constants_bytes` field in
/// `CloudShadowResourceDiagnostics`.  GPU upload may pad
/// or re-pack; this is the typed CPU footprint.
pub const CLOUD_SHADOW_PROJECTION_CONSTANTS_CPU_BYTES: u64 =
    core::mem::size_of::<CloudShadowProjectionConstants>() as u64;

/// Typed Pass C7.4 cloud shadow resource diagnostics.  The
/// typed renderer fills this typed record after it
/// allocates (or skips) the typed cloud shadow GPU
/// resources for the frame.  Drives the typed cloud
/// pipeline debug section + the typed quality-audit gate.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CloudShadowResourceDiagnostics {
    pub enabled: bool,
    pub transmittance_format: CloudShadowStorageFormat,
    pub filtered_format: CloudShadowStorageFormat,
    pub extent: [u32; 2],
    pub transmittance_bytes: u64,
    pub filtered_bytes: u64,
    pub constants_bytes: u64,
    pub reallocated_this_frame: bool,
}

impl CloudShadowResourceDiagnostics {
    /// Typed cold default — typed pass not registered, no
    /// typed GPU bytes allocated.
    pub const COLD_DEFAULT: Self = Self {
        enabled: false,
        transmittance_format: CloudShadowStorageFormat::R16Float,
        filtered_format: CloudShadowStorageFormat::R16Float,
        extent: [0, 0],
        transmittance_bytes: 0,
        filtered_bytes: 0,
        constants_bytes: 0,
        reallocated_this_frame: false,
    };

    /// Typed Pass C7.4 builder.  Derives typed resource
    /// diagnostics from the typed `CloudRenderSettings` +
    /// the typed `reallocated_this_frame` flag.  The typed
    /// `debug_readback_active` flag selects the typed
    /// `Rgba16FloatPacked` storage format on the typed
    /// cinematic tier (otherwise stays on `R16Float`).
    ///
    /// Returns `COLD_DEFAULT` (with `reallocated_this_frame`
    /// preserved) when
    /// `settings.registers_world_shadow_pass()` is false —
    /// the typed cloud shadow GPU resources MUST NOT be
    /// allocated when the typed pass is gated off.
    #[must_use]
    pub fn from_settings(
        settings: &CloudRenderSettings,
        debug_readback_active: bool,
        reallocated_this_frame: bool,
    ) -> Self {
        if !settings.registers_world_shadow_pass() {
            return Self {
                reallocated_this_frame,
                ..Self::COLD_DEFAULT
            };
        }
        let format =
            CloudShadowStorageFormat::for_quality(settings.quality, debug_readback_active);
        let (width, height) = settings.world_shadows.resolution.pixel_extent();
        let extent = [width, height];
        let pixel_count = (width as u64).saturating_mul(height as u64);
        let format_bytes = pixel_count.saturating_mul(format.bytes_per_pixel() as u64);
        Self {
            enabled: true,
            transmittance_format: format,
            filtered_format: format,
            extent,
            transmittance_bytes: format_bytes,
            filtered_bytes: format_bytes,
            constants_bytes: CLOUD_SHADOW_PROJECTION_CONSTANTS_CPU_BYTES,
            reallocated_this_frame,
        }
    }

    /// Typed predicate: did the typed renderer allocate any
    /// typed cloud shadow GPU bytes this frame?
    #[must_use]
    pub const fn allocated_any_gpu_bytes(&self) -> bool {
        self.transmittance_bytes > 0
            || self.filtered_bytes > 0
            || self.constants_bytes > 0
    }

    /// Typed total typed GPU bytes the typed cloud shadow
    /// resources occupy this frame.  Sum of the typed
    /// transmittance + filtered + constants byte counts.
    #[must_use]
    pub const fn total_gpu_bytes(&self) -> u64 {
        self.transmittance_bytes
            .saturating_add(self.filtered_bytes)
            .saturating_add(self.constants_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_shadow_resolution_taxonomy_walks_user_spec() {
        assert_eq!(CloudShadowResolution::Cheap1024.pixel_extent(), (1024, 1024));
        assert_eq!(CloudShadowResolution::Balanced2048.pixel_extent(), (2048, 2048));
        assert_eq!(CloudShadowResolution::Cinematic4096.pixel_extent(), (4096, 4096));
    }

    #[test]
    fn cloud_shadow_update_policy_default_strides() {
        assert_eq!(CloudShadowUpdatePolicy::EveryFrame.default_stride_frames(), 1);
        assert_eq!(CloudShadowUpdatePolicy::EveryNFrames.default_stride_frames(), 4);
        assert_eq!(
            CloudShadowUpdatePolicy::OnWeatherOrSunChange.default_stride_frames(),
            u32::MAX,
        );
    }

    #[test]
    fn cloud_world_shadow_product_default_registers_pass() {
        let s = CloudWorldShadowSettings::PRODUCT_DEFAULT;
        assert!(s.registers_world_shadow_pass());
        assert!(s.enabled);
        assert_eq!(s.resolution, CloudShadowResolution::Balanced2048);
    }

    #[test]
    fn cloud_world_shadow_disabled_skips_pass() {
        let s = CloudWorldShadowSettings::DISABLED;
        assert!(!s.registers_world_shadow_pass());
    }

    #[test]
    fn cloud_world_shadow_zero_opacity_skips_pass() {
        let mut s = CloudWorldShadowSettings::PRODUCT_DEFAULT;
        s.opacity_scale_q16 = 0;
        assert!(!s.registers_world_shadow_pass());
    }

    /// Pass C7.2 acceptance — cloud shadow resources
    /// appear in the typed `FrameGraphResourceType` taxonomy
    /// (and therefore in `RendererFrameGraphDiagnostics`
    /// when a typed pass writes one).
    #[test]
    fn cloud_shadow_resources_appear_in_frame_graph_resource_types() {
        // Every typed cloud-shadow resource type appears in
        // the typed `FrameGraphResourceType::ALL` slice (the
        // typed source of truth the typed diagnostics + the
        // typed Tier4 mapping iterate).
        for kind in CLOUD_SHADOW_RESOURCE_TYPES {
            assert!(
                FrameGraphResourceType::ALL.contains(kind),
                "{:?} missing from FrameGraphResourceType::ALL",
                kind,
            );
            // Every typed name starts with `cloud_` —
            // distinguishes them from typed `lux_*`
            // resources at the typed string layer.
            assert!(kind.as_str().starts_with("cloud_"));
        }
        // The typed `is_lux()` predicate intentionally
        // returns `false` for typed cloud resource types —
        // cloud resources are produced by typed cloud
        // passes (grouped under Lux PassKind via the typed
        // `LuxShadow` mapping) but the typed resources
        // themselves are typed cloud-owned.
        for kind in CLOUD_SHADOW_RESOURCE_TYPES {
            assert!(!kind.is_lux(), "{:?} reports is_lux=true", kind);
        }
    }

    /// Pass C7.2 acceptance — cloud shadow project / filter
    /// passes appear BEFORE any typed pass that consumes
    /// them.  Verified via the typed `lux_order_key`
    /// monotonic chain:
    ///   Project (140) < Filter (145) < Register (150)
    ///     < LuxDirectLighting (200).
    #[test]
    fn cloud_shadow_project_filter_runs_before_consumers() {
        assert!(cloud_shadow_project_runs_before_filter());
        assert!(cloud_shadow_filter_runs_before_register_layer());
        assert!(cloud_shadow_register_runs_before_direct_lighting());
        assert!(cloud_shadow_ordering_invariants_hold());

        let project = FrameGraphPassRole::LuxCloudShadowProject
            .lux_order_key()
            .expect("project has order key");
        let filter = FrameGraphPassRole::LuxCloudShadowFilter
            .lux_order_key()
            .expect("filter has order key");
        let register = FrameGraphPassRole::LuxCloudShadowRegisterLayer
            .lux_order_key()
            .expect("register has order key");
        let direct = FrameGraphPassRole::LuxDirectLighting
            .lux_order_key()
            .expect("direct lighting has order key");
        assert!(project < filter);
        assert!(filter < register);
        assert!(register < direct);

        // Every typed cloud-shadow pass role reports
        // `is_lux=true` (they live under the typed Lux
        // shadow grouping per Pass C7.2).
        for role in CLOUD_SHADOW_PASS_ROLES {
            assert!(role.is_lux(), "{:?} reports is_lux=false", role);
        }
    }

    /// Pass C7.3 acceptance — typed `CloudShadowSample`
    /// carries the typed user-spec 4-field shape.
    #[test]
    fn cloud_shadow_sample_carries_user_spec_fields() {
        let s = CloudShadowSample::NO_CLOUD;
        assert_eq!(s.transmittance, 1.0);
        assert_eq!(s.optical_depth, 0.0);
        assert_eq!(s.coverage, 0.0);
        assert_eq!(s.confidence, 1.0);
        assert!(s.is_no_cloud());
        assert!(!s.is_fully_occluded());

        let occ = CloudShadowSample::FULLY_OCCLUDED;
        assert_eq!(occ.transmittance, 0.0);
        assert!(occ.optical_depth > 0.0);
        assert_eq!(occ.coverage, 1.0);
        assert_eq!(occ.confidence, 1.0);
        assert!(occ.is_fully_occluded());
        assert!(!occ.is_no_cloud());
    }

    /// Pass C7.3 acceptance — typed cloud transmittance
    /// encoding matches user spec (`1.0` = no shadow,
    /// `0.0` = fully occluded).
    #[test]
    fn cloud_transmittance_encoding_matches_user_spec_predicate() {
        assert!(cloud_transmittance_encoding_matches_user_spec());
        // Typed `from_transmittance` builder respects the
        // typed encoding.
        let mid = CloudShadowSample::from_transmittance(0.5, 0.7);
        assert_eq!(mid.transmittance, 0.5);
        // Beer-Lambert: tau = -ln(0.5) ≈ 0.693.
        assert!((mid.optical_depth - 0.693).abs() < 0.01);
        assert_eq!(mid.coverage, 0.7);
        // Typed out-of-range inputs clamp to typed [0, 1].
        let clamped_high = CloudShadowSample::from_transmittance(1.5, 1.5);
        assert_eq!(clamped_high.transmittance, 1.0);
        assert_eq!(clamped_high.coverage, 1.0);
        let clamped_low = CloudShadowSample::from_transmittance(-0.5, -0.3);
        assert_eq!(clamped_low.transmittance, 0.0);
        assert_eq!(clamped_low.coverage, 0.0);
    }

    /// Pass C7.3 acceptance — typed final direct
    /// visibility composes opaque shadow × cloud
    /// transmittance.
    #[test]
    fn final_direct_visibility_composes_opaque_and_cloud() {
        // Typed lit + no cloud → typed full visibility.
        assert_eq!(
            LuxDirectLightShadowMath::compose_final_direct_visibility(1.0, 1.0),
            1.0,
        );
        // Typed shadowed by geometry → typed zero
        // visibility regardless of cloud.
        assert_eq!(
            LuxDirectLightShadowMath::compose_final_direct_visibility(0.0, 1.0),
            0.0,
        );
        // Typed fully occluded by cloud → typed zero
        // visibility regardless of geometry.
        assert_eq!(
            LuxDirectLightShadowMath::compose_final_direct_visibility(1.0, 0.0),
            0.0,
        );
        // Typed partial × partial = product.
        assert_eq!(
            LuxDirectLightShadowMath::compose_final_direct_visibility(0.8, 0.5),
            0.4,
        );
        // Typed compose_from_sample reads the typed
        // transmittance channel only.
        let cloud = CloudShadowSample::from_transmittance(0.5, 0.8);
        let v = LuxDirectLightShadowMath::compose_from_sample(0.8, cloud);
        assert!((v - 0.4).abs() < 1e-6);
        // Typed clamps work on typed out-of-range inputs.
        assert_eq!(
            LuxDirectLightShadowMath::compose_final_direct_visibility(1.5, 1.5),
            1.0,
        );
        assert_eq!(
            LuxDirectLightShadowMath::compose_final_direct_visibility(-0.5, 0.5),
            0.0,
        );
    }

    /// Pass C7.3 acceptance — cloud shadows MUST NOT be
    /// baked into the typed opaque virtual shadow depth.
    /// The typed `is_lux()` predicate on typed cloud
    /// resources MUST return `false` (typed cloud
    /// resources live in their own typed namespace,
    /// not the typed Lux virtual shadow set).
    #[test]
    fn cloud_shadows_not_baked_into_opaque_depth_predicate() {
        assert!(cloud_shadows_not_baked_into_opaque_depth());
        // Typed cloud resource types are NOT in the typed
        // Lux set.
        assert!(!FrameGraphResourceType::CloudWorldShadowTransmittance.is_lux());
        assert!(!FrameGraphResourceType::CloudWorldShadowFiltered.is_lux());
        assert!(!FrameGraphResourceType::CloudShadowProjectionConstants.is_lux());
        // Typed Lux virtual shadow resource types remain
        // in the typed Lux set (sanity check that the
        // typed split keeps both sides intact).
        assert!(FrameGraphResourceType::LuxVirtualShadowPages.is_lux());
        assert!(FrameGraphResourceType::LuxShadowAtlas.is_lux());
    }

    /// Pass C7.3 acceptance — typed
    /// `CloudShadowStorageFormat` taxonomy walks the
    /// user-spec formats.
    #[test]
    fn cloud_shadow_storage_format_taxonomy_walks_user_spec() {
        assert_eq!(CloudShadowStorageFormat::ALL.len(), 3);
        // Typed bytes per pixel:
        //   R8Unorm           = 1
        //   R16Float          = 2
        //   Rgba16FloatPacked = 8
        assert_eq!(CloudShadowStorageFormat::R8Unorm.bytes_per_pixel(), 1);
        assert_eq!(CloudShadowStorageFormat::R16Float.bytes_per_pixel(), 2);
        assert_eq!(
            CloudShadowStorageFormat::Rgba16FloatPacked.bytes_per_pixel(),
            8,
        );
        // Only the typed packed format carries the typed
        // full 4-channel sample.
        assert!(!CloudShadowStorageFormat::R8Unorm.carries_full_sample());
        assert!(!CloudShadowStorageFormat::R16Float.carries_full_sample());
        assert!(CloudShadowStorageFormat::Rgba16FloatPacked.carries_full_sample());
        // Only the typed R8Unorm format is typed cheap.
        assert!(CloudShadowStorageFormat::R8Unorm.is_cheap());
        assert!(!CloudShadowStorageFormat::R16Float.is_cheap());
        assert!(!CloudShadowStorageFormat::Rgba16FloatPacked.is_cheap());
        // Typed default is R16Float — typed balanced /
        // cinematic target.
        assert_eq!(
            CloudShadowStorageFormat::default(),
            CloudShadowStorageFormat::R16Float,
        );
    }

    /// Pass C7.2 acceptance — one-frame-delayed mode is
    /// explicit, not accidental.  The typed
    /// `CloudShadowFrameDelayMode` enum carries typed
    /// predicates that audit which frame's typed filtered
    /// shadow the typed direct-lighting pass samples.
    #[test]
    fn one_frame_delayed_mode_is_explicit() {
        // Typed `OneFrameDelayed` samples the typed
        // PREVIOUS frame.
        assert!(
            CloudShadowFrameDelayMode::OneFrameDelayed
                .samples_previous_frame_filtered_shadow(),
        );
        assert!(
            !CloudShadowFrameDelayMode::OneFrameDelayed
                .samples_current_frame_filtered_shadow(),
        );
        // Typed `SameFrame` samples the typed CURRENT
        // frame.
        assert!(
            CloudShadowFrameDelayMode::SameFrame
                .samples_current_frame_filtered_shadow(),
        );
        assert!(
            !CloudShadowFrameDelayMode::SameFrame
                .samples_previous_frame_filtered_shadow(),
        );
        // Typed default is the typed `OneFrameDelayed`
        // mode (easier to schedule).
        let default_mode = CloudShadowFrameDelayMode::default();
        assert_eq!(default_mode, CloudShadowFrameDelayMode::OneFrameDelayed);
        // Typed taxonomy has exactly 2 typed variants.
        assert_eq!(CloudShadowFrameDelayMode::ALL.len(), 2);
        // Typed predicates are mutually exclusive — exactly
        // one of `samples_current` / `samples_previous`
        // returns `true` per mode.
        for mode in CloudShadowFrameDelayMode::ALL {
            let current = mode.samples_current_frame_filtered_shadow();
            let previous = mode.samples_previous_frame_filtered_shadow();
            assert!(current ^ previous, "{:?} ambiguous", mode);
        }
    }

    // ========================================================
    // Pass C7.4 acceptance — typed projection model + typed
    // resource diagnostics.
    // ========================================================

    /// Pass C7.4 acceptance — typed `CloudShadowProjectionMode`
    /// taxonomy is dense; the typed
    /// `CameraCenteredPlane` variant is the typed initial
    /// production default.
    #[test]
    fn cloud_shadow_projection_mode_taxonomy_walks_user_spec() {
        assert_eq!(CloudShadowProjectionMode::ALL.len(), 3);
        let mut seen = std::collections::HashSet::new();
        for mode in CloudShadowProjectionMode::ALL {
            assert!(seen.insert(mode.as_str()), "duplicate: {}", mode.as_str());
        }
        // Typed initial production default — only the typed
        // `CameraCenteredPlane` returns `true`.
        for mode in CloudShadowProjectionMode::ALL {
            let is_default = mode.is_initial_production_default();
            assert_eq!(
                is_default,
                matches!(mode, CloudShadowProjectionMode::CameraCenteredPlane),
                "{:?} unexpected default flag",
                mode,
            );
        }
        // Typed `Default` instance is the typed initial
        // production default.
        assert_eq!(
            CloudShadowProjectionMode::default(),
            CloudShadowProjectionMode::CameraCenteredPlane,
        );
    }

    /// Pass C7.4 acceptance — projection constants can be
    /// built from `CloudRenderSettings`, weather profile,
    /// and primary Lux directional light.  Projection
    /// constants name the Lux light ID that owns the
    /// shadow layer.
    #[test]
    fn cloud_shadow_projection_constants_built_from_inputs() {
        let settings = CloudRenderSettings::PRODUCT_DEFAULT;
        let profile = CloudWeatherProfileId::Scattered;
        let light_id = LuxLightId::new(42);
        let sun = [0.3, 0.8, 0.5];
        let frame = 17;

        let cs = CloudShadowProjectionConstants::from_inputs(
            &settings, profile, light_id, sun, frame,
        );
        // Typed constants name the Lux light id.
        assert_eq!(cs.light_id, light_id);
        // Typed projection mode defaults to the typed
        // initial production default.
        assert_eq!(cs.mode, CloudShadowProjectionMode::CameraCenteredPlane);
        // Typed cloud slab is the typed profile's slab.
        let (base, top) = profile.cloud_slab_meters();
        assert_eq!(cs.cloud_base_meters, base as f32);
        assert_eq!(cs.cloud_top_meters, top as f32);
        assert_eq!(cs.frame_index, frame);
        // Typed sun direction carries through.
        assert_eq!(cs.sun_direction_ws, sun);
        // Typed max-distance + opacity-scale + softness
        // derived from settings.
        assert_eq!(
            cs.max_distance_meters,
            settings.world_shadows.max_distance_meters as f32,
        );
        let expected_opacity = (settings.world_shadows.opacity_scale_q16 as f32) / 65_535.0;
        assert!((cs.opacity_scale - expected_opacity).abs() < 1e-6);
        let expected_softness = (settings.world_shadows.softness_q16 as f32) / 65_535.0;
        assert!((cs.softness - expected_softness).abs() < 1e-6);
        // Typed predicates report a typed fully-live
        // projection.
        assert!(cs.projects_directional_lux_light());
        assert!(cs.has_valid_cloud_slab());
        assert!(cs.has_nonzero_shadow_region());
        assert!(cs.projects_world_shadow());
        // Typed transform matrices round-trip the typed
        // shadow-UV origin (0.5, 0.5) back to the typed
        // world origin (within scaffold precision).
        let d = cs.max_distance_meters;
        // shadow_uv_from_world applied to world (0,_,0) =
        // (0.5, _, 0.5, 1) — typed UV center.
        let u =
            cs.shadow_uv_from_world[0][0] * 0.0 + cs.shadow_uv_from_world[0][3];
        let v =
            cs.shadow_uv_from_world[2][2] * 0.0 + cs.shadow_uv_from_world[2][3];
        assert!((u - 0.5).abs() < 1e-6);
        assert!((v - 0.5).abs() < 1e-6);
        // world_from_shadow_uv applied to UV (0,_,0,1) =
        // (-D, _, -D, 1) — typed UV (0,0) maps to the
        // typed `(-max_distance, _, -max_distance)`
        // corner.
        let wx =
            cs.world_from_shadow_uv[0][0] * 0.0 + cs.world_from_shadow_uv[0][3];
        let wz =
            cs.world_from_shadow_uv[2][2] * 0.0 + cs.world_from_shadow_uv[2][3];
        assert!((wx - -d).abs() < 1e-3);
        assert!((wz - -d).abs() < 1e-3);
    }

    /// Pass C7.4 acceptance — invalid cloud slab disables
    /// projection cleanly.  Tested via the typed `DISABLED`
    /// constant + via a hand-constructed invalid-slab
    /// instance.
    #[test]
    fn cloud_shadow_projection_invalid_slab_disables_cleanly() {
        // Typed `DISABLED` constant — every typed predicate
        // returns `false`.
        let d = CloudShadowProjectionConstants::DISABLED;
        assert!(!d.projects_directional_lux_light());
        assert!(!d.has_valid_cloud_slab());
        assert!(!d.has_nonzero_shadow_region());
        assert!(!d.projects_world_shadow());
        assert_eq!(d.light_id, LuxLightId::INVALID);
        // Typed `Default` is the typed `DISABLED` constant.
        assert_eq!(CloudShadowProjectionConstants::default(), d);

        // Typed hand-constructed invalid-slab instance —
        // base >= top → `has_valid_cloud_slab` is false,
        // but the typed light id + opacity scale + max
        // distance can still report `true`.  This is the
        // typed audit precedent for the typed downstream
        // gate.
        let mut invalid = CloudShadowProjectionConstants::from_inputs(
            &CloudRenderSettings::PRODUCT_DEFAULT,
            CloudWeatherProfileId::Scattered,
            LuxLightId::new(7),
            [0.0, 1.0, 0.0],
            0,
        );
        invalid.cloud_base_meters = 5_000.0;
        invalid.cloud_top_meters = 5_000.0;
        assert!(!invalid.has_valid_cloud_slab());
        assert!(!invalid.projects_world_shadow());

        // Typed settings that gate the pass off → typed
        // `DISABLED` constants.
        let off = CloudShadowProjectionConstants::from_inputs(
            &CloudRenderSettings::DISABLED,
            CloudWeatherProfileId::Scattered,
            LuxLightId::new(7),
            [0.0, 1.0, 0.0],
            0,
        );
        assert_eq!(off, CloudShadowProjectionConstants::DISABLED);

        // Typed invalid light id → typed `DISABLED`.
        let no_light = CloudShadowProjectionConstants::from_inputs(
            &CloudRenderSettings::PRODUCT_DEFAULT,
            CloudWeatherProfileId::Scattered,
            LuxLightId::INVALID,
            [0.0, 1.0, 0.0],
            0,
        );
        assert_eq!(no_light, CloudShadowProjectionConstants::DISABLED);

        // Typed zero sun direction → typed `DISABLED`.
        let no_sun = CloudShadowProjectionConstants::from_inputs(
            &CloudRenderSettings::PRODUCT_DEFAULT,
            CloudWeatherProfileId::Scattered,
            LuxLightId::new(7),
            [0.0, 0.0, 0.0],
            0,
        );
        assert_eq!(no_sun, CloudShadowProjectionConstants::DISABLED);
    }

    /// Pass C7.4 acceptance — every typed weather profile
    /// produces a typed valid cloud slab when fed to
    /// `from_inputs`.
    #[test]
    fn cloud_shadow_projection_every_profile_builds_valid_slab() {
        let settings = CloudRenderSettings::PRODUCT_DEFAULT;
        let light_id = LuxLightId::new(1);
        let sun = [0.0, 1.0, 0.0];
        for profile in CloudWeatherProfileId::ALL {
            let cs = CloudShadowProjectionConstants::from_inputs(
                &settings, profile, light_id, sun, 0,
            );
            assert!(
                cs.has_valid_cloud_slab(),
                "{:?} produced an invalid slab",
                profile,
            );
            assert!(cs.projects_world_shadow(), "{:?} did not project", profile);
        }
    }

    /// Pass C7.4 acceptance — typed
    /// `CloudShadowStorageFormat::for_quality` walks the
    /// user-spec table:
    ///   Cheap     -> R8Unorm
    ///   Balanced  -> R16Float
    ///   Cinematic -> R16Float | Rgba16FloatPacked (debug)
    #[test]
    fn cloud_shadow_storage_format_for_quality_walks_user_spec() {
        assert_eq!(
            CloudShadowStorageFormat::for_quality(CloudQuality::Cheap, false),
            CloudShadowStorageFormat::R8Unorm,
        );
        assert_eq!(
            CloudShadowStorageFormat::for_quality(CloudQuality::Cheap, true),
            CloudShadowStorageFormat::R8Unorm,
        );
        assert_eq!(
            CloudShadowStorageFormat::for_quality(CloudQuality::Balanced, false),
            CloudShadowStorageFormat::R16Float,
        );
        assert_eq!(
            CloudShadowStorageFormat::for_quality(CloudQuality::Balanced, true),
            CloudShadowStorageFormat::R16Float,
        );
        assert_eq!(
            CloudShadowStorageFormat::for_quality(CloudQuality::Cinematic, false),
            CloudShadowStorageFormat::R16Float,
        );
        assert_eq!(
            CloudShadowStorageFormat::for_quality(CloudQuality::Cinematic, true),
            CloudShadowStorageFormat::Rgba16FloatPacked,
        );
        // Typed `Off` tier returns the typed cheap
        // fallback; the typed gate happens at
        // `registers_world_shadow_pass`.
        assert_eq!(
            CloudShadowStorageFormat::for_quality(CloudQuality::Off, false),
            CloudShadowStorageFormat::R8Unorm,
        );
    }

    /// Pass C7.4 acceptance — typed cloud shadow resources
    /// are allocated only when
    /// `CloudRenderSettings::registers_world_shadow_pass()`
    /// is true.  Resource allocation respects quality tier.
    #[test]
    fn cloud_shadow_resource_diagnostics_gate_on_registers_pass() {
        // Typed product default → typed allocation runs.
        let settings = CloudRenderSettings::PRODUCT_DEFAULT;
        let d = CloudShadowResourceDiagnostics::from_settings(&settings, false, true);
        assert!(d.enabled);
        assert_eq!(d.extent, [2048, 2048]);
        assert_eq!(d.transmittance_format, CloudShadowStorageFormat::R16Float);
        assert_eq!(d.filtered_format, CloudShadowStorageFormat::R16Float);
        // 2048 * 2048 * 2 bytes/pixel = 8,388,608 bytes per
        // typed R16Float target.
        assert_eq!(d.transmittance_bytes, 2048 * 2048 * 2);
        assert_eq!(d.filtered_bytes, 2048 * 2048 * 2);
        assert!(d.constants_bytes > 0);
        assert!(d.reallocated_this_frame);
        assert!(d.allocated_any_gpu_bytes());

        // Typed disabled settings → no typed allocation.
        let off = CloudShadowResourceDiagnostics::from_settings(
            &CloudRenderSettings::DISABLED,
            false,
            true,
        );
        assert!(!off.enabled);
        assert_eq!(off.extent, [0, 0]);
        assert_eq!(off.transmittance_bytes, 0);
        assert_eq!(off.filtered_bytes, 0);
        assert_eq!(off.constants_bytes, 0);
        assert!(!off.allocated_any_gpu_bytes());
        // Typed `reallocated_this_frame` preserved through
        // the typed gate (the typed renderer still records
        // whether it ran an allocation cycle).
        assert!(off.reallocated_this_frame);

        // Typed Off quality cascade-disables the pass.
        let mut off_quality = CloudRenderSettings::PRODUCT_DEFAULT;
        off_quality.quality = CloudQuality::Off;
        let d_off = CloudShadowResourceDiagnostics::from_settings(
            &off_quality, false, false,
        );
        assert!(!d_off.enabled);
        assert!(!d_off.allocated_any_gpu_bytes());

        // Typed Cheap quality → typed R8Unorm + typed
        // 1024x1024.
        let mut cheap = CloudRenderSettings::PRODUCT_DEFAULT;
        cheap.quality = CloudQuality::Cheap;
        cheap.world_shadows.resolution = CloudShadowResolution::Cheap1024;
        let d_cheap = CloudShadowResourceDiagnostics::from_settings(&cheap, false, false);
        assert!(d_cheap.enabled);
        assert_eq!(d_cheap.extent, [1024, 1024]);
        assert_eq!(d_cheap.transmittance_format, CloudShadowStorageFormat::R8Unorm);
        // 1024 * 1024 * 1 byte/pixel = 1,048,576 bytes.
        assert_eq!(d_cheap.transmittance_bytes, 1024 * 1024);

        // Typed Cinematic + typed debug readback → typed
        // `Rgba16FloatPacked` + typed 4096x4096.
        let mut cine = CloudRenderSettings::PRODUCT_DEFAULT;
        cine.quality = CloudQuality::Cinematic;
        cine.world_shadows.resolution = CloudShadowResolution::Cinematic4096;
        let d_cine = CloudShadowResourceDiagnostics::from_settings(&cine, true, false);
        assert!(d_cine.enabled);
        assert_eq!(d_cine.extent, [4096, 4096]);
        assert_eq!(
            d_cine.transmittance_format,
            CloudShadowStorageFormat::Rgba16FloatPacked,
        );
        // 4096 * 4096 * 8 bytes/pixel = 134,217,728 bytes
        // per target.
        assert_eq!(d_cine.transmittance_bytes, 4096u64 * 4096 * 8);
        assert_eq!(d_cine.filtered_bytes, 4096u64 * 4096 * 8);

        // Typed total GPU bytes sums the typed three
        // resource byte counts.
        assert_eq!(
            d_cine.total_gpu_bytes(),
            d_cine.transmittance_bytes
                + d_cine.filtered_bytes
                + d_cine.constants_bytes,
        );
    }

    /// Pass C7.4 acceptance — typed cold default reports
    /// zero allocation; typed
    /// `CLOUD_SHADOW_PROJECTION_CONSTANTS_CPU_BYTES` is
    /// non-zero (the typed struct has a non-empty layout).
    #[test]
    fn cloud_shadow_resource_diagnostics_cold_default_reports_zero() {
        let cold = CloudShadowResourceDiagnostics::COLD_DEFAULT;
        assert!(!cold.enabled);
        assert_eq!(cold.extent, [0, 0]);
        assert_eq!(cold.transmittance_bytes, 0);
        assert_eq!(cold.filtered_bytes, 0);
        assert_eq!(cold.constants_bytes, 0);
        assert!(!cold.reallocated_this_frame);
        assert!(!cold.allocated_any_gpu_bytes());
        assert_eq!(cold.total_gpu_bytes(), 0);

        // Typed CPU footprint is non-zero (sanity check
        // the typed `size_of` is wired).
        assert!(CLOUD_SHADOW_PROJECTION_CONSTANTS_CPU_BYTES > 0);
    }
}
