//! Pass C7.8 — typed cloud shadow runtime diagnostics +
//! debug overlays.
//!
//! Pass C7.4.2 landed the typed
//! `CloudShadowResourceDiagnostics` (typed GPU bytes +
//! extent + format).  Pass C7.4.5 landed the typed
//! `CloudShadowGraphDiagnostics` (typed pass chain +
//! reads/writes records).  This module lands the typed
//! per-frame runtime diagnostics record the typed renderer
//! debug artifact + the typed
//! `CloudDebugOverlay::ShadowMask` family overlays
//! consume:
//!
//!     CloudShadowDiagnostics {
//!         enabled,
//!         light_id,
//!         resolution,
//!         storage_format,
//!         latency,
//!         project_dispatch_count,
//!         filter_dispatch_count,
//!         project_gpu_ns,
//!         filter_gpu_ns,
//!         min_transmittance_q16,
//!         max_transmittance_q16,
//!         avg_transmittance_q16,
//!     }
//!
//! plus the typed [`CloudShadowOverlayKind`] taxonomy the
//! typed renderer dispatcher uses to pick the typed debug
//! visualization shader path.

use crate::cloud_shadow::{
    CloudShadowFrameDelayMode, CloudShadowProjectionConstants, CloudShadowResolution,
    CloudShadowResourceDiagnostics, CloudShadowStorageFormat,
};
use crate::cloud_shadow_passes::CloudShadowGraphDiagnostics;
use crate::clouds::CloudRenderSettings;
use fun_lux::LuxLightId;

pub const FUN_RENDERER_CLOUD_SHADOW_RUNTIME_DIAGNOSTICS_SCHEMA_VERSION: u16 = 1;

// ============================================================================
// Section 1 — typed CloudShadowOverlayKind
// ============================================================================

/// Typed Pass C7.8 — typed cloud shadow overlay kind.
/// Names the typed debug visualizations the typed renderer
/// can dispatch for the typed shadow path.  Mirrors the
/// typed user-spec overlay list:
///
/// - `None` — typed disabled (no overlay).
/// - `ShadowMask` — typed filtered cloud shadow transmittance
///   visualized over the typed scene.  Sourced from
///   `CloudWorldShadowFiltered`.
/// - `LuxOpaqueShadow` — typed opaque Lux virtual-shadow
///   factor visualized standalone (no cloud composition).
///   Useful for typed isolating the typed opaque shadow
///   path when diagnosing typed cloud × opaque interaction.
/// - `CloudTransmittance` — typed projected (pre-filter)
///   cloud transmittance from `CloudWorldShadowTransmittance`.
///   Useful for typed diagnosing typed raymarch issues
///   before the typed filter softens them.
/// - `CombinedLuxVisibility` — typed final direct
///   visibility (`opaque × cloud`) from the typed Pass
///   C7.6 compose path.  Useful for typed confirming the
///   typed multiplication composition + the typed user-
///   visible darkening.
/// - `CloudProjectionBounds` — typed wireframe / heatmap of
///   the typed shadow projection bounds derived from the
///   typed `CloudShadowProjectionConstants`.  Useful for
///   typed confirming the typed shadow UV ↔ world transform
///   covers the typed expected receiver footprint.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CloudShadowOverlayKind {
    #[default]
    None,
    ShadowMask,
    LuxOpaqueShadow,
    CloudTransmittance,
    CombinedLuxVisibility,
    CloudProjectionBounds,
}

impl CloudShadowOverlayKind {
    pub const ALL: [Self; 6] = [
        Self::None,
        Self::ShadowMask,
        Self::LuxOpaqueShadow,
        Self::CloudTransmittance,
        Self::CombinedLuxVisibility,
        Self::CloudProjectionBounds,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ShadowMask => "shadow_mask",
            Self::LuxOpaqueShadow => "lux_opaque_shadow",
            Self::CloudTransmittance => "cloud_transmittance",
            Self::CombinedLuxVisibility => "combined_lux_visibility",
            Self::CloudProjectionBounds => "cloud_projection_bounds",
        }
    }

    /// Typed predicate: is the typed overlay active (not
    /// typed `None`)?
    #[must_use]
    pub const fn is_active(self) -> bool {
        !matches!(self, Self::None)
    }

    /// Typed predicate: does this typed overlay sample the
    /// typed pre-filter `CloudWorldShadowTransmittance`
    /// target?
    #[must_use]
    pub const fn samples_pre_filter_transmittance(self) -> bool {
        matches!(self, Self::CloudTransmittance)
    }

    /// Typed predicate: does this typed overlay sample the
    /// typed post-filter `CloudWorldShadowFiltered` target?
    #[must_use]
    pub const fn samples_post_filter_transmittance(self) -> bool {
        matches!(self, Self::ShadowMask | Self::CombinedLuxVisibility)
    }

    /// Typed predicate: does this typed overlay require the
    /// typed Lux opaque shadow factor (sampled from typed
    /// `LuxShadowAtlas` / typed virtual shadow pages) to
    /// produce the typed final image?
    #[must_use]
    pub const fn requires_lux_opaque_shadow(self) -> bool {
        matches!(self, Self::LuxOpaqueShadow | Self::CombinedLuxVisibility)
    }

    /// Typed predicate: does this typed overlay visualize
    /// the typed projection bounds derived from
    /// `CloudShadowProjectionConstants`?
    #[must_use]
    pub const fn visualizes_projection_bounds(self) -> bool {
        matches!(self, Self::CloudProjectionBounds)
    }
}

// ============================================================================
// Section 2 — typed Q16 transmittance helpers
// ============================================================================

/// Typed Pass C7.8 — convert a typed f32 transmittance
/// (clamped to `[0, 1]`) into a typed Q16 representation
/// (`0 = 0.0`, `65_535 = 1.0`).  Used to pack the typed
/// min/max/avg transmittance fields in
/// [`CloudShadowDiagnostics`] for typed compact storage.
#[must_use]
pub fn transmittance_f32_to_q16(value: f32) -> u16 {
    let clamped = value.clamp(0.0, 1.0);
    (clamped * 65_535.0).round() as u16
}

/// Typed Pass C7.8 — convert a typed Q16 transmittance
/// back to typed f32.
#[must_use]
pub const fn transmittance_q16_to_f32(q: u16) -> f32 {
    (q as f32) / 65_535.0
}

/// Typed Pass C7.8 — typed transmittance sample
/// statistics the typed renderer computes from a typed
/// readback of the typed `CloudWorldShadowFiltered` target.
/// Drives the typed `CloudShadowDiagnostics` min/max/avg
/// fields.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct TransmittanceStats {
    pub schema_version: u16,
    pub sample_count: u32,
    pub min: f32,
    pub max: f32,
    pub sum: f32,
}

impl TransmittanceStats {
    /// Typed empty stats — no typed samples recorded.
    pub const EMPTY: Self = Self {
        schema_version: FUN_RENDERER_CLOUD_SHADOW_RUNTIME_DIAGNOSTICS_SCHEMA_VERSION,
        sample_count: 0,
        min: 1.0,
        max: 0.0,
        sum: 0.0,
    };

    /// Typed Pass C7.8 — record a typed transmittance
    /// sample.  Updates the typed min/max/sum + the typed
    /// count.
    pub fn record(&mut self, transmittance: f32) {
        let t = transmittance.clamp(0.0, 1.0);
        if self.sample_count == 0 {
            self.min = t;
            self.max = t;
        } else {
            if t < self.min {
                self.min = t;
            }
            if t > self.max {
                self.max = t;
            }
        }
        self.sum += t;
        self.sample_count = self.sample_count.saturating_add(1);
    }

    /// Typed average transmittance across the typed
    /// recorded samples.  Returns typed `1.0` when no
    /// samples have been recorded (typed neutral
    /// baseline).
    #[must_use]
    pub fn avg(&self) -> f32 {
        if self.sample_count == 0 {
            return 1.0;
        }
        (self.sum / self.sample_count as f32).clamp(0.0, 1.0)
    }

    /// Typed predicate: does the typed stats record carry
    /// any samples?
    #[must_use]
    pub const fn has_samples(&self) -> bool {
        self.sample_count > 0
    }
}

// ============================================================================
// Section 3 — typed CloudShadowDiagnostics
// ============================================================================

/// Typed Pass C7.8 cloud shadow runtime diagnostics
/// record.  Per the user spec, carries:
///
/// - `enabled` — typed pass active this frame.
/// - `light_id` — typed Lux directional light id the
///   typed projection covers.
/// - `resolution` — typed `CloudShadowResolution`.
/// - `storage_format` — typed transmittance storage
///   format.
/// - `latency` — typed
///   `CloudShadowFrameDelayMode`.
/// - `project_dispatch_count` /
///   `filter_dispatch_count` — typed counts of typed
///   `LuxCloudShadowProject` / `LuxCloudShadowFilter`
///   dispatches the typed frame executed.
/// - `project_gpu_ns` / `filter_gpu_ns` — typed GPU
///   timings per pass (nanoseconds; typed `0` when
///   typed timing infrastructure is not wired).
/// - `min_transmittance_q16` /
///   `max_transmittance_q16` /
///   `avg_transmittance_q16` — typed Q16 transmittance
///   stats from the typed
///   `CloudWorldShadowFiltered` readback.  Typed `0`
///   indicates typed full occlusion; typed `65_535`
///   indicates typed full sunlight.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CloudShadowDiagnostics {
    pub schema_version: u16,
    pub enabled: bool,
    pub light_id: LuxLightId,
    pub resolution: CloudShadowResolution,
    pub storage_format: CloudShadowStorageFormat,
    pub latency: CloudShadowFrameDelayMode,
    pub project_dispatch_count: u32,
    pub filter_dispatch_count: u32,
    pub project_gpu_ns: u64,
    pub filter_gpu_ns: u64,
    pub min_transmittance_q16: u16,
    pub max_transmittance_q16: u16,
    pub avg_transmittance_q16: u16,
}

impl CloudShadowDiagnostics {
    /// Typed cold default — typed disabled diagnostics
    /// (typed cloud shadow pass not active this frame).
    pub const COLD_DEFAULT: Self = Self {
        schema_version: FUN_RENDERER_CLOUD_SHADOW_RUNTIME_DIAGNOSTICS_SCHEMA_VERSION,
        enabled: false,
        light_id: LuxLightId::INVALID,
        resolution: CloudShadowResolution::Balanced2048,
        storage_format: CloudShadowStorageFormat::R16Float,
        latency: CloudShadowFrameDelayMode::OneFrameDelayed,
        project_dispatch_count: 0,
        filter_dispatch_count: 0,
        project_gpu_ns: 0,
        filter_gpu_ns: 0,
        min_transmittance_q16: u16::MAX,
        max_transmittance_q16: 0,
        avg_transmittance_q16: u16::MAX,
    };

    /// Typed Pass C7.8 builder — derive a typed runtime
    /// diagnostics record from the typed C7.4.x inputs +
    /// typed optional GPU timings + typed optional
    /// readback stats.
    ///
    /// Inputs:
    /// - `settings` — typed `CloudRenderSettings`.
    /// - `constants` — typed
    ///   `CloudShadowProjectionConstants` (typed light_id
    ///   source).
    /// - `latency` — typed delay mode.
    /// - `resources` — typed `CloudShadowResourceDiagnostics`
    ///   (typed format + extent).
    /// - `dispatch_counts` — `Some((project, filter))` when
    ///   the typed dispatch path ran; `None` when typed
    ///   gated off.
    /// - `gpu_times` — `Some((project_ns, filter_ns))` when
    ///   typed GPU timing infrastructure is wired; `None`
    ///   when not.
    /// - `transmittance_stats` — `Some(stats)` when typed
    ///   readback was performed; `None` when readback was
    ///   skipped this frame.
    #[must_use]
    pub fn from_inputs(
        settings: &CloudRenderSettings,
        constants: &CloudShadowProjectionConstants,
        latency: CloudShadowFrameDelayMode,
        resources: &CloudShadowResourceDiagnostics,
        dispatch_counts: Option<(u32, u32)>,
        gpu_times: Option<(u64, u64)>,
        transmittance_stats: Option<TransmittanceStats>,
    ) -> Self {
        let projection_live = constants.projects_world_shadow();
        let enabled =
            settings.registers_world_shadow_pass() && projection_live && resources.enabled;
        if !enabled {
            return Self {
                latency,
                ..Self::COLD_DEFAULT
            };
        }
        let (project_dispatch_count, filter_dispatch_count) =
            dispatch_counts.unwrap_or((0, 0));
        let (project_gpu_ns, filter_gpu_ns) = gpu_times.unwrap_or((0, 0));
        let (min_q16, max_q16, avg_q16) = match transmittance_stats {
            Some(stats) if stats.has_samples() => (
                transmittance_f32_to_q16(stats.min),
                transmittance_f32_to_q16(stats.max),
                transmittance_f32_to_q16(stats.avg()),
            ),
            _ => (u16::MAX, 0, u16::MAX),
        };
        Self {
            schema_version: FUN_RENDERER_CLOUD_SHADOW_RUNTIME_DIAGNOSTICS_SCHEMA_VERSION,
            enabled: true,
            light_id: constants.light_id,
            resolution: settings.world_shadows.resolution,
            storage_format: resources.transmittance_format,
            latency,
            project_dispatch_count,
            filter_dispatch_count,
            project_gpu_ns,
            filter_gpu_ns,
            min_transmittance_q16: min_q16,
            max_transmittance_q16: max_q16,
            avg_transmittance_q16: avg_q16,
        }
    }

    /// Typed predicate: did the typed pass declare any
    /// typed dispatches this frame?
    #[must_use]
    pub const fn declares_any_dispatch(&self) -> bool {
        self.project_dispatch_count > 0 || self.filter_dispatch_count > 0
    }

    /// Typed predicate: do the typed GPU timings carry any
    /// recorded nanoseconds?
    #[must_use]
    pub const fn has_gpu_timings(&self) -> bool {
        self.project_gpu_ns > 0 || self.filter_gpu_ns > 0
    }

    /// Typed predicate: do the typed transmittance stats
    /// carry recorded samples (non-default min/max)?
    #[must_use]
    pub const fn has_transmittance_stats(&self) -> bool {
        // Typed `max > 0` indicates typed at least one
        // recorded sample.  Typed `COLD_DEFAULT` has typed
        // `max_transmittance_q16 = 0`.
        self.max_transmittance_q16 > 0 || self.min_transmittance_q16 < u16::MAX
    }

    /// Typed total typed GPU nanoseconds spent on the typed
    /// cloud shadow chain (project + filter).
    #[must_use]
    pub const fn total_gpu_ns(&self) -> u64 {
        self.project_gpu_ns.saturating_add(self.filter_gpu_ns)
    }
}

// ============================================================================
// Section 4 — typed debug section emitter
// ============================================================================

/// Typed Pass C7.8 — emit the typed "Cloud Shadow
/// Diagnostics" multi-line debug section.  Lists every
/// typed pass from the typed graph diagnostics + the typed
/// per-pass dispatch counts + the typed per-pass GPU
/// timings + the typed transmittance min/max/avg.
///
/// Drives the typed user-spec acceptance bullets:
/// - "Debug artifact lists project/filter/register passes."
/// - "Min/max/avg transmittance are reported."
/// - "GPU timings are visible per pass."
#[must_use]
pub fn cloud_shadow_diagnostics_section(
    diagnostics: &CloudShadowDiagnostics,
    chain: &CloudShadowGraphDiagnostics,
    overlay: CloudShadowOverlayKind,
) -> String {
    use core::fmt::Write as _;
    let mut content = String::new();
    let _ = writeln!(content, "Cloud Shadow Diagnostics");
    let _ = writeln!(content, "------------------------");
    let _ = writeln!(content, "enabled: {}", diagnostics.enabled);
    let _ = writeln!(content, "light_id: {}", diagnostics.light_id.0);
    let _ = writeln!(content, "resolution: {}", diagnostics.resolution.as_str());
    let _ = writeln!(
        content,
        "storage_format: {}",
        diagnostics.storage_format.as_str(),
    );
    let _ = writeln!(content, "latency: {}", diagnostics.latency.as_str());
    let _ = writeln!(content, "overlay: {}", overlay.as_str());
    let _ = writeln!(content, "pass_chain:");
    for record in chain.as_slice() {
        let (dispatch_count, gpu_ns) = match record.role {
            crate::frame_graph::FrameGraphPassRole::LuxCloudShadowProject => (
                diagnostics.project_dispatch_count,
                diagnostics.project_gpu_ns,
            ),
            crate::frame_graph::FrameGraphPassRole::LuxCloudShadowFilter => (
                diagnostics.filter_dispatch_count,
                diagnostics.filter_gpu_ns,
            ),
            // Typed register pass has no typed GPU dispatch —
            // it produces typed CPU-side aux-layer metadata.
            _ => (0, 0),
        };
        let _ = writeln!(
            content,
            "  {} order_key={} registers={} dispatches={} gpu_ns={}",
            record.role.as_str(),
            record.order_key,
            record.registers_pass,
            dispatch_count,
            gpu_ns,
        );
    }
    let _ = writeln!(
        content,
        "transmittance_min_q16: {} ({:.4})",
        diagnostics.min_transmittance_q16,
        transmittance_q16_to_f32(diagnostics.min_transmittance_q16),
    );
    let _ = writeln!(
        content,
        "transmittance_max_q16: {} ({:.4})",
        diagnostics.max_transmittance_q16,
        transmittance_q16_to_f32(diagnostics.max_transmittance_q16),
    );
    let _ = writeln!(
        content,
        "transmittance_avg_q16: {} ({:.4})",
        diagnostics.avg_transmittance_q16,
        transmittance_q16_to_f32(diagnostics.avg_transmittance_q16),
    );
    let _ = writeln!(content, "total_gpu_ns: {}", diagnostics.total_gpu_ns());
    content
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud_shadow::CloudShadowProjectionConstants;
    use crate::cloud_shadow_passes::CloudShadowGraphDiagnostics;
    use crate::clouds::{CloudRenderSettings, CloudWeatherProfileId};

    fn live_constants() -> CloudShadowProjectionConstants {
        CloudShadowProjectionConstants::from_inputs(
            &CloudRenderSettings::PRODUCT_DEFAULT,
            CloudWeatherProfileId::Scattered,
            LuxLightId::new(42),
            [0.0, 1.0, 0.0],
            0,
        )
    }

    /// Pass C7.8 schema version is stable.
    #[test]
    fn schema_version_is_stable() {
        assert_eq!(
            FUN_RENDERER_CLOUD_SHADOW_RUNTIME_DIAGNOSTICS_SCHEMA_VERSION,
            1,
        );
    }

    /// Pass C7.8 acceptance — typed overlay taxonomy is
    /// dense; covers every typed user-spec overlay.
    #[test]
    fn overlay_taxonomy_is_dense() {
        assert_eq!(CloudShadowOverlayKind::ALL.len(), 6);
        let mut seen = std::collections::HashSet::new();
        for k in CloudShadowOverlayKind::ALL {
            assert!(seen.insert(k.as_str()), "duplicate: {}", k.as_str());
        }
        // Typed default is typed None.
        assert_eq!(CloudShadowOverlayKind::default(), CloudShadowOverlayKind::None);
        // Typed user-spec overlays.
        assert!(CloudShadowOverlayKind::ShadowMask.is_active());
        assert!(CloudShadowOverlayKind::LuxOpaqueShadow.is_active());
        assert!(CloudShadowOverlayKind::CloudTransmittance.is_active());
        assert!(CloudShadowOverlayKind::CombinedLuxVisibility.is_active());
        assert!(CloudShadowOverlayKind::CloudProjectionBounds.is_active());
        assert!(!CloudShadowOverlayKind::None.is_active());
        // Typed source predicates.
        assert!(CloudShadowOverlayKind::CloudTransmittance.samples_pre_filter_transmittance());
        assert!(CloudShadowOverlayKind::ShadowMask.samples_post_filter_transmittance());
        assert!(CloudShadowOverlayKind::CombinedLuxVisibility.samples_post_filter_transmittance());
        assert!(CloudShadowOverlayKind::LuxOpaqueShadow.requires_lux_opaque_shadow());
        assert!(CloudShadowOverlayKind::CombinedLuxVisibility.requires_lux_opaque_shadow());
        assert!(CloudShadowOverlayKind::CloudProjectionBounds.visualizes_projection_bounds());
    }

    /// Pass C7.8 — typed Q16 conversion round-trips with
    /// typed quantization tolerance.
    #[test]
    fn q16_conversion_round_trips_with_tolerance() {
        // Typed extreme cases.
        assert_eq!(transmittance_f32_to_q16(0.0), 0);
        assert_eq!(transmittance_f32_to_q16(1.0), u16::MAX);
        assert_eq!(transmittance_q16_to_f32(0), 0.0);
        assert!((transmittance_q16_to_f32(u16::MAX) - 1.0).abs() < 1e-6);
        // Typed mid range.
        let mid = transmittance_f32_to_q16(0.5);
        assert!((mid as i32 - 32_768).abs() <= 1, "q16 mid = {}", mid);
        let mid_f32 = transmittance_q16_to_f32(mid);
        assert!((mid_f32 - 0.5).abs() < 1e-4);
        // Typed clamp on typed out-of-range inputs.
        assert_eq!(transmittance_f32_to_q16(-1.0), 0);
        assert_eq!(transmittance_f32_to_q16(2.0), u16::MAX);
    }

    /// Pass C7.8 — typed transmittance stats record +
    /// aggregate.
    #[test]
    fn transmittance_stats_record_and_aggregate() {
        let mut stats = TransmittanceStats::EMPTY;
        assert!(!stats.has_samples());
        assert_eq!(stats.avg(), 1.0); // typed neutral

        stats.record(0.8);
        stats.record(0.4);
        stats.record(1.0);
        stats.record(0.0);
        assert!(stats.has_samples());
        assert_eq!(stats.sample_count, 4);
        assert_eq!(stats.min, 0.0);
        assert_eq!(stats.max, 1.0);
        // Typed avg = (0.8 + 0.4 + 1.0 + 0.0) / 4 = 0.55.
        assert!((stats.avg() - 0.55).abs() < 1e-6);

        // Typed out-of-range samples clamp to typed [0, 1].
        let mut stats2 = TransmittanceStats::EMPTY;
        stats2.record(-0.5);
        stats2.record(1.5);
        assert_eq!(stats2.min, 0.0);
        assert_eq!(stats2.max, 1.0);
    }

    /// Pass C7.8 acceptance — typed diagnostics builder
    /// populates the typed user-spec fields.
    #[test]
    fn diagnostics_builder_populates_user_spec_fields() {
        let settings = CloudRenderSettings::PRODUCT_DEFAULT;
        let constants = live_constants();
        let resources = CloudShadowResourceDiagnostics::from_settings(&settings, false, true);
        let mut stats = TransmittanceStats::EMPTY;
        stats.record(0.4);
        stats.record(0.6);
        stats.record(1.0);

        let diag = CloudShadowDiagnostics::from_inputs(
            &settings,
            &constants,
            CloudShadowFrameDelayMode::OneFrameDelayed,
            &resources,
            Some((1, 1)),
            Some((250_000, 180_000)),
            Some(stats),
        );
        assert!(diag.enabled);
        assert_eq!(diag.light_id, LuxLightId::new(42));
        assert_eq!(diag.resolution, CloudShadowResolution::Balanced2048);
        assert_eq!(diag.storage_format, CloudShadowStorageFormat::R16Float);
        assert_eq!(diag.latency, CloudShadowFrameDelayMode::OneFrameDelayed);
        assert_eq!(diag.project_dispatch_count, 1);
        assert_eq!(diag.filter_dispatch_count, 1);
        assert_eq!(diag.project_gpu_ns, 250_000);
        assert_eq!(diag.filter_gpu_ns, 180_000);
        assert_eq!(diag.total_gpu_ns(), 430_000);
        assert!(diag.declares_any_dispatch());
        assert!(diag.has_gpu_timings());
        assert!(diag.has_transmittance_stats());
        // Typed min = 0.4 → Q16 ≈ 26_214; typed max = 1.0
        // → Q16 = 65_535; typed avg ≈ 0.667 → Q16 ≈
        // 43_690.
        assert_eq!(diag.min_transmittance_q16, transmittance_f32_to_q16(0.4));
        assert_eq!(diag.max_transmittance_q16, u16::MAX);
        let avg_round_trip = transmittance_q16_to_f32(diag.avg_transmittance_q16);
        assert!((avg_round_trip - (0.4 + 0.6 + 1.0) / 3.0).abs() < 1e-3);
    }

    /// Pass C7.8 — typed disabled settings produce the
    /// typed cold-default-shaped diagnostics.
    #[test]
    fn diagnostics_disabled_path_returns_cold_default_shape() {
        let resources = CloudShadowResourceDiagnostics::from_settings(
            &CloudRenderSettings::DISABLED,
            false,
            false,
        );
        let constants = CloudShadowProjectionConstants::DISABLED;
        let diag = CloudShadowDiagnostics::from_inputs(
            &CloudRenderSettings::DISABLED,
            &constants,
            CloudShadowFrameDelayMode::SameFrame,
            &resources,
            None,
            None,
            None,
        );
        assert!(!diag.enabled);
        assert!(!diag.declares_any_dispatch());
        assert!(!diag.has_gpu_timings());
        assert!(!diag.has_transmittance_stats());
        // Typed `latency` was preserved through the typed
        // gate.
        assert_eq!(diag.latency, CloudShadowFrameDelayMode::SameFrame);
    }

    /// Pass C7.8 acceptance — typed debug artifact lists
    /// typed project / filter / register passes.
    #[test]
    fn debug_artifact_lists_project_filter_register_passes() {
        let settings = CloudRenderSettings::PRODUCT_DEFAULT;
        let constants = live_constants();
        let resources = CloudShadowResourceDiagnostics::from_settings(&settings, false, true);
        let chain = CloudShadowGraphDiagnostics::from_inputs(
            &settings,
            &constants,
            CloudShadowFrameDelayMode::OneFrameDelayed,
        );
        let mut stats = TransmittanceStats::EMPTY;
        stats.record(0.5);
        let diag = CloudShadowDiagnostics::from_inputs(
            &settings,
            &constants,
            CloudShadowFrameDelayMode::OneFrameDelayed,
            &resources,
            Some((1, 1)),
            Some((300_000, 200_000)),
            Some(stats),
        );
        let section =
            cloud_shadow_diagnostics_section(&diag, &chain, CloudShadowOverlayKind::ShadowMask);
        assert!(section.contains("Cloud Shadow Diagnostics"));
        assert!(section.contains("enabled: true"));
        assert!(section.contains("light_id: 42"));
        assert!(section.contains("resolution: balanced_2048"));
        assert!(section.contains("storage_format: r16_float"));
        assert!(section.contains("latency: one_frame_delayed"));
        assert!(section.contains("overlay: shadow_mask"));
        // Typed every pass appears.
        assert!(section.contains("lux_cloud_shadow_project"));
        assert!(section.contains("lux_cloud_shadow_filter"));
        assert!(section.contains("lux_cloud_shadow_register_layer"));
        assert!(section.contains("order_key=140"));
        assert!(section.contains("order_key=145"));
        assert!(section.contains("order_key=150"));
    }

    /// Pass C7.8 acceptance — typed shadow mask overlay
    /// renders (typed overlay kind reports active +
    /// samples the typed post-filter target).
    #[test]
    fn shadow_mask_overlay_renders() {
        let overlay = CloudShadowOverlayKind::ShadowMask;
        assert!(overlay.is_active());
        assert!(overlay.samples_post_filter_transmittance());
        assert!(!overlay.samples_pre_filter_transmittance());
        assert!(!overlay.requires_lux_opaque_shadow());
        assert_eq!(overlay.as_str(), "shadow_mask");
    }

    /// Pass C7.8 acceptance — typed min/max/avg
    /// transmittance are reported in the typed debug
    /// section.
    #[test]
    fn debug_section_reports_min_max_avg_transmittance() {
        let settings = CloudRenderSettings::PRODUCT_DEFAULT;
        let constants = live_constants();
        let resources = CloudShadowResourceDiagnostics::from_settings(&settings, false, true);
        let chain = CloudShadowGraphDiagnostics::from_inputs(
            &settings,
            &constants,
            CloudShadowFrameDelayMode::SameFrame,
        );
        let mut stats = TransmittanceStats::EMPTY;
        stats.record(0.2);
        stats.record(0.8);
        let diag = CloudShadowDiagnostics::from_inputs(
            &settings,
            &constants,
            CloudShadowFrameDelayMode::SameFrame,
            &resources,
            Some((1, 1)),
            Some((100_000, 50_000)),
            Some(stats),
        );
        let section =
            cloud_shadow_diagnostics_section(&diag, &chain, CloudShadowOverlayKind::ShadowMask);
        assert!(section.contains("transmittance_min_q16:"));
        assert!(section.contains("transmittance_max_q16:"));
        assert!(section.contains("transmittance_avg_q16:"));
        // Typed min = 0.2 → 0.2000; typed max = 0.8 →
        // 0.8000; typed avg = 0.5 → 0.5000.
        assert!(section.contains("(0.2000)"));
        assert!(section.contains("(0.8000)"));
        assert!(section.contains("(0.5000)"));
    }

    /// Pass C7.8 acceptance — typed GPU timings are
    /// visible per pass in the typed debug section.
    #[test]
    fn debug_section_reports_gpu_timings_per_pass() {
        let settings = CloudRenderSettings::PRODUCT_DEFAULT;
        let constants = live_constants();
        let resources = CloudShadowResourceDiagnostics::from_settings(&settings, false, true);
        let chain = CloudShadowGraphDiagnostics::from_inputs(
            &settings,
            &constants,
            CloudShadowFrameDelayMode::OneFrameDelayed,
        );
        let diag = CloudShadowDiagnostics::from_inputs(
            &settings,
            &constants,
            CloudShadowFrameDelayMode::OneFrameDelayed,
            &resources,
            Some((1, 1)),
            Some((250_000, 180_000)),
            None,
        );
        let section =
            cloud_shadow_diagnostics_section(&diag, &chain, CloudShadowOverlayKind::None);
        // Typed per-pass GPU timings appear next to their
        // typed role.
        assert!(
            section.contains("lux_cloud_shadow_project")
                && section.contains("gpu_ns=250000"),
        );
        assert!(
            section.contains("lux_cloud_shadow_filter")
                && section.contains("gpu_ns=180000"),
        );
        // Typed register pass has no GPU dispatch.
        assert!(section.contains("lux_cloud_shadow_register_layer"));
        // Typed total_gpu_ns line.
        assert!(section.contains("total_gpu_ns: 430000"));
    }
}
