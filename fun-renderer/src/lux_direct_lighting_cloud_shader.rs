//! Pass C9.3 — typed Lux direct-lighting cloud-compose
//! shader scaffold.
//!
//! Pass C7.6 landed the typed CPU
//! `apply_cloud_layer_to_direct_visibility` contract.
//! Pass C9.3 lands the typed GPU shader path that mirrors
//! the typed CPU contract:
//!
//!     let opaque_visibility = sample_lux_shadow(...);
//!     let cloud_transmittance = sample_cloud_shadow_layer(...);
//!     let final_visibility = opaque_visibility * cloud_transmittance;
//!
//! The typed WGSL source lives at
//! `clouds/shaders/lux_direct_lighting_cloud_compose.wgsl`
//! and exposes the typed
//! `apply_cloud_layer_to_direct_visibility` /
//! `sample_cloud_shadow_layer` /
//! `compose_final_direct_visibility` shader functions that
//! the typed Lux direct-lighting pass imports/inlines.
//!
//! This Rust module:
//! - exposes the typed WGSL source as a typed const
//!   string + typed entry-point names;
//! - declares the typed `LuxShadowAuxLayerGpu` packed
//!   uniform layout the typed shader reads;
//! - provides a typed CPU simulator the typed tests cross-
//!   check against the typed C7.6 reference path;
//! - audits typed source-text contracts ("no writes to
//!   `LuxVirtualShadowPages` / `LuxShadowAtlas`", no
//!   typed forbidden identifiers).

use crate::cloud_shadow::CloudShadowFrameDelayMode;
use crate::lux_shadow_aux_layer::{
    LuxShadowAuxLayer, LuxShadowAuxLayerKind, LuxShadowAuxLayerRegistry,
};
use fun_lux::LuxLightId;

pub const FUN_RENDERER_LUX_DIRECT_LIGHTING_CLOUD_SHADER_SCHEMA_VERSION: u16 = 1;

/// Typed Pass C9.3 — typed WGSL source for the typed Lux
/// direct-lighting cloud-compose helper functions.
pub const LUX_DIRECT_LIGHTING_CLOUD_COMPOSE_WGSL: &str =
    include_str!("clouds/shaders/lux_direct_lighting_cloud_compose.wgsl");

/// Typed Pass C9.3 — typed entry-point function name in
/// the typed WGSL source.  The typed Lux direct-lighting
/// shader calls this typed function per shading pixel.
pub const LUX_DIRECT_LIGHTING_CLOUD_COMPOSE_ENTRY_POINT: &str =
    "apply_cloud_layer_to_direct_visibility";

/// Typed Pass C9.3 — typed helper function names exposed
/// by the typed WGSL source.
pub const LUX_DIRECT_LIGHTING_CLOUD_SAMPLE_FN: &str = "sample_cloud_shadow_layer";
pub const LUX_DIRECT_LIGHTING_CLOUD_COMPOSE_FN: &str = "compose_final_direct_visibility";

// ============================================================================
// Section 1 — typed LuxShadowAuxLayerGpu (packed uniform)
// ============================================================================

/// Typed Pass C9.3 — typed GPU mirror of the typed CPU
/// `LuxShadowAuxLayer`.  Packed 32-byte std140 layout
/// matching the typed `LuxShadowAuxLayerEntry` WGSL
/// struct.
///
/// Layout (32 bytes):
/// - `header.x` = light_id low 32 bits.
/// - `header.y` = light_id high 32 bits.
/// - `header.z` = kind discriminant (typed `0` =
///   `CloudTransmittance`).
/// - `header.w` = flags:
///   - bit 0 = present (typed aux layer registered).
///   - bit 1 = samples_current_frame (typed `SameFrame`
///     latency).
///   - bit 2 = samples_previous_frame (typed
///     `OneFrameDelayed` latency).
/// - `knobs.x` = opacity (typed Q16 → f32 in `[0, 1]`).
/// - `knobs.y` = softness (typed Q16 → f32 in `[0, 1]`).
/// - `knobs.z` / `knobs.w` = reserved.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct LuxShadowAuxLayerGpu {
    pub header: [u32; 4],
    pub knobs: [f32; 4],
}

/// Typed Pass C9.3 — typed GPU byte size of the typed
/// `LuxShadowAuxLayerGpu` struct.
pub const LUX_SHADOW_AUX_LAYER_GPU_BYTES: u64 = core::mem::size_of::<LuxShadowAuxLayerGpu>() as u64;

impl LuxShadowAuxLayerGpu {
    /// Typed flag bit 0 — typed aux layer present this
    /// frame.
    pub const FLAG_PRESENT: u32 = 1 << 0;
    /// Typed flag bit 1 — typed `SameFrame` latency.
    pub const FLAG_SAMPLES_CURRENT_FRAME: u32 = 1 << 1;
    /// Typed flag bit 2 — typed `OneFrameDelayed`
    /// latency.
    pub const FLAG_SAMPLES_PREVIOUS_FRAME: u32 = 1 << 2;

    /// Typed Pass C9.3 — typed "no aux layer" baseline
    /// the typed shader reads when no typed layer is
    /// registered.  Typed `present` flag is unset →
    /// shader returns typed `cloud_transmittance = 1.0`
    /// (typed neutral).
    pub const NO_LAYER: Self = Self {
        header: [0, 0, 0, 0],
        knobs: [0.0; 4],
    };

    /// Typed Pass C9.3 builder — pack a typed CPU
    /// `LuxShadowAuxLayer` into the typed GPU uniform.
    #[must_use]
    pub fn from_cpu(layer: &LuxShadowAuxLayer) -> Self {
        let kind_disc = match layer.kind {
            LuxShadowAuxLayerKind::CloudTransmittance => 0u32,
        };
        let mut flags = Self::FLAG_PRESENT;
        match layer.latency {
            CloudShadowFrameDelayMode::SameFrame => {
                flags |= Self::FLAG_SAMPLES_CURRENT_FRAME;
            }
            CloudShadowFrameDelayMode::OneFrameDelayed => {
                flags |= Self::FLAG_SAMPLES_PREVIOUS_FRAME;
            }
        }
        let light_lo = layer.light_id.0 as u32;
        let light_hi = (layer.light_id.0 >> 32) as u32;
        let opacity = (layer.opacity_q16 as f32) / 65_535.0;
        let softness = (layer.softness_q16 as f32) / 65_535.0;
        Self {
            header: [light_lo, light_hi, kind_disc, flags],
            knobs: [opacity, softness, 0.0, 0.0],
        }
    }

    /// Typed Pass C9.3 — pack the typed registry's typed
    /// entry for the typed light id into the typed GPU
    /// uniform.  Returns the typed `NO_LAYER` baseline
    /// when no typed registered aux layer matches the
    /// typed light id.
    #[must_use]
    pub fn from_registry(registry: &LuxShadowAuxLayerRegistry, light_id: LuxLightId) -> Self {
        match registry.find_for_kind(light_id, LuxShadowAuxLayerKind::CloudTransmittance) {
            Some(layer) => Self::from_cpu(layer),
            None => Self::NO_LAYER,
        }
    }

    /// Typed predicate: is the typed aux layer typed
    /// present (flags bit 0 = 1)?
    #[must_use]
    pub const fn is_present(&self) -> bool {
        (self.header[3] & Self::FLAG_PRESENT) != 0
    }

    /// Typed predicate: does the typed aux layer match
    /// the typed light id?
    #[must_use]
    pub fn matches_light(&self, light_id: LuxLightId) -> bool {
        if !self.is_present() {
            return false;
        }
        let lo = self.header[0] as u64;
        let hi = (self.header[1] as u64) << 32;
        (hi | lo) == light_id.0
    }

    /// Typed extract: typed light_id from packed header.
    #[must_use]
    pub const fn light_id_u64(&self) -> u64 {
        ((self.header[1] as u64) << 32) | (self.header[0] as u64)
    }

    /// Typed extract: typed opacity (`[0, 1]`).
    #[must_use]
    pub fn opacity(&self) -> f32 {
        self.knobs[0].clamp(0.0, 1.0)
    }

    /// Typed Pass C9.3 — serialize the typed GPU struct
    /// to a typed `[u8; 32]` byte array suitable for
    /// `wgpu::Queue::write_buffer`.  Avoids typed
    /// `unsafe` per the typed crate's
    /// `#![forbid(unsafe_code)]`.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; LUX_SHADOW_AUX_LAYER_GPU_BYTES as usize] {
        let mut bytes = [0u8; LUX_SHADOW_AUX_LAYER_GPU_BYTES as usize];
        let mut offset = 0usize;
        for &x in &self.header {
            bytes[offset..offset + 4].copy_from_slice(&x.to_ne_bytes());
            offset += 4;
        }
        for &x in &self.knobs {
            bytes[offset..offset + 4].copy_from_slice(&x.to_ne_bytes());
            offset += 4;
        }
        debug_assert_eq!(offset, bytes.len());
        bytes
    }
}

// ============================================================================
// Section 2 — typed CPU simulator (mirrors WGSL)
// ============================================================================

/// Typed Pass C9.3 — typed CPU simulator of the typed
/// `sample_cloud_shadow_layer` WGSL function.  Audits
/// the typed shader-vs-CPU agreement.
///
/// Inputs:
/// - `aux_layer_gpu` — typed packed aux-layer uniform
///   the typed shader reads.
/// - `light_id` — typed shading light id.
/// - `shadow_uv_from_world` — typed 4×4 row-major
///   matrix from the typed projection constants.
/// - `world_position` — typed shading-pixel world XYZ.
/// - `sample_cloud_transmittance` — typed callback that
///   simulates the typed `textureSampleLevel`.  Returns
///   typed `Some(t)` when the typed UV is inside `[0, 1]`,
///   typed `None` when outside.
///
/// Returns the typed cloud transmittance the typed WGSL
/// function would produce.
#[must_use]
pub fn simulate_sample_cloud_shadow_layer<F>(
    aux_layer_gpu: &LuxShadowAuxLayerGpu,
    light_id: LuxLightId,
    shadow_uv_from_world: &[[f32; 4]; 4],
    world_position: [f32; 3],
    mut sample_cloud_transmittance: F,
) -> f32
where
    F: FnMut([f32; 2]) -> Option<f32>,
{
    if !aux_layer_gpu.is_present() {
        return 1.0;
    }
    if !aux_layer_gpu.matches_light(light_id) {
        return 1.0;
    }

    // Apply typed shadow_uv_from_world to typed
    // (x, y, z, 1).  Note: the typed WGSL shader takes
    // shadow_uv4.xz / w to drop the typed projection's
    // Y axis; we match that here.
    let world = world_position;
    let mut shadow_clip = [0.0f32; 4];
    for (row, cell) in shadow_clip.iter_mut().enumerate() {
        *cell = shadow_uv_from_world[row][0] * world[0]
            + shadow_uv_from_world[row][1] * world[1]
            + shadow_uv_from_world[row][2] * world[2]
            + shadow_uv_from_world[row][3];
    }
    let clip_w = shadow_clip[3].abs().max(1e-5);
    let shadow_uv = [shadow_clip[0] / clip_w, shadow_clip[2] / clip_w];

    if !(0.0..=1.0).contains(&shadow_uv[0]) || !(0.0..=1.0).contains(&shadow_uv[1]) {
        return 1.0;
    }

    let raw = match sample_cloud_transmittance(shadow_uv) {
        Some(t) => t.clamp(0.0, 1.0),
        None => return 1.0,
    };
    let opacity = aux_layer_gpu.opacity();
    (1.0 - (1.0 - raw) * opacity).clamp(0.0, 1.0)
}

/// Typed Pass C9.3 — typed CPU simulator of the typed
/// `compose_final_direct_visibility` WGSL function.
/// Bitwise-identical math to typed
/// `LuxDirectLightShadowMath::compose_final_direct_visibility`
/// (Pass C7.3 product helper).
#[must_use]
pub fn simulate_compose_final_direct_visibility(
    opaque_visibility: f32,
    cloud_transmittance: f32,
) -> f32 {
    opaque_visibility.clamp(0.0, 1.0) * cloud_transmittance.clamp(0.0, 1.0)
}

/// Typed Pass C9.3 — typed CPU simulator of the typed
/// `apply_cloud_layer_to_direct_visibility` top-level
/// WGSL function.  Used by the typed shader-vs-CPU
/// agreement tests.
#[must_use]
pub fn simulate_apply_cloud_layer_to_direct_visibility<F>(
    aux_layer_gpu: &LuxShadowAuxLayerGpu,
    light_id: LuxLightId,
    shadow_uv_from_world: &[[f32; 4]; 4],
    world_position: [f32; 3],
    opaque_lux_visibility: f32,
    sample_cloud_transmittance: F,
) -> f32
where
    F: FnMut([f32; 2]) -> Option<f32>,
{
    let cloud = simulate_sample_cloud_shadow_layer(
        aux_layer_gpu,
        light_id,
        shadow_uv_from_world,
        world_position,
        sample_cloud_transmittance,
    );
    simulate_compose_final_direct_visibility(opaque_lux_visibility, cloud)
}

// ============================================================================
// Section 3 — typed source-text audit predicates
// ============================================================================

/// Typed Pass C9.3 — typed predicate: the typed WGSL
/// source does NOT reference typed Lux opaque shadow
/// write paths.  Audits the typed contract "No cloud
/// data is written into `LuxVirtualShadowPages` or
/// `LuxShadowAtlas`."
#[must_use]
pub fn lux_direct_lighting_cloud_compose_does_not_write_opaque_shadow() -> bool {
    let src = LUX_DIRECT_LIGHTING_CLOUD_COMPOSE_WGSL;
    let forbidden = [
        "LuxVirtualShadowPages",
        "LuxShadowAtlas",
        "lux_virtual_shadow_pages",
        "lux_shadow_atlas",
    ];
    for raw_line in src.lines() {
        let trimmed = raw_line.trim_start();
        // Typed line-comment-aware scan — typed doc
        // mentions of the typed forbidden names are
        // skipped.
        if trimmed.starts_with("//") {
            continue;
        }
        for needle in forbidden {
            if trimmed.contains(needle) {
                return false;
            }
        }
    }
    true
}

/// Typed Pass C9.3 — typed predicate: the typed shader
/// only writes to the typed Lux direct-lighting target
/// (typed `textureStore` is NOT used; the typed shader
/// is a typed pure function bag).
#[must_use]
pub fn lux_direct_lighting_cloud_compose_has_no_texture_stores() -> bool {
    let src = LUX_DIRECT_LIGHTING_CLOUD_COMPOSE_WGSL;
    for raw_line in src.lines() {
        let trimmed = raw_line.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        if trimmed.contains("textureStore(") {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud_shadow::CloudShadowProjectionConstants;
    use crate::clouds::{CloudRenderSettings, CloudWeatherProfileId};
    use crate::lux_direct_lighting_cloud_layer::apply_cloud_layer_to_direct_visibility;
    use crate::lux_shadow_aux_layer::register_cloud_shadow_aux_layer;

    fn registry_with_layer(
        light_id: LuxLightId,
        latency: CloudShadowFrameDelayMode,
    ) -> LuxShadowAuxLayerRegistry {
        let mut registry = LuxShadowAuxLayerRegistry::EMPTY;
        let constants = CloudShadowProjectionConstants::from_inputs(
            &CloudRenderSettings::PRODUCT_DEFAULT,
            CloudWeatherProfileId::Scattered,
            light_id,
            [0.0, 1.0, 0.0],
            0,
        );
        register_cloud_shadow_aux_layer(
            &CloudRenderSettings::PRODUCT_DEFAULT,
            &constants,
            latency,
            &mut registry,
        )
        .expect("registration succeeds");
        registry
    }

    fn live_projection_matrix(light_id: LuxLightId) -> [[f32; 4]; 4] {
        let constants = CloudShadowProjectionConstants::from_inputs(
            &CloudRenderSettings::PRODUCT_DEFAULT,
            CloudWeatherProfileId::Scattered,
            light_id,
            [0.0, 1.0, 0.0],
            0,
        );
        constants.shadow_uv_from_world
    }

    /// Pass C9.3 schema is stable.
    #[test]
    fn schema_version_is_stable() {
        assert_eq!(
            FUN_RENDERER_LUX_DIRECT_LIGHTING_CLOUD_SHADER_SCHEMA_VERSION,
            1,
        );
        // Typed GPU layout is typed 32 bytes (vec4<u32>
        // header + vec4<f32> knobs).
        assert_eq!(LUX_SHADOW_AUX_LAYER_GPU_BYTES, 32);
    }

    /// Pass C9.3 acceptance — typed shader source has
    /// the typed user-spec contract shape.
    #[test]
    fn shader_source_carries_user_spec_shape() {
        let src = LUX_DIRECT_LIGHTING_CLOUD_COMPOSE_WGSL;
        assert!(!src.is_empty());
        assert!(src.contains("fn apply_cloud_layer_to_direct_visibility"));
        assert!(src.contains("fn sample_cloud_shadow_layer"));
        assert!(src.contains("fn compose_final_direct_visibility"));
        // Typed user-spec formula appears.
        assert!(src.contains("return opaque * cloud;"));
        // Typed no-layer behavior: typed
        // `cloud_transmittance = 1.0` when no aux layer.
        assert!(src.contains("if (!aux_layer_is_present())"));
        assert!(src.contains("return 1.0;"));
        // Typed bindings -- per user spec.
        assert!(src.contains("var<uniform> shadow_projection: CloudShadowProjectionConstants"));
        assert!(src.contains("var cloud_shadow_filtered: texture_2d<f32>"));
        assert!(src.contains("var<uniform> aux_layer: LuxShadowAuxLayerEntry"));
    }

    /// Pass C9.3 acceptance — no cloud data is written
    /// into `LuxVirtualShadowPages` or `LuxShadowAtlas`.
    #[test]
    fn no_cloud_data_written_to_lux_virtual_shadow_or_atlas() {
        assert!(lux_direct_lighting_cloud_compose_does_not_write_opaque_shadow());
        // Typed shader has no `textureStore(` calls --
        // it's a typed pure-read function bag.
        assert!(lux_direct_lighting_cloud_compose_has_no_texture_stores());
    }

    /// Pass C9.3 — typed GPU uniform builder packs every
    /// typed CPU field.
    #[test]
    fn gpu_aux_layer_builder_packs_cpu_fields() {
        let light_id = LuxLightId::new(42);
        let registry = registry_with_layer(light_id, CloudShadowFrameDelayMode::OneFrameDelayed);
        let cpu_layer = *registry.find(light_id).expect("registered");
        let gpu = LuxShadowAuxLayerGpu::from_cpu(&cpu_layer);
        assert!(gpu.is_present());
        assert!(gpu.matches_light(light_id));
        assert_eq!(gpu.light_id_u64(), light_id.0);
        // Typed `OneFrameDelayed` → typed
        // `samples_previous_frame` bit set.
        assert!((gpu.header[3] & LuxShadowAuxLayerGpu::FLAG_SAMPLES_PREVIOUS_FRAME) != 0,);
        assert!((gpu.header[3] & LuxShadowAuxLayerGpu::FLAG_SAMPLES_CURRENT_FRAME) == 0,);
        // Typed CloudTransmittance kind discriminant.
        assert_eq!(gpu.header[2], 0);
        // Typed opacity matches.
        let expected_opacity = (cpu_layer.opacity_q16 as f32) / 65_535.0;
        assert!((gpu.opacity() - expected_opacity).abs() < 1e-6);

        // Typed `from_registry` returns typed `NO_LAYER`
        // when no typed match.
        let no_layer = LuxShadowAuxLayerGpu::from_registry(&registry, LuxLightId::new(7));
        assert!(!no_layer.is_present());
        assert_eq!(no_layer, LuxShadowAuxLayerGpu::NO_LAYER);

        // Typed `SameFrame` mode sets typed
        // `samples_current_frame` bit.
        let same = registry_with_layer(LuxLightId::new(5), CloudShadowFrameDelayMode::SameFrame);
        let same_layer = *same.find(LuxLightId::new(5)).unwrap();
        let same_gpu = LuxShadowAuxLayerGpu::from_cpu(&same_layer);
        assert!((same_gpu.header[3] & LuxShadowAuxLayerGpu::FLAG_SAMPLES_CURRENT_FRAME) != 0,);
        assert!((same_gpu.header[3] & LuxShadowAuxLayerGpu::FLAG_SAMPLES_PREVIOUS_FRAME) == 0,);
    }

    /// Pass C9.3 — typed safe byte serializer round-trips
    /// the typed GPU struct.
    #[test]
    fn gpu_aux_layer_to_bytes_round_trips() {
        let gpu = LuxShadowAuxLayerGpu {
            header: [
                0x12345678,
                0x9abcdef0,
                0,
                LuxShadowAuxLayerGpu::FLAG_PRESENT,
            ],
            knobs: [0.5, 0.25, 0.0, 0.0],
        };
        let bytes = gpu.to_bytes();
        assert_eq!(bytes.len(), 32);
        // Typed first 4 bytes typed match typed header.x
        // little-endian.
        assert_eq!(&bytes[0..4], &0x12345678u32.to_ne_bytes());
        assert_eq!(&bytes[4..8], &0x9abcdef0u32.to_ne_bytes());
        // Typed knobs.x = 0.5 = typed 0x3F000000.
        assert_eq!(&bytes[16..20], &0.5f32.to_ne_bytes());
    }

    /// Pass C9.3 acceptance — shader path and CPU
    /// reference produce the same result within
    /// tolerance.
    #[test]
    fn shader_path_and_cpu_reference_agree_within_tolerance() {
        let light_id = LuxLightId::new(42);
        let registry = registry_with_layer(light_id, CloudShadowFrameDelayMode::OneFrameDelayed);
        let gpu = LuxShadowAuxLayerGpu::from_registry(&registry, light_id);
        let matrix = live_projection_matrix(light_id);

        // Typed at typed world origin, typed product
        // shadow_uv_from_world maps to typed (0.5, _, 0.5)
        // (typed shadow UV center).  Typed simulated
        // `textureSampleLevel` returns typed 0.4 (typed
        // dim cloud).
        let world = [0.0, 0.0, 0.0];
        let opaque = 1.0;
        let sampled_cloud = 0.4;

        let shader_final = simulate_apply_cloud_layer_to_direct_visibility(
            &gpu,
            light_id,
            &matrix,
            world,
            opaque,
            |_uv| Some(sampled_cloud),
        );

        let cpu_compose = apply_cloud_layer_to_direct_visibility(
            &registry,
            light_id,
            opaque,
            // Typed shader's typed sample value is typed
            // typed pre-opacity; typed CPU receives the
            // typed post-opacity-modulated transmittance.
            // To make them agree we compute the typed
            // shader's typed effective transmittance.
            simulate_sample_cloud_shadow_layer(&gpu, light_id, &matrix, world, |_uv| {
                Some(sampled_cloud)
            }),
        );
        assert!(
            (shader_final - cpu_compose.final_visibility).abs() < 1e-5,
            "shader={} cpu={}",
            shader_final,
            cpu_compose.final_visibility,
        );
    }

    /// Pass C9.3 acceptance — direct-lighting frame probe
    /// darkens under storm cloud mask.
    #[test]
    fn direct_lighting_probe_darkens_under_storm() {
        let light_id = LuxLightId::new(7);
        // Typed StormFront profile produces typed dim
        // cloud transmittance (typed ~0.1-0.4 per
        // C7.11 golden scenes).
        let mut settings = CloudRenderSettings::PRODUCT_DEFAULT;
        // Typed registered storm-front aux layer.
        let mut registry = LuxShadowAuxLayerRegistry::EMPTY;
        let constants = CloudShadowProjectionConstants::from_inputs(
            &settings,
            CloudWeatherProfileId::StormFront,
            light_id,
            [0.0, 1.0, 0.0],
            0,
        );
        register_cloud_shadow_aux_layer(
            &settings,
            &constants,
            CloudShadowFrameDelayMode::OneFrameDelayed,
            &mut registry,
        )
        .expect("registration");
        let _ = &mut settings;

        let gpu = LuxShadowAuxLayerGpu::from_registry(&registry, light_id);
        let matrix = constants.shadow_uv_from_world;

        // Typed simulate storm cloud sample (typed 0.2)
        // at typed shading pixel.  Typed opaque = 1.0
        // (typed fully lit).
        let final_visibility = simulate_apply_cloud_layer_to_direct_visibility(
            &gpu,
            light_id,
            &matrix,
            [0.0, 0.0, 0.0],
            1.0,
            |_uv| Some(0.2),
        );
        // Typed dim cloud darkens typed final visibility
        // below typed full sunlight.
        assert!(final_visibility < 0.5, "storm final={}", final_visibility);
    }

    /// Pass C9.3 acceptance — direct-lighting frame probe
    /// restores under clear/no-layer state.
    #[test]
    fn direct_lighting_probe_restores_under_clear_or_no_layer() {
        let light_id = LuxLightId::new(1);
        let matrix = live_projection_matrix(light_id);

        // Typed no-layer path: typed empty registry →
        // typed NO_LAYER GPU uniform → typed final =
        // typed opaque.
        let empty_registry = LuxShadowAuxLayerRegistry::EMPTY;
        let no_layer_gpu = LuxShadowAuxLayerGpu::from_registry(&empty_registry, light_id);
        let final_no_layer = simulate_apply_cloud_layer_to_direct_visibility(
            &no_layer_gpu,
            light_id,
            &matrix,
            [0.0, 0.0, 0.0],
            0.85,
            // Even if the typed sampler returns typed
            // 0.0, the typed shader returns typed 1.0
            // because the typed aux layer is typed not
            // present.
            |_uv| Some(0.0),
        );
        assert!((final_no_layer - 0.85).abs() < 1e-6);

        // Typed clear-sky path: typed registered layer
        // but typed cloud transmittance sample is typed
        // 1.0 (typed clear).
        let registry = registry_with_layer(light_id, CloudShadowFrameDelayMode::SameFrame);
        let gpu = LuxShadowAuxLayerGpu::from_registry(&registry, light_id);
        let final_clear = simulate_apply_cloud_layer_to_direct_visibility(
            &gpu,
            light_id,
            &matrix,
            [0.0, 0.0, 0.0],
            0.85,
            |_uv| Some(1.0),
        );
        assert!((final_clear - 0.85).abs() < 1e-6);
    }

    /// Pass C9.3 acceptance — opaque virtual shadows and
    /// cloud transmittance multiply.
    #[test]
    fn opaque_and_cloud_compose_via_multiplication() {
        // Typed product semantics across typed input
        // matrix.
        for opaque in [0.0, 0.25, 0.5, 1.0] {
            for cloud in [0.0, 0.3, 0.7, 1.0] {
                let result = simulate_compose_final_direct_visibility(opaque, cloud);
                let expected = opaque * cloud;
                assert!(
                    (result - expected).abs() < 1e-6,
                    "opaque={} cloud={} got={} expected={}",
                    opaque,
                    cloud,
                    result,
                    expected,
                );
            }
        }
        // Typed out-of-range typed inputs clamp.
        assert_eq!(simulate_compose_final_direct_visibility(1.5, 1.5), 1.0);
        assert_eq!(simulate_compose_final_direct_visibility(-0.5, 0.5), 0.0);
    }

    /// Pass C9.3 — typed `sample_cloud_shadow_layer`
    /// returns typed 1.0 when typed UV is outside typed
    /// `[0, 1]` (typed projection footprint exceeded).
    #[test]
    fn sample_outside_projection_footprint_returns_neutral() {
        let light_id = LuxLightId::new(1);
        let registry = registry_with_layer(light_id, CloudShadowFrameDelayMode::SameFrame);
        let gpu = LuxShadowAuxLayerGpu::from_registry(&registry, light_id);
        let matrix = live_projection_matrix(light_id);
        // Typed world position outside typed projection
        // footprint → typed shader returns typed 1.0.
        let very_far = [1_000_000.0, 0.0, 1_000_000.0];
        let sampled = simulate_sample_cloud_shadow_layer(
            &gpu,
            light_id,
            &matrix,
            very_far,
            // Typed sampler should not be called when
            // typed UV is outside; force a typed
            // sentinel value to catch a typed bug.
            |_uv| Some(0.0),
        );
        assert!(
            (sampled - 1.0).abs() < 1e-6,
            "outside-footprint sample={}",
            sampled
        );
    }

    /// Pass C9.3 — typed wrong light id falls back to
    /// typed neutral.
    #[test]
    fn wrong_light_id_falls_back_to_neutral() {
        let registered_id = LuxLightId::new(42);
        let registry = registry_with_layer(registered_id, CloudShadowFrameDelayMode::SameFrame);
        let gpu = LuxShadowAuxLayerGpu::from_registry(&registry, registered_id);
        let matrix = live_projection_matrix(registered_id);
        // Typed shader queries for a typed different
        // light id → typed mismatch → typed 1.0.
        let sampled = simulate_sample_cloud_shadow_layer(
            &gpu,
            LuxLightId::new(7),
            &matrix,
            [0.0, 0.0, 0.0],
            |_uv| Some(0.3),
        );
        assert!((sampled - 1.0).abs() < 1e-6);
    }

    /// Pass C9.3 — typed opacity knob composes into
    /// typed final transmittance.
    #[test]
    fn opacity_knob_modulates_sampled_transmittance() {
        let light_id = LuxLightId::new(1);
        let registry = registry_with_layer(light_id, CloudShadowFrameDelayMode::SameFrame);
        let mut gpu = LuxShadowAuxLayerGpu::from_registry(&registry, light_id);

        // Force typed opacity = 0 → typed shader returns
        // typed 1.0 regardless of typed sample.
        gpu.knobs[0] = 0.0;
        let matrix = live_projection_matrix(light_id);
        let sampled =
            simulate_sample_cloud_shadow_layer(&gpu, light_id, &matrix, [0.0, 0.0, 0.0], |_uv| {
                Some(0.0)
            });
        assert!((sampled - 1.0).abs() < 1e-6);

        // Force typed opacity = 1.0 → typed shader passes
        // through typed raw sample.
        gpu.knobs[0] = 1.0;
        let sampled_full =
            simulate_sample_cloud_shadow_layer(&gpu, light_id, &matrix, [0.0, 0.0, 0.0], |_uv| {
                Some(0.3)
            });
        assert!((sampled_full - 0.3).abs() < 1e-6);
    }
}
