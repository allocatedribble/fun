//! Pass C0 / C1 — typed cloud world-shadow settings.
//!
//! The typed cloud raymarch produces a typed transmittance
//! map (per-pixel cloud opacity toward the sun); Pass C2+
//! will project this transmittance onto the world to drive
//! the typed cloud world-shadow mask that terrain +
//! materials sample.
//!
//! Pass C0 / C1 lands the typed settings + the typed
//! resource intent shape; the actual GPU shadow projection
//! pass lands in a later cloud pass.

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
}
