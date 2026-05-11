//! Pass 3 — typed diagnostics for the Lux graph compiler.
//!
//! `LuxGraphCompileReport` is the typed handle the bridge +
//! observability harnesses read after
//! [`crate::lux_graph::LuxGraphCompiler::compile_lux_plan`]
//! runs. It records per-frame typed counters (passes
//! compiled, resources declared, validation failures) and
//! per-failure typed records (`LuxGraphCompileFailure`).
//!
//! The typed `LuxGraphCompileFailure` enum names every
//! failure class the compiler can emit; the typed
//! [`crate::frame_graph::FrameGraphValidationFailureCode`]
//! variants (`LuxPassReadsUnwrittenResource`,
//! `LuxPassOrderViolation`,
//! `LuxPassDeclaresNoReadsOrWrites`) mirror these on the
//! frame-graph validation side.

use crate::frame_graph::{FrameGraphPassRole, FrameGraphResourceType};

pub const FUN_RENDERER_LUX_DIAGNOSTICS_SCHEMA_VERSION: u16 = 1;

/// Typed compile-time failure. Records the typed class of
/// the failure plus enough context that the audit harness
/// can pinpoint it without re-parsing the frame graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxGraphCompileFailure {
    /// The compiler tried to bind a typed resource that
    /// was not declared by any pass's typed outputs.
    UnwrittenResourceRead {
        role: FrameGraphPassRole,
        resource_type: FrameGraphResourceType,
    },
    /// The compiler placed a Lux pass out of order against
    /// its typed `lux_order_key`. (For Pass 3 the compiler
    /// pre-sorts requests, so this fires only if the
    /// caller injects passes via a private path.)
    PassOrderViolation {
        role: FrameGraphPassRole,
        expected_order_key: u16,
        actual_index: u32,
    },
    /// A Lux pass declared neither reads nor writes. The
    /// typed contract refuses empty Lux passes — the
    /// `lux_passes::declares_any_resource_interaction`
    /// predicate enforces this for every typed Lux role.
    PassDeclaresNoReadsOrWrites { role: FrameGraphPassRole },
    /// The compiler could not register the Lux pass because
    /// the typed frame graph handle space was exhausted.
    PassRegistrationFailed { role: FrameGraphPassRole },
    /// The compiler could not declare the Lux resource
    /// because the typed frame graph handle space was
    /// exhausted.
    ResourceDeclarationFailed {
        resource_type: FrameGraphResourceType,
    },
}

impl LuxGraphCompileFailure {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnwrittenResourceRead { .. } => "unwritten_resource_read",
            Self::PassOrderViolation { .. } => "pass_order_violation",
            Self::PassDeclaresNoReadsOrWrites { .. } => "pass_declares_no_reads_or_writes",
            Self::PassRegistrationFailed { .. } => "pass_registration_failed",
            Self::ResourceDeclarationFailed { .. } => "resource_declaration_failed",
        }
    }

    /// Typed predicate: is this failure a Pass 3 acceptance
    /// violation? Every variant is. Reserved for future
    /// non-acceptance failures.
    #[must_use]
    pub const fn is_acceptance_violation(self) -> bool {
        true
    }

    /// Pass V2.2 — typed translation into the typed
    /// [`crate::frame_graph::FrameGraphValidationFailureCode`].
    /// Returns `None` for failure kinds that the frame
    /// graph layer does not yet model (handle-space
    /// exhaustion is an allocator concern, not a graph
    /// validation concern).
    #[must_use]
    pub const fn frame_graph_validation_code(
        self,
    ) -> Option<crate::frame_graph::FrameGraphValidationFailureCode> {
        use crate::frame_graph::FrameGraphValidationFailureCode;
        match self {
            Self::UnwrittenResourceRead { .. } => {
                Some(FrameGraphValidationFailureCode::LuxPassReadsUnwrittenResource)
            }
            Self::PassOrderViolation { .. } => {
                Some(FrameGraphValidationFailureCode::LuxPassOrderViolation)
            }
            Self::PassDeclaresNoReadsOrWrites { .. } => {
                Some(FrameGraphValidationFailureCode::LuxPassDeclaresNoReadsOrWrites)
            }
            Self::PassRegistrationFailed { .. } | Self::ResourceDeclarationFailed { .. } => None,
        }
    }
}

/// Typed report returned by
/// [`crate::lux_graph::LuxGraphCompiler::compile_lux_plan`].
/// Carries typed counters + typed failure records.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LuxGraphCompileReport {
    pub schema_version: u16,
    /// Frame index the compiled plan targets.
    pub frame_index: u64,
    /// Typed count of `fun_lux::LuxPassRequest` records the
    /// compiler walked.
    pub lux_pass_requests_walked: u32,
    /// Typed count of `fun_lux::LuxResourceIntent` records
    /// the compiler walked.
    pub lux_resource_intents_walked: u32,
    /// Typed count of frame graph passes the compiler
    /// registered.
    pub frame_graph_passes_registered: u32,
    /// Typed count of frame graph resources the compiler
    /// declared.
    pub frame_graph_resources_declared: u32,
    /// Typed count of typed reads the compiler added.
    pub frame_graph_reads_added: u32,
    /// Typed count of typed writes the compiler added.
    pub frame_graph_writes_added: u32,
    /// Typed count of unwritten-read violations the
    /// compiler flagged.
    pub unwritten_read_violation_count: u32,
    /// Typed count of pass-order violations the compiler
    /// flagged.
    pub pass_order_violation_count: u32,
    /// Typed count of empty-pass violations the compiler
    /// flagged.
    pub empty_pass_violation_count: u32,
    /// Typed per-failure records.
    pub failures: Vec<LuxGraphCompileFailure>,
}

impl LuxGraphCompileReport {
    #[must_use]
    pub fn new(frame_index: u64) -> Self {
        Self {
            schema_version: FUN_RENDERER_LUX_DIAGNOSTICS_SCHEMA_VERSION,
            frame_index,
            lux_pass_requests_walked: 0,
            lux_resource_intents_walked: 0,
            frame_graph_passes_registered: 0,
            frame_graph_resources_declared: 0,
            frame_graph_reads_added: 0,
            frame_graph_writes_added: 0,
            unwritten_read_violation_count: 0,
            pass_order_violation_count: 0,
            empty_pass_violation_count: 0,
            failures: Vec::new(),
        }
    }

    /// Typed predicate: did the compile complete with no
    /// typed acceptance violations?
    #[must_use]
    pub fn compile_succeeded(&self) -> bool {
        self.failures.is_empty()
    }

    /// Typed predicate: did the compile register at least
    /// one typed frame graph pass per typed Lux pass
    /// request?
    #[must_use]
    pub fn every_request_compiled(&self) -> bool {
        self.frame_graph_passes_registered >= self.lux_pass_requests_walked
            && self.lux_pass_requests_walked > 0
    }

    /// Typed predicate: did every pass declare at least one
    /// read or write?
    #[must_use]
    pub fn every_pass_declares_reads_or_writes(&self) -> bool {
        self.empty_pass_violation_count == 0
    }

    /// Typed total of read + write declarations the
    /// compiler emitted.
    #[must_use]
    pub fn total_reads_writes(&self) -> u32 {
        self.frame_graph_reads_added + self.frame_graph_writes_added
    }

    pub fn record_failure(&mut self, failure: LuxGraphCompileFailure) {
        match failure {
            LuxGraphCompileFailure::UnwrittenResourceRead { .. } => {
                self.unwritten_read_violation_count =
                    self.unwritten_read_violation_count.saturating_add(1);
            }
            LuxGraphCompileFailure::PassOrderViolation { .. } => {
                self.pass_order_violation_count = self.pass_order_violation_count.saturating_add(1);
            }
            LuxGraphCompileFailure::PassDeclaresNoReadsOrWrites { .. } => {
                self.empty_pass_violation_count = self.empty_pass_violation_count.saturating_add(1);
            }
            _ => {}
        }
        self.failures.push(failure);
    }

    /// Pass V2.2 — typed Lux Graph debug-artifact section.
    /// Returns a multi-line `String` matching the user spec
    /// section layout, ready to append to the typed
    /// [`crate::frame_graph::RendererFrameGraphDebugArtifact::content`].
    ///
    /// `plan_scenes` is passed in by the caller (the bridge
    /// has the typed `LuxFramePlan` handy; the report
    /// doesn't track scene count separately).
    #[must_use]
    pub fn debug_section(&self, plan_scenes: usize) -> String {
        use core::fmt::Write as _;
        let mut content = String::new();
        let _ = writeln!(content, "Lux Graph");
        let _ = writeln!(content, "---------");
        let _ = writeln!(content, "plan_scenes: {}", plan_scenes);
        let _ = writeln!(content, "plan_passes: {}", self.lux_pass_requests_walked);
        let _ = writeln!(
            content,
            "plan_resources: {}",
            self.lux_resource_intents_walked,
        );
        let _ = writeln!(
            content,
            "compiled_lux_passes: {}",
            self.frame_graph_passes_registered,
        );
        let _ = writeln!(
            content,
            "compiled_lux_resources: {}",
            self.frame_graph_resources_declared,
        );
        let _ = writeln!(content, "failures: {}", self.failures.len());
        for failure in &self.failures {
            let _ = writeln!(content, "  {}", failure.as_str());
        }
        content
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_RENDERER_LUX_DIAGNOSTICS_SCHEMA_VERSION, 1);
    }

    #[test]
    fn new_report_has_zero_counters_and_succeeds() {
        let report = LuxGraphCompileReport::new(42);
        assert_eq!(report.frame_index, 42);
        assert_eq!(report.lux_pass_requests_walked, 0);
        assert_eq!(report.frame_graph_passes_registered, 0);
        assert!(report.compile_succeeded());
        // every_request_compiled returns false when zero
        // requests were walked.
        assert!(!report.every_request_compiled());
        assert!(report.every_pass_declares_reads_or_writes());
    }

    #[test]
    fn failure_kinds_have_distinct_strings() {
        let kinds = [
            LuxGraphCompileFailure::UnwrittenResourceRead {
                role: FrameGraphPassRole::LuxDirectLighting,
                resource_type: FrameGraphResourceType::LuxLightBuffer,
            },
            LuxGraphCompileFailure::PassOrderViolation {
                role: FrameGraphPassRole::LuxClusterLights,
                expected_order_key: 110,
                actual_index: 0,
            },
            LuxGraphCompileFailure::PassDeclaresNoReadsOrWrites {
                role: FrameGraphPassRole::LuxDebugOverlay,
            },
            LuxGraphCompileFailure::PassRegistrationFailed {
                role: FrameGraphPassRole::LuxDenoise,
            },
            LuxGraphCompileFailure::ResourceDeclarationFailed {
                resource_type: FrameGraphResourceType::LuxLightBuffer,
            },
        ];
        let mut seen = std::collections::HashSet::new();
        for k in kinds {
            assert!(seen.insert(k.as_str()), "duplicate: {}", k.as_str());
            assert!(k.is_acceptance_violation());
        }
    }

    #[test]
    fn record_failure_increments_typed_counters() {
        let mut report = LuxGraphCompileReport::new(0);
        report.record_failure(LuxGraphCompileFailure::UnwrittenResourceRead {
            role: FrameGraphPassRole::LuxDirectLighting,
            resource_type: FrameGraphResourceType::LuxLightBuffer,
        });
        report.record_failure(LuxGraphCompileFailure::PassOrderViolation {
            role: FrameGraphPassRole::LuxClusterLights,
            expected_order_key: 110,
            actual_index: 0,
        });
        report.record_failure(LuxGraphCompileFailure::PassDeclaresNoReadsOrWrites {
            role: FrameGraphPassRole::LuxDebugOverlay,
        });
        assert_eq!(report.unwritten_read_violation_count, 1);
        assert_eq!(report.pass_order_violation_count, 1);
        assert_eq!(report.empty_pass_violation_count, 1);
        assert!(!report.compile_succeeded());
        assert!(!report.every_pass_declares_reads_or_writes());
    }

    /// Pass V2.2 acceptance — `UnwrittenResourceRead`
    /// translates to the typed
    /// `LuxPassReadsUnwrittenResource` frame-graph
    /// validation code.
    #[test]
    fn lux_unwritten_resource_read_becomes_frame_graph_validation_failure() {
        use crate::frame_graph::FrameGraphValidationFailureCode;
        let failure = LuxGraphCompileFailure::UnwrittenResourceRead {
            role: FrameGraphPassRole::LuxDirectLighting,
            resource_type: FrameGraphResourceType::LuxLightBuffer,
        };
        assert_eq!(
            failure.frame_graph_validation_code(),
            Some(FrameGraphValidationFailureCode::LuxPassReadsUnwrittenResource),
        );
    }

    /// Pass V2.2 acceptance — `PassOrderViolation`
    /// translates to the typed `LuxPassOrderViolation`
    /// frame-graph validation code.
    #[test]
    fn lux_pass_order_violation_becomes_frame_graph_validation_failure() {
        use crate::frame_graph::FrameGraphValidationFailureCode;
        let failure = LuxGraphCompileFailure::PassOrderViolation {
            role: FrameGraphPassRole::LuxClusterLights,
            expected_order_key: 110,
            actual_index: 0,
        };
        assert_eq!(
            failure.frame_graph_validation_code(),
            Some(FrameGraphValidationFailureCode::LuxPassOrderViolation),
        );
    }

    /// Pass V2.2 acceptance — `PassDeclaresNoReadsOrWrites`
    /// translates to the typed
    /// `LuxPassDeclaresNoReadsOrWrites` frame-graph
    /// validation code.
    #[test]
    fn lux_empty_pass_becomes_frame_graph_validation_failure() {
        use crate::frame_graph::FrameGraphValidationFailureCode;
        let failure = LuxGraphCompileFailure::PassDeclaresNoReadsOrWrites {
            role: FrameGraphPassRole::LuxDebugOverlay,
        };
        assert_eq!(
            failure.frame_graph_validation_code(),
            Some(FrameGraphValidationFailureCode::LuxPassDeclaresNoReadsOrWrites),
        );
    }

    /// Pass V2.2 acceptance — handle-space exhaustion
    /// failures (`PassRegistrationFailed`,
    /// `ResourceDeclarationFailed`) intentionally have no
    /// frame-graph validation mapping — they are allocator
    /// concerns, not graph contract violations.  The
    /// `failure_kinds_have_distinct_strings` test confirms
    /// they remain typed acceptance violations on the
    /// compile report side.
    #[test]
    fn handle_space_exhaustion_failures_have_no_frame_graph_code() {
        assert_eq!(
            LuxGraphCompileFailure::PassRegistrationFailed {
                role: FrameGraphPassRole::LuxDenoise,
            }
            .frame_graph_validation_code(),
            None,
        );
        assert_eq!(
            LuxGraphCompileFailure::ResourceDeclarationFailed {
                resource_type: FrameGraphResourceType::LuxLightBuffer,
            }
            .frame_graph_validation_code(),
            None,
        );
    }

    /// Pass V2.2 acceptance — the typed debug section
    /// emits every user-spec field (plan_scenes,
    /// plan_passes, plan_resources, compiled_lux_passes,
    /// compiled_lux_resources, failures).
    #[test]
    fn debug_section_emits_typed_lux_graph_layout() {
        let mut report = LuxGraphCompileReport::new(7);
        report.lux_pass_requests_walked = 5;
        report.lux_resource_intents_walked = 4;
        report.frame_graph_passes_registered = 5;
        report.frame_graph_resources_declared = 4;
        report.record_failure(LuxGraphCompileFailure::UnwrittenResourceRead {
            role: FrameGraphPassRole::LuxDirectLighting,
            resource_type: FrameGraphResourceType::LuxLightBuffer,
        });
        let section = report.debug_section(2);
        assert!(section.contains("Lux Graph"));
        assert!(section.contains("plan_scenes: 2"));
        assert!(section.contains("plan_passes: 5"));
        assert!(section.contains("plan_resources: 4"));
        assert!(section.contains("compiled_lux_passes: 5"));
        assert!(section.contains("compiled_lux_resources: 4"));
        assert!(section.contains("failures: 1"));
        assert!(section.contains("unwritten_resource_read"));
    }
}
