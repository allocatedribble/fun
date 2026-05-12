use crate::backend::NativeBackend;
use crate::component_api::NativeUiFrameToken;

use super::native_ui::{
    RendererNativeUiAlphaMode, RendererNativeUiExtent, RendererNativeUiFailClosedReason,
    RendererNativeUiFrameId, RendererNativeUiImportedFrame, RendererNativeUiTextureId,
    RendererNativeUiTransportMode,
};

pub const DX12_NATIVE_UI_TRANSPORT_SCHEMA_VERSION: u16 = 1;
pub const DX12_NATIVE_UI_TEXTURE_RING_DEFAULT_LEN: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dx12NativeUiBridgeMode {
    D3d11On12,
    D3d12Direct,
}

impl Dx12NativeUiBridgeMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::D3d11On12 => "d3d11on12",
            Self::D3d12Direct => "d3d12_direct",
        }
    }

    #[must_use]
    pub const fn requires_interop_layer(self) -> bool {
        matches!(self, Self::D3d11On12)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dx12SharedTextureHandle {
    pub raw_handle_low: u32,
    pub raw_handle_high: u32,
}

impl Dx12SharedTextureHandle {
    pub const INVALID: Self = Self {
        raw_handle_low: 0,
        raw_handle_high: 0,
    };

    #[must_use]
    pub const fn from_u64(raw: u64) -> Self {
        Self {
            raw_handle_low: raw as u32,
            raw_handle_high: (raw >> 32) as u32,
        }
    }

    #[must_use]
    pub const fn raw_u64(self) -> u64 {
        ((self.raw_handle_high as u64) << 32) | (self.raw_handle_low as u64)
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.raw_handle_low != 0 || self.raw_handle_high != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dx12SharedTextureCallback {
    pub handle: Dx12SharedTextureHandle,
    pub extent: RendererNativeUiExtent,
    pub alpha_mode: RendererNativeUiAlphaMode,
    pub frame_id: RendererNativeUiFrameId,
    pub callback_timestamp_ns: u64,
    pub dirty_rect_count: u32,
    pub bridge_mode: Dx12NativeUiBridgeMode,
    pub keyed_mutex_value: u64,
}

impl Dx12SharedTextureCallback {
    pub const fn validate_product(self) -> Result<(), Dx12NativeUiTransportError> {
        if !self.handle.is_valid() {
            return Err(Dx12NativeUiTransportError::InvalidSharedTextureHandle);
        }
        if !self.extent.is_valid() {
            return Err(Dx12NativeUiTransportError::InvalidExtent);
        }
        if !self.frame_id.is_valid() {
            return Err(Dx12NativeUiTransportError::InvalidFrameId);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dx12NativeUiTransportError {
    InvalidSharedTextureHandle,
    InvalidExtent,
    InvalidFrameId,
    BridgeModeNotAvailable,
    BackendNotDx12,
    KeyedMutexUnavailable,
    TextureRingExhausted,
    InteropFenceUnavailable,
}

impl Dx12NativeUiTransportError {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidSharedTextureHandle => "invalid_shared_texture_handle",
            Self::InvalidExtent => "invalid_extent",
            Self::InvalidFrameId => "invalid_frame_id",
            Self::BridgeModeNotAvailable => "bridge_mode_not_available",
            Self::BackendNotDx12 => "backend_not_dx12",
            Self::KeyedMutexUnavailable => "keyed_mutex_unavailable",
            Self::TextureRingExhausted => "texture_ring_exhausted",
            Self::InteropFenceUnavailable => "interop_fence_unavailable",
        }
    }

    #[must_use]
    pub const fn fail_closed_reason(self) -> RendererNativeUiFailClosedReason {
        match self {
            Self::InvalidSharedTextureHandle => {
                RendererNativeUiFailClosedReason::InvalidSharedTextureHandle
            }
            Self::InvalidExtent => RendererNativeUiFailClosedReason::InvalidFrameExtent,
            Self::InvalidFrameId => RendererNativeUiFailClosedReason::InvalidSharedTextureHandle,
            Self::BridgeModeNotAvailable
            | Self::BackendNotDx12
            | Self::KeyedMutexUnavailable
            | Self::InteropFenceUnavailable => {
                RendererNativeUiFailClosedReason::SharedTextureUnavailable
            }
            Self::TextureRingExhausted => RendererNativeUiFailClosedReason::ImportFailed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dx12NativeUiTransportConfig {
    pub schema_version: u16,
    pub bridge_mode: Dx12NativeUiBridgeMode,
    pub keyed_mutex_required: bool,
    pub texture_ring_len: u8,
    pub fence_interop_required: bool,
    pub native_backend: NativeBackend,
}

impl Dx12NativeUiTransportConfig {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: DX12_NATIVE_UI_TRANSPORT_SCHEMA_VERSION,
        bridge_mode: Dx12NativeUiBridgeMode::D3d11On12,
        keyed_mutex_required: true,
        texture_ring_len: DX12_NATIVE_UI_TEXTURE_RING_DEFAULT_LEN,
        fence_interop_required: true,
        native_backend: NativeBackend::Dx12,
    };

    pub const fn validate(self) -> Result<(), Dx12NativeUiTransportError> {
        if !matches!(self.native_backend, NativeBackend::Dx12) {
            return Err(Dx12NativeUiTransportError::BackendNotDx12);
        }
        if self.texture_ring_len == 0 {
            return Err(Dx12NativeUiTransportError::TextureRingExhausted);
        }
        Ok(())
    }
}

impl Default for Dx12NativeUiTransportConfig {
    fn default() -> Self {
        Self::PRODUCT_DEFAULT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dx12RendererTextureSlot {
    pub texture: RendererNativeUiTextureId,
    pub handle: Dx12SharedTextureHandle,
    pub fence_value: u64,
    pub frame_id: RendererNativeUiFrameId,
}

impl Dx12RendererTextureSlot {
    pub const VACANT: Self = Self {
        texture: RendererNativeUiTextureId::INVALID,
        handle: Dx12SharedTextureHandle::INVALID,
        fence_value: 0,
        frame_id: RendererNativeUiFrameId::INVALID,
    };

    #[must_use]
    pub const fn is_vacant(self) -> bool {
        self.fence_value == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dx12RendererTextureRing {
    config: Dx12NativeUiTransportConfig,
    slots: Vec<Dx12RendererTextureSlot>,
    next_generation: u32,
    next_fence_value: u64,
    inserted_frames: u64,
    overwritten_frames: u64,
}

impl Dx12RendererTextureRing {
    #[must_use]
    pub fn new(config: Dx12NativeUiTransportConfig) -> Self {
        let len = config.texture_ring_len.max(1) as usize;
        Self {
            config,
            slots: vec![Dx12RendererTextureSlot::VACANT; len],
            next_generation: 1,
            next_fence_value: 1,
            inserted_frames: 0,
            overwritten_frames: 0,
        }
    }

    pub fn copy_from_callback(
        &mut self,
        callback: Dx12SharedTextureCallback,
    ) -> Result<Dx12NativeUiFenceToken, Dx12NativeUiTransportError> {
        callback.validate_product()?;
        if callback.bridge_mode != self.config.bridge_mode
            && self.config.bridge_mode == Dx12NativeUiBridgeMode::D3d11On12
        {
            return Err(Dx12NativeUiTransportError::BridgeModeNotAvailable);
        }
        if self.config.keyed_mutex_required && callback.keyed_mutex_value == 0 {
            return Err(Dx12NativeUiTransportError::KeyedMutexUnavailable);
        }
        let fence_value = self.next_fence_value;
        self.next_fence_value = self.next_fence_value.saturating_add(1);
        let slot_index = self.select_slot();
        let prior = self.slots[slot_index];
        if !prior.is_vacant() {
            self.overwritten_frames = self.overwritten_frames.saturating_add(1);
        }
        let texture = RendererNativeUiTextureId {
            index: u32::try_from(slot_index).unwrap_or(u32::MAX),
            generation: self.next_generation,
        };
        self.next_generation = self.next_generation.saturating_add(1);
        let slot = Dx12RendererTextureSlot {
            texture,
            handle: callback.handle,
            fence_value,
            frame_id: callback.frame_id,
        };
        self.slots[slot_index] = slot;
        self.inserted_frames = self.inserted_frames.saturating_add(1);
        Ok(Dx12NativeUiFenceToken {
            schema_version: DX12_NATIVE_UI_TRANSPORT_SCHEMA_VERSION,
            texture,
            frame_id: callback.frame_id,
            fence_value,
            keyed_mutex_value: callback.keyed_mutex_value,
            bridge_mode: self.config.bridge_mode,
            extent: callback.extent,
            alpha_mode: callback.alpha_mode,
            transport: RendererNativeUiTransportMode::D3d11On12SharedTexture,
            callback_timestamp_ns: callback.callback_timestamp_ns,
            dirty_rect_count: callback.dirty_rect_count,
        })
    }

    #[must_use]
    pub fn slots(&self) -> &[Dx12RendererTextureSlot] {
        &self.slots
    }

    #[must_use]
    pub const fn config(&self) -> Dx12NativeUiTransportConfig {
        self.config
    }

    #[must_use]
    pub const fn inserted_frames(&self) -> u64 {
        self.inserted_frames
    }

    #[must_use]
    pub const fn overwritten_frames(&self) -> u64 {
        self.overwritten_frames
    }

    fn select_slot(&self) -> usize {
        let mut chosen = 0usize;
        let mut chosen_fence = u64::MAX;
        for (index, slot) in self.slots.iter().enumerate() {
            if slot.is_vacant() {
                return index;
            }
            if slot.fence_value < chosen_fence {
                chosen_fence = slot.fence_value;
                chosen = index;
            }
        }
        chosen
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dx12NativeUiFenceToken {
    pub schema_version: u16,
    pub texture: RendererNativeUiTextureId,
    pub frame_id: RendererNativeUiFrameId,
    pub fence_value: u64,
    pub keyed_mutex_value: u64,
    pub bridge_mode: Dx12NativeUiBridgeMode,
    pub extent: RendererNativeUiExtent,
    pub alpha_mode: RendererNativeUiAlphaMode,
    pub transport: RendererNativeUiTransportMode,
    pub callback_timestamp_ns: u64,
    pub dirty_rect_count: u32,
}

impl Dx12NativeUiFenceToken {
    #[must_use]
    pub const fn into_imported_frame(
        self,
        import_begin_timestamp_ns: u64,
        import_complete_timestamp_ns: u64,
        copied_bytes: u64,
    ) -> RendererNativeUiImportedFrame {
        RendererNativeUiImportedFrame {
            frame_id: self.frame_id,
            extent: self.extent,
            transport: self.transport,
            alpha_mode: self.alpha_mode,
            dirty_rect_count: self.dirty_rect_count,
            dirty_rect_union: None,
            callback_timestamp_ns: self.callback_timestamp_ns,
            import_begin_timestamp_ns,
            import_complete_timestamp_ns,
            copied_bytes,
        }
    }

    #[must_use]
    pub const fn into_component_token(
        self,
        producer: crate::component_api::RenderStableId,
    ) -> NativeUiFrameToken {
        NativeUiFrameToken::new(producer, self.frame_id.0, self.fence_value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn callback(handle: u64, frame_id: u64) -> Dx12SharedTextureCallback {
        Dx12SharedTextureCallback {
            handle: Dx12SharedTextureHandle::from_u64(handle),
            extent: RendererNativeUiExtent::new(1920, 1080),
            alpha_mode: RendererNativeUiAlphaMode::Premultiplied,
            frame_id: RendererNativeUiFrameId(frame_id),
            callback_timestamp_ns: 1_000,
            dirty_rect_count: 1,
            bridge_mode: Dx12NativeUiBridgeMode::D3d11On12,
            keyed_mutex_value: 1,
        }
    }

    #[test]
    fn dx12_transport_config_default_is_d3d11on12_with_keyed_mutex_and_three_slot_ring() {
        let config = Dx12NativeUiTransportConfig::default();
        assert_eq!(config.bridge_mode, Dx12NativeUiBridgeMode::D3d11On12);
        assert!(config.keyed_mutex_required);
        assert_eq!(
            config.texture_ring_len,
            DX12_NATIVE_UI_TEXTURE_RING_DEFAULT_LEN
        );
        assert!(config.fence_interop_required);
        assert!(config.bridge_mode.requires_interop_layer());
        assert_eq!(config.validate(), Ok(()));
    }

    #[test]
    fn dx12_transport_rejects_non_dx12_native_backend() {
        let config = Dx12NativeUiTransportConfig {
            native_backend: NativeBackend::Vulkan,
            ..Dx12NativeUiTransportConfig::default()
        };
        assert_eq!(
            config.validate(),
            Err(Dx12NativeUiTransportError::BackendNotDx12)
        );
    }

    #[test]
    fn ring_assigns_increasing_fence_values_and_recycles_oldest_slot() {
        let mut ring = Dx12RendererTextureRing::new(Dx12NativeUiTransportConfig {
            texture_ring_len: 2,
            ..Dx12NativeUiTransportConfig::default()
        });

        let token_one = ring.copy_from_callback(callback(0xAA, 1)).expect("copy 1");
        let token_two = ring.copy_from_callback(callback(0xBB, 2)).expect("copy 2");
        let token_three = ring.copy_from_callback(callback(0xCC, 3)).expect("copy 3");

        assert!(token_one.fence_value < token_two.fence_value);
        assert!(token_two.fence_value < token_three.fence_value);
        assert_eq!(ring.inserted_frames(), 3);
        assert_eq!(ring.overwritten_frames(), 1);
        assert!(token_three.texture.is_valid());
        assert_eq!(token_three.texture.index, token_one.texture.index);
    }

    #[test]
    fn invalid_shared_texture_handle_fails_closed_with_typed_reason() {
        let mut ring = Dx12RendererTextureRing::new(Dx12NativeUiTransportConfig::default());
        let mut bad = callback(0xAB, 4);
        bad.handle = Dx12SharedTextureHandle::INVALID;

        let err = ring.copy_from_callback(bad).expect_err("invalid handle");

        assert_eq!(err, Dx12NativeUiTransportError::InvalidSharedTextureHandle);
        assert_eq!(
            err.fail_closed_reason(),
            RendererNativeUiFailClosedReason::InvalidSharedTextureHandle
        );
    }

    #[test]
    fn keyed_mutex_required_but_zero_fails_closed() {
        let mut ring = Dx12RendererTextureRing::new(Dx12NativeUiTransportConfig::default());
        let mut bad = callback(0xAB, 5);
        bad.keyed_mutex_value = 0;

        let err = ring.copy_from_callback(bad).expect_err("zero keyed mutex");
        assert_eq!(err, Dx12NativeUiTransportError::KeyedMutexUnavailable);
        assert_eq!(
            err.fail_closed_reason(),
            RendererNativeUiFailClosedReason::SharedTextureUnavailable
        );
    }

    #[test]
    fn fence_token_converts_back_into_imported_frame_for_compositor_pipeline() {
        let mut ring = Dx12RendererTextureRing::new(Dx12NativeUiTransportConfig::default());
        let token = ring.copy_from_callback(callback(0xDE, 6)).expect("copy");

        let frame = token.into_imported_frame(1_500, 2_000, 1920 * 1080 * 4);

        assert_eq!(frame.frame_id, token.frame_id);
        assert_eq!(frame.extent, token.extent);
        assert_eq!(
            frame.transport,
            RendererNativeUiTransportMode::D3d11On12SharedTexture
        );
        assert_eq!(frame.import_begin_timestamp_ns, 1_500);
        assert_eq!(frame.import_complete_timestamp_ns, 2_000);
    }
}
