//! Commits: raw-byte writes with an `If-Match` base and node-guarded edit
//! ops, answered with the new revision (with `merged_from` when the head
//! moved under the write) or a 409 naming head and base. An `op_id` (query
//! parameter on a byte write, body field on edits) makes the write
//! idempotent: a repeat answers the stored acknowledgement with `replayed`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::models::StoredWriteAck;
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{
    CommentPolicy, CommitOutcome, CommitRequest, EditModeRequest, ProposeRequest,
};
use systemprompt_identifiers::{DocumentCommentId, DocumentId, DocumentOpId, DocumentRevisionId};
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;
use systemprompt_models::documents::{ActorKind, EditMode, EditOp};

use super::DocumentsState;
use super::caller::{Caller, api_author, caller};
use super::content::request_context;
use super::error::{DocumentsResult, conflict_response};
use super::headers::{base_revision, etag};
use super::items::document_id;
use super::proposals::proposed_response;
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
    op_id: Option<DocumentOpId>,
}

pub(super) async fn write(
    State(state): State<DocumentsState>,
    Path(id): Path<String>,
    Query(query): Query<WriteQuery>,
    parts: Parts,
    body: Bytes,
) -> DocumentsResult<Response> {
    let req_ctx = request_context(&parts)?;
    let who = caller(&state, req_ctx).await?;
    let headers = &parts.headers;
    let id = document_id(&id)?;
    let request = CommitRequest {
        document_id: id.clone(),
        base_revision: base_revision(headers)?,
        body: CommitBody::Bytes(body.to_vec()),
        author: api_author(req_ctx, headers),
        message: query.message,
        lock_holder_id: None,
        comments: CommentPolicy::detach(),
    };
    idempotent(&state, &who, request, query.op_id).await
}

#[derive(Debug, Deserialize)]
pub(super) struct EditsBody {
    #[serde(default)]
    base_revision: Option<DocumentRevisionId>,
    ops: Vec<EditOp>,
    #[serde(default)]
    mode: Option<EditMode>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    op_id: Option<DocumentOpId>,
    #[serde(default)]
    answering: Option<DocumentCommentId>,
    #[serde(default)]
    allow_detach: bool,
}

pub(super) async fn edits(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<EditsBody>,
) -> DocumentsResult<Response> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    if body.ops.is_empty() {
        return Err(ApiError::bad_request("ops must name at least one edit").into());
    }
    let mode = state
        .service
        .edit_mode(
            &who.user,
            EditModeRequest {
                document: &id,
                requested: body.mode,
                actor: ActorKind::User,
            },
            &who.trace,
        )
        .await?;
    if mode == EditMode::Propose {
        let request = ProposeRequest {
            document_id: id,
            base_revision: body.base_revision,
            ops: body.ops,
            author: api_author(&req_ctx, &headers),
            message: body.message,
        };
        let proposed = state
            .service
            .propose(&who.user, request, &who.trace)
            .await?;
        return Ok(proposed_response(proposed));
    }
    let request = CommitRequest {
        document_id: id,
        base_revision: body.base_revision,
        body: CommitBody::Ops(body.ops),
        author: api_author(&req_ctx, &headers),
        message: body.message,
        lock_holder_id: None,
        comments: CommentPolicy {
            allow_detach: body.allow_detach,
            answering: body.answering,
        },
    };
    idempotent(&state, &who, request, body.op_id).await
}

async fn idempotent(
    state: &DocumentsState,
    who: &Caller,
    request: CommitRequest,
    op_id: Option<DocumentOpId>,
) -> DocumentsResult<Response> {
    let id = request.document_id.clone();
    let Some(op_id) = op_id else {
        let outcome = state.service.commit(&who.user, request, &who.trace).await?;
        return Ok(outcome_response(outcome, &id));
    };
    if let Some(stored) = state
        .service
        .find_write_ack(&who.user, &id, &op_id, &who.trace)
        .await?
    {
        return Ok(replayed(stored.ack));
    }
    let outcome = state.service.commit(&who.user, request, &who.trace).await?;
    remember(state, &id, &op_id, &outcome).await;
    Ok(outcome_response(outcome, &id))
}

async fn remember(
    state: &DocumentsState,
    document: &DocumentId,
    op_id: &DocumentOpId,
    outcome: &CommitOutcome,
) {
    let (revision, merged_from) = match outcome {
        CommitOutcome::Committed { revision, .. } => (revision, None),
        CommitOutcome::Merged {
            revision,
            merged_from,
            ..
        } => (revision, Some(merged_from.clone())),
        CommitOutcome::Conflict { .. } => return,
    };
    let body = CommittedJson {
        revision: revision.clone().into(),
        merged_from,
    };
    let ack = match serde_json::to_value(&body) {
        Ok(ack) => ack,
        Err(error) => {
            tracing::warn!(error = %error, op_id = %op_id, "Write ack not stored");
            return;
        },
    };
    let stored = StoredWriteAck {
        revision_id: revision.id.clone(),
        ack,
    };
    if let Err(error) = state
        .service
        .record_write_ack(document, op_id, &stored)
        .await
    {
        tracing::warn!(error = %error, op_id = %op_id, "Write ack not stored");
    }
}

// JSON: the stored acknowledgement is replayed as written, flagged as a replay.
fn replayed(mut ack: serde_json::Value) -> Response {
    if let Some(fields) = ack.as_object_mut() {
        fields.insert("replayed".to_owned(), serde_json::Value::Bool(true));
    }
    Json(ack).into_response()
}
