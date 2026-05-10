//! Pass-73 fun-renderer-backed packet consumer integration tests.
//!
//! Drives the rvelte-side
//! [`rvelte_fun_render_adapter::FunRenderUiAdapter::submit`] against
//! [`fun_rvelte_bridge::fun_renderer_backend::FunRendererPacketConsumer`]
//! using frame packets produced by the existing pass-58 / 59 / 60
//! route fixtures. The consumer lifts each packet directly through
//! `fun_renderer::ui::native_adapter::NativeUiRendererDescriptors::from_packet`
//! — no per-call walker, no fake renderer.
//!
//! These tests only run when the `fun_renderer_backend` feature is
//! enabled (`cargo test --features fun_renderer_backend`).

#![cfg(feature = "fun_renderer_backend")]

use std::path::{Path, PathBuf};

use fun_rvelte_bridge::fun_renderer_backend::{
    FUN_RENDERER_PACKET_CONSUMER_SCHEMA, FUN_RENDERER_PACKET_CONSUMER_SCHEMA_VERSION,
    FunRendererPacketConsumer,
};
use rvelte_fun_native_codegen::{
    FunNativeManifest,
    host_bridge::{FunNativeHostBridge, FunNativeHostScenario, HostAuthorizationContext},
    runner::FunNativePipelineRunner,
};
use rvelte_fun_render_adapter::{FunRenderUiAdapter, FunRenderUiAdapterError};
use rvelte_fun_ui_core::{FunUiFramePacket, FunUiResourceDelta};

fn workspace_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("..")
}

fn read_to_string(path: &Path) -> String {
    match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => panic!("read {}: {error}", path.display()),
    }
}

fn must_ok<T, E: core::fmt::Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("expected Ok, got {error:?}"),
    }
}

fn must_err<T: core::fmt::Debug, E>(result: Result<T, E>) -> E {
    match result {
        Ok(value) => panic!("expected Err, got Ok({value:?})"),
        Err(error) => error,
    }
}

fn frame_for_route(
    manifest_name: &str,
    scenario_name: &str,
    scope: &str,
    declared_routes: Vec<u64>,
) -> FunUiFramePacket {
    let manifest_path = workspace_dir()
        .join("rvelte")
        .join("examples")
        .join("fun-native")
        .join("routes")
        .join(manifest_name);
    let manifest = must_ok(FunNativeManifest::parse(
        &read_to_string(&manifest_path),
        &manifest_path,
    ));
    let scenario_path = workspace_dir()
        .join("rvelte")
        .join("examples")
        .join("fun-native")
        .join("routes")
        .join(scenario_name);
    let scenario = must_ok(FunNativeHostScenario::parse(&read_to_string(
        &scenario_path,
    )));
    let mut runner = FunNativePipelineRunner::new(manifest);
    let mut bridge = FunNativeHostBridge::new(1, scope);
    bridge.set_declared_routes(declared_routes);
    must_ok(bridge.subscribe(HostAuthorizationContext {
        scope_label: String::from(scope),
        session_revision: 1,
        capability_mask: 0xFF,
    }));
    must_ok(bridge.apply_snapshot(scenario.lower_snapshot()));
    runner.sync_from_bridge(&bridge);
    let bundle = must_ok(runner.run_frame(1));
    bundle.frame
}

fn record_glyph_resources(consumer: &mut FunRendererPacketConsumer, frame: &FunUiFramePacket) {
    use fun_renderer::component_api::RenderTextureAssetId;
    use fun_renderer::ui::native_adapter::{
        RendererUiGlyphResource, RendererUiImageFormat, RendererUiImageResource, RendererUiSize,
    };

    for delta in &frame.resource_deltas {
        match delta {
            FunUiResourceDelta::Glyph(glyph) => {
                consumer.resource_table_mut().record_glyph(
                    glyph.glyph_run_id,
                    RendererUiGlyphResource {
                        atlas_texture: RenderTextureAssetId::default(),
                        atlas_size: RendererUiSize {
                            width: 256,
                            height: 256,
                        },
                    },
                );
            }
            FunUiResourceDelta::Image(image) => {
                consumer.resource_table_mut().record_image(
                    image.image_id,
                    RendererUiImageResource {
                        texture: RenderTextureAssetId::default(),
                        size: RendererUiSize::from_packet(image.size),
                        format: RendererUiImageFormat::from_packet(image.format),
                    },
                );
            }
        }
    }
}

#[test]
fn schema_label_is_stable() {
    assert_eq!(
        FUN_RENDERER_PACKET_CONSUMER_SCHEMA,
        "fun.product.rvelte_bridge.fun_renderer_backend.v1"
    );
    assert_eq!(FUN_RENDERER_PACKET_CONSUMER_SCHEMA_VERSION, 1);
}

#[test]
fn launcher_shell_lifts_through_fun_renderer_backend() {
    let frame = frame_for_route(
        "LauncherShell.rvelte.yaml",
        "LauncherShell.host.yaml",
        "launcher",
        vec![100, 101, 102, 400_000],
    );
    let mut consumer = FunRendererPacketConsumer::new();
    record_glyph_resources(&mut consumer, &frame);
    let adapter = FunRenderUiAdapter::new();
    let result = must_ok(adapter.submit(&frame, &mut consumer));
    assert!(result.layers_submitted >= 1);
    assert!(result.draws_submitted > 0);
    let descriptors = match consumer.last_descriptors() {
        Some(d) => d,
        None => panic!("expected last_descriptors after submit"),
    };
    assert_eq!(descriptors.frame_id, frame.frame_id.get());
    assert!(descriptors.passed_validation());
    assert_eq!(consumer.submit_count(), 1);
}

#[test]
fn hud_overlay_lifts_through_fun_renderer_backend() {
    let frame = frame_for_route(
        "HudOverlay.rvelte.yaml",
        "HudOverlay.host.yaml",
        "hud",
        vec![100, 400_000, 400_001, 400_002, 400_003],
    );
    let mut consumer = FunRendererPacketConsumer::new();
    record_glyph_resources(&mut consumer, &frame);
    let adapter = FunRenderUiAdapter::new();
    let result = must_ok(adapter.submit(&frame, &mut consumer));
    assert!(result.layers_submitted >= 1);
    assert!(result.draws_submitted > 0);
}

#[test]
fn pause_menu_lifts_through_fun_renderer_backend() {
    let frame = frame_for_route(
        "PauseMenuRoute.rvelte.yaml",
        "PauseMenuRoute.host.yaml",
        "pause_menu",
        vec![99, 100, 101, 102, 103, 104],
    );
    let mut consumer = FunRendererPacketConsumer::new();
    record_glyph_resources(&mut consumer, &frame);
    let adapter = FunRenderUiAdapter::new();
    let result = must_ok(adapter.submit(&frame, &mut consumer));
    assert!(result.layers_submitted >= 1);
    assert!(result.draws_submitted > 0);
}

#[test]
fn missing_glyph_resource_surfaces_typed_rejection() {
    // Don't pre-populate the resource table; the consumer will see
    // missing-glyph diagnostics from fun-renderer's validation and
    // surface them through the rvelte typed-error vocabulary.
    let frame = frame_for_route(
        "LauncherShell.rvelte.yaml",
        "LauncherShell.host.yaml",
        "launcher",
        vec![100, 101, 102, 400_000],
    );
    let mut consumer = FunRendererPacketConsumer::new();
    let adapter = FunRenderUiAdapter::new();
    let error = must_err(adapter.submit(&frame, &mut consumer));
    let FunRenderUiAdapterError::MissingResource { kind, .. } = error else {
        panic!("expected MissingResource error variant, got something else");
    };
    assert_eq!(kind, "glyph");
}

#[test]
fn submit_count_advances_per_frame() {
    let frame = frame_for_route(
        "HudOverlay.rvelte.yaml",
        "HudOverlay.host.yaml",
        "hud",
        vec![100, 400_000, 400_001, 400_002, 400_003],
    );
    let mut consumer = FunRendererPacketConsumer::new();
    record_glyph_resources(&mut consumer, &frame);
    let adapter = FunRenderUiAdapter::new();
    must_ok(adapter.submit(&frame, &mut consumer));
    must_ok(adapter.submit(&frame, &mut consumer));
    must_ok(adapter.submit(&frame, &mut consumer));
    assert_eq!(consumer.submit_count(), 3);
}

#[test]
fn capability_report_drives_adapter_pre_validation() {
    use rvelte_fun_render_adapter::FunRenderUiCapabilityReport;
    let frame = frame_for_route(
        "LauncherShell.rvelte.yaml",
        "LauncherShell.host.yaml",
        "launcher",
        vec![100, 101, 102, 400_000],
    );
    // Disable text-run support in the consumer's capability
    // report. The rvelte adapter's pre-walk capability-policy
    // gate rejects the frame before fun-renderer ever sees it,
    // even though the consumer would otherwise have accepted the
    // packet.
    let mut capability = FunRenderUiCapabilityReport::full();
    capability.supports_text_run_reference = false;
    let mut consumer = FunRendererPacketConsumer::new().with_capability(capability);
    record_glyph_resources(&mut consumer, &frame);
    let adapter = FunRenderUiAdapter::new();
    let error = must_err(adapter.submit(&frame, &mut consumer));
    assert_eq!(error.code(), "rvt.fun_render.adapter.unsupported_primitive");
}

#[test]
fn composite_layer_policy_is_threaded_into_descriptors() {
    use fun_renderer::ui::native_adapter::NativeUiCompositeLayer;
    let frame = frame_for_route(
        "PauseMenuRoute.rvelte.yaml",
        "PauseMenuRoute.host.yaml",
        "pause_menu",
        vec![99, 100, 101, 102, 103, 104],
    );
    let mut consumer =
        FunRendererPacketConsumer::new().with_composite_layer(NativeUiCompositeLayer::OverlayOnly);
    record_glyph_resources(&mut consumer, &frame);
    let adapter = FunRenderUiAdapter::new();
    must_ok(adapter.submit(&frame, &mut consumer));
    let descriptors = match consumer.last_descriptors() {
        Some(d) => d,
        None => panic!("expected last_descriptors"),
    };
    assert_eq!(
        descriptors.graph_pass.composite_into_layer,
        NativeUiCompositeLayer::OverlayOnly
    );
}
