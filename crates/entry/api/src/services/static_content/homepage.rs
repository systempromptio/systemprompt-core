use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use systemprompt_models::AppPaths;

use super::static_files::{compute_etag, CACHE_HTML};

pub async fn serve_homepage(headers: http::HeaderMap) -> impl IntoResponse {
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

    let homepage_path = dist_dir.join("index.html");

    if homepage_path.exists() {
        match tokio::fs::read(&homepage_path).await {
            Ok(content) => {
                let etag = compute_etag(&content);
                if let Some(client_etag) = headers
                    .get(header::IF_NONE_MATCH)
                    .and_then(|v| v.to_str().ok())
                {
                    if client_etag == etag {
                        return (
                            StatusCode::NOT_MODIFIED,
                            [
                                (header::ETAG, etag),
                                (header::CACHE_CONTROL, CACHE_HTML.to_string()),
                            ],
                        )
                            .into_response();
                    }
                }

                return (
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, "text/html; charset=utf-8".to_string()),
                        (header::CACHE_CONTROL, CACHE_HTML.to_string()),
                        (header::ETAG, etag),
                    ],
                    content,
                )
                    .into_response();
            },
            Err(e) => {
                tracing::error!(error = %e, "Failed to read homepage");
                return (StatusCode::INTERNAL_SERVER_ERROR, "Error reading homepage")
                    .into_response();
            },
        }
    }

    (
        StatusCode::NOT_FOUND,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8".to_string())],
        "Homepage not found - index.html missing from web distribution".to_string(),
    )
        .into_response()
}
