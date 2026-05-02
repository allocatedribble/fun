use bevy::{
    prelude::{Resource, Vec3},
    render::extract_resource::ExtractResource,
};

use super::{config::FunCloudQuality, profiles::builtin_weather_profile};

pub const MAX_WEATHER_PATTERN_PHASES: usize = 32;
pub const MIN_CLOUD_DENSITY: f32 = 0.0;
pub const MAX_CLOUD_DENSITY: f32 = 4.0;
pub const MIN_WEATHER_MAP_SCALE_METERS: f32 = 1_000.0;
pub const MAX_WEATHER_MAP_SCALE_METERS: f32 = 512_000.0;
pub const MAX_CLOUD_ALTITUDE_METERS: f32 = 24_000.0;
pub const MAX_WIND_SPEED_MPS: f32 = 180.0;
pub const MAX_WIND_SHEAR_MPS: f32 = 120.0;
pub const MAX_TRANSITION_SECONDS: f32 = 3_600.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunWeatherProfileId {
    Clear,
    Scattered,
    Overcast,
    StormFront,
    CinematicSunset,
    Custom,
}

impl FunWeatherProfileId {
    pub fn parse(value: &str) -> Option<Self> {
        if value.eq_ignore_ascii_case("clear") {
            return Some(Self::Clear);
        }
        if value.eq_ignore_ascii_case("scattered")
            || value.eq_ignore_ascii_case("sparse")
            || value.eq_ignore_ascii_case("default")
        {
            return Some(Self::Scattered);
        }
        if value.eq_ignore_ascii_case("overcast") || value.eq_ignore_ascii_case("cloudy") {
            return Some(Self::Overcast);
        }
        if value.eq_ignore_ascii_case("storm_front")
            || value.eq_ignore_ascii_case("storm-front")
            || value.eq_ignore_ascii_case("storm")
        {
            return Some(Self::StormFront);
        }
        if value.eq_ignore_ascii_case("cinematic_sunset")
            || value.eq_ignore_ascii_case("cinematic-sunset")
            || value.eq_ignore_ascii_case("sunset")
        {
            return Some(Self::CinematicSunset);
        }
        if value.eq_ignore_ascii_case("custom") {
            return Some(Self::Custom);
        }
        None
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::Scattered => "scattered",
            Self::Overcast => "overcast",
            Self::StormFront => "storm_front",
            Self::CinematicSunset => "cinematic_sunset",
            Self::Custom => "custom",
        }
    }

    pub const fn is_builtin(self) -> bool {
        !matches!(self, Self::Custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FunCloudTypeMix {
    pub cirrus: f32,
    pub cumulus: f32,
    pub stratocumulus: f32,
    pub cumulonimbus: f32,
}

impl FunCloudTypeMix {
    pub const fn new(cirrus: f32, cumulus: f32, stratocumulus: f32, cumulonimbus: f32) -> Self {
        Self {
            cirrus,
            cumulus,
            stratocumulus,
            cumulonimbus,
        }
    }

    fn validated(self) -> Result<Self, FunWeatherValidationError> {
        Ok(Self {
            cirrus: clamp_unit("cloud_type_mix.cirrus", self.cirrus)?,
            cumulus: clamp_unit("cloud_type_mix.cumulus", self.cumulus)?,
            stratocumulus: clamp_unit("cloud_type_mix.stratocumulus", self.stratocumulus)?,
            cumulonimbus: clamp_unit("cloud_type_mix.cumulonimbus", self.cumulonimbus)?,
        })
    }

    fn lerp(self, other: Self, alpha: f32) -> Self {
        Self {
            cirrus: lerp_f32(self.cirrus, other.cirrus, alpha),
            cumulus: lerp_f32(self.cumulus, other.cumulus, alpha),
            stratocumulus: lerp_f32(self.stratocumulus, other.stratocumulus, alpha),
            cumulonimbus: lerp_f32(self.cumulonimbus, other.cumulonimbus, alpha),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FunWeatherProfile {
    pub id: FunWeatherProfileId,
    pub display_name: &'static str,
    pub sky_zenith_rgb: [f32; 3],
    pub sky_horizon_rgb: [f32; 3],
    pub sun_direction_override: Option<Vec3>,
    pub sun_illuminance_lux: f32,
    pub ambient_rgb: [f32; 3],
    pub cloud_coverage: f32,
    pub cloud_density: f32,
    pub cloud_base_meters: f32,
    pub cloud_top_meters: f32,
    pub cloud_type_mix: FunCloudTypeMix,
    pub anvil_strength: f32,
    pub edge_softness: f32,
    pub detail_strength: f32,
    pub precipitation_probability: f32,
    pub storm_intensity: f32,
    pub wind_direction: Vec3,
    pub wind_speed_mps: f32,
    pub wind_shear_mps: f32,
    pub turbulence_strength: f32,
    pub weather_map_scale_meters: f32,
    pub transition_seconds: f32,
    pub render_quality_hint: FunCloudQuality,
}

impl FunWeatherProfile {
    pub fn validated(mut self) -> Result<Self, FunWeatherValidationError> {
        self.sky_zenith_rgb = validated_rgb("sky_zenith_rgb", self.sky_zenith_rgb)?;
        self.sky_horizon_rgb = validated_rgb("sky_horizon_rgb", self.sky_horizon_rgb)?;
        self.ambient_rgb = validated_rgb("ambient_rgb", self.ambient_rgb)?;
        if let Some(sun_direction) = self.sun_direction_override {
            self.sun_direction_override = Some(normalize_or_reject(
                "sun_direction_override",
                sun_direction,
            )?);
        }
        self.sun_illuminance_lux = clamp_range(
            "sun_illuminance_lux",
            self.sun_illuminance_lux,
            0.0,
            150_000.0,
        )?;
        self.cloud_coverage = clamp_unit("cloud_coverage", self.cloud_coverage)?;
        self.cloud_density = clamp_range(
            "cloud_density",
            self.cloud_density,
            MIN_CLOUD_DENSITY,
            MAX_CLOUD_DENSITY,
        )?;
        self.cloud_base_meters = clamp_range(
            "cloud_base_meters",
            self.cloud_base_meters,
            0.0,
            MAX_CLOUD_ALTITUDE_METERS,
        )?;
        self.cloud_top_meters = clamp_range(
            "cloud_top_meters",
            self.cloud_top_meters,
            0.0,
            MAX_CLOUD_ALTITUDE_METERS,
        )?;
        if self.cloud_top_meters <= self.cloud_base_meters {
            return Err(FunWeatherValidationError::InvertedCloudAltitudeRange);
        }
        self.cloud_type_mix = self.cloud_type_mix.validated()?;
        self.anvil_strength = clamp_unit("anvil_strength", self.anvil_strength)?;
        self.edge_softness = clamp_unit("edge_softness", self.edge_softness)?;
        self.detail_strength = clamp_unit("detail_strength", self.detail_strength)?;
        self.precipitation_probability =
            clamp_unit("precipitation_probability", self.precipitation_probability)?;
        self.storm_intensity = clamp_unit("storm_intensity", self.storm_intensity)?;
        self.wind_direction = normalize_or_default("wind_direction", self.wind_direction, Vec3::X)?;
        self.wind_speed_mps = clamp_range(
            "wind_speed_mps",
            self.wind_speed_mps,
            0.0,
            MAX_WIND_SPEED_MPS,
        )?;
        self.wind_shear_mps = clamp_range(
            "wind_shear_mps",
            self.wind_shear_mps,
            0.0,
            MAX_WIND_SHEAR_MPS,
        )?;
        self.turbulence_strength = clamp_unit("turbulence_strength", self.turbulence_strength)?;
        self.weather_map_scale_meters = clamp_range(
            "weather_map_scale_meters",
            self.weather_map_scale_meters,
            MIN_WEATHER_MAP_SCALE_METERS,
            MAX_WEATHER_MAP_SCALE_METERS,
        )?;
        self.transition_seconds = clamp_range(
            "transition_seconds",
            self.transition_seconds,
            f32::EPSILON,
            MAX_TRANSITION_SECONDS,
        )?;
        Ok(self)
    }

    pub fn builtin(id: FunWeatherProfileId) -> Option<Self> {
        builtin_weather_profile(id)
    }

    pub fn blend(from: Self, to: Self, alpha: f32) -> Result<Self, FunWeatherValidationError> {
        let alpha = clamp_unit("blend_alpha", alpha)?;
        let sun_direction_override = match (from.sun_direction_override, to.sun_direction_override)
        {
            (Some(a), Some(b)) => Some(normalize_or_default(
                "sun_direction_override",
                a.lerp(b, alpha),
                b,
            )?),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };

        Self {
            id: to.id,
            display_name: to.display_name,
            sky_zenith_rgb: lerp_rgb(from.sky_zenith_rgb, to.sky_zenith_rgb, alpha),
            sky_horizon_rgb: lerp_rgb(from.sky_horizon_rgb, to.sky_horizon_rgb, alpha),
            sun_direction_override,
            sun_illuminance_lux: lerp_f32(from.sun_illuminance_lux, to.sun_illuminance_lux, alpha),
            ambient_rgb: lerp_rgb(from.ambient_rgb, to.ambient_rgb, alpha),
            cloud_coverage: lerp_f32(from.cloud_coverage, to.cloud_coverage, alpha),
            cloud_density: lerp_f32(from.cloud_density, to.cloud_density, alpha),
            cloud_base_meters: lerp_f32(from.cloud_base_meters, to.cloud_base_meters, alpha),
            cloud_top_meters: lerp_f32(from.cloud_top_meters, to.cloud_top_meters, alpha),
            cloud_type_mix: from.cloud_type_mix.lerp(to.cloud_type_mix, alpha),
            anvil_strength: lerp_f32(from.anvil_strength, to.anvil_strength, alpha),
            edge_softness: lerp_f32(from.edge_softness, to.edge_softness, alpha),
            detail_strength: lerp_f32(from.detail_strength, to.detail_strength, alpha),
            precipitation_probability: lerp_f32(
                from.precipitation_probability,
                to.precipitation_probability,
                alpha,
            ),
            storm_intensity: lerp_f32(from.storm_intensity, to.storm_intensity, alpha),
            wind_direction: normalize_or_default(
                "wind_direction",
                from.wind_direction.lerp(to.wind_direction, alpha),
                to.wind_direction,
            )?,
            wind_speed_mps: lerp_f32(from.wind_speed_mps, to.wind_speed_mps, alpha),
            wind_shear_mps: lerp_f32(from.wind_shear_mps, to.wind_shear_mps, alpha),
            turbulence_strength: lerp_f32(from.turbulence_strength, to.turbulence_strength, alpha),
            weather_map_scale_meters: lerp_f32(
                from.weather_map_scale_meters,
                to.weather_map_scale_meters,
                alpha,
            ),
            transition_seconds: lerp_f32(from.transition_seconds, to.transition_seconds, alpha),
            render_quality_hint: if alpha < 0.5 {
                from.render_quality_hint
            } else {
                to.render_quality_hint
            },
        }
        .validated()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunWeatherTransitionCurve {
    Linear,
    SmoothStep,
    EaseInOutCubic,
}

impl FunWeatherTransitionCurve {
    pub const fn apply(self, alpha: f32) -> f32 {
        let alpha = if alpha < 0.0 {
            0.0
        } else if alpha > 1.0 {
            1.0
        } else {
            alpha
        };
        match self {
            Self::Linear => alpha,
            Self::SmoothStep => alpha * alpha * (3.0 - 2.0 * alpha),
            Self::EaseInOutCubic => {
                if alpha < 0.5 {
                    4.0 * alpha * alpha * alpha
                } else {
                    1.0 - (-2.0 * alpha + 2.0) * (-2.0 * alpha + 2.0) * (-2.0 * alpha + 2.0) / 2.0
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FunWeatherTransition {
    pub from_profile_id: FunWeatherProfileId,
    pub to_profile_id: FunWeatherProfileId,
    pub duration_seconds: f32,
    pub elapsed_seconds: f32,
    pub curve: FunWeatherTransitionCurve,
    pub storm_cell_seed: Option<u64>,
    pub wind_override: Option<Vec3>,
}

impl FunWeatherTransition {
    pub fn state(self, pattern_seed: u64) -> Result<FunWeatherState, FunWeatherValidationError> {
        if !self.duration_seconds.is_finite() || self.duration_seconds <= 0.0 {
            return Err(FunWeatherValidationError::InvalidTransitionSeconds);
        }
        if !self.elapsed_seconds.is_finite() || self.elapsed_seconds < 0.0 {
            return Err(FunWeatherValidationError::InvalidTransitionSeconds);
        }

        let from = profile_for_pattern(self.from_profile_id)?;
        let mut to = profile_for_pattern(self.to_profile_id)?;
        if let Some(wind_override) = self.wind_override {
            to.wind_direction =
                normalize_or_default("wind_override", wind_override, to.wind_direction)?;
        }
        let raw_alpha = (self.elapsed_seconds / self.duration_seconds).clamp(0.0, 1.0);
        let blend_alpha = self.curve.apply(raw_alpha);
        let profile = FunWeatherProfile::blend(from, to, blend_alpha)?;

        Ok(FunWeatherState {
            profile,
            from_profile_id: self.from_profile_id,
            to_profile_id: self.to_profile_id,
            blend_alpha,
            weather_seed: deterministic_weather_seed(pattern_seed, self.storm_cell_seed, 0),
            elapsed_seconds: self.elapsed_seconds,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FunWeatherPatternPhase {
    pub profile_id: FunWeatherProfileId,
    pub duration_seconds: f32,
    pub transition_in_seconds: f32,
    pub transition_curve: FunWeatherTransitionCurve,
    pub storm_cell_seed: Option<u64>,
    pub wind_override: Option<Vec3>,
}

impl FunWeatherPatternPhase {
    fn validated(self) -> Result<Self, FunWeatherValidationError> {
        if !self.profile_id.is_builtin() {
            return Err(FunWeatherValidationError::UnknownBuiltinProfile);
        }
        validate_positive_seconds("duration_seconds", self.duration_seconds)?;
        if !self.transition_in_seconds.is_finite() || self.transition_in_seconds < 0.0 {
            return Err(FunWeatherValidationError::InvalidTransitionSeconds);
        }
        if let Some(wind_override) = self.wind_override {
            normalize_or_reject("wind_override", wind_override)?;
        }
        Ok(Self {
            transition_in_seconds: self.transition_in_seconds.min(self.duration_seconds),
            ..self
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunWeatherPattern {
    pub seed: u64,
    pub phases: Vec<FunWeatherPatternPhase>,
}

impl FunWeatherPattern {
    pub fn new(
        seed: u64,
        phases: Vec<FunWeatherPatternPhase>,
    ) -> Result<Self, FunWeatherValidationError> {
        if phases.is_empty() {
            return Err(FunWeatherValidationError::EmptyPattern);
        }
        if phases.len() > MAX_WEATHER_PATTERN_PHASES {
            return Err(FunWeatherValidationError::TooManyPatternPhases);
        }

        let mut validated_phases = Vec::with_capacity(phases.len());
        for phase in phases {
            validated_phases.push(phase.validated()?);
        }

        Ok(Self {
            seed,
            phases: validated_phases,
        })
    }

    pub fn state_at_seconds(
        &self,
        elapsed_seconds: f32,
    ) -> Result<FunWeatherState, FunWeatherValidationError> {
        if !elapsed_seconds.is_finite() || elapsed_seconds < 0.0 {
            return Err(FunWeatherValidationError::InvalidPatternDuration);
        }

        let total_seconds = self
            .phases
            .iter()
            .map(|phase| phase.duration_seconds)
            .sum::<f32>();
        validate_positive_seconds("pattern_total_seconds", total_seconds)?;

        let mut local_seconds = elapsed_seconds % total_seconds;
        for (phase_index, phase) in self.phases.iter().enumerate() {
            if local_seconds > phase.duration_seconds {
                local_seconds -= phase.duration_seconds;
                continue;
            }

            let previous_phase = if phase_index == 0 {
                &self.phases[self.phases.len() - 1]
            } else {
                &self.phases[phase_index - 1]
            };

            let weather_seed =
                deterministic_weather_seed(self.seed, phase.storm_cell_seed, phase_index as u64);

            if phase.transition_in_seconds > 0.0 && local_seconds < phase.transition_in_seconds {
                let transition = FunWeatherTransition {
                    from_profile_id: previous_phase.profile_id,
                    to_profile_id: phase.profile_id,
                    duration_seconds: phase.transition_in_seconds,
                    elapsed_seconds: local_seconds,
                    curve: phase.transition_curve,
                    storm_cell_seed: phase.storm_cell_seed,
                    wind_override: phase.wind_override,
                };
                let mut state = transition.state(self.seed)?;
                state.weather_seed = weather_seed;
                state.elapsed_seconds = elapsed_seconds;
                return Ok(state);
            }

            let mut profile = profile_for_pattern(phase.profile_id)?;
            if let Some(wind_override) = phase.wind_override {
                profile.wind_direction =
                    normalize_or_default("wind_override", wind_override, profile.wind_direction)?;
            }
            return Ok(FunWeatherState {
                profile,
                from_profile_id: phase.profile_id,
                to_profile_id: phase.profile_id,
                blend_alpha: 1.0,
                weather_seed,
                elapsed_seconds,
            });
        }

        Err(FunWeatherValidationError::InvalidPatternDuration)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Resource)]
pub struct FunWeatherState {
    pub profile: FunWeatherProfile,
    pub from_profile_id: FunWeatherProfileId,
    pub to_profile_id: FunWeatherProfileId,
    pub blend_alpha: f32,
    pub weather_seed: u64,
    pub elapsed_seconds: f32,
}

impl FunWeatherState {
    pub fn from_profile_id(
        profile_id: FunWeatherProfileId,
    ) -> Result<Self, FunWeatherValidationError> {
        let profile = profile_for_pattern(profile_id)?;
        Ok(Self {
            profile,
            from_profile_id: profile_id,
            to_profile_id: profile_id,
            blend_alpha: 1.0,
            weather_seed: deterministic_weather_seed(0, None, profile_id as u64),
            elapsed_seconds: 0.0,
        })
    }
}

impl ExtractResource for FunWeatherState {
    type Source = FunWeatherState;

    fn extract_resource(source: &Self::Source) -> Self {
        *source
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunWeatherValidationError {
    EmptyPattern,
    TooManyPatternPhases,
    UnknownBuiltinProfile,
    NonFiniteField(&'static str),
    ZeroLengthVector(&'static str),
    InvertedCloudAltitudeRange,
    NonPositiveWeatherMapScale,
    InvalidPatternDuration,
    InvalidTransitionSeconds,
}

fn profile_for_pattern(
    profile_id: FunWeatherProfileId,
) -> Result<FunWeatherProfile, FunWeatherValidationError> {
    FunWeatherProfile::builtin(profile_id).ok_or(FunWeatherValidationError::UnknownBuiltinProfile)
}

fn validated_rgb(
    name: &'static str,
    value: [f32; 3],
) -> Result<[f32; 3], FunWeatherValidationError> {
    Ok([
        clamp_range(name, value[0], 0.0, 16.0)?,
        clamp_range(name, value[1], 0.0, 16.0)?,
        clamp_range(name, value[2], 0.0, 16.0)?,
    ])
}

fn clamp_unit(name: &'static str, value: f32) -> Result<f32, FunWeatherValidationError> {
    clamp_range(name, value, 0.0, 1.0)
}

fn clamp_range(
    name: &'static str,
    value: f32,
    min: f32,
    max: f32,
) -> Result<f32, FunWeatherValidationError> {
    if !value.is_finite() {
        return Err(FunWeatherValidationError::NonFiniteField(name));
    }
    if name == "weather_map_scale_meters" && value <= 0.0 {
        return Err(FunWeatherValidationError::NonPositiveWeatherMapScale);
    }
    Ok(value.clamp(min, max))
}

fn validate_positive_seconds(
    _name: &'static str,
    value: f32,
) -> Result<(), FunWeatherValidationError> {
    if !value.is_finite() || value <= 0.0 {
        return Err(FunWeatherValidationError::InvalidPatternDuration);
    }
    Ok(())
}

fn normalize_or_reject(name: &'static str, value: Vec3) -> Result<Vec3, FunWeatherValidationError> {
    if !value.is_finite() {
        return Err(FunWeatherValidationError::NonFiniteField(name));
    }
    if value.length_squared() <= f32::EPSILON {
        return Err(FunWeatherValidationError::ZeroLengthVector(name));
    }
    Ok(value.normalize())
}

fn normalize_or_default(
    name: &'static str,
    value: Vec3,
    default_value: Vec3,
) -> Result<Vec3, FunWeatherValidationError> {
    if !value.is_finite() {
        return Err(FunWeatherValidationError::NonFiniteField(name));
    }
    if value.length_squared() <= f32::EPSILON {
        return normalize_or_reject(name, default_value);
    }
    Ok(value.normalize())
}

fn lerp_rgb(from: [f32; 3], to: [f32; 3], alpha: f32) -> [f32; 3] {
    [
        lerp_f32(from[0], to[0], alpha),
        lerp_f32(from[1], to[1], alpha),
        lerp_f32(from[2], to[2], alpha),
    ]
}

fn lerp_f32(from: f32, to: f32, alpha: f32) -> f32 {
    from + (to - from) * alpha
}

fn deterministic_weather_seed(pattern_seed: u64, storm_seed: Option<u64>, phase_index: u64) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    hash = fnv1a_u64(hash, pattern_seed);
    hash = fnv1a_u64(hash, storm_seed.unwrap_or(0));
    fnv1a_u64(hash, phase_index)
}

fn fnv1a_u64(mut hash: u64, value: u64) -> u64 {
    for byte in value.to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_validation_clamps_coverage_and_density() {
        let mut profile = builtin_weather_profile(FunWeatherProfileId::Scattered).unwrap();
        profile.cloud_coverage = 2.0;
        profile.cloud_density = 99.0;

        let validated = profile.validated().expect("profile should clamp");

        assert_eq!(validated.cloud_coverage, 1.0);
        assert_eq!(validated.cloud_density, MAX_CLOUD_DENSITY);
    }

    #[test]
    fn profile_validation_rejects_inverted_cloud_altitudes() {
        let mut profile = builtin_weather_profile(FunWeatherProfileId::Scattered).unwrap();
        profile.cloud_base_meters = 8_000.0;
        profile.cloud_top_meters = 2_000.0;

        assert_eq!(
            profile.validated(),
            Err(FunWeatherValidationError::InvertedCloudAltitudeRange)
        );
    }

    #[test]
    fn profile_validation_rejects_zero_weather_map_scale() {
        let mut profile = builtin_weather_profile(FunWeatherProfileId::Scattered).unwrap();
        profile.weather_map_scale_meters = 0.0;

        assert_eq!(
            profile.validated(),
            Err(FunWeatherValidationError::NonPositiveWeatherMapScale)
        );
    }

    #[test]
    fn pattern_rejects_invalid_duration() {
        let phase = FunWeatherPatternPhase {
            profile_id: FunWeatherProfileId::Scattered,
            duration_seconds: 0.0,
            transition_in_seconds: 0.0,
            transition_curve: FunWeatherTransitionCurve::Linear,
            storm_cell_seed: None,
            wind_override: None,
        };

        assert_eq!(
            FunWeatherPattern::new(1, vec![phase]),
            Err(FunWeatherValidationError::InvalidPatternDuration)
        );
    }

    #[test]
    fn pattern_rejects_oversized_phase_list() {
        let phase = FunWeatherPatternPhase {
            profile_id: FunWeatherProfileId::Scattered,
            duration_seconds: 60.0,
            transition_in_seconds: 5.0,
            transition_curve: FunWeatherTransitionCurve::SmoothStep,
            storm_cell_seed: None,
            wind_override: None,
        };
        let phases = vec![phase; MAX_WEATHER_PATTERN_PHASES + 1];

        assert_eq!(
            FunWeatherPattern::new(1, phases),
            Err(FunWeatherValidationError::TooManyPatternPhases)
        );
    }

    #[test]
    fn pattern_rejects_custom_profile_until_registry_supplies_it() {
        let phase = FunWeatherPatternPhase {
            profile_id: FunWeatherProfileId::Custom,
            duration_seconds: 60.0,
            transition_in_seconds: 5.0,
            transition_curve: FunWeatherTransitionCurve::SmoothStep,
            storm_cell_seed: None,
            wind_override: None,
        };

        assert_eq!(
            FunWeatherPattern::new(1, vec![phase]),
            Err(FunWeatherValidationError::UnknownBuiltinProfile)
        );
    }

    #[test]
    fn profile_blending_is_deterministic() {
        let phases = vec![
            FunWeatherPatternPhase {
                profile_id: FunWeatherProfileId::Scattered,
                duration_seconds: 300.0,
                transition_in_seconds: 30.0,
                transition_curve: FunWeatherTransitionCurve::SmoothStep,
                storm_cell_seed: Some(7),
                wind_override: None,
            },
            FunWeatherPatternPhase {
                profile_id: FunWeatherProfileId::Overcast,
                duration_seconds: 300.0,
                transition_in_seconds: 45.0,
                transition_curve: FunWeatherTransitionCurve::EaseInOutCubic,
                storm_cell_seed: Some(11),
                wind_override: Some(Vec3::new(0.7, 0.0, 0.2)),
            },
        ];
        let pattern = FunWeatherPattern::new(99, phases).expect("pattern should be valid");

        let a = pattern.state_at_seconds(315.0).expect("state should exist");
        let b = pattern
            .state_at_seconds(315.0)
            .expect("state should repeat");

        assert_eq!(a, b);
        assert!(a.blend_alpha > 0.0 && a.blend_alpha < 1.0);
    }
}
