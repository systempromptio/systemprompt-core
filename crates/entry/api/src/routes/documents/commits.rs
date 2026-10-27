//! Commits: raw-byte writes with an `If-Match` base and hash-anchored edit
//! ops, answered with the new revision or a 409 naming head and base.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::request::Parts;
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{CommitOutcome, CommitRequest};
use systemprompt_identifiers::{DocumentId, DocumentRevisionId};
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;
use systemprompt_models::documents::EditOp;

use super::DocumentsState;
use super::caller::caller;
use super::content::request_context;
use super::error::{DocumentsResult, conflict_response};
use super::headers::{base_revision, etag};
use super::items::{api_author, document_id};
use super::views::RevisionJson;

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
    Path(id): Path<String>,
    Query(query): Query<WriteQuery>,
    parts: Parts,
    body: Bytes,
) -> DocumentsResult<Response> {
    let who = caller(&state, request_context(&parts)?).await?;
    let headers = &parts.headers;
    let id = document_id(&id)?;
    let request = CommitRequest {
        document_id: id.clone(),
        base_revision: base_revision(headers)?,
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
