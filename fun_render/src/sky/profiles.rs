use bevy::prelude::Vec3;

use super::{
    config::FunCloudQuality,
    weather::{FunCloudTypeMix, FunWeatherProfile, FunWeatherProfileId},
};

pub fn builtin_weather_profile(id: FunWeatherProfileId) -> Option<FunWeatherProfile> {
    match id {
        FunWeatherProfileId::Clear => Some(clear()),
        FunWeatherProfileId::Scattered => Some(scattered()),
        FunWeatherProfileId::Overcast => Some(overcast()),
        FunWeatherProfileId::StormFront => Some(storm_front()),
        FunWeatherProfileId::CinematicSunset => Some(cinematic_sunset()),
        FunWeatherProfileId::Custom => None,
    }
}

fn clear() -> FunWeatherProfile {
    FunWeatherProfile {
        id: FunWeatherProfileId::Clear,
        display_name: "Clear",
        sky_zenith_rgb: [0.18, 0.32, 0.68],
        sky_horizon_rgb: [0.58, 0.74, 0.96],
        sun_direction_override: None,
        sun_illuminance_lux: 42_000.0,
        ambient_rgb: [0.036, 0.043, 0.055],
        cloud_coverage: 0.02,
        cloud_density: 0.08,
        cloud_base_meters: 2_200.0,
        cloud_top_meters: 7_500.0,
        cloud_type_mix: FunCloudTypeMix::new(0.85, 0.1, 0.05, 0.0),
        anvil_strength: 0.0,
        edge_softness: 0.72,
        detail_strength: 0.25,
        precipitation_probability: 0.0,
        storm_intensity: 0.0,
        wind_direction: Vec3::new(1.0, 0.0, 0.22),
        wind_speed_mps: 9.0,
        wind_shear_mps: 4.0,
        turbulence_strength: 0.12,
        weather_map_scale_meters: 128_000.0,
        transition_seconds: 30.0,
        render_quality_hint: FunCloudQuality::Cheap,
    }
}

fn scattered() -> FunWeatherProfile {
    FunWeatherProfile {
        id: FunWeatherProfileId::Scattered,
        display_name: "Scattered",
        sky_zenith_rgb: [0.16, 0.29, 0.61],
        sky_horizon_rgb: [0.55, 0.72, 0.93],
        sun_direction_override: None,
        sun_illuminance_lux: 35_000.0,
        ambient_rgb: [0.034, 0.04, 0.052],
        cloud_coverage: 0.36,
        cloud_density: 0.72,
        cloud_base_meters: 1_700.0,
        cloud_top_meters: 6_200.0,
        cloud_type_mix: FunCloudTypeMix::new(0.08, 0.68, 0.22, 0.02),
        anvil_strength: 0.0,
        edge_softness: 0.55,
        detail_strength: 0.72,
        precipitation_probability: 0.03,
        storm_intensity: 0.0,
        wind_direction: Vec3::new(0.9, 0.0, 0.35),
        wind_speed_mps: 14.0,
        wind_shear_mps: 8.0,
        turbulence_strength: 0.28,
        weather_map_scale_meters: 64_000.0,
        transition_seconds: 45.0,
        render_quality_hint: FunCloudQuality::Balanced,
    }
}

fn overcast() -> FunWeatherProfile {
    FunWeatherProfile {
        id: FunWeatherProfileId::Overcast,
        display_name: "Overcast",
        sky_zenith_rgb: [0.31, 0.35, 0.42],
        sky_horizon_rgb: [0.46, 0.5, 0.56],
        sun_direction_override: None,
        sun_illuminance_lux: 12_000.0,
        ambient_rgb: [0.05, 0.052, 0.058],
        cloud_coverage: 0.9,
        cloud_density: 1.15,
        cloud_base_meters: 1_200.0,
        cloud_top_meters: 5_400.0,
        cloud_type_mix: FunCloudTypeMix::new(0.02, 0.18, 0.74, 0.06),
        anvil_strength: 0.08,
        edge_softness: 0.82,
        detail_strength: 0.48,
        precipitation_probability: 0.35,
        storm_intensity: 0.18,
        wind_direction: Vec3::new(0.74, 0.0, -0.42),
        wind_speed_mps: 18.0,
        wind_shear_mps: 14.0,
        turbulence_strength: 0.4,
        weather_map_scale_meters: 96_000.0,
        transition_seconds: 90.0,
        render_quality_hint: FunCloudQuality::Balanced,
    }
}

fn storm_front() -> FunWeatherProfile {
    FunWeatherProfile {
        id: FunWeatherProfileId::StormFront,
        display_name: "Storm Front",
        sky_zenith_rgb: [0.08, 0.1, 0.14],
        sky_horizon_rgb: [0.27, 0.29, 0.34],
        sun_direction_override: None,
        sun_illuminance_lux: 7_500.0,
        ambient_rgb: [0.036, 0.038, 0.046],
        cloud_coverage: 0.82,
        cloud_density: 1.85,
        cloud_base_meters: 900.0,
        cloud_top_meters: 9_000.0,
        cloud_type_mix: FunCloudTypeMix::new(0.0, 0.08, 0.28, 0.64),
        anvil_strength: 0.72,
        edge_softness: 0.48,
        detail_strength: 0.86,
        precipitation_probability: 0.78,
        storm_intensity: 0.88,
        wind_direction: Vec3::new(-0.52, 0.0, 0.86),
        wind_speed_mps: 33.0,
        wind_shear_mps: 42.0,
        turbulence_strength: 0.84,
        weather_map_scale_meters: 48_000.0,
        transition_seconds: 45.0,
        render_quality_hint: FunCloudQuality::Balanced,
    }
}

fn cinematic_sunset() -> FunWeatherProfile {
    FunWeatherProfile {
        id: FunWeatherProfileId::CinematicSunset,
        display_name: "Cinematic Sunset",
        sky_zenith_rgb: [0.12, 0.18, 0.38],
        sky_horizon_rgb: [0.96, 0.45, 0.22],
        sun_direction_override: Some(Vec3::new(-0.62, -0.18, -0.32)),
        sun_illuminance_lux: 18_000.0,
        ambient_rgb: [0.06, 0.045, 0.038],
        cloud_coverage: 0.42,
        cloud_density: 0.86,
        cloud_base_meters: 1_800.0,
        cloud_top_meters: 7_800.0,
        cloud_type_mix: FunCloudTypeMix::new(0.16, 0.42, 0.34, 0.08),
        anvil_strength: 0.14,
        edge_softness: 0.62,
        detail_strength: 0.76,
        precipitation_probability: 0.05,
        storm_intensity: 0.08,
        wind_direction: Vec3::new(0.35, 0.0, -0.94),
        wind_speed_mps: 11.0,
        wind_shear_mps: 10.0,
        turbulence_strength: 0.3,
        weather_map_scale_meters: 72_000.0,
        transition_seconds: 75.0,
        render_quality_hint: FunCloudQuality::Cinematic,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_profile_validates() {
        for id in [
            FunWeatherProfileId::Clear,
            FunWeatherProfileId::Scattered,
            FunWeatherProfileId::Overcast,
            FunWeatherProfileId::StormFront,
            FunWeatherProfileId::CinematicSunset,
        ] {
            let profile = builtin_weather_profile(id).expect("built-in profile exists");
            assert_eq!(profile.validated().expect("profile validates").id, id);
        }
    }
}
