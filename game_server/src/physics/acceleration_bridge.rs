//! Pass 20 secondary-owner work: thin adapter that lets a `fun` shard
//! consume avian's typed [`PhysicsAccelerationPolicy`] without
//! reaching into avian internals.
//!
//! Avian owns the typed policy surface
//! (`avian3d::schedule::PhysicsAccelerationPolicy` with one
//! [`PhysicsAccelerationMode`] per [`AccelerationKernel`]). This
//! module is the bridge that:
//!
//! - resolves a per-shard policy from a measurement tape using
//!   avian's pure decision function
//!   ([`resolve_acceleration_policy`]),
//! - reports a stable per-kernel diagnostics record so observer /
//!   bench dashboards can render the shard's acceleration footprint
//!   without dereferencing per-shard acceleration state directly.
//!
//! There is no per-body `use_simd` / `use_gpu` field on any fun-side
//! struct — the policy is shard-scoped, matching avian's
//! struct-shape rule.

use avian3d::schedule::{
    AccelerationKernel, KernelSpeedupMeasurement, PhysicsAccelerationMode,
    PhysicsAccelerationPolicy,
};

pub use avian3d::schedule::{
    AccelerationDecision, AccelerationDecisionPolicy, AccelerationOpComplexity,
    decide_acceleration, resolve_acceleration_policy,
};

/// Per-shard diagnostics record. Mirrors the
/// [`PhysicsAccelerationPolicy`] shape but stores the typed mode
/// codes so a renderer / dashboard can serialise the row without
/// pulling avian's reflection schema in.
///
/// This is the *view*; the policy itself remains the source of truth
/// inside avian's resource.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct ShardAccelerationDiagnostics {
    /// Mode code for the AABB update kernel.
    pub aabb_update_mode_code: u32,
    /// Mode code for the broad-phase candidate pair test.
    pub broad_candidate_pair_test_mode_code: u32,
    /// Mode code for the integration kernel.
    pub integration_mode_code: u32,
    /// Mode code for the constraint batch solver.
    pub constraint_batch_solve_mode_code: u32,
    /// Number of kernels currently on a non-`CpuPortable` mode.
    pub accelerated_kernel_count: u32,
}

impl ShardAccelerationDiagnostics {
    /// Build the diagnostics row from an avian policy.
    #[must_use]
    pub fn from_policy(policy: PhysicsAccelerationPolicy) -> Self {
        let accelerated_kernel_count = AccelerationKernel::all()
            .into_iter()
            .filter(|k| policy.mode_for(*k).is_accelerated())
            .count() as u32;
        Self {
            aabb_update_mode_code: policy.aabb_update.code(),
            broad_candidate_pair_test_mode_code: policy.broad_candidate_pair_test.code(),
            integration_mode_code: policy.integration.code(),
            constraint_batch_solve_mode_code: policy.constraint_batch_solve.code(),
            accelerated_kernel_count,
        }
    }

    /// Decode a single kernel's mode code back into the typed mode.
    /// Returns `None` if the underlying field carries an unknown
    /// code (which only happens when a future pass widens the enum
    /// without updating the bridge).
    #[must_use]
    pub fn mode_for(&self, kernel: AccelerationKernel) -> Option<PhysicsAccelerationMode> {
        let code = match kernel {
            AccelerationKernel::AabbUpdate => self.aabb_update_mode_code,
            AccelerationKernel::BroadCandidatePairTest => self.broad_candidate_pair_test_mode_code,
            AccelerationKernel::Integration => self.integration_mode_code,
            AccelerationKernel::ConstraintBatchSolve => self.constraint_batch_solve_mode_code,
        };
        PhysicsAccelerationMode::from_code(code)
    }
}

/// Build a per-shard [`PhysicsAccelerationPolicy`] from a measurement
/// tape. Pure wrapper around [`resolve_acceleration_policy`] that
/// keeps the call-site ergonomic for fun-side code that does not want
/// to import the full avian schedule prelude.
#[must_use]
pub fn build_shard_acceleration_policy(
    measurements: &[KernelSpeedupMeasurement],
    decision_policy: AccelerationDecisionPolicy,
) -> PhysicsAccelerationPolicy {
    resolve_acceleration_policy(measurements, decision_policy)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measurement(
        kernel: AccelerationKernel,
        mode: PhysicsAccelerationMode,
        portable_ns: u64,
        accelerated_ns: u64,
    ) -> KernelSpeedupMeasurement {
        KernelSpeedupMeasurement {
            kernel,
            mode,
            workload_size: 4_096,
            portable_cpu_time_ns: portable_ns,
            accelerated_time_ns: accelerated_ns,
            setup_overhead_ns: 0,
            operational_complexity: AccelerationOpComplexity::Simple,
        }
    }

    #[test]
    fn diagnostics_default_to_all_cpu_portable() {
        // Spec rule: defaults must be CpuPortable; the diagnostics
        // record reflects that with code 1 across the four kernels
        // and zero accelerated kernels.
        let diagnostics =
            ShardAccelerationDiagnostics::from_policy(PhysicsAccelerationPolicy::default());
        assert_eq!(diagnostics.aabb_update_mode_code, 1);
        assert_eq!(diagnostics.broad_candidate_pair_test_mode_code, 1);
        assert_eq!(diagnostics.integration_mode_code, 1);
        assert_eq!(diagnostics.constraint_batch_solve_mode_code, 1);
        assert_eq!(diagnostics.accelerated_kernel_count, 0);
    }

    #[test]
    fn diagnostics_count_accelerated_kernels() {
        let mut policy = PhysicsAccelerationPolicy::cpu_portable();
        policy.set_mode_for(
            AccelerationKernel::Integration,
            PhysicsAccelerationMode::CpuSimd,
        );
        policy.set_mode_for(
            AccelerationKernel::AabbUpdate,
            PhysicsAccelerationMode::GpuExperimental,
        );
        let diagnostics = ShardAccelerationDiagnostics::from_policy(policy);
        assert_eq!(diagnostics.accelerated_kernel_count, 2);
        assert_eq!(
            diagnostics.mode_for(AccelerationKernel::Integration),
            Some(PhysicsAccelerationMode::CpuSimd)
        );
        assert_eq!(
            diagnostics.mode_for(AccelerationKernel::AabbUpdate),
            Some(PhysicsAccelerationMode::GpuExperimental)
        );
        assert_eq!(
            diagnostics.mode_for(AccelerationKernel::BroadCandidatePairTest),
            Some(PhysicsAccelerationMode::CpuPortable)
        );
    }

    #[test]
    fn build_shard_policy_keeps_winning_kernels_only() {
        let measurements = [
            // Integration: 2.5× speedup → keeps.
            measurement(
                AccelerationKernel::Integration,
                PhysicsAccelerationMode::CpuSimd,
                10_000,
                4_000,
            ),
            // AabbUpdate: 1.1× speedup → rejects.
            measurement(
                AccelerationKernel::AabbUpdate,
                PhysicsAccelerationMode::CpuSimd,
                1_000,
                900,
            ),
        ];
        let policy = build_shard_acceleration_policy(
            &measurements,
            AccelerationDecisionPolicy::pass20_default(),
        );
        assert_eq!(
            policy.mode_for(AccelerationKernel::Integration),
            PhysicsAccelerationMode::CpuSimd
        );
        assert_eq!(
            policy.mode_for(AccelerationKernel::AabbUpdate),
            PhysicsAccelerationMode::CpuPortable
        );
        assert!(policy.has_acceleration());
    }

    #[test]
    fn empty_measurement_tape_yields_all_cpu_portable_diagnostics() {
        let policy =
            build_shard_acceleration_policy(&[], AccelerationDecisionPolicy::pass20_default());
        let diagnostics = ShardAccelerationDiagnostics::from_policy(policy);
        assert_eq!(diagnostics.accelerated_kernel_count, 0);
    }

    #[test]
    fn diagnostics_record_has_no_per_body_fields() {
        // Spec struct-shape rule: acceleration mode is shard/runtime
        // policy, never per-body. The diagnostics row mirrors the
        // policy shape — five fields, no Vec / HashMap. Pin the
        // exhaustive destructure so a future agent cannot add a
        // per-body field silently.
        let _check = |d: ShardAccelerationDiagnostics| {
            let ShardAccelerationDiagnostics {
                aabb_update_mode_code,
                broad_candidate_pair_test_mode_code,
                integration_mode_code,
                constraint_batch_solve_mode_code,
                accelerated_kernel_count,
            } = d;
            (
                aabb_update_mode_code,
                broad_candidate_pair_test_mode_code,
                integration_mode_code,
                constraint_batch_solve_mode_code,
                accelerated_kernel_count,
            )
        };
    }
}
