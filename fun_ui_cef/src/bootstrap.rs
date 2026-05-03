use cef::rc::Rc;
use cef::{
    App, CefString, CommandLine, ImplApp, ImplCommandLine, SchemeRegistrar, WrapApp, args::Args,
    wrap_app,
};

use crate::scheme::register_fun_ui_custom_scheme;

const DISABLED_BROWSER_SWITCHES: &[&str] = &[
    "disable-background-networking",
    "disable-component-update",
    "disable-domain-reliability",
    "disable-features",
    "disable-sync",
    "metrics-recording-only",
    "no-first-run",
];
const DISABLED_BROWSER_FEATURES: &str =
    "AutofillServerCommunication,MediaRouter,OptimizationHints,Translate";

wrap_app! {
    pub struct FunCefApp;

    impl App {
        fn on_before_command_line_processing(
            &self,
            _process_type: Option<&CefString>,
            command_line: Option<&mut CommandLine>,
        ) {
            if let Some(command_line) = command_line {
                apply_default_command_line_policy(command_line);
            }
        }

        fn on_register_custom_schemes(&self, registrar: Option<&mut SchemeRegistrar>) {
            if let Some(registrar) = registrar {
                register_fun_ui_custom_scheme(registrar);
            }
        }
    }
}

/// Result of CEF's early subprocess escape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CefSubprocessExit {
    BrowserProcess,
    SubprocessHandled { exit_code: i32 },
}

impl CefSubprocessExit {
    #[must_use]
    pub const fn handled_exit_code(self) -> Option<i32> {
        match self {
            Self::BrowserProcess => None,
            Self::SubprocessHandled { exit_code } => Some(exit_code),
        }
    }
}

/// Run CEF's subprocess dispatcher before the Bevy app is constructed.
#[must_use]
pub fn maybe_execute_cef_subprocess() -> CefSubprocessExit {
    let args = Args::new();
    maybe_execute_cef_subprocess_with_args(args.as_main_args())
}

#[must_use]
pub fn maybe_execute_cef_subprocess_with_args(args: &cef::MainArgs) -> CefSubprocessExit {
    let mut app = new_fun_cef_app();
    let exit_code = cef::execute_process(Some(args), Some(&mut app), std::ptr::null_mut());
    if exit_code >= 0 {
        CefSubprocessExit::SubprocessHandled { exit_code }
    } else {
        CefSubprocessExit::BrowserProcess
    }
}

#[must_use]
pub fn new_fun_cef_app() -> cef::App {
    FunCefApp::new()
}

pub fn apply_default_command_line_policy(command_line: &mut CommandLine) {
    for switch in DISABLED_BROWSER_SWITCHES {
        if *switch == "disable-features" {
            command_line.append_switch_with_value(
                Some(&CefString::from(*switch)),
                Some(&CefString::from(DISABLED_BROWSER_FEATURES)),
            );
        } else {
            command_line.append_switch(Some(&CefString::from(*switch)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_policy_declares_disabled_features() {
        assert!(DISABLED_BROWSER_SWITCHES.contains(&"disable-background-networking"));
        assert!(DISABLED_BROWSER_FEATURES.contains("MediaRouter"));
    }
}
