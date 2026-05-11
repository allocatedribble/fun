//! Pass C0 — typed cloud renderer migration doctrine.
//!
//! This module is a typed bridge / migration donor.
//! Product cloud execution is owned by `fun-renderer`
//! (see `fun_renderer::clouds`,
//! `fun_renderer::cloud_executor`,
//! `fun_renderer::cloud_passes`,
//! `fun_renderer::cloud_resources`,
//! `fun_renderer::cloud_shadow`,
//! `fun_renderer::cloud_diagnostics`).
//!
//! Pass C0 typed contract — every flag in
//! `fun_renderer::FunCloudRendererContract::CURRENT` is
//! `true`:
//!
//! - `fun_renderer_owns_cloud_execution`
//! - `fun_render_extracts_only`
//! - `clouds_use_renderer_frame_graph`
//! - `clouds_can_consume_lux_volumetric_lighting`
//! - `clouds_can_cast_world_shadows`
//!
//! `fun_render::sky` MUST NOT gain new cloud GPU
//! execution logic; new cloud execution code MUST land in
//! `fun-renderer`.  Today this module keeps the typed
//! Bevy render plugin + the typed env parsing + the
//! typed weather/profile records, all of which feed the
//! typed `fun-renderer` cloud renderer.  Subsequent
//! cloud passes (C2+) will increasingly route through
//! `fun-renderer` until this module collapses to a typed
//! extraction-only shim.

pub mod config;
pub mod plugin;
pub mod profiles;
pub mod render;
pub mod renderer_bridge;
pub mod weather;

pub use config::{FunCloudDebugOverlay, FunCloudInternalScale, FunCloudQuality, FunCloudSettings};
pub use plugin::{
    FunCloudHistoryResetEvent, FunCloudHistoryResetReason, FunCloudHistoryState, FunSkyPlugin,
    request_cloud_history_reset,
};
pub use profiles::builtin_weather_profile;
pub use weather::{
    FunCloudTypeMix, FunWeatherPattern, FunWeatherPatternPhase, FunWeatherProfile,
    FunWeatherProfileId, FunWeatherState, FunWeatherTransition, FunWeatherTransitionCurve,
    FunWeatherValidationError,
};
