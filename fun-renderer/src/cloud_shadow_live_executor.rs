//! Pass C9.1 — typed cloud shadow live executor.
//!
//! Wires the typed cloud shadow chain
//! (`LuxCloudShadowProject` → `LuxCloudShadowFilter` →
//! `LuxCloudShadowRegisterLayer`) into a live, product-route
//! renderer path.  Bundles the typed Pass C7.5 wgpu
//! pipeline + dispatch helpers + the typed Pass C7.4.5
//! pass-recording + the typed Pass C7.4.6 aux-layer
//! registration into a single typed entry point the typed
//! renderer frame-graph executor calls per frame.
//!
//! Ownership: this module lives in `fun-renderer`, NOT
//! `fun_render`.  The typed executor is the typed canonical
//! product cloud-shadow execution path (per Pass C7.12
//! retirement contract).

#[cfg(feature = "wgpu_bridge")]
use crate::cloud_shadow_pipelines::{
    CloudShadowDispatchCounts, CloudShadowFilterPipeline, CloudShadowGpuResources,
    CloudShadowProjectPipeline, create_filter_bind_group, create_project_bind_group,
    record_dispatch_cloud_shadow_chain,
};

use crate::cloud_shadow::{
    CloudShadowFrameDelayMode, CloudShadowProjectionConstants, CloudShadowResourceDiagnostics,
};
use crate::cloud_shadow_passes::{
    CloudShadowGraphDiagnostics, CloudShadowPassRecord, record_cloud_shadow_pass_chain,
};
use crate::clouds::CloudRenderSettings;
use crate::frame_graph::FrameGraphPassRole;
use crate::lux_shadow_aux_layer::{
    LuxShadowAuxLayer, LuxShadowAuxLayerRegistry, LuxShadowAuxLayerRegistryError,
    register_cloud_shadow_aux_layer,
};
use fun_lux::LuxLightId;

pub const FUN_RENDERER_CLOUD_SHADOW_LIVE_EXECUTOR_SCHEMA_VERSION: u16 = 1;

// ============================================================================
// Section 1 — typed CloudShadowLiveInputs
// ============================================================================

/// Typed Pass C9.1 — typed inputs the typed renderer
/// supplies to the typed `CloudShadowLiveExecutor`.
/// Bundles the typed settings + projection constants +
/// frame-delay mode + the typed frame index.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CloudShadowLiveInputs {
    pub schema_version: u16,
    pub settings: CloudRenderSettings,
    pub constants: CloudShadowProjectionConstants,
    pub frame_delay_mode: CloudShadowFrameDelayMode,
    pub debug_readback_active: bool,
    pub frame_index: u32,
}

impl Default for CloudShadowLiveInputs {
    fn default() -> Self {
        Self {
            schema_version: FUN_RENDERER_CLOUD_SHADOW_LIVE_EXECUTOR_SCHEMA_VERSION,
            settings: CloudRenderSettings::PRODUCT_DEFAULT,
            constants: CloudShadowProjectionConstants::DISABLED,
            frame_delay_mode: CloudShadowFrameDelayMode::OneFrameDelayed,
            debug_readback_active: false,
            frame_index: 0,
        }
    }
}

impl CloudShadowLiveInputs {
    /// Typed product baseline (typed product settings +
    /// typed live projection constants assembled later via
    /// `with_constants`).
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_RENDERER_CLOUD_SHADOW_LIVE_EXECUTOR_SCHEMA_VERSION,
        settings: CloudRenderSettings::PRODUCT_DEFAULT,
        constants: CloudShadowProjectionConstants::DISABLED,
        frame_delay_mode: CloudShadowFrameDelayMode::OneFrameDelayed,
        debug_readback_active: false,
        frame_index: 0,
    };

    /// Typed builder: typed update the typed projection
    /// constants.
    #[must_use]
    pub const fn with_constants(mut self, constants: CloudShadowProjectionConstants) -> Self {
        self.constants = constants;
        self
    }

    /// Typed builder: typed update the typed frame index.
    #[must_use]
    pub const fn with_frame_index(mut self, frame_index: u32) -> Self {
        self.frame_index = frame_index;
        self
    }

    /// Typed builder: typed update the typed frame-delay
    /// mode.
    #[must_use]
    pub const fn with_frame_delay_mode(mut self, mode: CloudShadowFrameDelayMode) -> Self {
        self.frame_delay_mode = mode;
        self
    }

    /// Typed builder: typed update the typed debug
    /// readback flag.
    #[must_use]
    pub const fn with_debug_readback(mut self, active: bool) -> Self {
        self.debug_readback_active = active;
        self
    }

    /// Typed predicate: should the typed executor run the
    /// typed cloud shadow chain this frame?  Composes the
    /// typed settings gate + the typed projection-live
    /// gate.
    #[must_use]
    pub fn should_run_chain(&self) -> bool {
        self.settings.registers_world_shadow_pass()
            && self.constants.projects_world_shadow()
    }

    /// Typed light id the typed projection constants name.
    #[must_use]
    pub const fn light_id(&self) -> LuxLightId {
        self.constants.light_id
    }
}

// ============================================================================
// Section 2 — typed CloudShadowLiveReport
// ============================================================================

/// Typed Pass C9.1 — typed executor report.  Bundles the
/// typed dispatch counts + the typed pass records + the
/// typed aux-layer registration outcome + the typed
/// resource diagnostics.  Drives the typed
/// `CloudShadowDispatchCounts::full_chain_dispatched()`
/// audit + the typed runtime debug surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CloudShadowLiveReport {
    pub schema_version: u16,
    /// Typed pass-chain records (project / filter /
    /// register).  Even when the typed chain is gated
    /// off, the typed records still appear so the typed
    /// graph diagnostics can audit "pass was gated off
    /// this frame".
    pub chain: CloudShadowGraphDiagnostics,
    /// Typed resource diagnostics for the typed wgpu
    /// allocations.
    pub resources: CloudShadowResourceDiagnostics,
    /// Typed project dispatch count this frame (`0` or
    /// `1`).
    pub project_dispatches: u32,
    /// Typed filter dispatch count this frame.
    pub filter_dispatches: u32,
    /// Typed predicate: did the typed register pass
    /// successfully register the typed aux layer this
    /// frame?
    pub aux_layer_registered: bool,
    /// Typed light id the typed aux layer covers (or
    /// typed `INVALID` when typed gated off).
    pub aux_layer_light_id: LuxLightId,
    /// Typed predicate: did the typed executor short-
    /// circuit because the typed settings / projection
    /// gated the typed pass off?
    pub gated_off: bool,
    /// Typed registry error (typed `None` on typed
    /// success).
    pub aux_layer_error: Option<LuxShadowAuxLayerRegistryError>,
}

impl Default for CloudShadowLiveReport {
    fn default() -> Self {
        Self::COLD_DEFAULT
    }
}

impl CloudShadowLiveReport {
    /// Typed cold default — typed everything gated off /
    /// not dispatched.
    pub const COLD_DEFAULT: Self = Self {
        schema_version: FUN_RENDERER_CLOUD_SHADOW_LIVE_EXECUTOR_SCHEMA_VERSION,
        chain: CloudShadowGraphDiagnostics {
            schema_version: 0,
            project: cold_pass_record(FrameGraphPassRole::LuxCloudShadowProject),
            filter: cold_pass_record(FrameGraphPassRole::LuxCloudShadowFilter),
            register_layer: cold_pass_record(FrameGraphPassRole::LuxCloudShadowRegisterLayer),
            frame_delay_mode: CloudShadowFrameDelayMode::OneFrameDelayed,
        },
        resources: CloudShadowResourceDiagnostics::COLD_DEFAULT,
        project_dispatches: 0,
        filter_dispatches: 0,
        aux_layer_registered: false,
        aux_layer_light_id: LuxLightId::INVALID,
        gated_off: true,
        aux_layer_error: None,
    };

    /// Typed Pass C9.1 acceptance predicate — did the
    /// typed full chain dispatch this frame?  Mirrors the
    /// typed `CloudShadowDispatchCounts::full_chain_dispatched`
    /// shape but at the typed report level.
    #[must_use]
    pub const fn full_chain_dispatched(&self) -> bool {
        self.project_dispatches > 0 && self.filter_dispatches > 0 && self.aux_layer_registered
    }

    /// Typed predicate: do the typed graph diagnostics +
    /// the typed runtime report agree on pass counts?
    /// Audited per the typed user-spec acceptance bullet
    /// "Graph diagnostics and runtime diagnostics agree
    /// on pass counts."
    #[must_use]
    pub fn diagnostics_agree_on_pass_counts(&self) -> bool {
        // Typed chain always reports typed 3 records
        // (project / filter / register).  Typed dispatch
        // counts agree when the typed chain.full_chain_ran
        // matches the typed runtime path's dispatch
        // counts.
        let chain_ran = self.chain.full_chain_ran_this_frame();
        let runtime_ran = self.full_chain_dispatched();
        chain_ran == runtime_ran
    }

    /// Typed predicate: did the typed register pass
    /// produce a typed valid aux-layer registration?
    #[must_use]
    pub const fn register_pass_succeeded(&self) -> bool {
        self.aux_layer_registered && self.aux_layer_light_id.is_valid()
    }
}

const fn cold_pass_record(role: FrameGraphPassRole) -> CloudShadowPassRecord {
    CloudShadowPassRecord {
        schema_version: 0,
        role,
        order_key: 0,
        reads: &[],
        writes: &[],
        frame_delay_mode: CloudShadowFrameDelayMode::OneFrameDelayed,
        registers_pass: false,
        projection_is_live: false,
    }
}

// ============================================================================
// Section 3 — typed CloudShadowLiveExecutor (CPU contract)
// ============================================================================

/// Typed Pass C9.1 — typed live executor stub the typed
/// renderer instantiates per cloud-shadow-enabled scope.
/// The typed executor is a typed zero-sized marker; all
/// typed state lives in the typed wgpu pipelines + typed
/// resources + typed registry the typed renderer
/// pipeline manages outside this struct.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CloudShadowLiveExecutor;

impl CloudShadowLiveExecutor {
    /// Typed Pass C9.1 — typed CPU-side dry-run of the
    /// typed cloud shadow chain.  Builds the typed
    /// `CloudShadowGraphDiagnostics` (pass-chain records),
    /// the typed `CloudShadowResourceDiagnostics`
    /// (resource shape), and the typed aux-layer
    /// registration outcome.
    ///
    /// Does NOT touch typed wgpu — the typed live wgpu
    /// dispatch path is the typed `execute_cloud_shadow_chain`
    /// method (gated on `wgpu_bridge`).  This typed
    /// CPU dry-run is the typed `cargo test` audit path
    /// that proves the typed pass-chain wiring is wired
    /// without requiring a typed live GPU device.
    ///
    /// Typed `dispatch_simulated` controls whether the
    /// typed dry-run reports typed project/filter
    /// dispatch counts of `1` (typed simulated live run)
    /// or `0` (typed contract-only).  The typed live
    /// `execute_cloud_shadow_chain` always reports `1` on
    /// typed success.
    #[must_use]
    pub fn dry_run(
        &self,
        inputs: &CloudShadowLiveInputs,
        registry: &mut LuxShadowAuxLayerRegistry,
        dispatch_simulated: bool,
    ) -> CloudShadowLiveReport {
        if !inputs.should_run_chain() {
            // Typed gated off — return typed cold default
            // with typed chain records preserved so the
            // typed diagnostics still audit the typed
            // disabled state.
            let chain = CloudShadowGraphDiagnostics::from_inputs(
                &inputs.settings,
                &inputs.constants,
                inputs.frame_delay_mode,
            );
            return CloudShadowLiveReport {
                schema_version: FUN_RENDERER_CLOUD_SHADOW_LIVE_EXECUTOR_SCHEMA_VERSION,
                chain,
                resources: CloudShadowResourceDiagnostics::from_settings(
                    &inputs.settings,
                    inputs.debug_readback_active,
                    false,
                ),
                project_dispatches: 0,
                filter_dispatches: 0,
                aux_layer_registered: false,
                aux_layer_light_id: LuxLightId::INVALID,
                gated_off: true,
                aux_layer_error: None,
            };
        }

        let chain = CloudShadowGraphDiagnostics::from_inputs(
            &inputs.settings,
            &inputs.constants,
            inputs.frame_delay_mode,
        );
        let resources = CloudShadowResourceDiagnostics::from_settings(
            &inputs.settings,
            inputs.debug_readback_active,
            /* reallocated_this_frame */ true,
        );

        let (project_dispatches, filter_dispatches) = if dispatch_simulated {
            (1, 1)
        } else {
            (0, 0)
        };

        // Typed register the typed aux layer in the typed
        // registry.  Mirrors the typed C9.1
        // `execute_cloud_shadow_chain` flow.
        let aux_layer_result = register_cloud_shadow_aux_layer(
            &inputs.settings,
            &inputs.constants,
            inputs.frame_delay_mode,
            registry,
        );
        let (aux_layer_registered, aux_layer_light_id, aux_layer_error) = match aux_layer_result {
            Ok(Some(layer)) => (true, layer.light_id, None),
            Ok(None) => (false, LuxLightId::INVALID, None),
            Err(err) => (false, LuxLightId::INVALID, Some(err)),
        };

        CloudShadowLiveReport {
            schema_version: FUN_RENDERER_CLOUD_SHADOW_LIVE_EXECUTOR_SCHEMA_VERSION,
            chain,
            resources,
            project_dispatches,
            filter_dispatches,
            aux_layer_registered,
            aux_layer_light_id,
            gated_off: false,
            aux_layer_error,
        }
    }

    /// Typed Pass C9.1 — typed pass roles the typed
    /// executor binds.  Drives the typed user-spec
    /// "Bind it to frame-graph roles: LuxCloudShadowProject,
    /// LuxCloudShadowFilter, LuxCloudShadowRegisterLayer".
    #[must_use]
    pub const fn bound_frame_graph_roles() -> [FrameGraphPassRole; 3] {
        [
            FrameGraphPassRole::LuxCloudShadowProject,
            FrameGraphPassRole::LuxCloudShadowFilter,
            FrameGraphPassRole::LuxCloudShadowRegisterLayer,
        ]
    }

    /// Typed Pass C9.1 — typed predicate: does the typed
    /// executor bind every typed cloud-shadow frame-graph
    /// role?
    #[must_use]
    pub fn binds_all_cloud_shadow_roles() -> bool {
        let roles = Self::bound_frame_graph_roles();
        let chain_records =
            record_cloud_shadow_pass_chain(
                &CloudRenderSettings::PRODUCT_DEFAULT,
                &CloudShadowProjectionConstants::DISABLED,
                CloudShadowFrameDelayMode::OneFrameDelayed,
            );
        // Typed every typed bound role MUST appear in the
        // typed C7.4.5 pass chain.
        for role in roles {
            let found = chain_records.iter().any(|r| r.role == role);
            if !found {
                return false;
            }
        }
        // Typed every typed chain record's role MUST be
        // typed in the typed bound set.
        for record in chain_records {
            if !roles.contains(&record.role) {
                return false;
            }
        }
        true
    }
}

// ============================================================================
// Section 4 — typed wgpu live executor (gated on `wgpu_bridge`)
// ============================================================================

#[cfg(feature = "wgpu_bridge")]
impl CloudShadowLiveExecutor {
    /// Typed Pass C9.1 — typed live `execute_cloud_shadow_chain`
    /// entry point.  Performs the typed full chain:
    ///
    /// 1. Compose typed `CloudShadowProjectionConstantsGpu`
    ///    from the typed CPU constants + typed upload to
    ///    the typed `projection_constants_buffer`.
    /// 2. Build the typed project bind group + record the
    ///    typed `LuxCloudShadowProject` dispatch.
    /// 3. Build the typed filter bind group + record the
    ///    typed `LuxCloudShadowFilter` dispatch.
    /// 4. Register the typed `LuxShadowAuxLayer` in the
    ///    typed `LuxShadowAuxLayerRegistry` (typed
    ///    `LuxCloudShadowRegisterLayer` step).
    /// 5. Return the typed `CloudShadowLiveReport`.
    ///
    /// Caller-supplied resources:
    /// - `device` / `queue` — typed wgpu handles.
    /// - `encoder` — typed `wgpu::CommandEncoder` the
    ///   typed dispatch records into.
    /// - `project_pipeline` / `filter_pipeline` — typed
    ///   `Cloud_Shadow{Project,Filter}Pipeline`.  The
    ///   typed renderer creates these once at boot.
    /// - `resources` — typed
    ///   [`CloudShadowGpuResources`].  Typed allocated
    ///   once + reused across frames.
    /// - `cloud_params_buffer` — typed `wgpu::Buffer` the
    ///   typed cloud raymarch uploads its typed
    ///   `CloudParams` uniform into.  Supplied by the
    ///   typed cloud executor.
    /// - `weather_map_view` / `shape_noise_view` — typed
    ///   `wgpu::TextureView` handles from the typed cloud
    ///   raymarch's persistent texture set.
    /// - `registry` — typed
    ///   [`LuxShadowAuxLayerRegistry`] the typed register
    ///   pass writes to.
    /// - `inputs` — typed
    ///   [`CloudShadowLiveInputs`].
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn execute_cloud_shadow_chain(
        &self,
        device: &::wgpu::Device,
        queue: &::wgpu::Queue,
        encoder: &mut ::wgpu::CommandEncoder,
        project_pipeline: &CloudShadowProjectPipeline,
        filter_pipeline: &CloudShadowFilterPipeline,
        resources: &CloudShadowGpuResources,
        cloud_params_buffer: &::wgpu::Buffer,
        weather_map_view: &::wgpu::TextureView,
        shape_noise_view: &::wgpu::TextureView,
        registry: &mut LuxShadowAuxLayerRegistry,
        inputs: &CloudShadowLiveInputs,
    ) -> CloudShadowLiveReport {
        // Typed early-out for typed gated-off frames.
        if !inputs.should_run_chain() {
            return self.dry_run(inputs, registry, /* dispatch_simulated */ false);
        }

        // 1. Upload typed projection constants.
        resources.upload_projection_constants(queue, &inputs.constants);

        // 2-3. Build typed bind groups + record typed
        // project + filter dispatches via the typed
        // `record_dispatch_cloud_shadow_chain` helper
        // (Pass C7.5).
        let project_bind_group = create_project_bind_group(
            device,
            project_pipeline,
            resources,
            cloud_params_buffer,
            weather_map_view,
            shape_noise_view,
        );
        let filter_bind_group = create_filter_bind_group(device, filter_pipeline, resources);

        let dispatch_counts: CloudShadowDispatchCounts = record_dispatch_cloud_shadow_chain(
            encoder,
            project_pipeline,
            filter_pipeline,
            &project_bind_group,
            &filter_bind_group,
            resources.extent,
        );

        // 4. Register typed aux layer.
        let aux_layer_result = register_cloud_shadow_aux_layer(
            &inputs.settings,
            &inputs.constants,
            inputs.frame_delay_mode,
            registry,
        );
        let (aux_layer_registered, aux_layer_light_id, aux_layer_error) = match aux_layer_result {
            Ok(Some(layer)) => (true, layer.light_id, None),
            Ok(None) => (false, LuxLightId::INVALID, None),
            Err(err) => (false, LuxLightId::INVALID, Some(err)),
        };

        // 5. Build typed live report.
        let chain = CloudShadowGraphDiagnostics::from_inputs(
            &inputs.settings,
            &inputs.constants,
            inputs.frame_delay_mode,
        );
        let resource_diag = CloudShadowResourceDiagnostics::from_settings(
            &inputs.settings,
            inputs.debug_readback_active,
            /* reallocated_this_frame */ true,
        );

        CloudShadowLiveReport {
            schema_version: FUN_RENDERER_CLOUD_SHADOW_LIVE_EXECUTOR_SCHEMA_VERSION,
            chain,
            resources: resource_diag,
            project_dispatches: dispatch_counts.project_dispatches,
            filter_dispatches: dispatch_counts.filter_dispatches,
            aux_layer_registered,
            aux_layer_light_id,
            gated_off: false,
            aux_layer_error,
        }
    }
}

// ============================================================================
// Section 5 — typed integration helpers (no wgpu)
// ============================================================================

/// Typed Pass C9.1 — typed audit predicate: do the typed
/// dispatch counts agree with the typed pass-chain records?
#[must_use]
pub fn dispatch_counts_agree_with_chain(
    project_dispatches: u32,
    filter_dispatches: u32,
    chain: &CloudShadowGraphDiagnostics,
) -> bool {
    let chain_ran = chain.full_chain_ran_this_frame();
    let runtime_ran = project_dispatches > 0 && filter_dispatches > 0;
    chain_ran == runtime_ran
}

/// Typed Pass C9.1 — typed aux-layer-registered audit.
/// Returns `true` when the typed registry holds an aux
/// layer for the typed light id from the typed inputs.
#[must_use]
pub fn aux_layer_registered_for_inputs(
    registry: &LuxShadowAuxLayerRegistry,
    inputs: &CloudShadowLiveInputs,
) -> bool {
    let light_id = inputs.light_id();
    if !light_id.is_valid() {
        return false;
    }
    registry.find(light_id).is_some()
}

/// Typed Pass C9.1 — typed canonical layer-found helper:
/// extract the typed `LuxShadowAuxLayer` from the typed
/// registry for the typed inputs' light id.  Returns
/// `None` when the typed register pass did not produce a
/// typed layer this frame.
#[must_use]
pub fn lookup_registered_aux_layer<'a>(
    registry: &'a LuxShadowAuxLayerRegistry,
    inputs: &CloudShadowLiveInputs,
) -> Option<&'a LuxShadowAuxLayer> {
    registry.find(inputs.light_id())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clouds::{CloudQuality, CloudWeatherProfileId};

    fn live_inputs() -> CloudShadowLiveInputs {
        let constants = CloudShadowProjectionConstants::from_inputs(
            &CloudRenderSettings::PRODUCT_DEFAULT,
            CloudWeatherProfileId::Scattered,
            LuxLightId::new(42),
            [0.0, 1.0, 0.0],
            0,
        );
        CloudShadowLiveInputs::PRODUCT_DEFAULT.with_constants(constants)
    }

    /// Pass C9.1 schema is stable.
    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_RENDERER_CLOUD_SHADOW_LIVE_EXECUTOR_SCHEMA_VERSION, 1);
    }

    /// Pass C9.1 acceptance — the typed executor binds
    /// every typed cloud-shadow frame-graph role.
    #[test]
    fn executor_binds_every_cloud_shadow_role() {
        let roles = CloudShadowLiveExecutor::bound_frame_graph_roles();
        assert_eq!(roles.len(), 3);
        assert!(roles.contains(&FrameGraphPassRole::LuxCloudShadowProject));
        assert!(roles.contains(&FrameGraphPassRole::LuxCloudShadowFilter));
        assert!(roles.contains(&FrameGraphPassRole::LuxCloudShadowRegisterLayer));
        assert!(CloudShadowLiveExecutor::binds_all_cloud_shadow_roles());
    }

    /// Pass C9.1 acceptance — typed full chain dispatch
    /// reports correct counts in the typed live report.
    #[test]
    fn full_chain_dispatched_reports_true_on_live_run() {
        let executor = CloudShadowLiveExecutor;
        let inputs = live_inputs();
        let mut registry = LuxShadowAuxLayerRegistry::EMPTY;
        let report = executor.dry_run(&inputs, &mut registry, /* dispatch_simulated */ true);
        assert!(!report.gated_off);
        assert_eq!(report.project_dispatches, 1);
        assert_eq!(report.filter_dispatches, 1);
        assert!(report.aux_layer_registered);
        assert!(report.full_chain_dispatched());
        assert!(report.register_pass_succeeded());
        assert_eq!(report.aux_layer_light_id, LuxLightId::new(42));
    }

    /// Pass C9.1 acceptance — typed
    /// `LuxCloudShadowRegisterLayer` writes/registers an
    /// aux layer.  Verified via typed registry lookup
    /// after the typed dry-run.
    #[test]
    fn register_layer_writes_aux_layer() {
        let executor = CloudShadowLiveExecutor;
        let inputs = live_inputs();
        let mut registry = LuxShadowAuxLayerRegistry::EMPTY;
        let report = executor.dry_run(&inputs, &mut registry, true);
        assert!(report.aux_layer_registered);
        assert!(aux_layer_registered_for_inputs(&registry, &inputs));
        let layer = lookup_registered_aux_layer(&registry, &inputs)
            .expect("aux layer registered by executor");
        assert_eq!(layer.light_id, LuxLightId::new(42));
    }

    /// Pass C9.1 acceptance — graph diagnostics and
    /// runtime diagnostics agree on pass counts.
    #[test]
    fn graph_and_runtime_diagnostics_agree_on_pass_counts() {
        let executor = CloudShadowLiveExecutor;
        let inputs = live_inputs();
        let mut registry = LuxShadowAuxLayerRegistry::EMPTY;
        let live_report = executor.dry_run(&inputs, &mut registry, true);
        assert!(live_report.diagnostics_agree_on_pass_counts());
        assert!(dispatch_counts_agree_with_chain(
            live_report.project_dispatches,
            live_report.filter_dispatches,
            &live_report.chain,
        ));
        // Typed gated-off path also agrees (both report
        // typed not-ran).
        let mut off_inputs = inputs;
        off_inputs.settings = CloudRenderSettings::DISABLED;
        let mut empty_registry = LuxShadowAuxLayerRegistry::EMPTY;
        let off_report = executor.dry_run(&off_inputs, &mut empty_registry, true);
        assert!(off_report.gated_off);
        assert_eq!(off_report.project_dispatches, 0);
        assert_eq!(off_report.filter_dispatches, 0);
        assert!(off_report.diagnostics_agree_on_pass_counts());
    }

    /// Pass C9.1 — typed gated-off settings return a typed
    /// cold-default-shaped report.
    #[test]
    fn gated_off_returns_cold_shaped_report() {
        let executor = CloudShadowLiveExecutor;
        let inputs = CloudShadowLiveInputs::PRODUCT_DEFAULT; // typed DISABLED constants
        let mut registry = LuxShadowAuxLayerRegistry::EMPTY;
        let report = executor.dry_run(&inputs, &mut registry, true);
        assert!(report.gated_off);
        assert_eq!(report.project_dispatches, 0);
        assert_eq!(report.filter_dispatches, 0);
        assert!(!report.aux_layer_registered);
        assert!(!report.full_chain_dispatched());
        assert!(report.diagnostics_agree_on_pass_counts());
    }

    /// Pass C9.1 — typed builder methods compose.
    #[test]
    fn inputs_builder_methods_compose() {
        let constants = CloudShadowProjectionConstants::from_inputs(
            &CloudRenderSettings::PRODUCT_DEFAULT,
            CloudWeatherProfileId::StormFront,
            LuxLightId::new(7),
            [0.0, 1.0, 0.0],
            5,
        );
        let inputs = CloudShadowLiveInputs::PRODUCT_DEFAULT
            .with_constants(constants)
            .with_frame_index(5)
            .with_frame_delay_mode(CloudShadowFrameDelayMode::SameFrame)
            .with_debug_readback(true);
        assert_eq!(inputs.frame_index, 5);
        assert_eq!(inputs.frame_delay_mode, CloudShadowFrameDelayMode::SameFrame);
        assert!(inputs.debug_readback_active);
        assert_eq!(inputs.light_id(), LuxLightId::new(7));
        assert!(inputs.should_run_chain());
    }

    /// Pass C9.1 — typed `CloudQuality::Off` cascade-
    /// disables the typed chain.
    #[test]
    fn off_quality_cascade_disables_chain() {
        let executor = CloudShadowLiveExecutor;
        let mut settings = CloudRenderSettings::PRODUCT_DEFAULT;
        settings.quality = CloudQuality::Off;
        let constants = CloudShadowProjectionConstants::from_inputs(
            &settings,
            CloudWeatherProfileId::Scattered,
            LuxLightId::new(1),
            [0.0, 1.0, 0.0],
            0,
        );
        let inputs = CloudShadowLiveInputs {
            settings,
            constants,
            ..CloudShadowLiveInputs::PRODUCT_DEFAULT
        };
        let mut registry = LuxShadowAuxLayerRegistry::EMPTY;
        let report = executor.dry_run(&inputs, &mut registry, true);
        assert!(report.gated_off);
        assert!(!report.full_chain_dispatched());
    }

    /// Pass C9.1 — typed cold default report shape.
    #[test]
    fn cold_default_report_is_gated_off() {
        let report = CloudShadowLiveReport::COLD_DEFAULT;
        assert!(report.gated_off);
        assert!(!report.full_chain_dispatched());
        assert!(!report.register_pass_succeeded());
        assert_eq!(report.aux_layer_light_id, LuxLightId::INVALID);
        assert!(report.aux_layer_error.is_none());
    }
}
