# DX12 Vendor-Specific Follow-Up Gate

status: active
owner_repo: fun
scope: Tier 16 DX12 vendor-specific optimization governance

## Policy

Vendor-specific optimization is allowed only after the DX12 parity dashboard or
an attached PIX/GPUView/Nsight trace identifies a specific bottleneck. It is not
a default renderer direction and must not hide base DX12 parity failures.

Every vendor-specific experiment must stay:

- optional at runtime or compile time;
- disabled for unrelated adapters unless explicitly proven safe;
- measured against Vulkan on the same hardware;
- measured against AMD or Intel before becoming cross-vendor default behavior;
- reversible through a narrow feature flag or configuration gate.

Acceptance rule:

```text
vendor_specific_optimization_optional: true
amd_intel_vulkan_regression_allowed: false
```

## NVIDIA Path

Use this path only when the adapter is NVIDIA and the bottleneck category is
specific enough to act on.

| bottleneck | required evidence | allowed first experiments |
| --- | --- | --- |
| descriptor churn | PIX descriptor heap switches or `render_churn_bind_group*` deltas | bindless-like material table evaluation where Bevy/wgpu permits it; fewer per-draw bind group changes; descriptor rollover inspection |
| pipeline churn / PSO churn | runtime pipeline creation, cache-miss counters, or PIX PSO creation | stronger pipeline warmup; persistent PSO cache experiment; lower material key fragmentation |
| upload-bound | `render_upload_*`, meshlet/world-stream upload, NATIVE_UI upload/copy counters, or PIX copy evidence | persistent staging or native upload-heap prototype for measured callsites only |
| barrier/state-bound | PIX resource barrier summary naming the pass | collapse redundant transitions; fix pass ordering/resource-state ownership before queue experiments |
| async scheduling / submission fragmentation | PIX/GPUView queue idle spans plus command-buffer/submit deltas | async copy/compute experiment only when overlap exists and p95 improves |
| shader/pass GPU bound | pass-level GPU regression plus Nsight or driver shader analysis | inspect register pressure, wave occupancy, memory loads, and branch divergence before rewriting shaders |

## Report Integration

`fun-data report dx12-parity` emits a `Vendor-Specific Follow-Up` section. The
section reports:

- inferred adapter vendor;
- eligibility status;
- required evidence for the detected bottleneck;
- guardrail that the experiment is optional and must not regress AMD, Intel, or
  Vulkan lanes;
- NVIDIA-specific first experiments when the adapter and bottleneck match.

The report must say `blocked_until_specific_bottleneck` for unknown,
present-only, no-clear-loss, or generic p95 cases. Present pacing work is still
important, but it is not a vendor shader/descriptor/upload optimization by
itself.

## Promotion Checklist

A vendor-specific optimization can become a default only after:

- before/after DX12 JSON on the target vendor;
- before/after Vulkan JSON on the same hardware;
- AMD or Intel smoke validation, or an explicit adapter gate that prevents
  activation there;
- p95 result is equal or better;
- visual output is unchanged or intentionally documented;
- the feature flag and fallback path remain tested.
