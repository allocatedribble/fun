use bevy_ecs::prelude::Resource;

use crate::{
    ECS_SPATIAL_MAX_DECODE_OVERLAYS, ECS_SPATIAL_MAX_DECODED_PAGE_ROWS,
    ECS_SPATIAL_MAX_SOURCE_PAYLOAD_BYTES, ECS_SPATIAL_MAX_SOURCE_QUEUE_ROWS, EcsPageChannelMask,
    EcsPageFailureCode, EcsProceduralWorldManifest, EcsSourceRequestId, EcsSpatialPageKey,
    EcsSpatialRegionKey, EcsSpatialSourceId, EcsSpatialValidationError, EcsStreamPriority,
    VOXEL_CLUSTER_SUMMARIES_PER_BRICK, VoxelBrickPayload, VoxelClusterSummary,
    generate_procedural_terrain_page,
};

pub trait EcsSpatialSource {
    fn source_id(&self) -> EcsSpatialSourceId;
    fn manifest_for_region(&self, key: EcsSpatialRegionKey) -> EcsSpatialRegionManifest;
    fn request_page(&self, key: EcsSpatialPageKey) -> EcsSourceRequest;
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsSpatialSourceKind {
    #[default]
    Procedural = 0,
    CookedPackage = 1,
    EditorMemory = 2,
    SaveGameDelta = 3,
    NetworkDelta = 4,
    DebugSynthetic = 5,
}

impl EcsSpatialSourceKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Procedural => "procedural",
            Self::CookedPackage => "cooked_package",
            Self::EditorMemory => "editor_memory",
            Self::SaveGameDelta => "save_game_delta",
            Self::NetworkDelta => "network_delta",
            Self::DebugSynthetic => "debug_synthetic",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsSourceChecksumAlgorithm {
    #[default]
    None = 0,
    Fnv1a64 = 1,
    PackageCrc64 = 2,
    Blake3Truncated64 = 3,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsSourceChecksum {
    pub algorithm: EcsSourceChecksumAlgorithm,
    pub value: u64,
}

impl EcsSourceChecksum {
    pub const NONE: Self = Self {
        algorithm: EcsSourceChecksumAlgorithm::None,
        value: 0,
    };

    #[must_use]
    pub fn fnv1a64(bytes: &[u8]) -> Self {
        let mut value = 0xcbf2_9ce4_8422_2325_u64;
        for byte in bytes {
            value ^= u64::from(*byte);
            value = value.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Self {
            algorithm: EcsSourceChecksumAlgorithm::Fnv1a64,
            value,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsSpatialRegionManifest {
    pub region: EcsSpatialRegionKey,
    pub source: EcsSpatialSourceId,
    pub source_kind: EcsSpatialSourceKind,
    pub manifest_epoch: u32,
    pub page_count: u32,
    pub channel_mask: EcsPageChannelMask,
    pub checksum: EcsSourceChecksum,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsSourcePayloadCodec {
    None = 0,
    #[default]
    Zstd = 1,
    Lz4 = 2,
    ProceduralRecipe = 3,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EcsCompressedPagePayload {
    pub key: EcsSpatialPageKey,
    pub codec: EcsSourcePayloadCodec,
    pub byte_len: u32,
    pub bytes: Vec<u8>,
    pub checksum: EcsSourceChecksum,
}

impl EcsCompressedPagePayload {
    pub fn try_new(
        key: EcsSpatialPageKey,
        codec: EcsSourcePayloadCodec,
        bytes: Vec<u8>,
        checksum: EcsSourceChecksum,
    ) -> Result<Self, EcsSpatialValidationError> {
        if bytes.len() > ECS_SPATIAL_MAX_SOURCE_PAYLOAD_BYTES {
            return Err(EcsSpatialValidationError::SourcePayloadTooLarge);
        }
        Ok(Self {
            key,
            codec,
            byte_len: bytes.len() as u32,
            bytes,
            checksum,
        })
    }

    pub fn validate(&self) -> Result<(), EcsSpatialValidationError> {
        if self.bytes.len() > ECS_SPATIAL_MAX_SOURCE_PAYLOAD_BYTES {
            return Err(EcsSpatialValidationError::SourcePayloadTooLarge);
        }
        if self.byte_len as usize != self.bytes.len() {
            return Err(EcsSpatialValidationError::SourcePayloadLengthMismatch);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsProceduralRecipeRef {
    pub key: EcsSpatialPageKey,
    pub recipe_id: u64,
    pub seed: u64,
    pub generator_version: u32,
    pub manifest_signature: u64,
    pub checksum: EcsSourceChecksum,
}

impl EcsProceduralRecipeRef {
    pub fn validate(self) -> Result<(), EcsSpatialValidationError> {
        if self.recipe_id == 0
            || self.generator_version == 0
            || self.manifest_signature == 0
            || self.checksum == EcsSourceChecksum::NONE
        {
            return Err(EcsSpatialValidationError::InvalidProceduralRecipe);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsSourceFailure {
    pub key: EcsSpatialPageKey,
    pub code: EcsPageFailureCode,
    pub checksum: EcsSourceChecksum,
    pub retry_after_frame: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EcsSourcePayload {
    CompressedPage(EcsCompressedPagePayload),
    ProceduralRecipe(EcsProceduralRecipeRef),
    Failure(EcsSourceFailure),
}

impl EcsSourcePayload {
    #[must_use]
    pub fn key(&self) -> EcsSpatialPageKey {
        match self {
            Self::CompressedPage(payload) => payload.key,
            Self::ProceduralRecipe(recipe) => recipe.key,
            Self::Failure(failure) => failure.key,
        }
    }

    #[must_use]
    pub fn checksum(&self) -> EcsSourceChecksum {
        match self {
            Self::CompressedPage(payload) => payload.checksum,
            Self::ProceduralRecipe(recipe) => recipe.checksum,
            Self::Failure(failure) => failure.checksum,
        }
    }

    #[must_use]
    pub fn byte_len(&self) -> u32 {
        match self {
            Self::CompressedPage(payload) => payload.byte_len,
            Self::ProceduralRecipe(_) | Self::Failure(_) => 0,
        }
    }

    pub fn validate(&self) -> Result<(), EcsSpatialValidationError> {
        match self {
            Self::CompressedPage(payload) => payload.validate(),
            Self::ProceduralRecipe(recipe) => recipe.validate(),
            Self::Failure(_) => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EcsSourceRequest {
    pub request_id: EcsSourceRequestId,
    pub source: EcsSpatialSourceId,
    pub source_kind: EcsSpatialSourceKind,
    pub key: EcsSpatialPageKey,
    pub region: EcsSpatialRegionKey,
    pub manifest_epoch: u32,
    pub source_epoch: u32,
    pub priority: EcsStreamPriority,
    pub payload: EcsSourcePayload,
}

impl EcsSourceRequest {
    pub fn validate_for(
        &self,
        source_id: EcsSpatialSourceId,
        key: EcsSpatialPageKey,
        region: EcsSpatialRegionKey,
    ) -> Result<(), EcsSpatialValidationError> {
        if self.source != source_id || self.key != key || self.region != region {
            return Err(EcsSpatialValidationError::SourceRequestMismatch);
        }
        if self.payload.key() != key {
            return Err(EcsSpatialValidationError::SourceRequestMismatch);
        }
        self.payload.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EcsSourceAcquireRecord {
    pub request: EcsSourceRequest,
    pub manifest: EcsSpatialRegionManifest,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct EcsSourceAcquireQueue {
    pub rows: Vec<EcsSourceAcquireRecord>,
}

impl EcsSourceAcquireQueue {
    pub fn push(&mut self, row: EcsSourceAcquireRecord) -> Result<(), EcsSpatialValidationError> {
        if self.rows.len() >= ECS_SPATIAL_MAX_SOURCE_QUEUE_ROWS {
            return Err(EcsSpatialValidationError::SourceAcquireQueueFull);
        }
        self.rows.push(row);
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EcsSourceAcquireReport {
    pub requested: u32,
    pub compressed_payloads: u32,
    pub procedural_recipes: u32,
    pub failures: u32,
}

pub fn acquire_sources<S: EcsSpatialSource>(
    source: &S,
    page_keys: &[EcsSpatialPageKey],
    region_edge_pages: u16,
    output: &mut EcsSourceAcquireQueue,
) -> Result<EcsSourceAcquireReport, EcsSpatialValidationError> {
    let mut report = EcsSourceAcquireReport::default();
    for page_key in page_keys {
        let region = EcsSpatialRegionKey::from_page(*page_key, region_edge_pages);
        let manifest = source.manifest_for_region(region);
        let request = source.request_page(*page_key);
        request.validate_for(source.source_id(), *page_key, region)?;
        match &request.payload {
            EcsSourcePayload::CompressedPage(_) => report.compressed_payloads += 1,
            EcsSourcePayload::ProceduralRecipe(_) => report.procedural_recipes += 1,
            EcsSourcePayload::Failure(_) => report.failures += 1,
        }
        output.push(EcsSourceAcquireRecord { request, manifest })?;
        report.requested += 1;
    }
    Ok(report)
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsDecodeOverlayKind {
    #[default]
    SaveGameDelta = 0,
    NetworkDelta = 1,
    EditorMemory = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsDecodeOverlay {
    pub key: EcsSpatialPageKey,
    pub kind: EcsDecodeOverlayKind,
    pub epoch: u32,
    pub checksum: EcsSourceChecksum,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsDecodedPagePayloadKind {
    #[default]
    Empty = 0,
    VoxelBrick = 1,
    ProceduralRecipeRef = 2,
    SourceFailed = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsDecodeTelemetry {
    pub key: EcsSpatialPageKey,
    pub source_epoch: u32,
    pub decode_epoch: u32,
    pub source_bytes: u32,
    pub overlay_count: u16,
    pub cluster_summary_count: u16,
    pub checksum: EcsSourceChecksum,
    pub failure: Option<EcsPageFailureCode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EcsDecodedPageRecord {
    pub key: EcsSpatialPageKey,
    pub source: EcsSpatialSourceId,
    pub source_epoch: u32,
    pub payload_kind: EcsDecodedPagePayloadKind,
    pub voxel_brick: Option<VoxelBrickPayload>,
    pub cluster_summaries: [VoxelClusterSummary; VOXEL_CLUSTER_SUMMARIES_PER_BRICK],
    pub telemetry: EcsDecodeTelemetry,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct EcsDecodedPageQueue {
    pub rows: Vec<EcsDecodedPageRecord>,
}

impl EcsDecodedPageQueue {
    pub fn push(&mut self, row: EcsDecodedPageRecord) -> Result<(), EcsSpatialValidationError> {
        if self.rows.len() >= ECS_SPATIAL_MAX_DECODED_PAGE_ROWS {
            return Err(EcsSpatialValidationError::DecodeQueueFull);
        }
        self.rows.push(row);
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EcsDecodeReport {
    pub decoded: u32,
    pub procedural: u32,
    pub failed: u32,
    pub overlays_read: u32,
}

pub fn decode_pages(
    acquired: &EcsSourceAcquireQueue,
    overlays: &[EcsDecodeOverlay],
    decode_epoch: u32,
    output: &mut EcsDecodedPageQueue,
) -> Result<EcsDecodeReport, EcsSpatialValidationError> {
    decode_pages_inner(acquired, overlays, decode_epoch, None, output)
}

pub fn decode_pages_with_procedural_manifest(
    acquired: &EcsSourceAcquireQueue,
    overlays: &[EcsDecodeOverlay],
    decode_epoch: u32,
    manifest: EcsProceduralWorldManifest,
    output: &mut EcsDecodedPageQueue,
) -> Result<EcsDecodeReport, EcsSpatialValidationError> {
    manifest.validate()?;
    decode_pages_inner(acquired, overlays, decode_epoch, Some(manifest), output)
}

fn decode_pages_inner(
    acquired: &EcsSourceAcquireQueue,
    overlays: &[EcsDecodeOverlay],
    decode_epoch: u32,
    procedural_manifest: Option<EcsProceduralWorldManifest>,
    output: &mut EcsDecodedPageQueue,
) -> Result<EcsDecodeReport, EcsSpatialValidationError> {
    if overlays.len() > ECS_SPATIAL_MAX_DECODE_OVERLAYS {
        return Err(EcsSpatialValidationError::DecodeOverlayQueueFull);
    }

    let mut report = EcsDecodeReport::default();
    for row in &acquired.rows {
        let overlay_count = overlays
            .iter()
            .filter(|overlay| overlay.key == row.request.key)
            .count();
        if overlay_count > u16::MAX as usize {
            return Err(EcsSpatialValidationError::DecodeOverlayQueueFull);
        }
        let record =
            decode_page_record(row, overlay_count as u16, decode_epoch, procedural_manifest)?;
        match record.payload_kind {
            EcsDecodedPagePayloadKind::VoxelBrick | EcsDecodedPagePayloadKind::Empty => {
                report.decoded += 1;
            }
            EcsDecodedPagePayloadKind::ProceduralRecipeRef => report.procedural += 1,
            EcsDecodedPagePayloadKind::SourceFailed => report.failed += 1,
        }
        report.overlays_read += u32::from(overlay_count as u16);
        output.push(record)?;
    }
    Ok(report)
}

fn decode_page_record(
    row: &EcsSourceAcquireRecord,
    overlay_count: u16,
    decode_epoch: u32,
    procedural_manifest: Option<EcsProceduralWorldManifest>,
) -> Result<EcsDecodedPageRecord, EcsSpatialValidationError> {
    row.request.payload.validate()?;
    let key = row.request.key;
    let empty_clusters = [VoxelClusterSummary::default(); VOXEL_CLUSTER_SUMMARIES_PER_BRICK];
    match &row.request.payload {
        EcsSourcePayload::CompressedPage(payload) => {
            let voxel_brick = if payload.bytes.is_empty() {
                VoxelBrickPayload::empty(key, row.request.source_epoch)
            } else {
                VoxelBrickPayload::uniform_solid(
                    key,
                    u16::from(payload.bytes[0]),
                    row.request.source_epoch,
                )
            };
            let cluster_summaries = voxel_brick.clusters;
            Ok(EcsDecodedPageRecord {
                key,
                source: row.request.source,
                source_epoch: row.request.source_epoch,
                payload_kind: if payload.bytes.is_empty() {
                    EcsDecodedPagePayloadKind::Empty
                } else {
                    EcsDecodedPagePayloadKind::VoxelBrick
                },
                voxel_brick: Some(voxel_brick),
                cluster_summaries,
                telemetry: EcsDecodeTelemetry {
                    key,
                    source_epoch: row.request.source_epoch,
                    decode_epoch,
                    source_bytes: payload.byte_len,
                    overlay_count,
                    cluster_summary_count: VOXEL_CLUSTER_SUMMARIES_PER_BRICK as u16,
                    checksum: payload.checksum,
                    failure: None,
                },
            })
        }
        EcsSourcePayload::ProceduralRecipe(recipe) => {
            if let Some(manifest) = procedural_manifest {
                generate_procedural_terrain_page(
                    manifest,
                    *recipe,
                    row.request.source,
                    row.request.source_epoch,
                    decode_epoch,
                    overlay_count,
                )
            } else {
                Ok(EcsDecodedPageRecord {
                    key,
                    source: row.request.source,
                    source_epoch: row.request.source_epoch,
                    payload_kind: EcsDecodedPagePayloadKind::ProceduralRecipeRef,
                    voxel_brick: None,
                    cluster_summaries: empty_clusters,
                    telemetry: EcsDecodeTelemetry {
                        key,
                        source_epoch: row.request.source_epoch,
                        decode_epoch,
                        source_bytes: 0,
                        overlay_count,
                        cluster_summary_count: 0,
                        checksum: recipe.checksum,
                        failure: None,
                    },
                })
            }
        }
        EcsSourcePayload::Failure(failure) => Ok(EcsDecodedPageRecord {
            key,
            source: row.request.source,
            source_epoch: row.request.source_epoch,
            payload_kind: EcsDecodedPagePayloadKind::SourceFailed,
            voxel_brick: None,
            cluster_summaries: empty_clusters,
            telemetry: EcsDecodeTelemetry {
                key,
                source_epoch: row.request.source_epoch,
                decode_epoch,
                source_bytes: 0,
                overlay_count,
                cluster_summary_count: 0,
                checksum: failure.checksum,
                failure: Some(failure.code),
            },
        }),
    }
}
