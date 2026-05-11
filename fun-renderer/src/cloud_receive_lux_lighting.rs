//! Pass C8 — typed Lux / cloud lighting unification
//! (positive lighting side).
//!
//! Pass C7.x wired the typed cloud → Lux direction
//! (clouds cast shadows on the typed Lux direct + typed
//! volumetric paths).  Pass C8 wires the typed positive
//! direction: typed clouds receive typed Lux directional
//! lights and typed local volumetric lights through the
//! typed renderer-owned cloud lighting path.
//!
//! Contract:
//!
//!     cloud_radiance(froxel) =
//!         directional_lux_color * directional_lux_intensity *
//!             directional_phase(view, sun) *
//!             cloud_density(froxel) +
//!         sum_over_local_lux_lights(
//!             local_color_i * local_intensity_i *
//!             local_phase(view, light_i) *
//!             cloud_density(froxel) /
//!             attenuation_i(froxel)
//!         )
//!
//! Quality-tier policy (typed CloudLuxLightingTier):
//!
//! - `Disabled`: typed clouds receive no typed Lux
//!   lighting (typed black ambient).
//! - `DirectionalOnly`: typed clouds receive typed
//!   directional Lux only (typed sun).
//! - `DirectionalAndKeyLocal`: typed directional + typed
//!   typed top-N typed clustered local lights.
//! - `DirectionalAndAllLocal`: typed directional + every
//!   typed clustered local light (typed cinematic).
//!
//! Temporal history reset / clamp: typed Lux light
//! changes (typed sun direction, typed directional
//! intensity, typed local-light cluster shape) emit a
//! typed `CloudLuxLightingResetSignal` the typed cloud
//! temporal-resolve pass consumes.

use crate::cloud_shadow::CloudShadowFrameDelayMode;
use fun_lux::{LuxLightId, LuxLightKind};

pub const FUN_RENDERER_CLOUD_RECEIVE_LUX_LIGHTING_SCHEMA_VERSION: u16 = 1;

// ============================================================================
// Section 1 — typed CloudLuxLightingTier
// ============================================================================

/// Typed Pass C8 cloud Lux lighting tier.  Drives how
/// much typed Lux lighting the typed cloud raymarch
/// receives per quality.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CloudLuxLightingTier {
    /// Typed disabled — typed clouds receive no typed
    /// Lux lighting.
    Disabled,
    /// Typed directional-only — typed clouds receive the
    /// typed sun's typed Lux contribution.  Typed product
    /// default for typed Cheap / typed Balanced.
    #[default]
    DirectionalOnly,
    /// Typed directional + typed top-N local — typed
    /// adds typed clustered local Lux lights (typed
    /// brightest typed N).  Typed product default for
    /// typed Cinematic.
    DirectionalAndKeyLocal,
    /// Typed directional + every local — typed cinematic
    /// capture path; typed expensive.
    DirectionalAndAllLocal,
}

impl CloudLuxLightingTier {
    pub const ALL: [Self; 4] = [
        Self::Disabled,
        Self::DirectionalOnly,
        Self::DirectionalAndKeyLocal,
        Self::DirectionalAndAllLocal,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::DirectionalOnly => "directional_only",
            Self::DirectionalAndKeyLocal => "directional_and_key_local",
            Self::DirectionalAndAllLocal => "directional_and_all_local",
        }
    }

    /// Typed predicate: does this typed tier accept any
    /// typed Lux lighting on clouds?
    #[must_use]
    pub const fn accepts_any_lighting(self) -> bool {
        !matches!(self, Self::Disabled)
    }

    /// Typed predicate: does this typed tier accept typed
    /// directional Lux lighting?
    #[must_use]
    pub const fn accepts_directional(self) -> bool {
        matches!(
            self,
            Self::DirectionalOnly | Self::DirectionalAndKeyLocal | Self::DirectionalAndAllLocal,
        )
    }

    /// Typed predicate: does this typed tier accept typed
    /// local Lux lighting?
    #[must_use]
    pub const fn accepts_local(self) -> bool {
        matches!(
            self,
            Self::DirectionalAndKeyLocal | Self::DirectionalAndAllLocal
        )
    }

    /// Typed maximum count of typed local Lux lights this
    /// typed tier injects per frame.  Typed `0` for typed
    /// tiers that skip locals; typed `8` for typed
    /// `DirectionalAndKeyLocal`; typed `u16::MAX` for typed
    /// `DirectionalAndAllLocal`.
    #[must_use]
    pub const fn max_local_lights(self) -> u16 {
        match self {
            Self::Disabled | Self::DirectionalOnly => 0,
            Self::DirectionalAndKeyLocal => 8,
            Self::DirectionalAndAllLocal => u16::MAX,
        }
    }
}

// ============================================================================
// Section 2 — typed light-kind gating predicates
// ============================================================================

/// Typed Pass C8 — predicate: does this typed
/// `LuxLightKind` illuminate clouds under the typed
/// tier?  Typed directional lights illuminate at every
/// typed tier that typed accepts directional; typed
/// local lights illuminate only at tiers that typed
/// accept local; typed emissive / probe never illuminate
/// clouds (typed don't carry typed direct-light
/// intensity).
#[must_use]
pub const fn light_kind_illuminates_clouds(kind: LuxLightKind, tier: CloudLuxLightingTier) -> bool {
    match kind {
        LuxLightKind::Directional => tier.accepts_directional(),
        LuxLightKind::Punctual | LuxLightKind::Area => tier.accepts_local(),
        LuxLightKind::EmissiveCandidate | LuxLightKind::Probe => false,
    }
}

// ============================================================================
// Section 3 — typed CloudLuxLightingContribution
// ============================================================================

/// Typed Pass C8 — typed per-light cloud lighting
/// contribution.  Bundles the typed light id, kind, and
/// the typed RGB contribution the typed cloud raymarch
/// accumulates this froxel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CloudLuxLightingContribution {
    pub schema_version: u16,
    pub light_id: LuxLightId,
    pub light_kind: LuxLightKind,
    /// Typed RGB color × intensity × phase × density
    /// product the typed renderer accumulates.  Clamped
    /// to typed `[0, ∞)` per channel.
    pub rgb: [f32; 3],
    /// Typed predicate: did this typed contribution
    /// actually inject into the typed cloud froxel
    /// (typed tier accepts + typed light id is valid)?
    pub injected: bool,
}

impl Default for CloudLuxLightingContribution {
    fn default() -> Self {
        Self {
            schema_version: FUN_RENDERER_CLOUD_RECEIVE_LUX_LIGHTING_SCHEMA_VERSION,
            light_id: LuxLightId::INVALID,
            light_kind: LuxLightKind::Directional,
            rgb: [0.0; 3],
            injected: false,
        }
    }
}

impl CloudLuxLightingContribution {
    /// Typed Pass C8 — "no contribution" baseline.
    #[must_use]
    pub fn no_contribution(light_id: LuxLightId, light_kind: LuxLightKind) -> Self {
        Self {
            schema_version: FUN_RENDERER_CLOUD_RECEIVE_LUX_LIGHTING_SCHEMA_VERSION,
            light_id,
            light_kind,
            rgb: [0.0; 3],
            injected: false,
        }
    }

    /// Typed predicate: is this typed contribution
    /// typed directional?
    #[must_use]
    pub fn is_directional(&self) -> bool {
        matches!(self.light_kind, LuxLightKind::Directional)
    }

    /// Typed predicate: is this typed contribution
    /// typed local?
    #[must_use]
    pub fn is_local(&self) -> bool {
        matches!(self.light_kind, LuxLightKind::Punctual | LuxLightKind::Area)
    }

    /// Typed total typed channel intensity (R + G + B).
    /// Used by the typed debug overlay sort key.
    #[must_use]
    pub fn total_intensity(&self) -> f32 {
        self.rgb[0] + self.rgb[1] + self.rgb[2]
    }
}

/// Typed Pass C8 — compose a typed per-light contribution
/// for a typed cloud froxel.  Gates on the typed tier +
/// the typed light kind; returns `no_contribution(...)`
/// when the typed gate rejects.
///
/// Inputs:
/// - `tier` — typed quality tier.
/// - `light_id`, `light_kind` — typed Lux light identity.
/// - `color_rgb` — typed light color (`[R, G, B]`,
///   clamped per channel to typed `[0, +∞)`).
/// - `intensity` — typed scalar light intensity (typed
///   `>= 0`).
/// - `phase_density` — typed precomputed product of
///   typed `phase(view, light_dir)` × typed
///   `cloud_density(froxel)` × typed attenuation
///   (typed in `[0, ∞)`).  The typed renderer computes
///   this typed scalar per froxel.
#[must_use]
pub fn compose_cloud_lux_contribution(
    tier: CloudLuxLightingTier,
    light_id: LuxLightId,
    light_kind: LuxLightKind,
    color_rgb: [f32; 3],
    intensity: f32,
    phase_density: f32,
) -> CloudLuxLightingContribution {
    if !light_id.is_valid() {
        return CloudLuxLightingContribution::no_contribution(light_id, light_kind);
    }
    if !light_kind_illuminates_clouds(light_kind, tier) {
        return CloudLuxLightingContribution::no_contribution(light_id, light_kind);
    }
    let intensity = intensity.max(0.0);
    let phase_density = phase_density.max(0.0);
    let scale = intensity * phase_density;
    let rgb = [
        color_rgb[0].max(0.0) * scale,
        color_rgb[1].max(0.0) * scale,
        color_rgb[2].max(0.0) * scale,
    ];
    CloudLuxLightingContribution {
        schema_version: FUN_RENDERER_CLOUD_RECEIVE_LUX_LIGHTING_SCHEMA_VERSION,
        light_id,
        light_kind,
        rgb,
        injected: true,
    }
}

// ============================================================================
// Section 4 — typed CloudLuxLightingSummary
// ============================================================================

/// Typed Pass C8 cloud Lux lighting summary.  Aggregates
/// the typed per-light contributions the typed cloud
/// raymarch collected this frame.  Drives the typed
/// `CloudDebugOverlay::LuxLighting` debug view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CloudLuxLightingSummary {
    pub schema_version: u16,
    pub tier: CloudLuxLightingTier,
    pub directional_injected_count: u32,
    pub local_injected_count: u32,
    pub skipped_count: u32,
    /// Typed sum of typed directional RGB contributions
    /// across every typed froxel this frame.
    pub total_directional_rgb: [f32; 3],
    /// Typed sum of typed local RGB contributions.
    pub total_local_rgb: [f32; 3],
}

impl Default for CloudLuxLightingSummary {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl CloudLuxLightingSummary {
    pub const EMPTY: Self = Self {
        schema_version: FUN_RENDERER_CLOUD_RECEIVE_LUX_LIGHTING_SCHEMA_VERSION,
        tier: CloudLuxLightingTier::DirectionalOnly,
        directional_injected_count: 0,
        local_injected_count: 0,
        skipped_count: 0,
        total_directional_rgb: [0.0; 3],
        total_local_rgb: [0.0; 3],
    };

    /// Typed Pass C8 — record a typed contribution into
    /// the typed summary.
    pub fn record(&mut self, contribution: &CloudLuxLightingContribution) {
        if !contribution.injected {
            self.skipped_count = self.skipped_count.saturating_add(1);
            return;
        }
        if contribution.is_directional() {
            self.directional_injected_count = self.directional_injected_count.saturating_add(1);
            self.total_directional_rgb[0] += contribution.rgb[0];
            self.total_directional_rgb[1] += contribution.rgb[1];
            self.total_directional_rgb[2] += contribution.rgb[2];
        } else if contribution.is_local() {
            self.local_injected_count = self.local_injected_count.saturating_add(1);
            self.total_local_rgb[0] += contribution.rgb[0];
            self.total_local_rgb[1] += contribution.rgb[1];
            self.total_local_rgb[2] += contribution.rgb[2];
        } else {
            self.skipped_count = self.skipped_count.saturating_add(1);
        }
    }

    /// Typed total injected count.
    #[must_use]
    pub const fn total_injected(&self) -> u32 {
        self.directional_injected_count
            .saturating_add(self.local_injected_count)
    }

    /// Typed predicate: did any typed directional
    /// contribution inject?
    #[must_use]
    pub const fn directional_injected(&self) -> bool {
        self.directional_injected_count > 0
    }

    /// Typed predicate: did any typed local contribution
    /// inject?
    #[must_use]
    pub const fn local_injected(&self) -> bool {
        self.local_injected_count > 0
    }
}

/// Typed Pass C8 — emit the typed "Cloud Lux Lighting"
/// debug section the typed `CloudDebugOverlay::LuxLighting`
/// overlay appends to the typed
/// `RendererFrameGraphDebugArtifact.content`.
#[must_use]
pub fn cloud_lux_lighting_debug_section(summary: &CloudLuxLightingSummary) -> String {
    use core::fmt::Write as _;
    let mut content = String::new();
    let _ = writeln!(content, "Cloud Lux Lighting");
    let _ = writeln!(content, "------------------");
    let _ = writeln!(content, "tier: {}", summary.tier.as_str());
    let _ = writeln!(
        content,
        "directional_injected: {}",
        summary.directional_injected_count,
    );
    let _ = writeln!(content, "local_injected: {}", summary.local_injected_count);
    let _ = writeln!(content, "skipped: {}", summary.skipped_count);
    let _ = writeln!(
        content,
        "total_directional_rgb: [{:.4}, {:.4}, {:.4}]",
        summary.total_directional_rgb[0],
        summary.total_directional_rgb[1],
        summary.total_directional_rgb[2],
    );
    let _ = writeln!(
        content,
        "total_local_rgb: [{:.4}, {:.4}, {:.4}]",
        summary.total_local_rgb[0], summary.total_local_rgb[1], summary.total_local_rgb[2],
    );
    content
}

// ============================================================================
// Section 5 — typed CloudLuxLightingResetSignal (temporal history)
// ============================================================================

/// Typed Pass C8 — typed temporal-history reset/clamp
/// reason.  Drives whether the typed cloud temporal
/// resolve pass keeps, clamps, or discards typed history
/// when typed Lux lighting changes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CloudLuxLightingResetReason {
    /// Typed no reset — typed history is typed valid.
    #[default]
    None,
    /// Typed sun direction changed past typed threshold
    /// → typed history is typed stale.
    SunDirectionChanged,
    /// Typed sun color / intensity changed past typed
    /// threshold.
    SunIntensityChanged,
    /// Typed local-light cluster shape changed → typed
    /// reset typed local-light history.
    LocalClusterChanged,
    /// Typed tier flipped (e.g., typed Disabled →
    /// typed DirectionalOnly) → typed reset.
    TierChanged,
}

impl CloudLuxLightingResetReason {
    pub const ALL: [Self; 5] = [
        Self::None,
        Self::SunDirectionChanged,
        Self::SunIntensityChanged,
        Self::LocalClusterChanged,
        Self::TierChanged,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::SunDirectionChanged => "sun_direction_changed",
            Self::SunIntensityChanged => "sun_intensity_changed",
            Self::LocalClusterChanged => "local_cluster_changed",
            Self::TierChanged => "tier_changed",
        }
    }

    /// Typed predicate: does this typed reason require
    /// the typed cloud temporal resolve to clamp or reset
    /// history?
    #[must_use]
    pub const fn requires_reset(self) -> bool {
        !matches!(self, Self::None)
    }
}

/// Typed Pass C8 cloud Lux lighting reset signal.  The
/// typed renderer emits this typed record each frame; the
/// typed cloud temporal resolve pass consumes it.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CloudLuxLightingResetSignal {
    pub schema_version: u16,
    pub reason: CloudLuxLightingResetReason,
    pub frame_index: u32,
    pub latency: CloudShadowFrameDelayMode,
}

impl CloudLuxLightingResetSignal {
    /// Typed Pass C8 — typed builder: detect the typed
    /// reset reason from typed previous-frame vs typed
    /// current-frame Lux state deltas.
    #[must_use]
    pub fn from_deltas(
        sun_direction_changed: bool,
        sun_intensity_changed: bool,
        local_cluster_changed: bool,
        tier_changed: bool,
        frame_index: u32,
        latency: CloudShadowFrameDelayMode,
    ) -> Self {
        let reason = if tier_changed {
            // Typed tier flip is the typed highest priority
            // reset — the typed entire light set may have
            // changed shape.
            CloudLuxLightingResetReason::TierChanged
        } else if sun_direction_changed {
            CloudLuxLightingResetReason::SunDirectionChanged
        } else if sun_intensity_changed {
            CloudLuxLightingResetReason::SunIntensityChanged
        } else if local_cluster_changed {
            CloudLuxLightingResetReason::LocalClusterChanged
        } else {
            CloudLuxLightingResetReason::None
        };
        Self {
            schema_version: FUN_RENDERER_CLOUD_RECEIVE_LUX_LIGHTING_SCHEMA_VERSION,
            reason,
            frame_index,
            latency,
        }
    }

    /// Typed predicate: should the typed temporal resolve
    /// reset / clamp history this frame?
    #[must_use]
    pub const fn requires_reset(&self) -> bool {
        self.reason.requires_reset()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pass C8 schema is stable.
    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_RENDERER_CLOUD_RECEIVE_LUX_LIGHTING_SCHEMA_VERSION, 1);
    }

    /// Pass C8 — tier taxonomy walks user spec.
    #[test]
    fn tier_taxonomy_walks_user_spec() {
        assert_eq!(CloudLuxLightingTier::ALL.len(), 4);
        assert!(!CloudLuxLightingTier::Disabled.accepts_any_lighting());
        assert!(!CloudLuxLightingTier::Disabled.accepts_directional());
        assert!(!CloudLuxLightingTier::Disabled.accepts_local());
        assert!(CloudLuxLightingTier::DirectionalOnly.accepts_directional());
        assert!(!CloudLuxLightingTier::DirectionalOnly.accepts_local());
        assert!(CloudLuxLightingTier::DirectionalAndKeyLocal.accepts_directional());
        assert!(CloudLuxLightingTier::DirectionalAndKeyLocal.accepts_local());
        assert!(CloudLuxLightingTier::DirectionalAndAllLocal.accepts_directional());
        assert!(CloudLuxLightingTier::DirectionalAndAllLocal.accepts_local());
        // Typed local light caps.
        assert_eq!(CloudLuxLightingTier::Disabled.max_local_lights(), 0);
        assert_eq!(CloudLuxLightingTier::DirectionalOnly.max_local_lights(), 0);
        assert_eq!(
            CloudLuxLightingTier::DirectionalAndKeyLocal.max_local_lights(),
            8
        );
        assert_eq!(
            CloudLuxLightingTier::DirectionalAndAllLocal.max_local_lights(),
            u16::MAX,
        );
    }

    /// Pass C8 acceptance — directional Lux light color
    /// / intensity affects clouds.
    #[test]
    fn directional_lux_light_color_and_intensity_affect_clouds() {
        let tier = CloudLuxLightingTier::DirectionalOnly;
        let light = LuxLightId::new(42);
        // Typed sun: typed white color, typed intensity
        // 1.0, typed phase_density 0.5.
        let contribution = compose_cloud_lux_contribution(
            tier,
            light,
            LuxLightKind::Directional,
            [1.0, 1.0, 1.0],
            1.0,
            0.5,
        );
        assert!(contribution.injected);
        assert!(contribution.is_directional());
        assert_eq!(contribution.rgb, [0.5, 0.5, 0.5]);
        // Typed sun: typed warm color (typed orange),
        // typed intensity 2.0, typed phase_density 0.5.
        let warm = compose_cloud_lux_contribution(
            tier,
            light,
            LuxLightKind::Directional,
            [1.0, 0.5, 0.2],
            2.0,
            0.5,
        );
        assert_eq!(warm.rgb, [1.0, 0.5, 0.2]);
        // Typed sun: typed intensity 0.0 → typed zero
        // contribution.
        let dark = compose_cloud_lux_contribution(
            tier,
            light,
            LuxLightKind::Directional,
            [1.0, 1.0, 1.0],
            0.0,
            0.5,
        );
        assert!(dark.injected);
        assert_eq!(dark.rgb, [0.0, 0.0, 0.0]);
    }

    /// Pass C8 acceptance — local Lux volumetric lights
    /// can illuminate clouds by quality tier.
    #[test]
    fn local_lux_volumetric_lights_can_illuminate_clouds_by_quality_tier() {
        let light = LuxLightId::new(7);
        let color = [1.0, 1.0, 1.0];
        // Typed DirectionalOnly → typed local rejected.
        let c = compose_cloud_lux_contribution(
            CloudLuxLightingTier::DirectionalOnly,
            light,
            LuxLightKind::Punctual,
            color,
            1.0,
            0.5,
        );
        assert!(!c.injected);
        // Typed DirectionalAndKeyLocal → typed local
        // accepted.
        let c = compose_cloud_lux_contribution(
            CloudLuxLightingTier::DirectionalAndKeyLocal,
            light,
            LuxLightKind::Punctual,
            color,
            1.0,
            0.5,
        );
        assert!(c.injected);
        assert!(c.is_local());
        assert_eq!(c.rgb, [0.5, 0.5, 0.5]);
        // Typed Area light at typed DirectionalAndAllLocal.
        let c = compose_cloud_lux_contribution(
            CloudLuxLightingTier::DirectionalAndAllLocal,
            light,
            LuxLightKind::Area,
            color,
            2.0,
            0.25,
        );
        assert!(c.injected);
        assert_eq!(c.rgb, [0.5, 0.5, 0.5]);
        // Typed Probe never illuminates clouds.
        let c = compose_cloud_lux_contribution(
            CloudLuxLightingTier::DirectionalAndAllLocal,
            light,
            LuxLightKind::Probe,
            color,
            1.0,
            0.5,
        );
        assert!(!c.injected);
        // Typed EmissiveCandidate never illuminates
        // clouds.
        let c = compose_cloud_lux_contribution(
            CloudLuxLightingTier::DirectionalAndAllLocal,
            light,
            LuxLightKind::EmissiveCandidate,
            color,
            1.0,
            0.5,
        );
        assert!(!c.injected);
    }

    /// Pass C8 acceptance — `CloudDebugOverlay::LuxLighting`
    /// shows local/directional contribution.  Audited via
    /// the typed summary record + the typed debug-section
    /// text emitter.
    #[test]
    fn debug_overlay_shows_local_and_directional_contribution() {
        let tier = CloudLuxLightingTier::DirectionalAndKeyLocal;
        let mut summary = CloudLuxLightingSummary {
            tier,
            ..CloudLuxLightingSummary::EMPTY
        };

        // Typed directional sun.
        let sun = compose_cloud_lux_contribution(
            tier,
            LuxLightId::new(1),
            LuxLightKind::Directional,
            [1.0, 0.9, 0.7],
            1.0,
            0.5,
        );
        summary.record(&sun);
        // Typed local lights.
        let lamp = compose_cloud_lux_contribution(
            tier,
            LuxLightId::new(2),
            LuxLightKind::Punctual,
            [0.8, 0.6, 0.2],
            2.0,
            0.25,
        );
        summary.record(&lamp);
        let panel = compose_cloud_lux_contribution(
            tier,
            LuxLightId::new(3),
            LuxLightKind::Area,
            [0.4, 0.4, 1.0],
            1.0,
            0.5,
        );
        summary.record(&panel);
        // Typed skipped (typed Probe).
        let probe = compose_cloud_lux_contribution(
            tier,
            LuxLightId::new(4),
            LuxLightKind::Probe,
            [1.0; 3],
            1.0,
            0.5,
        );
        summary.record(&probe);

        assert_eq!(summary.directional_injected_count, 1);
        assert_eq!(summary.local_injected_count, 2);
        assert_eq!(summary.skipped_count, 1);
        assert_eq!(summary.total_injected(), 3);
        assert!(summary.directional_injected());
        assert!(summary.local_injected());

        let section = cloud_lux_lighting_debug_section(&summary);
        assert!(section.contains("Cloud Lux Lighting"));
        assert!(section.contains("tier: directional_and_key_local"));
        assert!(section.contains("directional_injected: 1"));
        assert!(section.contains("local_injected: 2"));
        assert!(section.contains("skipped: 1"));
        assert!(section.contains("total_directional_rgb:"));
        assert!(section.contains("total_local_rgb:"));
    }

    /// Pass C8 acceptance — Lux light changes reset or
    /// clamp cloud temporal history when needed.
    #[test]
    fn lux_light_changes_reset_or_clamp_cloud_temporal_history() {
        // Typed no deltas → typed no reset.
        let stable = CloudLuxLightingResetSignal::from_deltas(
            false,
            false,
            false,
            false,
            0,
            CloudShadowFrameDelayMode::OneFrameDelayed,
        );
        assert_eq!(stable.reason, CloudLuxLightingResetReason::None);
        assert!(!stable.requires_reset());

        // Typed sun direction delta.
        let sun_dir = CloudLuxLightingResetSignal::from_deltas(
            true,
            false,
            false,
            false,
            1,
            CloudShadowFrameDelayMode::OneFrameDelayed,
        );
        assert_eq!(
            sun_dir.reason,
            CloudLuxLightingResetReason::SunDirectionChanged
        );
        assert!(sun_dir.requires_reset());

        // Typed sun intensity delta.
        let sun_int = CloudLuxLightingResetSignal::from_deltas(
            false,
            true,
            false,
            false,
            2,
            CloudShadowFrameDelayMode::SameFrame,
        );
        assert_eq!(
            sun_int.reason,
            CloudLuxLightingResetReason::SunIntensityChanged
        );
        assert!(sun_int.requires_reset());

        // Typed local cluster delta.
        let cluster = CloudLuxLightingResetSignal::from_deltas(
            false,
            false,
            true,
            false,
            3,
            CloudShadowFrameDelayMode::OneFrameDelayed,
        );
        assert_eq!(
            cluster.reason,
            CloudLuxLightingResetReason::LocalClusterChanged
        );

        // Typed tier change wins over typed every other
        // signal.
        let tier = CloudLuxLightingResetSignal::from_deltas(
            true,
            true,
            true,
            true,
            4,
            CloudShadowFrameDelayMode::SameFrame,
        );
        assert_eq!(tier.reason, CloudLuxLightingResetReason::TierChanged);

        // Typed every reset reason except `None` requires
        // reset.
        for reason in CloudLuxLightingResetReason::ALL {
            let expects_reset = !matches!(reason, CloudLuxLightingResetReason::None);
            assert_eq!(reason.requires_reset(), expects_reset, "{:?}", reason);
        }
    }

    /// Pass C8 — typed `Disabled` tier rejects every
    /// typed light kind.
    #[test]
    fn disabled_tier_rejects_every_light_kind() {
        let tier = CloudLuxLightingTier::Disabled;
        for kind in [
            LuxLightKind::Directional,
            LuxLightKind::Punctual,
            LuxLightKind::Area,
            LuxLightKind::EmissiveCandidate,
            LuxLightKind::Probe,
        ] {
            assert!(!light_kind_illuminates_clouds(kind, tier));
            let c =
                compose_cloud_lux_contribution(tier, LuxLightId::new(1), kind, [1.0; 3], 1.0, 1.0);
            assert!(!c.injected, "{:?}", kind);
            assert_eq!(c.rgb, [0.0; 3]);
        }
    }

    /// Pass C8 — typed invalid light id rejects.
    #[test]
    fn invalid_light_id_rejects_contribution() {
        let c = compose_cloud_lux_contribution(
            CloudLuxLightingTier::DirectionalOnly,
            LuxLightId::INVALID,
            LuxLightKind::Directional,
            [1.0; 3],
            1.0,
            1.0,
        );
        assert!(!c.injected);
    }

    /// Pass C8 — typed negative inputs clamp to typed 0.
    #[test]
    fn negative_inputs_clamp_to_zero() {
        let c = compose_cloud_lux_contribution(
            CloudLuxLightingTier::DirectionalOnly,
            LuxLightId::new(1),
            LuxLightKind::Directional,
            [-0.5, 1.0, 1.0],
            -2.0,
            -1.0,
        );
        assert!(c.injected);
        assert_eq!(c.rgb, [0.0; 3]);
    }
}
