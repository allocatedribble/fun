use cef::{CefString, ImplSchemeRegistrar, SchemeOptions, SchemeRegistrar};

pub const FUN_UI_SCHEME: &str = "fun-ui";
pub const FUN_UI_HOST: &str = "main";
pub const FUN_UI_MAIN_PATH: &str = "/index.html";
pub const FUN_UI_APP_JS_PATH: &str = "/assets/app.js";
pub const FUN_UI_APP_CSS_PATH: &str = "/assets/app.css";
pub const FUN_UI_MAIN_URL: &str = "fun-ui://main/index.html";
pub const FUN_UI_APP_JS_URL: &str = "fun-ui://main/assets/app.js";
pub const FUN_UI_APP_CSS_URL: &str = "fun-ui://main/assets/app.css";
pub const FUN_CEF_UI_DEV_SERVER_ENV: &str = "FUN_CEF_UI_DEV_SERVER";

const INDEX_HTML_BYTES: &[u8] = include_bytes!("../../game_client/ui/main/index.html");
const APP_JS_BYTES: &[u8] = include_bytes!("../../game_client/ui/main/assets/app.js");
const APP_CSS_BYTES: &[u8] = include_bytes!("../../game_client/ui/main/assets/app.css");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum FunUiRoute {
    Main,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum FunUiAssetRoute {
    IndexHtml,
    AppJs,
    AppCss,
}

impl FunUiAssetRoute {
    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::IndexHtml => FUN_UI_MAIN_PATH,
            Self::AppJs => FUN_UI_APP_JS_PATH,
            Self::AppCss => FUN_UI_APP_CSS_PATH,
        }
    }

    #[must_use]
    pub const fn mime_type(self) -> &'static str {
        match self {
            Self::IndexHtml => "text/html; charset=utf-8",
            Self::AppJs => "text/javascript; charset=utf-8",
            Self::AppCss => "text/css; charset=utf-8",
        }
    }

    #[must_use]
    pub const fn bytes(self) -> &'static [u8] {
        match self {
            Self::IndexHtml => INDEX_HTML_BYTES,
            Self::AppJs => APP_JS_BYTES,
            Self::AppCss => APP_CSS_BYTES,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunUiAsset {
    pub route: FunUiAssetRoute,
    pub mime_type: &'static str,
    pub bytes: &'static [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[must_use]
pub fn fun_ui_route_url(route: FunUiRoute) -> &'static str {
    match route {
        FunUiRoute::Main => FUN_UI_MAIN_URL,
    }
}

#[must_use]
pub fn fun_ui_asset_url(route: FunUiAssetRoute) -> &'static str {
    match route {
        FunUiAssetRoute::IndexHtml => FUN_UI_MAIN_URL,
        FunUiAssetRoute::AppJs => FUN_UI_APP_JS_URL,
        FunUiAssetRoute::AppCss => FUN_UI_APP_CSS_URL,
    }
}

pub fn validate_fun_ui_url(url: &str) -> Result<FunUiRoute, FunUiUrlError> {
    match validate_fun_ui_asset_url(url)? {
        FunUiAssetRoute::IndexHtml => Ok(FunUiRoute::Main),
        FunUiAssetRoute::AppJs | FunUiAssetRoute::AppCss => Err(FunUiUrlError::UnknownRoute),
    }
}

pub fn validate_fun_ui_asset_url(url: &str) -> Result<FunUiAssetRoute, FunUiUrlError> {
    let route_path = normalized_fun_ui_path(url)?;
    match route_path {
        FUN_UI_MAIN_PATH | "/" | "" => Ok(FunUiAssetRoute::IndexHtml),
        FUN_UI_APP_JS_PATH => Ok(FunUiAssetRoute::AppJs),
        FUN_UI_APP_CSS_PATH => Ok(FunUiAssetRoute::AppCss),
        _ => Err(FunUiUrlError::UnknownRoute),
    }
}

pub fn resolve_fun_ui_asset(url: &str) -> Result<FunUiAsset, FunUiUrlError> {
    let route = validate_fun_ui_asset_url(url)?;
    Ok(FunUiAsset {
        route,
        mime_type: route.mime_type(),
        bytes: route.bytes(),
    })
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
pub fn fun_ui_scheme_options() -> i32 {
    SchemeOptions::STANDARD.get_raw()
        | SchemeOptions::LOCAL.get_raw()
        | SchemeOptions::SECURE.get_raw()
        | SchemeOptions::FETCH_ENABLED.get_raw()
}

pub fn register_fun_ui_custom_scheme(registrar: &mut SchemeRegistrar) -> bool {
    registrar.add_custom_scheme(
        Some(&CefString::from(FUN_UI_SCHEME)),
        fun_ui_scheme_options(),
    ) != 0
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
    fn resolves_locked_known_assets() {
        assert_eq!(
            resolve_fun_ui_asset("fun-ui://main/assets/app.js")
                .expect("app js")
                .route,
            FunUiAssetRoute::AppJs
        );
        assert_eq!(
            resolve_fun_ui_asset("fun-ui://main/assets/app.css")
                .expect("app css")
                .mime_type,
            "text/css; charset=utf-8"
        );
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
}
