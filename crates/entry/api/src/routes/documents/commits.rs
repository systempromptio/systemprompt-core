//! Commits: raw-byte writes with an `If-Match` base and node-guarded edit
//! ops, answered with the new revision (with `merged_from` when the head
//! moved under the write) or a 409 `stale_base` naming head, its content hash
//! and base under `detail`. An `op_id` (query
//! parameter on a byte write, body field on edits) makes the write
//! idempotent: the commit stores its acknowledgement in its own transaction,
//! and a repeat of the same request (over REST or MCP) answers it with
//! `replayed`; a different request under a used op id is refused 409
//! `op_id_mismatch`.
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
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{
    CommentPolicy, CommitOutcome, CommitRequest, EditModeRequest, LandedCommit, ProposeRequest,
};
use systemprompt_identifiers::{DocumentCommentId, DocumentId, DocumentOpId, DocumentRevisionId};
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;
use systemprompt_models::documents::{ActorKind, EditMode, EditOp};
use systemprompt_runtime::documents::refusal::stale_base;

use super::DocumentsState;
use super::caller::{api_author, caller};
use super::content::request_context;
use super::error::{DocumentsResult, refused_response, status_of};
use super::headers::{base_revision, etag};
use super::items::document_id;
use super::proposals::proposed_response;
use super::views::RevisionJson;

#[derive(Debug, Serialize)]
pub(super) struct CommittedJson {
    revision: RevisionJson,
    #[serde(skip_serializing_if = "Option::is_none")]
    merged_from: Option<DocumentRevisionId>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    replayed: bool,
}

fn landed_response(landed: LandedCommit) -> Response {
    let tag = etag(&landed.revision.content_sha256);
    let merged = landed.ack.merged_from.is_some();
    let mut response = Json(CommittedJson {
        revision: landed.revision.into(),
        merged_from: landed.ack.merged_from,
        replayed: landed.replayed,
    })
    .into_response();
    if !merged && let Ok(value) = HeaderValue::from_str(&tag) {
        response.headers_mut().insert(header::ETAG, value);
    }
    response
}

pub(super) fn outcome_response(outcome: CommitOutcome, document: &DocumentId) -> Response {
    match outcome {
        CommitOutcome::Committed(landed) | CommitOutcome::Merged(landed) => {
            landed_response(*landed)
        },
        CommitOutcome::Conflict { head, base } => refused_response(
            status_of(&DocumentsError::Conflict {
                document_id: document.clone(),
                head: head.id.clone(),
                base: base.clone(),
            }),
            stale_base(document, &head, base.as_ref()),
        ),
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
        op_id: query.op_id,
    };
    let outcome = state
        .service
        .commit_external(&who.user, request, &who.trace)
        .await?;
    Ok(outcome_response(outcome, &id))
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
        document_id: id.clone(),
        base_revision: body.base_revision,
        body: CommitBody::Ops(body.ops),
        author: api_author(&req_ctx, &headers),
        message: body.message,
        lock_holder_id: None,
        comments: CommentPolicy {
            allow_detach: body.allow_detach,
            answering: body.answering,
        },
        op_id: body.op_id,
    };
    let outcome = state.service.commit(&who.user, request, &who.trace).await?;
    Ok(outcome_response(outcome, &id))
}
