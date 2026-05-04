use core::{ffi::c_void, fmt};

use super::diagnostics::{Dx12NativeInteropError, Dx12NativeInteropFailure, failure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dx12NativeObjectKind {
    Texture,
    Buffer,
    CommandQueue,
    CommandList,
    Fence,
    NativeInteropResource,
    CefRingTexture,
    DlssInput,
    DlssOutput,
}

impl Dx12NativeObjectKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Texture => "texture",
            Self::Buffer => "buffer",
            Self::CommandQueue => "command_queue",
            Self::CommandList => "command_list",
            Self::Fence => "fence",
            Self::NativeInteropResource => "native_interop_resource",
            Self::CefRingTexture => "cef_ring_texture",
            Self::DlssInput => "dlss_input",
            Self::DlssOutput => "dlss_output",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dx12ObjectLabel<'a> {
    Logical {
        kind: Dx12NativeObjectKind,
        subsystem: &'static str,
        pass: &'static str,
        name: &'a str,
        generation: Option<u64>,
        width: Option<u32>,
        height: Option<u32>,
        format: Option<&'a str>,
    },
    CefRingTexture {
        slot_index: usize,
        width: u32,
        height: u32,
        format: &'a str,
    },
    DlssSrInput {
        surface: &'a str,
        mode: &'a str,
    },
    DlssSrOutput {
        mode: &'a str,
    },
}

impl<'a> Dx12ObjectLabel<'a> {
    pub const fn logical(
        kind: Dx12NativeObjectKind,
        subsystem: &'static str,
        pass: &'static str,
        name: &'a str,
    ) -> Self {
        Self::Logical {
            kind,
            subsystem,
            pass,
            name,
            generation: None,
            width: None,
            height: None,
            format: None,
        }
    }

    pub const fn cef_ring_texture(
        slot_index: usize,
        width: u32,
        height: u32,
        format: &'a str,
    ) -> Self {
        Self::CefRingTexture {
            slot_index,
            width,
            height,
            format,
        }
    }

    pub const fn dlss_sr_input(surface: &'a str, mode: &'a str) -> Self {
        Self::DlssSrInput { surface, mode }
    }

    pub const fn dlss_sr_output(mode: &'a str) -> Self {
        Self::DlssSrOutput { mode }
    }

    pub fn rendered_name(self) -> String {
        use core::fmt::Write as _;

        let mut output = String::with_capacity(96);
        let _ = write!(&mut output, "{self}");
        output
    }
}

impl fmt::Display for Dx12ObjectLabel<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Logical {
                kind: _,
                subsystem,
                pass,
                name,
                generation,
                width,
                height,
                format,
            } => {
                write!(f, "FUN.{subsystem}.{pass}.{name}")?;
                if let Some(generation) = generation {
                    write!(f, ".Gen{generation}")?;
                }
                if let (Some(width), Some(height)) = (width, height) {
                    write!(f, ".{width}x{height}")?;
                }
                if let Some(format) = format {
                    write!(f, ".{format}")?;
                }
                Ok(())
            }
            Self::CefRingTexture {
                slot_index,
                width,
                height,
                format,
            } => write!(
                f,
                "FUN.CEF.RingSlot[{slot_index}].{width}x{height}.{format}"
            ),
            Self::DlssSrInput { surface, mode } => {
                write!(f, "FUN.DLSS.SR.Input.{surface}.{mode}")
            }
            Self::DlssSrOutput { mode } => write!(f, "FUN.DLSS.SR.Output.{mode}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dx12ObjectNameOutcome {
    Named,
    Disabled,
}

pub const fn dx12_object_naming_enabled() -> bool {
    cfg!(all(
        target_os = "windows",
        feature = "dx12_native_object_names"
    ))
}

/// Sets a debug name on a native D3D12 object for PIX, Nsight, and compatible captures.
///
/// # Safety
///
/// `raw_object` must be a live COM pointer for an object implementing
/// `ID3D12Object`. The function borrows the pointer only for the call and does
/// not take ownership of the object.
pub unsafe fn set_dx12_object_name(
    raw_object: *mut c_void,
    label: Dx12ObjectLabel<'_>,
) -> Result<Dx12ObjectNameOutcome, Dx12NativeInteropError> {
    if raw_object.is_null() {
        return Err(failure(
            Dx12NativeInteropFailure::ObjectNameUnavailable,
            "DX12 object naming received a null object pointer",
        ));
    }

    #[cfg(all(target_os = "windows", feature = "dx12_native_object_names"))]
    {
        set_dx12_object_name_enabled(raw_object, label)
    }
    #[cfg(not(all(target_os = "windows", feature = "dx12_native_object_names")))]
    {
        let _ = label;
        Ok(Dx12ObjectNameOutcome::Disabled)
    }
}

#[cfg(all(target_os = "windows", feature = "dx12_native_object_names"))]
fn set_dx12_object_name_enabled(
    raw_object: *mut c_void,
    label: Dx12ObjectLabel<'_>,
) -> Result<Dx12ObjectNameOutcome, Dx12NativeInteropError> {
    use windows::{
        Win32::Graphics::Direct3D12::ID3D12Object,
        core::{Interface as _, PCWSTR},
    };

    let object = unsafe {
        ID3D12Object::from_raw_borrowed(&raw_object).ok_or_else(|| {
            failure(
                Dx12NativeInteropFailure::ObjectNameUnavailable,
                "DX12 object naming could not borrow ID3D12Object",
            )
        })?
    };
    let name = label.rendered_name();
    let wide_name = name
        .encode_utf16()
        .chain(core::iter::once(0))
        .collect::<Vec<_>>();
    unsafe { object.SetName(PCWSTR(wide_name.as_ptr())) }.map_err(|_| {
        failure(
            Dx12NativeInteropFailure::ObjectNameFailed,
            "ID3D12Object::SetName failed",
        )
    })?;
    Ok(Dx12ObjectNameOutcome::Named)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cef_ring_texture_label_matches_capture_contract() {
        let label = Dx12ObjectLabel::cef_ring_texture(2, 1280, 720, "BGRA8");
        assert_eq!(label.rendered_name(), "FUN.CEF.RingSlot[2].1280x720.BGRA8");
    }

    #[test]
    fn dlss_sr_output_label_matches_capture_contract() {
        let label = Dx12ObjectLabel::dlss_sr_output("Quality");
        assert_eq!(label.rendered_name(), "FUN.DLSS.SR.Output.Quality");
    }
}
