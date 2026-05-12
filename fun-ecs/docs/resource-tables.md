# fun-ecs Resource Tables

status: active baseline
schema_version: fun_ecs_resource_tables_v1
owner: fun

## Purpose

`DenseResourceTable<K, Row>` is the reusable dense-table primitive behind the
future FUN ECS resource-table layer. The current spatial tables remain the
behavioral oracle; this layer generalizes their table behavior before any
storage promotion replaces them.

## Contract

The generic table provides:

- stable typed row keys
- dense row storage
- optional reverse index by row domain key
- configurable capacity
- deterministic insertion-order iteration
- row and table revisions
- deterministic table digest
- scheduler access descriptors per chunk
- chunk extraction by contiguous row range, spatial chunk key, consumer,
  artifact kind, priority band, and deadline class

The table does not make Bevy the authority. Bevy compatibility can mirror
resources, but `fun-ecs` owns the table facts and `fun-scheduler` owns
execution.

## Layout Selection

`ResourceTableLayout` names the storage shape:

- `AoS`: small command queues, low-row-count ledgers, diagnostics
- `SoA`: page residency state scans, priority sorting, streaming shell scans
- `HybridHotCold`: hot fields in columns with cold payload/debug sidecars

`TableLayoutAdvisor` maps use cases to an initial layout and hot-field mask.
This is a policy hint, not a performance claim. Promotion from AoS to SoA or
hybrid storage still requires benchmark evidence.

## Benchmark Gate

The default test suite runs smoke comparisons against current dense page and
artifact storage. The 1M-row acceptance harnesses are explicit because they are
promotion evidence:

```text
cargo test -p fun-ecs dense_resource_table_million -- --ignored
```

The 1M page harness covers:

- scan page residency state
- update priority-like hot fields
- mark dirty
- count request-diff candidates

The 1M artifact harness covers:

- scan ready renderer artifacts
- scan optional Lux refinements
- scan physics cook requests

Passing those tests proves deterministic equivalence and table digest stability.
It does not by itself claim a performance win over current tables.
