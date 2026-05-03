pub const FUN_UI_SCHEME: &str = "fun-ui";
pub const FUN_UI_HOST: &str = "main";
pub const FUN_UI_MAIN_PATH: &str = "/index.html";
pub const FUN_UI_MAIN_URL: &str = "fun-ui://main/index.html";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum FunUiRoute {
    Main,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunUiUrlError {
    WrongScheme,
    WrongHost,
    UnknownRoute,
    TraversalSegment,
    EncodedTraversalSegment,
}

#[must_use]
pub fn fun_ui_route_url(route: FunUiRoute) -> &'static str {
    match route {
        FunUiRoute::Main => FUN_UI_MAIN_URL,
    }
}

pub fn validate_fun_ui_url(url: &str) -> Result<FunUiRoute, FunUiUrlError> {
    let Some(rest) = url.strip_prefix("fun-ui://") else {
        return Err(FunUiUrlError::WrongScheme);
    };
    let Some(path) = rest.strip_prefix("main") else {
        return Err(FunUiUrlError::WrongHost);
    };
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
    match route_path {
        FUN_UI_MAIN_PATH | "/" | "" => Ok(FunUiRoute::Main),
        _ => Err(FunUiUrlError::UnknownRoute),
    }
}

fn contains_path_traversal(path: &str) -> bool {
    path.split('/').any(|segment| segment == "..")
}

fn contains_encoded_traversal(path: &str) -> bool {
    path.as_bytes()
        .windows(3)
        .any(|window| matches!(window, b"%2e" | b"%2E"))
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
    }
}
