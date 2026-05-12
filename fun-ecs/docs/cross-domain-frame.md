# fun-ecs Cross-Domain Frame

status: active baseline
schema_version: fun_ecs_cross_domain_frame_v1
owner: fun

## Owners

The cross-domain frame flow keeps ownership explicit:

- `fun-ecs`: sensing, streaming declarations, artifact records, handoff queues
- `fun-renderer`: renderer extraction, resource realization, frame graph,
  command record, submit, present
- `fun-lux`: lighting plan, shadow/radiance/probe refinement
- `avis`: physics cooks, collision proxies, fixed-step decisions
- `thunder`: network rows, snapshots, deltas, relevance

## Wait Tokens

`EcsCrossDomainWaitTokenKind` maps frame-level dependencies to scheduler wait
tokens. Tokens cover decoded pages, surface artifacts, renderer artifact
publication, Lux invalidations, shadow readiness, physics proxies, network
deltas, and load-animation publication.

## Present And Fixed-Step Policy

Renderer present may wait only on required render artifacts without a valid
fallback. Optional foliage, fine overlays, diagnostics, far SDF, far GI, and
non-critical physics cooks are rejected as present dependencies.

Physics fixed step waits on collision-critical terrain proxies or uses a
conservative physics fallback when available. Non-critical physics cooks do not
gate fixed-step correctness.

Lux cannot wait on renderer present. Renderer cannot wait on optional Lux
refinement.
