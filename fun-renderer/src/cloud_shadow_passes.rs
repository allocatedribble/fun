//! Pass C7.4.5 — typed cloud shadow pass-recording functions.
//!
//! Pass C7.2 named the typed cloud-shadow frame-graph roles
//! (`LuxCloudShadowProject` / `LuxCloudShadowFilter` /
//! `LuxCloudShadowRegisterLayer`) + their typed ordering;
//! Pass C7.3 / C7.4.1 / C7.4.2 added the typed shadow math
//! contract + the typed projection constants + the typed
//! resource diagnostics; Pass C7.4.3 / C7.4.4 added the
//! typed compute shaders.  This module lands the typed
//! pass-recording bridge between the typed Rust-side
//! settings + projection constants and the typed
//! frame-graph diagnostics.
//!
//! Ownership: this module lives in `fun-renderer`, NOT
//! `fun_render`.  The typed cloud renderer is owned by
//! `fun-renderer` per the typed C0 / C1 ownership contract;
//! the typed `fun_render::sky` module stays as a typed
//! extraction-only bridge.  This module is the typed
//! renderer-owned recorder.
//!
//! Recording functions:
//!
//!   record_lux_cloud_shadow_project(...)        — Pass C7.4.3
//!   record_lux_cloud_shadow_filter(...)         — Pass C7.4.4
//!   record_lux_cloud_shadow_register_layer(...) — Pass C7.4.5
//!
//! Each returns a typed [`CloudShadowPassRecord`] that
//! carries the typed pass role + typed order key + typed
//! reads + typed writes + typed frame-delay mode + typed
//! registers-pass flag.  The typed renderer pipeline
//! consumes this slice to register typed
//! `RendererFrameGraph` passes; the typed cloud diagnostics
//! consume the same slice to emit the typed "Cloud Shadow
//! Pipeline" debug section.

use crate::cloud_shadow::{CloudShadowFrameDelayMode, CloudShadowProjectionConstants};
use crate::clouds::CloudRenderSettings;
use crate::frame_graph::{FrameGraphPassRole, FrameGraphResourceType};

pub const FUN_RENDERER_CLOUD_SHADOW_PASSES_SCHEMA_VERSION: u16 = 1;

// ============================================================================
// Section 1 — typed read / write resource tables
// ============================================================================

/// Typed `LuxCloudShadowProject` read table (per user spec).
///
/// Reads:
/// - `CloudWeatherMap` — typed cloud coverage map sampled
///   along the typed sun ray.
/// - `CloudShapeNoise` — typed 3D noise sampled to derive
///   typed shape density at each typed march step.
/// - `CloudShadowProjectionConstants` — typed projection
///   uniform that drives the typed shadow UV ↔ world
///   transform.
pub const CLOUD_SHADOW_PROJECT_READS: &[FrameGraphResourceType] = &[
    FrameGraphResourceType::CloudWeatherMap,
    FrameGraphResourceType::CloudShapeNoise,
    FrameGraphResourceType::CloudShadowProjectionConstants,
];

/// Typed `LuxCloudShadowProject` write table (per user spec).
///
/// Writes:
/// - `CloudWorldShadowTransmittance` — typed projected
///   transmittance target the typed
///   `cloud_shadow_project.wgsl` compute shader writes.
pub const CLOUD_SHADOW_PROJECT_WRITES: &[FrameGraphResourceType] =
    &[FrameGraphResourceType::CloudWorldShadowTransmittance];

/// Typed `LuxCloudShadowFilter` read table (per user spec).
///
/// Reads:
/// - `CloudWorldShadowTransmittance` — typed projected
///   transmittance produced by the typed project pass.
/// - `CloudShadowProjectionConstants` — typed softness +
///   opacity knobs that drive the typed filter kernel.
pub const CLOUD_SHADOW_FILTER_READS: &[FrameGraphResourceType] = &[
    FrameGraphResourceType::CloudWorldShadowTransmittance,
    FrameGraphResourceType::CloudShadowProjectionConstants,
];

/// Typed `LuxCloudShadowFilter` write table (per user spec).
///
/// Writes:
/// - `CloudWorldShadowFiltered` — typed filtered
///   transmittance target the typed
///   `cloud_shadow_filter.wgsl` compute shader writes.
pub const CLOUD_SHADOW_FILTER_WRITES: &[FrameGraphResourceType] =
    &[FrameGraphResourceType::CloudWorldShadowFiltered];

/// Typed `LuxCloudShadowRegisterLayer` read table (per user
/// spec).
///
/// Reads:
/// - `CloudWorldShadowFiltered` — typed filtered shadow the
///   typed register-layer pass exposes to typed Lux.
/// - `CloudShadowProjectionConstants` — typed projection
///   metadata (light id, sun direction, slab altitudes)
///   the typed register pass forwards to typed
///   `LuxDirectLighting`.
pub const CLOUD_SHADOW_REGISTER_LAYER_READS: &[FrameGraphResourceType] = &[
    FrameGraphResourceType::CloudWorldShadowFiltered,
    FrameGraphResourceType::CloudShadowProjectionConstants,
];

/// Typed `LuxCloudShadowRegisterLayer` write table (per user
/// spec).
///
/// Writes:
/// - `CloudShadowAuxLayer` — typed cloud shadow aux-layer
///   metadata the typed `LuxDirectLighting` pass will read
///   in a typed later sub-pass to sample the typed filtered
///   cloud shadow alongside the typed Lux virtual shadow
///   pages.  The typed contract refuses writing to typed
///   `LuxVirtualShadowPages` or typed `LuxShadowAtlas` —
///   cloud shadows are a typed separate aux layer, not a
///   typed Lux opaque shadow target.
pub const CLOUD_SHADOW_REGISTER_LAYER_WRITES: &[FrameGraphResourceType] =
    &[FrameGraphResourceType::CloudShadowAuxLayer];

// ============================================================================
// Section 2 — typed const order keys (sourced from `FrameGraphPassRole`)
// ============================================================================

/// Typed `LuxCloudShadowProject` order key (140).
pub const CLOUD_SHADOW_PROJECT_ORDER_KEY: u16 =
    match FrameGraphPassRole::LuxCloudShadowProject.lux_order_key() {
        Some(k) => k,
        None => 0,
    };

/// Typed `LuxCloudShadowFilter` order key (145).
pub const CLOUD_SHADOW_FILTER_ORDER_KEY: u16 =
    match FrameGraphPassRole::LuxCloudShadowFilter.lux_order_key() {
        Some(k) => k,
        None => 0,
    };

/// Typed `LuxCloudShadowRegisterLayer` order key (150).
pub const CLOUD_SHADOW_REGISTER_LAYER_ORDER_KEY: u16 =
    match FrameGraphPassRole::LuxCloudShadowRegisterLayer.lux_order_key() {
        Some(k) => k,
        None => 0,
    };

/// Typed `LuxDirectLighting` order key (200) — kept here so
/// the typed `same-frame mode runs register-layer before
/// direct lighting` invariant is auditable at the typed
/// const layer.
pub const LUX_DIRECT_LIGHTING_ORDER_KEY: u16 =
    match FrameGraphPassRole::LuxDirectLighting.lux_order_key() {
        Some(k) => k,
        None => 0,
    };

// ============================================================================
// Section 3 — typed CloudShadowPassRecord
// ============================================================================

/// Typed Pass C7.4.5 cloud shadow pass record.  Bundles
/// the typed pass role + typed order key + typed reads /
/// writes + typed frame-delay mode + typed registration
/// gates into a typed record the typed renderer pipeline
/// consumes when wiring the typed frame graph.
///
/// The typed record is intentionally typed-and-static —
/// every field is either a typed enum, a typed const slice,
/// or a typed primitive.  The typed renderer can hold a
/// typed slice of these records without owning any typed
/// allocations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CloudShadowPassRecord {
    pub schema_version: u16,
    pub role: FrameGraphPassRole,
    pub order_key: u16,
    pub reads: &'static [FrameGraphResourceType],
    pub writes: &'static [FrameGraphResourceType],
    pub frame_delay_mode: CloudShadowFrameDelayMode,
    /// Typed predicate: should the typed renderer register
    /// this typed pass this frame?  Composes the typed
    /// `CloudRenderSettings::registers_world_shadow_pass` +
    /// the typed `CloudShadowProjectionConstants::projects_world_shadow`
    /// gates.
    pub registers_pass: bool,
    /// Typed predicate: do the typed projection constants
    /// describe a typed fully-live projection?  Carried
    /// through so the typed diagnostics can distinguish
    /// "pass gated off via settings" from "pass gated off
    /// via invalid projection".
    pub projection_is_live: bool,
}

impl CloudShadowPassRecord {
    /// Typed predicate: did the typed pass actually run
    /// this frame (registered + projection live)?
    #[must_use]
    pub const fn ran_this_frame(&self) -> bool {
        self.registers_pass && self.projection_is_live
    }

    /// Typed predicate: does the typed record declare any
    /// typed resource interaction (read OR write)?  Mirrors
    /// the typed `lux_passes::declares_any_resource_interaction`
    /// invariant for the typed cloud-shadow-owned recording
    /// path.
    #[must_use]
    pub const fn declares_any_resource_interaction(&self) -> bool {
        !self.reads.is_empty() || !self.writes.is_empty()
    }
}

// ============================================================================
// Section 4 — typed recording functions
// ============================================================================

/// Typed Pass C7.4.5 — record the typed
/// `LuxCloudShadowProject` pass.
///
/// Reads `CloudWeatherMap`, `CloudShapeNoise`, and
/// `CloudShadowProjectionConstants`; writes
/// `CloudWorldShadowTransmittance`.  Order key 140 (between
/// the typed virtual shadow filter at 135 and the typed
/// direct lighting at 200).
///
/// The typed `registers_pass` flag is `true` when the typed
/// settings opt into the typed world shadow pass AND the
/// typed projection constants describe a typed fully-live
/// projection.  Otherwise the typed record reports a typed
/// gated-off pass that the typed renderer can skip.
#[must_use]
pub fn record_lux_cloud_shadow_project(
    settings: &CloudRenderSettings,
    constants: &CloudShadowProjectionConstants,
    frame_delay_mode: CloudShadowFrameDelayMode,
) -> CloudShadowPassRecord {
    let projection_is_live = constants.projects_world_shadow();
    let registers_pass = settings.registers_world_shadow_pass() && projection_is_live;
    CloudShadowPassRecord {
        schema_version: FUN_RENDERER_CLOUD_SHADOW_PASSES_SCHEMA_VERSION,
        role: FrameGraphPassRole::LuxCloudShadowProject,
        order_key: CLOUD_SHADOW_PROJECT_ORDER_KEY,
        reads: CLOUD_SHADOW_PROJECT_READS,
        writes: CLOUD_SHADOW_PROJECT_WRITES,
        frame_delay_mode,
        registers_pass,
        projection_is_live,
    }
}

/// Typed Pass C7.4.5 — record the typed
/// `LuxCloudShadowFilter` pass.
///
/// Reads `CloudWorldShadowTransmittance` and
/// `CloudShadowProjectionConstants`; writes
/// `CloudWorldShadowFiltered`.  Order key 145 (immediately
/// after the typed project pass, before the typed register
/// pass).
#[must_use]
pub fn record_lux_cloud_shadow_filter(
    settings: &CloudRenderSettings,
    constants: &CloudShadowProjectionConstants,
    frame_delay_mode: CloudShadowFrameDelayMode,
) -> CloudShadowPassRecord {
    let projection_is_live = constants.projects_world_shadow();
    let registers_pass = settings.registers_world_shadow_pass() && projection_is_live;
    CloudShadowPassRecord {
        schema_version: FUN_RENDERER_CLOUD_SHADOW_PASSES_SCHEMA_VERSION,
        role: FrameGraphPassRole::LuxCloudShadowFilter,
        order_key: CLOUD_SHADOW_FILTER_ORDER_KEY,
        reads: CLOUD_SHADOW_FILTER_READS,
        writes: CLOUD_SHADOW_FILTER_WRITES,
        frame_delay_mode,
        registers_pass,
        projection_is_live,
    }
}

/// Typed Pass C7.4.5 — record the typed
/// `LuxCloudShadowRegisterLayer` pass.
///
/// Reads `CloudWorldShadowFiltered` and
/// `CloudShadowProjectionConstants`; writes the typed
/// `CloudShadowAuxLayer` metadata resource.  Order key 150
/// (immediately after the typed filter pass, before the
/// typed direct lighting at 200).
///
/// Same-frame mode strictly requires the typed register
/// pass to run before the typed direct lighting (so the
/// typed direct-lighting pass can sample the typed
/// CURRENT-frame filtered shadow).  One-frame-delayed mode
/// can run the typed register pass anywhere in the typed
/// current frame; the typed direct-lighting pass reads the
/// typed PREVIOUS frame's aux-layer metadata.
#[must_use]
pub fn record_lux_cloud_shadow_register_layer(
    settings: &CloudRenderSettings,
    constants: &CloudShadowProjectionConstants,
    frame_delay_mode: CloudShadowFrameDelayMode,
) -> CloudShadowPassRecord {
    let projection_is_live = constants.projects_world_shadow();
    let registers_pass = settings.registers_world_shadow_pass() && projection_is_live;
    CloudShadowPassRecord {
        schema_version: FUN_RENDERER_CLOUD_SHADOW_PASSES_SCHEMA_VERSION,
        role: FrameGraphPassRole::LuxCloudShadowRegisterLayer,
        order_key: CLOUD_SHADOW_REGISTER_LAYER_ORDER_KEY,
        reads: CLOUD_SHADOW_REGISTER_LAYER_READS,
        writes: CLOUD_SHADOW_REGISTER_LAYER_WRITES,
        frame_delay_mode,
        registers_pass,
        projection_is_live,
    }
}

/// Typed Pass C7.4.5 — record the typed full cloud-shadow
/// pass chain (`Project` → `Filter` → `RegisterLayer`) as
/// a typed `[CloudShadowPassRecord; 3]`.  The typed
/// renderer pipeline iterates this typed array to register
/// passes; the typed diagnostics iterate it to emit the
/// typed "Cloud Shadow Pipeline" debug section.
///
/// Typed ordering invariant:
///   `Project.order_key < Filter.order_key < RegisterLayer.order_key`.
/// Audited by the typed
/// `cloud_shadow_pass_chain_order_is_monotonic` test.
#[must_use]
pub fn record_cloud_shadow_pass_chain(
    settings: &CloudRenderSettings,
    constants: &CloudShadowProjectionConstants,
    frame_delay_mode: CloudShadowFrameDelayMode,
) -> [CloudShadowPassRecord; 3] {
    [
        record_lux_cloud_shadow_project(settings, constants, frame_delay_mode),
        record_lux_cloud_shadow_filter(settings, constants, frame_delay_mode),
        record_lux_cloud_shadow_register_layer(settings, constants, frame_delay_mode),
    ]
}

// ============================================================================
// Section 5 — typed graph-diagnostic surface
// ============================================================================

/// Typed Pass C7.4.5 cloud shadow diagnostics.  Bundles the
/// typed three pass records + typed predicates the typed
/// `cloud_diagnostics` debug section consumes.  Drives the
/// typed "Project, filter, register passes appear in graph
/// diagnostics" acceptance bullet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CloudShadowGraphDiagnostics {
    pub schema_version: u16,
    pub project: CloudShadowPassRecord,
    pub filter: CloudShadowPassRecord,
    pub register_layer: CloudShadowPassRecord,
    pub frame_delay_mode: CloudShadowFrameDelayMode,
}

impl CloudShadowGraphDiagnostics {
    /// Typed builder: derive a typed graph diagnostics
    /// record from typed settings + typed projection
    /// constants + typed frame-delay mode.
    #[must_use]
    pub fn from_inputs(
        settings: &CloudRenderSettings,
        constants: &CloudShadowProjectionConstants,
        frame_delay_mode: CloudShadowFrameDelayMode,
    ) -> Self {
        let chain = record_cloud_shadow_pass_chain(settings, constants, frame_delay_mode);
        Self {
            schema_version: FUN_RENDERER_CLOUD_SHADOW_PASSES_SCHEMA_VERSION,
            project: chain[0],
            filter: chain[1],
            register_layer: chain[2],
            frame_delay_mode,
        }
    }

    /// Typed slice of the typed pass records.  Used by the
    /// typed cloud diagnostics + the typed renderer
    /// pipeline iterator.
    #[must_use]
    pub fn as_slice(&self) -> [CloudShadowPassRecord; 3] {
        [self.project, self.filter, self.register_layer]
    }

    /// Typed predicate: did every typed cloud shadow pass
    /// actually run this frame?
    #[must_use]
    pub const fn full_chain_ran_this_frame(&self) -> bool {
        self.project.ran_this_frame()
            && self.filter.ran_this_frame()
            && self.register_layer.ran_this_frame()
    }

    /// Typed predicate: are every typed pass record's
    /// reads / writes non-empty?  Mirrors the typed
    /// `lux_passes::declares_any_resource_interaction`
    /// invariant.
    #[must_use]
    pub const fn every_pass_declares_resource_interaction(&self) -> bool {
        self.project.declares_any_resource_interaction()
            && self.filter.declares_any_resource_interaction()
            && self.register_layer.declares_any_resource_interaction()
    }
}

// ============================================================================
// Section 6 — typed const ordering invariants
// ============================================================================

/// Typed Pass C7.4.5 const predicate — typed
/// `LuxCloudShadowProject` order key is BEFORE the typed
/// `LuxCloudShadowFilter` order key.
#[must_use]
pub const fn cloud_shadow_project_order_key_is_before_filter() -> bool {
    CLOUD_SHADOW_PROJECT_ORDER_KEY < CLOUD_SHADOW_FILTER_ORDER_KEY
}

/// Typed Pass C7.4.5 const predicate — typed
/// `LuxCloudShadowFilter` order key is BEFORE the typed
/// `LuxCloudShadowRegisterLayer` order key.
#[must_use]
pub const fn cloud_shadow_filter_order_key_is_before_register_layer() -> bool {
    CLOUD_SHADOW_FILTER_ORDER_KEY < CLOUD_SHADOW_REGISTER_LAYER_ORDER_KEY
}

/// Typed Pass C7.4.5 const predicate — typed
/// `LuxCloudShadowRegisterLayer` order key is BEFORE the
/// typed `LuxDirectLighting` order key.  Required by the
/// typed same-frame mode (the typed direct-lighting pass
/// must see the typed registered cloud shadow aux layer);
/// one-frame-delayed mode also benefits since the typed
/// register pass at least produces typed metadata for the
/// typed NEXT frame's direct-lighting pass.
#[must_use]
pub const fn cloud_shadow_register_layer_order_key_is_before_direct_lighting() -> bool {
    CLOUD_SHADOW_REGISTER_LAYER_ORDER_KEY < LUX_DIRECT_LIGHTING_ORDER_KEY
}

/// Typed Pass C7.4.5 const predicate — every typed cloud
/// shadow ordering invariant holds at the typed const
/// layer.
#[must_use]
pub const fn cloud_shadow_pass_recording_invariants_hold() -> bool {
    cloud_shadow_project_order_key_is_before_filter()
        && cloud_shadow_filter_order_key_is_before_register_layer()
        && cloud_shadow_register_layer_order_key_is_before_direct_lighting()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud_shadow::CloudShadowProjectionConstants;
    use crate::clouds::{CloudQuality, CloudRenderSettings};
    use fun_lux::LuxLightId;

    fn make_live_constants() -> CloudShadowProjectionConstants {
        CloudShadowProjectionConstants::from_inputs(
            &CloudRenderSettings::PRODUCT_DEFAULT,
            crate::clouds::CloudWeatherProfileId::Scattered,
            LuxLightId::new(7),
            [0.0, 1.0, 0.0],
            0,
        )
    }

    /// Pass C7.4.5 acceptance — schema version is stable.
    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_RENDERER_CLOUD_SHADOW_PASSES_SCHEMA_VERSION, 1);
    }

    /// Pass C7.4.5 acceptance — typed project / filter /
    /// register-layer records carry the typed user-spec
    /// reads + writes.
    #[test]
    fn cloud_shadow_records_carry_user_spec_reads_and_writes() {
        let settings = CloudRenderSettings::PRODUCT_DEFAULT;
        let constants = make_live_constants();
        let project = record_lux_cloud_shadow_project(
            &settings,
            &constants,
            CloudShadowFrameDelayMode::OneFrameDelayed,
        );
        assert_eq!(project.role, FrameGraphPassRole::LuxCloudShadowProject);
        // Typed project reads exactly the typed user-spec set.
        assert_eq!(project.reads.len(), 3);
        assert!(
            project
                .reads
                .contains(&FrameGraphResourceType::CloudWeatherMap)
        );
        assert!(
            project
                .reads
                .contains(&FrameGraphResourceType::CloudShapeNoise)
        );
        assert!(
            project
                .reads
                .contains(&FrameGraphResourceType::CloudShadowProjectionConstants)
        );
        // Typed project writes exactly the typed user-spec
        // transmittance target.
        assert_eq!(project.writes.len(), 1);
        assert!(
            project
                .writes
                .contains(&FrameGraphResourceType::CloudWorldShadowTransmittance)
        );

        let filter = record_lux_cloud_shadow_filter(
            &settings,
            &constants,
            CloudShadowFrameDelayMode::OneFrameDelayed,
        );
        assert_eq!(filter.role, FrameGraphPassRole::LuxCloudShadowFilter);
        assert_eq!(filter.reads.len(), 2);
        assert!(
            filter
                .reads
                .contains(&FrameGraphResourceType::CloudWorldShadowTransmittance)
        );
        assert!(
            filter
                .reads
                .contains(&FrameGraphResourceType::CloudShadowProjectionConstants)
        );
        assert_eq!(filter.writes.len(), 1);
        assert!(
            filter
                .writes
                .contains(&FrameGraphResourceType::CloudWorldShadowFiltered)
        );

        let register = record_lux_cloud_shadow_register_layer(
            &settings,
            &constants,
            CloudShadowFrameDelayMode::OneFrameDelayed,
        );
        assert_eq!(
            register.role,
            FrameGraphPassRole::LuxCloudShadowRegisterLayer,
        );
        assert_eq!(register.reads.len(), 2);
        assert!(
            register
                .reads
                .contains(&FrameGraphResourceType::CloudWorldShadowFiltered)
        );
        assert!(
            register
                .reads
                .contains(&FrameGraphResourceType::CloudShadowProjectionConstants)
        );
        // Typed register pass writes the typed aux-layer
        // metadata resource.  MUST NOT write to typed Lux
        // virtual shadow / opaque shadow depth targets.
        assert_eq!(register.writes.len(), 1);
        assert!(
            register
                .writes
                .contains(&FrameGraphResourceType::CloudShadowAuxLayer)
        );
        for w in register.writes {
            assert!(!w.is_lux(), "register write {:?} is lux", w);
        }
    }

    /// Pass C7.4.5 acceptance — typed project / filter /
    /// register-layer passes appear in graph diagnostics.
    #[test]
    fn cloud_shadow_passes_appear_in_graph_diagnostics() {
        let settings = CloudRenderSettings::PRODUCT_DEFAULT;
        let constants = make_live_constants();
        let diagnostics = CloudShadowGraphDiagnostics::from_inputs(
            &settings,
            &constants,
            CloudShadowFrameDelayMode::OneFrameDelayed,
        );
        let slice = diagnostics.as_slice();
        let roles: Vec<_> = slice.iter().map(|r| r.role).collect();
        assert!(roles.contains(&FrameGraphPassRole::LuxCloudShadowProject));
        assert!(roles.contains(&FrameGraphPassRole::LuxCloudShadowFilter));
        assert!(roles.contains(&FrameGraphPassRole::LuxCloudShadowRegisterLayer));
        // Typed every record declares typed resource
        // interaction (reads or writes non-empty).
        assert!(diagnostics.every_pass_declares_resource_interaction());
        // Typed product-default + typed live projection →
        // the typed full chain ran this frame.
        assert!(diagnostics.full_chain_ran_this_frame());
    }

    /// Pass C7.4.5 acceptance — typed
    /// `LuxCloudShadowProject` order key remains before
    /// the typed `LuxCloudShadowFilter` order key.
    #[test]
    fn cloud_shadow_project_order_key_remains_before_filter() {
        assert!(cloud_shadow_project_order_key_is_before_filter());
        assert!(CLOUD_SHADOW_PROJECT_ORDER_KEY < CLOUD_SHADOW_FILTER_ORDER_KEY);
        let settings = CloudRenderSettings::PRODUCT_DEFAULT;
        let constants = make_live_constants();
        let p = record_lux_cloud_shadow_project(
            &settings,
            &constants,
            CloudShadowFrameDelayMode::SameFrame,
        );
        let f = record_lux_cloud_shadow_filter(
            &settings,
            &constants,
            CloudShadowFrameDelayMode::SameFrame,
        );
        assert!(p.order_key < f.order_key);
    }

    /// Pass C7.4.5 acceptance — typed
    /// `LuxCloudShadowRegisterLayer` order key remains
    /// BEFORE typed `LuxDirectLighting` in typed same-frame
    /// mode.  The typed predicate is mode-independent at
    /// the typed const layer (order keys don't shift per
    /// mode); the typed `frame_delay_mode` field on the
    /// typed record + the typed `samples_current_frame_filtered_shadow`
    /// predicate carry the typed same-frame intent.
    #[test]
    fn cloud_shadow_register_layer_remains_before_direct_lighting_same_frame() {
        assert!(cloud_shadow_register_layer_order_key_is_before_direct_lighting());
        assert!(CLOUD_SHADOW_REGISTER_LAYER_ORDER_KEY < LUX_DIRECT_LIGHTING_ORDER_KEY);
        let settings = CloudRenderSettings::PRODUCT_DEFAULT;
        let constants = make_live_constants();
        let register = record_lux_cloud_shadow_register_layer(
            &settings,
            &constants,
            CloudShadowFrameDelayMode::SameFrame,
        );
        // Typed same-frame mode: typed register pass MUST
        // run before typed direct lighting.  Const order
        // check + typed record check.
        assert_eq!(
            register.frame_delay_mode,
            CloudShadowFrameDelayMode::SameFrame
        );
        assert!(
            register
                .frame_delay_mode
                .samples_current_frame_filtered_shadow(),
        );
        assert!(register.order_key < LUX_DIRECT_LIGHTING_ORDER_KEY);
    }

    /// Pass C7.4.5 acceptance — typed one-frame-delayed
    /// mode remains explicit at the typed record level.
    /// Typed `samples_previous_frame_filtered_shadow`
    /// returns `true`; typed `samples_current_frame_filtered_shadow`
    /// returns `false`.
    #[test]
    fn one_frame_delayed_mode_remains_explicit_on_records() {
        let settings = CloudRenderSettings::PRODUCT_DEFAULT;
        let constants = make_live_constants();
        let chain = record_cloud_shadow_pass_chain(
            &settings,
            &constants,
            CloudShadowFrameDelayMode::OneFrameDelayed,
        );
        for record in chain {
            assert_eq!(
                record.frame_delay_mode,
                CloudShadowFrameDelayMode::OneFrameDelayed,
            );
            assert!(
                record
                    .frame_delay_mode
                    .samples_previous_frame_filtered_shadow(),
            );
            assert!(
                !record
                    .frame_delay_mode
                    .samples_current_frame_filtered_shadow(),
            );
        }
        // Typed const ordering invariants hold regardless of
        // typed mode — typed order keys are static.
        assert!(cloud_shadow_pass_recording_invariants_hold());
    }

    /// Pass C7.4.5 acceptance — typed disabled settings
    /// produce typed records with typed `registers_pass =
    /// false` but the typed records still appear in typed
    /// diagnostics (so the typed renderer can audit "pass
    /// was gated off this frame" as opposed to "pass is
    /// absent entirely").
    #[test]
    fn cloud_shadow_records_survive_disabled_settings() {
        let constants = CloudShadowProjectionConstants::DISABLED;
        let chain = record_cloud_shadow_pass_chain(
            &CloudRenderSettings::DISABLED,
            &constants,
            CloudShadowFrameDelayMode::OneFrameDelayed,
        );
        for record in chain {
            assert!(!record.registers_pass);
            assert!(!record.projection_is_live);
            assert!(!record.ran_this_frame());
            // Typed every record still declares typed reads
            // / writes — typed const tables don't shrink per
            // settings.
            assert!(record.declares_any_resource_interaction());
        }
        // Typed roles are still present.
        let roles: Vec<_> = chain.iter().map(|r| r.role).collect();
        assert!(roles.contains(&FrameGraphPassRole::LuxCloudShadowProject));
        assert!(roles.contains(&FrameGraphPassRole::LuxCloudShadowFilter));
        assert!(roles.contains(&FrameGraphPassRole::LuxCloudShadowRegisterLayer));
    }

    /// Pass C7.4.5 acceptance — typed
    /// `CloudQuality::Off` cascade-disables the typed pass
    /// chain (settings.is_active() = false →
    /// registers_world_shadow_pass = false).
    #[test]
    fn cloud_shadow_records_cascade_disable_on_off_quality() {
        let mut settings = CloudRenderSettings::PRODUCT_DEFAULT;
        settings.quality = CloudQuality::Off;
        let constants = make_live_constants();
        let chain = record_cloud_shadow_pass_chain(
            &settings,
            &constants,
            CloudShadowFrameDelayMode::SameFrame,
        );
        for record in chain {
            assert!(
                !record.registers_pass,
                "{:?} should be gated off",
                record.role
            );
            assert!(!record.ran_this_frame());
        }
    }

    /// Pass C7.4.5 acceptance — typed new resource types
    /// (`CloudWeatherMap`, `CloudShapeNoise`,
    /// `CloudShadowAuxLayer`) appear in typed
    /// `FrameGraphResourceType::ALL` and report typed
    /// `is_lux() = false` (cloud-owned, not lux-owned).
    #[test]
    fn cloud_shadow_resource_types_are_not_lux() {
        for ty in [
            FrameGraphResourceType::CloudWeatherMap,
            FrameGraphResourceType::CloudShapeNoise,
            FrameGraphResourceType::CloudShadowAuxLayer,
        ] {
            assert!(
                FrameGraphResourceType::ALL.contains(&ty),
                "{:?} missing from FrameGraphResourceType::ALL",
                ty,
            );
            assert!(!ty.is_lux(), "{:?} reports is_lux=true", ty);
            assert!(ty.as_str().starts_with("cloud_"));
        }
    }
}
