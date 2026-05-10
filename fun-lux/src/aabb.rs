//! Typed primitives for partial-update lighting state.
//!
//! `LuxAabb` is the typed world-space axis-aligned bounding
//! box used by Pass 2's typed `LuxDirtyRegion` + scene records
//! to record dirty volumes. `LuxTableRange` is the typed dense
//! `start..start+count` range used by `LuxLightDatabase` +
//! cluster grid + shadow request queue to record dirty slabs
//! in dense tables without re-uploading the whole table.
//!
//! Both types are renderer-neutral; `fun-renderer` consumes
//! them when it translates `LuxFramePlan` into real wgpu
//! resource updates.

pub const FUN_LUX_AABB_SCHEMA_VERSION: u16 = 1;

/// Typed world-space axis-aligned bounding box. Recorded as
/// `[min.x, min.y, min.z]` and `[max.x, max.y, max.z]`.
///
/// `LuxAabb` does **not** derive `Hash` or `Eq` because it
/// contains `f32` components; the typed equality compares
/// raw bit patterns via [`LuxAabb::bitwise_eq`] when needed.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct LuxAabb {
    pub schema_version: u16,
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl LuxAabb {
    /// Typed zero-volume AABB (origin, zero extent).
    pub const ZERO: Self = Self {
        schema_version: FUN_LUX_AABB_SCHEMA_VERSION,
        min: [0.0, 0.0, 0.0],
        max: [0.0, 0.0, 0.0],
    };

    /// Typed "whole world" AABB. Used as the typed sentinel
    /// for an unbounded dirty region; the planner treats this
    /// as "no spatial pruning is possible."
    pub const WHOLE_WORLD: Self = Self {
        schema_version: FUN_LUX_AABB_SCHEMA_VERSION,
        min: [f32::NEG_INFINITY; 3],
        max: [f32::INFINITY; 3],
    };

    #[must_use]
    pub const fn new(min: [f32; 3], max: [f32; 3]) -> Self {
        Self {
            schema_version: FUN_LUX_AABB_SCHEMA_VERSION,
            min,
            max,
        }
    }

    /// Typed centroid (midpoint of `min` and `max`).
    #[must_use]
    pub fn centroid(self) -> [f32; 3] {
        [
            (self.min[0] + self.max[0]) * 0.5,
            (self.min[1] + self.max[1]) * 0.5,
            (self.min[2] + self.max[2]) * 0.5,
        ]
    }

    /// Typed predicate: does this AABB contain a non-zero
    /// volume? (max > min on every axis.)
    #[must_use]
    pub fn is_non_empty(self) -> bool {
        self.max[0] > self.min[0] && self.max[1] > self.min[1] && self.max[2] > self.min[2]
    }

    /// Typed predicate: is this AABB the whole-world
    /// sentinel?
    #[must_use]
    pub fn is_whole_world(self) -> bool {
        self.min[0] == f32::NEG_INFINITY
            && self.min[1] == f32::NEG_INFINITY
            && self.min[2] == f32::NEG_INFINITY
            && self.max[0] == f32::INFINITY
            && self.max[1] == f32::INFINITY
            && self.max[2] == f32::INFINITY
    }

    /// Typed predicate: does this AABB contain the given
    /// world-space point?
    #[must_use]
    pub fn contains_point(self, p: [f32; 3]) -> bool {
        p[0] >= self.min[0]
            && p[1] >= self.min[1]
            && p[2] >= self.min[2]
            && p[0] <= self.max[0]
            && p[1] <= self.max[1]
            && p[2] <= self.max[2]
    }

    /// Typed predicate: do the two AABBs overlap (on every
    /// axis)?
    #[must_use]
    pub fn intersects(self, other: Self) -> bool {
        self.min[0] <= other.max[0]
            && self.max[0] >= other.min[0]
            && self.min[1] <= other.max[1]
            && self.max[1] >= other.min[1]
            && self.min[2] <= other.max[2]
            && self.max[2] >= other.min[2]
    }

    /// Typed union of two AABBs.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        Self {
            schema_version: FUN_LUX_AABB_SCHEMA_VERSION,
            min: [
                self.min[0].min(other.min[0]),
                self.min[1].min(other.min[1]),
                self.min[2].min(other.min[2]),
            ],
            max: [
                self.max[0].max(other.max[0]),
                self.max[1].max(other.max[1]),
                self.max[2].max(other.max[2]),
            ],
        }
    }

    /// Typed bit-by-bit equality (treats `NaN == NaN` as
    /// false; `+0.0 == -0.0` as false). Used by tests that
    /// compare typed AABB sentinels without
    /// floating-point-equality surprises.
    #[must_use]
    pub fn bitwise_eq(self, other: Self) -> bool {
        self.min
            .iter()
            .zip(other.min.iter())
            .all(|(a, b)| a.to_bits() == b.to_bits())
            && self
                .max
                .iter()
                .zip(other.max.iter())
                .all(|(a, b)| a.to_bits() == b.to_bits())
    }
}

/// Typed dense table range. Used by `LuxLightDatabase` and
/// `LuxClusterGrid` to record dirty slabs of contiguous
/// records that need re-upload, without re-uploading the
/// whole table.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxTableRange {
    pub schema_version: u16,
    pub start: u32,
    pub count: u32,
}

impl LuxTableRange {
    pub const EMPTY: Self = Self {
        schema_version: FUN_LUX_AABB_SCHEMA_VERSION,
        start: 0,
        count: 0,
    };

    #[must_use]
    pub const fn new(start: u32, count: u32) -> Self {
        Self {
            schema_version: FUN_LUX_AABB_SCHEMA_VERSION,
            start,
            count,
        }
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.count == 0
    }

    #[must_use]
    pub const fn end_exclusive(self) -> u32 {
        self.start.saturating_add(self.count)
    }

    /// Typed predicate: does this range contain the given
    /// dense-table index?
    #[must_use]
    pub const fn contains_index(self, index: u32) -> bool {
        index >= self.start && index < self.end_exclusive()
    }

    /// Typed union: smallest range that contains both `self`
    /// and `other`. Returns `EMPTY` when both inputs are
    /// empty.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        let start = self.start.min(other.start);
        let end = self.end_exclusive().max(other.end_exclusive());
        Self::new(start, end.saturating_sub(start))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_LUX_AABB_SCHEMA_VERSION, 1);
    }

    #[test]
    fn aabb_zero_default_is_empty() {
        assert!(!LuxAabb::ZERO.is_non_empty());
        assert!(!LuxAabb::ZERO.is_whole_world());
    }

    #[test]
    fn aabb_whole_world_predicate() {
        assert!(LuxAabb::WHOLE_WORLD.is_whole_world());
        assert!(LuxAabb::WHOLE_WORLD.contains_point([0.0, 0.0, 0.0]));
        assert!(LuxAabb::WHOLE_WORLD.contains_point([1e20, -1e20, 1e20]));
    }

    #[test]
    fn aabb_intersects_when_overlapping() {
        let a = LuxAabb::new([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
        let b = LuxAabb::new([5.0, 5.0, 5.0], [15.0, 15.0, 15.0]);
        assert!(a.intersects(b));
        let c = LuxAabb::new([20.0, 20.0, 20.0], [30.0, 30.0, 30.0]);
        assert!(!a.intersects(c));
    }

    #[test]
    fn aabb_union_covers_both() {
        let a = LuxAabb::new([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
        let b = LuxAabb::new([5.0, -5.0, 20.0], [15.0, 15.0, 30.0]);
        let u = a.union(b);
        assert_eq!(u.min, [0.0, -5.0, 0.0]);
        assert_eq!(u.max, [15.0, 15.0, 30.0]);
        // Both inputs are contained in the union.
        assert!(u.contains_point(a.centroid()));
        assert!(u.contains_point(b.centroid()));
    }

    #[test]
    fn aabb_centroid_is_midpoint() {
        let a = LuxAabb::new([-2.0, -4.0, -6.0], [2.0, 4.0, 6.0]);
        assert_eq!(a.centroid(), [0.0, 0.0, 0.0]);
    }

    #[test]
    fn aabb_bitwise_eq_treats_nan_as_unequal() {
        let nan = LuxAabb::new([f32::NAN; 3], [f32::NAN; 3]);
        // PartialEq treats NaN != NaN (Rust semantics);
        // bitwise_eq compares bit patterns. The same bits
        // are equal; different bit patterns of NaN are
        // unequal.
        assert!(nan.bitwise_eq(nan));
        let other_nan = LuxAabb::new(
            [f32::from_bits(0x7FC00001); 3], // different NaN bit pattern
            [f32::from_bits(0x7FC00001); 3],
        );
        assert!(!nan.bitwise_eq(other_nan));
    }

    #[test]
    fn table_range_empty_predicate() {
        assert!(LuxTableRange::EMPTY.is_empty());
        let r = LuxTableRange::new(4, 8);
        assert!(!r.is_empty());
        assert_eq!(r.end_exclusive(), 12);
    }

    #[test]
    fn table_range_contains_index() {
        let r = LuxTableRange::new(4, 8);
        assert!(!r.contains_index(3));
        assert!(r.contains_index(4));
        assert!(r.contains_index(11));
        assert!(!r.contains_index(12));
    }

    #[test]
    fn table_range_union_widens_to_smallest_cover() {
        let a = LuxTableRange::new(4, 4); // [4, 8)
        let b = LuxTableRange::new(10, 2); // [10, 12)
        let u = a.union(b);
        assert_eq!(u.start, 4);
        assert_eq!(u.end_exclusive(), 12);
        // Empty union with non-empty yields the non-empty.
        assert_eq!(a.union(LuxTableRange::EMPTY), a);
        assert_eq!(LuxTableRange::EMPTY.union(a), a);
    }
}
