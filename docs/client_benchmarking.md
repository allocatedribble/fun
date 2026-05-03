# Client Benchmarking Standard

Every client-facing change must be benchmarkable. Rendering, movement,
networking, asset streaming, UI, and gameplay changes should leave behind
numbers that can be compared across commits. Use Criterion for deterministic
code-path costs and the runtime client benchmark for end-to-end FPS/GPU pass
cost.

## Required Evidence

For any client change, capture a before and after benchmark on the same machine,
same backend, same present mode, same map, and same camera/player state whenever
possible. Do not claim a change is faster, smoother, or cheaper without a
benchmark delta.

At minimum, report:

- Git commit or dirty worktree state for both baseline and candidate.
- GPU, driver, CPU, render backend, present mode, profile, and Solari denoise
  mode.
- Sample duration and sample count.
- Criterion baseline/candidate names for code-path benchmarks.
- FPS mean, p50, and p95.
- Frame time mean and p95 in nanoseconds.
- Relevant GPU pass mean and p95 in nanoseconds.
- Criterion mean and regression/improvement for touched systems.
- Any known visual tradeoff, artifact, or test limitation.

Frame time and GPU pass time are the primary performance truth. FPS is reported
because it is easy to read, but pass-level nanoseconds explain why a change got
better or worse.

Criterion is the standard for CPU, scheduling, setup, serialization,
prediction, relevance, streaming, and pure math costs. It is not the sole source
of truth for GPU work: Solari, meshlets, DLSS RR, and compute denoisers must
also be measured through render diagnostics because GPU dispatch is asynchronous
and pass cost depends on resolution, scene, driver, and queue behavior.

## Criterion Command

Capture a named Criterion baseline before a change:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark_criterion.ps1 -SaveBaseline before
```

Compare the candidate against that baseline:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark_criterion.ps1 -Baseline before -SaveBaseline after
```

For fast compile/smoke validation while editing benchmark code:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark_criterion.ps1 -SaveBaseline smoke -WarmupSeconds 0.1 -MeasurementSeconds 0.2 -SampleSize 10
```

The first Criterion suite lives at `game_client/benches/client_costs.rs` and
covers:

- Solari setup and denoiser/RR mode selection.
- First-person movement math.
- Quantized physics state conversion and prediction error.
- Thunder packet encode/decode for input and streamed worlds.
- Streaming manifest planning and cache directives.
- Network relevance selection for large object counts.

Criterion writes its HTML and raw estimates under `target\criterion`.

## Runtime Benchmark Command

Use the client benchmark script for repeatable captures:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark_client.ps1
```

The script starts the server and client through `scripts\run_stack.ps1`, enables
render diagnostics, warms up, samples the client log, stops the stack, and writes
both files below:

- `target\benchmarks\client\<timestamp>\summary.json`
- `target\benchmarks\client\<timestamp>\summary.md`

Required 144 FPS lanes are run through the same script, not a separate
measurement universe:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark_required_lanes.ps1 -RenderBackend dx12 -PresentMode immediate
```

Each lane writes its own `summary.json`:

- `full_runtime`: Solari plus meshlets plus normal gameplay.
- `solari_floor`: Solari disabled only to expose the non-Solari floor.
- `meshlet_floor`: meshlets disabled only to expose meshlet cost.
- `cpu_floor`: tiny-window workload to expose CPU, scheduling, physics, and networking.
- `streaming_spike`: startup/chunk-apply lane with no warmup so stream p95 is visible.
- `presentation_floor`: optional overlay/presentation cost isolated from the normal path.

Performance claims must include `frame_ns.mean/p95`,
`meshlet_visibility_gpu_ns.mean/p95`, `meshlet_extract_cpu_ns`,
`meshlet_prepare_cpu_ns`, `world_stream_apply_cpu_ns`,
`physics_fixed_update_cpu_ns`, `post_process_gpu_ns`, `present_wait_ns`,
`cloud_raymarch_gpu_ns.mean/p95`, `cloud_temporal_gpu_ns.mean/p95`,
`cloud_resolve_gpu_ns.mean/p95`, `cloud_composite_gpu_ns.mean/p95`,
`cloud_shape_noise_gpu_ns.mean/p95`, `cloud_total_gpu_ns.mean/p95`,
`cloud_weather_update_gpu_ns.mean/p95`, `cloud_weather_update_cpu_ns`,
`cloud_internal_width`, `cloud_internal_height`, `cloud_primary_steps`, `cloud_light_steps`,
`cloud_history_accept_rate`, `cloud_history_reject_rate`,
`cloud_history_reset_count`, `cloud_history_average_age`,
`cloud_weather_profile_id`, `cloud_quality`, `cloud_vram_bytes`,
`meshlet_path_instance_count`, `raster_path_instance_count`,
`ray_proxy_only_count`, `standard_raster_gpu_ns`, transient render-resource
request/create/reuse/alias counts, and render scheduler pressure when
`-TraceDiagnostics` is enabled. Client CPU schedule work must also report the
`schedule_*` metrics for networking receive, world-stream apply, movement input,
look, physics movement, diagnostics logging, render config/window work, Solari
runtime params, meshlet extraction, and render interpolation. Meshlet hot-path
changes must report `meshlet_instance_full_buffer_writes`,
`meshlet_instance_range_buffer_writes`, `meshlet_material_full_buffer_writes`,
`meshlet_material_range_buffer_writes`, and
`meshlet_view_visibility_buffer_writes`. Meshlet material-queue changes must
also report `meshlet_material_queue_cpu_ns` and
`meshlet_material_queue_dirty_instance_count`. Per-view meshlet resource reset
changes must report `meshlet_view_reset_cpu_queue_writes`,
`meshlet_view_reset_cpu_queue_writes_per_view`, and `meshlet_view_count`.
When frame attribution is needed, run with `-FrameTimeDiagnostics`; this enables
the debug-only `game_client/render_diagnostics` feature and writes a per-frame
hierarchical `fun::frame_time` report with thread buckets, function names,
file/line callsites, inclusive ns, `self_ns`, and child percentages. Diagnostic
features are intentionally not compiled into release builds. Add
`-FrameTimeDiagnosticRowEvents` when the benchmark parser should consume one
structured tracing event per thread, slow span, and summary row instead of the
single multiline report.
The benchmark report includes a 144 FPS budget ledger and marks p95 pass/fail
for the buckets that are currently measurable.

## DX12 Parity Matrix

Use the DX12 parity matrix when comparing Windows DX12 against the Vulkan
control lane. It is measurement-first: every lane is routed through
`scripts\benchmark_client.ps1`, every lane records backend, present mode, CEF
visibility/transport, feature toggles, hardware, Windows build, manual display
annotations, and metric presence.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark_dx12_parity.ps1
```

For script/schema validation without launching the client:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark_dx12_parity.ps1 -PlanOnly
```

The quick profile captures the Vulkan and DX12 present-mode controls plus the
highest-risk UI and feature lanes. The full profile adds every declared lane:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark_dx12_parity.ps1 -MatrixSize full -ContinueOnFailure
```

The declared lane vocabulary is:

- Backend/present: `vulkan_immediate`, `vulkan_fifo`,
  `vulkan_auto_no_vsync`, `dx12_immediate`, `dx12_fifo`,
  `dx12_auto_no_vsync`, and `dx12_mailbox_if_available`.
- CEF visibility: `ui_hidden`, `ui_static`, `ui_animated`,
  `ui_animated_1440p_surface`, and `ui_animated_4k_surface`.
- Feature isolation: `clouds_off`, `clouds_on`, `solari_off`, `solari_on`,
  `meshlets_off`, `meshlets_on`, `editor_preview_off`, `editor_preview_on`,
  `cef_cpu_paint`, and `cef_gpu_accelerated`.

Every matrix writes:

- `target\benchmarks\dx12_parity\<timestamp>\matrix.json`
- `target\benchmarks\dx12_parity\<timestamp>\summary.md`

The required metric contract includes `frame_ns.mean/p50/p95/p99`,
`fps.mean/p95`, `present_wait_ns.mean/p95`,
`post_process_gpu_ns`, cloud/Solari/meshlet GPU timings, transient
resource request/create/reuse/alias counts, render scheduler pressure, and CEF
transport counters: `cef_on_paint_fps`, `cef_on_accelerated_paint_fps`,
`cef_cpu_upload_bytes`, `cef_gpu_copy_bytes`, `cef_gpu_copy_ns`,
`cef_transport_fallback_count`, `cef_published_generation`,
`cef_sampled_generation`, and `cef_stale_frame_count`.

PIX, GPUView, and PresentMon-only values are not guessed from client logs. The
matrix JSON lists them under `dx12_external_metrics` with
`status=requires_pix_presentmon_or_gpuview_capture`; attach those tools for
barrier counts, descriptor heap switches, command-list counts, fence waits,
submit counts, and GPU queue idle intervals.

Use the JSON file as the baseline for a second run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark_client.ps1 -Baseline target\benchmarks\client\<baseline>\summary.json
```

For parser-only checks against an existing client log:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark_client.ps1 -InputLog target\run-stack\logs\game_client.out.log
```

For denoiser and DLSS Ray Reconstruction comparisons:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark_denoisers.ps1
```

That matrix runs `off`, `cheap-temporal`, `balanced-fast`, `balanced`,
`quality`, and `rr`, then writes
`target\benchmarks\denoisers\<timestamp>\summary.md` with denoiser compute
nanoseconds, RR guide resolve nanoseconds, external RR nanoseconds, specular
regular/PSR costs, Solari total cost, frame p95, and FPS.

BalancedFast is the standard runtime denoiser. It uses cheap temporal filtering
plus one à trous pass with fused output, while regular Balanced keeps the second
à trous pass as the quality-leaning comparison point. Quality is opt-in only for
screenshots and explicit comparisons. DLSS Ray Reconstruction is currently not
functioning properly in this project: it can render a large black
square/rectangle and break or remove Solari shadows. Keep the `rr` mode in the
matrix for diagnosis, but do not use it as the default path until that artifact
is fixed.

For the RT/Solari capability matrix:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark_rt_matrix.ps1 -RenderBackend dx12 -PresentMode immediate
```

That matrix records both `rt_feature_gates.rt_feature_hash` from the requested
Fun RT gates and `render_capabilities.backend_capability_hash` from the Bevy
startup capability line into every `summary.json`. Its lanes cover baseline
Solari, direct-only, GI-only, direct+GI, DLSS RR diagnostic, half-resolution GI
reservoirs, async readback on/off, and BLAS compaction budget variants. Some
gates are still policy metadata until the matching Solari pass is wired; they
remain captured so later tiers cannot land without before/after numbers under
the same names.

## Default Client Matrix

The quick iteration benchmark is:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark_client.ps1 -RenderBackend dx12 -PresentMode immediate
```

The required performance benchmark for changes that claim client performance is:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark_client.ps1 -Release -StaticBevy -RenderBackend dx12 -PresentMode immediate
```

When a change touches one of these systems, also run the matching isolation case:

- Solari: add `-DisableSolari`.
- Clouds: compare `-DisableClouds` against default scattered, then run
  `-CloudProfile overcast` and `-CloudProfile storm_front` for dense and
  high-motion weather stress.
- Cloud quality: compare `-CloudQuality cheap` and `-CloudQuality balanced`.
- Meshlets: add `-DisableMeshlets`.
- DLSS Ray Reconstruction: diagnostic runs must opt in with
  `-EnableDx12DlssRr -SolariDenoiseMode rr`. Acceptance runs must additionally
  pass `-RequireDx12DlssRrAcceptance`. The script fails acceptance if
  `dlss_rr_gpu_ns`,
  `solari_pass_dlss_rr_guide_resolve_ns`, `frame_ns.mean`, or `frame_ns.p95`
  is missing, or if fewer than 500 estimated live frames were observed.
- Denoisers: compare `-SolariDenoiseMode off`, `cheap-temporal`,
  `balanced-fast`, `balanced`, `quality`, and the DLSS RR preset where
  available.
- Solari internal GI scale: compare `-SolariInternalScale 1.0` against `0.75`,
  `0.66`, and `0.5`; track diffuse initial/spatial ns and Solari VRAM logs.
- CPU-heavy gameplay or networking: keep rendering settings fixed and compare
  process CPU, memory, FPS, and frame time.

## Cloud Visual Smoke

For visible cloud-render changes, run the stack runner at least three ways:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\run_stack.ps1 -DisableClouds
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\run_stack.ps1 -CloudProfile scattered -CloudQuality balanced
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\run_stack.ps1 -CloudDebugOverlay coverage
```

Capture screenshots or a short note confirming that clouds are visible on
background pixels, the debug overlay reaches the final camera target, and
foreground geometry remains unclouded. The current cloud composite is a
sky/background replacement path, not full scene volumetric occlusion.

## Regression Budget

A candidate needs explanation before it lands if it does any of the following:

- Worsens mean frame nanoseconds by more than 3%.
- Worsens p95 frame nanoseconds by more than 5%.
- Worsens a targeted GPU pass by more than 5% without improving another pass or
  a visible quality metric.
- Reduces FPS while only moving cost from one pass to another.
- Improves performance by disabling meshlets, Solari, or required visual paths.

Temporary regressions are acceptable only when the summary includes the reason,
the expected follow-up, and the exact metric that will prove the follow-up
worked.

## Reporting Template

Use this shape in commit messages, PR notes, or review replies:

```text
Client benchmark:
- Command: scripts\benchmark_client.ps1 ...
- Baseline: target\benchmarks\client\<timestamp>\summary.json
- Candidate: target\benchmarks\client\<timestamp>\summary.json
- FPS mean/p50/p95: ... -> ... (...%)
- Frame ns mean/p95: ... -> ... (...%)
- Main pass deltas:
  - solari_gpu_ns mean: ... -> ... (...%)
  - meshlet_visibility_gpu_ns mean: ... -> ... (...%)
  - transient_texture_creates/reuses/aliases mean: ... -> ... (...%)
  - render_scheduler_pressure mean: ... -> ... (...%)
  - cloud_total_gpu_ns mean: ... -> ... (...%)
  - cloud_weather_update_gpu_ns mean: ... -> ... (...%)
  - cloud_shape_noise_gpu_ns mean: ... -> ... (...%)
  - cloud_raymarch_gpu_ns mean: ... -> ... (...%)
  - cloud_temporal_gpu_ns mean: ... -> ... (...%)
  - cloud_resolve_gpu_ns mean: ... -> ... (...%)
  - cloud_composite_gpu_ns mean: ... -> ... (...%)
  - schedule_physics_movement_ns mean: ... -> ... (...%)
  - schedule_world_stream_apply_ns mean: ... -> ... (...%)
  - relevant_pass_ns mean: ... -> ... (...%)
- Visual notes:
- Limitations:
```

## Standard

Performance work is not complete until the project can answer these questions:

- How many nanoseconds did the frame get better or worse?
- Which pass moved?
- Did p95 improve, or did only the average improve?
- Did visual quality change?
- Can another developer reproduce the measurement from committed scripts?
