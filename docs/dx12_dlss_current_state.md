# DX12 DLSS Current State

status: initial_current_state
owner_repo: fun
scope: native DirectX 12 DLSS Super Resolution integration boundary

## Existing Wiring

- `game_client` defaults to the existing Bevy-facing `dlss` feature and forwards it to `fun_render`.
- `fun_render/winit_presentation` inserts the NVIDIA `DlssProjectId` before Bevy `DefaultPlugins` when the existing Bevy DLSS feature is compiled.
- `fun_dx12_dlss` now exists as the native Windows C ABI bridge crate, but it is a fail-closed scaffold until Streamline or NGX is linked.
- The local Bevy fork exposes DLSS through `bevy_anti_alias::dlss`, and that crate depends on `dlss_wgpu`.
- `game_client` has camera-side plumbing for `Dlss<DlssRayReconstructionFeature>`, including support checks, history reset, and activation when Solari denoise mode is the RR preset.
- `fun_render` owns Solari denoiser selection, render path signatures, RT feature policy, backend selection, and diagnostics.
- Benchmark scripts already collect DLSS Ray Reconstruction timing via `dlss_rr_gpu_ns` and `solari_pass_dlss_rr_guide_resolve_ns`.

## Known Broken Behavior

- DLSS Ray Reconstruction is preserved as an explicit `rr`/`dlss-rr` comparison lane, but current docs already mark it as not functioning properly for normal runtime use.
- Current RR behavior can leave Solari shadows broken or missing; BalancedFast remains the normal runtime denoiser.
- Existing Bevy DLSS setup should not be assumed to provide a working DirectX 12 path.

## Missing DX12 Backend

- There is a native Windows bridge crate scaffold, but it does not initialize Streamline or NGX against a D3D12 device and command queue yet.
- `fun_render::dx12_native` is now the controlled wgpu HAL trapdoor for D3D12 device, queue, and texture resource handles.
- Command encoder HAL extraction is centralized there, but `ID3D12GraphicsCommandList` access intentionally fails closed because wgpu-hal 29 does not expose the raw command list publicly.
- Native SR now has Bevy-side resource, size, texture format, depth, and motion-vector gates before evaluation. There is still no SDK resource tagging or successful command-list evaluation path.
- SDK/runtime discovery exists in `fun_dx12_dlss` for `NVIDIA_STREAMLINE_SDK`, `NVIDIA_NGX_SDK`, `FUN_NVIDIA_DLSS_SDK`, and `FUN_NVIDIA_DLSS_DLL`, but support still reports false until the SDK integration is linked.
- The native SR schedule node is inserted into the 3D pipeline, and the Rust-side resize, mode-switch, and device-recovery lifecycle is represented. It still cannot call NVIDIA SDK resize/evaluate/destroy code until the native bridge and raw command-list accessor exist.

## New Experimental Surface

- `fun_render/dx12_dlss_native` is a Windows-only experimental feature gate.
- `game_client/dx12_dlss_native` forwards to `fun_render/dx12_dlss_native`.
- The feature currently adds configuration, diagnostics, a narrow DX12 native interop boundary, camera data-correctness gates, and the first SR schedule node. It does not compile or call NVIDIA SDK code yet.
- `FUN_RENDER_DX12_DLSS=1|auto` requests native DLSS, but the request only becomes active when the feature is compiled for Windows.
- `FUN_RENDER_DX12_DLSS_MODE=quality|balanced|performance|ultra_performance` selects the future SR mode.
- `FUN_DX12_DLSS=1` and `FUN_DX12_DLSS_MODE=quality|balanced|performance|ultra_performance` are supported aliases for the same SR request.
- `FUN_RENDER_DX12_DLSS_RR=1` is intentionally separate and defaults off; Super Resolution must work before RR is connected. `FUN_SOLARI_DENOISE_MODE=rr|dlss|dlss-rr|ray-reconstruction` now falls back to `balanced-fast` unless this explicit RR gate is enabled and the legacy `FUN_DISABLE_DLSS_RR` kill switch is absent.
- `fun-bench run-stack`, `fun-bench client`, and the RR benchmark matrix lanes use `--enable-dx12-dlss-rr` for the explicit gate; `--disable-dlss-rr` remains the kill switch.
- `FUN_RENDER_DX12_DLSS_DEBUG=1`, `FUN_RENDER_DX12_DLSS_RESET=1`, and `FUN_RENDER_DX12_DLSS_SHARPNESS=<f32>` are parsed for future bridge use.
- `FUN_RENDER_DX12_DLSS_DEBUG_VIEW=depth|depth_histogram|motion|zero|overlarge|moving` selects the future DLSS debug visualization surface.

## Super Resolution Graph Insertion

- `fun_render::Dx12NativeDlssSrNode` runs in `Core3dSystems::EarlyPostProcess`, after the jittered main scene pass and before bloom, tonemapping, UI composition, and final upscaling.
- The node extracts per-view native SR state only when `Dx12NativeDlssCamera` is present, the camera is active and perspective, `FUN_RENDER_DX12_DLSS` is enabled, and `Dx12NativeDlssSrSupport` says SR is ready.
- `Dx12NativeDlssSrSupport` defaults to unsupported because there is not yet a Streamline/NGX bridge. That keeps the renderer at native scale instead of rendering a reduced-resolution viewport without a working evaluator.
- `Dx12NativeDlssSrOutput` allocates an intermediate output-resolution HDR-compatible texture with render-attachment, sampled, storage, copy-source, and copy-destination usage. The swapchain image is not used as the DLSS output.
- The node validates depth and motion-vector availability, checks the expected input/output resolution relationship, validates native texture handles through `fun_render::dx12_native` when compiled for Windows, logs the resource-state plan, and records structured failure codes.
- The current node falls back to a debug copy if evaluation fails, then disables the native SR path in render-world status after the failure budget is exhausted. Until support is reported ready, main-world camera setup removes native DLSS render-scale overrides and resets mip bias to native.
- Native SDK evaluation still returns `native_shim_unavailable`; this is intentional until Streamline/NGX calls and a command-list accessor are linked into `fun_dx12_dlss`.

## NATIVE_UI HUD Composition Boundary

`fun_render::FunRenderCompositionStage` records the combined renderer ordering
contract:

```text
WorldRender
  -> DepthMotionVectors
  -> SolariLighting
  -> DlssReconstruction
  -> PostProcessing
  -> HudUi
  -> DebugOverlays
  -> Present
```

The NATIVE_UI/Svelte surface maps to `HudUi`. It is sampled only after DLSS SR/RR and
post-processing have produced the visible world image. It must not be bound as
DLSS input color, depth, motion vectors, or Ray Reconstruction guide data.
Browser pixels have no world-space motion-vector contract, so treating them as
temporal input would contaminate SR/RR history.

## Runtime Robustness

- `Dx12NativeDlssSrRuntimeMode` models runtime transitions between `Disabled`, `NativeTaa`, `DLSS Quality`, `DLSS Balanced`, `DLSS Performance`, and `DLSS Ultra Performance`.
- Camera setup stores `Dx12NativeDlssCameraRuntimeState` and requests `DlssHistoryReset` on startup, resize, support changes, mode changes, render-scale changes, device recreation, and backend restart/device-loss signals.
- When native SR support is not ready, marked cameras run as `NativeTaa`: stale DLSS render-scale overrides are removed and mip bias is reset to native.
- Render-world output management recreates the intermediate output target when input size, output size, texture format, DLSS mode, runtime mode, or device generation changes.
- Resource recreation records a runtime transition, clears the native SR failure counter, marks native resize pending, and skips DLSS evaluation for one frame before falling back to a copy for that frame.
- Device recovery observation uses Bevy `RenderRecoveryStatus` plus render-device change tracking. On device loss or recreation, render-world native SR output components are removed and the status returns to pending support so stale D3D12 device, queue, resource, descriptor, or command-list pointers cannot be reused by future native code.
- Streamline/NGX context destruction, SDK-backed `fun_dlss_resize`, SDK support re-query, and feature-context recreation are still pending. The `fun_dx12_dlss` crate currently provides the ABI and fail-closed lifecycle entry points those hooks will consume.

## Ray Reconstruction Gate

- `Dx12NativeDlssRrStatus` is separate from SR status and defaults disabled.
- The native RR gate only enables when all of these are true:
  - `NativeDlssConfig.allow_ray_reconstruction` is true from `FUN_RENDER_DX12_DLSS_RR=1`.
  - native SR support is ready through `Dx12NativeDlssSrSupport::super_resolution_ready`.
  - the native support query reports `ray_reconstruction_supported=true`.
  - Solari guide resources are valid.
  - the current scene path supports the required guide data.
- Current default status is still rejected because native SR support is unsupported until the native bridge exists.
- `game_client` still contains the old Bevy camera-side `Dlss<DlssRayReconstructionFeature>` activation hook, but it no longer becomes reachable from the Solari RR denoise token alone.

## Solari RR Guide Surface Audit

The current known guide surfaces are represented in `fun_render::solari_rr_guide_surface_audit()`.

| surface | required | status | format | resolution | convention | lifetime/state |
| --- | --- | --- | --- | --- | --- | --- |
| `input_color` | yes | pending native bridge | view target format | native DLSS input | jittered HDR scene color before bloom/UI | post-process source, shader resource |
| `depth` | yes | present | `Depth32Float` | native DLSS input | reversed-Z, infinite far, non-linear | current view prepass depth, depth-read or shader resource |
| `motion_vectors` | yes | present | `Rg16Float` | native DLSS input | current-minus-previous normalized UV, jitter-excluded | current view prepass motion vectors, shader resource |
| `diffuse_albedo` | yes | present | `Rgba8Unorm` | native DLSS input | Solari resolved diffuse albedo | `ViewDlssRayReconstructionTextures`, storage write then shader resource |
| `specular_albedo` | yes | present | `Rgba8Unorm` | native DLSS input | Solari env-BRDF specular albedo | `ViewDlssRayReconstructionTextures`, storage write then shader resource |
| `normal_roughness` | yes | present | `Rgba16Float` | native DLSS input | world normal xyz, roughness w | `ViewDlssRayReconstructionTextures`, storage write then shader resource |
| `specular_motion_vectors` | yes | present | `Rg16Float` | native DLSS input | specular virtual-position motion in normalized UV | `ViewDlssRayReconstructionTextures`, storage write then shader resource |
| `bias` | yes | present | `Rgba8Unorm` | native DLSS input | current-color bias scalar | `ViewDlssRayReconstructionTextures`, storage write then shader resource |
| `exposure` | no | pending native bridge | SDK optional | 1x1 or view-dependent | Bevy view exposure | future bridge input |
| `ray_distance` | no | missing | not allocated | native DLSS input | world-space ray t | future Solari guide |
| `reactive_mask` | no | missing | not allocated | native DLSS input | SDK reactive mask | future transparency/particle guide |

All required present guide surfaces must still be validated at the native-DX12 handle, resource-state, dimension, camera-association, and history-reset boundary before RR can be enabled. The audit records existing Bevy/Solari resources; it is not an SDK support claim.

## Current RR Artifact Investigation

- Existing RR artifacts are treated as correctness failures, not post-process polish issues.
- Known symptoms to reproduce before re-enabling RR: a large black square or rectangle, and broken or missing Solari shadows.
- Highest-risk candidate causes:
  - guide resource dimensions do not match color/depth/motion-vector dimensions after render-scale changes;
  - guide formats or packed normal/roughness conventions do not match SDK expectations;
  - stale descriptors survive resize or device recovery;
  - guide textures are in storage-write state when the SDK expects shader-readable state;
  - history is not reset after scene load, world-stream hard swap, resize, mode switch, or Solari reset;
  - the RR evaluation node runs before `solari_lighting/dlss_rr_guide_resolve` completes;
  - camera/view entity mismatch between color, prepass, Solari lighting, and guide textures;
  - motion-vector or depth convention mismatch;
  - background or out-of-viewport guide regions are uninitialized.
- RR should not be reintroduced into runtime rendering until SR evaluation is working and a targeted trace proves guide resolve order, dimensions, states, and reset behavior are correct.

## RR Acceptance Criteria

`fun-bench client --require-dx12-dlss-rr-acceptance` is the machine gate for a future RR acceptance run. A passing run must use `--enable-dx12-dlss-rr`, select an RR Solari denoise mode, observe at least 500 estimated live frames, and record all required metrics.

Required metrics in `summary.json`:

- `dlss_rr_gpu_ns`
- `solari_pass_dlss_rr_guide_resolve_ns`
- `frame_ns.mean`
- `frame_ns.p95`

The acceptance command fails if the estimated stress-frame count is below `-RrStressFrameTarget` which defaults to `500`, if `FUN_DISABLE_DLSS_RR` is active, if the explicit RR opt-in is missing, if an RR Solari denoise mode is not selected, or if any required metric is absent.

## DX12 Native Interop Boundary

- `fun_render/src/dx12_native/handles.rs` and `fun_render/src/dx12_native/command_encoder.rs` are the only FUN-layer files allowed to call `Device::as_hal::<Dx12>()`, `Queue::as_hal::<Dx12>()`, `Texture::as_hal::<Dx12>()`, or `CommandEncoder::as_hal_mut::<Dx12>()`.
- `with_dx12_device_queue_checked` validates the active wgpu backend is DX12, extracts borrowed `ID3D12Device` and `ID3D12CommandQueue` pointers, checks for null pointers, and logs each failure category once.
- `docs/dx12_native_interop_governance.md` is the shared governance note for NATIVE_UI, DLSS, PIX naming, and future debug tooling.
- `extract_dx12_texture_handle` validates single-layer, non-MSAA, non-zero 2D textures and maps only the texture formats currently expected for DLSS inputs and outputs.
- The returned native pointers are borrowed. Rust-side `wgpu::Texture` resources must stay alive through native evaluation, and DLSS input and output resources must not alias during first bring-up.
- `with_dx12_command_list_checked` validates DX12 command encoder HAL availability but returns `command_list_unavailable` until a sanctioned raw `ID3D12GraphicsCommandList` accessor exists.

## Initial Resource State Plan

- Input HDR color: `shader_resource`.
- Depth: `depth_read`.
- Motion vectors: `shader_resource`.
- Optional exposure: `shader_resource`.
- Output color: `unordered_access` during DLSS evaluation, then restored to `render_target` for the current first-pass plan.
- The initial transition owner is `native_shim_conservative`; the native shim may insert conservative transitions before and after evaluation until the render graph owns tighter state control.

## Data Correctness Gate

- `fun_render::validate_camera_for_dx12_dlss(camera, world)` now returns `DlssCameraValidation` with `has_depth`, `has_motion_vectors`, `has_jitter`, `has_previous_view_projection`, `msaa_disabled`, `format_supported`, `resolution_supported`, and `history_valid`.
- `Dx12NativeDlssCamera` marks a camera for the future native path and requires `TemporalJitter`, `MipBias`, `DepthPrepass`, and `MotionVectorPrepass`.
- `fun_render` disables MSAA for marked native DLSS cameras. It applies `MainPassResolutionOverride` and mode-driven mip bias only when native SR support is ready; otherwise it removes stale render-scale overrides and resets mip bias to native.
- Mode scale factors are currently deterministic: Quality `2/3`, Balanced `0.58`, Performance `0.5`, Ultra Performance `1/3`; mip bias is `log2(internal_scale)`.
- Previous/current jittered and non-jittered view-projection matrices are tracked by `Dx12DlssPreviousViewProjection`; the first frame is marked as missing a valid previous matrix.
- `DlssHistoryReset` centralizes reset state and reasons. The current implemented reset request sources are startup/default, window resize/scale-factor change, Solari reset events, render-scale/mode changes from the native camera setup, and explicit `FUN_RENDER_DX12_DLSS_RESET`.
- Depth convention is recorded as Bevy/FUN reversed-Z, infinite far plane, non-linear depth.
- Motion-vector convention is recorded as Bevy prepass current-minus-previous normalized UV offset, low-resolution, jitter-excluded, with camera and object motion included.
- Debug visualization modes and depth diagnostic payload types exist, but GPU-side depth preview, min/max histogram, invalid-depth counter, motion heatmap, zero/overlarge masks, and moving-object overlays still need render-node/readback implementation.

## Places Not To Modify Yet

- Do not rewrite Bevy's `bevy_anti_alias::dlss` internals.
- Do not replace `dlss_wgpu`; keep it as the existing Vulkan/Bevy comparison and fallback lane.
- Do not remove the explicit Vulkan comparison lane; Windows now defaults to
  DX12, but `FUN_RENDER_BACKEND=vulkan` and `--render-backend vulkan` must remain
  available for comparison and fallback captures.
- Do not route DLSS work through NATIVE_UI/Svelte or `fun_ui_native_ui`.
- Do not re-enable RR through the native path until native Super Resolution is stable and benchmarked.
- Do not scatter `as_hal::<Dx12>()`, `as_hal_mut::<Dx12>()`, raw COM pointers, or resource-handle extraction across `fun_render`.
- Do not vendor NVIDIA redistributable binaries without an explicit SDK/license decision.

## Next Implementation Boundary

- Replace the current `fun_dx12_dlss` unsupported shim with Streamline or NGX calls after the DX12 baseline gate is ready.
- Consume `fun_render::dx12_native` from native integration code instead of adding new HAL extraction call sites.
- Add or expose a sanctioned raw `ID3D12GraphicsCommandList` accessor before attempting DLSS evaluation.
- Replace the current `native_shim_unavailable` stub in `Dx12NativeDlssSrNode` with a Streamline or NGX-backed `fun_dlss_evaluate` call.
- Wire the runtime robustness hooks to native `fun_dlss_destroy`, support re-query, `fun_dlss_resize`, and feature-context recreation once the bridge crate exists.
- Wire `Dx12NativeDlssRrStatus` to the future native support query and the Solari guide validation pass instead of toggling it manually.
- Add GPU-side debug visualization/readback passes for depth and motion-vector audits before enabling user-facing DLSS claims.
- Add a C ABI around POD descriptors and raw pointers only.
- Make SDK discovery explicit through environment variables or documented repo-local third-party SDK paths.
- Fail clearly when the native feature is enabled but required SDK headers or runtime DLLs are absent.
