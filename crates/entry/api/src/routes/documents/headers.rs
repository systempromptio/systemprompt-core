//! Conditional and ranged request headers of the content routes.
//!
//! A revision's `ETag` is its content hash, quoted; a write names its base
//! revision as `If-Match: "<revision_id>"`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::{HeaderMap, header};
use systemprompt_identifiers::{ContentHash, DocumentRevisionId};
use systemprompt_models::api::ApiError;
use systemprompt_traits::ByteRange;

use super::error::DocumentsResult;

pub(super) fn etag(hash: &ContentHash) -> String {
    format!("\"{}\"", hash.as_str())
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
        .map_err(|source| {
            ApiError::bad_request("If-Match must be a quoted revision id").with_source(source)
        })?
        .trim();
    let unquoted = raw
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .ok_or_else(|| ApiError::bad_request("If-Match must be a quoted revision id"))?;
    Ok(Some(DocumentRevisionId::try_new(unquoted)?))
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
