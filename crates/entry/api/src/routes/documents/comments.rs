//! Comment threads on one document: list, create (a node anchor with an
//! optional quote, or a reply by `parent_id`) and resolve.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::Deserialize;
use systemprompt_files::documents::models::CommentAnchor;
use systemprompt_files::documents::service::{CommentRequest, ResolveCommentRequest};
use systemprompt_identifiers::{DocumentCommentId, DocumentNodeId, DocumentRevisionId};
use systemprompt_models::RequestContext;
use systemprompt_runtime::documents::views::{CommentItem, CommentOutput, comments_view};

use super::DocumentsState;
use super::caller::caller;
use super::error::DocumentsResult;
use super::items::document_id;

pub(super) async fn list(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> DocumentsResult<Json<CommentOutput>> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    Ok(Json(
        comments_view(&state.service, &who.user, &id, &who.trace).await?,
    ))
}

#[derive(Debug, Deserialize)]
pub(super) struct AnchorBody {
    node: DocumentNodeId,
    #[serde(default)]
    find: Option<String>,
    #[serde(default)]
    nth: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CommentBody {
    #[serde(default)]
    anchor: Option<AnchorBody>,
    body: String,
    #[serde(default)]
    parent_id: Option<DocumentCommentId>,
}

pub(super) async fn create(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<CommentBody>,
) -> DocumentsResult<Response> {
    let who = caller(&state, &req_ctx).await?;
    let request = CommentRequest {
        document: document_id(&id)?,
        anchor: body.anchor.map(|anchor| CommentAnchor {
            node: anchor.node,
            find: anchor.find,
            nth: anchor.nth,
        }),
        body: body.body,
        parent_id: body.parent_id,
    };
    let comment = state
        .service
        .create_comment(&who.user, request, &who.trace)
        .await?;
    Ok((StatusCode::CREATED, Json(CommentItem::from(comment))).into_response())
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct ResolveBody {
    #[serde(default)]
    answering: Option<DocumentRevisionId>,
}

pub(super) async fn resolve(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path((id, comment)): Path<(String, String)>,
    body: Option<Json<ResolveBody>>,
) -> DocumentsResult<Json<CommentItem>> {
    let who = caller(&state, &req_ctx).await?;
    let answering = body.and_then(|Json(body)| body.answering);
    let request = ResolveCommentRequest {
        document: document_id(&id)?,
        comment: DocumentCommentId::try_new(&comment)?,
        answering,
    };
    let resolved = state
        .service
        .resolve_comment(&who.user, request, &who.trace)
        .await?;
    Ok(Json(CommentItem::from(resolved)))
}
