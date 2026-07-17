//! Static-file response construction (headers, ranges).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse;

use super::cache::{CACHE_HTML, compute_etag, etag_matches, not_modified_response};

pub(super) fn not_prerendered_response(path: &str, slug: &str) -> axum::response::Response {
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

pub(super) async fn not_found_response(
    dist_dir: &std::path::Path,
    headers: &HeaderMap,
) -> axum::response::Response {
    let custom_404 = dist_dir.join("404.html");
    if custom_404.exists()
        && let Ok(content) = tokio::fs::read(&custom_404).await
    {
        let etag = compute_etag(&content);
        if etag_matches(headers, &etag) {
            return not_modified_response(&etag, CACHE_HTML);
        }
        return (
            StatusCode::NOT_FOUND,
            [
                (header::CONTENT_TYPE, "text/html".to_owned()),
                (header::CACHE_CONTROL, CACHE_HTML.to_owned()),
                (header::ETAG, etag),
            ],
            content,
        )
            .into_response();
    }

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
