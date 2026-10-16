//! Turning an object-store media response into a [`StoredFileRead`].
//!
//! Cloud Storage and S3 answer a ranged `GET` the same way: `206` with
//! `Content-Range: bytes a-b/size`, `200` when the range was ignored, and
//! `416` with `Content-Range: bytes */size` when it cannot be satisfied.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use chrono::{DateTime, Utc};
use futures::TryStreamExt;
use reqwest::header::{
    CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, ETAG, HeaderMap, LAST_MODIFIED,
};
use reqwest::{Response, StatusCode};
use systemprompt_traits::{
    FileStorageError, ServedRange, StoredFileId, StoredFileMetadata, StoredFileRead,
};

use crate::object_id::mime_for;

fn header(headers: &HeaderMap, name: reqwest::header::HeaderName) -> Option<&str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}

fn parse_content_range(value: &str) -> Option<(Option<(u64, u64)>, u64)> {
    let spec = value.trim().strip_prefix("bytes ")?;
    let (span, size) = spec.split_once('/')?;
    let size = size.trim().parse().ok()?;
    if span.trim() == "*" {
        return Some((None, size));
    }
    let (start, end) = span.split_once('-')?;
    Some((
        Some((start.trim().parse().ok()?, end.trim().parse().ok()?)),
        size,
    ))
}

pub(crate) fn into_read(
    id: &StoredFileId,
    response: Response,
) -> Result<StoredFileRead, FileStorageError> {
    let headers = response.headers();
    let content_range = header(headers, CONTENT_RANGE).and_then(parse_content_range);
    if response.status() == StatusCode::RANGE_NOT_SATISFIABLE {
        return Err(FileStorageError::RangeNotSatisfiable {
            size: content_range.map_or(0, |(_, size)| size),
        });
    }
    let range = match (response.status(), content_range) {
        (StatusCode::PARTIAL_CONTENT, Some((Some((start, end)), size))) => {
            Some(ServedRange { start, end, size })
        },
        _ => None,
    };
    let size = range
        .map(|r| r.size)
        .or_else(|| header(headers, CONTENT_LENGTH).and_then(|v| v.parse::<u64>().ok()));
    let updated_at = header(headers, LAST_MODIFIED)
        .and_then(|v| DateTime::parse_from_rfc2822(v).ok())
        .map_or_else(Utc::now, |v| v.with_timezone(&Utc));
    let metadata = StoredFileMetadata {
        id: id.clone(),
        path: id.as_str().to_owned(),
        mime_type: header(headers, CONTENT_TYPE).map_or_else(
            || mime_for(Path::new(id.as_str())).to_owned(),
            str::to_owned,
        ),
        size_bytes: size.and_then(|s| i64::try_from(s).ok()),
        etag: header(headers, ETAG).map(str::to_owned),
        created_at: updated_at,
        updated_at,
    };
    let body = response
        .bytes_stream()
        .map_err(|e| FileStorageError::Backend(Box::new(e)));
    Ok(StoredFileRead {
        metadata,
        range,
        body: Box::pin(body),
    })
}
