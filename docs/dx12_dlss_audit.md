# DX12 DLSS Audit

status: initial_audit
owner_repo: fun
scope: DirectX 12 native DLSS Super Resolution path planning

## Crate Graph

- `game_client`: runtime executable host. Enables `fun_render/winit_presentation`, forwards `dlss` and `force_disable_dlss` to Bevy and `fun_render`, owns the legacy camera-side DLSS Ray Reconstruction activation hook behind the new explicit RR gate.
- `fun_render`: first-party render policy and diagnostics layer. Owns render backend selection, Winit/offscreen presentation plugins, Solari settings, render path signatures, RT feature policy, and performance diagnostics labels.
- `game_shared`: shared protocol and diagnostics types used by client/server and render reporting.
- `bevy`: local patched engine checkout through workspace path dependency and crate patches.
- `bevy_anti_alias`: local Bevy anti-aliasing crate. Its `dlss` feature depends on `dlss_wgpu` and `bevy_render/raw_vulkan_init`.
- `bevy_solari`: local Bevy Solari implementation used by `fun_render` for lighting and DLSS Ray Reconstruction denoiser mode plumbing.

## Relevant Cargo Features

- `game_client/default`: `dlss`, `volumetric_clouds`.
- `game_client/dlss`: `bevy/dlss`, `fun_render/dlss`.
- `game_client/force_disable_dlss`: `bevy/force_disable_dlss`, `fun_render/force_disable_dlss`.
- `game_client/render_diagnostics`: enables Bevy debug, PBR/meshlet diagnostics, `fun_render/render_diagnostics`, and `game_shared/diagnostics`.
- `game_client/dx12_dlss_native`: forwards to `fun_render/dx12_dlss_native`.
- `fun_render/dlss`: forwards to `bevy/dlss`.
- `fun_render/force_disable_dlss`: forwards to `bevy/force_disable_dlss`.
- `fun_render/dx12_dlss_native`: experimental Windows-only native DLSS gate. It exposes config, diagnostics, camera correctness gates, the centralized DX12 native interop boundary, and the first SR schedule node; the NVIDIA native bridge is not implemented.

## Render Plugin Entry Points

- `fun_render::FunRenderWinitPresentationPlugin`: selects `FUN_RENDER_BACKEND`, canonical `FUN_RENDER_PRESENT_MODE` with the legacy `FUN_PRESENT_MODE` alias, creates the Winit window, injects the Bevy render plugin, and logs native DX12 DLSS startup status.
- `fun_render::FunRenderOffscreenPresentationPlugin`: selects the same backend for editor/offscreen presentation, inserts the offscreen target, and logs native DX12 DLSS startup status.
- `fun_render::FunRenderCorePlugin`: installs render policy, Solari plugins, diagnostics resources, render path signature, world-stream activation, and temporal reset systems.
- `game_client::build_app_with_options`: adds `FunRenderWinitPresentationPlugin` and `FunRenderCorePlugin` for the product client.

## Presentation Paths

- Winit/window presentation: `game_client` with `fun_render/winit_presentation`.
- Offscreen/editor presentation: `fun_render/offscreen`; currently used as a policy surface for editor-owned preview paths and does not own a child `game_client`.
- Unified CEF/Svelte UI presentation: `game_client` owns browser UI and overlays game render; this is not a DLSS integration point.

## Current Anti-Aliasing Path

- Existing `dlss` feature enters through Bevy's `bevy_anti_alias::dlss`.
- The local Bevy crate declares `bevy_anti_alias/dlss = ["dep:dlss_wgpu", "dep:uuid", "bevy_render/raw_vulkan_init"]`.
- `fun_render::winit` inserts `DlssProjectId` before `DefaultPlugins` when `dlss` is enabled and `force_disable_dlss` is absent.
- FUN camera code currently activates DLSS Ray Reconstruction components conditionally; it does not provide a native DX12 Super Resolution backend.

## Current Solari/RR Path

- `FUN_SOLARI_DENOISE_MODE=rr|dlss|dlss-rr|ray-reconstruction` only selects `SolariDenoiseMode::DlssRayReconstruction` when `FUN_RENDER_DX12_DLSS_RR=1` is set and `FUN_DISABLE_DLSS_RR` is absent.
- `FUN_DISABLE_DLSS_RR=1` remains a legacy kill switch and blocks RR activation even when the new gate is set.
- `game_client` only inserts `Dlss<DlssRayReconstructionFeature>` when the explicit RR gate is open and Bevy reports `DlssRayReconstructionSupported`.
- RR diagnostics include `dlss_rr_gpu_ns` and `solari_pass_dlss_rr_guide_resolve_ns`.
- BalancedFast remains the normal Solari denoiser; RR is an explicit comparison lane.

## Benchmark Hooks

- `scripts/benchmark_client.ps1`: captures FPS/frame/Solari/meshlet/DLSS RR metrics.
- `scripts/benchmark_denoisers.ps1`: compares denoiser and RR modes, including `dlss_rr_gpu_ns` plus guide resolve; RR lanes pass `-EnableDx12DlssRr`.
- `scripts/benchmark_rt_matrix.ps1`: includes a `dlss_rr_diagnostic` lane that passes `-EnableDx12DlssRr`.
- `scripts/benchmark_required_lanes.ps1`: runs required client benchmark lanes.
- `docs/client_benchmarking.md` and `docs/client_diagnostics.md` define the current benchmark and diagnostic expectations.

## DLSS References

- `fun_render/Cargo.toml`: `dlss`, `force_disable_dlss`, `dx12_dlss_native`.
- `game_client/Cargo.toml`: `dlss`, `force_disable_dlss`, `dx12_dlss_native`.
- `fun_render/src/winit.rs`: `DlssProjectId` insertion.
- `fun_render/src/config.rs`: DLSS RR disable flag, native DX12 DLSS config, backend selection.
- `fun_render/src/dlss_correctness.rs`: camera validation, native DLSS marker component, previous matrix tracking, mode-driven input resolution and mip bias, reset reasons, and debug visualization selectors.
- `fun_render/src/dx12_dlss_rr.rs`: explicit native RR gate, fail-closed status, and Solari guide-surface audit table.
- `fun_render/src/dx12_dlss_sr.rs`: native SR schedule insertion, per-view extraction, support/status state, output-resolution intermediate texture allocation, resize/mode/device lifecycle tracking, failure budget, and fail-closed evaluation stub.
- `fun_render/src/dx12_native`: isolated wgpu DX12 HAL extraction for borrowed device, queue, texture, and future command-list handles.
- `fun_render/src/core.rs`: disables RR unless Solari denoise mode is RR.
- `fun_render/src/solari.rs`: denoiser mode parsing.
- `fun_render/src/signature.rs`: render path signature records Bevy-facing RR state.
- `game_client/src/lib.rs`: camera-side RR activation, reset, and diagnostics.
- `scripts/benchmark_client.ps1`: `dlss_rr_gpu_ms` parsing and benchmark output.
- `scripts/benchmark_denoisers.ps1`: denoiser/RR cost comparison.

## Backend Selection

- `FUN_RENDER_BACKEND=dx12|d3d12|directx12` selects `Backends::DX12`.
- `FUN_RENDER_BACKEND=vulkan|vk` explicitly selects Vulkan.
- Empty, absent, or unknown values select the platform default: DX12 on Windows,
  Vulkan elsewhere.
- `FUN_RENDER_BACKEND=auto` selects `Backends::VULKAN | Backends::DX12`.
- The stack and benchmark PowerShell helpers default to DX12 on Windows.
