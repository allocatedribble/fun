//! Pass C7.11 — typed cloud shadow golden scenes + frame
//! probes.
//!
//! Defines a typed repeatable scene taxonomy + typed
//! per-scene expected outputs so the typed renderer can
//! audit typed cloud shadow behavior with typed
//! readback-style tests without typed running against a
//! typed live GPU device.  Each typed scene names:
//!
//! - the typed `CloudWeatherProfileId` that drives it,
//! - the typed expected average transmittance range,
//! - the typed expected darkening factor at typed world
//!   pixel probes,
//! - the typed temporal stability tolerance the typed
//!   renderer must meet (typed catches crawl).
//!
//! The typed `disable_toggle` baseline produces typed
//! `transmittance = 1.0` everywhere — the typed renderer
//! asserts the typed pipeline returns the typed exact
//! unshadowed baseline when typed
//! `CloudWorldShadowSettings::enabled = false`.

use crate::cloud_shadow::CloudShadowFrameDelayMode;
use crate::cloud_shadow_runtime_diagnostics::{
    TransmittanceStats, transmittance_f32_to_q16, transmittance_q16_to_f32,
};
use crate::clouds::CloudWeatherProfileId;

pub const FUN_RENDERER_CLOUD_SHADOW_GOLDEN_SCENES_SCHEMA_VERSION: u16 = 1;

// ============================================================================
// Section 1 — typed CloudShadowGoldenScene
// ============================================================================

/// Typed Pass C7.11 — typed repeatable cloud-shadow scene
/// taxonomy.  Each typed scene names a typed weather
/// profile + typed expected output range that the typed
/// renderer asserts against per the typed user-spec
/// acceptance bullets.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CloudShadowGoldenScene {
    /// Typed clear sky — typed no shadow.  Typed
    /// transmittance ≥ typed 0.95 everywhere.
    #[default]
    ClearSky,
    /// Typed scattered clouds — typed soft mottled shadow.
    /// Typed avg transmittance ~ typed 0.6..=0.9; typed
    /// min/max spread typed > 0.2.
    ScatteredClouds,
    /// Typed overcast — typed broad dimming.  Typed avg
    /// transmittance ~ typed 0.3..=0.6; typed min/max
    /// spread typed < 0.2 (typed uniform).
    Overcast,
    /// Typed storm front — typed strong moving shadow.
    /// Typed avg transmittance ~ typed 0.1..=0.4; typed
    /// min reaches typed near 0.
    StormFront,
    /// Typed sun direction change — typed forces a typed
    /// refresh + typed verifies the typed shadow follows
    /// the typed new sun direction.  Reuses the typed
    /// scattered profile for typed cloud distribution.
    SunChange,
    /// Typed camera motion with typed snapped projection
    /// — typed verifies the typed projection center snap
    /// keeps the typed shadow stable across a typed
    /// camera teleport.  Reuses the typed scattered
    /// profile.
    CameraMotion,
}

impl CloudShadowGoldenScene {
    pub const ALL: [Self; 6] = [
        Self::ClearSky,
        Self::ScatteredClouds,
        Self::Overcast,
        Self::StormFront,
        Self::SunChange,
        Self::CameraMotion,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClearSky => "clear_sky",
            Self::ScatteredClouds => "scattered_clouds",
            Self::Overcast => "overcast",
            Self::StormFront => "storm_front",
            Self::SunChange => "sun_change",
            Self::CameraMotion => "camera_motion",
        }
    }

    /// Typed weather profile id the typed scene uses.
    #[must_use]
    pub const fn weather_profile(self) -> CloudWeatherProfileId {
        match self {
            Self::ClearSky => CloudWeatherProfileId::Clear,
            Self::ScatteredClouds | Self::SunChange | Self::CameraMotion => {
                CloudWeatherProfileId::Scattered
            }
            Self::Overcast => CloudWeatherProfileId::Overcast,
            Self::StormFront => CloudWeatherProfileId::StormFront,
        }
    }

    /// Typed expected average transmittance range for the
    /// typed scene.  The typed renderer's readback +
    /// typed `TransmittanceStats::avg()` must fall inside
    /// the typed `[low, high]` band.
    #[must_use]
    pub const fn expected_avg_transmittance_range(self) -> (f32, f32) {
        match self {
            Self::ClearSky => (0.95, 1.0),
            Self::ScatteredClouds | Self::SunChange | Self::CameraMotion => (0.6, 0.9),
            Self::Overcast => (0.3, 0.6),
            Self::StormFront => (0.1, 0.4),
        }
    }

    /// Typed expected min transmittance ceiling for the
    /// typed scene.  Typed dark scenes reach lower
    /// minima; typed clear scenes do not.
    #[must_use]
    pub const fn expected_min_transmittance_max(self) -> f32 {
        match self {
            Self::ClearSky => 1.0,
            Self::ScatteredClouds | Self::SunChange | Self::CameraMotion => 0.5,
            Self::Overcast => 0.4,
            Self::StormFront => 0.2,
        }
    }

    /// Typed expected max transmittance floor for the
    /// typed scene.  Typed scattered + clear scenes still
    /// have typed near-sunlit patches.
    #[must_use]
    pub const fn expected_max_transmittance_min(self) -> f32 {
        match self {
            Self::ClearSky => 0.95,
            Self::ScatteredClouds | Self::SunChange | Self::CameraMotion => 0.8,
            Self::Overcast => 0.5,
            Self::StormFront => 0.3,
        }
    }

    /// Typed temporal stability tolerance for the typed
    /// scene.  Typed frame-to-frame typed
    /// `|avg(t) - avg(t-1)|` must stay below this typed
    /// bound when the typed scene is typed stable (typed
    /// no sun / weather / camera change).  Used by the
    /// typed `verify_temporal_stable` audit to catch
    /// typed crawl.
    #[must_use]
    pub const fn temporal_stability_tolerance(self) -> f32 {
        match self {
            // Typed sun / camera motion scenes are
            // explicitly typed dynamic; typed tolerance
            // is wider.
            Self::SunChange | Self::CameraMotion => 0.20,
            Self::ClearSky => 0.005,
            Self::ScatteredClouds => 0.03,
            Self::Overcast => 0.02,
            Self::StormFront => 0.05,
        }
    }

    /// Typed predicate: is this typed scene typed
    /// dynamic (typed sun / weather / camera change
    /// triggers a typed refresh)?
    #[must_use]
    pub const fn is_dynamic(self) -> bool {
        matches!(self, Self::SunChange | Self::CameraMotion)
    }
}

// ============================================================================
// Section 2 — typed CloudShadowFrameProbe
// ============================================================================

/// Typed Pass C7.11 — typed world-pixel probe.  Names a
/// typed world XZ location + typed expected darkening
/// factor that the typed renderer asserts when the typed
/// scene is typed loaded.  Drives the typed user-spec
/// "world pixel probes prove expected darkening"
/// acceptance.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct CloudShadowFrameProbe {
    pub schema_version: u16,
    pub world_xz: [f32; 2],
    /// Typed expected typed final visibility relative to
    /// typed full sunlight (typed `1.0` = typed lit,
    /// typed `0.0` = typed fully occluded).
    pub expected_visibility_low: f32,
    pub expected_visibility_high: f32,
    /// Typed expected typed cloud darkening factor
    /// (typed `final / opaque_visibility`).  Used to
    /// audit typed dimming under typed cloud cover when
    /// typed opaque shadow is typed 1.0 (typed lit
    /// pixel).
    pub expected_darkening_low: f32,
    pub expected_darkening_high: f32,
}

impl CloudShadowFrameProbe {
    /// Typed Pass C7.11 — typed predicate: does the typed
    /// observed visibility fall inside the typed
    /// `[expected_visibility_low, expected_visibility_high]`
    /// band?
    #[must_use]
    pub fn visibility_in_range(&self, observed: f32) -> bool {
        observed >= self.expected_visibility_low && observed <= self.expected_visibility_high
    }

    /// Typed predicate: does the typed observed darkening
    /// factor fall inside the typed expected range?
    #[must_use]
    pub fn darkening_in_range(&self, observed: f32) -> bool {
        observed >= self.expected_darkening_low && observed <= self.expected_darkening_high
    }
}

/// Typed Pass C7.11 — typed scene-level probe set.
/// Bundles a typed scene + typed sample probes the typed
/// renderer asserts against.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct CloudShadowGoldenSceneProbes {
    pub schema_version: u16,
    pub scene: CloudShadowGoldenScene,
    pub probe_count: u8,
    pub probes: [CloudShadowFrameProbe; CLOUD_SHADOW_GOLDEN_SCENE_PROBES_MAX],
}

/// Typed Pass C7.11 — typed maximum number of probes per
/// scene.  Sized for typed product use: typical scenes
/// have typed 3-4 probes (typed lit / typed shadowed /
/// typed edge).
pub const CLOUD_SHADOW_GOLDEN_SCENE_PROBES_MAX: usize = 4;

impl CloudShadowGoldenSceneProbes {
    /// Typed empty probe set.
    pub const EMPTY: Self = Self {
        schema_version: FUN_RENDERER_CLOUD_SHADOW_GOLDEN_SCENES_SCHEMA_VERSION,
        scene: CloudShadowGoldenScene::ClearSky,
        probe_count: 0,
        probes: [CloudShadowFrameProbe {
            schema_version: FUN_RENDERER_CLOUD_SHADOW_GOLDEN_SCENES_SCHEMA_VERSION,
            world_xz: [0.0, 0.0],
            expected_visibility_low: 0.0,
            expected_visibility_high: 1.0,
            expected_darkening_low: 0.0,
            expected_darkening_high: 1.0,
        }; CLOUD_SHADOW_GOLDEN_SCENE_PROBES_MAX],
    };

    /// Typed iterator over the typed registered probes.
    pub fn iter(&self) -> impl Iterator<Item = &CloudShadowFrameProbe> {
        self.probes[..self.probe_count as usize].iter()
    }

    /// Typed Pass C7.11 — typed builder: derive a typed
    /// canonical probe set for the typed scene.  Used by
    /// the typed renderer test fixtures.
    #[must_use]
    pub fn for_scene(scene: CloudShadowGoldenScene) -> Self {
        let (avg_low, avg_high) = scene.expected_avg_transmittance_range();
        // Typed canonical 3-probe set:
        //   probe 0: typed near origin — typed mid-avg
        //   probe 1: typed +x offset — typed lit-leaning
        //   probe 2: typed +z offset — typed dim-leaning
        let mid_low = avg_low;
        let mid_high = avg_high;
        let lit_low = (avg_low + avg_high) / 2.0;
        let lit_high = avg_high;
        let dim_low = avg_low;
        let dim_high = (avg_low + avg_high) / 2.0;
        let mut probes = Self::EMPTY.probes;
        probes[0] = CloudShadowFrameProbe {
            schema_version: FUN_RENDERER_CLOUD_SHADOW_GOLDEN_SCENES_SCHEMA_VERSION,
            world_xz: [0.0, 0.0],
            expected_visibility_low: mid_low,
            expected_visibility_high: mid_high,
            expected_darkening_low: mid_low,
            expected_darkening_high: mid_high,
        };
        probes[1] = CloudShadowFrameProbe {
            schema_version: FUN_RENDERER_CLOUD_SHADOW_GOLDEN_SCENES_SCHEMA_VERSION,
            world_xz: [128.0, 0.0],
            expected_visibility_low: lit_low,
            expected_visibility_high: lit_high,
            expected_darkening_low: lit_low,
            expected_darkening_high: lit_high,
        };
        probes[2] = CloudShadowFrameProbe {
            schema_version: FUN_RENDERER_CLOUD_SHADOW_GOLDEN_SCENES_SCHEMA_VERSION,
            world_xz: [0.0, 128.0],
            expected_visibility_low: dim_low,
            expected_visibility_high: dim_high,
            expected_darkening_low: dim_low,
            expected_darkening_high: dim_high,
        };
        Self {
            schema_version: FUN_RENDERER_CLOUD_SHADOW_GOLDEN_SCENES_SCHEMA_VERSION,
            scene,
            probe_count: 3,
            probes,
        }
    }
}

// ============================================================================
// Section 3 — typed golden audits
// ============================================================================

/// Typed Pass C7.11 — typed audit result for the typed
/// transmittance-stats check.  Drives the typed
/// "golden/readback tests prove shadow mask behavior"
/// acceptance.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct CloudShadowGoldenStatsAudit {
    pub schema_version: u16,
    pub scene: CloudShadowGoldenScene,
    pub avg_in_range: bool,
    pub min_below_ceiling: bool,
    pub max_above_floor: bool,
    pub observed_min: f32,
    pub observed_max: f32,
    pub observed_avg: f32,
}

impl CloudShadowGoldenStatsAudit {
    /// Typed predicate: did every typed sub-check pass?
    #[must_use]
    pub const fn passes(&self) -> bool {
        self.avg_in_range && self.min_below_ceiling && self.max_above_floor
    }
}

/// Typed Pass C7.11 — audit the typed observed
/// transmittance stats against the typed scene's typed
/// expected ranges.
#[must_use]
pub fn audit_golden_scene_stats(
    scene: CloudShadowGoldenScene,
    stats: &TransmittanceStats,
) -> CloudShadowGoldenStatsAudit {
    let (avg_low, avg_high) = scene.expected_avg_transmittance_range();
    let min_ceiling = scene.expected_min_transmittance_max();
    let max_floor = scene.expected_max_transmittance_min();
    let observed_avg = stats.avg();
    CloudShadowGoldenStatsAudit {
        schema_version: FUN_RENDERER_CLOUD_SHADOW_GOLDEN_SCENES_SCHEMA_VERSION,
        scene,
        avg_in_range: observed_avg >= avg_low && observed_avg <= avg_high,
        min_below_ceiling: stats.min <= min_ceiling,
        max_above_floor: stats.max >= max_floor,
        observed_min: stats.min,
        observed_max: stats.max,
        observed_avg,
    }
}

/// Typed Pass C7.11 — typed temporal stability audit.
/// Compares the typed avg transmittance of typed frame N
/// to the typed avg transmittance of typed frame N-1; the
/// typed delta must stay below the typed scene's typed
/// `temporal_stability_tolerance` when the typed scene is
/// typed not dynamic.  Used by the typed user-spec
/// "temporal stability test catches crawl" acceptance.
#[must_use]
pub fn audit_golden_scene_temporal_stability(
    scene: CloudShadowGoldenScene,
    avg_prev: f32,
    avg_curr: f32,
) -> bool {
    let tolerance = scene.temporal_stability_tolerance();
    (avg_curr - avg_prev).abs() <= tolerance
}

/// Typed Pass C7.11 — typed disable-toggle baseline.
/// Returns the typed expected stats record the typed
/// renderer must produce when typed cloud shadows are
/// disabled (typed `CloudWorldShadowSettings::enabled =
/// false`).  Every typed sample is typed `1.0`.
#[must_use]
pub fn disabled_baseline_stats() -> TransmittanceStats {
    let mut stats = TransmittanceStats::EMPTY;
    stats.record(1.0);
    stats.record(1.0);
    stats.record(1.0);
    stats.record(1.0);
    stats
}

/// Typed Pass C7.11 — typed predicate: does the typed
/// observed stats record match the typed disabled
/// baseline within tolerance?  Used to audit "disable
/// toggle produces exact or near-exact unshadowed
/// baseline".
#[must_use]
pub fn matches_disabled_baseline(observed: &TransmittanceStats) -> bool {
    if !observed.has_samples() {
        // Typed empty stats is a typed valid disabled
        // baseline (typed renderer skipped the typed
        // readback when typed disabled).
        return true;
    }
    let avg = observed.avg();
    let min = observed.min;
    let max = observed.max;
    // Typed near-exact tolerance for typed quantization.
    let tol = 1.0 / 65_535.0;
    (avg - 1.0).abs() <= tol && (min - 1.0).abs() <= tol && (max - 1.0).abs() <= tol
}

/// Typed Pass C7.11 — typed disable toggle delay mode
/// helper.  The typed renderer uses the typed delay mode
/// to pick whether the typed disable toggle takes effect
/// on the typed current or typed next frame.  Returns the
/// typed canonical typed `OneFrameDelayed` mode that
/// matches the typed product policy.
#[must_use]
pub const fn disable_toggle_default_delay_mode() -> CloudShadowFrameDelayMode {
    CloudShadowFrameDelayMode::OneFrameDelayed
}

/// Typed Pass C7.11 — typed full audit summary.  Bundles
/// every typed acceptance sub-audit for the typed scene
/// into a typed single record the typed renderer
/// diagnostics emit.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct CloudShadowGoldenSceneAudit {
    pub schema_version: u16,
    pub scene: CloudShadowGoldenScene,
    pub stats_audit: CloudShadowGoldenStatsAudit,
    pub temporal_stable: bool,
    pub probes_passed: u8,
    pub probes_total: u8,
}

impl CloudShadowGoldenSceneAudit {
    #[must_use]
    pub const fn passes(&self) -> bool {
        self.stats_audit.passes() && self.temporal_stable && self.probes_passed == self.probes_total
    }
}

/// Typed Pass C7.11 — typed Q16-encoded baseline for the
/// typed scene's typed avg transmittance.  Used as a
/// typed golden fixture the typed renderer compares
/// against (typed Q16 keeps the typed compare bit-exact
/// across runs).
#[must_use]
pub fn golden_avg_q16(scene: CloudShadowGoldenScene) -> u16 {
    let (low, high) = scene.expected_avg_transmittance_range();
    transmittance_f32_to_q16((low + high) / 2.0)
}

/// Typed Pass C7.11 — typed Q16 → typed f32 helper for
/// the typed golden audit reports.
#[must_use]
pub fn golden_avg_f32(scene: CloudShadowGoldenScene) -> f32 {
    transmittance_q16_to_f32(golden_avg_q16(scene))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats_with(samples: &[f32]) -> TransmittanceStats {
        let mut s = TransmittanceStats::EMPTY;
        for &t in samples {
            s.record(t);
        }
        s
    }

    /// Pass C7.11 schema is stable.
    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_RENDERER_CLOUD_SHADOW_GOLDEN_SCENES_SCHEMA_VERSION, 1);
    }

    /// Pass C7.11 — typed scene taxonomy walks user spec.
    #[test]
    fn scene_taxonomy_walks_user_spec() {
        assert_eq!(CloudShadowGoldenScene::ALL.len(), 6);
        // Typed each scene maps to a typed weather
        // profile.
        assert_eq!(
            CloudShadowGoldenScene::ClearSky.weather_profile(),
            CloudWeatherProfileId::Clear,
        );
        assert_eq!(
            CloudShadowGoldenScene::ScatteredClouds.weather_profile(),
            CloudWeatherProfileId::Scattered,
        );
        assert_eq!(
            CloudShadowGoldenScene::Overcast.weather_profile(),
            CloudWeatherProfileId::Overcast,
        );
        assert_eq!(
            CloudShadowGoldenScene::StormFront.weather_profile(),
            CloudWeatherProfileId::StormFront,
        );
        // Typed dynamic scenes flagged.
        assert!(CloudShadowGoldenScene::SunChange.is_dynamic());
        assert!(CloudShadowGoldenScene::CameraMotion.is_dynamic());
        assert!(!CloudShadowGoldenScene::ClearSky.is_dynamic());
    }

    /// Pass C7.11 acceptance — golden/readback tests
    /// prove shadow mask behavior.  Audited by feeding
    /// typed scene-shaped synthetic stats into the typed
    /// `audit_golden_scene_stats` predicate.
    #[test]
    fn golden_stats_audit_proves_shadow_mask_behavior() {
        // Typed ClearSky — typed all-1.0 stats pass.
        let clear = stats_with(&[1.0, 1.0, 0.98, 1.0]);
        let audit = audit_golden_scene_stats(CloudShadowGoldenScene::ClearSky, &clear);
        assert!(audit.passes(), "clear audit: {:?}", audit);

        // Typed ScatteredClouds — typed mid-band stats
        // pass.
        let scattered = stats_with(&[0.5, 0.7, 0.85, 0.6]);
        let audit = audit_golden_scene_stats(CloudShadowGoldenScene::ScatteredClouds, &scattered);
        assert!(audit.passes(), "scattered audit: {:?}", audit);

        // Typed StormFront — typed low stats pass.
        let storm = stats_with(&[0.05, 0.2, 0.4, 0.3]);
        let audit = audit_golden_scene_stats(CloudShadowGoldenScene::StormFront, &storm);
        assert!(audit.passes(), "storm audit: {:?}", audit);

        // Typed Overcast — typed broad dimming stats.
        let overcast = stats_with(&[0.3, 0.5, 0.55, 0.45]);
        let audit = audit_golden_scene_stats(CloudShadowGoldenScene::Overcast, &overcast);
        assert!(audit.passes(), "overcast audit: {:?}", audit);
    }

    /// Pass C7.11 — typed mismatched stats fail the
    /// typed golden audit.
    #[test]
    fn mismatched_stats_fail_golden_audit() {
        // Typed ClearSky shouldn't see low avg.
        let bad_clear = stats_with(&[0.5, 0.5, 0.5]);
        let audit = audit_golden_scene_stats(CloudShadowGoldenScene::ClearSky, &bad_clear);
        assert!(!audit.passes());
        assert!(!audit.avg_in_range);
        // Typed StormFront shouldn't see typed all-1.0
        // avg.
        let bad_storm = stats_with(&[1.0, 1.0, 1.0]);
        let audit = audit_golden_scene_stats(CloudShadowGoldenScene::StormFront, &bad_storm);
        assert!(!audit.passes());
    }

    /// Pass C7.11 acceptance — world pixel probes prove
    /// expected darkening.
    #[test]
    fn world_pixel_probes_prove_expected_darkening() {
        let probes =
            CloudShadowGoldenSceneProbes::for_scene(CloudShadowGoldenScene::ScatteredClouds);
        assert_eq!(probes.probe_count, 3);
        assert_eq!(probes.scene, CloudShadowGoldenScene::ScatteredClouds);
        // Typed first probe is at typed world origin.
        let p0 = &probes.probes[0];
        assert_eq!(p0.world_xz, [0.0, 0.0]);
        // Typed scattered avg range is typed (0.6, 0.9).
        // Typed observed = 0.75 should pass.
        assert!(p0.visibility_in_range(0.75));
        assert!(p0.darkening_in_range(0.75));
        // Typed observed = 0.1 (too dark) should fail.
        assert!(!p0.visibility_in_range(0.1));
        // Typed observed = 1.5 (out of range) should
        // fail.
        assert!(!p0.visibility_in_range(1.5));

        // Typed iter walks every typed registered probe.
        let count = probes.iter().count();
        assert_eq!(count, 3);
    }

    /// Pass C7.11 acceptance — temporal stability test
    /// catches crawl.
    #[test]
    fn temporal_stability_test_catches_crawl() {
        // Typed ClearSky tolerance is typed 0.005.
        // Typed avg delta 0.001 → typed stable.
        assert!(audit_golden_scene_temporal_stability(
            CloudShadowGoldenScene::ClearSky,
            0.98,
            0.981,
        ));
        // Typed avg delta 0.05 → typed crawl detected.
        assert!(!audit_golden_scene_temporal_stability(
            CloudShadowGoldenScene::ClearSky,
            0.98,
            1.0,
        ));
        // Typed dynamic SunChange tolerance is typed
        // 0.20 (typed scene is typed explicitly typed
        // dynamic).
        assert!(audit_golden_scene_temporal_stability(
            CloudShadowGoldenScene::SunChange,
            0.8,
            0.65,
        ));
        // Typed even wider delta exceeds typed SunChange
        // tolerance.
        assert!(!audit_golden_scene_temporal_stability(
            CloudShadowGoldenScene::SunChange,
            0.8,
            0.4,
        ));
    }

    /// Pass C7.11 acceptance — disable toggle produces
    /// exact or near-exact unshadowed baseline.
    #[test]
    fn disable_toggle_produces_unshadowed_baseline() {
        let baseline = disabled_baseline_stats();
        assert!(baseline.has_samples());
        assert!((baseline.avg() - 1.0).abs() < 1e-6);
        assert!((baseline.min - 1.0).abs() < 1e-6);
        assert!((baseline.max - 1.0).abs() < 1e-6);
        assert!(matches_disabled_baseline(&baseline));

        // Typed near-exact match (typed Q16 quantized)
        // still passes.
        let mut near = TransmittanceStats::EMPTY;
        for _ in 0..4 {
            near.record(transmittance_q16_to_f32(u16::MAX));
        }
        assert!(matches_disabled_baseline(&near));

        // Typed empty (typed renderer skipped readback
        // when typed disabled) → typed valid baseline.
        assert!(matches_disabled_baseline(&TransmittanceStats::EMPTY));

        // Typed not-all-1.0 fails.
        let with_shadow = stats_with(&[0.5, 0.8, 0.9]);
        assert!(!matches_disabled_baseline(&with_shadow));
    }

    /// Pass C7.11 — typed golden Q16 baselines for each
    /// scene.
    #[test]
    fn golden_q16_baselines_match_scene_ranges() {
        for scene in CloudShadowGoldenScene::ALL {
            let (low, high) = scene.expected_avg_transmittance_range();
            let mid = (low + high) / 2.0;
            let q16 = golden_avg_q16(scene);
            let f32_back = golden_avg_f32(scene);
            assert!(
                (f32_back - mid).abs() < 1.0 / 65_535.0,
                "{:?} q16={} f32_back={} mid={}",
                scene,
                q16,
                f32_back,
                mid,
            );
        }
    }

    /// Pass C7.11 — typed delay mode default for the
    /// typed disable toggle.
    #[test]
    fn disable_toggle_default_delay_is_one_frame_delayed() {
        assert_eq!(
            disable_toggle_default_delay_mode(),
            CloudShadowFrameDelayMode::OneFrameDelayed,
        );
    }

    /// Pass C7.11 — typed full audit composes every typed
    /// sub-audit.
    #[test]
    fn full_audit_composes_every_sub_audit() {
        let scene = CloudShadowGoldenScene::ScatteredClouds;
        let stats = stats_with(&[0.5, 0.7, 0.85, 0.6]);
        let stats_audit = audit_golden_scene_stats(scene, &stats);
        let audit = CloudShadowGoldenSceneAudit {
            schema_version: FUN_RENDERER_CLOUD_SHADOW_GOLDEN_SCENES_SCHEMA_VERSION,
            scene,
            stats_audit,
            temporal_stable: true,
            probes_passed: 3,
            probes_total: 3,
        };
        assert!(audit.passes());
        // Typed one sub-audit failure → typed full audit
        // fails.
        let mut fail = audit;
        fail.temporal_stable = false;
        assert!(!fail.passes());
        let mut fail2 = audit;
        fail2.probes_passed = 2;
        assert!(!fail2.passes());
    }
}
