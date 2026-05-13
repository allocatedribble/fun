use std::collections::BTreeMap;

use fun_ecs::{Resource, World};
use fun_window::{
    DpiScale, SurfaceId, SurfaceLifecycleEvent, SurfaceRequest, SurfaceSize, WindowId,
};

use crate::{
    pipeline::PipelineCache,
    resource::RendererResourceRegistry,
    schedule_contract::{RendererStaticPhaseProof, RendererWorkKind},
};

pub const FUN_RENDERER_INTEGRATION_SCHEMA_VERSION: u16 = 1;
pub const DEFAULT_RENDERER_SURFACE_EVENT_LIMIT: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct RendererIntegrationConfig {
    pub schema_version: u16,
    pub scheduler_visible_phases: bool,
    pub backend_handles_private: bool,
    pub bounded_surface_events: usize,
}

impl RendererIntegrationConfig {
    #[must_use]
    pub const fn render_phase_proof(self) -> RendererStaticPhaseProof {
        RendererStaticPhaseProof::PRODUCT
    }
}

impl Default for RendererIntegrationConfig {
    fn default() -> Self {
        Self {
            schema_version: FUN_RENDERER_INTEGRATION_SCHEMA_VERSION,
            scheduler_visible_phases: true,
            backend_handles_private: true,
            bounded_surface_events: DEFAULT_RENDERER_SURFACE_EVENT_LIMIT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Resource)]
pub struct FunRendererRenderWorld {
    pub schema_version: u16,
    pub table_count: u16,
    pub frame_generation: u64,
    pub backend_handles_private: bool,
}

impl FunRendererRenderWorld {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            schema_version: FUN_RENDERER_INTEGRATION_SCHEMA_VERSION,
            table_count: 0,
            frame_generation: 0,
            backend_handles_private: true,
        }
    }

    pub fn begin_frame(&mut self) {
        self.frame_generation = self.frame_generation.saturating_add(1);
    }

    pub fn register_table(&mut self) {
        self.table_count = self.table_count.saturating_add(1);
    }
}

impl Default for FunRendererRenderWorld {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct RendererPipelineCacheResource {
    pub cache: PipelineCache,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct RendererResourceRegistryResource {
    pub registry: RendererResourceRegistry,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct RendererFrameAdmissions {
    pub frame_generation: u64,
    pub admitted: Vec<RendererFrameAdmission>,
}

impl RendererFrameAdmissions {
    pub fn admit(&mut self, admission: RendererFrameAdmission) {
        self.frame_generation = self.frame_generation.saturating_add(1);
        self.admitted.push(admission);
    }
}

#[derive(Debug, Clone, PartialEq, Resource)]
pub struct RendererSurfaceRegistry {
    surfaces: BTreeMap<SurfaceId, RendererSurfaceRecord>,
    diagnostics: RendererSurfaceDiagnostics,
    event_limit: usize,
}

impl RendererSurfaceRegistry {
    #[must_use]
    pub fn new(event_limit: usize) -> Self {
        Self {
            surfaces: BTreeMap::new(),
            diagnostics: RendererSurfaceDiagnostics::default(),
            event_limit: event_limit.max(1),
        }
    }

    #[must_use]
    pub fn surface(&self, surface: SurfaceId) -> Option<&RendererSurfaceRecord> {
        self.surfaces.get(&surface)
    }

    #[must_use]
    pub fn diagnostics(&self) -> RendererSurfaceDiagnostics {
        self.diagnostics
    }

    #[must_use]
    pub fn surface_count(&self) -> usize {
        self.surfaces.len()
    }

    pub fn mark_configured(&mut self, surface: SurfaceId) -> Result<(), RendererSurfaceError> {
        let Some(record) = self.surfaces.get_mut(&surface) else {
            return Err(RendererSurfaceError::missing_surface(surface));
        };
        record.dirty = false;
        record.state = if record.occluded {
            RendererSurfaceState::Occluded
        } else {
            RendererSurfaceState::Ready
        };
        Ok(())
    }

    pub fn mark_surface_lost(&mut self, surface: SurfaceId) -> RendererSurfaceError {
        if let Some(record) = self.surfaces.get_mut(&surface) {
            record.dirty = true;
            record.state = RendererSurfaceState::Lost;
            RendererSurfaceError::surface_lost(surface, Some(record.window))
        } else {
            RendererSurfaceError::missing_surface(surface)
        }
    }

    pub fn apply_lifecycle_event(
        &mut self,
        event: SurfaceLifecycleEvent,
    ) -> RendererSurfaceActionBatch {
        self.diagnostics.observed_lifecycle_events =
            self.diagnostics.observed_lifecycle_events.saturating_add(1);
        let mut batch = RendererSurfaceActionBatch::default();
        if self.diagnostics.observed_lifecycle_events as usize > self.event_limit {
            self.diagnostics.event_overflow_count =
                self.diagnostics.event_overflow_count.saturating_add(1);
            batch.errors.push(RendererSurfaceError::queue_overflow());
            return batch;
        }

        match event {
            SurfaceLifecycleEvent::WindowCreated {
                surface,
                window,
                size,
                scale,
            } => {
                let record = RendererSurfaceRecord::new(surface, window, size, scale);
                let request = record.request();
                self.surfaces.insert(surface, record);
                self.diagnostics.created_surfaces =
                    self.diagnostics.created_surfaces.saturating_add(1);
                batch
                    .actions
                    .push(RendererSurfaceAction::CreateSurface { request });
            }
            SurfaceLifecycleEvent::HandlesAvailable { surface, window } => {
                if let Some(record) = self.surfaces.get_mut(&surface) {
                    record.window = window;
                    record.handles_available = true;
                    if record.state == RendererSurfaceState::AwaitingHandles {
                        record.state = RendererSurfaceState::Dirty;
                    }
                    batch
                        .actions
                        .push(RendererSurfaceAction::HandlesAvailable { surface, window });
                    if record.dirty {
                        batch
                            .actions
                            .push(RendererSurfaceAction::ReconfigureSurface {
                                request: record.request(),
                                reason: RendererSurfaceReconfigureReason::HandlesAvailable,
                            });
                    }
                } else {
                    batch
                        .errors
                        .push(RendererSurfaceError::missing_surface(surface));
                }
            }
            SurfaceLifecycleEvent::Resized {
                surface,
                window,
                size,
            } => {
                if let Some(record) = self.surfaces.get_mut(&surface) {
                    record.window = window;
                    record.size = size;
                    record.dirty = true;
                    record.state = RendererSurfaceState::Dirty;
                    record.generation = record.generation.saturating_add(1);
                    self.diagnostics.reconfigure_requests =
                        self.diagnostics.reconfigure_requests.saturating_add(1);
                    batch
                        .actions
                        .push(RendererSurfaceAction::ReconfigureSurface {
                            request: record.request(),
                            reason: RendererSurfaceReconfigureReason::Resized,
                        });
                } else {
                    batch
                        .errors
                        .push(RendererSurfaceError::missing_surface(surface));
                }
            }
            SurfaceLifecycleEvent::ScaleFactorChanged {
                surface,
                window,
                scale,
            } => {
                if let Some(record) = self.surfaces.get_mut(&surface) {
                    record.window = window;
                    record.scale = scale;
                    record.dirty = true;
                    record.state = RendererSurfaceState::Dirty;
                    record.generation = record.generation.saturating_add(1);
                    self.diagnostics.reconfigure_requests =
                        self.diagnostics.reconfigure_requests.saturating_add(1);
                    batch
                        .actions
                        .push(RendererSurfaceAction::ReconfigureSurface {
                            request: record.request(),
                            reason: RendererSurfaceReconfigureReason::ScaleFactorChanged,
                        });
                } else {
                    batch
                        .errors
                        .push(RendererSurfaceError::missing_surface(surface));
                }
            }
            SurfaceLifecycleEvent::FocusChanged {
                surface, focused, ..
            } => {
                if let Some(record) = self.surfaces.get_mut(&surface) {
                    record.focused = focused;
                } else {
                    batch
                        .errors
                        .push(RendererSurfaceError::missing_surface(surface));
                }
            }
            SurfaceLifecycleEvent::OcclusionChanged {
                surface, occluded, ..
            } => {
                if let Some(record) = self.surfaces.get_mut(&surface) {
                    record.occluded = occluded;
                    record.state = if occluded {
                        RendererSurfaceState::Occluded
                    } else if record.dirty {
                        RendererSurfaceState::Dirty
                    } else {
                        RendererSurfaceState::Ready
                    };
                } else {
                    batch
                        .errors
                        .push(RendererSurfaceError::missing_surface(surface));
                }
            }
            SurfaceLifecycleEvent::RedrawRequested { surface, window } => {
                if let Some(record) = self.surfaces.get_mut(&surface) {
                    record.window = window;
                    if record.occluded {
                        return batch;
                    }
                    let reconfigure_before_present = record.dirty;
                    if reconfigure_before_present {
                        batch
                            .actions
                            .push(RendererSurfaceAction::ReconfigureSurface {
                                request: record.request(),
                                reason: RendererSurfaceReconfigureReason::BeforePresent,
                            });
                    }
                    batch
                        .actions
                        .push(RendererSurfaceAction::AdmitFrame(RendererFrameAdmission {
                            surface,
                            window,
                            size: record.size,
                            reconfigure_before_present,
                            work: RendererFrameWorkPlan::for_surface_present(
                                reconfigure_before_present,
                            ),
                        }));
                    self.diagnostics.frame_admissions =
                        self.diagnostics.frame_admissions.saturating_add(1);
                } else {
                    batch
                        .errors
                        .push(RendererSurfaceError::missing_surface(surface));
                }
            }
            SurfaceLifecycleEvent::Destroyed { surface, window } => {
                self.surfaces.remove(&surface);
                self.diagnostics.destroyed_surfaces =
                    self.diagnostics.destroyed_surfaces.saturating_add(1);
                batch
                    .actions
                    .push(RendererSurfaceAction::ReleaseSurface { surface, window });
            }
        }
        batch
    }
}

impl Default for RendererSurfaceRegistry {
    fn default() -> Self {
        Self::new(DEFAULT_RENDERER_SURFACE_EVENT_LIMIT)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RendererSurfaceRecord {
    pub surface: SurfaceId,
    pub window: WindowId,
    pub size: SurfaceSize,
    pub scale: DpiScale,
    pub handles_available: bool,
    pub dirty: bool,
    pub focused: bool,
    pub occluded: bool,
    pub generation: u64,
    pub state: RendererSurfaceState,
}

impl RendererSurfaceRecord {
    #[must_use]
    pub const fn new(
        surface: SurfaceId,
        window: WindowId,
        size: SurfaceSize,
        scale: DpiScale,
    ) -> Self {
        Self {
            surface,
            window,
            size,
            scale,
            handles_available: false,
            dirty: true,
            focused: true,
            occluded: false,
            generation: 1,
            state: RendererSurfaceState::AwaitingHandles,
        }
    }

    #[must_use]
    pub const fn request(self) -> SurfaceRequest {
        SurfaceRequest::new(self.window, self.size).with_surface(self.surface)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererSurfaceState {
    AwaitingHandles,
    Dirty,
    Ready,
    Occluded,
    Released,
    Lost,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RendererSurfaceDiagnostics {
    pub observed_lifecycle_events: u64,
    pub event_overflow_count: u64,
    pub created_surfaces: u64,
    pub destroyed_surfaces: u64,
    pub reconfigure_requests: u64,
    pub frame_admissions: u64,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RendererSurfaceActionBatch {
    pub actions: Vec<RendererSurfaceAction>,
    pub errors: Vec<RendererSurfaceError>,
}

impl RendererSurfaceActionBatch {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty() && self.errors.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RendererSurfaceAction {
    CreateSurface {
        request: SurfaceRequest,
    },
    HandlesAvailable {
        surface: SurfaceId,
        window: WindowId,
    },
    ReconfigureSurface {
        request: SurfaceRequest,
        reason: RendererSurfaceReconfigureReason,
    },
    AdmitFrame(RendererFrameAdmission),
    ReleaseSurface {
        surface: SurfaceId,
        window: WindowId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererSurfaceReconfigureReason {
    HandlesAvailable,
    Resized,
    ScaleFactorChanged,
    BeforePresent,
    SurfaceLost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererFrameAdmission {
    pub surface: SurfaceId,
    pub window: WindowId,
    pub size: SurfaceSize,
    pub reconfigure_before_present: bool,
    pub work: RendererFrameWorkPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererFrameWorkPlan {
    pub work: Vec<RendererWorkKind>,
}

impl RendererFrameWorkPlan {
    #[must_use]
    pub fn for_surface_present(reconfigure_before_present: bool) -> Self {
        let mut work = Vec::with_capacity(14);
        work.extend_from_slice(&[
            RendererWorkKind::ExtractViews,
            RendererWorkKind::ExtractRenderables,
            RendererWorkKind::ExtractLights,
            RendererWorkKind::ExtractUiSurfaces,
            RendererWorkKind::ImportEcsRenderArtifacts,
            RendererWorkKind::PrepareGpuSceneChunk,
            RendererWorkKind::FrameGraphBuild,
        ]);
        if reconfigure_before_present {
            work.push(RendererWorkKind::FrameGraphCompile);
        }
        work.extend_from_slice(&[
            RendererWorkKind::RecordPass,
            RendererWorkKind::SubmitQueue,
            RendererWorkKind::PresentSurface,
            RendererWorkKind::DiagnosticCapture,
        ]);
        Self { work }
    }

    #[must_use]
    pub fn contains(&self, kind: RendererWorkKind) -> bool {
        self.work.contains(&kind)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererSurfaceError {
    pub surface: Option<SurfaceId>,
    pub window: Option<WindowId>,
    pub kind: RendererSurfaceErrorKind,
    pub stage: RendererSurfaceStage,
    pub recoverable: bool,
}

impl RendererSurfaceError {
    #[must_use]
    pub const fn missing_surface(surface: SurfaceId) -> Self {
        Self {
            surface: Some(surface),
            window: None,
            kind: RendererSurfaceErrorKind::MissingSurface,
            stage: RendererSurfaceStage::Lifecycle,
            recoverable: false,
        }
    }

    #[must_use]
    pub const fn queue_overflow() -> Self {
        Self {
            surface: None,
            window: None,
            kind: RendererSurfaceErrorKind::EventQueueOverflow,
            stage: RendererSurfaceStage::Lifecycle,
            recoverable: true,
        }
    }

    #[must_use]
    pub const fn surface_lost(surface: SurfaceId, window: Option<WindowId>) -> Self {
        Self {
            surface: Some(surface),
            window,
            kind: RendererSurfaceErrorKind::SurfaceLost,
            stage: RendererSurfaceStage::Present,
            recoverable: true,
        }
    }

    #[must_use]
    pub const fn device_lost(surface: Option<SurfaceId>, window: Option<WindowId>) -> Self {
        Self {
            surface,
            window,
            kind: RendererSurfaceErrorKind::DeviceLost,
            stage: RendererSurfaceStage::Submit,
            recoverable: false,
        }
    }

    #[must_use]
    pub const fn configure_failed(surface: SurfaceId, window: Option<WindowId>) -> Self {
        Self {
            surface: Some(surface),
            window,
            kind: RendererSurfaceErrorKind::ConfigureFailed,
            stage: RendererSurfaceStage::Configure,
            recoverable: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererSurfaceErrorKind {
    MissingSurface,
    MissingHandles,
    InvalidSize,
    ConfigureFailed,
    SurfaceLost,
    DeviceLost,
    PresentFailed,
    EventQueueOverflow,
}

#[cfg(feature = "wgpu_bridge")]
impl From<crate::bridge::wgpu::surface::WgpuSurfaceAcquireFailure> for RendererSurfaceErrorKind {
    fn from(value: crate::bridge::wgpu::surface::WgpuSurfaceAcquireFailure) -> Self {
        match value {
            crate::bridge::wgpu::surface::WgpuSurfaceAcquireFailure::Lost => Self::SurfaceLost,
            crate::bridge::wgpu::surface::WgpuSurfaceAcquireFailure::Outdated => {
                Self::ConfigureFailed
            }
            crate::bridge::wgpu::surface::WgpuSurfaceAcquireFailure::Timeout
            | crate::bridge::wgpu::surface::WgpuSurfaceAcquireFailure::Occluded
            | crate::bridge::wgpu::surface::WgpuSurfaceAcquireFailure::Validation => {
                Self::PresentFailed
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererSurfaceStage {
    Lifecycle,
    Configure,
    Record,
    Submit,
    Present,
}

pub fn install_fun_renderer_resources(world: &mut World) {
    world.init_resource::<RendererIntegrationConfig>();
    world.init_resource::<FunRendererRenderWorld>();
    world.init_resource::<RendererSurfaceRegistry>();
    world.init_resource::<RendererPipelineCacheResource>();
    world.init_resource::<RendererResourceRegistryResource>();
    world.init_resource::<RendererFrameAdmissions>();
}

pub fn admit_renderer_frame(world: &mut World, admission: RendererFrameAdmission) {
    world
        .resource_mut::<RendererFrameAdmissions>()
        .admit(admission);
    world.resource_mut::<FunRendererRenderWorld>().begin_frame();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn primary_ids() -> (SurfaceId, WindowId) {
        (SurfaceId::PRIMARY, WindowId::PRIMARY)
    }

    #[test]
    fn install_resources_uses_fun_world_only() {
        let mut world = World::default();
        install_fun_renderer_resources(&mut world);

        assert!(world.contains_resource::<RendererIntegrationConfig>());
        assert!(world.contains_resource::<FunRendererRenderWorld>());
        assert!(world.contains_resource::<RendererSurfaceRegistry>());
        assert!(world.contains_resource::<RendererPipelineCacheResource>());
        assert!(world.contains_resource::<RendererResourceRegistryResource>());
        assert!(world.contains_resource::<RendererFrameAdmissions>());
        assert!(
            world
                .resource::<RendererIntegrationConfig>()
                .scheduler_visible_phases
        );
        assert!(
            world
                .resource::<RendererIntegrationConfig>()
                .backend_handles_private
        );
    }

    #[test]
    fn window_created_and_handles_create_surface_request() {
        let (surface, window) = primary_ids();
        let mut registry = RendererSurfaceRegistry::default();

        let created = registry.apply_lifecycle_event(SurfaceLifecycleEvent::WindowCreated {
            surface,
            window,
            size: SurfaceSize::DEFAULT,
            scale: DpiScale::ONE,
        });
        assert_eq!(created.errors, []);
        assert!(matches!(
            created.actions.as_slice(),
            [RendererSurfaceAction::CreateSurface { request }]
                if request.surface() == surface && request.window() == window
        ));

        let handles = registry
            .apply_lifecycle_event(SurfaceLifecycleEvent::HandlesAvailable { surface, window });
        assert_eq!(handles.errors, []);
        assert!(handles.actions.iter().any(|action| matches!(
            action,
            RendererSurfaceAction::HandlesAvailable { surface: s, window: w }
                if *s == surface && *w == window
        )));
        assert!(handles.actions.iter().any(|action| matches!(
            action,
            RendererSurfaceAction::ReconfigureSurface {
                reason: RendererSurfaceReconfigureReason::HandlesAvailable,
                ..
            }
        )));
    }

    #[test]
    fn resize_redraw_emits_reconfigure_then_scheduler_visible_present_work() {
        let (surface, window) = primary_ids();
        let mut registry = RendererSurfaceRegistry::default();
        let size = SurfaceSize::new(1920, 1080).expect("valid surface size");

        registry.apply_lifecycle_event(SurfaceLifecycleEvent::WindowCreated {
            surface,
            window,
            size: SurfaceSize::DEFAULT,
            scale: DpiScale::ONE,
        });
        let resized = registry.apply_lifecycle_event(SurfaceLifecycleEvent::Resized {
            surface,
            window,
            size,
        });
        assert!(matches!(
            resized.actions.as_slice(),
            [RendererSurfaceAction::ReconfigureSurface {
                reason: RendererSurfaceReconfigureReason::Resized,
                ..
            }]
        ));

        let redraw = registry
            .apply_lifecycle_event(SurfaceLifecycleEvent::RedrawRequested { surface, window });
        assert_eq!(redraw.errors, []);
        assert!(redraw.actions.iter().any(|action| matches!(
            action,
            RendererSurfaceAction::ReconfigureSurface {
                reason: RendererSurfaceReconfigureReason::BeforePresent,
                ..
            }
        )));
        let admission = redraw.actions.iter().find_map(|action| match action {
            RendererSurfaceAction::AdmitFrame(admission) => Some(admission),
            _ => None,
        });
        let admission = admission
            .expect("redraw admits renderer frame work")
            .clone();
        assert_eq!(admission.size, size);
        assert!(admission.reconfigure_before_present);
        assert!(admission.work.contains(RendererWorkKind::FrameGraphBuild));
        assert!(admission.work.contains(RendererWorkKind::FrameGraphCompile));
        assert!(admission.work.contains(RendererWorkKind::RecordPass));
        assert!(admission.work.contains(RendererWorkKind::SubmitQueue));
        assert!(admission.work.contains(RendererWorkKind::PresentSurface));

        let mut world = World::default();
        install_fun_renderer_resources(&mut world);
        admit_renderer_frame(&mut world, admission.clone());
        assert_eq!(
            world.resource::<RendererFrameAdmissions>().admitted,
            [admission]
        );
        assert_eq!(
            world.resource::<FunRendererRenderWorld>().frame_generation,
            1
        );
    }

    #[test]
    fn destroyed_window_releases_surface_resources() {
        let (surface, window) = primary_ids();
        let mut registry = RendererSurfaceRegistry::default();

        registry.apply_lifecycle_event(SurfaceLifecycleEvent::WindowCreated {
            surface,
            window,
            size: SurfaceSize::DEFAULT,
            scale: DpiScale::ONE,
        });
        let destroyed =
            registry.apply_lifecycle_event(SurfaceLifecycleEvent::Destroyed { surface, window });

        assert!(matches!(
            destroyed.actions.as_slice(),
            [RendererSurfaceAction::ReleaseSurface { surface: s, window: w }]
                if *s == surface && *w == window
        ));
        assert_eq!(registry.surface_count(), 0);
    }

    #[test]
    fn surface_and_device_lost_have_structured_error_path() {
        let (surface, window) = primary_ids();
        let mut registry = RendererSurfaceRegistry::default();
        registry.apply_lifecycle_event(SurfaceLifecycleEvent::WindowCreated {
            surface,
            window,
            size: SurfaceSize::DEFAULT,
            scale: DpiScale::ONE,
        });

        let lost = registry.mark_surface_lost(surface);
        assert_eq!(lost.kind, RendererSurfaceErrorKind::SurfaceLost);
        assert!(lost.recoverable);
        assert_eq!(lost.stage, RendererSurfaceStage::Present);
        assert_eq!(
            registry.surface(surface).map(|record| record.state),
            Some(RendererSurfaceState::Lost)
        );

        let device_lost = RendererSurfaceError::device_lost(Some(surface), Some(window));
        assert_eq!(device_lost.kind, RendererSurfaceErrorKind::DeviceLost);
        assert!(!device_lost.recoverable);
        assert_eq!(device_lost.stage, RendererSurfaceStage::Submit);
    }

    #[cfg(feature = "wgpu_bridge")]
    #[test]
    fn wgpu_present_failures_are_classified_for_recovery_policy() {
        use crate::bridge::wgpu::surface::WgpuSurfaceAcquireFailure;

        assert_eq!(
            RendererSurfaceErrorKind::from(WgpuSurfaceAcquireFailure::Lost),
            RendererSurfaceErrorKind::SurfaceLost
        );
        assert_eq!(
            RendererSurfaceErrorKind::from(WgpuSurfaceAcquireFailure::Outdated),
            RendererSurfaceErrorKind::ConfigureFailed
        );
        for failure in [
            WgpuSurfaceAcquireFailure::Timeout,
            WgpuSurfaceAcquireFailure::Occluded,
            WgpuSurfaceAcquireFailure::Validation,
        ] {
            assert_eq!(
                RendererSurfaceErrorKind::from(failure),
                RendererSurfaceErrorKind::PresentFailed
            );
        }
    }
}
