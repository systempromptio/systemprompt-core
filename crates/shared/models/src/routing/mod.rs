use crate::modules::ApiPaths;
use crate::ContentRouting;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventMetadata {
    pub event_type: &'static str,
    pub event_category: &'static str,
    pub log_module: &'static str,
}

impl EventMetadata {
    pub const HTML_CONTENT: Self = Self {
        event_type: "page_view",
        event_category: "content",
        log_module: "page_view",
    };

    pub const API_REQUEST: Self = Self {
        event_type: "http_request",
        event_category: "api",
        log_module: "http_request",
    };

    pub const STATIC_ASSET: Self = Self {
        event_type: "asset_request",
        event_category: "static",
        log_module: "asset_request",
    };

    pub const NOT_FOUND: Self = Self {
        event_type: "not_found",
        event_category: "error",
        log_module: "not_found",
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteType {
    HtmlContent { source: String },
    ApiEndpoint { category: ApiCategory },
    StaticAsset { asset_type: AssetType },
    NotFound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiCategory {
    Content,
    Core,
    Agents,
    OAuth,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetType {
    JavaScript,
    Stylesheet,
    Image,
    Font,
    SourceMap,
    Other,
}

pub struct RouteClassifier {
    content_routing: Option<Arc<dyn ContentRouting>>,
}

impl std::fmt::Debug for RouteClassifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RouteClassifier")
            .field("content_routing", &self.content_routing.is_some())
            .finish()
    }
}

impl RouteClassifier {
    pub fn new(content_routing: Option<Arc<dyn ContentRouting>>) -> Self {
        Self { content_routing }
    }

    pub fn classify(&self, path: &str, _method: &str) -> RouteType {
        if Self::is_static_asset_path(path) {
            return RouteType::StaticAsset {
                asset_type: Self::determine_asset_type(path),
            };
        }

        if path.starts_with(ApiPaths::API_BASE) {
            return RouteType::ApiEndpoint {
                category: Self::determine_api_category(path),
            };
        }

        if let Some(routing) = &self.content_routing {
            if routing.is_html_page(path) {
                return RouteType::HtmlContent {
                    source: routing.determine_source(path),
                };
            }
        } else if !Self::is_static_asset_path(path) && !path.starts_with(ApiPaths::API_BASE) {
            return RouteType::HtmlContent {
                source: "unknown".to_string(),
            };
        }

        RouteType::NotFound
    }

    pub fn should_track_analytics(&self, path: &str, method: &str) -> bool {
        if method == "OPTIONS" {
            return false;
        }

        match self.classify(path, method) {
            RouteType::HtmlContent { .. } => true,
            RouteType::ApiEndpoint { category } => {
                matches!(category, ApiCategory::Core | ApiCategory::Content)
            },
            RouteType::StaticAsset { .. } | RouteType::NotFound => false,
        }
    }

    pub fn is_html(&self, path: &str) -> bool {
        matches!(self.classify(path, "GET"), RouteType::HtmlContent { .. })
    }

    pub fn get_event_metadata(&self, path: &str, method: &str) -> EventMetadata {
        match self.classify(path, method) {
            RouteType::HtmlContent { .. } => EventMetadata::HTML_CONTENT,
            RouteType::ApiEndpoint { .. } => EventMetadata::API_REQUEST,
            RouteType::StaticAsset { .. } => EventMetadata::STATIC_ASSET,
            RouteType::NotFound => EventMetadata::NOT_FOUND,
        }
    }

    fn is_static_asset_path(path: &str) -> bool {
        if path.starts_with(ApiPaths::ASSETS_BASE)
            || path.starts_with(ApiPaths::WELLKNOWN_BASE)
            || path.starts_with(ApiPaths::GENERATED_BASE)
            || path.starts_with(ApiPaths::FILES_BASE)
        {
            return true;
        }

        matches!(
            Path::new(path).extension().and_then(|e| e.to_str()),
            Some(
                "js" | "css"
                    | "map"
                    | "ttf"
                    | "woff"
                    | "woff2"
                    | "otf"
                    | "png"
                    | "jpg"
                    | "jpeg"
                    | "svg"
                    | "ico"
                    | "webp"
            )
        ) || path == "/vite.svg"
            || path == "/favicon.ico"
    }

    fn determine_asset_type(path: &str) -> AssetType {
        match Path::new(path).extension().and_then(|e| e.to_str()) {
            Some("js") => AssetType::JavaScript,
            Some("css") => AssetType::Stylesheet,
            Some("png" | "jpg" | "jpeg" | "svg" | "ico" | "webp") => AssetType::Image,
            Some("ttf" | "woff" | "woff2" | "otf") => AssetType::Font,
            Some("map") => AssetType::SourceMap,
            _ => AssetType::Other,
        }
    }

    fn determine_api_category(path: &str) -> ApiCategory {
        if path.starts_with(ApiPaths::CONTENT_BASE) {
            ApiCategory::Content
        } else if path.starts_with(ApiPaths::CORE_BASE) {
            ApiCategory::Core
        } else if path.starts_with(ApiPaths::AGENTS_BASE) {
            ApiCategory::Agents
        } else if path.starts_with(ApiPaths::OAUTH_BASE) {
            ApiCategory::OAuth
        } else {
            ApiCategory::Other
        }
    }
}
