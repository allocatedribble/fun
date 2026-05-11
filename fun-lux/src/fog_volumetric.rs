//! Pass 8 — typed volumetric lighting + fog policy surface.
//!
//! The existing [`crate::volumetric`] module ships the typed
//! `FunLuxVolumetricSettings` Pass 1+ runtime needs: froxel
//! extent, density model, scattering anisotropy, integration
//! mode, temporal reproject alpha.  Pass 8 extends that into
//! the typed product surface the user spec calls out:
//!
//! - Per-quality typed froxel grid settings (Low / Medium /
//!   High / Cinematic) matching the user's table verbatim.
//! - Typed temporal stability settings (jitter, blue noise,
//!   reproject, clamp, reset reasons).
//! - Typed fog-volume shape taxonomy (Global / Height /
//!   Sphere / Box / Capsule / SdfVolume).
//! - Typed fog-volume parameters (density, extinction,
//!   scattering color, emission color, anisotropy, height
//!   falloff, noise strength, noise scale, wind vector,
//!   blend mode, scene mask, priority, static/dynamic flag).
//! - Typed per-light volumetric controls (volumetric_enabled,
//!   volumetric_intensity, godray_intensity, godray_sharpness,
//!   phase_g, casts_volumetric_shadow).
//! - Typed renderer-side resource taxonomy (6 typed
//!   `LuxVolumetricResourceIntent` variants).
//! - Typed renderer-side pass taxonomy (9 typed
//!   `LuxVolumetricPassRole` variants matching the user's
//!   pass list verbatim).
//! - Typed debug-view taxonomy (froxel density, scattering,
//!   integrated fog, history rejection) — rule #6.
//! - Typed 6-rule acceptance verdict.
//!
//! Pass 8 keeps fun-lux renderer-neutral — every type is a
//! typed descriptor / predicate; no `wgpu`, `naga`, or
//! raw-window imports cross the crate boundary.
//!
//! f32 fields are encoded Q8 / Q16 fixed-point so the typed
//! records stay `Hash + Eq` stable, consistent with the
//! convention established in Pass 6.

pub const FUN_LUX_FOG_VOLUMETRIC_SCHEMA_VERSION: u16 = 1;
pub const FUN_LUX_VOLUMETRIC_QUALITY_COUNT: usize = 5;
pub const FUN_LUX_VOLUMETRIC_RESET_REASON_COUNT: usize = 5;
pub const FUN_LUX_FOG_VOLUME_SHAPE_COUNT: usize = 6;
pub const FUN_LUX_FOG_BLEND_MODE_COUNT: usize = 4;
pub const FUN_LUX_FOG_LIFETIME_FLAG_COUNT: usize = 2;
pub const FUN_LUX_VOLUMETRIC_PASS_ROLE_COUNT: usize = 9;
pub const FUN_LUX_VOLUMETRIC_RESOURCE_INTENT_COUNT: usize = 6;
pub const FUN_LUX_VOLUMETRIC_DEBUG_VIEW_COUNT: usize = 4;
pub const FUN_LUX_VOLUMETRIC_PASS8_ACCEPTANCE_RULE_COUNT: usize = 6;

// ============================================================================
// Section 1 — Typed quality tier (Off / Low / Medium / High / Cinematic)
// ============================================================================

/// Typed Pass 8 quality tier.  Drives the froxel grid
/// resolution and the temporal stability dial.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxVolumetricQuality {
    /// Off — the volumetric pipeline is disabled.
    Off,
    /// Quarter / eighth resolution + 16 Z slices.
    Low,
    /// Quarter resolution + 32 Z slices.
    Medium,
    /// Quarter resolution + 48..64 Z slices.
    #[default]
    High,
    /// Higher XY or better reconstruction + stronger
    /// temporal stability.
    Cinematic,
}

impl LuxVolumetricQuality {
    pub const ALL: [Self; FUN_LUX_VOLUMETRIC_QUALITY_COUNT] = [
        Self::Off,
        Self::Low,
        Self::Medium,
        Self::High,
        Self::Cinematic,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Cinematic => "cinematic",
        }
    }

    /// Typed predicate: is the pipeline active at all?
    #[must_use]
    pub const fn is_active(self) -> bool {
        !matches!(self, Self::Off)
    }
}

// ============================================================================
// Section 2 — Typed froxel grid settings (user spec table)
// ============================================================================

/// Typed froxel grid descriptor.  Carries XY resolution
/// (relative to viewport) + Z slice count.  The renderer
/// resolves the XY divisor against the viewport extent at
/// allocation time; fun-lux only encodes the typed table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxFroxelGridSettings {
    pub schema_version: u16,
    pub quality: LuxVolumetricQuality,
    /// Viewport XY divisor — 4 = quarter, 8 = eighth.
    pub xy_divisor: u8,
    /// Z slice count along the view ray.
    pub z_slices: u16,
    /// Typed flag — when `true`, the renderer uses a
    /// higher-quality reconstruction kernel (cinematic).
    pub uses_high_quality_reconstruction: bool,
}

impl LuxFroxelGridSettings {
    /// Typed `for_quality(quality)` builder matching the user
    /// spec table verbatim:
    ///
    /// - Off: 1×1×1 (typed inactive marker).
    /// - Low: 8x XY divisor + 16 Z slices.
    /// - Medium: 4x XY divisor + 32 Z slices.
    /// - High: 4x XY divisor + 64 Z slices.
    /// - Cinematic: 2x XY divisor + 64 Z slices + high-quality
    ///   reconstruction.
    #[must_use]
    pub const fn for_quality(quality: LuxVolumetricQuality) -> Self {
        match quality {
            LuxVolumetricQuality::Off => Self {
                schema_version: FUN_LUX_FOG_VOLUMETRIC_SCHEMA_VERSION,
                quality,
                xy_divisor: 0,
                z_slices: 0,
                uses_high_quality_reconstruction: false,
            },
            LuxVolumetricQuality::Low => Self {
                schema_version: FUN_LUX_FOG_VOLUMETRIC_SCHEMA_VERSION,
                quality,
                xy_divisor: 8,
                z_slices: 16,
                uses_high_quality_reconstruction: false,
            },
            LuxVolumetricQuality::Medium => Self {
                schema_version: FUN_LUX_FOG_VOLUMETRIC_SCHEMA_VERSION,
                quality,
                xy_divisor: 4,
                z_slices: 32,
                uses_high_quality_reconstruction: false,
            },
            LuxVolumetricQuality::High => Self {
                schema_version: FUN_LUX_FOG_VOLUMETRIC_SCHEMA_VERSION,
                quality,
                xy_divisor: 4,
                z_slices: 64,
                uses_high_quality_reconstruction: false,
            },
            LuxVolumetricQuality::Cinematic => Self {
                schema_version: FUN_LUX_FOG_VOLUMETRIC_SCHEMA_VERSION,
                quality,
                xy_divisor: 2,
                z_slices: 64,
                uses_high_quality_reconstruction: true,
            },
        }
    }

    /// Typed product default — High quality.
    pub const PRODUCT_DEFAULT: Self = Self::for_quality(LuxVolumetricQuality::High);

    /// Typed cell count given a viewport XY extent.  Returns
    /// `0` when the grid is the typed `Off` marker.
    #[must_use]
    pub const fn cell_count_for_viewport(&self, viewport_x: u32, viewport_y: u32) -> u64 {
        if self.xy_divisor == 0 || self.z_slices == 0 {
            return 0;
        }
        let div = self.xy_divisor as u32;
        let x_cells = (viewport_x.saturating_add(div - 1)) / div;
        let y_cells = (viewport_y.saturating_add(div - 1)) / div;
        (x_cells as u64) * (y_cells as u64) * (self.z_slices as u64)
    }

    /// Typed predicate: is the grid the typed-active state?
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.quality.is_active() && self.z_slices > 0 && self.xy_divisor > 0
    }
}

// ============================================================================
// Section 3 — Typed temporal stability settings
// ============================================================================

/// Typed temporal reset reason.  The user spec lists five
/// reset triggers; Pass 8 encodes them as a typed enum so
/// the renderer can match-dispatch on the cause.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxVolumetricResetReason {
    /// No reset — history is reused this frame.
    #[default]
    NoReset,
    /// Camera cut (scene-cam switch).
    CameraCut,
    /// FOV jump (focal-length change above threshold).
    FovJump,
    /// Scene revision jump (scene reload / world-stream
    /// boundary crossing).
    SceneRevisionJump,
    /// Fog profile changed (artist-side reauthor).
    FogProfileChanged,
    /// Quality / resolution dial changed (viewer resized
    /// the window or shifted quality tier).
    QualityOrResolutionChanged,
}

impl LuxVolumetricResetReason {
    /// Typed `ALL` — every variant including `NoReset`.
    pub const ALL: [Self; FUN_LUX_VOLUMETRIC_RESET_REASON_COUNT + 1] = [
        Self::NoReset,
        Self::CameraCut,
        Self::FovJump,
        Self::SceneRevisionJump,
        Self::FogProfileChanged,
        Self::QualityOrResolutionChanged,
    ];

    /// Typed list of the user's five "must reset" reasons
    /// — excludes `NoReset`.
    pub const RESET_REASONS: [Self; FUN_LUX_VOLUMETRIC_RESET_REASON_COUNT] = [
        Self::CameraCut,
        Self::FovJump,
        Self::SceneRevisionJump,
        Self::FogProfileChanged,
        Self::QualityOrResolutionChanged,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoReset => "no_reset",
            Self::CameraCut => "camera_cut",
            Self::FovJump => "fov_jump",
            Self::SceneRevisionJump => "scene_revision_jump",
            Self::FogProfileChanged => "fog_profile_changed",
            Self::QualityOrResolutionChanged => "quality_or_resolution_changed",
        }
    }

    /// Typed predicate: does this reason demand a typed
    /// history reset?
    #[must_use]
    pub const fn demands_history_reset(self) -> bool {
        !matches!(self, Self::NoReset)
    }
}

/// Typed temporal stability settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxVolumetricTemporalSettings {
    pub schema_version: u16,
    /// Jitter the froxel sample location per frame.
    pub jitter_enabled: bool,
    /// Use blue-noise jitter (vs white noise).
    pub blue_noise_enabled: bool,
    /// Reproject previous-frame volumetric history.
    pub reproject_history: bool,
    /// Clamp reprojected history against current frame.
    pub clamp_against_current_frame: bool,
    /// Reproject blend alpha Q8 (0=no reuse, 255=fully reuse).
    pub reproject_alpha_q8: u8,
}

impl LuxVolumetricTemporalSettings {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_FOG_VOLUMETRIC_SCHEMA_VERSION,
        jitter_enabled: true,
        blue_noise_enabled: true,
        reproject_history: true,
        clamp_against_current_frame: true,
        reproject_alpha_q8: 192,
    };

    pub const COLD_DEFAULT: Self = Self {
        schema_version: FUN_LUX_FOG_VOLUMETRIC_SCHEMA_VERSION,
        jitter_enabled: false,
        blue_noise_enabled: false,
        reproject_history: false,
        clamp_against_current_frame: false,
        reproject_alpha_q8: 0,
    };

    /// Typed predicate: rule #5 — typed temporal stability
    /// is configured for stable camera-motion behavior.
    /// Requires jitter + blue noise + reproject + clamp.
    #[must_use]
    pub const fn is_camera_motion_stable(&self) -> bool {
        self.jitter_enabled
            && self.blue_noise_enabled
            && self.reproject_history
            && self.clamp_against_current_frame
    }
}

// ============================================================================
// Section 4 — Typed fog-volume shape
// ============================================================================

/// Typed fog-volume shape.  Matches the user spec's six
/// listed shapes verbatim.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxFogVolumeShape {
    /// World-global fog (no spatial bound).
    #[default]
    Global,
    /// Height fog (1D Y-axis falloff).
    Height,
    /// Sphere-bounded local volume.
    Sphere,
    /// Box-bounded local volume.
    Box,
    /// Capsule-bounded local volume (optional in user spec).
    Capsule,
    /// Signed distance field volume (later in user spec).
    SdfVolume,
}

impl LuxFogVolumeShape {
    pub const ALL: [Self; FUN_LUX_FOG_VOLUME_SHAPE_COUNT] = [
        Self::Global,
        Self::Height,
        Self::Sphere,
        Self::Box,
        Self::Capsule,
        Self::SdfVolume,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Height => "height",
            Self::Sphere => "sphere",
            Self::Box => "box",
            Self::Capsule => "capsule",
            Self::SdfVolume => "sdf_volume",
        }
    }

    /// Typed predicate: does this shape bound a finite
    /// region of space?  `Global` does not.
    #[must_use]
    pub const fn is_local(self) -> bool {
        !matches!(self, Self::Global)
    }
}

// ============================================================================
// Section 5 — Typed fog-volume blend mode + lifetime flag
// ============================================================================

/// Typed fog-volume blend mode.  Drives how a fog volume
/// combines with the volume(s) below it in the typed
/// priority order.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxFogBlendMode {
    /// Replace prior contribution.
    Replace,
    /// Additive — add density / scattering.
    #[default]
    Additive,
    /// Multiply — modulate prior contribution.
    Multiply,
    /// Max — keep the brightest contributor (useful for
    /// godrays).
    Max,
}

impl LuxFogBlendMode {
    pub const ALL: [Self; FUN_LUX_FOG_BLEND_MODE_COUNT] =
        [Self::Replace, Self::Additive, Self::Multiply, Self::Max];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Replace => "replace",
            Self::Additive => "additive",
            Self::Multiply => "multiply",
            Self::Max => "max",
        }
    }
}

/// Typed fog static/dynamic flag.  Affects how the fog
/// participates in invalidation: `Static` is cache-eligible,
/// `Dynamic` updates every frame.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxFogLifetimeFlag {
    /// Static — author-set, never animated.
    Static,
    /// Dynamic — animated, gameplay-controlled.
    #[default]
    Dynamic,
}

impl LuxFogLifetimeFlag {
    pub const ALL: [Self; FUN_LUX_FOG_LIFETIME_FLAG_COUNT] = [Self::Static, Self::Dynamic];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Dynamic => "dynamic",
        }
    }

    /// Typed predicate: is this fog eligible for the static
    /// shadow-cache-style reuse path?
    #[must_use]
    pub const fn is_cache_eligible(self) -> bool {
        matches!(self, Self::Static)
    }
}

// ============================================================================
// Section 6 — Typed fog-volume parameters (13 user-spec fields)
// ============================================================================

/// Typed Q16 fixed-point color (R/G/B/A in 16-bit slots).
/// Avoids f32 in the record so it stays `Hash + Eq`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxQ16Color {
    pub r_q16: u16,
    pub g_q16: u16,
    pub b_q16: u16,
    pub a_q16: u16,
}

impl LuxQ16Color {
    pub const WHITE: Self = Self {
        r_q16: u16::MAX,
        g_q16: u16::MAX,
        b_q16: u16::MAX,
        a_q16: u16::MAX,
    };

    pub const BLACK: Self = Self {
        r_q16: 0,
        g_q16: 0,
        b_q16: 0,
        a_q16: u16::MAX,
    };
}

/// Typed Q16 wind vector (X/Y/Z each Q16 fixed-point).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxQ16Vec3 {
    pub x_q16: i32,
    pub y_q16: i32,
    pub z_q16: i32,
}

impl LuxQ16Vec3 {
    pub const ZERO: Self = Self {
        x_q16: 0,
        y_q16: 0,
        z_q16: 0,
    };
}

/// Typed fog-volume parameters.  Encodes the 13 user-spec
/// fields plus a typed `shape` + `schema_version`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxFogVolumeParameters {
    pub schema_version: u16,
    pub shape: LuxFogVolumeShape,
    /// Density (Q16, where 65_535 = 1.0).
    pub density_q16: u16,
    /// Extinction (Q16) — typically same magnitude as density.
    pub extinction_q16: u16,
    pub scattering_color: LuxQ16Color,
    pub emission_color: LuxQ16Color,
    /// Henyey-Greenstein g in Q8 fixed-point (i16 to allow
    /// backward scatter).
    pub anisotropy_g_q8: i16,
    /// Height falloff (Q16) for height-mode volumes; 0 for
    /// non-height volumes.
    pub height_falloff_q16: u16,
    /// Noise strength (Q16).
    pub noise_strength_q16: u16,
    /// Noise scale (Q16).
    pub noise_scale_q16: u16,
    pub wind_vec: LuxQ16Vec3,
    pub blend_mode: LuxFogBlendMode,
    /// Scene mask — bitset selecting which scenes / layers
    /// this volume affects.
    pub scene_mask: u32,
    /// Priority — higher wins under `LuxFogBlendMode::Max`.
    pub priority: u16,
    pub lifetime_flag: LuxFogLifetimeFlag,
}

impl LuxFogVolumeParameters {
    pub const PRODUCT_DEFAULT_GLOBAL: Self = Self {
        schema_version: FUN_LUX_FOG_VOLUMETRIC_SCHEMA_VERSION,
        shape: LuxFogVolumeShape::Global,
        density_q16: 6_553, // ~0.1
        extinction_q16: 6_553,
        scattering_color: LuxQ16Color::WHITE,
        emission_color: LuxQ16Color::BLACK,
        anisotropy_g_q8: 102, // ~0.4 — weak forward
        height_falloff_q16: 0,
        noise_strength_q16: 0,
        noise_scale_q16: 0,
        wind_vec: LuxQ16Vec3::ZERO,
        blend_mode: LuxFogBlendMode::Additive,
        scene_mask: u32::MAX,
        priority: 0,
        lifetime_flag: LuxFogLifetimeFlag::Static,
    };

    pub const PRODUCT_DEFAULT_HEIGHT: Self = Self {
        shape: LuxFogVolumeShape::Height,
        height_falloff_q16: 13_107, // ~0.2
        ..Self::PRODUCT_DEFAULT_GLOBAL
    };
}

// ============================================================================
// Section 7 — Typed fog-volume component-shaped records
// ============================================================================

/// Typed `LuxFogVolume` declaration.  Wraps the typed
/// parameters with a stable id so the renderer can resolve
/// per-frame state by key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxFogVolume {
    pub schema_version: u16,
    pub stable_id: &'static str,
    pub parameters: LuxFogVolumeParameters,
}

/// Typed global fog declaration (single typed instance per
/// scene). The user spec lists this as a separate
/// component; in fun-lux it is just a [`LuxFogVolume`] with
/// the `Global` shape, but Pass 8 keeps the typed alias
/// for clarity.
pub type LuxGlobalFog = LuxFogVolume;

/// Typed height fog declaration.
pub type LuxHeightFog = LuxFogVolume;

/// Typed local fog volume declaration (sphere / box /
/// capsule / SDF).
pub type LuxLocalFogVolume = LuxFogVolume;

// ============================================================================
// Section 8 — Typed per-light volumetric controls
// ============================================================================

/// Typed per-light volumetric override.  The user spec
/// lists six fields the light component must surface.
/// fun-lux ships them as a typed record so the renderer can
/// resolve them alongside the typed
/// [`crate::light_component::FunLuxLightComponent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxVolumetricLightOverride {
    pub schema_version: u16,
    pub volumetric_enabled: bool,
    /// Volumetric intensity multiplier Q16 (1.0 = 65_535).
    pub volumetric_intensity_q16: u16,
    /// Godray intensity multiplier Q16.
    pub godray_intensity_q16: u16,
    /// Godray sharpness Q16 (controls forward-scatter
    /// concentration along the light direction).
    pub godray_sharpness_q16: u16,
    /// Phase function `g` (Henyey-Greenstein) Q8 fixed-point.
    pub phase_g_q8: i16,
    pub casts_volumetric_shadow: bool,
}

impl LuxVolumetricLightOverride {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_FOG_VOLUMETRIC_SCHEMA_VERSION,
        volumetric_enabled: true,
        volumetric_intensity_q16: u16::MAX,
        godray_intensity_q16: u16::MAX / 2,
        godray_sharpness_q16: u16::MAX / 2,
        phase_g_q8: 102, // ~0.4 weak forward
        casts_volumetric_shadow: true,
    };

    pub const COLD_DEFAULT: Self = Self {
        schema_version: FUN_LUX_FOG_VOLUMETRIC_SCHEMA_VERSION,
        volumetric_enabled: false,
        volumetric_intensity_q16: 0,
        godray_intensity_q16: 0,
        godray_sharpness_q16: 0,
        phase_g_q8: 0,
        casts_volumetric_shadow: false,
    };

    /// Typed predicate: does this override contribute to
    /// the volumetric pass at all?
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.volumetric_enabled && self.volumetric_intensity_q16 > 0
    }
}

// ============================================================================
// Section 9 — Typed sub-settings bundles (global / height / godray)
// ============================================================================

/// Typed global fog sub-settings.  Surfaces the typed
/// global-fog policy the planner-level
/// [`LuxVolumetricSettings`] owns.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxGlobalFogSettings {
    pub schema_version: u16,
    pub enabled: bool,
    pub parameters: LuxFogVolumeParameters,
}

impl LuxGlobalFogSettings {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_FOG_VOLUMETRIC_SCHEMA_VERSION,
        enabled: true,
        parameters: LuxFogVolumeParameters::PRODUCT_DEFAULT_GLOBAL,
    };

    pub const COLD_DEFAULT: Self = Self {
        schema_version: FUN_LUX_FOG_VOLUMETRIC_SCHEMA_VERSION,
        enabled: false,
        parameters: LuxFogVolumeParameters::PRODUCT_DEFAULT_GLOBAL,
    };
}

/// Typed height fog sub-settings.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxHeightFogSettings {
    pub schema_version: u16,
    pub enabled: bool,
    pub parameters: LuxFogVolumeParameters,
}

impl LuxHeightFogSettings {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_FOG_VOLUMETRIC_SCHEMA_VERSION,
        enabled: true,
        parameters: LuxFogVolumeParameters::PRODUCT_DEFAULT_HEIGHT,
    };

    pub const COLD_DEFAULT: Self = Self {
        schema_version: FUN_LUX_FOG_VOLUMETRIC_SCHEMA_VERSION,
        enabled: false,
        parameters: LuxFogVolumeParameters::PRODUCT_DEFAULT_HEIGHT,
    };
}

/// Typed godray sub-settings.
///
/// Pass 9 supersedes the Pass 8 placeholder with the typed
/// canonical [`crate::godrays::LuxGodraySettings`] (8 fields
/// matching the user spec verbatim).  The canonical home is
/// [`crate::godrays`] because Pass 9 rule #5 requires the
/// settings live on the typed [`crate::look::FunLuxLookProfile`].
/// `LuxVolumetricSettings` re-exports the same record so the
/// volumetric pipeline + the look profile share one typed
/// source of truth.
pub use crate::godrays::LuxGodraySettings;

// ============================================================================
// Section 10 — Typed top-level volumetric settings (user spec)
// ============================================================================

/// Typed top-level `LuxVolumetricSettings` matching the
/// user spec verbatim.  Carries every Pass 8 sub-bundle so
/// the planner can hand the renderer one typed handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxVolumetricSettings {
    pub schema_version: u16,
    pub enabled: bool,
    pub quality: LuxVolumetricQuality,
    pub froxel_grid: LuxFroxelGridSettings,
    pub temporal: LuxVolumetricTemporalSettings,
    pub global_fog: LuxGlobalFogSettings,
    pub height_fog: LuxHeightFogSettings,
    pub godrays: LuxGodraySettings,
}

impl LuxVolumetricSettings {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_FOG_VOLUMETRIC_SCHEMA_VERSION,
        enabled: true,
        quality: LuxVolumetricQuality::High,
        froxel_grid: LuxFroxelGridSettings::PRODUCT_DEFAULT,
        temporal: LuxVolumetricTemporalSettings::PRODUCT_DEFAULT,
        global_fog: LuxGlobalFogSettings::PRODUCT_DEFAULT,
        height_fog: LuxHeightFogSettings::PRODUCT_DEFAULT,
        godrays: LuxGodraySettings::PRODUCT_DEFAULT,
    };

    pub const COLD_DEFAULT: Self = Self {
        schema_version: FUN_LUX_FOG_VOLUMETRIC_SCHEMA_VERSION,
        enabled: false,
        quality: LuxVolumetricQuality::Off,
        froxel_grid: LuxFroxelGridSettings::for_quality(LuxVolumetricQuality::Off),
        temporal: LuxVolumetricTemporalSettings::COLD_DEFAULT,
        global_fog: LuxGlobalFogSettings::COLD_DEFAULT,
        height_fog: LuxHeightFogSettings::COLD_DEFAULT,
        godrays: LuxGodraySettings::COLD_DEFAULT,
    };

    /// Typed predicate: is the pipeline active?
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.enabled && self.quality.is_active() && self.froxel_grid.is_active()
    }

    /// Typed predicate: rule #5 — temporal stability is
    /// configured for stable camera-motion behavior AND a
    /// typed reset is honored when needed.
    #[must_use]
    pub const fn temporal_is_camera_motion_stable(&self) -> bool {
        self.temporal.is_camera_motion_stable()
    }
}

// ============================================================================
// Section 11 — Typed renderer-side pass + resource taxonomy
// ============================================================================

/// Typed renderer-side volumetric pass role.  Matches the
/// user spec's nine listed passes verbatim.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxVolumetricPassRole {
    /// Clear the volumetric froxel textures.
    #[default]
    VolumetricFogClear,
    /// Inject the global / height fog density into the
    /// froxel grid.
    VolumetricFogInjectGlobal,
    /// Inject local fog volumes (sphere / box / capsule /
    /// SDF) into the froxel grid.
    VolumetricFogInjectLocalVolumes,
    /// Inject directional-light contribution into the
    /// scattering froxel.
    VolumetricLightInjectDirectional,
    /// Inject point / spot / area light contributions into
    /// the scattering froxel (clustered light list).
    VolumetricLightInjectLocal,
    /// Sample shadows into the scattering froxel.
    VolumetricShadowSample,
    /// Temporal reproject previous-frame history into the
    /// scattering froxel.
    VolumetricTemporalReproject,
    /// Integrate along the view ray into the typed
    /// integrated-fog target.
    VolumetricIntegrate,
    /// Composite the integrated fog into the HDR scene
    /// color target — MUST happen before bloom + tone
    /// mapping (Pass 8 rule #4).
    VolumetricCompositeIntoHdr,
}

impl LuxVolumetricPassRole {
    pub const ALL: [Self; FUN_LUX_VOLUMETRIC_PASS_ROLE_COUNT] = [
        Self::VolumetricFogClear,
        Self::VolumetricFogInjectGlobal,
        Self::VolumetricFogInjectLocalVolumes,
        Self::VolumetricLightInjectDirectional,
        Self::VolumetricLightInjectLocal,
        Self::VolumetricShadowSample,
        Self::VolumetricTemporalReproject,
        Self::VolumetricIntegrate,
        Self::VolumetricCompositeIntoHdr,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VolumetricFogClear => "volumetric_fog_clear",
            Self::VolumetricFogInjectGlobal => "volumetric_fog_inject_global",
            Self::VolumetricFogInjectLocalVolumes => "volumetric_fog_inject_local_volumes",
            Self::VolumetricLightInjectDirectional => "volumetric_light_inject_directional",
            Self::VolumetricLightInjectLocal => "volumetric_light_inject_local",
            Self::VolumetricShadowSample => "volumetric_shadow_sample",
            Self::VolumetricTemporalReproject => "volumetric_temporal_reproject",
            Self::VolumetricIntegrate => "volumetric_integrate",
            Self::VolumetricCompositeIntoHdr => "volumetric_composite_into_hdr",
        }
    }

    /// Typed render order key.  100..900 in 100-step
    /// increments so other lighting passes can interleave.
    #[must_use]
    pub const fn order_key(self) -> u16 {
        match self {
            Self::VolumetricFogClear => 100,
            Self::VolumetricFogInjectGlobal => 200,
            Self::VolumetricFogInjectLocalVolumes => 300,
            Self::VolumetricLightInjectDirectional => 400,
            Self::VolumetricLightInjectLocal => 500,
            Self::VolumetricShadowSample => 600,
            Self::VolumetricTemporalReproject => 700,
            Self::VolumetricIntegrate => 800,
            Self::VolumetricCompositeIntoHdr => 900,
        }
    }

    /// Typed predicate: rule #4 — the composite pass runs
    /// before bloom + tone mapping.  Encoded via the
    /// `order_key()` — composite is the last typed
    /// volumetric pass, and the typed HDR pipeline orders
    /// composite before the bloom + tonemap stages.
    #[must_use]
    pub const fn is_composite_into_hdr(self) -> bool {
        matches!(self, Self::VolumetricCompositeIntoHdr)
    }
}

/// Typed renderer-side volumetric resource intent.  Matches
/// the user spec's six listed resources.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxVolumetricResourceIntent {
    #[default]
    LuxVolumetricFroxelDensity,
    LuxVolumetricFroxelScattering,
    LuxVolumetricLightInjection,
    LuxVolumetricIntegratedFog,
    LuxVolumetricHistory,
    BlueNoiseTexture,
}

impl LuxVolumetricResourceIntent {
    pub const ALL: [Self; FUN_LUX_VOLUMETRIC_RESOURCE_INTENT_COUNT] = [
        Self::LuxVolumetricFroxelDensity,
        Self::LuxVolumetricFroxelScattering,
        Self::LuxVolumetricLightInjection,
        Self::LuxVolumetricIntegratedFog,
        Self::LuxVolumetricHistory,
        Self::BlueNoiseTexture,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LuxVolumetricFroxelDensity => "lux_volumetric_froxel_density",
            Self::LuxVolumetricFroxelScattering => "lux_volumetric_froxel_scattering",
            Self::LuxVolumetricLightInjection => "lux_volumetric_light_injection",
            Self::LuxVolumetricIntegratedFog => "lux_volumetric_integrated_fog",
            Self::LuxVolumetricHistory => "lux_volumetric_history",
            Self::BlueNoiseTexture => "blue_noise_texture",
        }
    }
}

// ============================================================================
// Section 12 — Typed debug view taxonomy (rule #6)
// ============================================================================

/// Typed volumetric debug view.  Matches the user spec's
/// four listed views verbatim.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxVolumetricDebugView {
    #[default]
    FroxelDensity,
    FroxelScattering,
    IntegratedFog,
    HistoryRejection,
}

impl LuxVolumetricDebugView {
    pub const ALL: [Self; FUN_LUX_VOLUMETRIC_DEBUG_VIEW_COUNT] = [
        Self::FroxelDensity,
        Self::FroxelScattering,
        Self::IntegratedFog,
        Self::HistoryRejection,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FroxelDensity => "froxel_density",
            Self::FroxelScattering => "froxel_scattering",
            Self::IntegratedFog => "integrated_fog",
            Self::HistoryRejection => "history_rejection",
        }
    }
}

/// Typed bundle of debug-view support flags.  Rule #6
/// holds when every typed view in the user spec is
/// supported by the renderer.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxVolumetricDebugViewSupport {
    pub schema_version: u16,
    pub froxel_density: bool,
    pub froxel_scattering: bool,
    pub integrated_fog: bool,
    pub history_rejection: bool,
}

impl LuxVolumetricDebugViewSupport {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_FOG_VOLUMETRIC_SCHEMA_VERSION,
        froxel_density: true,
        froxel_scattering: true,
        integrated_fog: true,
        history_rejection: true,
    };

    /// Typed predicate: rule #6.
    #[must_use]
    pub const fn supports_every_pass8_view(&self) -> bool {
        self.froxel_density
            && self.froxel_scattering
            && self.integrated_fog
            && self.history_rejection
    }
}

// ============================================================================
// Section 13 — Typed Pass 8 acceptance verdict
// ============================================================================

/// Typed Pass 8 routing-signal record.  The renderer fills
/// these typed flags each frame so the verdict can audit
/// the live pipeline against the user's six acceptance
/// criteria.  Pass 8 stays renderer-neutral by encoding
/// the signal shape here, not the actual signal source.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxVolumetricFrameSignals {
    pub schema_version: u16,
    pub directional_light_injection_ran: bool,
    pub local_light_injection_ran_via_clusters: bool,
    pub shadow_sample_pass_ran: bool,
    pub composite_runs_before_bloom_and_tonemap: bool,
    pub temporal_reset_reason: LuxVolumetricResetReason,
}

impl LuxVolumetricFrameSignals {
    /// Typed "renderer-not-wired" baseline.
    pub const NOT_EMITTED: Self = Self {
        schema_version: 0,
        directional_light_injection_ran: false,
        local_light_injection_ran_via_clusters: false,
        shadow_sample_pass_ran: false,
        composite_runs_before_bloom_and_tonemap: false,
        temporal_reset_reason: LuxVolumetricResetReason::NoReset,
    };

    /// Typed "fully-wired, every-rule-satisfied" baseline
    /// — used by tests + the typed product handshake.
    pub const FULLY_WIRED: Self = Self {
        schema_version: FUN_LUX_FOG_VOLUMETRIC_SCHEMA_VERSION,
        directional_light_injection_ran: true,
        local_light_injection_ran_via_clusters: true,
        shadow_sample_pass_ran: true,
        composite_runs_before_bloom_and_tonemap: true,
        temporal_reset_reason: LuxVolumetricResetReason::NoReset,
    };

    /// Typed predicate: have the signals been emitted at
    /// all?
    #[must_use]
    pub const fn is_emitted(&self) -> bool {
        self.schema_version != 0
    }
}

/// Typed Pass 8 acceptance verdict.  One flag per
/// user-listed acceptance criterion.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxVolumetricPass8AcceptanceVerdict {
    pub schema_version: u16,
    /// Rule #1: directional light produces visible
    /// volumetric shafts through fog.
    pub directional_shafts_render: bool,
    /// Rule #2: point + spot lights illuminate fog via the
    /// clustered light list.
    pub local_lights_illuminate_via_clusters: bool,
    /// Rule #3: shadows affect volumetric scattering.
    pub shadows_affect_scattering: bool,
    /// Rule #4: fog composites into HDR BEFORE bloom and
    /// tone mapping.
    pub composite_before_bloom_and_tonemap: bool,
    /// Rule #5: temporal history is stable during camera
    /// movement AND resets cleanly.
    pub temporal_stable_and_resets_cleanly: bool,
    /// Rule #6: debug views cover froxel density,
    /// scattering, integrated fog, and history rejection.
    pub debug_views_cover_pass8_set: bool,
}

impl LuxVolumetricPass8AcceptanceVerdict {
    /// Typed `evaluate(settings, signals, debug_support)`
    /// constructor — derives every typed verdict from the
    /// supplied handles.
    #[must_use]
    pub fn evaluate(
        settings: &LuxVolumetricSettings,
        signals: &LuxVolumetricFrameSignals,
        debug_support: &LuxVolumetricDebugViewSupport,
    ) -> Self {
        // Rule #5: temporal stability requires both the
        // typed config flags AND the typed reset signal
        // wiring (we audit BOTH halves).
        let temporal_stable = settings.temporal_is_camera_motion_stable() && signals.is_emitted();
        Self {
            schema_version: FUN_LUX_FOG_VOLUMETRIC_SCHEMA_VERSION,
            directional_shafts_render: signals.is_emitted()
                && signals.directional_light_injection_ran,
            local_lights_illuminate_via_clusters: signals.is_emitted()
                && signals.local_light_injection_ran_via_clusters,
            shadows_affect_scattering: signals.is_emitted() && signals.shadow_sample_pass_ran,
            composite_before_bloom_and_tonemap: signals.is_emitted()
                && signals.composite_runs_before_bloom_and_tonemap,
            temporal_stable_and_resets_cleanly: temporal_stable,
            debug_views_cover_pass8_set: debug_support.supports_every_pass8_view(),
        }
    }

    /// Typed count of satisfied rules (0..=6).
    #[must_use]
    pub const fn rules_satisfied(&self) -> u32 {
        let mut count = 0u32;
        if self.directional_shafts_render {
            count += 1;
        }
        if self.local_lights_illuminate_via_clusters {
            count += 1;
        }
        if self.shadows_affect_scattering {
            count += 1;
        }
        if self.composite_before_bloom_and_tonemap {
            count += 1;
        }
        if self.temporal_stable_and_resets_cleanly {
            count += 1;
        }
        if self.debug_views_cover_pass8_set {
            count += 1;
        }
        count
    }

    /// Typed bundle predicate.
    #[must_use]
    pub const fn obeys_all_six_rules(&self) -> bool {
        self.rules_satisfied() as usize == FUN_LUX_VOLUMETRIC_PASS8_ACCEPTANCE_RULE_COUNT
    }
}

// ============================================================================
// Tests — Pass 8 typed audits
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taxonomy_counts_are_dense() {
        assert_eq!(
            LuxVolumetricQuality::ALL.len(),
            FUN_LUX_VOLUMETRIC_QUALITY_COUNT
        );
        assert_eq!(
            LuxVolumetricResetReason::RESET_REASONS.len(),
            FUN_LUX_VOLUMETRIC_RESET_REASON_COUNT,
        );
        assert_eq!(LuxFogVolumeShape::ALL.len(), FUN_LUX_FOG_VOLUME_SHAPE_COUNT);
        assert_eq!(LuxFogBlendMode::ALL.len(), FUN_LUX_FOG_BLEND_MODE_COUNT);
        assert_eq!(
            LuxFogLifetimeFlag::ALL.len(),
            FUN_LUX_FOG_LIFETIME_FLAG_COUNT
        );
        assert_eq!(
            LuxVolumetricPassRole::ALL.len(),
            FUN_LUX_VOLUMETRIC_PASS_ROLE_COUNT,
        );
        assert_eq!(
            LuxVolumetricResourceIntent::ALL.len(),
            FUN_LUX_VOLUMETRIC_RESOURCE_INTENT_COUNT,
        );
        assert_eq!(
            LuxVolumetricDebugView::ALL.len(),
            FUN_LUX_VOLUMETRIC_DEBUG_VIEW_COUNT,
        );
    }

    /// Pass 8 user spec: froxel grid table matches.
    #[test]
    fn froxel_grid_for_quality_matches_user_spec() {
        let off = LuxFroxelGridSettings::for_quality(LuxVolumetricQuality::Off);
        assert_eq!(off.xy_divisor, 0);
        assert_eq!(off.z_slices, 0);
        assert!(!off.is_active());

        let low = LuxFroxelGridSettings::for_quality(LuxVolumetricQuality::Low);
        assert_eq!(low.xy_divisor, 8);
        assert_eq!(low.z_slices, 16);

        let medium = LuxFroxelGridSettings::for_quality(LuxVolumetricQuality::Medium);
        assert_eq!(medium.xy_divisor, 4);
        assert_eq!(medium.z_slices, 32);

        let high = LuxFroxelGridSettings::for_quality(LuxVolumetricQuality::High);
        assert_eq!(high.xy_divisor, 4);
        assert_eq!(high.z_slices, 64);

        let cine = LuxFroxelGridSettings::for_quality(LuxVolumetricQuality::Cinematic);
        assert_eq!(cine.xy_divisor, 2);
        assert_eq!(cine.z_slices, 64);
        assert!(cine.uses_high_quality_reconstruction);
    }

    #[test]
    fn froxel_cell_count_walks_viewport() {
        let high = LuxFroxelGridSettings::for_quality(LuxVolumetricQuality::High);
        // 1920×1080 viewport / 4 = 480×270 × 64 = 8_294_400 cells.
        assert_eq!(high.cell_count_for_viewport(1920, 1080), 480u64 * 270 * 64);
        // Off-grid returns 0 cells.
        let off = LuxFroxelGridSettings::for_quality(LuxVolumetricQuality::Off);
        assert_eq!(off.cell_count_for_viewport(1920, 1080), 0);
    }

    #[test]
    fn temporal_reset_reason_demands_reset_predicate() {
        assert!(!LuxVolumetricResetReason::NoReset.demands_history_reset());
        for r in LuxVolumetricResetReason::RESET_REASONS {
            assert!(r.demands_history_reset(), "{:?}", r);
        }
    }

    #[test]
    fn temporal_product_default_is_camera_motion_stable() {
        assert!(LuxVolumetricTemporalSettings::PRODUCT_DEFAULT.is_camera_motion_stable());
        assert!(!LuxVolumetricTemporalSettings::COLD_DEFAULT.is_camera_motion_stable());
    }

    #[test]
    fn fog_volume_shape_is_local_predicate() {
        assert!(!LuxFogVolumeShape::Global.is_local());
        for s in [
            LuxFogVolumeShape::Height,
            LuxFogVolumeShape::Sphere,
            LuxFogVolumeShape::Box,
            LuxFogVolumeShape::Capsule,
            LuxFogVolumeShape::SdfVolume,
        ] {
            assert!(s.is_local(), "{:?}", s);
        }
    }

    #[test]
    fn fog_lifetime_flag_cache_eligibility() {
        assert!(LuxFogLifetimeFlag::Static.is_cache_eligible());
        assert!(!LuxFogLifetimeFlag::Dynamic.is_cache_eligible());
    }

    #[test]
    fn fog_volume_parameters_product_defaults_carry_user_spec_fields() {
        let g = LuxFogVolumeParameters::PRODUCT_DEFAULT_GLOBAL;
        assert_eq!(g.shape, LuxFogVolumeShape::Global);
        assert!(g.density_q16 > 0);
        assert!(g.extinction_q16 > 0);
        assert_eq!(g.scattering_color, LuxQ16Color::WHITE);
        assert!(g.anisotropy_g_q8 > 0);
        assert_eq!(g.blend_mode, LuxFogBlendMode::Additive);
        assert_eq!(g.lifetime_flag, LuxFogLifetimeFlag::Static);

        let h = LuxFogVolumeParameters::PRODUCT_DEFAULT_HEIGHT;
        assert_eq!(h.shape, LuxFogVolumeShape::Height);
        assert!(h.height_falloff_q16 > 0);
    }

    #[test]
    fn light_override_active_predicate() {
        let mut o = LuxVolumetricLightOverride::PRODUCT_DEFAULT;
        assert!(o.is_active());
        o.volumetric_enabled = false;
        assert!(!o.is_active());
        let mut o2 = LuxVolumetricLightOverride::PRODUCT_DEFAULT;
        o2.volumetric_intensity_q16 = 0;
        assert!(!o2.is_active());
        assert!(!LuxVolumetricLightOverride::COLD_DEFAULT.is_active());
    }

    #[test]
    fn settings_product_default_is_active() {
        assert!(LuxVolumetricSettings::PRODUCT_DEFAULT.is_active());
        assert!(!LuxVolumetricSettings::COLD_DEFAULT.is_active());
    }

    /// Pass 8 rule #4 — composite into HDR is the last typed
    /// volumetric pass; its order key is the highest.
    #[test]
    fn composite_pass_runs_after_every_other_volumetric_pass() {
        let composite_key = LuxVolumetricPassRole::VolumetricCompositeIntoHdr.order_key();
        for role in LuxVolumetricPassRole::ALL {
            if matches!(role, LuxVolumetricPassRole::VolumetricCompositeIntoHdr) {
                continue;
            }
            assert!(
                role.order_key() < composite_key,
                "{:?} order key {} >= composite {}",
                role,
                role.order_key(),
                composite_key,
            );
        }
        assert!(LuxVolumetricPassRole::VolumetricCompositeIntoHdr.is_composite_into_hdr());
    }

    #[test]
    fn resource_intent_names_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for r in LuxVolumetricResourceIntent::ALL {
            assert!(seen.insert(r.as_str()), "duplicate: {}", r.as_str());
        }
    }

    #[test]
    fn debug_view_support_bundle_predicate() {
        assert!(LuxVolumetricDebugViewSupport::PRODUCT_DEFAULT.supports_every_pass8_view());
        let missing = LuxVolumetricDebugViewSupport {
            history_rejection: false,
            ..LuxVolumetricDebugViewSupport::PRODUCT_DEFAULT
        };
        assert!(!missing.supports_every_pass8_view());
    }

    /// Pass 8 acceptance verdict — every rule holds when
    /// settings + signals + debug support are fully wired.
    #[test]
    fn verdict_passes_when_everything_is_fully_wired() {
        let settings = LuxVolumetricSettings::PRODUCT_DEFAULT;
        let signals = LuxVolumetricFrameSignals::FULLY_WIRED;
        let debug = LuxVolumetricDebugViewSupport::PRODUCT_DEFAULT;
        let verdict = LuxVolumetricPass8AcceptanceVerdict::evaluate(&settings, &signals, &debug);
        assert!(verdict.obeys_all_six_rules(), "verdict: {:?}", verdict);
        assert_eq!(
            verdict.rules_satisfied(),
            FUN_LUX_VOLUMETRIC_PASS8_ACCEPTANCE_RULE_COUNT as u32,
        );
    }

    /// Verdict flips when ANY rule fails.
    #[test]
    fn verdict_flips_when_any_rule_fails() {
        let settings = LuxVolumetricSettings::PRODUCT_DEFAULT;
        let mut signals = LuxVolumetricFrameSignals::FULLY_WIRED;
        signals.composite_runs_before_bloom_and_tonemap = false;
        let debug = LuxVolumetricDebugViewSupport::PRODUCT_DEFAULT;
        let verdict = LuxVolumetricPass8AcceptanceVerdict::evaluate(&settings, &signals, &debug);
        assert!(!verdict.obeys_all_six_rules());
        assert!(!verdict.composite_before_bloom_and_tonemap);
        assert_eq!(verdict.rules_satisfied(), 5);
    }

    /// Verdict — NOT_EMITTED signals fail every signal-gated
    /// rule but the debug-views rule still passes.
    #[test]
    fn verdict_fails_signal_gated_rules_when_signals_not_emitted() {
        let settings = LuxVolumetricSettings::PRODUCT_DEFAULT;
        let signals = LuxVolumetricFrameSignals::NOT_EMITTED;
        let debug = LuxVolumetricDebugViewSupport::PRODUCT_DEFAULT;
        let verdict = LuxVolumetricPass8AcceptanceVerdict::evaluate(&settings, &signals, &debug);
        assert!(!verdict.directional_shafts_render);
        assert!(!verdict.local_lights_illuminate_via_clusters);
        assert!(!verdict.shadows_affect_scattering);
        assert!(!verdict.composite_before_bloom_and_tonemap);
        assert!(!verdict.temporal_stable_and_resets_cleanly);
        assert!(verdict.debug_views_cover_pass8_set);
        assert_eq!(verdict.rules_satisfied(), 1);
    }

    /// Verdict — cold-default settings fail the temporal
    /// rule even when signals are fully wired.
    #[test]
    fn verdict_temporal_rule_keys_off_settings_too() {
        let settings = LuxVolumetricSettings::COLD_DEFAULT;
        let signals = LuxVolumetricFrameSignals::FULLY_WIRED;
        let debug = LuxVolumetricDebugViewSupport::PRODUCT_DEFAULT;
        let verdict = LuxVolumetricPass8AcceptanceVerdict::evaluate(&settings, &signals, &debug);
        assert!(!verdict.temporal_stable_and_resets_cleanly);
    }

    #[test]
    fn fog_volume_aliases_compile() {
        // `LuxGlobalFog`, `LuxHeightFog`, `LuxLocalFogVolume`
        // are typed aliases for `LuxFogVolume`.  Round-trip a
        // value through each alias to confirm.
        let v = LuxFogVolume {
            schema_version: FUN_LUX_FOG_VOLUMETRIC_SCHEMA_VERSION,
            stable_id: "demo.global",
            parameters: LuxFogVolumeParameters::PRODUCT_DEFAULT_GLOBAL,
        };
        let g: LuxGlobalFog = v;
        let h: LuxHeightFog = LuxFogVolume {
            stable_id: "demo.height",
            parameters: LuxFogVolumeParameters::PRODUCT_DEFAULT_HEIGHT,
            ..v
        };
        let l: LuxLocalFogVolume = LuxFogVolume {
            stable_id: "demo.local",
            parameters: LuxFogVolumeParameters {
                shape: LuxFogVolumeShape::Sphere,
                ..LuxFogVolumeParameters::PRODUCT_DEFAULT_GLOBAL
            },
            ..v
        };
        assert_eq!(g.parameters.shape, LuxFogVolumeShape::Global);
        assert_eq!(h.parameters.shape, LuxFogVolumeShape::Height);
        assert_eq!(l.parameters.shape, LuxFogVolumeShape::Sphere);
    }
}
