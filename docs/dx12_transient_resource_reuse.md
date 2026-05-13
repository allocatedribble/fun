# DX12 Transient Resource Reuse

## Purpose

DX12 p95 can suffer when render scratch textures or buffers are recreated during
steady-state frames. The RetiredEngine fork already has a transient arena that supports
descriptor-key reuse across frames and conservative same-frame aliasing when
declared pass lifetimes do not overlap. This document defines the Tier 7
measurement and cleanup contract.

## Runtime Metrics

`retired_engine_render::transient` emits the frame summary when render diagnostics enable
that target. `fun-bench run-stack --render-diagnostics` now enables:

```text
retired_engine_render::transient=debug
```

The frame summary includes the existing metrics:

- `transient_texture_requests`
- `transient_texture_creates`
- `transient_texture_reuses`
- `transient_texture_aliases`
- `transient_buffer_requests`
- `transient_buffer_creates`
- `transient_buffer_reuses`
- `transient_buffer_aliases`
- `transient_cached_texture_slots`
- `transient_cached_buffer_slots`

It also includes descriptor audit metrics:

- `transient_texture_descriptor_miss_creates`
- `transient_texture_lifetime_conflict_creates`
- `transient_buffer_descriptor_miss_creates`
- `transient_buffer_lifetime_conflict_creates`
- `transient_texture_near_miss_size`
- `transient_texture_near_miss_format`
- `transient_texture_near_miss_usage`
- `transient_texture_near_miss_view_formats`
- `transient_texture_label_variant_descriptors`
- `transient_buffer_label_variant_descriptors`
- `transient_texture_every_frame_create_descriptors`
- `transient_buffer_every_frame_create_descriptors`
- `transient_texture_resize_like_create_descriptors`
- `transient_buffer_resize_like_create_descriptors`

Benchmark JSON stores top rows under:

- `transient_descriptor_creates`
- `transient_descriptor_label_variants`

Generate a focused reuse decision artifact with:

```text
cargo run --manifest-path ..\fun-cli\Cargo.toml -p fun-data-cli --bin fun-data -- report dx12-transient-reuse --matrix-json target\dx12-parity\current\matrix.json --markdown-report target\dx12-parity\current\dx12_transient_reuse_report.md --json-report target\dx12-parity\current\dx12_transient_reuse_report.json
```

The report aggregates top descriptor-create rows, label variants, transient
create/reuse metrics, and the aliasing exclusion table. If descriptor rows are
missing or do not isolate a usage/format/every-frame miss, the correct decision
is to rerun a transient-focused lane rather than guessing at allocator changes.

## Descriptor Reports

Top descriptor-create rows use:

```text
transient descriptor create top resource=texture label=... reason=descriptor_miss near_miss=usage create_pattern=sporadic ...
```

Reasons:

- `descriptor_miss`: no exact cached descriptor key existed.
- `lifetime_overlap`: the descriptor matched, but all matching slots had
  overlapping declared lifetimes this frame.

Near-miss labels:

- `size`: likely resize or render-scale churn.
- `usage`: usage flags differ from an otherwise matching descriptor.
- `format`: format differs from an otherwise matching descriptor.
- `view_formats`: view-format list differs from an otherwise matching
  descriptor.
- `other`: more than one field differs.
- `none`: no similar cached descriptor was found.

Create patterns:

- `new_descriptor`: first observed descriptor.
- `resize_like`: size-only descriptor miss.
- `lifetime_overlap`: extra slot needed for overlapping lifetimes.
- `every_frame`: same descriptor key created in at least three consecutive
  frames.
- `sporadic`: descriptor miss that is not resize-like or every-frame.

Label-variant rows prove label drift separately from descriptor reuse. Labels
are not part of the RetiredEngine transient arena reuse key, so label-only drift is
reported but does not prevent reuse.

## Canonical Descriptors

`retired_engine_render` exposes:

```rust
canonical_transient_texture_desc(kind, size)
```

and:

```rust
pub enum TransientTextureKind {
    MeshletDummyRenderTarget,
    HdrFullRes,
    HdrHalfRes,
    Rgba8Ui,
    DepthLike,
    MotionVectorLike,
}
```

Use this helper when a logical scratch resource has a stable family. The helper
uses a safe usage superset for broad families so passes do not fragment reuse by
toggling a pass-local bit. Native interop resources such as NATIVE_UI/DLSS textures
must stay outside this helper until their state and aliasing contracts are
documented.

The current concrete migration is the meshlet dummy render target. Post-process,
cloud, Solari, and DLSS placeholder scratch should only move after the descriptor
report shows their create/reuse behavior.

## Alias Pools

The RetiredEngine fork records alias buckets as diagnostic policy:

- `HdrFullRes`
- `HdrHalfRes`
- `Rgba8Ui`
- `DepthLike`
- `MotionVectorLike`
- `TinyRenderAttachment`

These are not native heap aliasing contracts. They are the starting taxonomy for
a render-graph lifetime map:

| resource family | first use | last use | pool | native interop |
|---|---:|---:|---|---|
| meshlet dummy render target | 35 | 45 | `TinyRenderAttachment` | no |
| NATIVE_UI UI ring / RetiredEngine UI image | n/a | n/a | excluded | yes |
| DLSS SR/RR input/output | n/a | n/a | excluded | yes |
| Solari RR guide resources | n/a | n/a | excluded | future native DLSS input |
| readback/capture resources | n/a | n/a | excluded | synchronization/capture |
| raw DX12 command-list resources | n/a | n/a | excluded | yes |

NATIVE_UI ring textures, DLSS input/output, Solari RR guide resources,
readback/capture resources, and any resource touched by a raw DX12 command list
remain excluded until PIX validation proves their state transitions, fences,
and native handles cannot be invalidated by aliasing.

## Acceptance Rules

In steady-state lanes:

- major post-process scratch textures must not report nonzero
  `transient_texture_every_frame_create_descriptors`;
- size-only near misses should happen only during resize/render-scale changes;
- usage/format near misses should become canonical descriptor candidates;
- label variants should not be treated as reuse blockers;
- alias count may increase only when declared lifetimes do not overlap;
- DX12 p95 must improve or stay flat after canonicalization.
