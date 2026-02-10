use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode, Uri};
use axum::response::IntoResponse;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use super::config::StaticContentMatcher;
use systemprompt_content::ContentRepository;
use systemprompt_files::FilesConfig;
use systemprompt_models::{AppPaths, RouteClassifier, RouteType};
use systemprompt_runtime::AppContext;

#[derive(Clone, Debug)]
pub struct StaticContentState {
    pub ctx: Arc<AppContext>,
    pub matcher: Arc<StaticContentMatcher>,
    pub route_classifier: Arc<RouteClassifier>,
}

pub const CACHE_STATIC_ASSET: &str = "public, max-age=86400";
pub const CACHE_HTML: &str = "no-cache";
pub const CACHE_METADATA: &str = "public, max-age=3600";

pub fn compute_etag(content: &[u8]) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    format!("\"{}\"", hasher.finish())
}

fn etag_matches(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        == Some(etag)
}

fn not_modified_response(etag: &str, cache_control: &'static str) -> axum::response::Response {
    (
        StatusCode::NOT_MODIFIED,
        [
            (header::ETAG, etag.to_string()),
            (header::CACHE_CONTROL, cache_control.to_string()),
        ],
    )
        .into_response()
}

fn serve_file_response(
    content: Vec<u8>,
    content_type: String,
    cache_control: &'static str,
    etag: String,
) -> axum::response::Response {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, cache_control.to_string()),
            (header::ETAG, etag),
        ],
        content,
    )
        .into_response()
}

async fn serve_cached_file(
    file_path: &std::path::Path,
    headers: &HeaderMap,
    content_type: &str,
    cache_control: &'static str,
) -> axum::response::Response {
    match tokio::fs::read(file_path).await {
        Ok(content) => {
            let etag = compute_etag(&content);
            if etag_matches(headers, &etag) {
                return not_modified_response(&etag, cache_control);
            }
            serve_file_response(content, content_type.to_string(), cache_control, etag)
        },
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Error reading file").into_response(),
    }
}

fn resolve_mime_type(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("js") => "application/javascript",
        Some("css") => "text/css",
        Some("woff" | "woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}

pub async fn serve_static_content(
    State(state): State<StaticContentState>,
    uri: Uri,
    headers: HeaderMap,
    _req_ctx: Option<axum::Extension<systemprompt_models::RequestContext>>,
) -> impl IntoResponse {
    let dist_dir = match AppPaths::get() {
        Ok(paths) => paths.web().dist().to_path_buf(),
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "AppPaths not initialized",
            )
                .into_response();
        },
    };

    let path = uri.path();

    if matches!(
        state.route_classifier.classify(path, "GET"),
        RouteType::StaticAsset { .. }
    ) {
        return serve_static_asset(path, &dist_dir, &headers).await;
    }

    if path == "/" {
        return serve_cached_file(
            &dist_dir.join("index.html"),
            &headers,
            "text/html",
            CACHE_HTML,
        )
        .await;
    }

    if matches!(
        path,
        "/sitemap.xml" | "/robots.txt" | "/llms.txt" | "/feed.xml"
    ) {
        return serve_metadata_file(path, &dist_dir, &headers).await;
    }

    let trimmed_path = path.trim_start_matches('/');
    let parent_route_path = dist_dir.join(trimmed_path).join("index.html");
    if parent_route_path.exists() {
        return serve_cached_file(&parent_route_path, &headers, "text/html", CACHE_HTML).await;
    }

    if let Some((slug, _source_id)) = state.matcher.matches(path) {
        let req = ContentPageRequest {
            path,
            trimmed_path,
            slug: &slug,
            dist_dir: &dist_dir,
            headers: &headers,
        };
        return serve_content_page(req, &state.ctx).await;
    }

    not_found_response()
}

async fn serve_static_asset(
    path: &str,
    dist_dir: &std::path::Path,
    headers: &HeaderMap,
) -> axum::response::Response {
    let files_config = match FilesConfig::get() {
        Ok(config) => config,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "FilesConfig not initialized",
            )
                .into_response();
        },
    };

    let files_prefix = format!("{}/", files_config.url_prefix());
    let asset_path = if let Some(relative_path) = path.strip_prefix(&files_prefix) {
        files_config.files().join(relative_path)
    } else {
        dist_dir.join(path.trim_start_matches('/'))
    };

    if asset_path.exists() && asset_path.is_file() {
        let mime_type = resolve_mime_type(&asset_path);
        return serve_cached_file(&asset_path, headers, mime_type, CACHE_STATIC_ASSET).await;
    }

    (StatusCode::NOT_FOUND, "Asset not found").into_response()
}

async fn serve_metadata_file(
    path: &str,
    dist_dir: &std::path::Path,
    headers: &HeaderMap,
) -> axum::response::Response {
    let trimmed_path = path.trim_start_matches('/');
    let file_path = dist_dir.join(trimmed_path);
    if !file_path.exists() {
        return (StatusCode::NOT_FOUND, "File not found").into_response();
    }

    let mime_type = if path == "/feed.xml" {
        "application/rss+xml; charset=utf-8"
    } else {
        match file_path.extension().and_then(|ext| ext.to_str()) {
            Some("xml") => "application/xml",
            _ => "text/plain",
        }
    };

    serve_cached_file(&file_path, headers, mime_type, CACHE_METADATA).await
}

struct ContentPageRequest<'a> {
    path: &'a str,
    trimmed_path: &'a str,
    slug: &'a str,
    dist_dir: &'a std::path::Path,
    headers: &'a HeaderMap,
}

async fn serve_content_page(
    req: ContentPageRequest<'_>,
    ctx: &AppContext,
) -> axum::response::Response {
    let exact_path = req.dist_dir.join(req.trimmed_path);
    if exact_path.exists() && exact_path.is_file() {
        return serve_cached_file(&exact_path, req.headers, "text/html", CACHE_HTML).await;
    }

    let index_path = req.dist_dir.join(req.trimmed_path).join("index.html");
    if index_path.exists() {
        return serve_cached_file(&index_path, req.headers, "text/html", CACHE_HTML).await;
    }

    let content_repo = match ContentRepository::new(ctx.db_pool()) {
        Ok(r) => r,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::response::Html("Database connection error"),
            )
                .into_response();
        },
    };

    match content_repo.get_by_slug(req.slug).await {
        Ok(Some(_)) => not_prerendered_response(req.path, req.slug),
        Ok(None) => not_found_response(),
        Err(e) => {
            tracing::error!(error = %e, "Database error checking content");
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
        },
    }
}

fn not_prerendered_response(path: &str, slug: &str) -> axum::response::Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        axum::response::Html(format!(
            concat!(
                "<!DOCTYPE html><html><head><title>Content Not Prerendered</title>",
                "<meta charset=\"utf-8\"><meta name=\"viewport\" ",
                "content=\"width=device-width, initial-scale=1\">",
                "</head><body><h1>Content Not Prerendered</h1>",
                "<p>Content exists but has not been prerendered to HTML.</p>",
                "<p>Route: <code>{}</code></p><p>Slug: <code>{}</code></p>",
                "</body></html>",
            ),
            path, slug
        )),
    )
        .into_response()
}

fn not_found_response() -> axum::response::Response {
    (
        StatusCode::NOT_FOUND,
        axum::response::Html(concat!(
            "<!DOCTYPE html><html><head><title>404 Not Found</title>",
            "<meta charset=\"utf-8\"><meta name=\"viewport\" ",
            "content=\"width=device-width, initial-scale=1\">",
            "</head><body><h1>404 - Page Not Found</h1>",
            "<p>The page you're looking for doesn't exist.</p>",
            "<p><a href=\"/\">Back to home</a></p></body></html>",
        )),
    )
        .into_response()
}
