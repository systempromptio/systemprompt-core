use axum::extract::State;
use axum::http::{header, StatusCode, Uri};
use axum::response::IntoResponse;
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

pub async fn serve_static_content(
    State(state): State<StaticContentState>,
    uri: Uri,
    req_ctx: Option<axum::Extension<systemprompt_models::RequestContext>>,
) -> impl IntoResponse {
    let matcher = state.matcher;
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
            let trimmed_path = path.trim_start_matches('/');
            dist_dir.join(trimmed_path)
        };

        if asset_path.exists() && asset_path.is_file() {
            match std::fs::read(&asset_path) {
                Ok(content) => {
                    let mime_type = match asset_path.extension().and_then(|ext| ext.to_str()) {
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
                    };

                    return (StatusCode::OK, [(header::CONTENT_TYPE, mime_type)], content)
                        .into_response();
                },
                Err(_) => {
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Error reading asset")
                        .into_response();
                },
            }
        }
        return (StatusCode::NOT_FOUND, "Asset not found").into_response();
    }

    if path == "/" {
        let index_path = dist_dir.join("index.html");
        if index_path.exists() {
            match std::fs::read(&index_path) {
                Ok(content) => {
                    return (
                        StatusCode::OK,
                        [(header::CONTENT_TYPE, "text/html")],
                        content,
                    )
                        .into_response();
                },
                Err(_) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Error reading index.html",
                    )
                        .into_response();
                },
            }
        }
        return (StatusCode::NOT_FOUND, "Homepage not found").into_response();
    }

    if path == "/sitemap.xml"
        || path == "/robots.txt"
        || path == "/llms.txt"
        || path == "/feed.xml"
    {
        let trimmed_path = path.trim_start_matches('/');
        let file_path = dist_dir.join(trimmed_path);
        if file_path.exists() {
            match std::fs::read(&file_path) {
                Ok(content) => {
                    let mime_type = if path == "/feed.xml" {
                        "application/rss+xml; charset=utf-8"
                    } else {
                        match file_path.extension().and_then(|ext| ext.to_str()) {
                            Some("xml") => "application/xml",
                            _ => "text/plain",
                        }
                    };
                    return (StatusCode::OK, [(header::CONTENT_TYPE, mime_type)], content)
                        .into_response();
                },
                Err(_) => {
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Error reading file")
                        .into_response();
                },
            }
        }
        return (StatusCode::NOT_FOUND, "File not found").into_response();
    }

    let trimmed_path = path.trim_start_matches('/');
    let parent_route_path = dist_dir.join(trimmed_path).join("index.html");
    if parent_route_path.exists() {
        match std::fs::read(&parent_route_path) {
            Ok(content) => {
                return (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "text/html")],
                    content,
                )
                    .into_response();
            },
            Err(_) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Error reading parent route",
                )
                    .into_response();
            },
        }
    }

    if let Some((slug, source_id)) = matcher.matches(path) {
        let exact_path = dist_dir.join(trimmed_path);
        if exact_path.exists() && exact_path.is_file() {
            return serve_html_with_analytics(
                &exact_path,
                &slug,
                &source_id,
                req_ctx.as_ref().map(|ext| ext.0.clone()),
            )
            .into_response();
        }

        let index_path = dist_dir.join(trimmed_path).join("index.html");
        if index_path.exists() {
            return serve_html_with_analytics(
                &index_path,
                &slug,
                &source_id,
                req_ctx.as_ref().map(|ext| ext.0.clone()),
            )
            .into_response();
        }

        let content_repo = match ContentRepository::new(state.ctx.db_pool()) {
            Ok(r) => r,
            Err(_) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    axum::response::Html("Database connection error"),
                )
                    .into_response();
            },
        };
        match content_repo.get_by_slug(&slug).await {
            Ok(Some(_)) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    axum::response::Html(format!(
                        r#"<!DOCTYPE html>
<html>
<head>
    <title>Content Not Prerendered</title>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <style>
        body {{ font-family: system-ui, sans-serif; max-width: 600px; margin: 100px auto; padding: 20px; }}
        h1 {{ color: #d32f2f; }}
        code {{ background: #f5f5f5; padding: 2px 6px; border-radius: 3px; }}
    </style>
</head>
<body>
    <h1>Content Not Prerendered</h1>
    <p>The content exists in the database but has not been prerendered to HTML.</p>
    <p>Route: <code>{}</code></p>
    <p>Slug: <code>{}</code></p>
    <p>Run the prerendering build step to generate static HTML.</p>
</body>
</html>"#,
                        path, slug
                    ))
                ).into_response();
            },
            Ok(None) => {
                return (
                    StatusCode::NOT_FOUND,
                    axum::response::Html(r#"<!DOCTYPE html>
<html>
<head>
    <title>404 Not Found</title>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <style>
        body { font-family: system-ui, sans-serif; max-width: 600px; margin: 100px auto; padding: 20px; }
        h1 { color: #333; }
        a { color: #1976d2; text-decoration: none; }
        a:hover { text-decoration: underline; }
    </style>
</head>
<body>
    <h1>404 - Page Not Found</h1>
    <p>The page you're looking for doesn't exist.</p>
    <p><a href="/">← Back to home</a></p>
</body>
</html>"#.to_string())
                ).into_response();
            },
            Err(e) => {
                tracing::error!(error = %e, "Database error checking content");
                return (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
                    .into_response();
            },
        }
    }

    (
        StatusCode::NOT_FOUND,
        axum::response::Html(r#"<!DOCTYPE html>
<html>
<head>
    <title>404 Not Found</title>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <style>
        body { font-family: system-ui, sans-serif; max-width: 600px; margin: 100px auto; padding: 20px; }
        h1 { color: #333; }
        a { color: #1976d2; text-decoration: none; }
        a:hover { text-decoration: underline; }
    </style>
</head>
<body>
    <h1>404 - Page Not Found</h1>
    <p>The page you're looking for doesn't exist.</p>
    <p><a href="/">← Back to home</a></p>
</body>
</html>"#)
    ).into_response()
}

fn serve_html_with_analytics(
    html_path: &std::path::Path,
    _slug: &str,
    _source_id: &str,
    _req_ctx: Option<systemprompt_models::RequestContext>,
) -> impl IntoResponse {
    let Ok(html_content) = std::fs::read(html_path) else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Error reading file").into_response();
    };

    let mut response = (StatusCode::OK, html_content).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        http::HeaderValue::from_static("text/html"),
    );

    response
}
