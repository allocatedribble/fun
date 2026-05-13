pub mod ffi;
mod params;
mod support;

use core::ptr::NonNull;
use std::ffi::{CStr, CString};

pub use params::{CreateDesc, DlssMode, DlssSizeState, EvaluateDesc, ResizeDesc};
pub use support::{DlssRuntimeDiscovery, DlssSupportReport, query_support_from_env};

use crate::{
    ffi::{FunDlssCreateDesc, FunDlssEvaluateDesc, FunDlssResizeDesc, FunDlssSupport},
    params::bool_i32,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dx12DlssError {
    pub code: ffi::FunDlssResult,
    pub message: String,
}

impl core::fmt::Display for Dx12DlssError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for Dx12DlssError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DlssSupport {
    pub sr_supported: bool,
    pub rr_supported: bool,
    pub needs_updated_driver: bool,
}

pub struct Dx12DlssContext {
    raw: NonNull<ffi::FunDlssContext>,
    current_size: Option<DlssSizeState>,
    support: DlssSupport,
}

impl Dx12DlssContext {
    /// Creates a native DLSS context around borrowed D3D12 device and queue pointers.
    ///
    /// # Safety
    ///
    /// `desc.d3d12_device` and `desc.d3d12_queue` must be valid D3D12 COM
    /// pointers that outlive the returned context. The native shim does not own
    /// the renderer, and callers must destroy this context before the RetiredEngine/wgpu
    /// device or queue is torn down.
    pub unsafe fn create(desc: CreateDesc) -> Result<Self, Dx12DlssError> {
        let app_id = cstring_lossless(desc.app_id);
        let app_name = cstring_lossless(desc.app_name);
        let sdk_path = desc
            .sdk_path
            .as_ref()
            .map(|path| cstring_lossless(path.to_string_lossy().as_ref()));
        let ffi_desc = FunDlssCreateDesc {
            app_id: app_id.as_ptr(),
            app_name: app_name.as_ptr(),
            sdk_path: sdk_path
                .as_ref()
                .map_or(core::ptr::null(), |path| path.as_ptr()),
            d3d12_device: desc.d3d12_device,
            d3d12_queue: desc.d3d12_queue,
            enable_debug: bool_i32(desc.enable_debug),
        };
        let Some(raw) = NonNull::new(unsafe { ffi::fun_dlss_create(&ffi_desc) }) else {
            return Err(Dx12DlssError {
                code: ffi::FunDlssResult::SdkInitFailed,
                message: String::from("native DLSS context creation returned null"),
            });
        };

        let mut context = Self {
            raw,
            current_size: None,
            support: DlssSupport {
                sr_supported: false,
                rr_supported: false,
                needs_updated_driver: false,
            },
        };
        context.support = context.query_support()?;
        Ok(context)
    }

    pub fn support(&self) -> DlssSupport {
        self.support
    }

    pub fn current_size(&self) -> Option<DlssSizeState> {
        self.current_size
    }

    pub fn query_support(&self) -> Result<DlssSupport, Dx12DlssError> {
        let mut support = FunDlssSupport::default();
        let result = ffi::FunDlssResult::from_i32(unsafe {
            ffi::fun_dlss_query_support(self.raw.as_ptr(), &mut support)
        });
        match result {
            ffi::FunDlssResult::Ok
            | ffi::FunDlssResult::Unsupported
            | ffi::FunDlssResult::MissingDll => Ok(DlssSupport {
                sr_supported: support.sr_supported != 0,
                rr_supported: support.rr_supported != 0,
                needs_updated_driver: support.needs_updated_driver != 0,
            }),
            error => Err(self.error(error)),
        }
    }

    pub fn resize(&mut self, desc: ResizeDesc) -> Result<(), Dx12DlssError> {
        let ffi_desc = FunDlssResizeDesc {
            input_width: desc.input_width,
            input_height: desc.input_height,
            output_width: desc.output_width,
            output_height: desc.output_height,
            mode: desc.mode.as_ffi(),
            hdr: bool_i32(desc.hdr),
        };
        let result = ffi::FunDlssResult::from_i32(unsafe {
            ffi::fun_dlss_resize(self.raw.as_ptr(), &ffi_desc)
        });
        if result == ffi::FunDlssResult::Ok {
            self.current_size = Some(desc.into());
            Ok(())
        } else {
            Err(self.error(result))
        }
    }

    /// Evaluates DLSS against native D3D12 resources.
    ///
    /// # Safety
    ///
    /// All raw pointers in `desc` must be valid for the duration of the call.
    /// The command list must be open and recording. Resource states must match
    /// the FUN render-state plan, and resources must not alias unless the native
    /// SDK path has explicitly documented and validated that aliasing.
    pub unsafe fn evaluate(&mut self, desc: EvaluateDesc) -> Result<(), Dx12DlssError> {
        let ffi_desc = FunDlssEvaluateDesc {
            d3d12_cmdlist: desc.d3d12_cmdlist,
            input_color: desc.input_color,
            output_color: desc.output_color,
            depth: desc.depth,
            motion_vectors: desc.motion_vectors,
            exposure: desc.exposure,
            input_width: desc.input_width,
            input_height: desc.input_height,
            output_width: desc.output_width,
            output_height: desc.output_height,
            jitter_x: desc.jitter_x,
            jitter_y: desc.jitter_y,
            sharpness: desc.sharpness,
            camera_near: desc.camera_near,
            camera_far: desc.camera_far,
            reset_history: bool_i32(desc.reset_history),
            hdr: bool_i32(desc.hdr),
            motion_vectors_are_low_res: bool_i32(desc.motion_vectors_are_low_res),
            inverted_depth: bool_i32(desc.inverted_depth),
            reserved: 0,
        };
        let result = ffi::FunDlssResult::from_i32(unsafe {
            ffi::fun_dlss_evaluate(self.raw.as_ptr(), &ffi_desc)
        });
        if result == ffi::FunDlssResult::Ok {
            Ok(())
        } else {
            Err(self.error(result))
        }
    }

    fn error(&self, code: ffi::FunDlssResult) -> Dx12DlssError {
        let raw = unsafe { ffi::fun_dlss_last_error(self.raw.as_ptr()) };
        let message = if raw.is_null() {
            String::from("native DLSS bridge returned no error detail")
        } else {
            unsafe { CStr::from_ptr(raw) }
                .to_string_lossy()
                .into_owned()
        };
        Dx12DlssError { code, message }
    }
}

impl Drop for Dx12DlssContext {
    fn drop(&mut self) {
        unsafe {
            ffi::fun_dlss_destroy(self.raw.as_ptr());
        }
    }
}

fn cstring_lossless(value: &str) -> CString {
    let bytes = value
        .as_bytes()
        .iter()
        .copied()
        .filter(|byte| *byte != 0)
        .collect::<Vec<_>>();
    CString::new(bytes).expect("interior NUL bytes were filtered")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_result_labels_are_stable() {
        assert_eq!(ffi::FunDlssResult::MissingDll.as_str(), "missing_dll");
        assert_eq!(
            ffi::FunDlssResult::SdkEvaluateFailed.as_str(),
            "sdk_evaluate_failed"
        );
    }

    #[test]
    fn support_query_from_env_fails_closed_without_runtime() {
        let support = query_support_from_env();
        assert!(!support.sr_supported);
        assert!(!support.rr_supported);
    }
}
