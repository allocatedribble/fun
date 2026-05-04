use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DlssRuntimeDiscovery {
    pub sdk_root: Option<PathBuf>,
    pub runtime_dll: Option<PathBuf>,
}

impl DlssRuntimeDiscovery {
    #[must_use]
    pub fn from_env() -> Self {
        Self::from_env_reader(|name| std::env::var(name).ok())
    }

    #[must_use]
    pub fn from_env_reader(mut read: impl FnMut(&'static str) -> Option<String>) -> Self {
        let sdk_root = read("NVIDIA_STREAMLINE_SDK")
            .or_else(|| read("NVIDIA_NGX_SDK"))
            .or_else(|| read("FUN_NVIDIA_DLSS_SDK"))
            .map(PathBuf::from);
        let runtime_dll = read("FUN_NVIDIA_DLSS_DLL").map(PathBuf::from).or_else(|| {
            sdk_root
                .as_ref()
                .and_then(|root| find_runtime_dll(root.as_path()))
        });
        Self {
            sdk_root,
            runtime_dll,
        }
    }

    #[must_use]
    pub fn runtime_found(&self) -> bool {
        self.runtime_dll.as_ref().is_some_and(|path| path.is_file())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DlssSupportReport {
    pub runtime_found: bool,
    pub sr_supported: bool,
    pub rr_supported: bool,
    pub needs_updated_driver: bool,
    pub reason: &'static str,
}

impl DlssSupportReport {
    #[must_use]
    pub fn from_discovery(discovery: &DlssRuntimeDiscovery) -> Self {
        if discovery.runtime_found() {
            Self {
                runtime_found: true,
                sr_supported: false,
                rr_supported: false,
                needs_updated_driver: false,
                reason: "native_streamline_or_ngx_sdk_not_linked",
            }
        } else {
            Self {
                runtime_found: false,
                sr_supported: false,
                rr_supported: false,
                needs_updated_driver: false,
                reason: "runtime_dll_not_found",
            }
        }
    }
}

#[must_use]
pub fn query_support_from_env() -> DlssSupportReport {
    let discovery = DlssRuntimeDiscovery::from_env();
    DlssSupportReport::from_discovery(&discovery)
}

fn find_runtime_dll(root: &Path) -> Option<PathBuf> {
    const RUNTIME_DLL_CANDIDATES: [&str; 4] = [
        "bin/x64/sl.interposer.dll",
        "bin/x64/nvngx_dlss.dll",
        "lib/x64/nvngx_dlss.dll",
        "nvngx_dlss.dll",
    ];

    RUNTIME_DLL_CANDIDATES
        .iter()
        .map(|relative| root.join(relative))
        .find(|candidate| candidate.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_runtime_reports_fail_closed_support() {
        let discovery = DlssRuntimeDiscovery::from_env_reader(|_| None);
        let report = DlssSupportReport::from_discovery(&discovery);

        assert!(!report.runtime_found);
        assert!(!report.sr_supported);
        assert_eq!(report.reason, "runtime_dll_not_found");
    }

    #[test]
    fn explicit_runtime_path_is_recorded_without_claiming_sr_support() {
        let discovery = DlssRuntimeDiscovery::from_env_reader(|name| match name {
            "FUN_NVIDIA_DLSS_DLL" => Some("C:/missing/nvngx_dlss.dll".into()),
            _ => None,
        });

        assert_eq!(
            discovery.runtime_dll,
            Some(PathBuf::from("C:/missing/nvngx_dlss.dll"))
        );
        assert!(!DlssSupportReport::from_discovery(&discovery).sr_supported);
    }
}
