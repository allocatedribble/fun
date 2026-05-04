# DX12 Parity Decision Pass

status: active
owner_repo: fun
scope: live pass-control checklist for the next DX12 parity implementation campaign

## Current Commit Baseline

| repo | commit | status |
| --- | --- | --- |
| project-FUN root | `b126d66` | umbrella baseline before CEF transport decision pass |
| fun | `6c99257` | CEF transport decision pass baseline before edits |
| bevy | `020d7d6` | root gitlink baseline before this pass |
| fun-warden | `b0aec72` | dependency baseline before this pass; checkout has unrelated local edits |

The unrelated `game_client/src/warden.rs` validation blocker has been cleared
against the current `fun-warden` API. The same pass also cleared default
`game_client`/`game_server` build blockers needed by the stack benchmark. No
renderer behavior was intentionally changed.

## Latest Local Evidence

| artifact | status | path |
| --- | --- | --- |
| selected parity matrix | measured | `target\dx12-parity\current\matrix.json` |
| selected parity summary | measured | `target\dx12-parity\current\summary.md` |
| parity dashboard | measured | `target\dx12-parity\current\dx12_parity_report.md` |
| parity dashboard JSON | measured | `target\dx12-parity\current\dx12_parity_report.json` |
| perf regression gate | measured_fail | `target\dx12-parity\current\dx12_perf_regression.md` |
| CEF transport matrix mode | measured | `scripts\benchmark_dx12_parity.ps1 -MatrixSize cef_transport -PlanOnly` |
| CEF accelerated live lane | measured_blocked | `target\benchmarks\client\20260504-005500-247\summary.json` |
| PIX barrier summary | blocked | `target\dx12-pix\barrier_summary.md` |
| pipeline cardinality report | measured | `target\dx12-pix\pipeline_cardinality_report.md` |

Selected local lanes were run at 1280x720 for `dx12` and `vulkan` across
`immediate`, `fifo`, and `auto_no_vsync`, plus CEF hidden, CEF CPU paint,
requested CEF D3D11On12, and clouds/Solari/meshlet toggle lanes. This is not a
full `-MatrixSize present` run and does not include PIX, PresentMon, or the
representative/cloud-heavy/stream-stress scene expansion.

## Hardware Lanes Available

| lane | status | notes |
| --- | --- | --- |
| local developer machine | measured | selected DX12/Vulkan matrix attached in `target\dx12-parity\current` |
| self-hosted dx12-perf runner | missing | workflow target exists for `self-hosted`, `windows`, `dx12-perf`; no current matrix artifact is attached |
| NVIDIA coverage | measured | local adapter reported NVIDIA GeForce RTX 2070 SUPER / driver `32.0.15.9636` in the dashboard |
| AMD coverage | unknown | no current vendor-specific parity artifact is attached |
| Intel coverage | unknown | no current vendor-specific parity artifact is attached |

## Required Evidence Still Missing

| evidence | status | required artifact |
| --- | --- | --- |
| current DX12 vs Vulkan parity JSON | measured | `target\dx12-parity\current\matrix.json` plus matched dashboard artifacts |
| present matrix | measured | selected immediate/fifo/auto-no-vsync lanes ran; full `-MatrixSize present` output is still missing |
| upload top-callsite table | measured | current `summary.json` files include render upload counters; top-callsite review is still pending |
| CEF accelerated health report | blocked | latest 1280x720 animated `d3d11on12` request selected CPU fallback with `fallback_reason=render_backend_not_dx12`, `bridge_ready=false`, `cef_cpu_upload_bytes.mean=46080000`, `cef_gpu_copy_bytes.mean=0`, `cef_on_accelerated_paint_fps.mean=0`, and health `fallback`; new `cef_ui_transport_health` badge and `-MatrixSize cef_transport` lanes are ready for the next live capture |
| PIX barrier summary | blocked | blocked artifact written to `target\dx12-pix\barrier_summary.md`; no PIX CSV was available locally |
| steady-state pipeline creation report | measured | `target\dx12-pix\pipeline_cardinality_report.md` reports render pipeline p95 `22`, compute pipeline p95 `82`, shader pipeline p95 `104`; shader creation families are led by PBR, Solari, meshlet, and UI pipelines |

## Doctrine Gate Checklist

| order | doctrine gate | current_status | evidence | blocker |
| ---: | --- | --- | --- | --- |
| 1 | Make DX12 observable. | measured | selected local matrix, dashboard, perf gate, upload/churn/command/shader/readback diagnostics exist | full scene/present expansion still missing |
| 2 | Remove obvious hot-path uploads. | measured | upload counters and selected matrix summaries exist | current top-callsite review still pending |
| 3 | Harden CEF GPU transport. | blocked | requested accelerated lane ran; health badge and ring-depth matrix are instrumented | runtime selected CPU fallback with `render_backend_not_dx12` and no accelerated paint callbacks |
| 4 | Reduce barriers, descriptors, and PSO churn. | blocked | pipeline cardinality report exists and `FUN_RENDER_PIPELINE_WARMUP=observed` is available | barrier cleanup still blocked on PIX CSV; layout canonicalization blocked on creation-event/PIX descriptor rows |
| 5 | Tune present pacing with evidence. | measured | selected immediate/fifo/auto-no-vsync lanes ran | full present matrix and PresentMon evidence missing |
| 6 | Centralize native DX12 interop. | measured | `fun_render::dx12_native` owns FUN-layer HAL extraction | raw command-list accessor still intentionally fails closed |
| 7 | Bring up DLSS Super Resolution. | blocked | fail-closed native DLSS scaffolding exists | `docs/dx12_dlss_boundary_gate.md` says baseline ready is `no` |
| 8 | Consider Ray Reconstruction. | blocked | RR remains explicitly gated | DLSS SR is not stable and baseline gate is not ready |

Allowed statuses: `missing`, `measured`, `optimized`, `blocked`,
`not_applicable`.

## Decision Log

| decision | status | current decision | next evidence |
| --- | --- | --- | --- |
| upload path decision | measured | no new upload optimization selected | current top ten upload callsites and CEF CPU upload isolation |
| CEF transport decision | blocked | keep accelerated path gated with CPU fallback; current D3D11On12 request falls back to CPU and is not healthy enough to default-on | run `-MatrixSize cef_transport`, screenshot diff, resize, alt-tab, and editor/launcher transition checks after DX12 bridge readiness is fixed |
| present default decision | measured | do not change defaults; selected matrix favors `auto_no_vsync` for mean FPS but lacks full present evidence | full present matrix with mean FPS, p95, and present-wait recommendations |
| barrier cleanup decision | blocked | no cleanup selected; local barrier artifact is blocked because no PIX CSV was available | PIX barrier/resource-state summary with named resources and transitions |
| PSO/churn decision | measured | runtime PSO churn is a current bottleneck candidate; use `FUN_RENDER_PIPELINE_WARMUP=observed` for the next measured lane before layout canonicalization | rerun benchmark after creation-focused churn rows are present, then compare before/after observed warmup |
| DLSS gate decision | blocked | DLSS SR remains fail-closed | boundary gate changes from baseline ready `no` to `yes` with attached evidence |

## Live Gate Controls

- PR metadata file: `.dx12_change_category`
- Doctrine checker: `tools\check_dx12_doctrine.ps1`
- Hardware-free validation: `tools\check_dx12_doctrine.ps1 -SelfTest`

Every later DX12 PR updates this document when it changes a gate status,
attaches a new evidence artifact, or makes one of the decision-log calls.
