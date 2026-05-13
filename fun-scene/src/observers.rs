#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunOnTemplate;

#[must_use]
pub fn fun_on<T>(_target: T) -> FunOnTemplate {
    FunOnTemplate
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunSceneObserverPolicy {
    pub observers_are_scene_authored: bool,
    pub observer_targets_require_validation: bool,
}

impl FunSceneObserverPolicy {
    pub const DEFAULT: Self = Self {
        observers_are_scene_authored: true,
        observer_targets_require_validation: true,
    };
}

impl Default for FunSceneObserverPolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}
