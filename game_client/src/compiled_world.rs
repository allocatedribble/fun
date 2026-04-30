use bevy::prelude::Resource;
use game_shared::{RenderAssetId, RenderCostClass};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct CompiledWorldPackageId(pub u64);

#[derive(Debug, Clone, Resource)]
pub(crate) struct CompiledWorldPackage {
    pub id: CompiledWorldPackageId,
    pub revision: u64,
    pub content_hash: u64,
    pub static_assets: Vec<CompiledStaticAsset>,
}

#[derive(Debug, Clone)]
pub(crate) struct CompiledStaticAsset {
    pub asset_id: RenderAssetId,
    pub cost_class: RenderCostClass,
    pub occlusion_cell: u32,
}

impl CompiledWorldPackage {
    pub(crate) fn demo_package() -> Self {
        let static_assets = game_shared::DEMO_RENDER_CATALOG
            .iter()
            .map(|entry| CompiledStaticAsset {
                asset_id: entry.asset_id,
                cost_class: entry.cost_class,
                occlusion_cell: entry.occlusion_cell.0,
            })
            .collect::<Vec<_>>();
        let content_hash = hash_demo_catalog();
        Self {
            id: CompiledWorldPackageId(content_hash),
            revision: 1,
            content_hash,
            static_assets,
        }
    }
}

fn hash_demo_catalog() -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for entry in game_shared::DEMO_RENDER_CATALOG {
        hash = fnv1a(hash, entry.asset_id.0);
        hash = fnv1a(hash, entry.material.0);
        hash = fnv1a(hash, entry.occlusion_cell.0);
        hash = fnv1a(hash, entry.lighting.0);
    }
    hash
}

fn fnv1a(mut hash: u64, value: u32) -> u64 {
    for byte in value.to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}
