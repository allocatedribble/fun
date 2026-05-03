use std::sync::{Arc, Mutex};

use cef::rc::Rc as _;
use cef::{
    Browser, Callback, CefString, Frame, ImplRequest, ImplResourceHandler, ImplResponse,
    ImplSchemeHandlerFactory, ImplSchemeRegistrar, Request, ResourceHandler, ResourceReadCallback,
    ResourceSkipCallback, Response, SchemeHandlerFactory, SchemeOptions, SchemeRegistrar,
    WrapResourceHandler, WrapSchemeHandlerFactory, register_scheme_handler_factory,
    wrap_resource_handler, wrap_scheme_handler_factory,
};

pub const FUN_UI_SCHEME: &str = "fun-ui";
pub const FUN_UI_HOST: &str = "main";
pub const FUN_UI_MAIN_PATH: &str = "/index.html";
pub const FUN_UI_MAIN_URL: &str = "fun-ui://main/index.html";
pub const FUN_CEF_UI_DEV_SERVER_ENV: &str = "FUN_CEF_UI_DEV_SERVER";
const FUN_UI_ASSET_CHARSET: &str = "utf-8";
const FUN_UI_CORS_ALLOW_ORIGIN: &str = "*";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum FunUiRoute {
    Main,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunUiAssetRoute {
    path: &'static str,
}

impl FunUiAssetRoute {
    #[must_use]
    pub const fn new(path: &'static str) -> Self {
        Self { path }
    }

    #[must_use]
    pub const fn path(self) -> &'static str {
        self.path
    }
}

pub const FUN_UI_INDEX_HTML_ROUTE: FunUiAssetRoute = FunUiAssetRoute::new(FUN_UI_MAIN_PATH);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneratedFunUiAsset {
    pub path: &'static str,
    pub mime_type: &'static str,
    pub content_type: &'static str,
    pub bytes: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/fun_ui_assets.rs"));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunUiAsset {
    pub route: FunUiAssetRoute,
    pub mime_type: &'static str,
    pub content_type: &'static str,
    pub bytes: &'static [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum FunUiUrlError {
    WrongScheme,
    WrongHost,
    UnknownRoute,
    TraversalSegment,
    EncodedTraversalSegment,
    HiddenPathSegment,
    BackslashSegment,
    AbsolutePathSegment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunUiDevServerUrl {
    url: String,
}

impl FunUiDevServerUrl {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.url
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunUiDevServerError {
    WrongScheme,
    NonLoopbackHost,
    MissingPort,
    InvalidPort,
    ContainsPath,
    ContainsCredentials,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum FunUiNavigationTarget {
    MainFrame,
    Popup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum FunUiNavigationBlockReason {
    Popup,
    FileScheme,
    ExternalNetwork,
    UnknownScheme,
    InvalidFunUiUrl,
    DevServerDisabled,
    DevServerOriginMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum FunUiNavigationDecision {
    Allow,
    Block { reason: FunUiNavigationBlockReason },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunUiNavigationPolicy {
    pub dev_server_url: Option<FunUiDevServerUrl>,
    pub popups_allowed: bool,
}

impl FunUiNavigationPolicy {
    #[must_use]
    pub const fn production() -> Self {
        Self {
            dev_server_url: None,
            popups_allowed: false,
        }
    }

    #[must_use]
    pub fn with_dev_server(dev_server_url: FunUiDevServerUrl) -> Self {
        Self {
            dev_server_url: Some(dev_server_url),
            popups_allowed: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunUiSchemeRequestOutcome {
    Served {
        route: FunUiAssetRoute,
        byte_len: u32,
    },
    Rejected {
        reason: FunUiUrlError,
    },
}

#[must_use]
pub fn fun_ui_route_url(route: FunUiRoute) -> &'static str {
    match route {
        FunUiRoute::Main => FUN_UI_MAIN_URL,
    }
}

#[must_use]
pub fn fun_ui_asset_url(route: FunUiAssetRoute) -> &'static str {
    route.path()
}

pub fn validate_fun_ui_url(url: &str) -> Result<FunUiRoute, FunUiUrlError> {
    match validate_fun_ui_asset_url(url)? {
        route if route.path() == FUN_UI_MAIN_PATH => Ok(FunUiRoute::Main),
        _ => Err(FunUiUrlError::UnknownRoute),
    }
}

pub fn validate_fun_ui_asset_url(url: &str) -> Result<FunUiAssetRoute, FunUiUrlError> {
    let route_path = normalized_fun_ui_asset_path(url)?;
    generated_fun_ui_asset(route_path)
        .map(|asset| asset.route)
        .ok_or(FunUiUrlError::UnknownRoute)
}

pub fn resolve_fun_ui_asset(url: &str) -> Result<FunUiAsset, FunUiUrlError> {
    let route_path = normalized_fun_ui_asset_path(url)?;
    generated_fun_ui_asset(route_path).ok_or(FunUiUrlError::UnknownRoute)
}

#[must_use]
pub const fn generated_fun_ui_assets() -> &'static [GeneratedFunUiAsset] {
    GENERATED_FUN_UI_ASSETS
}

fn generated_fun_ui_asset(path: &str) -> Option<FunUiAsset> {
    generated_fun_ui_assets()
        .iter()
        .find(|asset| asset.path == path)
        .map(|asset| FunUiAsset {
            route: FunUiAssetRoute::new(asset.path),
            mime_type: asset.mime_type,
            content_type: asset.content_type,
            bytes: asset.bytes,
        })
}

#[must_use]
pub fn classify_fun_ui_scheme_request(url: &str) -> FunUiSchemeRequestOutcome {
    match resolve_fun_ui_asset(url) {
        Ok(asset) => FunUiSchemeRequestOutcome::Served {
            route: asset.route,
            byte_len: asset.bytes.len().min(u32::MAX as usize) as u32,
        },
        Err(reason) => FunUiSchemeRequestOutcome::Rejected { reason },
    }
}

pub fn validate_fun_ui_dev_server_url(url: &str) -> Result<FunUiDevServerUrl, FunUiDevServerError> {
    let Some(rest) = url.strip_prefix("http://") else {
        return Err(FunUiDevServerError::WrongScheme);
    };
    if rest.contains('@') {
        return Err(FunUiDevServerError::ContainsCredentials);
    }
    let (authority, path) = rest
        .split_once('/')
        .map_or((rest, ""), |(authority, path)| (authority, path));
    if !path.is_empty() {
        return Err(FunUiDevServerError::ContainsPath);
    }

    let port = if let Some(port) = authority.strip_prefix("127.0.0.1:") {
        port
    } else if let Some(port) = authority.strip_prefix("localhost:") {
        port
    } else if let Some(port) = authority.strip_prefix("[::1]:") {
        port
    } else {
        return Err(FunUiDevServerError::NonLoopbackHost);
    };
    if port.is_empty() {
        return Err(FunUiDevServerError::MissingPort);
    }
    let port = port
        .parse::<u16>()
        .map_err(|_| FunUiDevServerError::InvalidPort)?;
    if port == 0 {
        return Err(FunUiDevServerError::InvalidPort);
    }

    Ok(FunUiDevServerUrl {
        url: url.trim_end_matches('/').to_owned(),
    })
}

pub fn fun_ui_dev_server_from_env() -> Result<Option<FunUiDevServerUrl>, FunUiDevServerError> {
    std::env::var(FUN_CEF_UI_DEV_SERVER_ENV)
        .ok()
        .filter(|value| !value.is_empty())
        .map(|value| validate_fun_ui_dev_server_url(&value))
        .transpose()
}

#[must_use]
pub fn validate_fun_ui_navigation(
    url: &str,
    target: FunUiNavigationTarget,
    policy: &FunUiNavigationPolicy,
) -> FunUiNavigationDecision {
    if matches!(target, FunUiNavigationTarget::Popup) && !policy.popups_allowed {
        return FunUiNavigationDecision::Block {
            reason: FunUiNavigationBlockReason::Popup,
        };
    }
    if url.starts_with("fun-ui://") {
        return if validate_fun_ui_asset_url(url).is_ok() {
            FunUiNavigationDecision::Allow
        } else {
            FunUiNavigationDecision::Block {
                reason: FunUiNavigationBlockReason::InvalidFunUiUrl,
            }
        };
    }
    if url.starts_with("file://") {
        return FunUiNavigationDecision::Block {
            reason: FunUiNavigationBlockReason::FileScheme,
        };
    }
    if url.starts_with("http://") {
        let Some(dev_server_url) = &policy.dev_server_url else {
            return FunUiNavigationDecision::Block {
                reason: FunUiNavigationBlockReason::DevServerDisabled,
            };
        };
        return if http_origin_matches(url, dev_server_url.as_str()) {
            FunUiNavigationDecision::Allow
        } else {
            FunUiNavigationDecision::Block {
                reason: FunUiNavigationBlockReason::DevServerOriginMismatch,
            }
        };
    }
    if url.starts_with("https://") {
        return FunUiNavigationDecision::Block {
            reason: FunUiNavigationBlockReason::ExternalNetwork,
        };
    }
    FunUiNavigationDecision::Block {
        reason: FunUiNavigationBlockReason::UnknownScheme,
    }
}

#[must_use]
pub fn fun_ui_scheme_options() -> i32 {
    SchemeOptions::STANDARD.get_raw()
        | SchemeOptions::LOCAL.get_raw()
        | SchemeOptions::SECURE.get_raw()
        | SchemeOptions::CORS_ENABLED.get_raw()
        | SchemeOptions::FETCH_ENABLED.get_raw()
}

pub fn register_fun_ui_custom_scheme(registrar: &mut SchemeRegistrar) -> bool {
    registrar.add_custom_scheme(
        Some(&CefString::from(FUN_UI_SCHEME)),
        fun_ui_scheme_options(),
    ) != 0
}

pub fn register_fun_ui_scheme_handler_factory() -> bool {
    let scheme_name = CefString::from(FUN_UI_SCHEME);
    let domain_name = CefString::from(FUN_UI_HOST);
    let mut factory = new_fun_ui_scheme_handler_factory();
    register_scheme_handler_factory(Some(&scheme_name), Some(&domain_name), Some(&mut factory)) != 0
}

#[must_use]
pub fn new_fun_ui_scheme_handler_factory() -> SchemeHandlerFactory {
    FunUiSchemeHandlerFactory::new()
}

wrap_scheme_handler_factory! {
    pub struct FunUiSchemeHandlerFactory;

    impl SchemeHandlerFactory {
        fn create(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _scheme_name: Option<&CefString>,
            request: Option<&mut Request>,
        ) -> Option<ResourceHandler> {
            let request = request?;
            let url = CefString::from(&request.url()).to_string();
            resolve_fun_ui_asset(&url)
                .ok()
                .map(new_fun_ui_resource_handler)
        }
    }
}

#[derive(Debug, Clone)]
struct FunUiResourceState {
    asset: FunUiAsset,
    cursor: usize,
}

wrap_resource_handler! {
    pub struct FunUiResourceHandler {
        state: Arc<Mutex<FunUiResourceState>>,
    }

    impl ResourceHandler {
        fn open(
            &self,
            _request: Option<&mut Request>,
            handle_request: Option<&mut i32>,
            _callback: Option<&mut Callback>,
        ) -> i32 {
            if let Some(handle_request) = handle_request {
                *handle_request = 1;
            }
            1
        }

        fn process_request(
            &self,
            _request: Option<&mut Request>,
            _callback: Option<&mut Callback>,
        ) -> i32 {
            1
        }

        fn response_headers(
            &self,
            response: Option<&mut Response>,
            response_length: Option<&mut i64>,
            _redirect_url: Option<&mut CefString>,
        ) {
            let Ok(state) = self.state.lock() else {
                return;
            };
            if let Some(response) = response {
                response.set_status(200);
                response.set_status_text(Some(&CefString::from("OK")));
                response.set_mime_type(Some(&CefString::from(state.asset.mime_type)));
                response.set_charset(Some(&CefString::from(FUN_UI_ASSET_CHARSET)));
                response.set_header_by_name(
                    Some(&CefString::from("Cache-Control")),
                    Some(&CefString::from("no-store")),
                    1,
                );
                response.set_header_by_name(
                    Some(&CefString::from("Content-Type")),
                    Some(&CefString::from(state.asset.content_type)),
                    1,
                );
                response.set_header_by_name(
                    Some(&CefString::from("Access-Control-Allow-Origin")),
                    Some(&CefString::from(FUN_UI_CORS_ALLOW_ORIGIN)),
                    1,
                );
                response.set_header_by_name(
                    Some(&CefString::from("Cross-Origin-Resource-Policy")),
                    Some(&CefString::from("same-origin")),
                    1,
                );
            }
            if let Some(response_length) = response_length {
                *response_length = state.asset.bytes.len().min(i64::MAX as usize) as i64;
            }
        }

        fn skip(
            &self,
            bytes_to_skip: i64,
            bytes_skipped: Option<&mut i64>,
            _callback: Option<&mut ResourceSkipCallback>,
        ) -> i32 {
            let Ok(mut state) = self.state.lock() else {
                return 0;
            };
            let skip = usize::try_from(bytes_to_skip.max(0)).unwrap_or(usize::MAX);
            let remaining = state.asset.bytes.len().saturating_sub(state.cursor);
            let skipped = skip.min(remaining);
            state.cursor = state.cursor.saturating_add(skipped);
            if let Some(bytes_skipped) = bytes_skipped {
                *bytes_skipped = skipped.min(i64::MAX as usize) as i64;
            }
            1
        }

        fn read(
            &self,
            data_out: *mut u8,
            bytes_to_read: i32,
            bytes_read: Option<&mut i32>,
            _callback: Option<&mut ResourceReadCallback>,
        ) -> i32 {
            read_fun_ui_resource(&self.state, data_out, bytes_to_read, bytes_read)
        }

        fn read_response(
            &self,
            data_out: *mut u8,
            bytes_to_read: i32,
            bytes_read: Option<&mut i32>,
            _callback: Option<&mut Callback>,
        ) -> i32 {
            read_fun_ui_resource(&self.state, data_out, bytes_to_read, bytes_read)
        }

        fn cancel(&self) {}
    }
}

fn new_fun_ui_resource_handler(asset: FunUiAsset) -> ResourceHandler {
    FunUiResourceHandler::new(Arc::new(Mutex::new(FunUiResourceState {
        asset,
        cursor: 0,
    })))
}

fn read_fun_ui_resource(
    state: &Arc<Mutex<FunUiResourceState>>,
    data_out: *mut u8,
    bytes_to_read: i32,
    bytes_read: Option<&mut i32>,
) -> i32 {
    if data_out.is_null() || bytes_to_read <= 0 {
        if let Some(bytes_read) = bytes_read {
            *bytes_read = 0;
        }
        return 0;
    }
    let Ok(mut state) = state.lock() else {
        return 0;
    };
    let remaining = state.asset.bytes.len().saturating_sub(state.cursor);
    if remaining == 0 {
        if let Some(bytes_read) = bytes_read {
            *bytes_read = 0;
        }
        return 0;
    }
    let requested = usize::try_from(bytes_to_read).unwrap_or(0);
    let count = requested.min(remaining);
    let start = state.cursor;
    let end = start.saturating_add(count);
    // CEF provides `data_out` as a writable buffer of at least `bytes_to_read`
    // bytes for the duration of this callback.
    unsafe {
        std::ptr::copy_nonoverlapping(state.asset.bytes[start..end].as_ptr(), data_out, count);
    }
    state.cursor = end;
    if let Some(bytes_read) = bytes_read {
        *bytes_read = count.min(i32::MAX as usize) as i32;
    }
    1
}

fn normalized_fun_ui_path(url: &str) -> Result<&str, FunUiUrlError> {
    let Some(rest) = url.strip_prefix("fun-ui://") else {
        return Err(FunUiUrlError::WrongScheme);
    };
    let Some(path) = rest.strip_prefix("main") else {
        return Err(FunUiUrlError::WrongHost);
    };
    if path.starts_with(':') {
        return Err(FunUiUrlError::WrongHost);
    }
    if path.contains('\\') {
        return Err(FunUiUrlError::BackslashSegment);
    }
    if contains_path_traversal(path) {
        return Err(FunUiUrlError::TraversalSegment);
    }
    if contains_encoded_traversal(path) {
        return Err(FunUiUrlError::EncodedTraversalSegment);
    }
    let without_query = path
        .split_once('?')
        .map_or(path, |(without_query, _)| without_query);
    let route_path = without_query
        .split_once('#')
        .map_or(without_query, |(without_hash, _)| without_hash);
    validate_route_segments(route_path)?;
    Ok(route_path)
}

fn normalized_fun_ui_asset_path(url: &str) -> Result<&str, FunUiUrlError> {
    match normalized_fun_ui_path(url)? {
        "" | "/" => Ok(FUN_UI_MAIN_PATH),
        path => Ok(path),
    }
}

fn validate_route_segments(path: &str) -> Result<(), FunUiUrlError> {
    for segment in path.split('/').filter(|segment| !segment.is_empty()) {
        if segment.starts_with('.') {
            return Err(FunUiUrlError::HiddenPathSegment);
        }
        if segment.contains(':') {
            return Err(FunUiUrlError::AbsolutePathSegment);
        }
    }
    Ok(())
}

fn contains_path_traversal(path: &str) -> bool {
    path.split('/').any(|segment| segment == "..")
}

fn contains_encoded_traversal(path: &str) -> bool {
    path.as_bytes()
        .windows(3)
        .any(|window| matches!(window, b"%2e" | b"%2E" | b"%2f" | b"%2F" | b"%5c" | b"%5C"))
}

fn http_origin_matches(url: &str, allowed_origin: &str) -> bool {
    let Some(url_origin) = http_origin(url) else {
        return false;
    };
    let Some(allowed_origin) = http_origin(allowed_origin) else {
        return false;
    };
    url_origin == allowed_origin
}

fn http_origin(url: &str) -> Option<&str> {
    let rest = url.strip_prefix("http://")?;
    if rest.contains('@') {
        return None;
    }
    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .filter(|authority| !authority.is_empty())?;
    if authority.starts_with("127.0.0.1:")
        || authority.starts_with("localhost:")
        || authority.starts_with("[::1]:")
    {
        Some(authority)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_main_page_with_state_fragments() {
        assert_eq!(
            validate_fun_ui_url("fun-ui://main/index.html#scoreboard"),
            Ok(FunUiRoute::Main)
        );
        assert_eq!(
            validate_fun_ui_url("fun-ui://main/index.html?route=chat"),
            Ok(FunUiRoute::Main)
        );
    }

    #[test]
    fn resolves_vite_build_assets() {
        let js_asset = generated_fun_ui_assets()
            .iter()
            .find(|asset| asset.path.starts_with("/assets/") && asset.path.ends_with(".js"))
            .expect("vite js asset");
        let css_asset = generated_fun_ui_assets()
            .iter()
            .find(|asset| asset.path.starts_with("/assets/") && asset.path.ends_with(".css"))
            .expect("vite css asset");
        assert_eq!(
            resolve_fun_ui_asset(&format!("fun-ui://main{}", js_asset.path))
                .expect("vite js")
                .route,
            FunUiAssetRoute::new(js_asset.path)
        );
        assert_eq!(
            resolve_fun_ui_asset(&format!("fun-ui://main{}", css_asset.path))
                .expect("vite css")
                .mime_type,
            "text/css"
        );
        assert_eq!(
            resolve_fun_ui_asset("fun-ui://main/index.html")
                .expect("index html")
                .mime_type,
            "text/html",
            "CEF Response::set_mime_type expects only the MIME token"
        );
        assert_eq!(
            resolve_fun_ui_asset("fun-ui://main/")
                .expect("main slash")
                .content_type,
            "text/html; charset=utf-8"
        );
    }

    #[test]
    fn fun_ui_scheme_is_cors_enabled_for_vite_module_assets() {
        let options = fun_ui_scheme_options();
        assert_ne!(options & SchemeOptions::STANDARD.get_raw(), 0);
        assert_ne!(options & SchemeOptions::SECURE.get_raw(), 0);
        assert_ne!(options & SchemeOptions::CORS_ENABLED.get_raw(), 0);
        assert_ne!(options & SchemeOptions::FETCH_ENABLED.get_raw(), 0);
    }

    #[test]
    fn rejects_external_or_traversal_urls() {
        assert_eq!(
            validate_fun_ui_url("https://main/index.html"),
            Err(FunUiUrlError::WrongScheme)
        );
        assert_eq!(
            validate_fun_ui_url("fun-ui://main/../secrets"),
            Err(FunUiUrlError::TraversalSegment)
        );
        assert_eq!(
            validate_fun_ui_url("fun-ui://main/%2e%2e/secrets"),
            Err(FunUiUrlError::EncodedTraversalSegment)
        );
        assert_eq!(
            validate_fun_ui_url("fun-ui://main/.hidden"),
            Err(FunUiUrlError::HiddenPathSegment)
        );
        assert_eq!(
            validate_fun_ui_url("fun-ui://main/C:/secret"),
            Err(FunUiUrlError::AbsolutePathSegment)
        );
    }

    #[test]
    fn dev_server_accepts_only_loopback_http_roots() {
        assert!(validate_fun_ui_dev_server_url("http://127.0.0.1:5173").is_ok());
        assert!(validate_fun_ui_dev_server_url("http://localhost:5173").is_ok());
        assert!(validate_fun_ui_dev_server_url("http://[::1]:5173").is_ok());
        assert_eq!(
            validate_fun_ui_dev_server_url("https://127.0.0.1:5173"),
            Err(FunUiDevServerError::WrongScheme)
        );
        assert_eq!(
            validate_fun_ui_dev_server_url("http://192.168.1.10:5173"),
            Err(FunUiDevServerError::NonLoopbackHost)
        );
        assert_eq!(
            validate_fun_ui_dev_server_url("http://127.0.0.1:5173/app"),
            Err(FunUiDevServerError::ContainsPath)
        );
    }

    #[test]
    fn navigation_policy_blocks_external_file_and_popups() {
        let policy = FunUiNavigationPolicy::production();

        assert_eq!(
            validate_fun_ui_navigation(
                "fun-ui://main/index.html",
                FunUiNavigationTarget::MainFrame,
                &policy
            ),
            FunUiNavigationDecision::Allow
        );
        assert_eq!(
            validate_fun_ui_navigation(
                "file:///C:/Users/premi/secrets.txt",
                FunUiNavigationTarget::MainFrame,
                &policy
            ),
            FunUiNavigationDecision::Block {
                reason: FunUiNavigationBlockReason::FileScheme
            }
        );
        assert_eq!(
            validate_fun_ui_navigation(
                "https://example.com",
                FunUiNavigationTarget::MainFrame,
                &policy
            ),
            FunUiNavigationDecision::Block {
                reason: FunUiNavigationBlockReason::ExternalNetwork
            }
        );
        assert_eq!(
            validate_fun_ui_navigation(
                "fun-ui://main/index.html",
                FunUiNavigationTarget::Popup,
                &policy
            ),
            FunUiNavigationDecision::Block {
                reason: FunUiNavigationBlockReason::Popup
            }
        );
    }

    #[test]
    fn navigation_policy_allows_only_configured_loopback_dev_origin() {
        let dev_server =
            validate_fun_ui_dev_server_url("http://127.0.0.1:5173").expect("dev server");
        let policy = FunUiNavigationPolicy::with_dev_server(dev_server);

        assert_eq!(
            validate_fun_ui_navigation(
                "http://127.0.0.1:5173/src/main.ts",
                FunUiNavigationTarget::MainFrame,
                &policy
            ),
            FunUiNavigationDecision::Allow
        );
        assert_eq!(
            validate_fun_ui_navigation(
                "http://localhost:5173/src/main.ts",
                FunUiNavigationTarget::MainFrame,
                &policy
            ),
            FunUiNavigationDecision::Block {
                reason: FunUiNavigationBlockReason::DevServerOriginMismatch
            }
        );
        assert_eq!(
            validate_fun_ui_navigation(
                "http://127.0.0.1:3000/src/main.ts",
                FunUiNavigationTarget::MainFrame,
                &policy
            ),
            FunUiNavigationDecision::Block {
                reason: FunUiNavigationBlockReason::DevServerOriginMismatch
            }
        );
    }

    #[test]
    fn classifies_scheme_requests_without_serving_unknown_paths() {
        let css_asset = generated_fun_ui_assets()
            .iter()
            .find(|asset| asset.path.starts_with("/assets/") && asset.path.ends_with(".css"))
            .expect("vite css asset");
        assert!(matches!(
            classify_fun_ui_scheme_request(&format!("fun-ui://main{}", css_asset.path)),
            FunUiSchemeRequestOutcome::Served {
                route,
                ..
            } if route.path() == css_asset.path
        ));
        assert_eq!(
            classify_fun_ui_scheme_request("fun-ui://main/.env"),
            FunUiSchemeRequestOutcome::Rejected {
                reason: FunUiUrlError::HiddenPathSegment
            }
        );
    }
}
