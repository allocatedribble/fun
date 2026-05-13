# Fun

## What It Offers

`fun` is the main game workspace for `project-FUN`. It owns the playable game:
gameplay crates, client/server runtime, launcher and host state, shared game
protocols, renderer integration, scene authoring, Lux lighting integration, and
benchmark pressure around the product runtime.

This is where reusable platform contracts become game behavior. AI, data,
networking, backend, Warden, UI, and physics work may start in sibling
workspaces, but `fun` decides how those contracts serve the tactical FPS.
Use this workspace when a change affects what players, servers, launchers, or
runtime hosts actually do.

## Current State

status: active runtime authority

The workspace is the active authority for runtime game behavior. It depends on
first-party FUN workspaces plus Thunder, rvelte, fun-data, fun-ai, backend, and
Warden integration. The local Avian checkout remains a physics baseline while
Avis matures. The umbrella [workspace map](../docs/workspace-map.md)
names `fun` as owner for gameplay, client/server crates, launcher code, game
protocol types, arena tooling, rendering integration, benchmarking, and
game-specific engine/networking use.

## Progress

The game client, game server, shared protocol crate, launcher, product renderer,
asset layer, ECS spatial declaration layer, scene layer, Lux lighting layer, and
benchmark surfaces exist. Renderer identity and the V4 renderer doctrine exist.
The asset, ECS, scene, Lux, and render split is explicit: `fun-asset` owns asset
IDs, handles, stores, manifests, bounded queues, cache state, and
scheduler-visible IO/decode/upload plans; `fun-ecs` owns generic spatial
streaming declarations and hot page-state resources, `fun-scene` owns scene
declaration, `fun-lux` owns lighting and radiance policy, and `fun-renderer`
owns product renderer GPU realization. `fun_render` is now an excluded donor
shell pending deletion, not a product dependency.

The old server/scene compatibility crates are excluded from active workspace
resolution until they are migrated onto FUN-owned engine, scene, physics, and
networking contracts. Active metadata must not require a local engine checkout.

The current physics data plane remains in `game_server` plus the Avian baseline
while Avis matures. Runtime UI is moving toward native rvelte/FUN packets with
Rust-owned host state.

## Goals

- Make `fun-renderer` the product renderer default with measured evidence.
- Move launcher, editor, HUD, menu, diagnostics, and debug UI toward native
  rvelte/FUN UI packets.
- Move runtime orchestration, extraction, and scheduling onto FUN-owned
  contracts.
- Keep game semantics in `fun`; move reusable primitives to sibling workspaces
  only when the ownership boundary is proven.
- Keep client/server protocols bounded, typed, and rejection-tested.

## Ownership

`fun` owns gameplay semantics, runtime authority, host state, scenes, game UI
integration, renderer integration, and game benchmarks. It must not own
reusable networking primitives, account/customer policy, telemetry schema
authority, Warden enforcement policy, reusable AI runtime, or physics engine
primitives after they graduate to Avis.

## Validation

Run from this directory:

```text
cargo fmt-check
cargo check-workspace
cargo test-workspace
cargo clippy-strict
```

For narrow work, validate the owning crate first:

```text
cargo check -p game_client --all-targets
cargo test -p game_client
```

Renderer, UI, networking, and physics claims need the relevant benchmark,
diagnostic, or trace command recorded in `agent-report.ndjson`.

Documentation-only changes should still run the root README gate from the
umbrella root:

```text
cargo run --manifest-path fun-cli\Cargo.toml -p fun -- quality check-readmes --path fun/README.md
```

## Production Rules

All runtime boundary input is hostile: packets, files, browser input, tools,
backend responses, telemetry, and AI output require typed validation. Runtime
authority stays Rust-owned and server-owned where applicable. Public errors and
diagnostics are redacted. Hot paths borrow first, allocate late, and avoid
unbounded reads, runtime panics, and hidden process launches.

## Key Files

- [Cargo.toml](Cargo.toml): game workspace manifest.
- [game_client](game_client): client runtime and host pressure point.
- [game_server](game_server): server authority and current physics baseline.
- [game_shared](game_shared): shared game protocol contracts.
- [fun-asset](fun-asset): asset handles, stores, manifests, load queues, cache
  state, and scheduler-visible asset work plans.
- [fun-ecs](fun-ecs): ECS spatial streaming declarations and hot page tables.
- [fun-renderer](fun-renderer): product renderer core.
- [fun-scene](fun-scene): scene declarations and manifests.
- [fun-lux](fun-lux): lighting and radiance policy.
- [fun_render](fun_render): excluded donor shell pending final deletion.
