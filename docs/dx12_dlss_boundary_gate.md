# DX12 DLSS Boundary Gate

status: active
owner_repo: fun
scope: native DX12 DLSS Super Resolution bring-up gate

DX12 baseline ready for DLSS SR bring-up: no

## Gate Conditions

| gate | status | evidence source |
| --- | --- | --- |
| DX12 vs Vulkan parity report exists | partial | selected local report exists at `target\dx12-parity\current\dx12_parity_report.md`; full scene/present coverage is not complete |
| Present-mode matrix complete | partial | selected immediate/fifo/auto-no-vsync lanes ran; full `--matrix-size present` output is not attached |
| CEF accelerated path health is isolated | blocked | latest 1280x720 animated `d3d11on12` request selected CPU fallback with `fallback_reason=render_backend_not_dx12`, `bridge_ready=false`, nonzero CPU upload bytes, and no accelerated paint callbacks; the health badge and CEF transport matrix are instrumented for the next live proof |
| Hot upload callsites identified | partial | current matrix summaries include upload counters; top-callsite review is not attached |
| Barrier audit complete | not_ready | `target\dx12-pix\barrier_summary.md` exists but is blocked because no PIX CSV/capture rows are attached |
| Steady-state pipeline creation mostly eliminated | not_ready | `target\dx12-pix\pipeline_cardinality_report.md` reports render pipeline p95 `22`, compute pipeline p95 `82`, shader pipeline p95 `104`; `FUN_RENDER_PIPELINE_WARMUP=observed` is the next measured lane, not proof of elimination |
| Native DX12 handle boundary exists | ready | `fun_render::dx12_native` owns FUN-layer wgpu HAL extraction |
| Native DLSS shim boundary exists | scaffolded | `fun_dx12_dlss` exposes the C ABI and fail-closed support query; Streamline/NGX integration is not linked |

## Policy

Native DLSS SR must not be used to hide base DX12 losses. Until this gate says
`yes`, DLSS SR work is limited to:

- fail-closed SDK/runtime discovery;
- C ABI and native shim scaffolding;
- Rust resource/camera/data-correctness validation;
- render-order tests and docs proving UI is after temporal reconstruction;
- support-query diagnostics that clearly report unsupported state.

No pass may claim a DX12 FPS improvement from DLSS until the baseline evidence
above is attached and the gate is updated.

This gate follows [`dx12_implementation_doctrine.md`](dx12_implementation_doctrine.md):
DX12 must first be observable, upload-cleaned, CEF-transport-hardened,
barrier/descriptor/PSO-audited, present-paced with evidence, and routed through
the centralized native interop boundary. DLSS Super Resolution comes after that
baseline; Ray Reconstruction comes after SR is stable.

## Runtime Opt-In

The future SR path is requested with either env spelling:

```text
FUN_DX12_DLSS=1
FUN_DX12_DLSS_MODE=quality
```

The existing explicit renderer-prefixed spelling remains supported:

```text
FUN_RENDER_DX12_DLSS=1
FUN_RENDER_DX12_DLSS_MODE=quality
```

If unsupported, the app must start, keep native SR disabled, and log a clean
fallback reason. The current reason is `runtime_dll_not_found` or
`native_streamline_or_ngx_sdk_not_linked` depending on local SDK/runtime
discovery.

## Render Order

DLSS SR is a world-image reconstruction pass. CEF/Svelte and debug overlays are
not temporal inputs.

```text
world render
  -> depth/motion vectors
  -> Solari/clouds/lighting
  -> DLSS SR if active
  -> bloom/tonemap/post
  -> CEF UI composition
  -> debug overlays
  -> present
```

CEF UI must not be bound as DLSS input color, motion vectors, depth, exposure,
reactive mask, or Ray Reconstruction guide data. The UI remains an
output-resolution composition layer.
