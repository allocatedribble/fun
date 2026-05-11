//! Physical class + LOD policy for hosted populations.
//!
//! Pass 16 introduced cold/warm/active/ghost lifecycle tiers and the
//! 66-byte `ColdRecord`. Pass 17 layers a typed *physical class* on
//! top so the demotion / promotion / aggregate-eligibility decisions
//! are driven by a *policy table* rather than per-entity thresholds.
//!
//! Per the struct-shape rule:
//! - Per-entity state stays narrow: class, lifecycle tier, shard /
//!   cell, stable id. Wake radii, sleep thresholds, replication
//!   classes, aggregate eligibility, and max-active counts live on
//!   the [`PhysicalClassPolicy`] table indexed by [`PhysicalClass`].
//! - Aggregate variants are typed: [`AggregateKind`] enumerates the
//!   spec's three flavors (rubble field, crowd, background motion)
//!   instead of per-entity flag piles.
//! - Split / merge rules are pure functions over `(policy,
//!   AggregateContext)` so two runs over the same input produce the
//!   same decision.

use bevy::prelude::Resource;

use super::shard::BodyLifecycleTier;

/// Discrete physical archetype tag carried on each body. Drives the
/// LOD policy table.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum PhysicalClass {
    /// First-class authoritative gameplay actor; never demoted under
    /// budget pressure.
    Player,
    /// Vehicle; gameplay-critical for collision and routing.
    Vehicle,
    /// Projectile; short-lived, high cadence.
    Projectile,
    /// Piece of a destructible structure; can demote when settled.
    DestructibleChunk,
    /// Final settled rubble; first to demote and merge into
    /// aggregates under pressure.
    Rubble,
    /// Pickup item; can sleep when no relevance.
    Pickup,
    /// Movable scenery (doors, platforms, lifts).
    DoorPlatform,
    /// Sensor / trigger volume; never solver-active.
    SensorVolume,
    /// Background NPC or distant simulation actor.
    #[default]
    BackgroundActor,
    /// Aggregate row standing in for a clump of rubble or background
    /// actors.
    AggregateRubbleField,
}

impl PhysicalClass {
    /// Stable identifier used by diagnostics, telemetry, and CLI.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Player => "player",
            Self::Vehicle => "vehicle",
            Self::Projectile => "projectile",
            Self::DestructibleChunk => "destructible_chunk",
            Self::Rubble => "rubble",
            Self::Pickup => "pickup",
            Self::DoorPlatform => "door_platform",
            Self::SensorVolume => "sensor_volume",
            Self::BackgroundActor => "background_actor",
            Self::AggregateRubbleField => "aggregate_rubble_field",
        }
    }

    /// Stable integer code for telemetry counters.
    pub const fn code(self) -> u8 {
        match self {
            Self::Player => 0,
            Self::Vehicle => 1,
            Self::Projectile => 2,
            Self::DestructibleChunk => 3,
            Self::Rubble => 4,
            Self::Pickup => 5,
            Self::DoorPlatform => 6,
            Self::SensorVolume => 7,
            Self::BackgroundActor => 8,
            Self::AggregateRubbleField => 9,
        }
    }

    /// Number of distinct classes; used as the size of policy arrays.
    pub const COUNT: usize = 10;

    /// Iterator over every class in declaration order (stable).
    pub const ALL: [Self; Self::COUNT] = [
        Self::Player,
        Self::Vehicle,
        Self::Projectile,
        Self::DestructibleChunk,
        Self::Rubble,
        Self::Pickup,
        Self::DoorPlatform,
        Self::SensorVolume,
        Self::BackgroundActor,
        Self::AggregateRubbleField,
    ];

    fn index(self) -> usize {
        self.code() as usize
    }
}

/// How a class participates in the network replication pipeline. Used
/// by the dirty-row bridge and snapshot planner.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub enum ReplicationClass {
    /// Default: replicated to clients in the relevance set with the
    /// standard cadence.
    #[default]
    Default,
    /// Always replicated, even at low relevance scores.
    AlwaysRelevant,
    /// Replicated only at low cadence (background actors).
    LowFrequency,
    /// Not replicated; server-authoritative but invisible to clients.
    ServerOnly,
}

impl ReplicationClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::AlwaysRelevant => "always_relevant",
            Self::LowFrequency => "low_frequency",
            Self::ServerOnly => "server_only",
        }
    }
}

/// Policy for one physical class. Lives on the LOD registry, **not**
/// on the per-entity record. Per the struct-shape rule, per-entity
/// state is just `class`, lifecycle tier, shard / cell, and stable
/// id; everything else flows from this policy.
#[derive(Clone, Copy, Debug)]
pub struct PhysicalClassPolicy {
    /// Distance at which a body wakes or stays awake when an
    /// observer is present. Stored in mm to match cold-record units.
    pub wake_radius_mm: u32,
    /// Squared linear-velocity threshold (in `(cm/s)^2`) below which
    /// the body is considered "settled" and may demote.
    pub sleep_threshold_squared_cm_per_s: u32,
    /// Minimum number of ticks the body must spend at rest before
    /// it may demote.
    pub demotion_delay_ticks: u32,
    /// Replication class consumed by the snapshot planner.
    pub replication_class: ReplicationClass,
    /// Whether a body of this class may collapse into an aggregate
    /// (rubble field / crowd / background motion field).
    pub aggregate_eligible: bool,
    /// Maximum number of active bodies of this class allowed per
    /// shard. Once exceeded, surplus bodies queue for demotion if
    /// they are not gameplay-critical.
    pub max_active_per_shard: u32,
    /// `true` for classes that **never** demote under budget
    /// pressure (player, vehicle, projectile, etc.). Soft tiers like
    /// rubble or background actors carry `false`.
    pub gameplay_critical: bool,
}

impl PhysicalClassPolicy {
    /// Returns true when a body of this class is allowed to demote
    /// under budget pressure.
    pub const fn may_demote_under_pressure(self) -> bool {
        !self.gameplay_critical
    }
}

impl PhysicalClass {
    /// True when bodies of this class are gameplay-critical under the
    /// spec-default policy table — i.e., they must never be demoted
    /// in response to shard / budget pressure. Mirrors
    /// [`PhysicalClassPolicy::gameplay_critical`] for the default
    /// policy and is the predicate the V2-P5 shard runtime consults
    /// before scheduling demotion actions.
    pub const fn is_gameplay_critical(self) -> bool {
        self.default_policy().gameplay_critical
    }
}

impl PhysicalClass {
    /// Spec-default LOD policy. Tuned so that:
    /// - Players, vehicles, projectiles, and door/platforms are
    ///   gameplay-critical and can never demote under pressure.
    /// - Sensor volumes are never solver-active.
    /// - Rubble and background actors are aggregate-eligible.
    pub const fn default_policy(self) -> PhysicalClassPolicy {
        match self {
            Self::Player => PhysicalClassPolicy {
                wake_radius_mm: 200_000, // 200 m
                sleep_threshold_squared_cm_per_s: 0,
                demotion_delay_ticks: u32::MAX,
                replication_class: ReplicationClass::AlwaysRelevant,
                aggregate_eligible: false,
                max_active_per_shard: 256,
                gameplay_critical: true,
            },
            Self::Vehicle => PhysicalClassPolicy {
                wake_radius_mm: 150_000,
                sleep_threshold_squared_cm_per_s: 0,
                demotion_delay_ticks: u32::MAX,
                replication_class: ReplicationClass::AlwaysRelevant,
                aggregate_eligible: false,
                max_active_per_shard: 256,
                gameplay_critical: true,
            },
            Self::Projectile => PhysicalClassPolicy {
                wake_radius_mm: 50_000,
                sleep_threshold_squared_cm_per_s: 0,
                demotion_delay_ticks: u32::MAX,
                replication_class: ReplicationClass::Default,
                aggregate_eligible: false,
                max_active_per_shard: 1024,
                gameplay_critical: true,
            },
            Self::DestructibleChunk => PhysicalClassPolicy {
                wake_radius_mm: 80_000,
                sleep_threshold_squared_cm_per_s: 4, // 0.2 cm/s
                demotion_delay_ticks: 60,
                replication_class: ReplicationClass::Default,
                aggregate_eligible: true,
                max_active_per_shard: 1024,
                gameplay_critical: false,
            },
            Self::Rubble => PhysicalClassPolicy {
                wake_radius_mm: 30_000,
                sleep_threshold_squared_cm_per_s: 1,
                demotion_delay_ticks: 30,
                replication_class: ReplicationClass::LowFrequency,
                aggregate_eligible: true,
                max_active_per_shard: 4096,
                gameplay_critical: false,
            },
            Self::Pickup => PhysicalClassPolicy {
                wake_radius_mm: 60_000,
                sleep_threshold_squared_cm_per_s: 4,
                demotion_delay_ticks: 120,
                replication_class: ReplicationClass::Default,
                aggregate_eligible: false,
                max_active_per_shard: 512,
                gameplay_critical: false,
            },
            Self::DoorPlatform => PhysicalClassPolicy {
                wake_radius_mm: 100_000,
                sleep_threshold_squared_cm_per_s: 0,
                demotion_delay_ticks: u32::MAX,
                replication_class: ReplicationClass::AlwaysRelevant,
                aggregate_eligible: false,
                max_active_per_shard: 64,
                gameplay_critical: true,
            },
            Self::SensorVolume => PhysicalClassPolicy {
                wake_radius_mm: 0,
                sleep_threshold_squared_cm_per_s: 0,
                demotion_delay_ticks: u32::MAX,
                replication_class: ReplicationClass::ServerOnly,
                aggregate_eligible: false,
                max_active_per_shard: 0,
                gameplay_critical: true,
            },
            Self::BackgroundActor => PhysicalClassPolicy {
                wake_radius_mm: 100_000,
                sleep_threshold_squared_cm_per_s: 16,
                demotion_delay_ticks: 240,
                replication_class: ReplicationClass::LowFrequency,
                aggregate_eligible: true,
                max_active_per_shard: 2048,
                gameplay_critical: false,
            },
            Self::AggregateRubbleField => PhysicalClassPolicy {
                wake_radius_mm: 0,
                sleep_threshold_squared_cm_per_s: 0,
                demotion_delay_ticks: u32::MAX,
                replication_class: ReplicationClass::LowFrequency,
                aggregate_eligible: false,
                max_active_per_shard: 64,
                gameplay_critical: false,
            },
        }
    }
}

/// Concrete aggregate variant. Lives behind a single
/// [`PhysicalClass::AggregateRubbleField`] tier so the per-body row
/// stays narrow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AggregateKind {
    /// Settled debris field standing in for many [`Rubble`] bodies.
    RubbleAggregate {
        /// Members folded into this aggregate.
        member_count: u32,
        /// Bounding-sphere radius in mm.
        bounding_radius_mm: u32,
    },
    /// Background NPC crowd standing in for many
    /// [`BackgroundActor`]s.
    CrowdAggregate {
        member_count: u32,
        bounding_radius_mm: u32,
    },
    /// Distant motion field (water, foliage, dust) standing in for
    /// long-range ambient simulation.
    BackgroundMotionField {
        member_count: u32,
        bounding_radius_mm: u32,
    },
}

impl AggregateKind {
    pub const fn member_count(self) -> u32 {
        match self {
            Self::RubbleAggregate { member_count, .. }
            | Self::CrowdAggregate { member_count, .. }
            | Self::BackgroundMotionField { member_count, .. } => member_count,
        }
    }

    pub const fn bounding_radius_mm(self) -> u32 {
        match self {
            Self::RubbleAggregate {
                bounding_radius_mm, ..
            }
            | Self::CrowdAggregate {
                bounding_radius_mm, ..
            }
            | Self::BackgroundMotionField {
                bounding_radius_mm, ..
            } => bounding_radius_mm,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RubbleAggregate { .. } => "rubble_aggregate",
            Self::CrowdAggregate { .. } => "crowd_aggregate",
            Self::BackgroundMotionField { .. } => "background_motion_field",
        }
    }
}

/// Reason a split was triggered. Recorded so diagnostics can attribute
/// LOD churn to gameplay-relevant causes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SplitReason {
    /// An active collision disturbed the aggregate.
    ActiveCollisionDisturbance,
    /// A player observer entered relevance.
    PlayerEnteredRelevance,
    /// A scripted gameplay event requires per-body fidelity.
    GameplayEventFidelity,
}

impl SplitReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ActiveCollisionDisturbance => "active_collision_disturbance",
            Self::PlayerEnteredRelevance => "player_entered_relevance",
            Self::GameplayEventFidelity => "gameplay_event_fidelity",
        }
    }
}

/// Reason a merge was triggered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MergeReason {
    /// Active debris is now settled and can fold back into the
    /// aggregate.
    ActiveDebrisSettled,
    /// No observer relevance for an extended period.
    NoRelevance,
    /// Shard pressure forced demotion of low-priority bodies.
    ShardPressureHigh,
}

impl MergeReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ActiveDebrisSettled => "active_debris_settled",
            Self::NoRelevance => "no_relevance",
            Self::ShardPressureHigh => "shard_pressure_high",
        }
    }
}

/// Inputs the split / merge decision helpers consume. Consolidated
/// into one struct so the decision functions stay pure.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AggregateContext {
    /// True when an active collision is touching the aggregate this
    /// tick.
    pub active_collision_disturbance: bool,
    /// True when a player observer is inside the relevance radius.
    pub player_within_relevance: bool,
    /// True when a scripted event explicitly requested per-body
    /// fidelity (e.g. an objective triggered).
    pub gameplay_event_fidelity_requested: bool,
    /// True when the bodies in the aggregate area have been below
    /// their class's sleep threshold for at least `demotion_delay`
    /// ticks.
    pub debris_settled_for_required_ticks: bool,
    /// True when no observer has been within the relevance radius for
    /// the full demotion delay.
    pub relevance_drained_for_required_ticks: bool,
    /// True when the shard's active body count has exceeded its
    /// budget cap and demotion of eligible bodies is required.
    pub shard_pressure_high: bool,
}

/// Pure decision: should the aggregate split into per-body cold or
/// warm rows? Returns the [`SplitReason`] driving the decision.
pub fn should_split_aggregate(context: AggregateContext) -> Option<SplitReason> {
    if context.active_collision_disturbance {
        return Some(SplitReason::ActiveCollisionDisturbance);
    }
    if context.player_within_relevance {
        return Some(SplitReason::PlayerEnteredRelevance);
    }
    if context.gameplay_event_fidelity_requested {
        return Some(SplitReason::GameplayEventFidelity);
    }
    None
}

/// Pure decision: should bodies in this region merge into an
/// aggregate? Returns the [`MergeReason`] driving the decision. Only
/// classes whose policy is `aggregate_eligible` should be folded —
/// the caller is responsible for filtering, since the policy table
/// is consulted upstream.
pub fn should_merge_into_aggregate(context: AggregateContext) -> Option<MergeReason> {
    if context.shard_pressure_high {
        return Some(MergeReason::ShardPressureHigh);
    }
    if context.debris_settled_for_required_ticks && context.relevance_drained_for_required_ticks {
        return Some(MergeReason::NoRelevance);
    }
    if context.debris_settled_for_required_ticks {
        return Some(MergeReason::ActiveDebrisSettled);
    }
    None
}

/// Per-tick LOD diagnostics surface.
#[derive(Clone, Copy, Debug, Default)]
pub struct PhysicalLodDiagnostics {
    pub aggregate_splits_this_tick: u32,
    pub aggregate_merges_this_tick: u32,
    pub active_bodies_avoided_this_tick: u32,
    pub fidelity_drops_due_to_budget_pressure_this_tick: u32,
}

/// Resource holding the policy table plus the cumulative per-tick
/// diagnostics. Insert this resource and update its counters from
/// the systems that drive split / merge / demotion decisions.
#[derive(Resource, Clone, Debug)]
pub struct PhysicalLodRegistry {
    policies: [PhysicalClassPolicy; PhysicalClass::COUNT],
    diagnostics: PhysicalLodDiagnostics,
}

impl Default for PhysicalLodRegistry {
    fn default() -> Self {
        let mut policies = [PhysicalClass::default().default_policy(); PhysicalClass::COUNT];
        for class in PhysicalClass::ALL {
            policies[class.index()] = class.default_policy();
        }
        Self {
            policies,
            diagnostics: PhysicalLodDiagnostics::default(),
        }
    }
}

impl PhysicalLodRegistry {
    /// Read-only access to the policy for a class.
    pub fn policy(&self, class: PhysicalClass) -> PhysicalClassPolicy {
        self.policies[class.index()]
    }

    /// Mutate the policy for a class. Useful for runtime LOD tuning
    /// (e.g. cranking the rubble cap during stress events).
    pub fn set_policy(&mut self, class: PhysicalClass, policy: PhysicalClassPolicy) {
        self.policies[class.index()] = policy;
    }

    /// Read the current per-tick diagnostics snapshot.
    pub fn diagnostics(&self) -> PhysicalLodDiagnostics {
        self.diagnostics
    }

    /// Reset per-tick counters. Call once per simulation tick.
    pub fn begin_tick(&mut self) {
        self.diagnostics = PhysicalLodDiagnostics::default();
    }

    /// Record an aggregate split. The caller is responsible for the
    /// underlying state mutation; this just tallies the diagnostic
    /// counter and the spec's `active_bodies_avoided` accounting (a
    /// split *creates* per-body rows, so it consumes future budget
    /// rather than avoiding it).
    pub fn record_split(&mut self, _reason: SplitReason) {
        self.diagnostics.aggregate_splits_this_tick = self
            .diagnostics
            .aggregate_splits_this_tick
            .saturating_add(1);
    }

    /// Record an aggregate merge plus the number of active bodies it
    /// avoided. `bodies_avoided` should be the count of per-body
    /// rows the merge replaced — the spec's `active_bodies_avoided`
    /// metric.
    pub fn record_merge(&mut self, _reason: MergeReason, bodies_avoided: u32) {
        self.diagnostics.aggregate_merges_this_tick = self
            .diagnostics
            .aggregate_merges_this_tick
            .saturating_add(1);
        self.diagnostics.active_bodies_avoided_this_tick = self
            .diagnostics
            .active_bodies_avoided_this_tick
            .saturating_add(bodies_avoided);
    }

    /// Record a fidelity drop driven by shard / budget pressure (one
    /// soft-tier body demoted to make room for gameplay-critical
    /// promotions).
    pub fn record_fidelity_drop(&mut self) {
        self.diagnostics
            .fidelity_drops_due_to_budget_pressure_this_tick = self
            .diagnostics
            .fidelity_drops_due_to_budget_pressure_this_tick
            .saturating_add(1);
    }

    /// Decide whether `class` is allowed to be demoted under shard
    /// pressure. Mirrors the policy invariant `gameplay_critical` /
    /// `may_demote_under_pressure`.
    pub fn may_demote_under_pressure(&self, class: PhysicalClass) -> bool {
        self.policy(class).may_demote_under_pressure()
    }

    /// Apply budget pressure: starting from a candidate set of
    /// `(class, count)` rows in this shard, return the number of
    /// rows that *may* demote without violating the gameplay-
    /// critical invariant. Increments the fidelity-drop counter for
    /// each demoted row.
    pub fn apply_budget_pressure(&mut self, candidates: &[(PhysicalClass, u32)]) -> u32 {
        let mut demoted = 0_u32;
        for (class, count) in candidates {
            if self.may_demote_under_pressure(*class) {
                demoted = demoted.saturating_add(*count);
                for _ in 0..*count {
                    self.record_fidelity_drop();
                }
            }
        }
        demoted
    }
}

/// Lightweight per-entity LOD record. Keeps the per-entity surface
/// narrow per the struct-shape rule — class, lifecycle tier, shard /
/// cell, and stable id only. Wake / sleep / replication thresholds
/// flow from the policy table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PhysicalLodRecord {
    pub class: PhysicalClass,
    pub tier: BodyLifecycleTier,
    pub shard_id: u32,
    pub cell_x: i32,
    pub cell_y: i32,
    pub cell_z: i32,
    pub stable_id: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> AggregateContext {
        AggregateContext::default()
    }

    #[test]
    fn classes_have_unique_codes_and_strings() {
        let mut codes = hashbrown::HashSet::new();
        let mut names = hashbrown::HashSet::new();
        for class in PhysicalClass::ALL {
            assert!(codes.insert(class.code()), "duplicate code for {:?}", class);
            assert!(
                names.insert(class.as_str()),
                "duplicate name for {:?}",
                class
            );
        }
        assert_eq!(PhysicalClass::ALL.len(), PhysicalClass::COUNT);
    }

    #[test]
    fn gameplay_critical_classes_cannot_demote_under_pressure() {
        let registry = PhysicalLodRegistry::default();
        for critical in [
            PhysicalClass::Player,
            PhysicalClass::Vehicle,
            PhysicalClass::Projectile,
            PhysicalClass::DoorPlatform,
            PhysicalClass::SensorVolume,
        ] {
            assert!(
                !registry.may_demote_under_pressure(critical),
                "class {:?} must not be demotable under pressure",
                critical
            );
        }
        for soft in [
            PhysicalClass::DestructibleChunk,
            PhysicalClass::Rubble,
            PhysicalClass::Pickup,
            PhysicalClass::BackgroundActor,
            PhysicalClass::AggregateRubbleField,
        ] {
            assert!(
                registry.may_demote_under_pressure(soft),
                "class {:?} must be demotable under pressure",
                soft
            );
        }
    }

    #[test]
    fn budget_pressure_demotes_only_eligible_classes() {
        let mut registry = PhysicalLodRegistry::default();
        registry.begin_tick();
        let candidates = [
            (PhysicalClass::Player, 4),
            (PhysicalClass::Vehicle, 2),
            (PhysicalClass::Rubble, 32),
            (PhysicalClass::BackgroundActor, 16),
            (PhysicalClass::DoorPlatform, 1),
        ];
        let demoted = registry.apply_budget_pressure(&candidates);
        assert_eq!(
            demoted,
            32 + 16,
            "only the soft-tier candidates are eligible for demotion"
        );
        let diagnostics = registry.diagnostics();
        assert_eq!(
            diagnostics.fidelity_drops_due_to_budget_pressure_this_tick,
            48
        );
    }

    #[test]
    fn active_collision_disturbance_splits_aggregate() {
        let mut context = ctx();
        context.active_collision_disturbance = true;
        assert_eq!(
            should_split_aggregate(context),
            Some(SplitReason::ActiveCollisionDisturbance)
        );
    }

    #[test]
    fn player_relevance_splits_aggregate() {
        let mut context = ctx();
        context.player_within_relevance = true;
        assert_eq!(
            should_split_aggregate(context),
            Some(SplitReason::PlayerEnteredRelevance)
        );
    }

    #[test]
    fn gameplay_event_splits_aggregate_when_other_signals_off() {
        let mut context = ctx();
        context.gameplay_event_fidelity_requested = true;
        assert_eq!(
            should_split_aggregate(context),
            Some(SplitReason::GameplayEventFidelity)
        );
    }

    #[test]
    fn quiet_aggregate_does_not_split() {
        let context = ctx();
        assert_eq!(should_split_aggregate(context), None);
    }

    #[test]
    fn split_decision_is_deterministic_for_fixed_input() {
        // Run the decision four times against identical context;
        // all answers must match.
        let mut context = ctx();
        context.active_collision_disturbance = true;
        let answers: Vec<_> = (0..4).map(|_| should_split_aggregate(context)).collect();
        let first = answers[0];
        for value in &answers[1..] {
            assert_eq!(*value, first);
        }
    }

    #[test]
    fn settled_debris_merges_when_observer_drained() {
        let mut context = ctx();
        context.debris_settled_for_required_ticks = true;
        context.relevance_drained_for_required_ticks = true;
        assert_eq!(
            should_merge_into_aggregate(context),
            Some(MergeReason::NoRelevance)
        );
    }

    #[test]
    fn settled_debris_merges_back_into_aggregate() {
        let mut context = ctx();
        context.debris_settled_for_required_ticks = true;
        assert_eq!(
            should_merge_into_aggregate(context),
            Some(MergeReason::ActiveDebrisSettled)
        );
    }

    #[test]
    fn shard_pressure_forces_merge() {
        let mut context = ctx();
        context.shard_pressure_high = true;
        assert_eq!(
            should_merge_into_aggregate(context),
            Some(MergeReason::ShardPressureHigh)
        );
    }

    #[test]
    fn pieces_split_then_merge_back_round_trip() {
        // Active collision disturbs the aggregate -> split.
        let mut context = ctx();
        context.active_collision_disturbance = true;
        let split = should_split_aggregate(context);
        assert!(split.is_some());

        // Collision passes; debris settles; relevance drains.
        let mut quiet = ctx();
        quiet.debris_settled_for_required_ticks = true;
        quiet.relevance_drained_for_required_ticks = true;
        let merge = should_merge_into_aggregate(quiet);
        assert!(merge.is_some());
    }

    #[test]
    fn registry_tracks_split_merge_diagnostics() {
        let mut registry = PhysicalLodRegistry::default();
        registry.begin_tick();
        registry.record_split(SplitReason::PlayerEnteredRelevance);
        registry.record_merge(MergeReason::NoRelevance, 64);
        let diagnostics = registry.diagnostics();
        assert_eq!(diagnostics.aggregate_splits_this_tick, 1);
        assert_eq!(diagnostics.aggregate_merges_this_tick, 1);
        assert_eq!(diagnostics.active_bodies_avoided_this_tick, 64);
    }

    #[test]
    fn aggregate_kind_member_count_round_trips() {
        let kind = AggregateKind::RubbleAggregate {
            member_count: 1024,
            bounding_radius_mm: 5_000,
        };
        assert_eq!(kind.member_count(), 1024);
        assert_eq!(kind.bounding_radius_mm(), 5_000);
        assert_eq!(kind.as_str(), "rubble_aggregate");
    }

    #[test]
    fn registry_set_policy_overrides_default() {
        let mut registry = PhysicalLodRegistry::default();
        let mut updated = registry.policy(PhysicalClass::Rubble);
        updated.max_active_per_shard = 1;
        registry.set_policy(PhysicalClass::Rubble, updated);
        assert_eq!(
            registry.policy(PhysicalClass::Rubble).max_active_per_shard,
            1
        );
        // Other classes are untouched.
        assert!(registry.policy(PhysicalClass::Player).max_active_per_shard >= 256);
    }
}
