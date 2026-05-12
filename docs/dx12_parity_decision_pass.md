# DX12 Parity Decision Pass

status: active
owner_repo: fun
scope: live pass-control checklist for the next DX12 parity implementation campaign

## Current Commit Baseline

| repo | commit | status |
| --- | --- | --- |
| project-FUN root | `6b1c966` | umbrella baseline before meshlet/world-stream pressure pass |
| fun | `f90d367` | command/readback decision report baseline before Tier 11 edits |
| bevy | `020d7d6` | root gitlink baseline before this pass |
| fun-warden | `4faff82` | dependency baseline before this pass |

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
| present matrix plan | planned | `target\dx12-parity\present-plan-v2\matrix.json` |
| present smoke slice | measured | `target\dx12-parity\present-smoke-auto-no-vsync-fl2\matrix.json` |
| present smoke decision report | measured | `target\dx12-parity\present-smoke-auto-no-vsync-fl2\dx12_present_decision_report.md` |
| present decision report | measured | `target\dx12-parity\current\dx12_present_decision_report.md` |
| transient reuse report | measured | `target\dx12-parity\current\dx12_transient_reuse_report.md` |
| command/readback report | measured | `target\dx12-parity\current\dx12_command_readback_report.md` |
| command/readback smoke report | measured | `target\dx12-parity\present-smoke-auto-no-vsync-fl2\dx12_command_readback_report.md` |
| stream-pressure matrix mode | measured | `target\dx12-parity\stream-pressure-plan-validation\matrix.json` |
| stream-pressure decision report smoke | measured | `target\dx12-parity\current\dx12_meshlet_stream_pressure_report.md` |
| stream-pressure DX12/Vulkan control smoke | measured | `target\dx12-parity\stream-pressure-smoke-capture-from-start\dx12_meshlet_stream_pressure_report.md` |
| NATIVE_UI transport matrix mode | measured | `fun-bench dx12-parity --matrix-size native_ui_transport --plan-only` |
| NATIVE_UI accelerated live lane | measured_blocked | `target\benchmarks\client\20260504-005500-247\summary.json` |
| Pass 9 backend truth smoke | measured_fail | `target\benchmarks\client\20260506-032407-482\summary.json` |
| Pass 9 capability report | measured_fail | `target\run-stack\renderer-capabilities.json` |
| PIX barrier summary | blocked | `target\dx12-pix\barrier_summary.md` |
| pipeline cardinality report | measured | `target\dx12-pix\pipeline_cardinality_report.md` |

Selected local lanes were run at 1280x720 for `dx12` and `vulkan` across
`immediate`, `fifo`, and `auto_no_vsync`, plus NATIVE_UI hidden, NATIVE_UI CPU paint,
requested NATIVE_UI D3D11On12, and clouds/Solari/meshlet toggle lanes. This is not a
full `--matrix-size present` run and does not include PIX, PresentMon, or the
representative/cloud-heavy/stream-stress scene expansion.

The present matrix definition now includes the required decision scenarios:
`ui_hidden`, `ui_accelerated`, `representative_gameplay`, and
`solari_cloud_heavy`. The generated plan defines 128 lanes: required
`immediate`, `auto_no_vsync`, and `fifo` lanes across Vulkan/DX12, frame
latency `1..4`, and all four scenarios, plus optional `auto_vsync` lanes. A
short live DX12 smoke slice covered all four scenarios for
`auto_no_vsync`/frame-latency `2`; all four lanes passed, but
`present_wait_ns` remained absent and the accelerated NATIVE_UI lane selected CPU
fallback with `native_ui_cpu_upload_bytes.mean=44236800`.

Pass 9 adds a stricter backend truth contract. The latest short smoke requested
and selected DX12, but the runtime capability report records
`actual_graphics_backend=vulkan`, `fallback_graphics_backend=vulkan`,
`graphics_backend_truth_state=actual_backend_mismatch`, and
`premium_rendering_gate=blocked_backend_mismatch`. The benchmark wrapper timed
out before a normal sample-window closeout, but the runtime log and capability
report were harvested through the parser-only benchmark path into
`target\benchmarks\client\20260506-032407-482\summary.json`. This is now a hard
foundation blocker for NATIVE_UI GPU transport, native DX12 interop, DLSS, FSR, and
frame generation claims.

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
| present matrix | measured | selected immediate/fifo/auto-no-vsync lanes ran; required scenario plan exists; DX12 smoke slice ran all four scenarios at `auto_no_vsync`/latency `2` |
| upload top-callsite table | measured | current `summary.json` files include render upload counters; top-callsite review is still pending |
| command/readback decision report | measured | current report says `candidate_needs_pix_before_behavior_change` for command submission and `nonblocking_proven` for readback; no submit reduction is selected |
| meshlet/world-stream pressure report | measured | DX12/Vulkan control smoke exists at `target\dx12-parity\stream-pressure-smoke-capture-from-start\dx12_meshlet_stream_pressure_report.md`; full budget/chunk-cap expansion and matched before/after baseline remain missing |
| NATIVE_UI accelerated health report | blocked | latest present smoke `d3d11on12` request selected CPU fallback with `native_ui_cpu_upload_bytes.mean=44236800`, `native_ui_gpu_copy_bytes.mean=0`, `native_ui_on_accelerated_paint_fps.mean=0`, and health `fallback`; Pass 9 also shows requested DX12 resolving to actual Vulkan in the capability report; new `native_ui_transport_health` badge and `--matrix-size native_ui_transport` lanes are ready for the next live capture |
| PIX barrier summary | blocked | blocked artifact written to `target\dx12-pix\barrier_summary.md`; no PIX CSV was available locally |
| steady-state pipeline creation report | measured | `target\dx12-pix\pipeline_cardinality_report.md` reports render pipeline p95 `22`, compute pipeline p95 `82`, shader pipeline p95 `104`; shader creation families are led by PBR, Solari, meshlet, and UI pipelines |

## Doctrine Gate Checklist

| order | doctrine gate | current_status | evidence | blocker |
| ---: | --- | --- | --- | --- |
| 1 | Make DX12 observable. | measured_fail | selected local matrix, dashboard, perf gate, upload/churn/command/shader/readback diagnostics, focused command/readback decision report, and Pass 9 backend truth fields exist | latest requested-DX12 lane reports actual Vulkan; full scene/present expansion still missing |
| 2 | Remove obvious hot-path uploads. | measured | upload counters and selected matrix summaries exist | current top-callsite review still pending |
| 3 | Harden NATIVE_UI GPU transport. | blocked | requested accelerated lane ran; health badge and ring-depth matrix are instrumented; backend truth fields now flow into NATIVE_UI status and benchmark summaries | runtime selected CPU fallback with `render_backend_not_dx12`, no accelerated paint callbacks, and latest requested-DX12 smoke actual backend is Vulkan |
| 4 | Reduce barriers, descriptors, and PSO churn. | blocked | pipeline cardinality report exists and `FUN_RENDER_PIPELINE_WARMUP=observed` is available | barrier cleanup still blocked on PIX CSV; layout canonicalization blocked on creation-event/PIX descriptor rows |
| 5 | Tune present pacing with evidence. | measured | selected immediate/fifo/auto-no-vsync lanes ran; required scenario plan, DX12 smoke slice, and decision report exist | full live present matrix and PresentMon/GPUView evidence missing |
| 6 | Centralize native DX12 interop. | measured | `fun_render::dx12_native` owns FUN-layer HAL extraction | raw command-list accessor still intentionally fails closed |
| 7 | Bring up DLSS Super Resolution. | blocked | fail-closed native DLSS scaffolding exists | `docs/dx12_dlss_boundary_gate.md` says baseline ready is `no` |
| 8 | Consider Ray Reconstruction. | blocked | RR remains explicitly gated | DLSS SR is not stable and baseline gate is not ready |

Allowed statuses: `missing`, `measured`, `optimized`, `blocked`,
`not_applicable`.

## Decision Log

| decision | status | current decision | next evidence |
| --- | --- | --- | --- |
| upload path decision | measured | no new upload optimization selected | current top ten upload callsites and NATIVE_UI CPU upload isolation |
| backend truth decision | measured_fail | requested/selected DX12 is not sufficient evidence; current local smoke is actual Vulkan with `actual_backend_mismatch`, so premium rendering remains blocked | produce a live artifact with `actual_graphics_backend=dx12`, `fallback_graphics_backend=none`, and `dx12_native_interop_support=supported` |
| NATIVE_UI transport decision | blocked | keep accelerated path fail-closed; current D3D11On12 request cannot be treated as healthy while backend truth reports actual Vulkan or bridge readiness reports `render_backend_not_dx12` | run `--matrix-size native_ui_transport`, screenshot diff, resize, alt-tab, and editor/launcher transition checks after DX12 bridge readiness is fixed |
| present default decision | measured | do not change defaults; the decision report requires a complete live scenario matrix plus latency evidence before recommending a default change | full present matrix with mean FPS, p95, and present-wait recommendations |
| barrier cleanup decision | blocked | no cleanup selected; local barrier artifact is blocked because no PIX CSV was available | PIX barrier/resource-state summary with named resources and transitions |
| PSO/churn decision | measured | runtime PSO churn is a current bottleneck candidate; use `FUN_RENDER_PIPELINE_WARMUP=observed` for the next measured lane before layout canonicalization | rerun benchmark after creation-focused churn rows are present, then compare before/after observed warmup |
| transient reuse decision | measured | no transient descriptor canonicalization selected; current matrix has no descriptor-create rows, and native interop resources remain excluded from aliasing | rerun a transient-focused lane with `bevy_render::transient=debug` before normalizing another texture family |
| command/readback decision | measured | command counters are high but no submit reduction is selected without PIX/GPUView queue-idle evidence; readback is nonblocking with p95 blocking waits at `0` in current and present-smoke lanes | attach PIX/GPUView queue-idle span before moving copy-only work or merging tiny passes; keep diagnostic readback overhead visible |
| meshlet/world-stream pressure decision | measured | control smoke reports `reallocations_disappeared`, no render-prep limiter, DX12 `frame_ns.p95=102210000`, Vulkan `frame_ns.p95=27000000`, and no matched baseline for before/after improvement claims; remaining top upload offenders are generic Bevy texture/uniform/buffer uploads, not semantic meshlet stream labels | run the full budget/chunk-cap expansion and attach a matched baseline before promoting tuning defaults |
| DLSS gate decision | blocked | DLSS SR remains fail-closed | boundary gate changes from baseline ready `no` to `yes` with attached evidence |

## Live Gate Controls

- PR metadata file: `.dx12_change_category`
- Doctrine checker: `fun-bench dx12-doctrine-check`
- Hardware-free validation: `fun-bench dx12-doctrine-check --self-test`
- Rust command: fun-bench dx12-doctrine-check

Every later DX12 PR updates this document when it changes a gate status,
attaches a new evidence artifact, or makes one of the decision-log calls.

## Pass 9 Backend Truth Fields

The renderer capability report and benchmark summaries now include:

- `requested_graphics_backend`
- `selected_graphics_backend`
- `actual_graphics_backend`
- `fallback_graphics_backend`
- `graphics_backend_selection_reason`
- `graphics_backend_truth_state`
- `dx12_native_interop_support`
- `premium_rendering_gate`
- `backend_parity_scene_ids`

The NATIVE_UI transport status JSON mirrors the same backend truth fields so a UI
lane cannot claim D3D11On12 readiness while the renderer is actually Vulkan or
any other non-DX12 backend.
