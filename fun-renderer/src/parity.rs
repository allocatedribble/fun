#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackendParitySceneKind {
    ClearPresent,
    StaticMesh,
    Material,
    DepthMotionVector,
    CefUiComposite,
    PostUpscalePlaceholder,
    BenchmarkCapture,
}

impl BackendParitySceneKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClearPresent => "clear_present",
            Self::StaticMesh => "static_mesh",
            Self::Material => "material",
            Self::DepthMotionVector => "depth_motion_vector",
            Self::CefUiComposite => "cef_ui_composite",
            Self::PostUpscalePlaceholder => "post_upscale_placeholder",
            Self::BenchmarkCapture => "benchmark_capture",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BackendParityScene {
    pub stable_id: &'static str,
    pub kind: BackendParitySceneKind,
    pub requires_depth: bool,
    pub requires_motion_vectors: bool,
    pub requires_cef_gpu_transport: bool,
    pub premium_gate: bool,
}

impl BackendParityScene {
    #[must_use]
    pub const fn new(
        stable_id: &'static str,
        kind: BackendParitySceneKind,
        requires_depth: bool,
        requires_motion_vectors: bool,
        requires_cef_gpu_transport: bool,
        premium_gate: bool,
    ) -> Self {
        Self {
            stable_id,
            kind,
            requires_depth,
            requires_motion_vectors,
            requires_cef_gpu_transport,
            premium_gate,
        }
    }
}

pub const BACKEND_PARITY_SCENE_IDS: [&str; 7] = [
    "backend_parity.clear_present",
    "backend_parity.static_mesh",
    "backend_parity.material",
    "backend_parity.depth_motion_vector",
    "backend_parity.cef_ui_composite",
    "backend_parity.post_upscale_placeholder",
    "backend_parity.benchmark_capture",
];

pub const BACKEND_PARITY_SCENES: [BackendParityScene; 7] = [
    BackendParityScene::new(
        BACKEND_PARITY_SCENE_IDS[0],
        BackendParitySceneKind::ClearPresent,
        false,
        false,
        false,
        false,
    ),
    BackendParityScene::new(
        BACKEND_PARITY_SCENE_IDS[1],
        BackendParitySceneKind::StaticMesh,
        false,
        false,
        false,
        false,
    ),
    BackendParityScene::new(
        BACKEND_PARITY_SCENE_IDS[2],
        BackendParitySceneKind::Material,
        false,
        false,
        false,
        false,
    ),
    BackendParityScene::new(
        BACKEND_PARITY_SCENE_IDS[3],
        BackendParitySceneKind::DepthMotionVector,
        true,
        true,
        false,
        false,
    ),
    BackendParityScene::new(
        BACKEND_PARITY_SCENE_IDS[4],
        BackendParitySceneKind::CefUiComposite,
        false,
        false,
        true,
        false,
    ),
    BackendParityScene::new(
        BACKEND_PARITY_SCENE_IDS[5],
        BackendParitySceneKind::PostUpscalePlaceholder,
        true,
        true,
        false,
        true,
    ),
    BackendParityScene::new(
        BACKEND_PARITY_SCENE_IDS[6],
        BackendParitySceneKind::BenchmarkCapture,
        false,
        false,
        false,
        false,
    ),
];

#[must_use]
pub fn backend_parity_scene(stable_id: &str) -> Option<&'static BackendParityScene> {
    BACKEND_PARITY_SCENES
        .iter()
        .find(|scene| scene.stable_id == stable_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parity_scene_catalog_contains_required_dx12_lanes() {
        assert_eq!(BACKEND_PARITY_SCENES.len(), 7);
        assert!(
            BACKEND_PARITY_SCENES
                .iter()
                .any(|scene| scene.kind == BackendParitySceneKind::ClearPresent)
        );
        assert!(
            BACKEND_PARITY_SCENES
                .iter()
                .any(|scene| scene.kind == BackendParitySceneKind::StaticMesh)
        );
        assert!(
            BACKEND_PARITY_SCENES
                .iter()
                .any(|scene| scene.kind == BackendParitySceneKind::Material)
        );
        assert!(
            BACKEND_PARITY_SCENES
                .iter()
                .any(|scene| scene.requires_depth && scene.requires_motion_vectors)
        );
        assert!(
            BACKEND_PARITY_SCENES
                .iter()
                .any(|scene| scene.requires_cef_gpu_transport)
        );
        assert!(BACKEND_PARITY_SCENES.iter().any(|scene| scene.premium_gate));
    }
}
