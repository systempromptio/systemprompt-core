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

use axum::body::{Body, Bytes};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::blob_store::BlobStoreError;
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{
    CommitOutcome, CommitRequest, HistoryPage, StreamRequest,
};
use systemprompt_identifiers::{DocumentId, DocumentRevisionId};
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;
use systemprompt_models::documents::{EditOp, Projection};
use systemprompt_traits::{FileStorageError, StoredFileRead};

use super::DocumentsState;
use super::caller::caller;
use super::documents::{api_author, cursor, document_id};
use super::error::{DocumentsHttpError, DocumentsResult, conflict_response};
use super::headers::{base_revision, content_disposition, etag, if_range_holds, none_match, range};
use super::views::RevisionJson;

const PREVIEW_CSP: &str =
    "sandbox; default-src 'none'; style-src 'unsafe-inline'; img-src data:";
const DEFAULT_HISTORY: i64 = 50;
const MAX_HISTORY: i64 = 500;

#[derive(Debug, Deserialize)]
pub(super) struct RevisionQuery {
    revision: Option<String>,
}

fn revision_id(raw: Option<&str>) -> Result<Option<DocumentRevisionId>, ApiError> {
    raw.map(DocumentRevisionId::try_new)
        .transpose()
        .map_err(ApiError::from)
}

pub(super) async fn read(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    method: Method,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<RevisionQuery>,
) -> DocumentsResult<Response> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    let revision = revision_id(query.revision.as_deref())?;
    let view = state.service.get(&who.user, &id, &who.trace).await?;
    let wanted = (method == Method::GET).then(|| range(&headers)).flatten();
    let request = StreamRequest {
        document: &id,
        revision: revision.as_ref(),
        range: wanted,
    };
    let opened = state.service.open_stream(&who.user, request, &who.trace).await;
    let (revision, read) = match opened {
        Err(DocumentsError::Blob(BlobStoreError::Storage(
            FileStorageError::RangeNotSatisfiable { size },
        ))) => return Ok(range_not_satisfiable(size)),
        other => other?,
    };
    let tag = etag(&revision.content_sha256);
    if none_match(&headers, &tag) {
        return Ok(not_modified(&tag));
    }
    let read = if read.range.is_some() && !if_range_holds(&headers, &tag) {
        let whole = StreamRequest {
            document: &id,
            revision: Some(&revision.id),
            range: None,
        };
        state.service.open_stream(&who.user, whole, &who.trace).await?.1
    } else {
        read
    };
    let served = Served {
        tag: &tag,
        mime: &view.document.mime_type,
        name: &view.document.name,
        revision: &revision.id,
        head: method == Method::HEAD,
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
    set(header::CONTENT_DISPOSITION, content_disposition(served.name));
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

#[derive(Debug, Serialize)]
pub(super) struct CommittedJson {
    revision: RevisionJson,
    #[serde(skip_serializing_if = "Option::is_none")]
    merged_from: Option<DocumentRevisionId>,
}

pub(super) fn outcome_response(outcome: CommitOutcome, document: &DocumentId) -> Response {
    match outcome {
        CommitOutcome::Committed { revision, .. } => {
            let tag = etag(&revision.content_sha256);
            let mut response = Json(CommittedJson {
                revision: revision.into(),
                merged_from: None,
            })
            .into_response();
            if let Ok(value) = HeaderValue::from_str(&tag) {
                response.headers_mut().insert(header::ETAG, value);
            }
            response
        },
        CommitOutcome::Merged {
            revision,
            merged_from,
            ..
        } => Json(CommittedJson {
            revision: revision.into(),
            merged_from: Some(merged_from),
        })
        .into_response(),
        CommitOutcome::Conflict { head, base } => {
            let error = DocumentsError::Conflict {
                document_id: document.clone(),
                head: head.id.clone(),
                base: base.clone(),
            };
            conflict_response(
                error.to_string(),
                &head.id,
                Some(&head.content_sha256),
                base.as_ref(),
            )
        },
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct WriteQuery {
    message: Option<String>,
}

pub(super) async fn write(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<WriteQuery>,
    body: Bytes,
) -> DocumentsResult<Response> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    let request = CommitRequest {
        document_id: id.clone(),
        base_revision: base_revision(&headers)?,
        body: CommitBody::Bytes(body.to_vec()),
        author: api_author(),
        message: query.message,
        lock_holder_id: None,
    };
    let outcome = state.service.commit(&who.user, request, &who.trace).await?;
    Ok(outcome_response(outcome, &id))
}

#[derive(Debug, Deserialize)]
pub(super) struct EditsBody {
    base_revision: DocumentRevisionId,
    ops: Vec<EditOp>,
    #[serde(default)]
    message: Option<String>,
}

pub(super) async fn edits(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<EditsBody>,
) -> DocumentsResult<Response> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    if body.ops.is_empty() {
        return Err(ApiError::bad_request("ops must name at least one edit").into());
    }
    let request = CommitRequest {
        document_id: id.clone(),
        base_revision: Some(body.base_revision),
        body: CommitBody::Ops(body.ops),
        author: api_author(),
        message: body.message,
        lock_holder_id: None,
    };
    let outcome = state.service.commit(&who.user, request, &who.trace).await?;
    Ok(outcome_response(outcome, &id))
}

#[derive(Debug, Deserialize)]
pub(super) struct HistoryQuery {
    limit: Option<i64>,
    before: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct HistoryJson {
    revisions: Vec<RevisionJson>,
}

pub(super) async fn revisions(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> DocumentsResult<Json<HistoryJson>> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    let page = HistoryPage {
        limit: query.limit.unwrap_or(DEFAULT_HISTORY).clamp(1, MAX_HISTORY),
        before: cursor(query.before.as_deref())?,
    };
    let revisions = state
        .service
        .history(&who.user, &id, page, &who.trace)
        .await?;
    Ok(Json(HistoryJson {
        revisions: revisions.into_iter().map(Into::into).collect(),
    }))
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ProjectionView {
    #[default]
    Outline,
    Text,
}

#[derive(Debug, Deserialize)]
pub(super) struct ProjectionQuery {
    #[serde(default)]
    view: ProjectionView,
    revision: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(super) enum ProjectionJson {
    Outline {
        revision_id: DocumentRevisionId,
        projection: Projection,
    },
    Text {
        revision_id: DocumentRevisionId,
        text: String,
    },
}

pub(super) async fn projection(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Query(query): Query<ProjectionQuery>,
) -> DocumentsResult<Json<ProjectionJson>> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    let revision = revision_id(query.revision.as_deref())?;
    let body = match query.view {
        ProjectionView::Outline => {
            let (revision, projection) = state
                .service
                .project(&who.user, &id, revision.as_ref(), &who.trace)
                .await?;
            ProjectionJson::Outline {
                revision_id: revision.id,
                projection,
            }
        },
        ProjectionView::Text => {
            let (revision, text) = state
                .service
                .plain_text(&who.user, &id, revision.as_ref(), &who.trace)
                .await?;
            ProjectionJson::Text {
                revision_id: revision.id,
                text,
            }
        },
    };
    Ok(Json(body))
}

pub(super) async fn preview(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Query(query): Query<RevisionQuery>,
) -> Result<Response, DocumentsHttpError> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    let revision = revision_id(query.revision.as_deref())?;
    let (revision, html) = state
        .service
        .preview_html(&who.user, &id, revision.as_ref(), &who.trace)
        .await?;
    let mut response = Html(html).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(PREVIEW_CSP),
    );
    if let Ok(value) = HeaderValue::from_str(&etag(&revision.content_sha256)) {
        headers.insert(header::ETAG, value);
    }
    Ok(response)
}
