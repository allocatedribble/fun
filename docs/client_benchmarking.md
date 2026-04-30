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

## Default Client Matrix

The quick iteration benchmark is:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark_client.ps1 -RenderBackend vulkan -PresentMode immediate
```

The required performance benchmark for changes that claim client performance is:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark_client.ps1 -Release -StaticBevy -RenderBackend vulkan -PresentMode immediate
```

When a change touches one of these systems, also run the matching isolation case:

- Solari: add `-DisableSolari`.
- Meshlets: add `-DisableMeshlets`.
- DLSS Ray Reconstruction: run once with `-SolariDenoiseMode rr`, but record
  the known black square/rectangle and missing-shadow artifact if it appears.
- Denoisers: compare `-SolariDenoiseMode off`, `cheap-temporal`,
  `balanced-fast`, `balanced`, `quality`, and the DLSS RR preset where
  available.
- Solari internal GI scale: compare `-SolariInternalScale 1.0` against `0.75`,
  `0.66`, and `0.5`; track diffuse initial/spatial ns and Solari VRAM logs.
- CPU-heavy gameplay or networking: keep rendering settings fixed and compare
  process CPU, memory, FPS, and frame time.

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
