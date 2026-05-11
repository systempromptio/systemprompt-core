mod cache;
mod responses;

pub use cache::{CACHE_HTML, CACHE_METADATA, CACHE_STATIC_ASSET, compute_etag};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::IntoResponse;
use std::sync::Arc;

use super::config::StaticContentMatcher;
use cache::{resolve_mime_type, serve_cached_file};
use responses::{not_found_response, not_prerendered_response};
use systemprompt_content::ContentRepository;
use systemprompt_files::FilesConfig;
use systemprompt_identifiers::{LocaleCode, SourceId};
use systemprompt_models::{RouteClassifier, RouteType};
use systemprompt_runtime::AppContext;

#[derive(Clone, Debug)]
pub struct StaticContentState {
    pub ctx: Arc<AppContext>,
    pub matcher: Arc<StaticContentMatcher>,
    pub route_classifier: Arc<RouteClassifier>,
}

pub async fn serve_static_content(
    State(state): State<StaticContentState>,
    uri: Uri,
    headers: HeaderMap,
    _req_ctx: Option<axum::Extension<systemprompt_models::RequestContext>>,
) -> impl IntoResponse {
    let dist_dir = state.ctx.app_paths().web().dist().to_path_buf();

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

    if let Some((slug, source_id)) = state.matcher.matches(path) {
        let req = ContentPageRequest {
            path,
            trimmed_path,
            slug: &slug,
            source_id: &source_id,
            dist_dir: &dist_dir,
            headers: &headers,
        };
        return serve_content_page(req, &state.ctx).await;
    }

    not_found_response(&dist_dir, &headers).await
}

async fn serve_static_asset(
    path: &str,
    dist_dir: &std::path::Path,
    headers: &HeaderMap,
) -> axum::response::Response {
    let Ok(files_config) = FilesConfig::get() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "FilesConfig not initialized",
        )
            .into_response();
    };

    let files_prefix = format!("{}/", files_config.url_prefix());
    let asset_path = path.strip_prefix(&files_prefix).map_or_else(
        || dist_dir.join(path.trim_start_matches('/')),
        |relative_path| files_config.files().join(relative_path),
    );

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
    source_id: &'a str,
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

    let Ok(content_repo) = ContentRepository::new(ctx.db_pool()) else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            axum::response::Html("Database connection error"),
        )
            .into_response();
    };

    let source_id = SourceId::new(req.source_id);
    match content_repo
        .get_by_source_and_slug(&source_id, req.slug, &LocaleCode::new("en"))
        .await
    {
        Ok(Some(_)) => not_prerendered_response(req.path, req.slug),
        Ok(None) => not_found_response(req.dist_dir, req.headers).await,
        Err(e) => {
            tracing::error!(error = %e, "Database error checking content");
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
        },
    }
}
