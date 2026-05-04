# DX12 Command Submission Strategy

This note covers Tier 8 of the DX12 parity campaign. The first pass is
measurement-only for submission shape, plus explicit investigation gates for
upload batching, async copy, and async compute.

## 8.1 Command Counters

`scripts/run_stack.ps1 -RenderDiagnostics` enables:

- `FUN_RENDER_COMMAND_COUNTERS=1`
- `BEVY_RENDER_COMMAND_COUNTERS=1`

The local Bevy fork records:

- `command_encoder_creations`
- `render_passes`
- `compute_passes`
- `command_buffers_submitted`
- `queue_submits`
- `copy_commands`
- `native_interop_command_insertions`
- top command events by operation, category, and label

`game_client` emits parser-stable rows:

```text
[client perf] render commands: command_encoder_creations=... render_passes=... compute_passes=... command_buffers_submitted=... queue_submits=... copy_commands=... native_interop_command_insertions=... event_count=...
[client perf] render command top: rank=1 operation=compute_pass category=meshlets label=meshlet_first_instance_cull calls=...
```

`scripts/benchmark_client.ps1` stores the metrics under the
`render_command_*` prefix and writes `render_command_events` to `summary.json`.
`tools/dx12_parity_report.py` compares the same metrics between Vulkan and
DX12 and classifies higher DX12 submit or command-buffer counts as
`submission fragmentation`.

For the focused Tier 10 command/readback pass, generate the smaller decision
artifact with:

```powershell
python tools\dx12_command_readback_report.py `
  --matrix-json target\dx12-parity\current\matrix.json `
  --markdown-report target\dx12-parity\current\dx12_command_readback_report.md `
  --json-report target\dx12-parity\current\dx12_command_readback_report.json
```

That report is the gate for command-submission behavior changes. High submit or
command-buffer counts alone are not enough to merge passes, move copy work, or
batch a path. A change needs either a named actionable command event plus a
PIX/GPUView queue-idle span, or a before/after lane proving p95 and latency do
not regress. If the report says `candidate_needs_pix_before_behavior_change`,
leave runtime scheduling unchanged and attach the requested trace first.

Categories are intentionally coarse:

- `main_scene`
- `meshlets`
- `solari`
- `clouds`
- `post_process`
- `cef_copy`
- `ui_composition`
- `debug_overlay`
- `uploads`
- `readback`
- `other`

The first pass is not a full command encoder wrapper. It counts centralized
encoder creation and queue submission, tracked render passes, selected direct
pass/copy sites, and the CEF DX12 native interop insertion points. If a PIX
capture shows an important unlabeled pass under `other`, add a narrow label at
that pass instead of wrapping every wgpu command.

## 8.2 Upload Scheduler Gate

Do not move uploads yet. Use the Tier 3 upload counters and Tier 8 command
counters first. A render-stage upload scheduler is eligible only when a trace
shows upload work landing late on the critical path.

Proposed first implementation shape:

1. Collect upload and copy requests during extract/prepare.
2. Sort requests before their dependent render passes.
3. Batch compatible buffer uploads and texture copies.
4. Submit the batch at the earliest point wgpu allows without adding a fence
   wait.
5. Keep the existing direct upload path as fallback.

Acceptance requires before/after DX12 and Vulkan JSON, plus evidence that p95
does not regress.

## 8.3 Async Copy Gate

`BackendCapabilities` already reports async copy queue availability. The
experimental lane remains disabled until traces prove it helps.

Reserved policy variable:

```text
FUN_DX12_ASYNC_COPY=0|1|auto
```

Candidate work:

- CEF GPU frame copy
- large streaming texture uploads
- readback/capture copies
- large static asset upload

Do not use an async copy queue for tiny work. Keep disabled if PIX or GPUView
shows that synchronization costs replace the original copy cost.

## 8.4 Async Compute Gate

Async compute stays out of the default path until the graphics queue dependency
chain is understood. Candidate lanes are clouds, denoising, meshlet visibility,
post-process compute, and Solari guide resolve. A lane only survives if PIX
shows real queue overlap, `frame_ns.p95` improves, and Vulkan is not harmed by
shared scheduling changes.
