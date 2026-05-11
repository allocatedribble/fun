//! Pass C7.9 — typed cloud shadow quality director +
//! update scheduler.
//!
//! Decides when to refresh the typed cloud shadow chain
//! based on:
//!
//! - typed pass GPU timings (budget pressure),
//! - typed weather change signal,
//! - typed sun change signal,
//! - typed camera movement (snap threshold),
//! - typed world-stream event (forced refresh),
//! - typed quality tier policy.
//!
//! Policy (per user spec):
//! - `Cheap`: typed lower resolution, typed every N
//!   frames (typed default N = 6).
//! - `Balanced`: typed 2048, typed every N frames
//!   (typed default N = 4) or typed
//!   weather/sun change.
//! - `Cinematic`: typed same-frame or typed tighter
//!   cadence (typed default N = 1).
//! - `Debug`: typed optional every-frame (typed N = 1
//!   always).

use crate::cloud_shadow::{CloudShadowFrameDelayMode, CloudShadowResolution};

pub const FUN_RENDERER_CLOUD_SHADOW_DIRECTOR_SCHEMA_VERSION: u16 = 1;

// ============================================================================
// Section 1 — typed quality tier policy
// ============================================================================

/// Typed Pass C7.9 quality tier.  Maps to the typed
/// `CloudQuality` taxonomy + adds the typed `Debug` tier
/// for the typed always-refresh / typed every-frame path
/// the typed debug overlay relies on.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CloudShadowQualityTier {
    /// Typed cheap tier — typed lower resolution, typed
    /// long cadence.
    Cheap,
    /// Typed balanced tier — typed product default.
    /// Typed 2048 resolution, typed cadence-driven
    /// refresh, typed weather/sun change forces refresh.
    #[default]
    Balanced,
    /// Typed cinematic tier — typed 4096 resolution, typed
    /// same-frame or typed tight cadence.
    Cinematic,
    /// Typed debug tier — typed every-frame refresh.
    Debug,
}

impl CloudShadowQualityTier {
    pub const ALL: [Self; 4] = [Self::Cheap, Self::Balanced, Self::Cinematic, Self::Debug];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cheap => "cheap",
            Self::Balanced => "balanced",
            Self::Cinematic => "cinematic",
            Self::Debug => "debug",
        }
    }

    /// Typed default cadence (frames between refreshes)
    /// for the typed quality tier.
    #[must_use]
    pub const fn default_cadence_frames(self) -> u32 {
        match self {
            Self::Cheap => 6,
            Self::Balanced => 4,
            Self::Cinematic => 1,
            Self::Debug => 1,
        }
    }

    /// Typed default resolution for the typed quality
    /// tier.
    #[must_use]
    pub const fn default_resolution(self) -> CloudShadowResolution {
        match self {
            Self::Cheap => CloudShadowResolution::Cheap1024,
            Self::Balanced => CloudShadowResolution::Balanced2048,
            Self::Cinematic => CloudShadowResolution::Cinematic4096,
            Self::Debug => CloudShadowResolution::Balanced2048,
        }
    }

    /// Typed default frame-delay mode for the typed
    /// quality tier.
    #[must_use]
    pub const fn default_latency(self) -> CloudShadowFrameDelayMode {
        match self {
            Self::Cheap | Self::Balanced => CloudShadowFrameDelayMode::OneFrameDelayed,
            Self::Cinematic | Self::Debug => CloudShadowFrameDelayMode::SameFrame,
        }
    }

    /// Typed predicate: does this typed tier refresh
    /// every frame regardless of cadence + signals?
    #[must_use]
    pub const fn refreshes_every_frame(self) -> bool {
        matches!(self, Self::Debug)
    }

    /// Typed predicate: does this typed tier honor the
    /// typed weather/sun-change refresh signals?
    #[must_use]
    pub const fn honors_change_signals(self) -> bool {
        // Typed every tier honors change signals — they
        // are typed cheap to evaluate and typed mandatory
        // for typed perceptual quality (typed sun jump
        // shouldn't leave typed stale shadows around).
        true
    }

    /// Typed predicate: does this typed tier permit the
    /// typed quality director to downgrade typed
    /// resolution under typed budget pressure?
    #[must_use]
    pub const fn permits_resolution_downgrade(self) -> bool {
        matches!(self, Self::Balanced | Self::Cinematic)
    }
}

// ============================================================================
// Section 2 — typed director budget + inputs
// ============================================================================

/// Typed Pass C7.9 — typed budget caps the typed director
/// applies when deciding whether to downgrade typed
/// resolution / cadence.  The typed renderer reads these
/// caps from the typed product quality settings; the typed
/// director compares the typed last frame's GPU times
/// against them.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct CloudShadowDirectorBudget {
    pub schema_version: u16,
    /// Typed max acceptable GPU nanoseconds for the typed
    /// `LuxCloudShadowProject` dispatch.  Zero typed
    /// disables budget gating.
    pub project_budget_ns: u64,
    /// Typed max acceptable GPU nanoseconds for the typed
    /// `LuxCloudShadowFilter` dispatch.
    pub filter_budget_ns: u64,
    /// Typed max resolution the typed director may
    /// upgrade to under typed clear-budget conditions.
    pub max_resolution: CloudShadowResolution,
    /// Typed camera movement threshold (meters) above
    /// which the typed projection center snaps + a typed
    /// refresh is required.
    pub camera_snap_threshold_meters: f32,
}

impl CloudShadowDirectorBudget {
    /// Typed product-default budget — typed `Balanced`
    /// tier targets.  `300_000` ns project + `200_000` ns
    /// filter per frame (~0.5 ms total at 2048x2048);
    /// typed camera snap = typed 64 m (typed terrain
    /// unit).
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_RENDERER_CLOUD_SHADOW_DIRECTOR_SCHEMA_VERSION,
        project_budget_ns: 300_000,
        filter_budget_ns: 200_000,
        max_resolution: CloudShadowResolution::Balanced2048,
        camera_snap_threshold_meters: 64.0,
    };

    /// Typed cinematic-capture budget — typed wider GPU
    /// caps for typed 4096x4096 shadow targets.
    pub const CINEMATIC: Self = Self {
        schema_version: FUN_RENDERER_CLOUD_SHADOW_DIRECTOR_SCHEMA_VERSION,
        project_budget_ns: 1_200_000,
        filter_budget_ns: 800_000,
        max_resolution: CloudShadowResolution::Cinematic4096,
        camera_snap_threshold_meters: 32.0,
    };

    /// Typed predicate: are the typed project + filter
    /// GPU budgets active (non-zero)?
    #[must_use]
    pub const fn budgets_active(&self) -> bool {
        self.project_budget_ns > 0 || self.filter_budget_ns > 0
    }
}

/// Typed Pass C7.9 — typed director inputs the typed
/// renderer computes each frame.  The typed
/// `decide_cloud_shadow_refresh` consumer reads these
/// inputs + the typed [`CloudShadowDirectorBudget`] to
/// produce the typed [`CloudShadowDirectorDecision`].
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct CloudShadowDirectorInputs {
    pub schema_version: u16,
    pub quality_tier: CloudShadowQualityTier,
    pub current_resolution: CloudShadowResolution,
    pub frame_index: u32,
    pub frames_since_last_refresh: u32,
    pub last_project_gpu_ns: u64,
    pub last_filter_gpu_ns: u64,
    /// Typed weather profile or weather-mask change
    /// detected this frame.
    pub weather_changed: bool,
    /// Typed sun direction / intensity change detected
    /// this frame (above the typed perceptual threshold).
    pub sun_changed: bool,
    /// Typed camera movement magnitude (meters) since
    /// the typed last refresh.  Triggers the typed
    /// projection-center snap when above
    /// `camera_snap_threshold_meters`.
    pub camera_movement_meters: f32,
    /// Typed world-stream event (typed chunk swap /
    /// typed entity bulk-update) pending this frame.
    /// Forces a typed refresh so the typed shadow
    /// stays consistent with the typed new world state.
    pub world_stream_event_pending: bool,
}

impl CloudShadowDirectorInputs {
    /// Typed stable baseline — typed no signals firing.
    /// Used as a typed test fixture + typed renderer
    /// reset state.
    pub const STABLE: Self = Self {
        schema_version: FUN_RENDERER_CLOUD_SHADOW_DIRECTOR_SCHEMA_VERSION,
        quality_tier: CloudShadowQualityTier::Balanced,
        current_resolution: CloudShadowResolution::Balanced2048,
        frame_index: 0,
        frames_since_last_refresh: 0,
        last_project_gpu_ns: 0,
        last_filter_gpu_ns: 0,
        weather_changed: false,
        sun_changed: false,
        camera_movement_meters: 0.0,
        world_stream_event_pending: false,
    };
}

// ============================================================================
// Section 3 — typed director decision + reason
// ============================================================================

/// Typed Pass C7.9 — typed refresh action the typed
/// director picks for the frame.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CloudShadowRefreshAction {
    /// Typed skip — typed neither project nor filter
    /// dispatch this frame.  Typed downstream consumers
    /// sample the typed previous frame's typed filtered
    /// shadow (typed one-frame-delayed mode).
    #[default]
    Skip,
    /// Typed refresh both passes — typed run typed
    /// project + typed filter.  Typed normal cadence
    /// path.
    RefreshProjectAndFilter,
    /// Typed refresh filter only — typed re-blur the
    /// typed previous frame's typed projected
    /// transmittance.  Used when the typed projection
    /// is typed stale but the typed weather hasn't
    /// changed (typed cheap re-filter; typed planned
    /// for typed future per-tier policy).
    RefreshFilterOnly,
    /// Typed forced refresh — typed weather / sun jump
    /// / typed world-stream event / typed debug
    /// override.  Same dispatch shape as typed
    /// `RefreshProjectAndFilter` but the typed
    /// `reason` enum distinguishes the typed forcing
    /// signal.
    ForceRefresh,
}

impl CloudShadowRefreshAction {
    pub const ALL: [Self; 4] = [
        Self::Skip,
        Self::RefreshProjectAndFilter,
        Self::RefreshFilterOnly,
        Self::ForceRefresh,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Skip => "skip",
            Self::RefreshProjectAndFilter => "refresh_project_and_filter",
            Self::RefreshFilterOnly => "refresh_filter_only",
            Self::ForceRefresh => "force_refresh",
        }
    }

    /// Typed predicate: does the typed action run the
    /// typed project dispatch this frame?
    #[must_use]
    pub const fn runs_project(self) -> bool {
        matches!(self, Self::RefreshProjectAndFilter | Self::ForceRefresh)
    }

    /// Typed predicate: does the typed action run the
    /// typed filter dispatch this frame?
    #[must_use]
    pub const fn runs_filter(self) -> bool {
        matches!(
            self,
            Self::RefreshProjectAndFilter | Self::RefreshFilterOnly | Self::ForceRefresh,
        )
    }

    /// Typed predicate: did the typed director skip the
    /// typed dispatch this frame?
    #[must_use]
    pub const fn skipped(self) -> bool {
        matches!(self, Self::Skip)
    }
}

/// Typed Pass C7.9 — typed refresh reason.  Surfaced in
/// the typed director decision so the typed renderer
/// diagnostics can audit "why did the typed director
/// refresh / skip this frame?".
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CloudShadowRefreshReason {
    /// Typed stable inputs + typed cadence not yet
    /// reached → typed skip.
    #[default]
    StableSkipped,
    /// Typed cadence reached → typed normal refresh.
    CadenceReached,
    /// Typed weather changed → typed forced refresh.
    WeatherChanged,
    /// Typed sun changed → typed forced refresh.
    SunChanged,
    /// Typed camera moved past the typed snap threshold
    /// → typed forced refresh + typed projection center
    /// snap.
    CameraSnap,
    /// Typed world-stream event pending → typed forced
    /// refresh.
    WorldStreamEvent,
    /// Typed `Debug` quality tier → typed every-frame
    /// refresh.
    DebugForce,
    /// Typed budget pressure → typed downgrade
    /// resolution + typed refresh at typed lower cost.
    BudgetDowngrade,
}

impl CloudShadowRefreshReason {
    pub const ALL: [Self; 8] = [
        Self::StableSkipped,
        Self::CadenceReached,
        Self::WeatherChanged,
        Self::SunChanged,
        Self::CameraSnap,
        Self::WorldStreamEvent,
        Self::DebugForce,
        Self::BudgetDowngrade,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StableSkipped => "stable_skipped",
            Self::CadenceReached => "cadence_reached",
            Self::WeatherChanged => "weather_changed",
            Self::SunChanged => "sun_changed",
            Self::CameraSnap => "camera_snap",
            Self::WorldStreamEvent => "world_stream_event",
            Self::DebugForce => "debug_force",
            Self::BudgetDowngrade => "budget_downgrade",
        }
    }

    /// Typed predicate: is the typed reason a typed
    /// forced refresh (typed weather / sun / camera /
    /// stream / debug)?
    #[must_use]
    pub const fn is_forced(self) -> bool {
        matches!(
            self,
            Self::WeatherChanged
                | Self::SunChanged
                | Self::CameraSnap
                | Self::WorldStreamEvent
                | Self::DebugForce,
        )
    }
}

/// Typed Pass C7.9 — typed director decision the typed
/// renderer applies to the typed frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CloudShadowDirectorDecision {
    pub schema_version: u16,
    pub action: CloudShadowRefreshAction,
    pub reason: CloudShadowRefreshReason,
    pub next_resolution: CloudShadowResolution,
    pub next_cadence_frames: u32,
    /// `Some(world_xz)` when the typed projection center
    /// must snap this frame (typed camera moved past the
    /// typed snap threshold OR typed world-stream event
    /// requires a typed re-center).  Stores the typed
    /// `[x, z]` world coordinates the typed projection
    /// should re-center to.  `None` when the typed
    /// projection center stays put.
    pub projection_center_snap: Option<[f32; 2]>,
    pub budget_pressure: bool,
    pub next_latency: CloudShadowFrameDelayMode,
}

impl Default for CloudShadowDirectorDecision {
    fn default() -> Self {
        Self::SKIP_STABLE
    }
}

impl CloudShadowDirectorDecision {
    /// Typed default decision — typed skip + typed stable.
    pub const SKIP_STABLE: Self = Self {
        schema_version: FUN_RENDERER_CLOUD_SHADOW_DIRECTOR_SCHEMA_VERSION,
        action: CloudShadowRefreshAction::Skip,
        reason: CloudShadowRefreshReason::StableSkipped,
        next_resolution: CloudShadowResolution::Balanced2048,
        next_cadence_frames: 4,
        projection_center_snap: None,
        budget_pressure: false,
        next_latency: CloudShadowFrameDelayMode::OneFrameDelayed,
    };

    /// Typed predicate: did the typed director refresh
    /// this frame (any non-skip action)?
    #[must_use]
    pub const fn refreshed_this_frame(&self) -> bool {
        !self.action.skipped()
    }

    /// Typed predicate: did the typed director downgrade
    /// the typed resolution under typed budget pressure?
    #[must_use]
    pub fn downgraded_resolution(&self, previous: CloudShadowResolution) -> bool {
        resolution_index(self.next_resolution) < resolution_index(previous)
    }

    /// Typed predicate: does the typed decision require
    /// the typed projection center to snap this frame?
    #[must_use]
    pub const fn requires_projection_center_snap(&self) -> bool {
        self.projection_center_snap.is_some()
    }
}

#[must_use]
const fn resolution_index(res: CloudShadowResolution) -> u8 {
    match res {
        CloudShadowResolution::Cheap1024 => 0,
        CloudShadowResolution::Balanced2048 => 1,
        CloudShadowResolution::Cinematic4096 => 2,
    }
}

#[must_use]
const fn downgrade_resolution(res: CloudShadowResolution) -> CloudShadowResolution {
    match res {
        CloudShadowResolution::Cinematic4096 => CloudShadowResolution::Balanced2048,
        CloudShadowResolution::Balanced2048 => CloudShadowResolution::Cheap1024,
        CloudShadowResolution::Cheap1024 => CloudShadowResolution::Cheap1024,
    }
}

// ============================================================================
// Section 4 — typed decide_cloud_shadow_refresh
// ============================================================================

/// Typed Pass C7.9 — pick the typed refresh action +
/// reason + typed next resolution / cadence for the
/// frame.
///
/// Priority order (typed first-wins):
/// 1. `Debug` quality tier → typed `DebugForce` →
///    typed every frame refresh.
/// 2. World-stream event pending → typed
///    `WorldStreamEvent` → typed forced refresh +
///    typed projection center snap.
/// 3. Weather changed → typed `WeatherChanged` →
///    typed forced refresh.
/// 4. Sun changed → typed `SunChanged` → typed forced
///    refresh.
/// 5. Camera moved past snap threshold → typed
///    `CameraSnap` → typed forced refresh + typed
///    projection center snap.
/// 6. Budget pressure (typed last frame's GPU times
///    exceeded typed budget) → typed
///    `BudgetDowngrade` → typed refresh at typed lower
///    resolution.
/// 7. Cadence reached (typed `frames_since_last_refresh`
///    at least typed `cadence`) → typed `CadenceReached`
///    → typed normal refresh.
/// 8. Otherwise → typed `StableSkipped` → typed skip.
///
/// The typed next cadence + next resolution defaults to
/// the typed quality-tier defaults; typed budget
/// pressure can lower the typed resolution if the typed
/// tier permits it.
#[must_use]
pub fn decide_cloud_shadow_refresh(
    inputs: &CloudShadowDirectorInputs,
    budget: &CloudShadowDirectorBudget,
) -> CloudShadowDirectorDecision {
    let tier = inputs.quality_tier;
    let tier_cadence = tier.default_cadence_frames();
    let tier_resolution = tier.default_resolution();
    let tier_latency = tier.default_latency();

    // Typed `Debug` tier always refreshes (priority 1).
    if tier.refreshes_every_frame() {
        return CloudShadowDirectorDecision {
            schema_version: FUN_RENDERER_CLOUD_SHADOW_DIRECTOR_SCHEMA_VERSION,
            action: CloudShadowRefreshAction::ForceRefresh,
            reason: CloudShadowRefreshReason::DebugForce,
            next_resolution: tier_resolution,
            next_cadence_frames: 1,
            projection_center_snap: None,
            budget_pressure: false,
            next_latency: tier_latency,
        };
    }

    // World-stream event (priority 2).
    if inputs.world_stream_event_pending {
        return CloudShadowDirectorDecision {
            schema_version: FUN_RENDERER_CLOUD_SHADOW_DIRECTOR_SCHEMA_VERSION,
            action: CloudShadowRefreshAction::ForceRefresh,
            reason: CloudShadowRefreshReason::WorldStreamEvent,
            next_resolution: tier_resolution,
            next_cadence_frames: tier_cadence,
            // World stream events re-center the typed
            // projection so the typed shadow stays
            // consistent with the typed newly streamed
            // chunks.  Snap world point defaults to typed
            // origin; typed renderer can override via
            // typed camera read.
            projection_center_snap: Some([0.0, 0.0]),
            budget_pressure: false,
            next_latency: tier_latency,
        };
    }

    // Weather change (priority 3).
    if inputs.weather_changed && tier.honors_change_signals() {
        return CloudShadowDirectorDecision {
            schema_version: FUN_RENDERER_CLOUD_SHADOW_DIRECTOR_SCHEMA_VERSION,
            action: CloudShadowRefreshAction::ForceRefresh,
            reason: CloudShadowRefreshReason::WeatherChanged,
            next_resolution: tier_resolution,
            next_cadence_frames: tier_cadence,
            projection_center_snap: None,
            budget_pressure: false,
            next_latency: tier_latency,
        };
    }

    // Sun change (priority 4).
    if inputs.sun_changed && tier.honors_change_signals() {
        return CloudShadowDirectorDecision {
            schema_version: FUN_RENDERER_CLOUD_SHADOW_DIRECTOR_SCHEMA_VERSION,
            action: CloudShadowRefreshAction::ForceRefresh,
            reason: CloudShadowRefreshReason::SunChanged,
            next_resolution: tier_resolution,
            next_cadence_frames: tier_cadence,
            projection_center_snap: None,
            budget_pressure: false,
            next_latency: tier_latency,
        };
    }

    // Camera snap (priority 5).
    if inputs.camera_movement_meters >= budget.camera_snap_threshold_meters {
        return CloudShadowDirectorDecision {
            schema_version: FUN_RENDERER_CLOUD_SHADOW_DIRECTOR_SCHEMA_VERSION,
            action: CloudShadowRefreshAction::ForceRefresh,
            reason: CloudShadowRefreshReason::CameraSnap,
            next_resolution: tier_resolution,
            next_cadence_frames: tier_cadence,
            // Typed projection center snaps to the typed
            // current camera footprint — the typed
            // renderer fills in the actual world XZ via
            // typed override before recording the typed
            // projection constants.  Placeholder typed
            // `[movement, movement]` keeps the typed
            // record auditable.
            projection_center_snap: Some([
                inputs.camera_movement_meters,
                inputs.camera_movement_meters,
            ]),
            budget_pressure: false,
            next_latency: tier_latency,
        };
    }

    // Typed budget pressure (priority 6).  Active only
    // when typed budgets are set AND typed last frame's
    // GPU times exceeded them AND typed the tier permits
    // downgrade.
    let project_over = budget.project_budget_ns > 0
        && inputs.last_project_gpu_ns > budget.project_budget_ns;
    let filter_over =
        budget.filter_budget_ns > 0 && inputs.last_filter_gpu_ns > budget.filter_budget_ns;
    let budget_pressure = project_over || filter_over;
    if budget_pressure && tier.permits_resolution_downgrade() {
        return CloudShadowDirectorDecision {
            schema_version: FUN_RENDERER_CLOUD_SHADOW_DIRECTOR_SCHEMA_VERSION,
            action: CloudShadowRefreshAction::RefreshProjectAndFilter,
            reason: CloudShadowRefreshReason::BudgetDowngrade,
            next_resolution: downgrade_resolution(inputs.current_resolution),
            // Typed cadence widens under typed pressure
            // — typed twice the tier default keeps the
            // typed GPU cost down further.
            next_cadence_frames: tier_cadence.saturating_mul(2),
            projection_center_snap: None,
            budget_pressure: true,
            next_latency: tier_latency,
        };
    }

    // Typed cadence reached (priority 7).
    if inputs.frames_since_last_refresh >= tier_cadence {
        return CloudShadowDirectorDecision {
            schema_version: FUN_RENDERER_CLOUD_SHADOW_DIRECTOR_SCHEMA_VERSION,
            action: CloudShadowRefreshAction::RefreshProjectAndFilter,
            reason: CloudShadowRefreshReason::CadenceReached,
            next_resolution: tier_resolution,
            next_cadence_frames: tier_cadence,
            projection_center_snap: None,
            budget_pressure: false,
            next_latency: tier_latency,
        };
    }

    // Typed stable — typed skip the refresh (priority 8).
    CloudShadowDirectorDecision {
        schema_version: FUN_RENDERER_CLOUD_SHADOW_DIRECTOR_SCHEMA_VERSION,
        action: CloudShadowRefreshAction::Skip,
        reason: CloudShadowRefreshReason::StableSkipped,
        next_resolution: inputs.current_resolution,
        next_cadence_frames: tier_cadence,
        projection_center_snap: None,
        budget_pressure: false,
        next_latency: tier_latency,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pass C7.9 schema is stable.
    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_RENDERER_CLOUD_SHADOW_DIRECTOR_SCHEMA_VERSION, 1);
    }

    /// Pass C7.9 — typed tier taxonomy walks user spec.
    #[test]
    fn quality_tier_taxonomy_walks_user_spec() {
        assert_eq!(CloudShadowQualityTier::ALL.len(), 4);
        // Typed defaults match the user-spec policy.
        assert_eq!(CloudShadowQualityTier::Cheap.default_cadence_frames(), 6);
        assert_eq!(CloudShadowQualityTier::Balanced.default_cadence_frames(), 4);
        assert_eq!(CloudShadowQualityTier::Cinematic.default_cadence_frames(), 1);
        assert_eq!(CloudShadowQualityTier::Debug.default_cadence_frames(), 1);
        // Typed resolutions match the user-spec policy.
        assert_eq!(
            CloudShadowQualityTier::Cheap.default_resolution(),
            CloudShadowResolution::Cheap1024,
        );
        assert_eq!(
            CloudShadowQualityTier::Balanced.default_resolution(),
            CloudShadowResolution::Balanced2048,
        );
        assert_eq!(
            CloudShadowQualityTier::Cinematic.default_resolution(),
            CloudShadowResolution::Cinematic4096,
        );
        // Typed latency: typed Cheap/Balanced → typed
        // one-frame-delayed; typed Cinematic/Debug → typed
        // same-frame.
        assert_eq!(
            CloudShadowQualityTier::Cheap.default_latency(),
            CloudShadowFrameDelayMode::OneFrameDelayed,
        );
        assert_eq!(
            CloudShadowQualityTier::Cinematic.default_latency(),
            CloudShadowFrameDelayMode::SameFrame,
        );
        // Typed Debug always refreshes.
        assert!(CloudShadowQualityTier::Debug.refreshes_every_frame());
        assert!(!CloudShadowQualityTier::Balanced.refreshes_every_frame());
        // Typed every tier honors change signals.
        for tier in CloudShadowQualityTier::ALL {
            assert!(tier.honors_change_signals(), "{:?}", tier);
        }
        // Typed only Balanced + Cinematic permit
        // resolution downgrade.
        assert!(!CloudShadowQualityTier::Cheap.permits_resolution_downgrade());
        assert!(CloudShadowQualityTier::Balanced.permits_resolution_downgrade());
        assert!(CloudShadowQualityTier::Cinematic.permits_resolution_downgrade());
        assert!(!CloudShadowQualityTier::Debug.permits_resolution_downgrade());
    }

    /// Pass C7.9 — typed action / reason taxonomies are
    /// dense.
    #[test]
    fn action_and_reason_taxonomies_are_dense() {
        assert_eq!(CloudShadowRefreshAction::ALL.len(), 4);
        assert_eq!(CloudShadowRefreshReason::ALL.len(), 8);
        // Typed runs-project / runs-filter walk every
        // action.
        assert!(!CloudShadowRefreshAction::Skip.runs_project());
        assert!(!CloudShadowRefreshAction::Skip.runs_filter());
        assert!(CloudShadowRefreshAction::Skip.skipped());
        assert!(CloudShadowRefreshAction::RefreshProjectAndFilter.runs_project());
        assert!(CloudShadowRefreshAction::RefreshProjectAndFilter.runs_filter());
        assert!(!CloudShadowRefreshAction::RefreshFilterOnly.runs_project());
        assert!(CloudShadowRefreshAction::RefreshFilterOnly.runs_filter());
        assert!(CloudShadowRefreshAction::ForceRefresh.runs_project());
        assert!(CloudShadowRefreshAction::ForceRefresh.runs_filter());
        // Typed reason `is_forced`.
        assert!(CloudShadowRefreshReason::WeatherChanged.is_forced());
        assert!(CloudShadowRefreshReason::SunChanged.is_forced());
        assert!(CloudShadowRefreshReason::CameraSnap.is_forced());
        assert!(CloudShadowRefreshReason::WorldStreamEvent.is_forced());
        assert!(CloudShadowRefreshReason::DebugForce.is_forced());
        assert!(!CloudShadowRefreshReason::CadenceReached.is_forced());
        assert!(!CloudShadowRefreshReason::StableSkipped.is_forced());
        assert!(!CloudShadowRefreshReason::BudgetDowngrade.is_forced());
    }

    /// Pass C7.9 acceptance — stable camera/weather does
    /// not reproject every frame unless required.
    #[test]
    fn stable_camera_weather_does_not_reproject_every_frame() {
        let mut inputs = CloudShadowDirectorInputs::STABLE;
        inputs.quality_tier = CloudShadowQualityTier::Balanced;
        // Typed frames 0 .. cadence-1 → typed skip.
        for frames_idle in 0..3 {
            inputs.frames_since_last_refresh = frames_idle;
            let decision = decide_cloud_shadow_refresh(
                &inputs,
                &CloudShadowDirectorBudget::PRODUCT_DEFAULT,
            );
            assert_eq!(decision.action, CloudShadowRefreshAction::Skip);
            assert_eq!(decision.reason, CloudShadowRefreshReason::StableSkipped);
            assert!(!decision.refreshed_this_frame());
        }
        // Typed cadence reached at typed frame 4 → typed
        // refresh.
        inputs.frames_since_last_refresh = 4;
        let decision = decide_cloud_shadow_refresh(
            &inputs,
            &CloudShadowDirectorBudget::PRODUCT_DEFAULT,
        );
        assert_eq!(decision.action, CloudShadowRefreshAction::RefreshProjectAndFilter);
        assert_eq!(decision.reason, CloudShadowRefreshReason::CadenceReached);
    }

    /// Pass C7.9 acceptance — sun jump forces a refresh.
    #[test]
    fn sun_jump_forces_refresh() {
        let mut inputs = CloudShadowDirectorInputs::STABLE;
        inputs.quality_tier = CloudShadowQualityTier::Balanced;
        inputs.frames_since_last_refresh = 0; // typed below cadence
        inputs.sun_changed = true;
        let decision = decide_cloud_shadow_refresh(
            &inputs,
            &CloudShadowDirectorBudget::PRODUCT_DEFAULT,
        );
        assert_eq!(decision.action, CloudShadowRefreshAction::ForceRefresh);
        assert_eq!(decision.reason, CloudShadowRefreshReason::SunChanged);
        assert!(decision.refreshed_this_frame());
    }

    /// Pass C7.9 acceptance — weather jump forces a
    /// refresh.
    #[test]
    fn weather_jump_forces_refresh() {
        let mut inputs = CloudShadowDirectorInputs::STABLE;
        inputs.quality_tier = CloudShadowQualityTier::Balanced;
        inputs.frames_since_last_refresh = 0;
        inputs.weather_changed = true;
        let decision = decide_cloud_shadow_refresh(
            &inputs,
            &CloudShadowDirectorBudget::PRODUCT_DEFAULT,
        );
        assert_eq!(decision.action, CloudShadowRefreshAction::ForceRefresh);
        assert_eq!(decision.reason, CloudShadowRefreshReason::WeatherChanged);
    }

    /// Pass C7.9 acceptance — projection center snaps to
    /// prevent crawling.
    #[test]
    fn projection_center_snaps_to_prevent_crawling() {
        let mut inputs = CloudShadowDirectorInputs::STABLE;
        inputs.quality_tier = CloudShadowQualityTier::Balanced;
        inputs.frames_since_last_refresh = 0;
        // Typed camera movement = 100 m (above typed
        // PRODUCT_DEFAULT snap threshold of 64 m).
        inputs.camera_movement_meters = 100.0;
        let decision = decide_cloud_shadow_refresh(
            &inputs,
            &CloudShadowDirectorBudget::PRODUCT_DEFAULT,
        );
        assert_eq!(decision.reason, CloudShadowRefreshReason::CameraSnap);
        assert!(decision.requires_projection_center_snap());
        assert!(decision.projection_center_snap.is_some());
        // Typed movement just below threshold → typed no
        // snap, typed cadence skip.
        inputs.camera_movement_meters = 50.0;
        let decision = decide_cloud_shadow_refresh(
            &inputs,
            &CloudShadowDirectorBudget::PRODUCT_DEFAULT,
        );
        assert_ne!(decision.reason, CloudShadowRefreshReason::CameraSnap);
        assert!(!decision.requires_projection_center_snap());
    }

    /// Pass C7.9 acceptance — quality director can lower
    /// resolution or cadence under budget pressure.
    #[test]
    fn quality_director_lowers_resolution_under_budget_pressure() {
        let mut inputs = CloudShadowDirectorInputs::STABLE;
        inputs.quality_tier = CloudShadowQualityTier::Balanced;
        inputs.current_resolution = CloudShadowResolution::Balanced2048;
        inputs.frames_since_last_refresh = 0;
        // Typed last frame's project GPU ns exceeds the
        // typed budget.
        inputs.last_project_gpu_ns = 1_000_000; // > 300_000 budget
        let decision = decide_cloud_shadow_refresh(
            &inputs,
            &CloudShadowDirectorBudget::PRODUCT_DEFAULT,
        );
        assert!(decision.budget_pressure);
        assert_eq!(decision.reason, CloudShadowRefreshReason::BudgetDowngrade);
        // Typed next resolution typed lower than typed
        // current.
        assert!(decision.downgraded_resolution(CloudShadowResolution::Balanced2048));
        assert_eq!(decision.next_resolution, CloudShadowResolution::Cheap1024);
        // Typed cadence widens (2 * default = 8).
        assert_eq!(decision.next_cadence_frames, 8);
    }

    /// Pass C7.9 — typed Cheap tier does NOT permit
    /// resolution downgrade; under budget pressure the
    /// typed decision instead reaches the typed cadence
    /// refresh path with typed unchanged resolution.
    #[test]
    fn cheap_tier_does_not_downgrade_under_budget_pressure() {
        let mut inputs = CloudShadowDirectorInputs::STABLE;
        inputs.quality_tier = CloudShadowQualityTier::Cheap;
        inputs.current_resolution = CloudShadowResolution::Cheap1024;
        inputs.frames_since_last_refresh = 6; // typed cadence reached
        inputs.last_project_gpu_ns = 1_000_000;
        let decision = decide_cloud_shadow_refresh(
            &inputs,
            &CloudShadowDirectorBudget::PRODUCT_DEFAULT,
        );
        assert!(!decision.budget_pressure);
        assert_eq!(decision.reason, CloudShadowRefreshReason::CadenceReached);
        assert_eq!(decision.next_resolution, CloudShadowResolution::Cheap1024);
    }

    /// Pass C7.9 — typed `Debug` tier refreshes every
    /// frame regardless of typed signals.
    #[test]
    fn debug_tier_refreshes_every_frame() {
        let mut inputs = CloudShadowDirectorInputs::STABLE;
        inputs.quality_tier = CloudShadowQualityTier::Debug;
        // Even with typed stable inputs + typed cadence
        // not reached, typed Debug forces a refresh.
        inputs.frames_since_last_refresh = 0;
        let decision = decide_cloud_shadow_refresh(
            &inputs,
            &CloudShadowDirectorBudget::PRODUCT_DEFAULT,
        );
        assert_eq!(decision.action, CloudShadowRefreshAction::ForceRefresh);
        assert_eq!(decision.reason, CloudShadowRefreshReason::DebugForce);
        assert_eq!(decision.next_cadence_frames, 1);
    }

    /// Pass C7.9 — typed world-stream event forces typed
    /// refresh AND typed projection center snap.
    #[test]
    fn world_stream_event_forces_refresh_and_snap() {
        let mut inputs = CloudShadowDirectorInputs::STABLE;
        inputs.quality_tier = CloudShadowQualityTier::Balanced;
        inputs.world_stream_event_pending = true;
        let decision = decide_cloud_shadow_refresh(
            &inputs,
            &CloudShadowDirectorBudget::PRODUCT_DEFAULT,
        );
        assert_eq!(decision.action, CloudShadowRefreshAction::ForceRefresh);
        assert_eq!(decision.reason, CloudShadowRefreshReason::WorldStreamEvent);
        assert!(decision.requires_projection_center_snap());
    }

    /// Pass C7.9 — typed priority order: typed Debug
    /// beats every other signal.
    #[test]
    fn debug_priority_beats_every_signal() {
        let mut inputs = CloudShadowDirectorInputs::STABLE;
        inputs.quality_tier = CloudShadowQualityTier::Debug;
        inputs.weather_changed = true;
        inputs.sun_changed = true;
        inputs.world_stream_event_pending = true;
        inputs.camera_movement_meters = 1000.0;
        let decision = decide_cloud_shadow_refresh(
            &inputs,
            &CloudShadowDirectorBudget::PRODUCT_DEFAULT,
        );
        assert_eq!(decision.reason, CloudShadowRefreshReason::DebugForce);
    }

    /// Pass C7.9 — typed world-stream > typed weather >
    /// typed sun > typed camera priority chain.
    #[test]
    fn priority_chain_world_stream_weather_sun_camera() {
        let base = CloudShadowDirectorInputs {
            quality_tier: CloudShadowQualityTier::Balanced,
            ..CloudShadowDirectorInputs::STABLE
        };
        // Typed world-stream wins over typed weather +
        // typed sun + typed camera.
        let mut inputs = base;
        inputs.world_stream_event_pending = true;
        inputs.weather_changed = true;
        inputs.sun_changed = true;
        inputs.camera_movement_meters = 100.0;
        let d = decide_cloud_shadow_refresh(&inputs, &CloudShadowDirectorBudget::PRODUCT_DEFAULT);
        assert_eq!(d.reason, CloudShadowRefreshReason::WorldStreamEvent);
        // Typed weather wins over typed sun + typed
        // camera.
        let mut inputs = base;
        inputs.weather_changed = true;
        inputs.sun_changed = true;
        inputs.camera_movement_meters = 100.0;
        let d = decide_cloud_shadow_refresh(&inputs, &CloudShadowDirectorBudget::PRODUCT_DEFAULT);
        assert_eq!(d.reason, CloudShadowRefreshReason::WeatherChanged);
        // Typed sun wins over typed camera.
        let mut inputs = base;
        inputs.sun_changed = true;
        inputs.camera_movement_meters = 100.0;
        let d = decide_cloud_shadow_refresh(&inputs, &CloudShadowDirectorBudget::PRODUCT_DEFAULT);
        assert_eq!(d.reason, CloudShadowRefreshReason::SunChanged);
        // Typed camera fires alone.
        let mut inputs = base;
        inputs.camera_movement_meters = 100.0;
        let d = decide_cloud_shadow_refresh(&inputs, &CloudShadowDirectorBudget::PRODUCT_DEFAULT);
        assert_eq!(d.reason, CloudShadowRefreshReason::CameraSnap);
    }

    /// Pass C7.9 — typed inactive budgets do not trigger
    /// typed budget pressure even when typed GPU ns are
    /// high.
    #[test]
    fn inactive_budgets_do_not_trigger_pressure() {
        let mut inputs = CloudShadowDirectorInputs::STABLE;
        inputs.quality_tier = CloudShadowQualityTier::Balanced;
        inputs.frames_since_last_refresh = 0;
        inputs.last_project_gpu_ns = 10_000_000; // huge
        let inactive_budget = CloudShadowDirectorBudget {
            schema_version: FUN_RENDERER_CLOUD_SHADOW_DIRECTOR_SCHEMA_VERSION,
            project_budget_ns: 0,
            filter_budget_ns: 0,
            max_resolution: CloudShadowResolution::Balanced2048,
            camera_snap_threshold_meters: 64.0,
        };
        assert!(!inactive_budget.budgets_active());
        let decision = decide_cloud_shadow_refresh(&inputs, &inactive_budget);
        assert!(!decision.budget_pressure);
        assert_eq!(decision.reason, CloudShadowRefreshReason::StableSkipped);
    }
}
