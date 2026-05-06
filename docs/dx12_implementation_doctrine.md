# DX12 Implementation Doctrine

status: active
owner_repo: fun
scope: Windows DX12 parity, CEF transport, native interop, and DLSS sequencing

## Strategy

The DX12 path should become boring before it becomes ambitious. Do not add
native Windows calls just because the backend is DX12. Native calls are allowed
only when measurement proves the bottleneck or when a feature such as CEF shared
textures or DLSS requires a narrow interop boundary.

The target baseline is:

- fewer uploads;
- fewer barriers;
- fewer descriptors;
- fewer runtime PSOs;
- fewer waits;
- clearer present pacing;
- one well-policed native interop gate.

DLSS, Ray Reconstruction, bindless-style tables, GPU-driven visibility, and
other DX12-only features must ride on that baseline. They must not become the
explanation for why the base DX12 path is slow.

## Required Order

| order | work | exit gate |
| ---: | --- | --- |
| 1 | Make DX12 observable. | Parity JSON, dashboard, regression gate, upload/churn/command/readback counters, and external trace hooks exist for the lane being changed. |
| 2 | Remove obvious hot-path uploads. | Top `RenderQueue::write_*` callsites are measured, CEF CPU upload bytes are isolated, and small-buffer/large-texture fixes have before/after Vulkan and DX12 evidence. |
| 3 | Harden CEF GPU transport. | Accelerated CEF reports real `OnAcceleratedPaint` cadence, GPU copy bytes/time, zero normal-frame blocking waits, and safe CPU fallback. |
| 4 | Reduce barriers, descriptors, and PSO churn. | PIX/resource-state evidence, render churn counters, and steady-state pipeline creation counters prove the target issue and the fix. |
| 5 | Tune present pacing with evidence. | Present matrix and frame-latency lanes identify the best DX12 present policy for mean FPS, p95, and latency without hiding render-pass losses. |
| 6 | Centralize native DX12 interop. | All wgpu HAL extraction stays inside `fun_render::dx12_native` until the backend abstraction moves to `fun-renderer`; CEF, DLSS, and debug tooling call that boundary instead of opening new trapdoors. |
| 7 | Bring up DLSS Super Resolution. | `docs/dx12_dlss_boundary_gate.md` says the baseline is ready, SR is fail-closed, and CEF/UI composition stays after temporal reconstruction. |
| 8 | Consider Ray Reconstruction. | SR is stable first; RR guide surfaces, history resets, Solari data, and stress-scene evidence are valid before user-facing claims. |

Moonshot experiments remain after this sequence. They are not permitted to
preempt measurement, CEF hardening, upload cleanup, or the shared native
interop gate.

## Anti-Patterns

- Adding random native D3D12 calls outside the shared interop module or the
  future `fun-renderer` backend abstraction.
- Using DLSS to mask CPU upload, present pacing, descriptor churn, or barrier
  regressions.
- Treating the Svelte RAF badge as evidence of CEF paint or GPU transport.
- Claiming DX12 parity from average FPS while p95, present wait, or upload bytes
  regress.
- Optimizing DX12 by regressing Vulkan or removing the Vulkan control lane.
- Letting CEF UI feed DLSS input color, depth, motion vectors, exposure, or Ray
  Reconstruction guide buffers.

## Evidence Contract

Every DX12 performance patch states one category:

- `measurement-only`
- `upload-cleanup`
- `cef-transport`
- `barrier-state`
- `descriptor-pso-churn`
- `present-pacing`
- `native-interop`
- `dlss-sr`
- `dlss-rr`
- `moonshot`

The category lives in `.dx12_change_category` as the first non-comment line.
`fun-bench dx12-doctrine-check` validates the category, required gate files,
accelerated CEF CPU-upload evidence when benchmark summaries are supplied, and
the DX12 native interop boundary. `tools\check_dx12_doctrine.ps1` remains a
temporary compatibility wrapper around the Rust rule engine.

Every non-measurement category includes:

- before and after DX12 JSON;
- a Vulkan control lane unless the change is Windows-only interop and Vulkan is
  explicitly unaffected;
- matching hardware, resolution, present mode, CEF visibility, and build
  profile;
- the relevant dashboard or trace artifact;
- a statement that no new native interop path bypasses
  `fun_render::dx12_native`.
