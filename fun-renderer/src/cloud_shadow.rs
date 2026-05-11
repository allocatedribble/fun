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
