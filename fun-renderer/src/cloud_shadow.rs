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

use crate::frame_graph::{FrameGraphPassRole, FrameGraphResourceType};

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
}
