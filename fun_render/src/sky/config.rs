use std::ffi::OsString;

use bevy::{
    prelude::{Resource, UVec2},
    render::extract_resource::ExtractResource,
};
use tracing::warn;

use super::weather::FunWeatherProfileId;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum FunCloudQuality {
    Off,
    Cheap,
    #[default]
    Balanced,
    Cinematic,
}

impl FunCloudQuality {
    pub fn parse(value: &str) -> Option<Self> {
        if value.eq_ignore_ascii_case("off")
            || value.eq_ignore_ascii_case("none")
            || value.eq_ignore_ascii_case("disabled")
        {
            return Some(Self::Off);
        }
        if value.eq_ignore_ascii_case("cheap")
            || value.eq_ignore_ascii_case("low")
            || value.eq_ignore_ascii_case("competitive")
        {
            return Some(Self::Cheap);
        }
        if value.eq_ignore_ascii_case("balanced")
            || value.eq_ignore_ascii_case("default")
            || value.eq_ignore_ascii_case("normal")
        {
            return Some(Self::Balanced);
        }
        if value.eq_ignore_ascii_case("cinematic")
            || value.eq_ignore_ascii_case("quality")
            || value.eq_ignore_ascii_case("high")
        {
            return Some(Self::Cinematic);
        }
        None
    }

    pub const fn as_env_value(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Cheap => "cheap",
            Self::Balanced => "balanced",
            Self::Cinematic => "cinematic",
        }
    }

    pub const fn primary_step_count(self) -> u32 {
        match self {
            Self::Off => 0,
            Self::Cheap => 10,
            Self::Balanced => 20,
            Self::Cinematic => 40,
        }
    }

    pub const fn light_step_count(self) -> u32 {
        match self {
            Self::Off => 0,
            Self::Cheap => 2,
            Self::Balanced => 4,
            Self::Cinematic => 8,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum FunCloudInternalScale {
    Full,
    ThreeQuarter,
    #[default]
    Half,
    Third,
}

impl FunCloudInternalScale {
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "1" | "1.0" | "full" | "native" => Some(Self::Full),
            "0.75" | ".75" | "75" | "3/4" | "three-quarter" | "three_quarter" => {
                Some(Self::ThreeQuarter)
            }
            "0.5" | ".5" | "50" | "1/2" | "half" => Some(Self::Half),
            "0.33" | "0.333" | ".33" | ".333" | "33" | "1/3" | "third" => Some(Self::Third),
            _ => None,
        }
    }

    pub const fn as_env_value(self) -> &'static str {
        match self {
            Self::Full => "1.0",
            Self::ThreeQuarter => "0.75",
            Self::Half => "0.5",
            Self::Third => "0.33",
        }
    }

    pub const fn numerator_denominator(self) -> (u32, u32) {
        match self {
            Self::Full => (1, 1),
            Self::ThreeQuarter => (3, 4),
            Self::Half => (1, 2),
            Self::Third => (1, 3),
        }
    }

    pub fn scale_extent(self, extent: u32) -> u32 {
        let (numerator, denominator) = self.numerator_denominator();
        ((extent.saturating_mul(numerator)).saturating_add(denominator - 1) / denominator).max(1)
    }

    pub fn scale_size(self, size: UVec2) -> UVec2 {
        UVec2::new(self.scale_extent(size.x), self.scale_extent(size.y))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum FunCloudDebugOverlay {
    #[default]
    None,
    Coverage,
    Density,
    Steps,
    History,
    Weather,
}

impl FunCloudDebugOverlay {
    pub fn parse(value: &str) -> Option<Self> {
        if value.eq_ignore_ascii_case("none")
            || value.eq_ignore_ascii_case("off")
            || value.eq_ignore_ascii_case("disabled")
        {
            return Some(Self::None);
        }
        if value.eq_ignore_ascii_case("coverage") || value.eq_ignore_ascii_case("weather_map") {
            return Some(Self::Coverage);
        }
        if value.eq_ignore_ascii_case("density") || value.eq_ignore_ascii_case("density_field") {
            return Some(Self::Density);
        }
        if value.eq_ignore_ascii_case("steps") || value.eq_ignore_ascii_case("step_count") {
            return Some(Self::Steps);
        }
        if value.eq_ignore_ascii_case("history")
            || value.eq_ignore_ascii_case("temporal")
            || value.eq_ignore_ascii_case("reprojection")
        {
            return Some(Self::History);
        }
        if value.eq_ignore_ascii_case("weather") || value.eq_ignore_ascii_case("profile") {
            return Some(Self::Weather);
        }
        None
    }

    pub const fn as_env_value(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Coverage => "coverage",
            Self::Density => "density",
            Self::Steps => "steps",
            Self::History => "history",
            Self::Weather => "weather",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct FunCloudSettings {
    pub enabled: bool,
    pub quality: FunCloudQuality,
    pub internal_scale: FunCloudInternalScale,
    pub temporal_enabled: bool,
    pub shadows_enabled: bool,
    pub profile_id: FunWeatherProfileId,
    pub debug_overlay: FunCloudDebugOverlay,
}

impl FunCloudSettings {
    pub fn from_env() -> Self {
        Self::from_env_reader(std::env::var_os, cfg!(feature = "volumetric_clouds"))
    }

    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            quality: FunCloudQuality::Off,
            internal_scale: FunCloudInternalScale::Half,
            temporal_enabled: false,
            shadows_enabled: false,
            profile_id: FunWeatherProfileId::Scattered,
            debug_overlay: FunCloudDebugOverlay::None,
        }
    }

    pub const fn shader_profile_flags(self) -> &'static str {
        match (self.enabled, self.temporal_enabled, self.shadows_enabled) {
            (false, _, _) => "clouds:off",
            (true, false, false) => "clouds:raymarch",
            (true, true, false) => "clouds:raymarch+temporal",
            (true, false, true) => "clouds:raymarch+shadows",
            (true, true, true) => "clouds:raymarch+temporal+shadows",
        }
    }

    pub(crate) fn from_env_reader<F>(mut read_env: F, feature_enabled: bool) -> Self
    where
        F: FnMut(&'static str) -> Option<OsString>,
    {
        let mut settings = Self {
            enabled: feature_enabled && read_env("FUN_DISABLE_CLOUDS").is_none(),
            quality: parse_env(
                &mut read_env,
                "FUN_CLOUD_QUALITY",
                FunCloudQuality::default(),
                FunCloudQuality::parse,
            ),
            internal_scale: parse_env(
                &mut read_env,
                "FUN_CLOUD_INTERNAL_SCALE",
                FunCloudInternalScale::default(),
                FunCloudInternalScale::parse,
            ),
            temporal_enabled: parse_env_bool(&mut read_env, "FUN_CLOUD_TEMPORAL", true),
            shadows_enabled: parse_env_bool(&mut read_env, "FUN_CLOUD_SHADOWS", false),
            profile_id: parse_env(
                &mut read_env,
                "FUN_CLOUD_PROFILE",
                FunWeatherProfileId::Scattered,
                FunWeatherProfileId::parse,
            ),
            debug_overlay: parse_env(
                &mut read_env,
                "FUN_CLOUD_DEBUG_OVERLAY",
                FunCloudDebugOverlay::None,
                FunCloudDebugOverlay::parse,
            ),
        };

        if settings.quality == FunCloudQuality::Off {
            settings.enabled = false;
            settings.temporal_enabled = false;
            settings.shadows_enabled = false;
        }

        settings
    }
}

impl ExtractResource for FunCloudSettings {
    type Source = FunCloudSettings;

    fn extract_resource(source: &Self::Source) -> Self {
        *source
    }
}

fn parse_env<T, F, R>(read_env: &mut R, name: &'static str, default_value: T, parse_value: F) -> T
where
    T: Copy,
    F: Fn(&str) -> Option<T>,
    R: FnMut(&'static str) -> Option<OsString>,
{
    let Some(raw) = read_env(name) else {
        return default_value;
    };
    let value = raw.to_string_lossy();
    parse_value(&value).unwrap_or_else(|| {
        warn!(
            target: "fun::render::clouds",
            setting = name,
            value = %value,
            "ignored invalid cloud render setting"
        );
        default_value
    })
}

fn parse_env_bool<R>(read_env: &mut R, name: &'static str, default_value: bool) -> bool
where
    R: FnMut(&'static str) -> Option<OsString>,
{
    let Some(raw) = read_env(name) else {
        return default_value;
    };
    let value = raw.to_string_lossy();
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => true,
        "0" | "false" | "no" | "off" => false,
        _ => {
            warn!(
                target: "fun::render::clouds",
                setting = name,
                value = %value,
                default_value,
                "ignored invalid cloud boolean setting"
            );
            default_value
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_reader(
        entries: &[(&'static str, &'static str)],
        name: &'static str,
    ) -> Option<OsString> {
        entries
            .iter()
            .find_map(|(key, value)| (*key == name).then(|| OsString::from(value)))
    }

    #[test]
    fn cloud_settings_parse_env_values() {
        let entries = [
            ("FUN_CLOUD_QUALITY", "cinematic"),
            ("FUN_CLOUD_INTERNAL_SCALE", "0.75"),
            ("FUN_CLOUD_TEMPORAL", "0"),
            ("FUN_CLOUD_SHADOWS", "1"),
            ("FUN_CLOUD_PROFILE", "storm_front"),
            ("FUN_CLOUD_DEBUG_OVERLAY", "history"),
        ];

        let settings = FunCloudSettings::from_env_reader(|name| env_reader(&entries, name), true);

        assert!(settings.enabled);
        assert_eq!(settings.quality, FunCloudQuality::Cinematic);
        assert_eq!(settings.internal_scale, FunCloudInternalScale::ThreeQuarter);
        assert!(!settings.temporal_enabled);
        assert!(settings.shadows_enabled);
        assert_eq!(settings.profile_id, FunWeatherProfileId::StormFront);
        assert_eq!(settings.debug_overlay, FunCloudDebugOverlay::History);
    }

    #[test]
    fn disabled_clouds_force_safe_off_state() {
        let entries = [
            ("FUN_DISABLE_CLOUDS", "1"),
            ("FUN_CLOUD_QUALITY", "balanced"),
            ("FUN_CLOUD_TEMPORAL", "1"),
        ];

        let settings = FunCloudSettings::from_env_reader(|name| env_reader(&entries, name), true);

        assert!(!settings.enabled);
        assert_eq!(settings.quality, FunCloudQuality::Balanced);
        assert!(settings.temporal_enabled);
    }

    #[test]
    fn off_quality_disables_temporal_and_shadows() {
        let entries = [
            ("FUN_CLOUD_QUALITY", "off"),
            ("FUN_CLOUD_TEMPORAL", "1"),
            ("FUN_CLOUD_SHADOWS", "1"),
        ];

        let settings = FunCloudSettings::from_env_reader(|name| env_reader(&entries, name), true);

        assert!(!settings.enabled);
        assert_eq!(settings.quality, FunCloudQuality::Off);
        assert!(!settings.temporal_enabled);
        assert!(!settings.shadows_enabled);
    }

    #[test]
    fn feature_gate_disables_clouds_even_when_env_requests_quality() {
        let entries = [("FUN_CLOUD_QUALITY", "cinematic")];

        let settings = FunCloudSettings::from_env_reader(|name| env_reader(&entries, name), false);

        assert!(!settings.enabled);
        assert_eq!(settings.quality, FunCloudQuality::Cinematic);
    }

    #[test]
    fn internal_scale_rounds_up_to_nonzero_extent() {
        assert_eq!(
            FunCloudInternalScale::ThreeQuarter.scale_size(UVec2::new(1919, 1079)),
            UVec2::new(1440, 810)
        );
        assert_eq!(
            FunCloudInternalScale::Third.scale_size(UVec2::new(1, 1)),
            UVec2::ONE
        );
    }
}
