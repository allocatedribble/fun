# DX12 Moonshot Experiments Gate

status: active
owner_repo: fun
scope: Tier 19 post-parity DX12 renderer experiments

## Policy

These experiments are deliberately not first-wave fixes. They become candidates
only after the normal parity campaign has controlled:

- measurement and regression gates;
- upload hot paths;
- CEF accelerated transport;
- barrier/resource-state churn;
- descriptor and pipeline churn.

They also come after the required sequence in
[`dx12_implementation_doctrine.md`](dx12_implementation_doctrine.md): observable
DX12, upload cleanup, hardened CEF GPU transport, barrier/descriptor/PSO
cleanup, evidence-based present pacing, centralized native interop, DLSS SR, and
only then RR or moonshot work.

No moonshot may replace the current Vulkan control lane, hide DX12 parity
failures, or become default without before/after benchmark evidence.

## Experiments

| experiment | idea | gate | first evidence |
| --- | --- | --- | --- |
| GPU-driven visibility pipeline refinement | push more culling, compaction, and draw preparation onto GPU | CPU-side meshlet prep or upload pressure is confirmed | `meshlet_prepare_cpu_ns`, meshlet upload bytes, indirect/compaction writes, PIX queue evidence |
| Bindless-style material resource table | move material/texture resources into larger indexed tables | descriptor churn is confirmed | `render_churn_bind_group*` deltas or PIX descriptor heap switch counters |
| DX12-specific render graph compiler | coalesce tiny passes, minimize state transitions, and schedule copies earlier | barrier/state churn, submission fragmentation, or tiny pass overhead is confirmed | PIX barrier counts, command-buffer/submit deltas, render graph flame map |
| Native DX12 residency and memory budget diagnostics | expose DXGI/D3D12 budget and usage telemetry | p95 spikes, transient allocation churn, or VRAM pressure remain unexplained | `dx12_memory` summary block, PIX/RMV/Nsight memory evidence |

## Benchmark Integration

`fun-data report dx12-parity` emits:

- `DX12 Memory Budget`: local budget, usage, reservation, adapter RAM, source,
  and status when available.
- `Moonshot Experiments`: eligibility status, required gate, current evidence,
  and expected payoff.

`fun-bench client` and `fun-bench dx12-parity` include a
normalized `dx12_memory` block. Today the block records adapter RAM from WMI and
accepts native/DXGI budget samples when a runner provides these environment
variables:

```text
FUN_BENCH_DX12_LOCAL_BUDGET_BYTES
FUN_BENCH_DX12_LOCAL_USAGE_BYTES
FUN_BENCH_DX12_LOCAL_AVAILABLE_FOR_RESERVATION_BYTES
FUN_BENCH_DX12_LOCAL_CURRENT_RESERVATION_BYTES
```

This keeps the report schema stable before the native DXGI query lands. The
future native collector should write the same fields, not invent a parallel
memory schema.

## Acceptance

```text
moonshot_default_enabled: false
moonshot_requires_confirmed_bottleneck: true
benchmark_report_includes_memory_budget_when_available: true
vulkan_control_lane_required: true
```
