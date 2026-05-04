use core::ffi::c_void;
use std::path::PathBuf;

use crate::ffi;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DlssMode {
    Quality,
    Balanced,
    Performance,
    UltraPerformance,
}

impl DlssMode {
    pub const fn as_ffi(self) -> ffi::FunDlssMode {
        match self {
            Self::Quality => ffi::FunDlssMode::Quality,
            Self::Balanced => ffi::FunDlssMode::Balanced,
            Self::Performance => ffi::FunDlssMode::Performance,
            Self::UltraPerformance => ffi::FunDlssMode::UltraPerformance,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CreateDesc {
    pub app_id: &'static str,
    pub app_name: &'static str,
    pub sdk_path: Option<PathBuf>,
    pub d3d12_device: *mut c_void,
    pub d3d12_queue: *mut c_void,
    pub enable_debug: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResizeDesc {
    pub input_width: u32,
    pub input_height: u32,
    pub output_width: u32,
    pub output_height: u32,
    pub mode: DlssMode,
    pub hdr: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct EvaluateDesc {
    pub d3d12_cmdlist: *mut c_void,
    pub input_color: *mut c_void,
    pub output_color: *mut c_void,
    pub depth: *mut c_void,
    pub motion_vectors: *mut c_void,
    pub exposure: *mut c_void,
    pub input_width: u32,
    pub input_height: u32,
    pub output_width: u32,
    pub output_height: u32,
    pub jitter_x: f32,
    pub jitter_y: f32,
    pub sharpness: f32,
    pub camera_near: f32,
    pub camera_far: f32,
    pub reset_history: bool,
    pub hdr: bool,
    pub motion_vectors_are_low_res: bool,
    pub inverted_depth: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DlssSizeState {
    pub input_width: u32,
    pub input_height: u32,
    pub output_width: u32,
    pub output_height: u32,
    pub mode: DlssMode,
    pub hdr: bool,
}

impl From<ResizeDesc> for DlssSizeState {
    fn from(value: ResizeDesc) -> Self {
        Self {
            input_width: value.input_width,
            input_height: value.input_height,
            output_width: value.output_width,
            output_height: value.output_height,
            mode: value.mode,
            hdr: value.hdr,
        }
    }
}

pub const fn bool_i32(value: bool) -> i32 {
    if value { 1 } else { 0 }
}
