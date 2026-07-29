//! Cache-control constants and `ETag` computation/matching for static file
//! responses.
//!
//! Assets are classified by URL shape, not by how they were built, so
//! `/css/content.css` and `/css/content.4f3a9c1e.css` reach the same handler.
//! Only a name carrying a content hash may be answered with `immutable`: a
//! client that believes that promise will not revalidate for the lifetime of
//! the `max-age`, leaving the `ETag` computed here unreachable.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse;
use std::hash::{Hash, Hasher};
use std::path::Path;

pub const CACHE_STATIC_ASSET: &str = "public, max-age=31536000, immutable";
pub const CACHE_STATIC_ASSET_REVALIDATE: &str = "public, max-age=0, must-revalidate";
pub const CACHE_HTML: &str = "no-cache";
pub const CACHE_METADATA: &str = "public, max-age=3600";

const CONTENT_HASH_MIN_LEN: usize = 8;
const CONTENT_HASH_MAX_LEN: usize = 32;

/// The hash predicate is deliberately conservative: a missed hash costs one
/// conditional request, whereas a false positive pins a mutable URL in client
/// and CDN caches for a year.
pub fn asset_cache_policy(path: &Path) -> &'static str {
    let hashed = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(has_content_hash_segment);

    if hashed {
        CACHE_STATIC_ASSET
    } else {
        CACHE_STATIC_ASSET_REVALIDATE
    }
}

fn has_content_hash_segment(file_name: &str) -> bool {
    let segments: Vec<&str> = file_name.split(['.', '-']).collect();
    let Some(interior) = segments.get(1..segments.len().saturating_sub(1)) else {
        return false;
    };
    interior.iter().copied().any(is_content_hash)
}

fn is_content_hash(segment: &str) -> bool {
    if !(CONTENT_HASH_MIN_LEN..=CONTENT_HASH_MAX_LEN).contains(&segment.len())
        || !segment.chars().all(|c| c.is_ascii_alphanumeric())
    {
        return false;
    }
    let all_hex = segment.chars().all(|c| c.is_ascii_hexdigit());
    let mixed = segment.chars().any(|c| c.is_ascii_digit())
        && segment.chars().any(|c| c.is_ascii_alphabetic());
    all_hex || mixed
}

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

pub(super) fn not_modified_response(etag: &str, cache_control: &str) -> axum::response::Response {
    (
        StatusCode::NOT_MODIFIED,
        [
            (header::ETAG, etag.to_owned()),
            (header::CACHE_CONTROL, cache_control.to_owned()),
        ],
    )
        .into_response()
}

fn serve_file_response(
    content: Vec<u8>,
    content_type: String,
    cache_control: &str,
    etag: String,
) -> axum::response::Response {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, cache_control.to_owned()),
            (header::ETAG, etag),
        ],
        content,
    )
        .into_response()
}

pub(super) async fn serve_cached_file(
    file_path: &Path,
    headers: &HeaderMap,
    content_type: &str,
    cache_control: &str,
) -> axum::response::Response {
    match tokio::fs::read(file_path).await {
        Ok(content) => {
            let etag = compute_etag(&content);
            if etag_matches(headers, &etag) {
                return not_modified_response(&etag, cache_control);
            }
            serve_file_response(content, content_type.to_owned(), cache_control, etag)
        },
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Error reading file").into_response(),
    }
}
