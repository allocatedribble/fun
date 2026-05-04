use core::ffi::{c_char, c_void};

#[repr(C)]
pub struct FunDlssContext {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunDlssMode {
    Quality = 0,
    Balanced = 1,
    Performance = 2,
    UltraPerformance = 3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunDlssResult {
    Ok = 0,
    Unsupported = 1,
    InvalidArgument = 2,
    InvalidDevice = 3,
    InvalidCommandList = 4,
    InvalidResource = 5,
    InvalidDimensions = 6,
    SdkInitFailed = 7,
    SdkEvaluateFailed = 8,
    MissingDll = 9,
    DriverUnsupported = 10,
}

impl FunDlssResult {
    pub const fn from_i32(value: i32) -> Self {
        match value {
            0 => Self::Ok,
            1 => Self::Unsupported,
            2 => Self::InvalidArgument,
            3 => Self::InvalidDevice,
            4 => Self::InvalidCommandList,
            5 => Self::InvalidResource,
            6 => Self::InvalidDimensions,
            7 => Self::SdkInitFailed,
            8 => Self::SdkEvaluateFailed,
            9 => Self::MissingDll,
            10 => Self::DriverUnsupported,
            _ => Self::Unsupported,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Unsupported => "unsupported",
            Self::InvalidArgument => "invalid_argument",
            Self::InvalidDevice => "invalid_device",
            Self::InvalidCommandList => "invalid_command_list",
            Self::InvalidResource => "invalid_resource",
            Self::InvalidDimensions => "invalid_dimensions",
            Self::SdkInitFailed => "sdk_init_failed",
            Self::SdkEvaluateFailed => "sdk_evaluate_failed",
            Self::MissingDll => "missing_dll",
            Self::DriverUnsupported => "driver_unsupported",
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FunDlssSupport {
    pub sr_supported: i32,
    pub rr_supported: i32,
    pub needs_updated_driver: i32,
    pub reserved: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FunDlssCreateDesc {
    pub app_id: *const c_char,
    pub app_name: *const c_char,
    pub sdk_path: *const c_char,
    pub d3d12_device: *mut c_void,
    pub d3d12_queue: *mut c_void,
    pub enable_debug: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FunDlssResizeDesc {
    pub input_width: u32,
    pub input_height: u32,
    pub output_width: u32,
    pub output_height: u32,
    pub mode: FunDlssMode,
    pub hdr: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FunDlssEvaluateDesc {
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
    pub reset_history: i32,
    pub hdr: i32,
    pub motion_vectors_are_low_res: i32,
    pub inverted_depth: i32,
    pub reserved: i32,
}

unsafe extern "C" {
    pub fn fun_dlss_create(desc: *const FunDlssCreateDesc) -> *mut FunDlssContext;
    pub fn fun_dlss_destroy(ctx: *mut FunDlssContext);
    pub fn fun_dlss_query_support(
        ctx: *mut FunDlssContext,
        out_support: *mut FunDlssSupport,
    ) -> i32;
    pub fn fun_dlss_resize(ctx: *mut FunDlssContext, desc: *const FunDlssResizeDesc) -> i32;
    pub fn fun_dlss_evaluate(ctx: *mut FunDlssContext, desc: *const FunDlssEvaluateDesc) -> i32;
    pub fn fun_dlss_last_error(ctx: *mut FunDlssContext) -> *const c_char;
}
