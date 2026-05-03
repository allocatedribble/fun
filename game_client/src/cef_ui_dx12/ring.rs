use fun_ui_cef::{CefDirtyRect, render_handler::CefUiFrameGeneration};
use windows::Win32::Graphics::{
    Direct3D11::ID3D11Resource,
    Direct3D12::ID3D12Resource,
    Dxgi::Common::{DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM},
};

pub const CEF_GPU_RING_LEN: usize = 3;

pub type DxgiFormat = DXGI_FORMAT;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dx12CefRingSlotRequest {
    Reuse { index: usize },
    Allocate { index: usize },
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dx12CefSlotState {
    Free,
    Copying,
    Ready,
    Consumed,
}

pub struct Dx12CefTextureSlot {
    pub generation: CefUiFrameGeneration,
    pub width: u32,
    pub height: u32,
    pub format: DxgiFormat,
    pub d3d12_resource: ID3D12Resource,
    pub wrapped_d3d11_resource: ID3D11Resource,
    pub fence_value: u64,
    pub state: Dx12CefSlotState,
    pub dirty_rects: Vec<CefDirtyRect>,
}

pub struct Dx12CefTextureRing {
    slots: [Option<Dx12CefTextureSlot>; CEF_GPU_RING_LEN],
    cursor: usize,
}

impl Default for Dx12CefTextureRing {
    fn default() -> Self {
        Self::empty()
    }
}

impl Dx12CefTextureRing {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            slots: std::array::from_fn(|_| None),
            cursor: 0,
        }
    }

    #[must_use]
    pub const fn capacity(&self) -> usize {
        CEF_GPU_RING_LEN
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.iter().flatten().count()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    #[must_use]
    pub fn count_by_state(&self, state: Dx12CefSlotState) -> usize {
        self.slots
            .iter()
            .flatten()
            .filter(|slot| slot.state == state)
            .count()
    }

    #[must_use]
    pub fn ready_generation(&self) -> Option<CefUiFrameGeneration> {
        self.slots
            .iter()
            .flatten()
            .filter(|slot| slot.state == Dx12CefSlotState::Ready)
            .map(|slot| slot.generation)
            .max_by_key(|generation| generation.0)
    }

    #[must_use]
    pub fn next_copy_slot_request(
        &mut self,
        width: u32,
        height: u32,
        format: DxgiFormat,
    ) -> Dx12CefRingSlotRequest {
        if let Some((index, _)) = self.slots.iter().enumerate().find(|(_, slot)| {
            slot.as_ref().is_some_and(|slot| {
                slot.width == width
                    && slot.height == height
                    && slot.format == format
                    && matches!(
                        slot.state,
                        Dx12CefSlotState::Free | Dx12CefSlotState::Consumed
                    )
            })
        }) {
            self.cursor = (index + 1) % CEF_GPU_RING_LEN;
            return Dx12CefRingSlotRequest::Reuse { index };
        }

        if let Some(index) = self.slots.iter().position(Option::is_none) {
            self.cursor = (index + 1) % CEF_GPU_RING_LEN;
            return Dx12CefRingSlotRequest::Allocate { index };
        }

        for offset in 0..CEF_GPU_RING_LEN {
            let index = (self.cursor + offset) % CEF_GPU_RING_LEN;
            if self.slots[index]
                .as_ref()
                .is_some_and(|slot| matches!(slot.state, Dx12CefSlotState::Consumed))
            {
                self.cursor = (index + 1) % CEF_GPU_RING_LEN;
                return Dx12CefRingSlotRequest::Allocate { index };
            }
        }

        Dx12CefRingSlotRequest::Unavailable
    }

    pub fn install_slot(&mut self, index: usize, slot: Dx12CefTextureSlot) {
        if index < CEF_GPU_RING_LEN {
            self.slots[index] = Some(slot);
        }
    }

    #[must_use]
    pub fn slot_mut(&mut self, index: usize) -> Option<&mut Dx12CefTextureSlot> {
        self.slots.get_mut(index).and_then(Option::as_mut)
    }

    #[must_use]
    pub fn slot(&self, index: usize) -> Option<&Dx12CefTextureSlot> {
        self.slots.get(index).and_then(Option::as_ref)
    }

    #[must_use]
    pub fn latest_ready_slot_index(&self) -> Option<usize> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| {
                slot.as_ref()
                    .filter(|slot| slot.state == Dx12CefSlotState::Ready)
                    .map(|slot| (index, slot.generation))
            })
            .max_by_key(|(_, generation)| generation.0)
            .map(|(index, _)| index)
    }

    #[must_use]
    pub fn ready_slot_index_by_generation(
        &self,
        generation: CefUiFrameGeneration,
    ) -> Option<usize> {
        self.slots.iter().enumerate().find_map(|(index, slot)| {
            slot.as_ref()
                .filter(|slot| {
                    slot.state == Dx12CefSlotState::Ready && slot.generation == generation
                })
                .map(|_| index)
        })
    }

    pub fn retire_completed_copying_slots(&mut self, completed_fence_value: u64) {
        for slot in self.slots.iter_mut().flatten() {
            if slot.state == Dx12CefSlotState::Copying && slot.fence_value <= completed_fence_value
            {
                slot.state = Dx12CefSlotState::Consumed;
            }
        }
    }

    #[must_use]
    pub fn debug_slot_summary(&self) -> Dx12CefTextureRingSummary {
        Dx12CefTextureRingSummary {
            capacity: self.capacity(),
            len: self.len(),
            free_count: self.count_by_state(Dx12CefSlotState::Free),
            copying_count: self.count_by_state(Dx12CefSlotState::Copying),
            ready_count: self.count_by_state(Dx12CefSlotState::Ready),
            consumed_count: self.count_by_state(Dx12CefSlotState::Consumed),
            cursor: self.cursor,
            default_format: DXGI_FORMAT_B8G8R8A8_UNORM,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dx12CefTextureRingSummary {
    pub capacity: usize,
    pub len: usize,
    pub free_count: usize,
    pub copying_count: usize,
    pub ready_count: usize,
    pub consumed_count: usize,
    pub cursor: usize,
    pub default_format: DxgiFormat,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dx12_cef_texture_ring_starts_empty_and_triple_buffered() {
        let mut ring = Dx12CefTextureRing::default();

        assert_eq!(ring.capacity(), CEF_GPU_RING_LEN);
        assert_eq!(ring.capacity(), 3);
        assert!(ring.is_empty());
        assert_eq!(ring.debug_slot_summary().ready_count, 0);
        assert_eq!(ring.ready_generation(), None);
        assert_eq!(
            ring.next_copy_slot_request(1280, 720, DXGI_FORMAT_B8G8R8A8_UNORM),
            Dx12CefRingSlotRequest::Allocate { index: 0 }
        );
    }
}
