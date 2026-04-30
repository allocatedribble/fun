# Fun

Fun is a work-in-progress first person shooter built around tactical movement,
destructible environments, and large-scale combined-arms battles.

## Vision

The game should feel sharp enough for a competitive 5v5 match, but the sandbox is
being designed for much larger battles: three-team, PlanetSide 2-like wars with
roughly 100v100v100 players or more, infantry squads, vehicles, and territory
pressure all fighting over the same physical space.

## Core Pillars

- First person shooter fundamentals: readable movement, responsive weapons, and
  clear tactical decision-making.
- Environment destruction that changes routes, sightlines, cover, objectives,
  and vehicle access during a match.
- Competitive 5v5 mode for tight team play, balance testing, and fast iteration.
- Large three-faction war mode built for 100v100v100-scale battles and beyond.
- Vehicles as part of the battlefield, not a separate minigame: transport, armor,
  fire support, logistics, and objective pressure.
- A shared sandbox across modes so tactics learned in 5v5 still matter in the
  larger war.

## Modes

### Competitive 5v5

A smaller, focused mode for round-to-round mastery. Maps are dense, objectives
are readable, and destruction is constrained enough to keep matches fair while
still allowing teams to reshape the fight.

### Three-Faction War

A large-scale combined-arms mode with three opposing teams fighting over a
persistent battlefield. The target scale is 100v100v100 or larger, with infantry,
ground vehicles, air vehicles, destructible strongholds, supply pressure, and
front lines that move as players break and rebuild control of the environment.

## Development Status

This repository is currently an early Rust/Bevy prototype. The immediate focus is
on first-person movement, collision, rendering, networking foundations, and the
technical groundwork needed for meshlet rendering, raytraced lighting, and future
large-match simulation.

## Client Benchmarking

Client changes must be measurable. Use Criterion through
[`scripts/benchmark_criterion.ps1`](scripts/benchmark_criterion.ps1) for
deterministic code-path costs, and use
[`scripts/benchmark_client.ps1`](scripts/benchmark_client.ps1) to capture FPS,
frame nanoseconds, Solari pass timings, meshlet timings, DLSS RR timings, CPU,
memory, and before/after deltas.

Default Criterion capture:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark_criterion.ps1 -SaveBaseline before
```

Default runtime capture:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark_client.ps1 -RenderBackend vulkan -PresentMode immediate
```

Denoiser and DLSS Ray Reconstruction comparison:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark_denoisers.ps1
```

Rich tracing diagnostics:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\run_stack.ps1 -RenderDiagnostics -TraceDiagnostics -RenderBackend vulkan -PresentMode immediate
```

The standard runtime path is Solari plus meshlets with the Balanced denoiser.
DLSS Ray Reconstruction remains available as the explicit `rr`/`dlss-rr`
denoiser preset, but it is currently known not to function properly in this
project: it can produce a large black square/rectangle and broken or missing
shadows. Treat RR as a targeted diagnostic mode until that issue is fixed.

The full standard lives in
[`docs/client_benchmarking.md`](docs/client_benchmarking.md). Do not describe a
client change as faster, smoother, or cheaper unless Criterion and/or runtime
benchmark summaries show the difference in nanoseconds and FPS.
Tracing target details live in
[`docs/client_diagnostics.md`](docs/client_diagnostics.md).
