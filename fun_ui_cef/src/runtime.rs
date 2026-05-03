use std::{
    env,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use cef::{CefString, Settings, args::Args};

use crate::bootstrap::new_fun_cef_app;

const FUN_CEF_REMOTE_DEBUGGING_PORT: &str = "FUN_CEF_REMOTE_DEBUGGING_PORT";
const FUN_CEF_USER_DATA_DIR: &str = "FUN_CEF_USER_DATA_DIR";
const FUN_CEF_MULTI_THREADED_MESSAGE_LOOP: &str = "FUN_CEF_MULTI_THREADED_MESSAGE_LOOP";
const FUN_CEF_EXTERNAL_MESSAGE_PUMP: &str = "FUN_CEF_EXTERNAL_MESSAGE_PUMP";
const DEV_CACHE_DIR: &str = "target/fun-cef";
const PROD_CACHE_DIR: &str = "Fun/cef";
const BOUNDED_SHUTDOWN_DRAIN: Duration = Duration::from_millis(250);

static CEF_INITIALIZED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CefMessageLoopStrategy {
    MultiThreaded,
    ExternalPump,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CefRuntimeConfig {
    pub cache_path: PathBuf,
    pub message_loop_strategy: CefMessageLoopStrategy,
    pub remote_debugging_port: Option<u16>,
    pub transparent_painting: bool,
    pub bounded_shutdown_drain: Duration,
}

impl CefRuntimeConfig {
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            cache_path: cache_path_from_env(),
            message_loop_strategy: message_loop_strategy_from_env(),
            remote_debugging_port: remote_debugging_port_from_env(),
            transparent_painting: true,
            bounded_shutdown_drain: BOUNDED_SHUTDOWN_DRAIN,
        }
    }

    #[must_use]
    pub fn cef_settings(&self) -> Settings {
        let mut settings = Settings {
            windowless_rendering_enabled: 1,
            background_color: transparent_background_color(self.transparent_painting),
            cache_path: cef_string_from_path(&self.cache_path),
            root_cache_path: cef_string_from_path(&self.cache_path),
            remote_debugging_port: self.remote_debugging_port.map_or(0, i32::from),
            ..Settings::default()
        };
        match self.message_loop_strategy {
            CefMessageLoopStrategy::MultiThreaded => {
                settings.multi_threaded_message_loop = 1;
                settings.external_message_pump = 0;
            }
            CefMessageLoopStrategy::ExternalPump => {
                settings.multi_threaded_message_loop = 0;
                settings.external_message_pump = 1;
            }
        }
        settings
    }
}

impl Default for CefRuntimeConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

#[derive(Debug)]
pub enum CefRuntimeError {
    AlreadyInitialized,
    CacheDirectory(std::io::Error),
    InitializeRejected,
}

impl std::fmt::Display for CefRuntimeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyInitialized => formatter.write_str("CEF runtime is already initialized"),
            Self::CacheDirectory(error) => {
                write!(formatter, "CEF cache directory setup failed: {error}")
            }
            Self::InitializeRejected => formatter.write_str("CEF rejected initialization"),
        }
    }
}

impl std::error::Error for CefRuntimeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CacheDirectory(error) => Some(error),
            Self::AlreadyInitialized | Self::InitializeRejected => None,
        }
    }
}

pub struct CefRuntime {
    config: CefRuntimeConfig,
    shutdown_started: bool,
}

impl CefRuntime {
    pub fn initialize(config: CefRuntimeConfig) -> Result<Self, CefRuntimeError> {
        if CEF_INITIALIZED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(CefRuntimeError::AlreadyInitialized);
        }

        if let Err(error) = std::fs::create_dir_all(&config.cache_path) {
            CEF_INITIALIZED.store(false, Ordering::Release);
            return Err(CefRuntimeError::CacheDirectory(error));
        }

        let args = Args::new();
        let settings = config.cef_settings();
        let mut app = new_fun_cef_app();
        let initialized = cef::initialize(
            Some(args.as_main_args()),
            Some(&settings),
            Some(&mut app),
            std::ptr::null_mut(),
        );
        if initialized == 0 {
            CEF_INITIALIZED.store(false, Ordering::Release);
            return Err(CefRuntimeError::InitializeRejected);
        }

        Ok(Self {
            config,
            shutdown_started: false,
        })
    }

    #[must_use]
    pub fn config(&self) -> &CefRuntimeConfig {
        &self.config
    }

    pub fn pump_message_loop_work(&self) {
        if matches!(
            self.config.message_loop_strategy,
            CefMessageLoopStrategy::ExternalPump
        ) {
            cef::do_message_loop_work();
        }
    }

    pub fn begin_shutdown(&mut self) {
        self.shutdown_started = true;
    }

    pub fn shutdown(mut self) {
        self.shutdown_started = true;
        drain_shutdown_callbacks(self.config.bounded_shutdown_drain);
        cef::shutdown();
        CEF_INITIALIZED.store(false, Ordering::Release);
    }

    #[must_use]
    pub const fn shutdown_started(&self) -> bool {
        self.shutdown_started
    }
}

fn drain_shutdown_callbacks(duration: Duration) {
    if duration.is_zero() {
        return;
    }
    let started = std::time::Instant::now();
    while started.elapsed() < duration {
        cef::do_message_loop_work();
        std::thread::yield_now();
    }
}

fn cache_path_from_env() -> PathBuf {
    if let Some(path) = env::var_os(FUN_CEF_USER_DATA_DIR).filter(|value| !value.is_empty()) {
        return PathBuf::from(path);
    }

    if cfg!(debug_assertions) {
        return PathBuf::from(DEV_CACHE_DIR);
    }

    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir)
        .join(PROD_CACHE_DIR)
}

fn message_loop_strategy_from_env() -> CefMessageLoopStrategy {
    if env_flag(FUN_CEF_EXTERNAL_MESSAGE_PUMP) {
        return CefMessageLoopStrategy::ExternalPump;
    }
    if env_flag(FUN_CEF_MULTI_THREADED_MESSAGE_LOOP) {
        return CefMessageLoopStrategy::MultiThreaded;
    }
    if cfg!(target_os = "windows") {
        CefMessageLoopStrategy::MultiThreaded
    } else {
        CefMessageLoopStrategy::ExternalPump
    }
}

fn remote_debugging_port_from_env() -> Option<u16> {
    if !cfg!(feature = "debug_remote") && env::var_os(FUN_CEF_REMOTE_DEBUGGING_PORT).is_none() {
        return None;
    }
    env::var(FUN_CEF_REMOTE_DEBUGGING_PORT)
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|port| *port != 0)
}

fn cef_string_from_path(path: &Path) -> CefString {
    CefString::from(path.to_string_lossy().as_ref())
}

const fn transparent_background_color(enabled: bool) -> u32 {
    if enabled { 0x0000_0000 } else { 0xff00_0000 }
}

fn env_flag(name: &str) -> bool {
    match env::var(name) {
        Ok(value) => {
            value.eq_ignore_ascii_case("1")
                || value.eq_ignore_ascii_case("true")
                || value.eq_ignore_ascii_case("yes")
                || value.eq_ignore_ascii_case("on")
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cef_settings_are_windowless_and_transparent_by_default() {
        let config = CefRuntimeConfig {
            cache_path: PathBuf::from("target/fun-cef-test"),
            message_loop_strategy: CefMessageLoopStrategy::ExternalPump,
            remote_debugging_port: None,
            transparent_painting: true,
            bounded_shutdown_drain: Duration::ZERO,
        };

        let settings = config.cef_settings();

        assert_eq!(settings.windowless_rendering_enabled, 1);
        assert_eq!(settings.background_color, 0x0000_0000);
        assert_eq!(settings.remote_debugging_port, 0);
        assert_eq!(settings.external_message_pump, 1);
        assert_eq!(settings.multi_threaded_message_loop, 0);
    }

    #[test]
    fn remote_debugging_requires_explicit_port() {
        let config = CefRuntimeConfig {
            cache_path: PathBuf::from("target/fun-cef-test"),
            message_loop_strategy: CefMessageLoopStrategy::MultiThreaded,
            remote_debugging_port: Some(9223),
            transparent_painting: true,
            bounded_shutdown_drain: Duration::ZERO,
        };

        assert_eq!(config.cef_settings().remote_debugging_port, 9223);
    }
}
