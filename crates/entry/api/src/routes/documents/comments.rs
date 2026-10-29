//! Comment threads on one document: list, create (a node anchor with an
//! optional quote and range of engine cursors, or a reply by `parent_id`,
//! with optional mentions), resolve, edit (the author's body and mentions)
//! and delete.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::Deserialize;
use systemprompt_files::documents::models::{CommentAnchor, CommentRange};
use systemprompt_files::documents::service::{
    CommentRequest, DeleteCommentRequest, EditCommentRequest, MentionOption, ResolveCommentRequest,
};
use systemprompt_identifiers::{DocumentCommentId, DocumentNodeId, DocumentRevisionId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_models::notifications::Attention;
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
    #[serde(default)]
    range: Option<CommentRange>,
}

#[derive(Debug, Deserialize)]
pub(super) struct MentionOptionBody {
    user_id: UserId,
    attention: Attention,
}

fn mention_options(options: Vec<MentionOptionBody>) -> Vec<MentionOption> {
    options
        .into_iter()
        .map(|option| MentionOption {
            user_id: option.user_id,
            attention: option.attention,
        })
        .collect()
}

#[derive(Debug, Deserialize)]
pub(super) struct CommentBody {
    #[serde(default)]
    anchor: Option<AnchorBody>,
    body: String,
    #[serde(default)]
    mentions: Vec<UserId>,
    #[serde(default)]
    mention_options: Vec<MentionOptionBody>,
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
    let (anchor, range) = body
        .anchor
        .map(|anchor| {
            let node = CommentAnchor {
                node: anchor.node,
                find: anchor.find,
                nth: anchor.nth,
            };
            (node, anchor.range)
        })
        .unzip();
    let request = CommentRequest {
        document: document_id(&id)?,
        anchor,
        range: range.flatten(),
        body: body.body,
        mentions: body.mentions,
        mention_options: mention_options(body.mention_options),
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

#[derive(Debug, Deserialize)]
pub(super) struct EditBody {
    body: String,
    #[serde(default)]
    mentions: Vec<UserId>,
    #[serde(default)]
    mention_options: Vec<MentionOptionBody>,
}

pub(super) async fn edit(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path((id, comment)): Path<(String, String)>,
    Json(body): Json<EditBody>,
) -> DocumentsResult<Json<CommentItem>> {
    let who = caller(&state, &req_ctx).await?;
    let request = EditCommentRequest {
        document: document_id(&id)?,
        comment: DocumentCommentId::try_new(&comment)?,
        body: body.body,
        mentions: body.mentions,
        mention_options: mention_options(body.mention_options),
    };
    let edited = state
        .service
        .edit_comment(&who.user, request, &who.trace)
        .await?;
    Ok(Json(CommentItem::from(edited)))
}

pub(super) async fn delete(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path((id, comment)): Path<(String, String)>,
) -> DocumentsResult<StatusCode> {
    let who = caller(&state, &req_ctx).await?;
    let request = DeleteCommentRequest {
        document: document_id(&id)?,
        comment: DocumentCommentId::try_new(&comment)?,
    };
    state
        .service
        .delete_comment(&who.user, request, &who.trace)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
