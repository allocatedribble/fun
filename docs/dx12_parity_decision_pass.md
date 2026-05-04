# DX12 Parity Decision Pass

status: active
owner_repo: fun
scope: live pass-control checklist for the next DX12 parity implementation campaign

## Current Commit Baseline

| repo | commit | status |
| --- | --- | --- |
| project-FUN root | `99fb494` | umbrella baseline before this pass |
| fun | `bfdc649` | doctrine baseline before this pass |
| bevy | `020d7d6` | root gitlink baseline before this pass |
| fun-warden | `1c267de` | checkout has uncommitted variant/trust changes |

`docs/dx12_dlss_boundary_gate.md` still records `game_client` validation as
blocked by an unrelated `warden.rs` API mismatch. This pass did not rerun
`game_client` validation, so the mismatch remains `blocked_pending_recheck` for
the DX12 campaign.

## Hardware Lanes Available

| lane | status | notes |
| --- | --- | --- |
| local developer machine | measured | Windows developer machine can run parser/self-test lanes; current live DX12/Vulkan hardware JSON is not attached |
| self-hosted dx12-perf runner | missing | workflow target exists for `self-hosted`, `windows`, `dx12-perf`; no current matrix artifact is attached |
| NVIDIA coverage | unknown | no current vendor-specific parity artifact is attached |
| AMD coverage | unknown | no current vendor-specific parity artifact is attached |
| Intel coverage | unknown | no current vendor-specific parity artifact is attached |

## Required Evidence Still Missing

| evidence | status | required artifact |
| --- | --- | --- |
| current DX12 vs Vulkan parity JSON | missing | `target\benchmarks\dx12_parity\<timestamp>\matrix.json` plus matched dashboard |
| present matrix | missing | `scripts\benchmark_dx12_parity.ps1 -MatrixSize present -ContinueOnFailure` output |
| upload top-callsite table | missing | `render_upload_callsites` from a current `-RenderDiagnostics` run |
| CEF accelerated health report | missing | `cef_gpu_accelerated` lane with `cef_cpu_upload_bytes=0`, accelerated paint FPS, GPU copy counters, and no normal-frame blocking waits |
| PIX barrier summary | missing | filled `docs/dx12_pix_barrier_audit.md` summary or attached PIX CSV fields |
| steady-state pipeline creation report | missing | churn counters showing steady-state render/compute/shader pipeline creation is zero or justified |

## Doctrine Gate Checklist

| order | doctrine gate | current_status | evidence | blocker |
| ---: | --- | --- | --- | --- |
| 1 | Make DX12 observable. | measured | parity dashboard, perf gate, upload/churn/command/shader/readback diagnostics exist | current hardware parity JSON missing |
| 2 | Remove obvious hot-path uploads. | measured | upload counters and upload audit exist | current top-callsite table missing |
| 3 | Harden CEF GPU transport. | blocked | accelerated transport and shared native boundary exist | current health report and `game_client` validation blocked pending warden mismatch recheck |
| 4 | Reduce barriers, descriptors, and PSO churn. | measured | churn/transient/command/readback diagnostics exist | PIX barrier summary and cleanup decisions missing |
| 5 | Tune present pacing with evidence. | missing | present matrix script exists | current present matrix artifact missing |
| 6 | Centralize native DX12 interop. | measured | `fun_render::dx12_native` owns FUN-layer HAL extraction | raw command-list accessor still intentionally fails closed |
| 7 | Bring up DLSS Super Resolution. | blocked | fail-closed native DLSS scaffolding exists | `docs/dx12_dlss_boundary_gate.md` says baseline ready is `no` |
| 8 | Consider Ray Reconstruction. | blocked | RR remains explicitly gated | DLSS SR is not stable and baseline gate is not ready |

Allowed statuses: `missing`, `measured`, `optimized`, `blocked`,
`not_applicable`.

## Decision Log

| decision | status | current decision | next evidence |
| --- | --- | --- | --- |
| upload path decision | missing | no new upload optimization selected | current top ten upload callsites and CEF CPU upload isolation |
| CEF transport decision | blocked | keep accelerated path gated with CPU fallback | health report proving accelerated paint/copy counters and zero normal-frame waits |
| present default decision | missing | do not change defaults | present matrix with mean FPS, p95, and present-wait recommendations |
| barrier cleanup decision | missing | no cleanup selected | PIX barrier/resource-state summary |
| PSO/churn decision | missing | no pipeline/layout cleanup selected | steady-state churn counters and top creation events |
| DLSS gate decision | blocked | DLSS SR remains fail-closed | boundary gate changes from baseline ready `no` to `yes` with attached evidence |

## Live Gate Controls

- PR metadata file: `.dx12_change_category`
- Doctrine checker: `tools\check_dx12_doctrine.ps1`
- Hardware-free validation: `tools\check_dx12_doctrine.ps1 -SelfTest`

Every later DX12 PR updates this document when it changes a gate status,
attaches a new evidence artifact, or makes one of the decision-log calls.
