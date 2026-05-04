# DX12 Readback And Render Graph Diagnostics

This note covers the Tier 10/11 measurement slice for the Windows DX12
performance campaign. It does not change render pass ordering by itself.
Reordering or pass fusion needs PIX/benchmark evidence first.

## Readback Inventory

| source | cadence | category | blocking policy |
|---|---|---|---|
| `bevy_render::gpu_readback` | only entities with `Readback`; can be every frame if the component stays active | explicit GPU readback | nonblocking `map_async`; completions are consumed on later extract ticks |
| `bevy_render::view::window::screenshot` | capture-only | screenshot | nonblocking `map_async`; completion is awaited in the async task, outside the render hot path |
| `bevy_render::diagnostic::internal` | render diagnostics only | timestamp/query/value diagnostics | nonblocking query readback with submitted-frame reuse |
| `bevy_render::diagnostic::tracy_gpu` | Tracy GPU context initialization | timestamp | one explicit wait during Tracy GPU startup only |
| CEF GPU interop | UI transport only | native copy, not a readback | must not map to CPU in accelerated lanes |

Normal performance lanes should have zero blocking readback waits. Any active
readback, map, or device poll in a performance lane must be visible in benchmark
JSON so it cannot silently explain a DX12 p95 regression.

## Counters

Enable counters with:

```powershell
$env:FUN_RENDER_READBACK_DIAGNOSTICS = "1"
$env:BEVY_RENDER_READBACK_DIAGNOSTICS = "1"
```

`scripts/run_stack.ps1 -RenderDiagnostics` and `-FrameTimeDiagnostics` enable
these automatically.

Benchmark metrics use the `render_readback_` prefix:

- `render_readback_readback_requested_count`
- `render_readback_readback_completed_count`
- `render_readback_readback_dropped_count`
- `render_readback_readback_blocking_wait_count`
- `render_readback_readback_latency_frame_sum`
- `render_readback_readback_latency_frame_max`
- `render_readback_map_async_count`
- `render_readback_poll_count`
- `render_readback_event_count`

Top events are emitted as:

```text
[client perf] render readback top: rank=1 operation=requested category=gpu_readback label=gpu_readback_texture_to_buffer calls=1 latency_frame_sum=0 latency_frame_max=0
```

Use the focused Tier 10 report to prove readback behavior before changing
render scheduling:

```powershell
python tools\dx12_command_readback_report.py `
  --matrix-json target\dx12-parity\current\matrix.json `
  --markdown-report target\dx12-parity\current\dx12_command_readback_report.md `
  --json-report target\dx12-parity\current\dx12_command_readback_report.json
```

The readback acceptance line is `render_readback_readback_blocking_wait_count`.
Normal benchmark lanes must stay at zero. Diagnostic and capture lanes may have
large `requested`, `completed`, and `map_async` counts, but those rows must be
visible in the report so their overhead is not mistaken for product-lane render
cost. Required readback work should remain N-frame delayed unless a capture-only
path explicitly marks its overhead.

## Render Graph Flame Map

`scripts/benchmark_client.ps1` now writes a coarse frame artifact to:

```text
target/dx12/render_graph_frame_0000.json
```

The first version is a benchmark-summary flame map. It records pass-family
ordering, p95 timing metrics where available, coarse resource reads/writes, and
native interop/readback points. `barriers_known` is intentionally false until a
PIX barrier summary is imported.

The intended ordering remains:

```text
world render -> Solari/clouds/lighting -> post process -> CEF UI -> debug overlays -> present
```

CEF UI is modeled after post-processing and before debug/present. It must not
feed DLSS input color, motion vectors, depth, or Ray Reconstruction guide
buffers.

## Pass Cleanup Rule

Pass reordering or fusion should only land after the flame map and PIX data show
a concrete target, such as a tiny pass with disproportionate barriers,
descriptor churn, command buffer fragmentation, or readback/copy overhead. The
first diagnostic pass deliberately leaves render graph behavior unchanged.
