use std::{
    ffi::OsString,
    fmt::Write as _,
    fs, io,
    path::{Path, PathBuf},
};

use bevy::prelude::*;
use fun_rvelte_bridge::{
    ProductRouteKind, ProductRouteRegistry, ProductRvelteAdapter,
    fun_renderer_backend::FunRendererPacketConsumer,
};
use serde_json::{Value, json};
use thunder::prelude::WorldRevision;
use tracing::{info, warn};

const ALPHA_RENDER_PROOF_SCHEMA: &str = "fun.alpha.render_proof.v1";
const ALPHA_RENDER_PROOF_DIR: &str = "target/alpha/render/arena-blockout";

#[derive(Debug, Clone, Copy)]
struct NativeUiAlphaSubmission {
    route: ProductRouteKind,
    frame_id: u64,
    layers_submitted: u32,
    draws_submitted: u32,
    diagnostic_count: u32,
    failed: bool,
    error_code: Option<&'static str>,
}

impl NativeUiAlphaSubmission {
    const fn failed(route: ProductRouteKind, code: &'static str) -> Self {
        Self {
            route,
            frame_id: 0,
            layers_submitted: 0,
            draws_submitted: 0,
            diagnostic_count: 0,
            failed: true,
            error_code: Some(code),
        }
    }
}

pub(crate) fn emit_alpha_render_proof() {
    if !alpha_artifacts_enabled() {
        return;
    }

    match write_alpha_render_proof() {
        Ok(paths) => {
            info!(
                target: "fun::alpha",
                schema = ALPHA_RENDER_PROOF_SCHEMA,
                json = %paths.json.display(),
                markdown = %paths.markdown.display(),
                "alpha render proof emitted"
            );
        }
        Err(error) => {
            warn!(
                target: "fun::alpha",
                schema = ALPHA_RENDER_PROOF_SCHEMA,
                %error,
                "alpha render proof emission failed"
            );
        }
    }
}

fn alpha_artifacts_enabled() -> bool {
    env_flag("FUN_FRAME_GRAPH_TRACE") || env_flag("FUN_ALPHA_RENDER_PROOF")
}

fn write_alpha_render_proof() -> io::Result<AlphaRenderProofPaths> {
    let proof = build_alpha_render_proof();
    let output_dir = alpha_render_proof_dir();
    let json_path = output_dir.join("render-proof.json");
    let markdown_path = output_dir.join("render-proof.md");
    let json = serde_json::to_vec_pretty(&proof).map_err(io::Error::other)?;
    atomic_write_bytes(&json_path, &json)?;
    atomic_write_bytes(&markdown_path, render_proof_markdown(&proof).as_bytes())?;
    Ok(AlphaRenderProofPaths {
        json: json_path,
        markdown: markdown_path,
    })
}

fn alpha_render_proof_dir() -> PathBuf {
    game_client_workspace_root().join(ALPHA_RENDER_PROOF_DIR)
}

fn game_client_workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn build_alpha_render_proof() -> Value {
    let renderer_backend =
        std::env::var("FUN_RENDERER_BACKEND").unwrap_or_else(|_| "fun".to_owned());
    let scene_id =
        std::env::var("FUN_SCENE_ID").unwrap_or_else(|_| game_scene::DEFAULT_SCENE_ID.0.to_owned());
    let manifest = game_scene::scene_manifest_by_id(&scene_id, WorldRevision(1));
    let native_ui = sample_native_ui_submission(ProductRouteKind::LauncherShell);

    let scene_manifest_loaded = manifest.is_some();
    let renderable_count = manifest
        .as_ref()
        .map(|manifest| manifest.renderer.render_entity_count)
        .unwrap_or_default();
    let lux_data_present = manifest.as_ref().is_some_and(|manifest| {
        manifest.lux.gi_participant_count > 0 || manifest.lux.lighting.is_some()
    });
    let preview_camera_present = manifest
        .as_ref()
        .is_some_and(|manifest| manifest.renderer.preview_camera.is_some());
    let frame_not_blank_clear_only = renderer_backend == "fun"
        && scene_id == game_scene::DEFAULT_SCENE_ID.0
        && scene_manifest_loaded
        && renderable_count > 0
        && lux_data_present
        && !native_ui.failed;

    json!({
        "schema": ALPHA_RENDER_PROOF_SCHEMA,
        "renderer_backend": renderer_backend,
        "scene_id": scene_id,
        "scene_manifest_loaded": scene_manifest_loaded,
        "renderable_count": renderable_count,
        "lux_data_present": lux_data_present,
        "preview_camera_present": preview_camera_present,
        "frame_not_blank_clear_only": frame_not_blank_clear_only,
        "frame_blank_evidence_basis": "scene_manifest_renderables_lux_and_native_ui_packet",
        "capture_available": false,
        "native_ui_packet_submission": {
            "route": native_ui.route.as_str(),
            "frame_id": native_ui.frame_id,
            "submitted_layer_count": native_ui.layers_submitted,
            "submitted_draw_count": native_ui.draws_submitted,
            "diagnostic_count": native_ui.diagnostic_count,
            "failed": native_ui.failed,
            "error_code": native_ui.error_code
        },
        "outputs": {
            "json": "target/alpha/render/arena-blockout/render-proof.json",
            "markdown": "target/alpha/render/arena-blockout/render-proof.md",
            "optional_frame_png": "target/alpha/render/arena-blockout/frame.png"
        }
    })
}

fn sample_native_ui_submission(route: ProductRouteKind) -> NativeUiAlphaSubmission {
    let renderer = FunRendererPacketConsumer::new().with_auto_materialize_resources(true);
    let mut adapter = ProductRvelteAdapter::new(ProductRouteRegistry::with_well_known(), renderer);
    if let Err(diagnostic) = adapter.set_active_route(route) {
        return NativeUiAlphaSubmission::failed(route, diagnostic.code());
    }
    match adapter.tick() {
        Ok(frame) => NativeUiAlphaSubmission {
            route: frame.route,
            frame_id: frame.frame_id,
            layers_submitted: frame.submit.layers_submitted,
            draws_submitted: frame.submit.draws_submitted,
            diagnostic_count: u32::try_from(frame.diagnostics.len()).unwrap_or(u32::MAX),
            failed: false,
            error_code: None,
        },
        Err(diagnostic) => NativeUiAlphaSubmission::failed(route, diagnostic.code()),
    }
}

fn render_proof_markdown(proof: &Value) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "# Alpha Render Proof");
    let _ = writeln!(output);
    let _ = writeln!(output, "- Schema: `{}`", value_str(proof, "schema"));
    let _ = writeln!(
        output,
        "- Renderer backend: `{}`",
        value_str(proof, "renderer_backend")
    );
    let _ = writeln!(output, "- Scene ID: `{}`", value_str(proof, "scene_id"));
    let _ = writeln!(
        output,
        "- Scene manifest loads: `{}`",
        value_bool(proof, "scene_manifest_loaded")
    );
    let _ = writeln!(
        output,
        "- Renderable count: `{}`",
        value_u64(proof, "renderable_count")
    );
    let _ = writeln!(
        output,
        "- Lux data present: `{}`",
        value_bool(proof, "lux_data_present")
    );
    let _ = writeln!(
        output,
        "- Frame not blank/clear-only: `{}`",
        value_bool(proof, "frame_not_blank_clear_only")
    );
    if let Some(native_ui) = proof.get("native_ui_packet_submission") {
        let _ = writeln!(output);
        let _ = writeln!(output, "## Native UI Packet");
        let _ = writeln!(output);
        let _ = writeln!(output, "- Route: `{}`", value_str(native_ui, "route"));
        let _ = writeln!(output, "- Frame ID: `{}`", value_u64(native_ui, "frame_id"));
        let _ = writeln!(
            output,
            "- Submitted layers: `{}`",
            value_u64(native_ui, "submitted_layer_count")
        );
        let _ = writeln!(
            output,
            "- Submitted draws: `{}`",
            value_u64(native_ui, "submitted_draw_count")
        );
        let _ = writeln!(
            output,
            "- Submission failed: `{}`",
            value_bool(native_ui, "failed")
        );
    }
    output
}

fn value_str<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("unknown")
}

fn value_bool(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn value_u64(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or_default()
}

fn env_flag(name: &str) -> bool {
    std::env::var(name).is_ok_and(|value| {
        value == "1" || value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("yes")
    })
}

fn atomic_write_bytes(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut temp_name = path
        .file_name()
        .map(OsString::from)
        .unwrap_or_else(|| OsString::from("artifact"));
    temp_name.push(".tmp");
    let temp_path = path.with_file_name(temp_name);
    if temp_path.exists() {
        fs::remove_file(&temp_path)?;
    }
    fs::write(&temp_path, bytes)?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(temp_path, path)
}

#[derive(Debug)]
struct AlphaRenderProofPaths {
    json: PathBuf,
    markdown: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_render_proof_covers_arena_blockout_and_native_ui_packet() {
        let proof = build_alpha_render_proof();

        assert_eq!(proof["schema"], ALPHA_RENDER_PROOF_SCHEMA);
        assert_eq!(proof["scene_id"], game_scene::DEFAULT_SCENE_ID.0);
        assert_eq!(proof["renderer_backend"], "fun");
        assert_eq!(proof["scene_manifest_loaded"], true);
        assert_eq!(proof["frame_not_blank_clear_only"], true);
        assert!(
            proof["native_ui_packet_submission"]["submitted_layer_count"]
                .as_u64()
                .unwrap_or_default()
                > 0
        );
        assert_eq!(proof["native_ui_packet_submission"]["failed"], false);

        let paths = write_alpha_render_proof().expect("alpha render proof should write");
        assert!(paths.json.exists());
        assert!(paths.markdown.exists());
    }
}
