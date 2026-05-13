# fun-scene Migration

`fun-scene` is FUN's first-party scene authoring crate. It keeps RetiredEngine ECS as the
runtime data model while moving product scene authoring away from direct RetiredEngine
`bsn!` usage and into FUN-owned `fun!` / `fun_list!` macros.

## Current Scope

- `fun-scene` wraps the RetiredEngine scene resolver and spawn APIs under FUN names.
- `fun-scene-macros` exposes `fun!` and `fun_list!` while the first migration
  cycle still targets RetiredEngine's proven BSN macro implementation internally.
- `fun_scene::prelude::*` is the expected scene-authoring import.
- `game_scene` is the game-specific scene catalog and should not import or call
  `bsn!` / `bsn_list!` directly.
- Generic scene identity, manifest, renderer, streaming, and lighting authoring
  data belongs in `fun-scene`; game-specific arenas and benchmark scenes stay in
  `game_scene`.

## Renderer Migration Ownership

Scene migration feeds the Pass 1 renderer crate split:

- `fun-scene` owns scene authoring, stable identities, manifests, streaming
  declarations, and renderer/lux authoring components.
- `fun-renderer` owns renderer-core traits, backend abstraction, frame graph,
  scene database, resource allocation, pass registration, presentation, and the
  no-op clear-color boot path.
- `fun-lux` owns lighting/GI modes, light database contracts, shadow/GI policy,
  and denoising/reconstruction hooks.
- `fun_render` owns the RetiredEngine-facing bridge settings, feature toggles, extraction
  hook registry, debug overlay hooks, and benchmark hooks.
- `fun-ai` owns reusable model runtime policy. Renderer-facing model hooks are
  tensor schemas and proposal surfaces only.

## Authoring Rule

New scene code should use:

```rust
use fun_scene::prelude::*;

commands.spawn_fun_scene_list(fun_list![
    (
        #Example
        fun_value(SceneStableIdentity(entity))
        Transform::default()
    ),
]);
```

Do not add:

```rust
use retired_engine::scene::{bsn, bsn_list};
use retired_engine::scene::prelude::{bsn, bsn_list};
```

The temporary deprecated `fun_scene::{bsn, bsn_list}` aliases exist only for
short-lived migration shims. Product scene code should not use them.

## Checker

Run the migration checker from the `fun` workspace root:

```text
fun-quality check-code-shape -SelfTest
fun-quality check-code-shape
```

The checker rejects direct `retired_engine::scene` BSN imports, direct RetiredEngine scene macro
paths, and plain `bsn!` / `bsn_list!` usage inside `game_scene/src`. The
`fun-scene-macros` bridge files are the only allowed place to name the internal
RetiredEngine BSN macro target during this transition.

## Validation

For this migration slice, use:

```text
cargo fmt --check -p fun-scene -p fun-scene-macros -p game_scene
cargo check -p fun-scene
cargo check -p fun-scene-macros
cargo test -p game_scene --locked
fun-quality check-code-shape -SelfTest
fun-quality check-code-shape
git diff --check
```
