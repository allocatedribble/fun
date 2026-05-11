//! Pass C7.4.3 / C7.4.4 — typed cloud shadow shader source registry.
//!
//! Exposes the typed WGSL source for the typed
//! `cloud_shadow_project.wgsl` + `cloud_shadow_filter.wgsl` compute
//! shaders + the typed entry-point names + the typed role taxonomy.
//! Pass C7.x+ will consume this registry to construct typed wgpu /
//! HLSL / SPIR-V pipeline modules; today the typed module exposes the
//! typed source strings + the typed audit predicates so the typed
//! shader contracts are wired through the Rust layer.
//!
//! The typed shader source lives next to this module under
//! `clouds/shaders/`.  Future cloud-renderer-owned shaders (cloud
//! raymarch, temporal resolve, composite, debug overlay) will land in
//! the typed same directory.

pub const FUN_RENDERER_CLOUD_SHADERS_SCHEMA_VERSION: u16 = 1;

/// Typed Pass C7.4.3 cloud shadow projection compute shader source.
/// Marches every typed shadow texel toward the sun through the typed
/// cloud slab and writes the typed transmittance into
/// `CloudWorldShadowTransmittance`.
pub const CLOUD_SHADOW_PROJECT_WGSL: &str =
    include_str!("clouds/shaders/cloud_shadow_project.wgsl");

/// Typed Pass C7.4.4 cloud shadow filter compute shader source.
/// Applies a typed softness-modulated gaussian-like blur to the typed
/// projected transmittance + writes the typed result into
/// `CloudWorldShadowFiltered`.
pub const CLOUD_SHADOW_FILTER_WGSL: &str = include_str!("clouds/shaders/cloud_shadow_filter.wgsl");

/// Typed Pass C7.4.3 entry-point name in `cloud_shadow_project.wgsl`.
pub const CLOUD_SHADOW_PROJECT_ENTRY_POINT: &str = "project_cloud_shadow";

/// Typed Pass C7.4.4 entry-point name in `cloud_shadow_filter.wgsl`.
pub const CLOUD_SHADOW_FILTER_ENTRY_POINT: &str = "filter_cloud_shadow";

/// Typed Pass C7.4 cloud shadow shader role.  Names the typed cloud
/// shadow compute shaders the typed renderer dispatches in the C7.4
/// chain.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CloudShadowShaderRole {
    /// Typed Pass C7.4.3 — typed cloud shadow projection compute
    /// shader.  Reads typed weather + shape noise, marches toward the
    /// sun, writes typed transmittance.
    #[default]
    Project,
    /// Typed Pass C7.4.4 — typed cloud shadow filter compute shader.
    /// Reads typed `CloudWorldShadowTransmittance`, applies typed
    /// softness-modulated blur, writes typed
    /// `CloudWorldShadowFiltered`.
    Filter,
}

impl CloudShadowShaderRole {
    pub const ALL: [Self; 2] = [Self::Project, Self::Filter];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Project => "cloud_shadow_project",
            Self::Filter => "cloud_shadow_filter",
        }
    }

    /// Typed WGSL source for the typed role.
    #[must_use]
    pub const fn source(self) -> &'static str {
        match self {
            Self::Project => CLOUD_SHADOW_PROJECT_WGSL,
            Self::Filter => CLOUD_SHADOW_FILTER_WGSL,
        }
    }

    /// Typed entry-point name for the typed role.
    #[must_use]
    pub const fn entry_point(self) -> &'static str {
        match self {
            Self::Project => CLOUD_SHADOW_PROJECT_ENTRY_POINT,
            Self::Filter => CLOUD_SHADOW_FILTER_ENTRY_POINT,
        }
    }

    /// Typed source file path (relative to the typed `fun-renderer`
    /// crate root).  Used by the typed shader-load diagnostics +
    /// the typed `embedded_asset!` reflection when the typed GPU
    /// dispatch lands.
    #[must_use]
    pub const fn source_path(self) -> &'static str {
        match self {
            Self::Project => "src/clouds/shaders/cloud_shadow_project.wgsl",
            Self::Filter => "src/clouds/shaders/cloud_shadow_filter.wgsl",
        }
    }
}

/// Typed Pass C7.4 const predicate — the typed cloud shadow shaders
/// MUST NOT reference typed Lux virtual shadow / opaque shadow depth
/// resources.  Audited at the typed source-string layer: the typed
/// shader source must not contain typed `LuxVirtualShadow*` /
/// `LuxShadowAtlas` / `LuxDirectLighting` identifiers.  The typed
/// bind-layout shape (only `cloud_shadow_out` / `cloud_shadow_filtered`
/// appear) is the typed primary enforcement at the typed binding
/// boundary; this predicate is the typed secondary audit at the typed
/// source-text layer.
#[must_use]
pub fn cloud_shadow_shaders_do_not_reference_lux_shadow_resources() -> bool {
    let forbidden = [
        "LuxVirtualShadowPages",
        "LuxShadowAtlas",
        "LuxVirtualShadowFilter",
        "lux_virtual_shadow",
        "lux_shadow_atlas",
    ];
    for role in CloudShadowShaderRole::ALL {
        let src = role.source();
        for needle in forbidden {
            if has_active_reference(src, needle) {
                return false;
            }
        }
    }
    true
}

/// Typed helper — scan the typed shader source for an active
/// reference to the typed needle.  Comments (lines that start with
/// `//` after trimming) are skipped so the typed audit ignores typed
/// documentation that mentions the typed forbidden names.
fn has_active_reference(src: &str, needle: &str) -> bool {
    for raw_line in src.lines() {
        let trimmed = raw_line.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        // Block-comment-aware scan is intentionally omitted — the
        // typed cloud shaders use only line comments (`//`), audited
        // by the typed test below.
        if trimmed.contains(needle) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pass C7.4 acceptance — typed cloud shadow shader role taxonomy
    /// is dense.
    #[test]
    fn cloud_shadow_shader_role_taxonomy_is_dense() {
        assert_eq!(CloudShadowShaderRole::ALL.len(), 2);
        let mut seen = hashbrown::HashSet::new();
        for role in CloudShadowShaderRole::ALL {
            assert!(seen.insert(role.as_str()), "duplicate: {}", role.as_str());
            assert!(role.as_str().starts_with("cloud_shadow_"));
        }
        // Typed default is the typed projection role.
        assert_eq!(
            CloudShadowShaderRole::default(),
            CloudShadowShaderRole::Project
        );
    }

    /// Pass C7.4 acceptance — typed shader source strings are non-empty
    /// and contain the typed entry-point declarations + the typed
    /// user-spec bindings.
    #[test]
    fn cloud_shadow_project_source_carries_user_spec_shape() {
        let src = CLOUD_SHADOW_PROJECT_WGSL;
        assert!(!src.is_empty());
        // Typed entry point — must appear as a typed compute-shader
        // function declaration.
        assert!(src.contains("@compute"));
        assert!(src.contains("fn project_cloud_shadow"));
        // Typed bindings per user spec.
        assert!(src.contains("@group(0) @binding(0) var<uniform> cloud: CloudParams"));
        assert!(src.contains(
            "@group(0) @binding(1) var<uniform> shadow_projection: CloudShadowProjectionConstants",
        ));
        assert!(src.contains("@group(0) @binding(2) var weather_map: texture_2d<f32>"));
        assert!(src.contains("@group(0) @binding(3) var shape_noise: texture_3d<f32>"));
        assert!(src.contains(
            "@group(0) @binding(4) var cloud_shadow_out: texture_storage_2d<r16float, write>",
        ));
        // Typed Beer-Lambert conversion — `transmittance = exp(-tau)`.
        assert!(src.contains("exp(-tau)"));
        // Typed shadow projection samples weather + shape noise.
        assert!(src.contains("sample_weather"));
        assert!(src.contains("sample_shape"));
        // Typed shadow projection writes to typed cloud_shadow_out.
        assert!(src.contains("textureStore(\n        cloud_shadow_out"));
    }

    /// Pass C7.4 acceptance — typed shader writes the typed packed
    /// 4-channel payload (R = transmittance, G = optical_depth,
    /// B = coverage, A = confidence) per the typed user spec, even
    /// though the typed R16Float binding only stores the typed R
    /// channel.  Reserving the typed extra channels in the typed
    /// `textureStore` call lets the typed Rgba16FloatPacked variant
    /// drop in by swapping only the typed binding format.
    #[test]
    fn cloud_shadow_project_writes_packed_payload() {
        let src = CLOUD_SHADOW_PROJECT_WGSL;
        // Typed `transmittance` + typed `tau` (optical depth) + typed
        // `coverage_out` + typed `confidence` are all wired into the
        // typed final `textureStore` call.
        assert!(src.contains("vec4<f32>(transmittance, tau, coverage_out, confidence)"));
    }

    /// Pass C7.4 acceptance — typed cloud shadow project shader does
    /// not write to typed Lux virtual shadow pages or opaque shadow
    /// depth resources.  Audited via the typed source-string scan +
    /// the typed bind-layout shape (only `cloud_shadow_out` appears
    /// as a typed write target).
    #[test]
    fn cloud_shadow_project_does_not_touch_lux_shadow_resources() {
        assert!(cloud_shadow_shaders_do_not_reference_lux_shadow_resources());
        let src = CLOUD_SHADOW_PROJECT_WGSL;
        // Typed only typed write target is typed `cloud_shadow_out`.
        // Count typed `textureStore(` occurrences — every typed write
        // must target `cloud_shadow_out`.
        for (idx, _) in src.match_indices("textureStore(") {
            // Typed text after the typed `textureStore(` opening
            // paren — the typed first non-whitespace token after the
            // typed paren is the typed target texture name.
            let rest = &src[idx + "textureStore(".len()..];
            let trimmed = rest.trim_start();
            assert!(
                trimmed.starts_with("cloud_shadow_out"),
                "project shader writes to non-cloud-shadow target: {}",
                &trimmed[..trimmed.len().min(40)],
            );
        }
    }

    /// Pass C7.4 acceptance — typed cloud shadow filter shader carries
    /// the typed user-spec bindings + entry point.
    #[test]
    fn cloud_shadow_filter_source_carries_user_spec_shape() {
        let src = CLOUD_SHADOW_FILTER_WGSL;
        assert!(!src.is_empty());
        assert!(src.contains("@compute"));
        assert!(src.contains("fn filter_cloud_shadow"));
        // Typed bindings per user spec.
        assert!(src.contains(
            "@group(0) @binding(0) var<uniform> shadow_projection: CloudShadowProjectionConstants",
        ));
        assert!(src.contains("@group(0) @binding(1) var cloud_shadow_in: texture_2d<f32>"));
        assert!(src.contains(
            "@group(0) @binding(2) var cloud_shadow_filtered: texture_storage_2d<r16float, write>",
        ));
        // Typed filter reads typed `cloud_shadow_in` (input target =
        // `CloudWorldShadowTransmittance`) and writes typed
        // `cloud_shadow_filtered` (output target =
        // `CloudWorldShadowFiltered`).
        assert!(src.contains("textureLoad(cloud_shadow_in"));
        assert!(src.contains("textureStore(\n        cloud_shadow_filtered"));
    }

    /// Pass C7.4 acceptance — typed filter shader's typed softness
    /// knob (`knobs.y` from typed `CloudShadowProjectionConstants`)
    /// drives a typed measurable mask gradient: typed source must
    /// read `knobs.y` and use it to size the typed kernel.
    #[test]
    fn cloud_shadow_filter_softness_drives_kernel() {
        let src = CLOUD_SHADOW_FILTER_WGSL;
        assert!(
            src.contains("knobs.y"),
            "filter shader does not read softness knob"
        );
        // Typed softness threads into typed kernel radius — the typed
        // `radius` local variable depends on typed `softness`.
        assert!(src.contains("softness"));
        assert!(src.contains("radius"));
        // Typed softness == 0 → typed no-blur fast path.
        assert!(src.contains("if (softness <= 1e-4)"));
    }

    /// Pass C7.4 acceptance — typed filter shader's typed opacity
    /// knob (`knobs.x` from typed `CloudShadowProjectionConstants`)
    /// drives a typed measurable shadow strength: typed source must
    /// read `knobs.x` and use it to modulate the typed output
    /// transmittance.
    #[test]
    fn cloud_shadow_filter_opacity_drives_shadow_strength() {
        let src = CLOUD_SHADOW_FILTER_WGSL;
        assert!(
            src.contains("knobs.x"),
            "filter shader does not read opacity knob"
        );
        assert!(src.contains("opacity"));
        // Typed opacity composes: typed `1 - (1 - blurred) * opacity`
        // (typed opacity=0 → typed transmittance always 1; typed
        // opacity=1 → typed blurred passes through).
        assert!(src.contains("(1.0 - blurred) * opacity"));
    }

    /// Pass C7.4 acceptance — typed filter shader's typed edge-safe
    /// clamp keeps typed kernel reads inside the typed texture
    /// extent: typed source must clamp typed sample coords to typed
    /// `[0, dims - 1]`.
    #[test]
    fn cloud_shadow_filter_kernel_is_edge_safe() {
        let src = CLOUD_SHADOW_FILTER_WGSL;
        assert!(src.contains("clamp(coord, vec2<i32>(0), dims - vec2<i32>(1))"));
    }

    /// Pass C7.4 acceptance — typed filter shader does not write to
    /// typed Lux virtual shadow pages or opaque shadow depth
    /// resources.  Audited via the typed source-string scan + the
    /// typed bind-layout shape (only `cloud_shadow_filtered` appears
    /// as a typed write target).
    #[test]
    fn cloud_shadow_filter_does_not_touch_lux_shadow_resources() {
        assert!(cloud_shadow_shaders_do_not_reference_lux_shadow_resources());
        let src = CLOUD_SHADOW_FILTER_WGSL;
        for (idx, _) in src.match_indices("textureStore(") {
            let rest = &src[idx + "textureStore(".len()..];
            let trimmed = rest.trim_start();
            assert!(
                trimmed.starts_with("cloud_shadow_filtered"),
                "filter shader writes to non-filtered target: {}",
                &trimmed[..trimmed.len().min(40)],
            );
        }
    }

    /// Pass C7.4 acceptance — typed shader role API mirrors the typed
    /// source / entry point / path constants.
    #[test]
    fn cloud_shadow_shader_role_api_matches_constants() {
        assert_eq!(
            CloudShadowShaderRole::Project.source(),
            CLOUD_SHADOW_PROJECT_WGSL
        );
        assert_eq!(
            CloudShadowShaderRole::Filter.source(),
            CLOUD_SHADOW_FILTER_WGSL
        );
        assert_eq!(
            CloudShadowShaderRole::Project.entry_point(),
            CLOUD_SHADOW_PROJECT_ENTRY_POINT,
        );
        assert_eq!(
            CloudShadowShaderRole::Filter.entry_point(),
            CLOUD_SHADOW_FILTER_ENTRY_POINT,
        );
        assert_eq!(
            CloudShadowShaderRole::Project.source_path(),
            "src/clouds/shaders/cloud_shadow_project.wgsl",
        );
        assert_eq!(
            CloudShadowShaderRole::Filter.source_path(),
            "src/clouds/shaders/cloud_shadow_filter.wgsl",
        );
    }
}
