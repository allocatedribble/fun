# DX12 Shader Compilation And Quality

This note covers Tier 9 of the DX12 parity campaign. Shader work starts with
diagnostics and export discipline. Shader rewrites require measured before/after
GPU timing and visual regression evidence.

## 9.1 Shader Diagnostics

`scripts/run_stack.ps1 -RenderDiagnostics` enables:

- `FUN_RENDER_SHADER_DIAGNOSTICS=1`
- `BEVY_RENDER_SHADER_DIAGNOSTICS=1`

The local Bevy fork records:

- shader module creation count and elapsed ns;
- shader variant requests and total shader definition count;
- render pipeline creation count and elapsed ns;
- compute pipeline creation count and elapsed ns;
- pipeline specialization count;
- material specialization count;
- top shader events by operation, category, and label.

`game_client` emits parser-stable rows:

```text
[client perf] render shaders: shader_module_creations=... shader_module_create_ns=... shader_variant_requests=... shader_def_count=... material_specializations=... render_pipeline_create_count=... render_pipeline_create_ns=... compute_pipeline_create_count=... compute_pipeline_create_ns=... pipeline_create_count=... pipeline_create_ns=... pipeline_specialization_count=... event_count=...
[client perf] render shader top: rank=1 operation=pipeline_created category=post_process label=post_bloom calls=1 elapsed_ns=... shader_defs=...
```

`scripts/benchmark_client.ps1` stores these under the `render_shader_*` prefix
and writes `render_shader_events` to `summary.json`.
`tools/dx12_parity_report.py` classifies observed DX12 shader module or
pipeline creation in the sample window as `shader compilation`, and higher
shader/material specialization pressure as `shader variant pressure`.

Steady-state gameplay should have:

- `render_shader_shader_module_creations.p95 == 0`
- `render_shader_pipeline_create_count.p95 == 0`
- no unexpected material specialization spikes after scene warmup

Scene transitions may create shader work, but it must be visible in the
benchmark sample and attributable to top event labels.

## 9.2 Shader Analysis Lane

Export and inspect hot variants only after benchmark data identifies them.
Initial hot families:

- clouds;
- meshlet visibility;
- Solari;
- post-process;
- UI.

For NVIDIA, use Nsight or driver shader tools where available. For AMD, use
Radeon GPU Analyzer where applicable.

Compare:

- instruction count;
- register pressure;
- wave occupancy;
- memory loads;
- branch divergence;
- texture sampling pattern.

Every shader rewrite needs:

- Vulkan and DX12 before JSON;
- Vulkan and DX12 after JSON;
- pass-level GPU timing delta;
- screenshot or capture-based visual regression check.

## 9.3 Permutation Pressure Rules

Move only non-structural options toward uniforms:

- debug overlay enable;
- minor quality modes;
- optional tinting;
- non-layout-changing fog/cloud options.

Keep specialization for:

- texture format differences;
- sample count;
- topology;
- storage versus sampled access;
- binding layout changes;
- algorithmic branches with major performance impact.

No permutation reduction lands solely because a shader has many definitions.
It must lower variant/pipeline counts without regressing hot shader timings or
visual output.
