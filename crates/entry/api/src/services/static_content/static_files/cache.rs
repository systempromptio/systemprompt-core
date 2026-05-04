use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse;
use std::hash::{Hash, Hasher};

pub const CACHE_STATIC_ASSET: &str = "public, max-age=31536000, immutable";
pub const CACHE_HTML: &str = "no-cache";
pub const CACHE_METADATA: &str = "public, max-age=3600";

pub fn compute_etag(content: &[u8]) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    format!("\"{}\"", hasher.finish())
}

pub(super) fn etag_matches(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        == Some(etag)
}

pub(super) fn not_modified_response(
    etag: &str,
    cache_control: &'static str,
) -> axum::response::Response {
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

pub(super) async fn serve_cached_file(
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

pub(super) fn resolve_mime_type(path: &std::path::Path) -> &'static str {
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
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    }
}
