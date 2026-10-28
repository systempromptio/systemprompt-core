//! `GET /{id}/content`: the blob a completed upload produced, streamed from
//! storage. A single `Range` answers `206`, an unsatisfiable one `416`; the
//! blob hash is the `ETag`, and a matching `If-None-Match` answers `304`.
//! The body is served as user content: `nosniff` and a sandbox CSP.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{Extensions, HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use systemprompt_files::documents::blob_store::BlobStoreError;
use systemprompt_files::uploads::UploadError;
use systemprompt_traits::{ByteRange, FileStorageError};

use super::handlers::upload_id;
use super::principal::uploader;
use crate::error::ApiHttpError;
use crate::routes::documents::DocumentsState;
use crate::routes::user_content::{USER_CONTENT_CSP, harden};

pub(super) async fn content(
    State(state): State<DocumentsState>,
    extensions: Extensions,
    method: Method,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiHttpError> {
    let who = uploader(&extensions)?;
    let range = (method == Method::GET)
        .then(|| {
            headers
                .get(header::RANGE)?
                .to_str()
                .ok()
                .and_then(ByteRange::parse)
        })
        .flatten();
    let opened = state
        .uploads
        .open_blob(&who.user, &upload_id(&id)?, range)
        .await;
    let (session, read) = match opened {
        Err(UploadError::Blob(BlobStoreError::Storage(
            FileStorageError::RangeNotSatisfiable { size },
        ))) => return Ok(unsatisfiable(size)),
        other => other?,
    };
    let tag = session
        .blob_sha256
        .as_ref()
        .map(|hash| format!("\"{hash}\""))
        .unwrap_or_default();
    let matched = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.split(',').any(|candidate| candidate.trim() == tag));
    if matched {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }
    let length = read.range.map_or_else(
        || {
            read.metadata
                .size_bytes
                .and_then(|size| u64::try_from(size).ok())
        },
        |range| Some(range.length()),
    );
    let status = if read.range.is_some() {
        StatusCode::PARTIAL_CONTENT
    } else {
        StatusCode::OK
    };
    let body = if method == Method::HEAD {
        Body::empty()
    } else {
        Body::from_stream(read.body)
    };
    let mut response = Response::new(body);
    *response.status_mut() = status;
    let out = response.headers_mut();
    let mut set = |name, value: String| {
        if let Ok(value) = HeaderValue::from_str(&value) {
            out.insert(name, value);
        }
    };
    let mime = session.detected_mime.unwrap_or(session.declared_mime);
    set(header::CONTENT_TYPE, mime);
    set(header::ETAG, tag);
    set(header::ACCEPT_RANGES, "bytes".to_owned());
    set(header::CACHE_CONTROL, "private, no-cache".to_owned());
    set(header::CONTENT_DISPOSITION, disposition(&session.file_name));
    if let Some(length) = length {
        set(header::CONTENT_LENGTH, length.to_string());
    }
    if let Some(range) = read.range {
        set(header::CONTENT_RANGE, range.content_range());
    }
    harden(&mut response, USER_CONTENT_CSP);
    Ok(response)
}

fn disposition(name: &str) -> String {
    let encoded: String = url::form_urlencoded::byte_serialize(name.as_bytes()).collect();
    format!(
        "attachment; filename*=UTF-8''{}",
        encoded.replace('+', "%20")
    )
}

fn unsatisfiable(size: u64) -> Response {
    (
        StatusCode::RANGE_NOT_SATISFIABLE,
        [
            (header::CONTENT_RANGE, format!("bytes */{size}")),
            (header::ACCEPT_RANGES, "bytes".to_owned()),
        ],
    )
        .into_response()
}
