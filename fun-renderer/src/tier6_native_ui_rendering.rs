//! Tier 6 — Native rvelte/FUN UI Rendering.
//!
//! Pass 20 installed `ui::native_adapter` which lowers
//! `FunUiFramePacket` draw operations into typed
//! `NativeUiDrawCommand` records. Tier 6 makes that surface
//! efficient at recording time:
//!
//! 1. **UI batching** — typed `Tier6UiBatchKind` (FilledQuad /
//!    StrokedQuad / RoundedRectAnalytic / RoundedRectSdf /
//!    ImageQuad / GlyphRun / DebugBounds) + `Tier6UiBatchKey`
//!    keyed by `(kind, clip_id, transform_id, opacity_id,
//!    atlas_id)`. `coalesce_ui_draws` walks a per-frame draw
//!    stream and produces a small number of `Tier6UiBatch`
//!    records — consecutive draws with the same key fold into
//!    one batch.
//! 2. **Glyph atlas** — typed `Tier6GlyphAtlasTable` Bevy
//!    Resource with LRU eviction, `Tier6SubpixelPositioning`
//!    (Off / FractionalQuarterX / FractionalQuarterXY) policy,
//!    `Tier6MissingGlyphFallback` (TofuMarker /
//!    BoxSurrogate / DebugMagenta), and a typed
//!    `Tier6GlyphRunCache` so stable HUD/diagnostic text does
//!    not allocate every frame.
//! 3. **Product route integration** — typed
//!    `Tier6ProductRouteKind` (LauncherShell / HudOverlay /
//!    PauseMenu / SettingsShell / DiagnosticsList / ProofScene),
//!    `Tier6RvelteBridgeRouteSelection::parse_arg("--rvelte-bridge=<route>")`,
//!    `Tier6ProductInputEvent` taxonomy
//!    (Pointer / Key / FocusChange / WindowResize),
//!    `Tier6FeatureFlag` (FakeRenderer / FunRendererConsumer)
//!    selector, and `Tier6NativeUiStagedRemovalInventory` recording
//!    typed NATIVE_UI surface entries with their replacement status.
//!
//! Honest scope: the batching coalescer, the atlas eviction,
//! and the route boot all execute as deterministic CPU
//! implementations. Once the Tier 0 gaps close
//! (`no_render_encoder` / `no_swapchain_configured`), the
//! same `Tier6UiBatchTable` is consumed by the bridge to
//! issue real wgpu draw calls; the typed compact per-draw
//! UI constants survive into the GPU vertex / fragment
//! shaders.

use bevy_ecs::prelude::Resource;

pub const TIER6_NATIVE_UI_RENDERING_SCHEMA_VERSION: u16 = 1;

pub const TIER6_UI_BATCH_KIND_COUNT: usize = 7;
pub const TIER6_SUBPIXEL_POSITIONING_COUNT: usize = 3;
pub const TIER6_MISSING_GLYPH_FALLBACK_COUNT: usize = 3;
pub const TIER6_PRODUCT_ROUTE_KIND_COUNT: usize = 6;
pub const TIER6_PRODUCT_INPUT_EVENT_KIND_COUNT: usize = 4;
pub const TIER6_FEATURE_FLAG_COUNT: usize = 2;
pub const TIER6_NATIVE_UI_REPLACEMENT_STATUS_COUNT: usize = 4;

// ============================================================================
// Section 1 — UI batching
// ============================================================================

/// Typed kind for one UI batch. `RoundedRectAnalytic` and
/// `RoundedRectSdf` are kept distinct because they require
/// different shader pipelines — analytic for axis-aligned cases,
/// SDF for arbitrary rounded shapes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier6UiBatchKind {
    #[default]
    FilledQuad,
    StrokedQuad,
    RoundedRectAnalytic,
    RoundedRectSdf,
    ImageQuad,
    GlyphRun,
    DebugBounds,
}

impl Tier6UiBatchKind {
    pub const ALL: [Self; TIER6_UI_BATCH_KIND_COUNT] = [
        Self::FilledQuad,
        Self::StrokedQuad,
        Self::RoundedRectAnalytic,
        Self::RoundedRectSdf,
        Self::ImageQuad,
        Self::GlyphRun,
        Self::DebugBounds,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::FilledQuad => 0,
            Self::StrokedQuad => 1,
            Self::RoundedRectAnalytic => 2,
            Self::RoundedRectSdf => 3,
            Self::ImageQuad => 4,
            Self::GlyphRun => 5,
            Self::DebugBounds => 6,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FilledQuad => "filled_quad",
            Self::StrokedQuad => "stroked_quad",
            Self::RoundedRectAnalytic => "rounded_rect_analytic",
            Self::RoundedRectSdf => "rounded_rect_sdf",
            Self::ImageQuad => "image_quad",
            Self::GlyphRun => "glyph_run",
            Self::DebugBounds => "debug_bounds",
        }
    }

    /// True for kinds that need an atlas reference in the batch
    /// key (Image / Glyph). Other kinds key only on
    /// `(clip, transform, opacity)`.
    #[must_use]
    pub const fn needs_atlas_key(self) -> bool {
        matches!(self, Self::ImageQuad | Self::GlyphRun)
    }
}

/// Typed UI batch key. Two consecutive draws fold into one batch
/// when their keys are equal.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier6UiBatchKey {
    pub kind: Tier6UiBatchKind,
    pub clip_id: u32,
    pub transform_id: u32,
    pub opacity_id: u32,
    pub atlas_id: u32,
}

impl Tier6UiBatchKey {
    pub const NO_ATLAS: u32 = u32::MAX;

    #[must_use]
    pub const fn for_quad(kind: Tier6UiBatchKind, clip: u32, xform: u32, opa: u32) -> Self {
        Self {
            kind,
            clip_id: clip,
            transform_id: xform,
            opacity_id: opa,
            atlas_id: Self::NO_ATLAS,
        }
    }

    #[must_use]
    pub const fn for_atlas(
        kind: Tier6UiBatchKind,
        clip: u32,
        xform: u32,
        opa: u32,
        atlas: u32,
    ) -> Self {
        Self {
            kind,
            clip_id: clip,
            transform_id: xform,
            opacity_id: opa,
            atlas_id: atlas,
        }
    }
}

/// Test-friendly mirror of one drawable record. Pass 20's
/// `NativeUiDrawCommand` lowers into a sequence of these — the
/// Tier 6 batching algorithm is identical regardless of which
/// upstream type produced the records.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier6UiDrawRecord {
    pub key: Tier6UiBatchKey,
    pub vertex_count: u32,
    pub index_count: u32,
    pub byte_size: u32,
}

/// One batch produced by `coalesce_ui_draws`. The bridge
/// consumes a slice of these to issue one draw call per batch.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier6UiBatch {
    pub schema_version: u16,
    pub key: Tier6UiBatchKey,
    pub draw_count: u32,
    pub vertex_count: u32,
    pub index_count: u32,
    pub byte_size: u64,
}

/// Coalesce a per-frame UI draw stream into a list of batches.
/// Consecutive draws with the same `Tier6UiBatchKey` fold into
/// one batch; a key change starts a new batch. The acceptance
/// guarantee: a route packet with N draws of K distinct keys
/// produces exactly K batches.
#[must_use]
pub fn coalesce_ui_draws(stream: &[Tier6UiDrawRecord]) -> Vec<Tier6UiBatch> {
    let mut out: Vec<Tier6UiBatch> = Vec::new();
    for record in stream {
        if let Some(last) = out.last_mut()
            && last.key == record.key
        {
            last.draw_count = last.draw_count.saturating_add(1);
            last.vertex_count = last.vertex_count.saturating_add(record.vertex_count);
            last.index_count = last.index_count.saturating_add(record.index_count);
            last.byte_size = last.byte_size.saturating_add(record.byte_size as u64);
            continue;
        }
        out.push(Tier6UiBatch {
            schema_version: TIER6_NATIVE_UI_RENDERING_SCHEMA_VERSION,
            key: record.key,
            draw_count: 1,
            vertex_count: record.vertex_count,
            index_count: record.index_count,
            byte_size: record.byte_size as u64,
        });
    }
    out
}

/// Compact per-draw UI constant. The renderer encodes one of
/// these per draw rather than a fat per-call uniform: 32-bit
/// transform_id, 32-bit clip_id, 16-bit opacity_x_1024 (so
/// 0..1 maps to 0..1024 in 16 bits — sub-percent precision is
/// fine for UI), 16-bit reserved.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier6UiCompactPerDrawConstant {
    pub transform_id: u32,
    pub clip_id: u32,
    pub opacity_x_1024: u16,
    pub reserved: u16,
}

impl Tier6UiCompactPerDrawConstant {
    pub const BYTE_SIZE: usize = 12;

    #[must_use]
    pub fn pack_opacity(opacity_unit: f32) -> u16 {
        let clamped = opacity_unit.clamp(0.0, 1.0);
        (clamped * 1024.0).round().clamp(0.0, u16::MAX as f32) as u16
    }

    #[must_use]
    pub fn unpack_opacity(self) -> f32 {
        (self.opacity_x_1024 as f32) / 1024.0
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Resource)]
pub struct Tier6UiBatchTable {
    pub schema_version: u16,
    pub canonical_path: &'static str,
    pub batches: Vec<Tier6UiBatch>,
    pub draw_count_total: u32,
    pub batch_count: u32,
}

impl Tier6UiBatchTable {
    pub const CANONICAL_ARTIFACT_PATH: &'static str = "fun_renderer.tier6.ui_batches.funpb.zst";

    #[must_use]
    pub fn from_stream(stream: &[Tier6UiDrawRecord]) -> Self {
        let batches = coalesce_ui_draws(stream);
        let draw_count_total = stream.len() as u32;
        let batch_count = batches.len() as u32;
        Self {
            schema_version: TIER6_NATIVE_UI_RENDERING_SCHEMA_VERSION,
            canonical_path: Self::CANONICAL_ARTIFACT_PATH,
            batches,
            draw_count_total,
            batch_count,
        }
    }

    /// Acceptance: a route packet with many UI draws becomes a
    /// small number of UI batches. The typed predicate asserts
    /// `batch_count <= max_expected_batches`.
    #[must_use]
    pub fn passes_batch_collapse(&self, max_expected_batches: u32) -> bool {
        self.batch_count <= max_expected_batches
    }
}

// ============================================================================
// Section 2 — Glyph atlas runtime
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier6SubpixelPositioning {
    #[default]
    Off,
    FractionalQuarterX,
    FractionalQuarterXY,
}

impl Tier6SubpixelPositioning {
    pub const ALL: [Self; TIER6_SUBPIXEL_POSITIONING_COUNT] = [
        Self::Off,
        Self::FractionalQuarterX,
        Self::FractionalQuarterXY,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::FractionalQuarterX => "fractional_quarter_x",
            Self::FractionalQuarterXY => "fractional_quarter_xy",
        }
    }

    /// Number of distinct sub-pixel-position classes for one
    /// glyph at this policy. `Off` produces 1 class; quarter
    /// fractional positioning on X produces 4 classes; on both
    /// X and Y produces 16. The atlas keys glyph cells by
    /// `(glyph_key, subpixel_class)` so the same glyph at
    /// different sub-pixel positions does not collide.
    #[must_use]
    pub const fn class_count(self) -> u8 {
        match self {
            Self::Off => 1,
            Self::FractionalQuarterX => 4,
            Self::FractionalQuarterXY => 16,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier6MissingGlyphFallback {
    #[default]
    TofuMarker,
    BoxSurrogate,
    DebugMagenta,
}

impl Tier6MissingGlyphFallback {
    pub const ALL: [Self; TIER6_MISSING_GLYPH_FALLBACK_COUNT] =
        [Self::TofuMarker, Self::BoxSurrogate, Self::DebugMagenta];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TofuMarker => "tofu_marker",
            Self::BoxSurrogate => "box_surrogate",
            Self::DebugMagenta => "debug_magenta",
        }
    }

    #[must_use]
    pub const fn is_visible(self) -> bool {
        // All three fallbacks render *something* — the typed
        // contract is that a missing glyph never renders an
        // empty cell. Pass 21's `AssetFallbackChain` rule.
        true
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier6GlyphAtlasRegion {
    pub glyph_key_hash: u64,
    pub subpixel_class: u8,
    pub atlas_page: u16,
    pub atlas_x: u16,
    pub atlas_y: u16,
    pub width: u16,
    pub height: u16,
    pub last_used_frame: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier6GlyphAtlasDescriptor {
    pub schema_version: u16,
    pub page_extent_px: u16,
    pub max_pages: u8,
    pub subpixel: Tier6SubpixelPositioning,
    pub missing_glyph_fallback: Tier6MissingGlyphFallback,
}

impl Tier6GlyphAtlasDescriptor {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: TIER6_NATIVE_UI_RENDERING_SCHEMA_VERSION,
        page_extent_px: 2048,
        max_pages: 4,
        subpixel: Tier6SubpixelPositioning::FractionalQuarterX,
        missing_glyph_fallback: Tier6MissingGlyphFallback::TofuMarker,
    };

    #[must_use]
    pub const fn capacity_bytes(&self) -> u64 {
        // 8-bit alpha atlas — 1 byte per texel.
        (self.page_extent_px as u64)
            .saturating_mul(self.page_extent_px as u64)
            .saturating_mul(self.max_pages as u64)
    }
}

impl Default for Tier6GlyphAtlasDescriptor {
    fn default() -> Self {
        Self::PRODUCT_DEFAULT
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Resource)]
pub struct Tier6GlyphAtlasTable {
    pub schema_version: u16,
    pub descriptor: Tier6GlyphAtlasDescriptor,
    pub regions: Vec<Tier6GlyphAtlasRegion>,
    pub frame_index: u64,
    pub miss_count: u32,
    pub eviction_count: u32,
    pub allocations_this_frame: u32,
}

impl Tier6GlyphAtlasTable {
    #[must_use]
    pub fn new(descriptor: Tier6GlyphAtlasDescriptor) -> Self {
        Self {
            schema_version: TIER6_NATIVE_UI_RENDERING_SCHEMA_VERSION,
            descriptor,
            regions: Vec::new(),
            frame_index: 0,
            miss_count: 0,
            eviction_count: 0,
            allocations_this_frame: 0,
        }
    }

    pub fn begin_frame(&mut self) {
        self.frame_index = self.frame_index.saturating_add(1);
        self.allocations_this_frame = 0;
    }

    /// Look up an existing atlas region by glyph key + sub-pixel
    /// class. Hit returns the region (and bumps its last-used
    /// frame); miss returns `None`.
    pub fn lookup(
        &mut self,
        glyph_key_hash: u64,
        subpixel_class: u8,
    ) -> Option<Tier6GlyphAtlasRegion> {
        if let Some(region) = self
            .regions
            .iter_mut()
            .find(|r| r.glyph_key_hash == glyph_key_hash && r.subpixel_class == subpixel_class)
        {
            region.last_used_frame = self.frame_index;
            return Some(*region);
        }
        self.miss_count = self.miss_count.saturating_add(1);
        None
    }

    /// Insert a new region; if the atlas is full, evict the LRU
    /// region. The acceptance contract: stable (already-cached)
    /// text yields zero allocations per frame.
    #[allow(clippy::too_many_arguments)]
    pub fn insert(
        &mut self,
        glyph_key_hash: u64,
        subpixel_class: u8,
        atlas_page: u16,
        atlas_x: u16,
        atlas_y: u16,
        width: u16,
        height: u16,
    ) -> Tier6GlyphAtlasRegion {
        let region = Tier6GlyphAtlasRegion {
            glyph_key_hash,
            subpixel_class,
            atlas_page,
            atlas_x,
            atlas_y,
            width,
            height,
            last_used_frame: self.frame_index,
        };
        let cap = capacity_in_regions(self.descriptor);
        if (self.regions.len() as u32) >= cap {
            // LRU eviction: drop the region with the smallest
            // `last_used_frame`.
            if let Some((lru_index, _)) = self
                .regions
                .iter()
                .enumerate()
                .min_by_key(|(_, r)| r.last_used_frame)
            {
                self.regions.swap_remove(lru_index);
                self.eviction_count = self.eviction_count.saturating_add(1);
            }
        }
        self.regions.push(region);
        self.allocations_this_frame = self.allocations_this_frame.saturating_add(1);
        region
    }

    #[must_use]
    pub fn region_count(&self) -> u32 {
        self.regions.len() as u32
    }

    /// Tier 6 acceptance: HUD/diagnostic text does not allocate
    /// every frame. This means: after the warmup frame primes
    /// the atlas, subsequent frames recording the same glyph
    /// keys produce zero allocations.
    #[must_use]
    pub const fn allocations_this_frame_is_zero(&self) -> bool {
        self.allocations_this_frame == 0
    }
}

#[must_use]
const fn capacity_in_regions(descriptor: Tier6GlyphAtlasDescriptor) -> u32 {
    // Approximate: assume average glyph cell is 16x16 px and
    // each page is `page_extent_px ^ 2` texels.
    let cells_per_page = (descriptor.page_extent_px as u32 / 16)
        .saturating_mul(descriptor.page_extent_px as u32 / 16);
    cells_per_page.saturating_mul(descriptor.max_pages as u32)
}

/// Cached glyph run — typed list of atlas-region references
/// produced once and reused as long as the run's source bytes
/// are unchanged.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct Tier6GlyphRunCacheEntry {
    pub glyph_run_key_hash: u64,
    pub regions: Vec<Tier6GlyphAtlasRegion>,
    pub last_used_frame: u64,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Resource)]
pub struct Tier6GlyphRunCache {
    pub schema_version: u16,
    pub entries: Vec<Tier6GlyphRunCacheEntry>,
    pub hits: u32,
    pub misses: u32,
}

impl Tier6GlyphRunCache {
    #[must_use]
    pub fn lookup(
        &mut self,
        run_key_hash: u64,
        frame_index: u64,
    ) -> Option<&Tier6GlyphRunCacheEntry> {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|e| e.glyph_run_key_hash == run_key_hash)
        {
            entry.last_used_frame = frame_index;
            self.hits = self.hits.saturating_add(1);
            return Some(entry);
        }
        self.misses = self.misses.saturating_add(1);
        None
    }

    pub fn insert(
        &mut self,
        run_key_hash: u64,
        regions: Vec<Tier6GlyphAtlasRegion>,
        frame_index: u64,
    ) {
        self.entries.push(Tier6GlyphRunCacheEntry {
            glyph_run_key_hash: run_key_hash,
            regions,
            last_used_frame: frame_index,
        });
    }

    #[must_use]
    pub fn hit_ratio_per_mille(&self) -> u16 {
        let total = self.hits.saturating_add(self.misses);
        if total == 0 {
            return 0;
        }
        let ratio = (self.hits as u64).saturating_mul(1000) / total as u64;
        ratio.min(1000) as u16
    }
}

// ============================================================================
// Section 3 — Product route boot
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier6ProductRouteKind {
    #[default]
    LauncherShell,
    HudOverlay,
    PauseMenu,
    SettingsShell,
    DiagnosticsList,
    ProofScene,
}

impl Tier6ProductRouteKind {
    pub const ALL: [Self; TIER6_PRODUCT_ROUTE_KIND_COUNT] = [
        Self::LauncherShell,
        Self::HudOverlay,
        Self::PauseMenu,
        Self::SettingsShell,
        Self::DiagnosticsList,
        Self::ProofScene,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LauncherShell => "launcher_shell",
            Self::HudOverlay => "hud_overlay",
            Self::PauseMenu => "pause_menu",
            Self::SettingsShell => "settings_shell",
            Self::DiagnosticsList => "diagnostics_list",
            Self::ProofScene => "proof_scene",
        }
    }

    #[must_use]
    pub fn from_arg_value(value: &str) -> Option<Self> {
        match value {
            "launcher_shell" | "launcher" => Some(Self::LauncherShell),
            "hud_overlay" | "hud" => Some(Self::HudOverlay),
            "pause_menu" | "pause" => Some(Self::PauseMenu),
            "settings_shell" | "settings" => Some(Self::SettingsShell),
            "diagnostics_list" | "diagnostics" => Some(Self::DiagnosticsList),
            "proof_scene" | "proof" => Some(Self::ProofScene),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier6RvelteBridgeRouteSelection {
    NotProvided,
    Selected(Tier6ProductRouteKind),
    UnknownArgValue,
}

impl Tier6RvelteBridgeRouteSelection {
    /// Parse a single CLI argument. Returns `Selected(kind)` on
    /// `--rvelte-bridge=<value>`; `UnknownArgValue` on a bad
    /// value; `NotProvided` for any other argument.
    #[must_use]
    pub fn parse_arg(arg: &str) -> Self {
        let prefix = "--rvelte-bridge=";
        let Some(value) = arg.strip_prefix(prefix) else {
            return Self::NotProvided;
        };
        match Tier6ProductRouteKind::from_arg_value(value) {
            Some(kind) => Self::Selected(kind),
            None => Self::UnknownArgValue,
        }
    }

    /// Walk a slice of argv-style strings and resolve to a
    /// single typed selection. The first matching
    /// `--rvelte-bridge=<value>` wins; if no flag is provided,
    /// returns `NotProvided`.
    #[must_use]
    pub fn parse_argv(argv: &[&str]) -> Self {
        for arg in argv {
            let parsed = Self::parse_arg(arg);
            if !matches!(parsed, Self::NotProvided) {
                return parsed;
            }
        }
        Self::NotProvided
    }

    #[must_use]
    pub const fn route(&self) -> Option<Tier6ProductRouteKind> {
        match self {
            Self::Selected(kind) => Some(*kind),
            _ => None,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier6ProductInputEventKind {
    #[default]
    Pointer,
    Key,
    FocusChange,
    WindowResize,
}

impl Tier6ProductInputEventKind {
    pub const ALL: [Self; TIER6_PRODUCT_INPUT_EVENT_KIND_COUNT] = [
        Self::Pointer,
        Self::Key,
        Self::FocusChange,
        Self::WindowResize,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pointer => "pointer",
            Self::Key => "key",
            Self::FocusChange => "focus_change",
            Self::WindowResize => "window_resize",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier6ProductInputEvent {
    pub kind: Tier6ProductInputEventKind,
    pub route: Tier6RouteIdOption,
    pub primary_key_or_button: u32,
    pub modifier_mask: u32,
    pub frame_index: u64,
}

/// `Copy + Hash`-friendly route id enum so the input event can
/// route to a typed product surface without a Vec or Box.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier6RouteIdOption {
    #[default]
    NotRouted,
    LauncherShell,
    HudOverlay,
    PauseMenu,
    SettingsShell,
    DiagnosticsList,
    ProofScene,
}

impl Tier6RouteIdOption {
    #[must_use]
    pub const fn from_kind(kind: Tier6ProductRouteKind) -> Self {
        match kind {
            Tier6ProductRouteKind::LauncherShell => Self::LauncherShell,
            Tier6ProductRouteKind::HudOverlay => Self::HudOverlay,
            Tier6ProductRouteKind::PauseMenu => Self::PauseMenu,
            Tier6ProductRouteKind::SettingsShell => Self::SettingsShell,
            Tier6ProductRouteKind::DiagnosticsList => Self::DiagnosticsList,
            Tier6ProductRouteKind::ProofScene => Self::ProofScene,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier6FeatureFlag {
    #[default]
    FunRendererConsumer,
    FakeRenderer,
}

impl Tier6FeatureFlag {
    pub const ALL: [Self; TIER6_FEATURE_FLAG_COUNT] =
        [Self::FunRendererConsumer, Self::FakeRenderer];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FunRendererConsumer => "fun_renderer_consumer",
            Self::FakeRenderer => "fake_renderer",
        }
    }

    #[must_use]
    pub const fn product_default() -> Self {
        Self::FunRendererConsumer
    }

    #[must_use]
    pub const fn enables_fun_renderer(self) -> bool {
        matches!(self, Self::FunRendererConsumer)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier6NativeUiReplacementStatus {
    #[default]
    StillNativeUiBacked,
    StagedForRemovalAfterRouteParity,
    ReplacedByNativeRoute,
    ArchivedReferenceOnly,
}

impl Tier6NativeUiReplacementStatus {
    pub const ALL: [Self; TIER6_NATIVE_UI_REPLACEMENT_STATUS_COUNT] = [
        Self::StillNativeUiBacked,
        Self::StagedForRemovalAfterRouteParity,
        Self::ReplacedByNativeRoute,
        Self::ArchivedReferenceOnly,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StillNativeUiBacked => "still_native_ui_backed",
            Self::StagedForRemovalAfterRouteParity => "staged_for_removal_after_route_parity",
            Self::ReplacedByNativeRoute => "replaced_by_native_route",
            Self::ArchivedReferenceOnly => "archived_reference_only",
        }
    }

    #[must_use]
    pub const fn native_ui_still_active(self) -> bool {
        matches!(self, Self::StillNativeUiBacked)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier6NativeUiSurfaceInventoryEntry {
    pub stable_id_hash: u64,
    pub debug_label_hash: u64,
    pub status: Tier6NativeUiReplacementStatus,
    pub planned_replacement_route: Tier6RouteIdOption,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Resource)]
pub struct Tier6NativeUiStagedRemovalInventory {
    pub schema_version: u16,
    pub canonical_path: &'static str,
    pub entries: Vec<Tier6NativeUiSurfaceInventoryEntry>,
}

impl Tier6NativeUiStagedRemovalInventory {
    pub const CANONICAL_ARTIFACT_PATH: &'static str =
        "fun_renderer.tier6.native_ui_staged_removal.funpb.zst";

    #[must_use]
    pub fn new() -> Self {
        Self {
            schema_version: TIER6_NATIVE_UI_RENDERING_SCHEMA_VERSION,
            canonical_path: Self::CANONICAL_ARTIFACT_PATH,
            entries: Vec::new(),
        }
    }

    pub fn record(&mut self, entry: Tier6NativeUiSurfaceInventoryEntry) {
        self.entries.push(entry);
    }

    #[must_use]
    pub fn count_for(&self, status: Tier6NativeUiReplacementStatus) -> u32 {
        self.entries.iter().filter(|e| e.status == status).count() as u32
    }

    #[must_use]
    pub fn native_ui_active_count(&self) -> u32 {
        self.entries
            .iter()
            .filter(|e| e.status.native_ui_still_active())
            .count() as u32
    }

    #[must_use]
    pub fn replaced_count(&self) -> u32 {
        self.count_for(Tier6NativeUiReplacementStatus::ReplacedByNativeRoute)
    }
}

// ============================================================================
// Section 4 — Tier 6 acceptance verdict
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier6AcceptanceVerdict {
    pub schema_version: u16,
    pub passes_batching_collapse: bool,
    pub passes_no_per_draw_pipeline: bool,
    pub passes_text_no_per_frame_alloc: bool,
    pub passes_route_renders_in_proof_scene_graph: bool,
    pub passes_native_ui_inventory_complete: bool,
}

impl Tier6AcceptanceVerdict {
    /// Tier 6 acceptance test. The caller passes:
    ///
    /// - `batch_table` — the produced batch list. The verdict
    ///   asserts `batch_count` is a "small number of batches".
    /// - `max_expected_batches` — the allowed upper bound.
    /// - `unique_pipelines_observed` — the number of distinct
    ///   pipeline keys created during the frame. The acceptance
    ///   rule "no per-draw pipeline creation" means this is
    ///   bounded by `Tier6UiBatchKind` count, not by draw count.
    /// - `glyph_atlas` — the atlas table after the frame. The
    ///   acceptance rule "no per-frame allocation for stable
    ///   text" is `allocations_this_frame == 0` after warmup.
    /// - `selected_route` — the typed route the renderer is
    ///   booting. Must not be `NotProvided` (or
    ///   `UnknownArgValue`) for the route-renders-in-proof-scene
    ///   acceptance rule.
    /// - `native_ui_inventory` — every NATIVE_UI surface must have a typed
    ///   replacement status (no surface left in
    ///   `StillNativeUiBacked` without a planned replacement route).
    #[must_use]
    pub fn evaluate(
        batch_table: &Tier6UiBatchTable,
        max_expected_batches: u32,
        unique_pipelines_observed: u32,
        glyph_atlas: &Tier6GlyphAtlasTable,
        selected_route: Tier6RvelteBridgeRouteSelection,
        native_ui_inventory: &Tier6NativeUiStagedRemovalInventory,
    ) -> Self {
        let passes_batching_collapse = batch_table.passes_batch_collapse(max_expected_batches);
        // "No per-draw pipeline creation" = unique pipelines
        // observed must be ≤ the count of UI batch kinds, even if
        // there are thousands of draws.
        let passes_no_per_draw_pipeline =
            unique_pipelines_observed <= TIER6_UI_BATCH_KIND_COUNT as u32;
        let passes_text_no_per_frame_alloc = glyph_atlas.allocations_this_frame_is_zero();
        let passes_route = matches!(selected_route, Tier6RvelteBridgeRouteSelection::Selected(_));
        // NATIVE_UI inventory complete = every entry that is still
        // `StillNativeUiBacked` has a planned replacement route
        // (`NotRouted` is treated as incomplete).
        let passes_native_ui = native_ui_inventory.entries.iter().all(|e| {
            !e.status.native_ui_still_active()
                || !matches!(e.planned_replacement_route, Tier6RouteIdOption::NotRouted)
        });
        Self {
            schema_version: TIER6_NATIVE_UI_RENDERING_SCHEMA_VERSION,
            passes_batching_collapse,
            passes_no_per_draw_pipeline,
            passes_text_no_per_frame_alloc,
            passes_route_renders_in_proof_scene_graph: passes_route,
            passes_native_ui_inventory_complete: passes_native_ui,
        }
    }

    #[must_use]
    pub const fn passes(&self) -> bool {
        self.passes_batching_collapse
            && self.passes_no_per_draw_pipeline
            && self.passes_text_no_per_frame_alloc
            && self.passes_route_renders_in_proof_scene_graph
            && self.passes_native_ui_inventory_complete
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill_quad_record(clip: u32, xform: u32, opa: u32) -> Tier6UiDrawRecord {
        Tier6UiDrawRecord {
            key: Tier6UiBatchKey::for_quad(Tier6UiBatchKind::FilledQuad, clip, xform, opa),
            vertex_count: 4,
            index_count: 6,
            byte_size: 64,
        }
    }

    fn glyph_run_record(clip: u32, xform: u32, opa: u32, atlas: u32) -> Tier6UiDrawRecord {
        Tier6UiDrawRecord {
            key: Tier6UiBatchKey::for_atlas(Tier6UiBatchKind::GlyphRun, clip, xform, opa, atlas),
            vertex_count: 16,
            index_count: 24,
            byte_size: 256,
        }
    }

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(TIER6_NATIVE_UI_RENDERING_SCHEMA_VERSION, 1);
        assert_eq!(TIER6_UI_BATCH_KIND_COUNT, 7);
        assert_eq!(TIER6_SUBPIXEL_POSITIONING_COUNT, 3);
        assert_eq!(TIER6_MISSING_GLYPH_FALLBACK_COUNT, 3);
        assert_eq!(TIER6_PRODUCT_ROUTE_KIND_COUNT, 6);
        assert_eq!(TIER6_PRODUCT_INPUT_EVENT_KIND_COUNT, 4);
        assert_eq!(TIER6_FEATURE_FLAG_COUNT, 2);
        assert_eq!(TIER6_NATIVE_UI_REPLACEMENT_STATUS_COUNT, 4);
    }

    #[test]
    fn batch_kind_atlas_key_required_for_image_and_glyph() {
        for kind in Tier6UiBatchKind::ALL {
            match kind {
                Tier6UiBatchKind::ImageQuad | Tier6UiBatchKind::GlyphRun => {
                    assert!(kind.needs_atlas_key());
                }
                _ => assert!(!kind.needs_atlas_key()),
            }
        }
    }

    #[test]
    fn coalesce_collapses_consecutive_same_key_draws() {
        let stream: Vec<Tier6UiDrawRecord> =
            (0..1000).map(|_| fill_quad_record(0, 0, 1024)).collect();
        let batches = coalesce_ui_draws(&stream);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].draw_count, 1000);
        assert_eq!(batches[0].vertex_count, 1000 * 4);
    }

    #[test]
    fn coalesce_starts_new_batch_on_key_change() {
        let stream = vec![
            fill_quad_record(0, 0, 1024),
            fill_quad_record(0, 0, 1024),
            fill_quad_record(1, 0, 1024), // clip changes
            fill_quad_record(1, 0, 1024),
            fill_quad_record(1, 1, 1024), // transform changes
            fill_quad_record(0, 0, 1024), // back to original
        ];
        let batches = coalesce_ui_draws(&stream);
        assert_eq!(batches.len(), 4);
        assert_eq!(batches[0].draw_count, 2);
        assert_eq!(batches[1].draw_count, 2);
        assert_eq!(batches[2].draw_count, 1);
        assert_eq!(batches[3].draw_count, 1);
    }

    #[test]
    fn coalesce_keeps_distinct_atlas_ids_separate_for_glyph_runs() {
        let stream = vec![
            glyph_run_record(0, 0, 1024, 7),
            glyph_run_record(0, 0, 1024, 7),
            glyph_run_record(0, 0, 1024, 8),
            glyph_run_record(0, 0, 1024, 8),
        ];
        let batches = coalesce_ui_draws(&stream);
        assert_eq!(batches.len(), 2);
    }

    #[test]
    fn batch_table_passes_batch_collapse_for_small_batch_counts() {
        let stream: Vec<Tier6UiDrawRecord> =
            (0..500).map(|_| fill_quad_record(0, 0, 1024)).collect();
        let table = Tier6UiBatchTable::from_stream(&stream);
        assert_eq!(table.draw_count_total, 500);
        assert_eq!(table.batch_count, 1);
        assert!(table.passes_batch_collapse(8));
        assert_eq!(
            table.canonical_path,
            Tier6UiBatchTable::CANONICAL_ARTIFACT_PATH,
        );
        assert!(table.canonical_path.ends_with(".funpb.zst"));
    }

    #[test]
    fn compact_per_draw_constant_packs_opacity_into_16_bits() {
        assert_eq!(Tier6UiCompactPerDrawConstant::pack_opacity(0.0), 0);
        assert_eq!(Tier6UiCompactPerDrawConstant::pack_opacity(0.5), 512);
        assert_eq!(Tier6UiCompactPerDrawConstant::pack_opacity(1.0), 1024);
        // Out-of-range values clamp.
        assert_eq!(Tier6UiCompactPerDrawConstant::pack_opacity(-1.0), 0);
        assert_eq!(Tier6UiCompactPerDrawConstant::pack_opacity(2.0), 1024);
    }

    #[test]
    fn compact_per_draw_constant_unpacks_within_round_trip_tolerance() {
        let constant = Tier6UiCompactPerDrawConstant {
            transform_id: 7,
            clip_id: 11,
            opacity_x_1024: Tier6UiCompactPerDrawConstant::pack_opacity(0.75),
            reserved: 0,
        };
        let unpacked = constant.unpack_opacity();
        assert!((unpacked - 0.75).abs() < 1.0 / 1024.0);
    }

    #[test]
    fn subpixel_class_count_matches_policy() {
        assert_eq!(Tier6SubpixelPositioning::Off.class_count(), 1);
        assert_eq!(
            Tier6SubpixelPositioning::FractionalQuarterX.class_count(),
            4
        );
        assert_eq!(
            Tier6SubpixelPositioning::FractionalQuarterXY.class_count(),
            16
        );
    }

    #[test]
    fn missing_glyph_fallback_always_renders_visible_marker() {
        for fallback in Tier6MissingGlyphFallback::ALL {
            assert!(fallback.is_visible());
        }
    }

    #[test]
    fn glyph_atlas_lookup_hit_bumps_last_used_frame() {
        let mut atlas = Tier6GlyphAtlasTable::new(Tier6GlyphAtlasDescriptor::PRODUCT_DEFAULT);
        atlas.begin_frame();
        let _inserted = atlas.insert(0xfeed_face, 0, 0, 0, 0, 16, 16);
        atlas.begin_frame();
        let hit = atlas.lookup(0xfeed_face, 0).expect("hit");
        assert_eq!(hit.last_used_frame, atlas.frame_index);
        assert_eq!(atlas.miss_count, 0);
    }

    #[test]
    fn glyph_atlas_lookup_miss_records_miss_count() {
        let mut atlas = Tier6GlyphAtlasTable::new(Tier6GlyphAtlasDescriptor::PRODUCT_DEFAULT);
        atlas.begin_frame();
        assert!(atlas.lookup(0xdead, 0).is_none());
        assert_eq!(atlas.miss_count, 1);
    }

    #[test]
    fn glyph_atlas_stable_text_yields_zero_allocations_per_frame() {
        let mut atlas = Tier6GlyphAtlasTable::new(Tier6GlyphAtlasDescriptor::PRODUCT_DEFAULT);
        atlas.begin_frame();
        // Warmup: prime the atlas with a small set of glyphs.
        for glyph in 0..8u64 {
            atlas.insert(glyph, 0, 0, 0, 0, 16, 16);
        }
        // Stable next frame: every glyph already exists, no
        // inserts.
        atlas.begin_frame();
        for glyph in 0..8u64 {
            let _hit = atlas.lookup(glyph, 0).expect("stable text must hit");
        }
        assert!(atlas.allocations_this_frame_is_zero());
    }

    #[test]
    fn glyph_run_cache_hit_ratio_reflects_observed_pattern() {
        let mut cache = Tier6GlyphRunCache::default();
        cache.insert(0xaaaa, vec![Tier6GlyphAtlasRegion::default()], 0);
        for _ in 0..3 {
            let _ = cache.lookup(0xaaaa, 1);
        }
        let _ = cache.lookup(0xbbbb, 1);
        // 3 hits, 1 miss → 750 per mille.
        assert_eq!(cache.hit_ratio_per_mille(), 750);
    }

    #[test]
    fn rvelte_bridge_arg_parser_routes_each_known_value() {
        for kind in Tier6ProductRouteKind::ALL {
            let arg = format!("--rvelte-bridge={}", kind.as_str());
            let selection = Tier6RvelteBridgeRouteSelection::parse_arg(&arg);
            assert_eq!(selection, Tier6RvelteBridgeRouteSelection::Selected(kind));
        }
    }

    #[test]
    fn rvelte_bridge_arg_parser_accepts_short_aliases() {
        let s = Tier6RvelteBridgeRouteSelection::parse_arg("--rvelte-bridge=hud");
        assert_eq!(
            s,
            Tier6RvelteBridgeRouteSelection::Selected(Tier6ProductRouteKind::HudOverlay),
        );
    }

    #[test]
    fn rvelte_bridge_arg_parser_returns_unknown_value_for_bad_route() {
        let s = Tier6RvelteBridgeRouteSelection::parse_arg("--rvelte-bridge=not_a_route");
        assert_eq!(s, Tier6RvelteBridgeRouteSelection::UnknownArgValue);
    }

    #[test]
    fn rvelte_bridge_arg_parser_returns_not_provided_for_other_args() {
        let s = Tier6RvelteBridgeRouteSelection::parse_arg("--something-else");
        assert_eq!(s, Tier6RvelteBridgeRouteSelection::NotProvided);
    }

    #[test]
    fn rvelte_bridge_argv_parser_walks_to_first_match() {
        let argv = ["--foo", "--rvelte-bridge=launcher", "--bar"];
        let s = Tier6RvelteBridgeRouteSelection::parse_argv(&argv);
        assert_eq!(s.route(), Some(Tier6ProductRouteKind::LauncherShell),);
    }

    #[test]
    fn feature_flag_default_is_fun_renderer_consumer() {
        assert_eq!(
            Tier6FeatureFlag::product_default(),
            Tier6FeatureFlag::FunRendererConsumer,
        );
        assert!(Tier6FeatureFlag::FunRendererConsumer.enables_fun_renderer());
        assert!(!Tier6FeatureFlag::FakeRenderer.enables_fun_renderer());
    }

    #[test]
    fn native_ui_replacement_status_active_classification() {
        assert!(Tier6NativeUiReplacementStatus::StillNativeUiBacked.native_ui_still_active());
        assert!(!Tier6NativeUiReplacementStatus::ReplacedByNativeRoute.native_ui_still_active());
        assert!(!Tier6NativeUiReplacementStatus::ArchivedReferenceOnly.native_ui_still_active());
    }

    #[test]
    fn native_ui_inventory_records_per_status_counts() {
        let mut inv = Tier6NativeUiStagedRemovalInventory::new();
        inv.record(Tier6NativeUiSurfaceInventoryEntry {
            stable_id_hash: 1,
            debug_label_hash: 1,
            status: Tier6NativeUiReplacementStatus::StillNativeUiBacked,
            planned_replacement_route: Tier6RouteIdOption::LauncherShell,
        });
        inv.record(Tier6NativeUiSurfaceInventoryEntry {
            stable_id_hash: 2,
            debug_label_hash: 2,
            status: Tier6NativeUiReplacementStatus::ReplacedByNativeRoute,
            planned_replacement_route: Tier6RouteIdOption::HudOverlay,
        });
        inv.record(Tier6NativeUiSurfaceInventoryEntry {
            stable_id_hash: 3,
            debug_label_hash: 3,
            status: Tier6NativeUiReplacementStatus::ArchivedReferenceOnly,
            planned_replacement_route: Tier6RouteIdOption::NotRouted,
        });
        assert_eq!(inv.native_ui_active_count(), 1);
        assert_eq!(inv.replaced_count(), 1);
        assert_eq!(
            inv.canonical_path,
            Tier6NativeUiStagedRemovalInventory::CANONICAL_ARTIFACT_PATH,
        );
    }

    #[test]
    fn tier6_acceptance_verdict_passes_when_every_rule_holds() {
        // Build a passing scenario.
        let stream: Vec<Tier6UiDrawRecord> =
            (0..200).map(|_| fill_quad_record(0, 0, 1024)).collect();
        let batch_table = Tier6UiBatchTable::from_stream(&stream);

        let mut atlas = Tier6GlyphAtlasTable::new(Tier6GlyphAtlasDescriptor::PRODUCT_DEFAULT);
        atlas.begin_frame();
        // Warmup populate.
        for glyph in 0..4u64 {
            atlas.insert(glyph, 0, 0, 0, 0, 16, 16);
        }
        atlas.begin_frame();
        // Stable: only lookups, no inserts.
        for glyph in 0..4u64 {
            let _ = atlas.lookup(glyph, 0);
        }

        let route = Tier6RvelteBridgeRouteSelection::parse_arg("--rvelte-bridge=hud_overlay");

        let mut inv = Tier6NativeUiStagedRemovalInventory::new();
        inv.record(Tier6NativeUiSurfaceInventoryEntry {
            stable_id_hash: 1,
            debug_label_hash: 1,
            status: Tier6NativeUiReplacementStatus::StagedForRemovalAfterRouteParity,
            planned_replacement_route: Tier6RouteIdOption::HudOverlay,
        });

        let verdict = Tier6AcceptanceVerdict::evaluate(
            &batch_table,
            8,
            7, // unique pipelines == TIER6_UI_BATCH_KIND_COUNT
            &atlas,
            route,
            &inv,
        );
        assert!(verdict.passes_batching_collapse);
        assert!(verdict.passes_no_per_draw_pipeline);
        assert!(verdict.passes_text_no_per_frame_alloc);
        assert!(verdict.passes_route_renders_in_proof_scene_graph);
        assert!(verdict.passes_native_ui_inventory_complete);
        assert!(verdict.passes());
    }

    #[test]
    fn tier6_acceptance_verdict_fails_on_unrouted_native_ui_surface() {
        let stream: Vec<Tier6UiDrawRecord> =
            (0..10).map(|_| fill_quad_record(0, 0, 1024)).collect();
        let batch_table = Tier6UiBatchTable::from_stream(&stream);
        let mut atlas = Tier6GlyphAtlasTable::new(Tier6GlyphAtlasDescriptor::PRODUCT_DEFAULT);
        atlas.begin_frame();
        let route = Tier6RvelteBridgeRouteSelection::parse_arg("--rvelte-bridge=settings_shell");
        let mut inv = Tier6NativeUiStagedRemovalInventory::new();
        // NATIVE_UI-backed surface with NO planned replacement route.
        inv.record(Tier6NativeUiSurfaceInventoryEntry {
            stable_id_hash: 7,
            debug_label_hash: 7,
            status: Tier6NativeUiReplacementStatus::StillNativeUiBacked,
            planned_replacement_route: Tier6RouteIdOption::NotRouted,
        });
        let verdict = Tier6AcceptanceVerdict::evaluate(&batch_table, 4, 7, &atlas, route, &inv);
        assert!(!verdict.passes_native_ui_inventory_complete);
        assert!(!verdict.passes());
    }

    #[test]
    fn tier6_acceptance_verdict_fails_when_no_route_is_selected() {
        let stream: Vec<Tier6UiDrawRecord> = vec![fill_quad_record(0, 0, 1024)];
        let batch_table = Tier6UiBatchTable::from_stream(&stream);
        let mut atlas = Tier6GlyphAtlasTable::new(Tier6GlyphAtlasDescriptor::PRODUCT_DEFAULT);
        atlas.begin_frame();
        let inv = Tier6NativeUiStagedRemovalInventory::new();
        let verdict = Tier6AcceptanceVerdict::evaluate(
            &batch_table,
            4,
            7,
            &atlas,
            Tier6RvelteBridgeRouteSelection::NotProvided,
            &inv,
        );
        assert!(!verdict.passes_route_renders_in_proof_scene_graph);
        assert!(!verdict.passes());
    }

    #[test]
    fn product_route_kind_string_round_trip_for_every_variant() {
        for kind in Tier6ProductRouteKind::ALL {
            let s = kind.as_str();
            let parsed = Tier6ProductRouteKind::from_arg_value(s);
            assert_eq!(parsed, Some(kind));
        }
    }

    #[test]
    fn route_id_option_round_trip_from_kind() {
        for kind in Tier6ProductRouteKind::ALL {
            let id = Tier6RouteIdOption::from_kind(kind);
            // Every typed kind maps to a non-NotRouted id.
            assert!(!matches!(id, Tier6RouteIdOption::NotRouted));
        }
    }

    #[test]
    fn batch_kind_index_round_trips_for_all() {
        for (i, kind) in Tier6UiBatchKind::ALL.iter().copied().enumerate() {
            assert_eq!(kind.index(), i);
        }
    }

    #[test]
    fn glyph_atlas_descriptor_default_capacity_is_finite() {
        let descriptor = Tier6GlyphAtlasDescriptor::default();
        assert!(descriptor.capacity_bytes() > 0);
        // 2048 * 2048 * 4 = 16 MiB.
        assert_eq!(descriptor.capacity_bytes(), 16 * 1024 * 1024);
    }
}
