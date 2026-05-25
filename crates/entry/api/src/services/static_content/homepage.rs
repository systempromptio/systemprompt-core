use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;

use super::static_files::{CACHE_HTML, StaticContentState, compute_etag};

pub async fn serve_homepage(
    State(state): State<StaticContentState>,
    headers: http::HeaderMap,
) -> impl IntoResponse {
    let dist_dir = state.ctx.app_paths().web().dist().to_path_buf();

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
                                (header::CACHE_CONTROL, CACHE_HTML.to_owned()),
                            ],
                        )
                            .into_response();
                    }
                }

                return (
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, "text/html; charset=utf-8".to_owned()),
                        (header::CACHE_CONTROL, CACHE_HTML.to_owned()),
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
        [(header::CONTENT_TYPE, "text/html; charset=utf-8".to_owned())],
        "Homepage not found - index.html missing from web distribution".to_owned(),
    )
        .into_response()
}
