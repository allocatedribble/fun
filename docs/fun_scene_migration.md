# fun-scene Migration

`fun-scene` is FUN's first-party scene authoring crate. It keeps Bevy ECS as the
runtime data model while moving product scene authoring away from direct Bevy
`bsn!` usage and into FUN-owned `fun!` / `fun_list!` macros.

## Current Scope

- `fun-scene` wraps the Bevy scene resolver and spawn APIs under FUN names.
- `fun-scene-macros` exposes `fun!` and `fun_list!` while the first migration
  cycle still targets Bevy's proven BSN macro implementation internally.
- `fun_scene::prelude::*` is the expected scene-authoring import.
- `game_scene` is the game-specific scene catalog and should not import or call
  `bsn!` / `bsn_list!` directly.
- Generic scene identity, manifest, renderer, streaming, and lighting authoring
  data belongs in `fun-scene`; game-specific arenas and benchmark scenes stay in
  `game_scene`.

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
use bevy::scene::{bsn, bsn_list};
use bevy::scene::prelude::{bsn, bsn_list};
```

The temporary deprecated `fun_scene::{bsn, bsn_list}` aliases exist only for
short-lived migration shims. Product scene code should not use them.

## Checker

Run the migration checker from the `fun` workspace root:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/check_fun_scene_migration.ps1 -SelfTest
powershell -NoProfile -ExecutionPolicy Bypass -File tools/check_fun_scene_migration.ps1
```

The checker rejects direct `bevy::scene` BSN imports, direct Bevy scene macro
paths, and plain `bsn!` / `bsn_list!` usage inside `game_scene/src`. The
`fun-scene-macros` bridge files are the only allowed place to name the internal
Bevy BSN macro target during this transition.

## Validation

For this migration slice, use:

```powershell
cargo fmt --check -p fun-scene -p fun-scene-macros -p game_scene
cargo check -p fun-scene
cargo check -p fun-scene-macros
cargo test -p game_scene --locked
powershell -NoProfile -ExecutionPolicy Bypass -File tools/check_fun_scene_migration.ps1 -SelfTest
powershell -NoProfile -ExecutionPolicy Bypass -File tools/check_fun_scene_migration.ps1
git diff --check
```
