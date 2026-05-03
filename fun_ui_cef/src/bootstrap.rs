use std::sync::Arc;

use cef::rc::Rc;
use cef::wrapper::message_router::{
    MessageRouterConfig, MessageRouterRendererSide, MessageRouterRendererSideHandlerCallbacks,
    RendererSideRouter,
};
use cef::{
    App, Browser, CefString, CommandLine, Frame, ImplApp, ImplCommandLine,
    ImplRenderProcessHandler, ProcessId, ProcessMessage, RenderProcessHandler, SchemeRegistrar,
    V8Context, WrapApp, WrapRenderProcessHandler, args::Args, wrap_app,
    wrap_render_process_handler,
};

use crate::runtime::configure_cef_api_version;
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
    pub struct FunCefApp {
        render_process_handler: RenderProcessHandler,
    }

    impl App {
        fn render_process_handler(&self) -> Option<RenderProcessHandler> {
            Some(self.render_process_handler.clone())
        }

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

wrap_render_process_handler! {
    struct FunCefRenderProcessHandler {
        router: Arc<RendererSideRouter>,
    }

    impl RenderProcessHandler {
        fn on_context_created(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            context: Option<&mut V8Context>,
        ) {
            self.router.on_context_created(
                browser.map(|browser| browser.clone()),
                frame.map(|frame| frame.clone()),
                context.map(|context| context.clone()),
            );
        }

        fn on_context_released(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            context: Option<&mut V8Context>,
        ) {
            self.router.on_context_released(
                browser.map(|browser| browser.clone()),
                frame.map(|frame| frame.clone()),
                context.map(|context| context.clone()),
            );
        }

        fn on_process_message_received(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            source_process: ProcessId,
            message: Option<&mut ProcessMessage>,
        ) -> std::os::raw::c_int {
            i32::from(self.router.on_process_message_received(
                browser.map(|browser| browser.clone()),
                frame.map(|frame| frame.clone()),
                Some(source_process),
                message.map(|message| message.clone()),
            ))
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
    configure_cef_api_version();
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
    FunCefApp::new(FunCefRenderProcessHandler::new(RendererSideRouter::new(
        MessageRouterConfig::default(),
    )))
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
