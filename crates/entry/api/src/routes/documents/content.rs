//! Revision content: streamed reads, byte and op commits, history,
//! projections and the HTML preview.
//!
//! A read answers with the revision's content hash as its `ETag`: a matching
//! `If-None-Match` is `304`, a single `Range` is `206` (unless a stale
//! `If-Range` asks for the whole body). A write names its base revision in
//! `If-Match`; a base that is no longer the head answers `409` with both.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::blob_store::BlobStoreError;
use systemprompt_files::documents::service::StreamRequest;
use systemprompt_identifiers::{DocumentId, DocumentRevisionId};
use systemprompt_models::RequestContext;
use systemprompt_traits::{FileStorageError, StoredFileRead};

use super::DocumentsState;
use super::caller::{Caller, caller};
use super::error::DocumentsResult;
use super::headers::{content_disposition, etag, if_range_holds, none_match, range};
use super::items::document_id;
use crate::error::ApiHttpError;

#[derive(Debug, Deserialize)]
pub(super) struct RevisionQuery {
    pub(super) revision: Option<String>,
}

pub(super) fn revision_id(raw: Option<&str>) -> DocumentsResult<Option<DocumentRevisionId>> {
    Ok(raw.map(DocumentRevisionId::try_new).transpose()?)
}

pub(super) async fn read(
    State(state): State<DocumentsState>,
    Path(id): Path<String>,
    Query(query): Query<RevisionQuery>,
    parts: Parts,
) -> DocumentsResult<Response> {
    let who = caller(&state, request_context(&parts)?).await?;
    let id = document_id(&id)?;
    let revision = revision_id(query.revision.as_deref())?;
    let request = ContentRequest {
        who: &who,
        method: &parts.method,
        headers: &parts.headers,
        id: &id,
        revision,
    };
    serve(&state, request).await
}

pub(super) fn request_context(parts: &Parts) -> DocumentsResult<&RequestContext> {
    parts.extensions.get::<RequestContext>().ok_or_else(|| {
        ApiHttpError::unauthorized("document routes need an authenticated session").into()
    })
}

pub(super) struct ContentRequest<'a> {
    pub(super) who: &'a Caller,
    pub(super) method: &'a Method,
    pub(super) headers: &'a HeaderMap,
    pub(super) id: &'a DocumentId,
    pub(super) revision: Option<DocumentRevisionId>,
}

pub(super) async fn serve(
    state: &DocumentsState,
    request: ContentRequest<'_>,
) -> DocumentsResult<Response> {
    let ContentRequest {
        who,
        method,
        headers,
        id,
        revision,
    } = request;
    let view = state.service.get(&who.user, id, &who.trace).await?;
    let wanted = (*method == Method::GET).then(|| range(headers)).flatten();
    let request = StreamRequest {
        document: id,
        revision: revision.as_ref(),
        range: wanted,
    };
    let opened = state
        .service
        .open_stream(&who.user, request, &who.trace)
        .await;
    let (revision, read) = match opened {
        Err(DocumentsError::Blob(BlobStoreError::Storage(
            FileStorageError::RangeNotSatisfiable { size },
        ))) => return Ok(range_not_satisfiable(size)),
        other => other?,
    };
    let tag = etag(&revision.content_sha256);
    if none_match(headers, &tag) {
        return Ok(not_modified(&tag));
    }
    let read = if read.range.is_some() && !if_range_holds(headers, &tag) {
        let whole = StreamRequest {
            document: id,
            revision: Some(&revision.id),
            range: None,
        };
        state
            .service
            .open_stream(&who.user, whole, &who.trace)
            .await?
            .1
    } else {
        read
    };
    let served = Served {
        tag: &tag,
        mime: &view.document.mime_type,
        name: &view.document.name,
        revision: &revision.id,
        head: *method == Method::HEAD,
    };
    Ok(stream_response(read, &served))
}

struct Served<'a> {
    tag: &'a str,
    mime: &'a str,
    name: &'a str,
    revision: &'a DocumentRevisionId,
    head: bool,
}

fn stream_response(read: StoredFileRead, served: &Served<'_>) -> Response {
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
    let body = if served.head {
        Body::empty()
    } else {
        Body::from_stream(read.body)
    };
    let mut response = Response::new(body);
    *response.status_mut() = status;
    let headers = response.headers_mut();
    let mut set = |name, value: String| {
        if let Ok(value) = HeaderValue::from_str(&value) {
            headers.insert(name, value);
        }
    };
    set(header::CONTENT_TYPE, served.mime.to_owned());
    set(header::ETAG, served.tag.to_owned());
    set(header::ACCEPT_RANGES, "bytes".to_owned());
    set(header::CACHE_CONTROL, "private, no-cache".to_owned());
    set(
        header::CONTENT_DISPOSITION,
        content_disposition(served.name),
    );
    set(
        header::HeaderName::from_static("x-revision-id"),
        served.revision.as_str().to_owned(),
    );
    if let Some(length) = length {
        set(header::CONTENT_LENGTH, length.to_string());
    }
    if let Some(range) = read.range {
        set(header::CONTENT_RANGE, range.content_range());
    }
    response
}

fn not_modified(tag: &str) -> Response {
    let mut response = StatusCode::NOT_MODIFIED.into_response();
    if let Ok(value) = HeaderValue::from_str(tag) {
        response.headers_mut().insert(header::ETAG, value);
    }
    response
}

fn range_not_satisfiable(size: u64) -> Response {
    (
        StatusCode::RANGE_NOT_SATISFIABLE,
        [
            (header::CONTENT_RANGE, format!("bytes */{size}")),
            (header::ACCEPT_RANGES, "bytes".to_owned()),
        ],
    )
        .into_response()
}
