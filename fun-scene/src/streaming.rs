use bevy_transform::components::Transform;
use thunder::prelude::{
    AuthorityMode, PackedColorRgba8, Quantization, QuantizedQuat, QuantizedTransform3,
    QuantizedVec3, ReplicationClass, WorldCatalogRef, WorldCollider, WorldEntitySpec, WorldLevelId,
    WorldPrimitive, WorldRevision, WorldStreamChunk,
};

pub const WORLD_STREAM_ENTITIES_PER_CHUNK: usize = 16;
pub const MAX_WORLD_STREAM_CHUNKS: usize = u16::MAX as usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamChunkError {
    TooManyChunks,
}

#[must_use]
pub fn chunk_world_specs(
    level_id: &str,
    revision: WorldRevision,
    specs: Vec<WorldEntitySpec>,
) -> Vec<WorldStreamChunk> {
    try_chunk_world_specs(level_id, revision, specs).unwrap_or_default()
}

pub fn try_chunk_world_specs(
    level_id: &str,
    revision: WorldRevision,
    specs: Vec<WorldEntitySpec>,
) -> Result<Vec<WorldStreamChunk>, StreamChunkError> {
    let chunk_count = chunk_count_for_spec_len(specs.len())?;
    if chunk_count == 0 {
        return Ok(Vec::new());
    }
    let manifest_signature = world_stream_manifest_signature(&specs);

    Ok(specs
        .chunks(WORLD_STREAM_ENTITIES_PER_CHUNK)
        .enumerate()
        .map(|(chunk_index, entities)| WorldStreamChunk {
            level_id: WorldLevelId(level_id.to_owned()),
            revision,
            chunk_index: chunk_index as u16,
            chunk_count,
            manifest_signature,
            entities: entities.to_vec(),
        })
        .collect())
}

pub fn chunk_count_for_spec_len(spec_len: usize) -> Result<u16, StreamChunkError> {
    if spec_len == 0 {
        return Ok(0);
    }
    let chunk_count = spec_len.div_ceil(WORLD_STREAM_ENTITIES_PER_CHUNK);
    if chunk_count > MAX_WORLD_STREAM_CHUNKS {
        return Err(StreamChunkError::TooManyChunks);
    }
    Ok(chunk_count as u16)
}

#[must_use]
pub fn world_stream_manifest_signature(specs: &[WorldEntitySpec]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    hash = fnv1a_u64(hash, specs.len() as u64);
    for spec in specs {
        hash = fnv1a_u64(hash, spec.entity.0);
        hash = fnv1a_str(hash, &spec.name);
        hash = hash_replication_class(hash, spec.class);
        hash = hash_authority_mode(hash, spec.authority);
        hash = hash_transform(hash, spec.transform);
        hash = hash_catalog(hash, spec.catalog);
        hash = hash_render(hash, spec.render);
        hash = hash_collider(hash, spec.collider);
        hash = hash_color(hash, spec.color);
    }
    hash
}

#[must_use]
pub fn qtransform(transform: &Transform) -> QuantizedTransform3 {
    QuantizedTransform3 {
        translation: qvec(transform.translation.to_array()),
        rotation: QuantizedQuat::from_f32([
            transform.rotation.x,
            transform.rotation.y,
            transform.rotation.z,
            transform.rotation.w,
        ]),
    }
}

fn qvec(value: [f32; 3]) -> QuantizedVec3 {
    QuantizedVec3::from_f32(value, Quantization::MILLIMETERS)
}

fn hash_replication_class(mut hash: u64, class: ReplicationClass) -> u64 {
    match class {
        ReplicationClass::Pawn => fnv1a(hash, 0),
        ReplicationClass::Projectile => fnv1a(hash, 1),
        ReplicationClass::Destructible => fnv1a(hash, 2),
        ReplicationClass::Vehicle => fnv1a(hash, 3),
        ReplicationClass::Objective => fnv1a(hash, 4),
        ReplicationClass::World => fnv1a(hash, 5),
        ReplicationClass::Custom(value) => {
            hash = fnv1a(hash, 6);
            fnv1a_u16(hash, value)
        }
    }
}

fn hash_authority_mode(mut hash: u64, authority: AuthorityMode) -> u64 {
    match authority {
        AuthorityMode::ServerOnly => fnv1a(hash, 0),
        AuthorityMode::ClientPredicted { owner } => {
            hash = fnv1a(hash, 1);
            fnv1a_u64(hash, owner.0)
        }
        AuthorityMode::StaticServer => fnv1a(hash, 2),
    }
}

fn hash_transform(mut hash: u64, transform: QuantizedTransform3) -> u64 {
    hash = hash_qvec(hash, transform.translation);
    hash = fnv1a_i16(hash, transform.rotation.x);
    hash = fnv1a_i16(hash, transform.rotation.y);
    hash = fnv1a_i16(hash, transform.rotation.z);
    fnv1a_i16(hash, transform.rotation.w)
}

fn hash_catalog(mut hash: u64, catalog: Option<WorldCatalogRef>) -> u64 {
    match catalog {
        Some(catalog) => {
            hash = fnv1a_u8(hash, 1);
            hash = fnv1a(hash, catalog.asset_id);
            hash = fnv1a(hash, catalog.material_id);
            fnv1a(hash, catalog.collider_id)
        }
        None => fnv1a_u8(hash, 0),
    }
}

fn hash_render(mut hash: u64, render: Option<WorldPrimitive>) -> u64 {
    match render {
        Some(WorldPrimitive::Plane { size }) => {
            hash = fnv1a_u8(hash, 1);
            hash_qvec(hash, size)
        }
        Some(WorldPrimitive::Cuboid { size }) => {
            hash = fnv1a_u8(hash, 2);
            hash_qvec(hash, size)
        }
        None => fnv1a_u8(hash, 0),
    }
}

fn hash_collider(mut hash: u64, collider: Option<WorldCollider>) -> u64 {
    match collider {
        Some(WorldCollider::Cuboid { size }) => {
            hash = fnv1a_u8(hash, 1);
            hash_qvec(hash, size)
        }
        None => fnv1a_u8(hash, 0),
    }
}

fn hash_color(mut hash: u64, color: Option<PackedColorRgba8>) -> u64 {
    match color {
        Some(color) => {
            hash = fnv1a_u8(hash, 1);
            hash = fnv1a_u8(hash, color.r);
            hash = fnv1a_u8(hash, color.g);
            hash = fnv1a_u8(hash, color.b);
            fnv1a_u8(hash, color.a)
        }
        None => fnv1a_u8(hash, 0),
    }
}

fn hash_qvec(mut hash: u64, value: QuantizedVec3) -> u64 {
    hash = fnv1a_i32(hash, value.x);
    hash = fnv1a_i32(hash, value.y);
    fnv1a_i32(hash, value.z)
}

fn fnv1a(mut hash: u64, value: u32) -> u64 {
    for byte in value.to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn fnv1a_u8(mut hash: u64, value: u8) -> u64 {
    hash ^= u64::from(value);
    hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    hash
}

fn fnv1a_u16(mut hash: u64, value: u16) -> u64 {
    for byte in value.to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn fnv1a_i16(hash: u64, value: i16) -> u64 {
    fnv1a_u16(hash, value as u16)
}

fn fnv1a_i32(hash: u64, value: i32) -> u64 {
    fnv1a(hash, value as u32)
}

fn fnv1a_u64(mut hash: u64, value: u64) -> u64 {
    for byte in value.to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn fnv1a_str(mut hash: u64, value: &str) -> u64 {
    hash = fnv1a_u64(hash, value.len() as u64);
    for byte in value.bytes() {
        hash = fnv1a_u8(hash, byte);
    }
    hash
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunSceneStreamingPolicy {
    pub chunked_world_streams: bool,
    pub stable_chunk_indices: bool,
    pub procedural_expansion_after_validation: bool,
}

impl FunSceneStreamingPolicy {
    pub const DEFAULT: Self = Self {
        chunked_world_streams: true,
        stable_chunk_indices: true,
        procedural_expansion_after_validation: true,
    };
}

impl Default for FunSceneStreamingPolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use thunder::prelude::{NetEntity, PackedColorRgba8};

    use super::*;

    fn sample_specs() -> Vec<WorldEntitySpec> {
        vec![
            WorldEntitySpec {
                entity: NetEntity(1),
                name: "Wall".to_owned(),
                class: ReplicationClass::World,
                authority: AuthorityMode::StaticServer,
                transform: qtransform(&Transform::from_xyz(1.0, 2.0, 3.0)),
                catalog: Some(WorldCatalogRef {
                    asset_id: 10,
                    material_id: 20,
                    collider_id: 30,
                }),
                render: Some(WorldPrimitive::Cuboid {
                    size: qvec([4.0, 5.0, 6.0]),
                }),
                collider: Some(WorldCollider::Cuboid {
                    size: qvec([4.0, 5.0, 6.0]),
                }),
                color: Some(PackedColorRgba8 {
                    r: 128,
                    g: 64,
                    b: 32,
                    a: 255,
                }),
            },
            WorldEntitySpec {
                entity: NetEntity(2),
                name: "Floor".to_owned(),
                class: ReplicationClass::World,
                authority: AuthorityMode::StaticServer,
                transform: qtransform(&Transform::from_xyz(0.0, 0.0, 0.0)),
                catalog: None,
                render: Some(WorldPrimitive::Plane {
                    size: qvec([12.0, 0.0, 12.0]),
                }),
                collider: None,
                color: None,
            },
        ]
    }

    #[test]
    fn world_stream_chunks_are_deterministic() {
        let first = try_chunk_world_specs("arena/blockout", WorldRevision(1), sample_specs())
            .expect("sample specs fit in chunk table");
        let second = try_chunk_world_specs("arena/blockout", WorldRevision(1), sample_specs())
            .expect("sample specs fit in chunk table");

        assert_eq!(first, second);
        assert_eq!(first.len(), 1);
        assert_eq!(
            first[0].manifest_signature,
            world_stream_manifest_signature(&sample_specs())
        );
    }

    #[test]
    fn empty_world_stream_has_no_chunks() {
        let chunks = try_chunk_world_specs("arena/blockout", WorldRevision(1), Vec::new())
            .expect("empty stream is valid");

        assert!(chunks.is_empty());
    }

    #[test]
    fn chunk_count_rejects_index_wrap() {
        let too_many_specs =
            MAX_WORLD_STREAM_CHUNKS.saturating_mul(WORLD_STREAM_ENTITIES_PER_CHUNK) + 1;

        assert_eq!(
            chunk_count_for_spec_len(too_many_specs),
            Err(StreamChunkError::TooManyChunks)
        );
    }

    #[test]
    fn world_stream_signature_changes_with_rendered_fields() {
        let mut specs = sample_specs();
        let original = world_stream_manifest_signature(&specs);

        specs[0].render = Some(WorldPrimitive::Cuboid {
            size: qvec([8.0, 5.0, 6.0]),
        });

        assert_ne!(original, world_stream_manifest_signature(&specs));
    }
}
