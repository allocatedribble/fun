# DX12 Native Interop Governance

status: active
scope: `fun_render::dx12_native`, CEF accelerated paint, native DLSS, PIX/Nsight naming

## Boundary

`fun_render::dx12_native` is the only approved FUN-layer module that may call
`as_hal::<wgpu::hal::api::Dx12>()` or
`as_hal_mut::<wgpu::hal::api::Dx12>()`.

This boundary is step 6 of
[`dx12_implementation_doctrine.md`](dx12_implementation_doctrine.md). Do not add
native DX12 calls to make the renderer "more Windowsy"; add them only after the
observable/upload/CEF/churn/present gates prove a native interop boundary is the
right tool or a feature such as CEF shared textures or DLSS requires it.

Approved files:

- `fun_render/src/dx12_native/handles.rs`
- `fun_render/src/dx12_native/command_encoder.rs`

Callers must use the shared boundary:

- CEF accelerated paint calls `with_dx12_device_queue_checked` and
  `with_dx12_texture_checked`.
- DLSS SR/RR calls `extract_dx12_texture_handle` and
  `with_dx12_command_list_checked`.
- Future PIX/debug tooling uses `labels.rs` instead of opening a new HAL path.

Validation check:

```powershell
rg -n "as_hal::<|as_hal_mut::<|wgpu::hal::api::Dx12" --glob "*.rs" fun_render/src game_client/src fun_ui_cef/src
```

Every hit should be inside `fun_render/src/dx12_native`.

## Feature Gates

- `fun_render/dx12_native_interop`: compiles the shared Windows-only DX12
  native boundary.
- `fun_render/dx12_dlss_native`: includes `dx12_native_interop` and remains the
  DLSS runtime feature.
- `game_client/cef_ui_dx12_accelerated_paint`: includes
  `fun_render/dx12_native_interop` and keeps CEF-specific D3D11On12 code in
  `game_client`.
- `fun_render/dx12_native_object_names` and
  `game_client/dx12_native_object_names`: optional capture naming path.

## Handle Rules

`Dx12DeviceQueueHandles`, `Dx12TextureHandle`, and `Dx12CommandListHandle`
contain borrowed pointers. Callers must not release them. Callers that store a
COM object beyond the callback must clone or otherwise AddRef through their
own Windows binding before returning.

The command-list accessor currently fails closed because `wgpu-hal` 29 does
not expose `ID3D12GraphicsCommandList`. The failed path is intentional; it keeps
future sanctioned command-list access centralized.

## Object Names

Object naming is compile-time optional and must not enter hot paths unless
`dx12_native_object_names` is enabled. Capture names use stable FUN prefixes:

- `FUN.CEF.RingSlot[2].1280x720.BGRA8`
- `FUN.Post.HDRColor.Main`
- `FUN.Solari.Guide.Normals`
- `FUN.DLSS.SR.Output.Quality`

CEF currently labels its D3D12 queue, copy fence, copy command list, and ring
textures when `game_client/dx12_native_object_names` is enabled. DLSS should use
the same label helpers when the native shim starts owning real D3D12 resources.
