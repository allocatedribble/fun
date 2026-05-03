#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum CefUiJavaScriptAuthority {
    PresentationOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum CefUiHelperProcessPolicy {
    CefInternalHelpersAllowed,
    RegisterWithWardenBeforeChildProcessEnforcement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum CefUiStateExposurePolicy {
    UiSafeStateOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct CefUiSecurityPolicy {
    pub javascript_authority: CefUiJavaScriptAuthority,
    pub helper_process_policy: CefUiHelperProcessPolicy,
    pub state_exposure_policy: CefUiStateExposurePolicy,
    pub expose_warden_material_to_js: bool,
}

impl CefUiSecurityPolicy {
    pub const DEFAULT: Self = Self {
        javascript_authority: CefUiJavaScriptAuthority::PresentationOnly,
        helper_process_policy:
            CefUiHelperProcessPolicy::RegisterWithWardenBeforeChildProcessEnforcement,
        state_exposure_policy: CefUiStateExposurePolicy::UiSafeStateOnly,
        expose_warden_material_to_js: false,
    };

    #[must_use]
    pub const fn exposes_warden_material_to_js(self) -> bool {
        self.expose_warden_material_to_js
    }
}

impl Default for CefUiSecurityPolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_keeps_js_presentation_only_and_warden_safe() {
        let policy = CefUiSecurityPolicy::default();

        assert_eq!(
            policy.javascript_authority,
            CefUiJavaScriptAuthority::PresentationOnly
        );
        assert!(!policy.exposes_warden_material_to_js());
        assert_eq!(
            policy.helper_process_policy,
            CefUiHelperProcessPolicy::RegisterWithWardenBeforeChildProcessEnforcement
        );
    }
}
