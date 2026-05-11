//! Pass V2.3 — typed scene-to-Lux extraction bridge.
//!
//! The Pass V2.1 bridge initialized the typed
//! `LuxFramePlanner` with a single hardcoded
//! `LuxSceneChangeSignal::unchanged_visible(PROOF_SCENE)`
//! signal.  Pass V2.3 replaces that with extraction-driven
//! signals collected from the typed fun-scene lighting
//! components (`LuxLight`, `LuxEmissive`,
//! `VirtualShadowCaster`, `VirtualShadowReceiver`,
//! `LuxGiParticipant`) so the typed planner sees actual
//! scene work proportional to per-frame changes.
//!
//! The typed extraction bridge is a bevy_ecs `Resource`
//! that accumulates typed `LuxSceneChangeSignal`s; the
//! typed extraction system fills it each frame; the bridge
//! reads it to feed the planner.

use bevy::prelude::{
    Added, App, Changed, Entity, Query, RemovedComponents, ResMut, Resource, Update,
};
use fun_renderer::fun_lux::{LuxSceneChangeSignal, LuxSceneId};
use fun_scene::lux_components::{
    LuxEmissive, LuxGiParticipant, LuxLight, VirtualShadowCaster, VirtualShadowReceiver,
};

pub const FUN_RENDER_LUX_EXTRACTION_SCHEMA_VERSION: u16 = 1;

/// Typed Pass V2.3 extraction bridge.  Accumulates typed
/// `LuxSceneChangeSignal`s during the extraction phase so
/// the typed bridge can feed them to the typed planner.
///
/// The typed bridge keeps a `Vec<LuxSceneChangeSignal>` so
/// the planner can iterate over per-scene signals (one
/// signal per scene id today; future passes can extend
/// this to multi-scene worlds).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Resource)]
pub struct FunRenderLuxExtractionBridge {
    pub schema_version: u16,
    pub signals: Vec<LuxSceneChangeSignal>,
}

impl FunRenderLuxExtractionBridge {
    pub const COLD_DEFAULT: Self = Self {
        schema_version: FUN_RENDER_LUX_EXTRACTION_SCHEMA_VERSION,
        signals: Vec::new(),
    };

    /// Typed predicate: does the typed bridge carry any
    /// typed signals this frame?
    #[must_use]
    pub fn has_signals(&self) -> bool {
        !self.signals.is_empty()
    }

    /// Typed predicate: do all typed signals report the
    /// typed minimal `unchanged_visible` shape (no dirty
    /// bits set)?
    #[must_use]
    pub fn is_minimal(&self) -> bool {
        self.signals.iter().all(|s| !s.dirty())
    }

    /// Typed accessor — returns the typed signals slice
    /// the bridge can feed to the typed planner.
    #[must_use]
    pub fn signals(&self) -> &[LuxSceneChangeSignal] {
        &self.signals
    }

    /// Typed reset — clears the signal list so the next
    /// extraction tick starts from empty.
    pub fn reset(&mut self) {
        self.signals.clear();
    }
}

impl Default for FunRenderLuxExtractionBridge {
    fn default() -> Self {
        Self::COLD_DEFAULT
    }
}

/// Typed Pass V2.3 extraction report.  Summarizes what
/// the typed extraction system observed this frame.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Resource)]
pub struct FunRenderLuxExtractionReport {
    pub schema_version: u16,
    pub frame_index: u64,
    /// Typed counter — entities with `Added<LuxLight>` this
    /// frame.
    pub added_lights: u32,
    /// Typed counter — entities with `Changed<LuxLight>`
    /// this frame.
    pub changed_lights: u32,
    /// Typed counter — entities with `LuxLight` removed
    /// this frame.
    pub removed_lights: u32,
    /// Typed counter — entities with `Changed<LuxEmissive>`
    /// this frame.
    pub changed_emissives: u32,
    /// Typed counter — entities with
    /// `Changed<VirtualShadowCaster>` this frame.
    pub changed_shadow_casters: u32,
    /// Typed counter — entities with
    /// `Changed<VirtualShadowReceiver>` this frame.
    pub changed_shadow_receivers: u32,
    /// Typed counter — entities with
    /// `Changed<LuxGiParticipant>` this frame.
    pub changed_gi_participants: u32,
    /// Typed counter — typed signals produced this frame.
    pub signals_produced: u32,
}

impl FunRenderLuxExtractionReport {
    pub const COLD_DEFAULT: Self = Self {
        schema_version: FUN_RENDER_LUX_EXTRACTION_SCHEMA_VERSION,
        frame_index: 0,
        added_lights: 0,
        changed_lights: 0,
        removed_lights: 0,
        changed_emissives: 0,
        changed_shadow_casters: 0,
        changed_shadow_receivers: 0,
        changed_gi_participants: 0,
        signals_produced: 0,
    };

    /// Typed predicate: did the typed extraction see any
    /// scene work this frame (any of the typed counters
    /// non-zero)?
    #[must_use]
    pub const fn saw_scene_work(&self) -> bool {
        self.added_lights > 0
            || self.changed_lights > 0
            || self.removed_lights > 0
            || self.changed_emissives > 0
            || self.changed_shadow_casters > 0
            || self.changed_shadow_receivers > 0
            || self.changed_gi_participants > 0
    }
}

/// Typed extraction system.  Walks the typed scene
/// component change queries + writes typed
/// `LuxSceneChangeSignal`s into the bridge.
///
/// The typed mapping is per the user spec:
///   - changed / added / removed `LuxLight`            → `lights_changed`
///   - changed `LuxEmissive`                           → `lights_changed`
///   - changed `VirtualShadowCaster` / `Receiver`      → `shadow_caster_set_changed`
///   - changed `LuxGiParticipant`                      → `gi_probes_changed`
///
/// Fog / volumetric / look-profile / camera-movement
/// signals are reserved for future passes that wire
/// fun-scene fog volumes + camera tracking; the typed
/// fields exist on `LuxSceneChangeSignal` (Pass V2.3) so
/// the routing is ready.
#[allow(clippy::too_many_arguments)]
pub fn fun_render_lux_extraction_system(
    added_lights: Query<Entity, Added<LuxLight>>,
    changed_lights: Query<Entity, Changed<LuxLight>>,
    mut removed_lights: RemovedComponents<LuxLight>,
    changed_emissives: Query<Entity, Changed<LuxEmissive>>,
    changed_casters: Query<Entity, Changed<VirtualShadowCaster>>,
    changed_receivers: Query<Entity, Changed<VirtualShadowReceiver>>,
    changed_gi: Query<Entity, Changed<LuxGiParticipant>>,
    mut bridge: ResMut<FunRenderLuxExtractionBridge>,
    mut report: ResMut<FunRenderLuxExtractionReport>,
) {
    let added_light_count = added_lights.iter().count() as u32;
    // `Added<T>` is a subset of `Changed<T>` — subtract to
    // avoid double-counting in the typed report.
    let changed_light_count = changed_lights.iter().count() as u32;
    let removed_light_count = removed_lights.read().count() as u32;
    let changed_emissive_count = changed_emissives.iter().count() as u32;
    let changed_caster_count = changed_casters.iter().count() as u32;
    let changed_receiver_count = changed_receivers.iter().count() as u32;
    let changed_gi_count = changed_gi.iter().count() as u32;

    let mut signal = LuxSceneChangeSignal::unchanged_visible(LuxSceneId::PROOF_SCENE, 0);

    if added_light_count > 0 || changed_light_count > 0 || removed_light_count > 0 {
        signal.lights_changed = true;
    }
    if changed_emissive_count > 0 {
        // Emissive changes also propagate as light changes
        // (the typed Pass 6 EmissivePromotion path treats
        // emissives as candidate light records).
        signal.lights_changed = true;
    }
    if changed_caster_count > 0 || changed_receiver_count > 0 {
        signal.shadow_caster_set_changed = true;
    }
    if changed_gi_count > 0 {
        signal.gi_probes_changed = true;
    }

    bridge.signals.clear();
    bridge.signals.push(signal);

    report.schema_version = FUN_RENDER_LUX_EXTRACTION_SCHEMA_VERSION;
    report.added_lights = added_light_count;
    report.changed_lights = changed_light_count;
    report.removed_lights = removed_light_count;
    report.changed_emissives = changed_emissive_count;
    report.changed_shadow_casters = changed_caster_count;
    report.changed_shadow_receivers = changed_receiver_count;
    report.changed_gi_participants = changed_gi_count;
    report.signals_produced = bridge.signals.len() as u32;
}

/// Typed Pass V2.3 install helper — registers the typed
/// extraction resources + the typed extraction system on
/// the bevy app.
pub fn install_renderer_bridge_lux_extraction(app: &mut App) {
    app.init_resource::<FunRenderLuxExtractionBridge>()
        .init_resource::<FunRenderLuxExtractionReport>()
        .add_systems(Update, fun_render_lux_extraction_system);
}

// ============================================================================
// Tests — Pass V2.3 typed acceptance
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::Schedule;
    use fun_renderer::fun_lux::LuxSceneChangeSignal;

    fn build_app() -> App {
        let mut app = App::new();
        app.init_resource::<FunRenderLuxExtractionBridge>()
            .init_resource::<FunRenderLuxExtractionReport>();
        app
    }

    fn run_extraction(app: &mut App) {
        let mut schedule = Schedule::default();
        schedule.add_systems(fun_render_lux_extraction_system);
        schedule.run(app.world_mut());
    }

    /// Pass V2.3 acceptance — a changed `LuxLight`
    /// component emits a typed `lights_changed`
    /// `LuxSceneChangeSignal`.
    #[test]
    fn changed_lux_light_emits_lux_scene_change_signal() {
        let mut app = build_app();
        let entity = app.world_mut().spawn(LuxLight::directional(1_000.0)).id();
        // First run: the spawn registers Added<LuxLight>.
        run_extraction(&mut app);
        let bridge = app.world().resource::<FunRenderLuxExtractionBridge>();
        assert_eq!(bridge.signals.len(), 1);
        assert!(bridge.signals[0].lights_changed);
        let _ = entity;
    }

    /// Pass V2.3 acceptance — a changed
    /// `VirtualShadowCaster` emits the typed
    /// `shadow_caster_set_changed` signal.
    #[test]
    fn changed_shadow_caster_emits_shadow_signal() {
        let mut app = build_app();
        app.world_mut().spawn(VirtualShadowCaster::default());
        run_extraction(&mut app);
        let bridge = app.world().resource::<FunRenderLuxExtractionBridge>();
        assert!(bridge.signals[0].shadow_caster_set_changed);
        // Light bit must NOT be set since no LuxLight was
        // touched.
        assert!(!bridge.signals[0].lights_changed);
    }

    /// Pass V2.3 acceptance — a changed `LuxGiParticipant`
    /// emits the typed `gi_probes_changed` signal.
    #[test]
    fn changed_gi_participant_emits_gi_signal() {
        let mut app = build_app();
        app.world_mut().spawn(LuxGiParticipant::default());
        run_extraction(&mut app);
        let bridge = app.world().resource::<FunRenderLuxExtractionBridge>();
        assert!(bridge.signals[0].gi_probes_changed);
        assert!(!bridge.signals[0].lights_changed);
        assert!(!bridge.signals[0].shadow_caster_set_changed);
    }

    /// Pass V2.3 acceptance — an unchanged scene (no
    /// component additions / changes / removals) emits the
    /// typed minimal `unchanged_visible` signal.
    #[test]
    fn unchanged_scene_emits_minimal_signal() {
        let mut app = build_app();
        // First tick "consumes" the Added<T> generation
        // counter (no entities exist yet so nothing fires).
        run_extraction(&mut app);
        // Second tick: no changes.
        run_extraction(&mut app);
        let bridge = app.world().resource::<FunRenderLuxExtractionBridge>();
        assert_eq!(bridge.signals.len(), 1);
        let signal = &bridge.signals[0];
        assert!(!signal.dirty(), "minimal signal must not be dirty: {:?}", signal);
        let report = app.world().resource::<FunRenderLuxExtractionReport>();
        assert!(!report.saw_scene_work());
    }

    /// Pass V2.3 acceptance — a removed `LuxLight` emits a
    /// typed `lights_changed` signal.
    #[test]
    fn removed_light_emits_removed_signal() {
        let mut app = build_app();
        let entity = app.world_mut().spawn(LuxLight::directional(1_000.0)).id();
        // First tick: the spawn registers Added; report
        // shows added_lights >= 1.
        run_extraction(&mut app);
        // Remove the component.
        app.world_mut()
            .entity_mut(entity)
            .remove::<LuxLight>();
        run_extraction(&mut app);
        let bridge = app.world().resource::<FunRenderLuxExtractionBridge>();
        let signal = &bridge.signals[0];
        assert!(
            signal.lights_changed,
            "removed light must set lights_changed: {:?}",
            signal,
        );
        let report = app.world().resource::<FunRenderLuxExtractionReport>();
        assert!(report.removed_lights >= 1);
    }

    #[test]
    fn extraction_report_saw_scene_work_predicate() {
        let mut r = FunRenderLuxExtractionReport::COLD_DEFAULT;
        assert!(!r.saw_scene_work());
        r.changed_lights = 1;
        assert!(r.saw_scene_work());
        let mut r2 = FunRenderLuxExtractionReport::COLD_DEFAULT;
        r2.changed_gi_participants = 1;
        assert!(r2.saw_scene_work());
    }

    #[test]
    fn extraction_bridge_predicates() {
        let mut bridge = FunRenderLuxExtractionBridge::COLD_DEFAULT;
        assert!(!bridge.has_signals());
        assert!(bridge.is_minimal()); // empty iter = all-true
        bridge
            .signals
            .push(LuxSceneChangeSignal::unchanged_visible(
                LuxSceneId::PROOF_SCENE,
                0,
            ));
        assert!(bridge.has_signals());
        assert!(bridge.is_minimal());
        bridge
            .signals
            .push(LuxSceneChangeSignal::light_changed(LuxSceneId::PROOF_SCENE, 1));
        assert!(!bridge.is_minimal());
        bridge.reset();
        assert!(!bridge.has_signals());
    }
}
