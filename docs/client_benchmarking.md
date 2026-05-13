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

```text
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-bench -- criterion --save-baseline before
```

Compare the candidate against that baseline:

```text
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-bench -- criterion --baseline before --save-baseline after
```

For fast compile/smoke validation while editing benchmark code:

```text
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-bench -- criterion --save-baseline smoke --warmup-seconds 0.1 --measurement-seconds 0.2 --sample-size 10
```

The old client-local Criterion suite has been retired from the product path.
Client CPU and frame-cost checks now run through the first-party benchmark
commands so the measured surface stays FUN-owned:

- Renderer-core CPU setup and resource churn through `fun-renderer` benches.
- Engine, window, scheduler, and telemetry smoke paths through `game_client`.
- Runtime frame metrics through `fun-bench client` and `fun-bench run-stack`.

Criterion writes its HTML and raw estimates under `target\criterion`.

## Runtime Benchmark Command

Use the Rust benchmark CLI for repeatable captures. First-party docs, CI,
fixtures, and agent commands call these Rust subcommands directly.

```text
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-bench -- client
```

The Rust command starts the server and client through `fun-bench run-stack`,
enables render diagnostics, warms up, samples typed telemetry, stops the stack,
and writes the canonical telemetry bundle first. Files below with text-oriented
extensions are rendered views generated from the decoded bundle:

- `target\benchmarks\client\<timestamp>\benchmark.funpb.zst`
- `target\benchmarks\client\<timestamp>\summary.json`
- `target\benchmarks\client\<timestamp>\summary.md`

The hard telemetry contract lives in the umbrella docs:

- [`../../docs/data-platform/protobuf-telemetry-standard.md`](../../docs/data-platform/protobuf-telemetry-standard.md)
- [`../../docs/data-platform/telemetry-size-runtime-budgets.md`](../../docs/data-platform/telemetry-size-runtime-budgets.md)
- [`../../docs/data-platform/diagnostic-retention-policy.md`](../../docs/data-platform/diagnostic-retention-policy.md)

Benchmark captures use `BenchmarkCapture` unless a narrower lane explicitly
declares `HotPathCounters`, `SampledRuntime`, or `TargetedTrace`.

Rendered view consumers may request:

- `target\benchmarks\client\<timestamp>\summary.json`
- `target\benchmarks\client\<timestamp>\summary.md`

`fun-bench client` calls the stack runner through a resolved stack profile. By
default it selects `default.<backend>.<present>`. The stack runner is Rust-owned:
`fun-bench run-stack --profile <name>` reads
`scripts/stack/profiles/<name>.json`, validates the typed `stack_profile_v1`
contract, and writes the resolved build plan, profile path, redacted
environment fingerprint, process plan, and PIDs under `target\run-stack\`.
Use `--stack-profile <profile-name>` only for lanes that intentionally need a
different profile contract. Every emitted benchmark bundle records the
additive, schema-marked `stack_runner` object with the resolved profile,
override map, command, and runner session path. Rendered JSON views expose the
same fields for humans and compatibility import tests.

Required 144 FPS lanes are run through the same Rust engine, not a separate
measurement universe:

```text
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-bench -- required-lanes --render-backend dx12 --present-mode immediate
```

Each lane writes its own benchmark bundle and optional rendered summary views:

- `full_runtime`: Solari plus meshlets plus normal gameplay.
- `solari_floor`: Solari disabled only to expose the non-Solari floor.
- `meshlet_floor`: meshlets disabled only to expose meshlet cost.
- `cpu_floor`: tiny-window workload to expose CPU, scheduling, physics, and networking.
- `streaming_spike`: startup/chunk-apply lane with no warmup so stream p95 is visible.
  This lane samples the client log from process start so startup chunk application
  is not skipped by the normal warmup boundary.
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
request/create/reuse/alias counts, descriptor miss/near-miss counts, top
transient descriptor create rows, and render scheduler pressure when render
diagnostics are enabled. Client CPU schedule work must also report the
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
When frame attribution is needed, run with `--frame-time-diagnostics`; this enables
the debug-only `game_client/debug-diagnostics` feature and writes a per-frame
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
`fun-bench client`, every lane records backend, present mode, NATIVE_UI
visibility/transport, feature toggles, hardware, Windows build, manual display
annotations, and metric presence.

The controlling order of operations is in
[`dx12_implementation_doctrine.md`](dx12_implementation_doctrine.md). In short:
make DX12 observable, remove hot uploads, harden NATIVE_UI GPU transport, reduce
barrier/descriptor/PSO churn, tune present pacing with evidence, centralize
native interop, then bring up DLSS Super Resolution and only later consider Ray
Reconstruction.

```text
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-bench -- dx12-parity
```

For command/schema validation without launching the client:

```text
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-bench -- dx12-parity --plan-only
```

The quick profile captures the Vulkan and DX12 present-mode controls plus the
highest-risk UI and feature lanes. The full profile adds every declared lane:

```text
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-bench -- dx12-parity --matrix-size full --continue-on-failure
```

Use the present matrix before changing any Windows present defaults. It expands
Vulkan/DX12 across `immediate`, `auto_no_vsync`, `fifo`, and `auto_vsync`,
frame latency `1..4`, and the required decision scenarios:

- `ui_hidden`: presentation floor with NATIVE_UI hidden.
- `ui_accelerated`: presentation floor requesting NATIVE_UI D3D11On12 accelerated
  paint.
- `representative_gameplay`: normal `full_runtime` gameplay lane.
- `solari_cloud_heavy`: `full_runtime` with Solari cinematic target and
  cinematic storm-front clouds.

Current stack support records these as windowed lanes; borderless fullscreen
still needs a dedicated host/window-mode switch before it can be included as a
live lane.

```text
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-bench -- dx12-parity --matrix-size present --continue-on-failure
```

Use the stream-pressure matrix before claiming meshlet/world-stream p95
improvements. It runs the `streaming_spike` lane through DX12 and Vulkan
controls, then expands DX12 render-prep budget and chunk-cap tuning lanes:

```text
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-bench -- dx12-parity --matrix-size stream-pressure --continue-on-failure
```

The live controls are:

- `--stream-render-prep-budget-ms 1|2|4|8`, forwarded as
  `FUN_STREAM_RENDER_PREP_BUDGET_MS`.
- `--stream-render-prep-max-chunks-per-frame <n>`, forwarded as
  `FUN_STREAM_RENDER_PREP_MAX_CHUNKS_PER_FRAME`; `0` means uncapped.

Use the focused report to decide whether power-of-two meshlet capacity changes
actually bounded reallocations and whether the render-prep budget is limiting
time-to-ready:

```text
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-data-cli --bin fun-data -- report dx12-meshlet-stream-pressure `
  --matrix-json target\dx12-parity\stream-pressure\matrix.json `
  --markdown-report target\dx12-parity\stream-pressure\dx12_meshlet_stream_pressure_report.md `
  --json-report target\dx12-parity\stream-pressure\dx12_meshlet_stream_pressure_report.json
```

The declared lane vocabulary is:

- Backend/present: `vulkan_immediate`, `vulkan_fifo`,
  `vulkan_auto_no_vsync`, `dx12_immediate`, `dx12_fifo`,
  `dx12_auto_no_vsync`, and `dx12_mailbox_if_available`.
- NATIVE_UI visibility: `ui_hidden`, `ui_static`, `ui_animated`,
  `ui_animated_1440p_surface`, and `ui_animated_4k_surface`.
- Feature isolation: `clouds_off`, `clouds_on`, `solari_off`, `solari_on`,
  `meshlets_off`, `meshlets_on`, `editor_preview_off`, `editor_preview_on`,
  `native_ui_cpu_paint`, and `native_ui_gpu_accelerated`.
- Stream pressure: `stream_pressure_dx12_control_budget2`,
  `stream_pressure_vulkan_control_budget2`, `stream_pressure_dx12_budget1`,
  `stream_pressure_dx12_budget2`, `stream_pressure_dx12_budget4`,
  `stream_pressure_dx12_budget8`, and
  `stream_pressure_dx12_budget2_max_chunks1|2|4|8`.

NATIVE_UI transport controls used by the matrix and direct client benchmark runs:

- `--native_ui-paint-transport disabled|cpu|auto|d3d11on12`
- `--native_ui-accelerated-strict`
- `--native_ui-gpu-ring-depth 2|3|4|5`
- `--native_ui-copy-dirty-rects`
- `--native_ui-debug-timings`

`auto` never fails the app just because accelerated setup is unavailable;
`--native_ui-accelerated-strict` is the debugging lane that turns accelerated setup or
copy failures into loud errors instead of quiet CPU fallback.

Every matrix writes a compressed protobuf bundle first and rendered views only
when requested:

- `target\benchmarks\dx12_parity\<timestamp>\matrix.json`
- `target\benchmarks\dx12_parity\<timestamp>\summary.md`

The matrix summary reports the best observed DX12 lane for maximum FPS, frame
p95, and present-wait p95. It intentionally keeps the product default unchanged
until those recommendations come from comparable live runs.

For an explicit default/benchmark decision artifact, run:

```text
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-data-cli --bin fun-data -- report dx12-present-decision --matrix-json target\dx12-parity\current\matrix.json --markdown-report target\dx12-parity\current\dx12_present_decision_report.md --json-report target\dx12-parity\current\dx12_present_decision_report.json
```

The decision report keeps the product default unchanged unless the required
scenario matrix is complete, p95 and latency agree, and the chosen setting stays
within five percent of the best throughput lane. Hardware-class defaults remain
`not_justified` unless adapter detection, measured class-specific evidence,
explicit override, and startup logging all exist.

The required metric contract includes `frame_ns.mean/p50/p95/p99`,
`fps.mean/p95`, `present_wait_ns.mean/p95`,
`post_process_gpu_ns`, cloud/Solari/meshlet GPU timings, transient
resource request/create/reuse/alias counts, render scheduler pressure, and NATIVE_UI
transport counters: `native_ui_on_paint_fps`, `native_ui_on_accelerated_paint_fps`,
`native_ui_cpu_upload_bytes`, `native_ui_gpu_copy_bytes`, `native_ui_gpu_copy_ns`,
`native_ui_gpu_frame_ready_count`, `native_ui_gpu_frame_not_ready_count`,
`native_ui_gpu_frame_reused_count`, `native_ui_gpu_frame_blocking_wait_count`,
`native_ui_transport_fallback_count`, `native_ui_published_generation`,
`native_ui_sampled_generation`, and `native_ui_stale_frame_count`. The benchmark bundle also
records `native_ui_transport_selection` with the requested transport, selected
transport, backend, bridge readiness, CPU fallback policy, ring depth, copy
mode, strict flag, debug-timing flag, and fallback reason. Render upload
counters are enabled by `--render-diagnostics` and recorded as
`render_upload_write_texture_calls`, `render_upload_write_texture_bytes`,
`render_upload_write_buffer_calls`, `render_upload_write_buffer_bytes`,
`render_upload_write_buffer_with_calls`,
`render_upload_write_buffer_with_bytes`, and
`render_upload_callsite_count`; the rendered JSON view also includes
`render_upload_callsites` with the top ten callsites aggregated over the sample
window. Render churn counters are enabled by the same diagnostics path and
recorded as `render_churn_bind_group_creations`,
`render_churn_bind_group_layout_creations`,
`render_churn_bind_group_layout_cache_hits`,
`render_churn_bind_group_layout_cache_misses`,
`render_churn_pipeline_layout_creations`,
`render_churn_render_pipeline_queued`,
`render_churn_compute_pipeline_queued`,
`render_churn_render_pipeline_creations`,
`render_churn_compute_pipeline_creations`,
`render_churn_pipeline_cache_hits`,
`render_churn_pipeline_cache_misses`, and per-family pipeline-key counts such as
`render_churn_material_pipeline_key_count` and
`render_churn_cloud_pipeline_key_count`; the rendered JSON view also includes
`render_churn_events` with the top ten creation/cache events.
Command submission metrics are recorded as
`render_command_command_encoder_creations`, `render_command_render_passes`,
`render_command_compute_passes`,
`render_command_command_buffers_submitted`, `render_command_queue_submits`,
`render_command_copy_commands`,
`render_command_native_interop_command_insertions`, and
`render_command_event_count`; the rendered JSON view also includes
  `render_command_events` with the top ten operation/category/label rows.
Stream-pressure diagnostics are recorded as
`meshlet_buffer_reallocations`,
`meshlet_instance_buffer_upload_bytes`,
`meshlet_material_buffer_upload_bytes`,
`meshlet_view_visibility_buffer_upload_bytes`,
`meshlet_asset_buffer_upload_bytes`,
`meshlet_asset_buffer_grow_copies`,
`meshlet_buffer_capacity_high_water_bytes`,
`world_stream_apply_cpu_ns`,
`world_stream_render_prep_budget_ns`,
`world_stream_render_prep_max_chunks_per_frame`,
`world_stream_render_prep_limit_reason_code`,
`world_stream_render_prep_queue_depth`,
`world_stream_render_prep_deferred_chunks`,
`world_stream_render_prep_applied_chunks`, and
`world_stream_render_prep_dynamic_mesh_assets`.
`world_stream_render_prep_limit_reason_code` is stable:
`0` means no limiter, `1` means the time budget stopped draining, and `2`
means the max-chunks cap stopped draining.
Readback diagnostics are recorded as
`render_readback_readback_requested_count`,
`render_readback_readback_completed_count`,
`render_readback_readback_dropped_count`,
`render_readback_readback_blocking_wait_count`,
`render_readback_readback_latency_frame_sum`,
`render_readback_readback_latency_frame_max`,
`render_readback_map_async_count`, `render_readback_poll_count`, and
`render_readback_event_count`; the rendered JSON view also includes
`render_readback_events`.

For the focused command/readback decision artifact, run:

```text
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-data-cli --bin fun-data -- report dx12-command-readback `
  --matrix-json target\dx12-parity\current\matrix.json `
  --markdown-report target\dx12-parity\current\dx12_command_readback_report.md `
  --json-report target\dx12-parity\current\dx12_command_readback_report.json
```

The report selects no submit-reduction patch unless high command counts are
paired with an actionable event and a required PIX/GPUView queue-idle trace.
Normal lanes must show `render_readback_readback_blocking_wait_count=0`; debug
or capture lanes may show high requested/map counts, but the report keeps that
overhead explicit.
Shader diagnostics are recorded as
`render_shader_shader_module_creations`,
`render_shader_shader_module_create_ns`,
`render_shader_shader_variant_requests`,
`render_shader_shader_def_count`,
`render_shader_material_specializations`,
`render_shader_render_pipeline_create_count`,
`render_shader_render_pipeline_create_ns`,
`render_shader_compute_pipeline_create_count`,
`render_shader_compute_pipeline_create_ns`,
`render_shader_pipeline_create_count`,
`render_shader_pipeline_create_ns`,
`render_shader_pipeline_specialization_count`, and
`render_shader_event_count`; the rendered JSON view also includes
`render_shader_events` with the top ten shader/pipeline events.

PIX, GPUView, and PresentMon-only values are not guessed from client logs. The
matrix JSON lists them under `dx12_external_metrics` with
`status=requires_pix_presentmon_or_gpuview_capture`; attach those tools for
barrier counts, descriptor heap switches, command-list counts, fence waits,
submit counts, and GPU queue idle intervals.

Use the protobuf bundle as the baseline for a second run. Rendered JSON remains
a generated view for compatibility import tests:

```text
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-bench -- client --baseline target\benchmarks\client\<baseline>\benchmark.funpb.zst
```

For a generated DX12 parity dashboard from one Vulkan bundle and one DX12
bundle:

```text
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-data-cli --bin fun-data -- report dx12-parity `
  --vulkan target\benchmarks\client\<vulkan>\benchmark.funpb.zst `
  --dx12 target\benchmarks\client\<dx12>\benchmark.funpb.zst `
  --markdown target\benchmarks\dx12_parity\dashboard.md `
  --csv target\benchmarks\dx12_parity\dashboard.csv
```

Optional `--pix` and `--presentmon` CSV inputs are parsed into the report as
external trace evidence. The dashboard marks red/yellow/green regressions,
prints a likely bottleneck category when DX12 loses, calls out the special case
where DX12 wins average FPS but loses frame p95, and fails accelerated NATIVE_UI lanes
that still report nonzero `native_ui_cpu_upload_bytes`. It also prints a
vendor-specific follow-up section. NVIDIA experiments are marked eligible only
when the adapter is NVIDIA and the bottleneck is specific enough to act on; they
remain optional and must not regress AMD, Intel, or Vulkan lanes.

See [`dx12_vendor_followup.md`](dx12_vendor_followup.md) for the Tier 16 gate.
See [`dx12_implementation_doctrine.md`](dx12_implementation_doctrine.md) for the
required implementation sequence before DX12-only feature work.
See [`dx12_moonshot_experiments.md`](dx12_moonshot_experiments.md) for the
post-parity Tier 19 experiments. The parity dashboard prints moonshot
eligibility and a `DX12 Memory Budget` table when native/DXGI budget samples or
adapter RAM are available.

CI and agent workflows call `fun-data report <kind>` directly.

## DX12 Perf Regression Gate

Use the local gate when baseline and candidate `benchmark.funpb.zst` bundles
are available from the same hardware, resolution, present mode, NATIVE_UI mode, and
build profile:

```text
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-bench -- dx12-perf-regression-check `
  --baseline target\benchmarks\client\<baseline>\benchmark.funpb.zst `
  --current target\benchmarks\client\<candidate>\benchmark.funpb.zst `
  --report-path target\benchmarks\dx12_perf_gate\report.md `
  --json-out target\benchmarks\dx12_perf_gate\report.json
```

The checked-in envelope is
[`../tools/dx12_perf_baseline_envelopes.json`](../tools/dx12_perf_baseline_envelopes.json).
Hard failures are:

- `frame_ns.p95` regresses by more than 10 percent in the required DX12 lane;
- `native_ui_cpu_upload_bytes.mean` is nonzero in an accelerated NATIVE_UI lane;
- runtime render, compute, or shader pipeline creation appears after warmup.

Warnings do not fail by default. They cover mean FPS regression over 3 percent,
present-wait p95 increases, upload-byte growth over 1 MiB at p95, and transient
create-count increases. Use `--fail-on-warning` only for local ratcheting runs.

CI runs correctness-only self-tests on normal Windows runners:

```text
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-bench -- dx12-perf-regression-check --self-test
```

## DX12 Doctrine Gate

Use the doctrine gate for hardware-free PR checks and to validate supplied NATIVE_UI
summary artifacts:

```text
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-bench -- dx12-doctrine-check --self-test
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-bench -- dx12-doctrine-check
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-bench -- dx12-doctrine-check --summary-path target\benchmarks\client\<candidate>\benchmark.funpb.zst
```

The checker requires `.dx12_change_category`, blocks `dlss-sr` changes until
`docs\dx12_dlss_boundary_gate.md` says the baseline is ready, denies raw DX12
HAL extraction outside `fun_render\src\dx12_native`, and fails accelerated NATIVE_UI
summary artifacts that report nonzero `native_ui_cpu_upload_bytes`.

The `.github/workflows/dx12-perf-gates.yml` hardware job is manual and
non-blocking. It targets self-hosted runners labeled `windows` and `dx12-perf`;
those runners can execute the full benchmark matrix when the sibling path
dependencies are present.

Umbrella CI runs direct Rust benchmark/report fixtures under:

```text
target/data-platform-cutover/fun-fixtures/
```

Standalone `fun` CI emits the same direct Rust artifact shape when `..\fun-cli`
is present. If the nested checkout is tested alone, it writes a skip JSON
artifact and the umbrella root workflow remains the authoritative cutover gate.

For parser-only checks against an existing local debug view:

```text
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-bench -- client --input-log target\run-stack\logs\game_client.out.log --dry-run
```

For static CPU-vs-GPU NATIVE_UI visual checks after capturing matched UI screenshots,
use `fun-data report dx12-parity` or a typed `fun-bench` visual comparison
subcommand once that lane is wired. Do not add shell-script launchers for this
analysis path.

For denoiser and DLSS Ray Reconstruction comparisons:

```text
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-bench -- denoisers
```

That matrix runs `off`, `cheap-temporal`, `balanced-fast`, `balanced`,
`quality`, and `rr`, then writes
`target\benchmarks\denoisers\<timestamp>\summary.md` as a rendered view with denoiser compute
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

```text
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-bench -- rt-matrix --render-backend dx12 --present-mode immediate
```

That matrix records both `rt_feature_gates.rt_feature_hash` from the requested
Fun RT gates and `render_capabilities.backend_capability_hash` from the
FUN-owned renderer capability line into every benchmark bundle. The benchmark
parser records redacted adapter identity, selected backend, present mode,
frame-latency setting, and pipeline-cache policy as renderer capability
diagnostics. Its lanes cover baseline Solari, direct-only, GI-only, direct+GI,
DLSS RR diagnostic, half-resolution GI reservoirs, async readback on/off, and
BLAS compaction budget variants. Some gates are still policy metadata until the
matching Solari pass is wired; they remain captured so later tiers cannot land
without before/after numbers under the same names.

## Renderer Benchmark Suite

Renderer-core benchmark artifacts use schema `fun.renderer.benchmark.v1` and
are owned by `fun_renderer::benchmark`. The suite declares these stable scene
IDs so agents can compare renderer progress without relying on memory:

- `renderer.benchmark.scene.clear_present`
- `renderer.benchmark.scene.static_scene`
- `renderer.benchmark.scene.native_ui_composition`
- `renderer.benchmark.scene.dx12_vulkan_parity`
- `renderer.benchmark.scene.upload_stress`
- `renderer.benchmark.scene.pipeline_warmup_hot_loop`
- `renderer.benchmark.scene.virtual_geometry_stress`
- `renderer.benchmark.scene.dynamic_geometry_stress`
- `renderer.benchmark.scene.procedural_invalidation`
- `renderer.benchmark.scene.many_light_stress`
- `renderer.benchmark.scene.virtual_shadow_stress`
- `renderer.benchmark.scene.gi_reflection_scene`
- `renderer.benchmark.scene.upscaling_scene`
- `renderer.benchmark.scene.fg_eligibility_pacing`

Every artifact records active renderer settings, capability facts, git
revisions, feature flags, optional captures, p50/p95/p99 CPU and GPU frame
time, pass timings, upload bytes, allocation counts, runtime pipeline creation,
page faults, evictions, visible/drawn clusters, light/candidate counts, shadow
page refreshes, GI cache occupancy, NATIVE_UI import/composite latency, upscaler
time, FG generated/presented counts, external UI dependency status, and fallback
reasons.

The hard gates are:

- no unexpected runtime pipeline creation in the hot loop;
- no silent backend fallback;
- no product CPU NATIVE_UI fallback;
- no unbounded page-fault storm;
- no unsupported frame generation;
- no hidden product external engine UI dependency;
- no performance claim without a compressed protobuf benchmark bundle.

Local validation:

```text
cargo test -p fun-renderer --lib benchmark
cargo test -p fun-renderer --lib benchmark::tests::benchmark_artifacts_record_bundle_and_gate_status -- --exact --nocapture
```

The artifact writer emits compressed protobuf first. Renderer performance notes
should cite the bundle before claiming a pass improved, regressed, or merely
moved complexity; JSON or Markdown may be generated only as views.

## Renderer Research Spike Benchmarks

Pass 22 keeps exploratory graphics work isolated from the default renderer.
The registered spikes and compile-time gates are:

| spike | feature flag | owner | promotion floor |
| --- | --- | --- | --- |
| Work Graphs | `experimental_work_graphs` | `fun-renderer` | capability check, runtime opt-in, benchmark comparison, deterministic fallback |
| Mesh shader path | `mesh_shader_path` | `fun-renderer` | capability check, benchmark gate, compute fallback preserved |
| Radiance/neural cache | `radiance_neural_cache` | `fun-lux` | stability, invalidation, latency, memory, and quality comparison |
| Learned page-priority predictor | `learned_page_priority_predictor` | `fun-ai` contract through `fun-renderer` | page faults fall, p95/p99 remain stable, false negatives controlled, deterministic fallback |
| Neural texture compression | `neural_texture_compression` | offline asset pipeline | asset size, decode cost, quality, streaming, and cache-pressure comparison |

Local validation:

```text
cargo test -p fun-renderer --lib research
cargo test -p fun-lux --lib research
cargo test -p fun-renderer --lib research::tests::research_spike_artifact_records_recommendations -- --exact --nocapture
```

The artifact writer emits compressed protobuf first; JSON and Markdown are
views. Spike recommendations must be one of `abandon`, `keep_experimental`, or
`promote_to_production_pass`; promotion is invalid if a spike becomes a default
boot dependency, loses compile/runtime disable support, lacks capability facts,
or lacks a benchmark artifact.

## Renderer Default-Flip Benchmark Gate

Pass 23 makes `FUN_RENDERER_BACKEND=fun` the default core path. The benchmark
contract is now:

- unset or `auto` resolves to `fun`;
- explicit `legacy` remains loud and diagnostic-only;
- product external engine UI and CPU NATIVE_UI fallback gates remain closed;
- renderer ownership is recorded before any performance claim is made.

Local validation:

```text
cargo test -p fun-renderer --lib default_flip
cargo test -p fun_render --lib auto_backend_initializes_fun_core_by_default
cargo test -p fun_render --lib explicit_legacy_backend_is_diagnostic_only_and_loud
cargo test -p fun-renderer --lib default_flip::tests::default_flip_artifact_records_stage_policy_and_owners -- --exact --nocapture
```

Attach the generated default-flip rendered view next to renderer benchmark
bundles when a run depends on the default backend policy.

## Default Client Matrix

The quick iteration benchmark is:

```text
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-bench -- client --render-backend dx12 --present-mode immediate
```

The required performance benchmark for changes that claim client performance is:

```text
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-bench -- client --release --render-backend dx12 --present-mode immediate
```

When a change touches one of these systems, also run the matching isolation case:

- Solari: add `--disable-solari`.
- Clouds: compare `--disable-clouds` against default scattered, then run
  `--cloud-profile overcast` and `--cloud-profile storm-front` for dense and
  high-motion weather stress.
- Cloud quality: compare `--cloud-quality cheap` and `--cloud-quality balanced`.
- Meshlets: add `--disable-meshlets`.
- DLSS Ray Reconstruction: diagnostic runs must opt in with
  `--enable-dx12-dlss-rr --solari-denoise-mode rr`. Acceptance runs must additionally
  pass `--require-dx12-dlss-rr-acceptance`. The Rust benchmark command fails acceptance if
  `dlss_rr_gpu_ns`,
  `solari_pass_dlss_rr_guide_resolve_ns`, `frame_ns.mean`, or `frame_ns.p95`
  is missing, or if fewer than 500 estimated live frames were observed.
- Denoisers: compare `--solari-denoise-mode off`, `cheap-temporal`,
  `balanced-fast`, `balanced`, `quality`, and the DLSS RR preset where
  available.
- Solari internal GI scale: compare `--solari-internal-scale 1.0` against `0.75`,
  `0.66`, and `0.5`; track diffuse initial/spatial ns and Solari VRAM logs.
- CPU-heavy gameplay or networking: keep rendering settings fixed and compare
  process CPU, memory, FPS, and frame time.

## Cloud Visual Smoke

For visible cloud-render changes, run the stack runner at least three ways:

```text
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-bench -- run-stack --disable-clouds
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-bench -- run-stack --cloud-profile scattered --cloud-quality balanced
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-bench -- run-stack --cloud-debug-overlay coverage
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
- Command: cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-bench -- client ...
- Baseline: target\benchmarks\client\<timestamp>\benchmark.funpb.zst
- Candidate: target\benchmarks\client\<timestamp>\benchmark.funpb.zst
- FPS mean/p50/p95: ... -> ... (...%)
- Frame ns mean/p95: ... -> ... (...%)
- Main pass deltas:
  - solari_gpu_ns mean: ... -> ... (...%)
  - meshlet_visibility_gpu_ns mean: ... -> ... (...%)
  - transient_texture_creates/reuses/aliases mean: ... -> ... (...%)
  - transient_texture_descriptor_miss_creates mean: ... -> ... (...%)
  - transient_texture_every_frame_create_descriptors mean: ... -> ... (...%)
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
- Can another developer reproduce the measurement from committed Rust commands?
