//! `/api/v1/document-folders`: listing, creating, renaming, moving,
//! setting the agent edit policy of, and deleting folders.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::Deserialize;
use systemprompt_files::documents::service::NewFolderRequest;
use systemprompt_identifiers::DocumentFolderId;
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;
use systemprompt_models::documents::AgentEditPolicy;

use super::DocumentsState;
use super::caller::caller;
use super::error::DocumentsResult;
use super::items::{FolderTarget, folder_target};
use super::listing::ListQuery;
use super::views::FolderJson;

fn folder(raw: &str) -> DocumentsResult<DocumentFolderId> {
    Ok(DocumentFolderId::try_new(raw)?)
}

pub(super) async fn list(
    state: State<DocumentsState>,
    req_ctx: Extension<RequestContext>,
    query: Query<ListQuery>,
) -> DocumentsResult<Response> {
    Ok(super::listing::list(state, req_ctx, query)
        .await?
        .into_response())
}

#[derive(Debug, Deserialize)]
pub(super) struct CreateBody {
    #[serde(default)]
    parent_id: Option<DocumentFolderId>,
    name: String,
}

pub(super) async fn create(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Json(body): Json<CreateBody>,
) -> DocumentsResult<Response> {
    let who = caller(&state, &req_ctx).await?;
    let request = NewFolderRequest {
        parent: body.parent_id,
        name: body.name,
    };
    let created = state
        .service
        .create_folder(&who.user, request, &who.trace)
        .await?;
    Ok((StatusCode::CREATED, Json(FolderJson::from(created))).into_response())
}

#[derive(Debug, Deserialize)]
pub(super) struct UpdateBody {
    #[serde(default)]
    name: Option<String>,
    #[serde(default, deserialize_with = "folder_target")]
    parent_id: FolderTarget,
    #[serde(default)]
    agent_edit_policy: Option<AgentEditPolicy>,
}

pub(super) async fn update(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<UpdateBody>,
) -> DocumentsResult<Json<Option<FolderJson>>> {
    let who = caller(&state, &req_ctx).await?;
    let id = folder(&id)?;
    if body.name.is_none() && body.parent_id.is_keep() && body.agent_edit_policy.is_none() {
        return Err(ApiError::bad_request(
            "give a new `name`, a `parent_id` or an `agent_edit_policy`",
        )
        .into());
    }
    let moved = if body.parent_id.is_keep() {
        None
    } else {
        Some(
            state
                .service
                .move_folder(&who.user, &id, body.parent_id.folder(), &who.trace)
                .await?,
        )
    };
    let renamed = match body.name {
        Some(name) => Some(
            state
                .service
                .rename_folder(&who.user, &id, &name, &who.trace)
                .await?,
        ),
        None => None,
    };
    let governed = match body.agent_edit_policy {
        Some(policy) => Some(
            state
                .service
                .set_folder_agent_edit_policy(&who.user, &id, policy, &who.trace)
                .await?,
        ),
        None => None,
    };
    let updated = governed.or(renamed).or(moved);
    Ok(Json(updated.map(FolderJson::from)))
}

#[derive(Debug, Deserialize)]
pub(super) struct DeleteQuery {
    #[serde(default)]
    recursive: bool,
}

pub(super) async fn delete(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Query(query): Query<DeleteQuery>,
) -> DocumentsResult<StatusCode> {
    let who = caller(&state, &req_ctx).await?;
    let id = folder(&id)?;
    state
        .service
        .delete_folder(&who.user, &id, query.recursive, &who.trace)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
