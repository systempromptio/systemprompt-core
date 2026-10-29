//! Conditional and ranged request headers of the content routes.
//!
//! A revision's `ETag` is its revision id, quoted (`"<revision_id>"`), on
//! every read and commit that answers one revision; its content hash rides
//! in `x-document-content-sha256`. A write names its base revision as
//! `If-Match: "<revision_id>"` (the `ETag` of the read it edits), and a value
//! that is not a quoted revision id is `400` with `error_key`
//! `invalid_if_match`. `If-None-Match` and `If-Range` on a revision read
//! compare against the same revision `ETag`. Representations that are not a
//! revision's own bytes carry the hash of what they serve as their `ETag`: a
//! drive blob (addressed by its hash) and a rendered PDF.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::{HeaderMap, HeaderName, HeaderValue, header};
use systemprompt_identifiers::{ContentHash, DocumentRevisionId};
use systemprompt_models::api::ApiError;
use systemprompt_traits::ByteRange;

use super::error::DocumentsResult;

const CONTENT_SHA256: &str = "x-document-content-sha256";

const INVALID_IF_MATCH: &str = "invalid_if_match";

pub(super) fn revision_etag(revision: &DocumentRevisionId) -> String {
    format!("\"{}\"", revision.as_str())
}

pub(super) fn hash_etag(hash: &ContentHash) -> String {
    format!("\"{}\"", hash.as_str())
}

pub(super) fn set_revision_tags(
    headers: &mut HeaderMap,
    revision: &DocumentRevisionId,
    hash: &ContentHash,
) {
    if let Ok(value) = HeaderValue::from_str(&revision_etag(revision)) {
        headers.insert(header::ETAG, value);
    }
    set_content_sha256(headers, hash);
}

pub(super) fn set_content_sha256(headers: &mut HeaderMap, hash: &ContentHash) {
    if let Ok(value) = HeaderValue::from_str(hash.as_str()) {
        headers.insert(HeaderName::from_static(CONTENT_SHA256), value);
    }
}

fn invalid_if_match() -> ApiError {
    ApiError::bad_request("If-Match must be a quoted revision id, the ETag of a document read")
        .with_error_key(INVALID_IF_MATCH)
}

pub(super) fn none_match(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(|tag| tag.trim().trim_start_matches("W/"))
        .any(|tag| tag == "*" || tag == etag)
}

pub(super) fn range(headers: &HeaderMap) -> Option<ByteRange> {
    headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(ByteRange::parse)
}

pub(super) fn if_range_holds(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get(header::IF_RANGE)
        .and_then(|value| value.to_str().ok())
        .is_none_or(|value| value.trim() == etag)
}

pub(crate) fn base_revision(headers: &HeaderMap) -> DocumentsResult<Option<DocumentRevisionId>> {
    let Some(value) = headers.get(header::IF_MATCH) else {
        return Ok(None);
    };
    let raw = value
        .to_str()
        .map_err(|source| invalid_if_match().with_source(source))?
        .trim();
    let unquoted = raw
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .ok_or_else(invalid_if_match)?;
    let revision = DocumentRevisionId::try_new(unquoted)
        .map_err(|source| invalid_if_match().with_source(source))?;
    Ok(Some(revision))
}

pub(super) fn content_disposition(name: &str) -> String {
    let fallback: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_graphic() && c != '"' && c != '\\' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!(
        "attachment; filename=\"{fallback}\"; filename*=UTF-8''{}",
        urlencoding::encode(name)
    )
}
