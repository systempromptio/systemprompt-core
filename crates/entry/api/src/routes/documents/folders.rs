//! `/api/v1/document-folders`: listing, creating, renaming, moving and
//! deleting folders.
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

use super::DocumentsState;
use super::caller::caller;
use super::documents::{ListQuery, present};
use super::error::DocumentsResult;
use super::views::FolderJson;

fn folder(raw: &str) -> Result<DocumentFolderId, ApiError> {
    Ok(DocumentFolderId::try_new(raw)?)
}

pub(super) async fn list(
    state: State<DocumentsState>,
    req_ctx: Extension<RequestContext>,
    query: Query<ListQuery>,
) -> DocumentsResult<Response> {
    Ok(super::documents::list(state, req_ctx, query)
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
    #[serde(default, deserialize_with = "present")]
    parent_id: Option<Option<DocumentFolderId>>,
}

pub(super) async fn update(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<UpdateBody>,
) -> DocumentsResult<Json<Option<FolderJson>>> {
    let who = caller(&state, &req_ctx).await?;
    let id = folder(&id)?;
    if body.name.is_none() && body.parent_id.is_none() {
        return Err(ApiError::bad_request("give a new `name`, a `parent_id`, or both").into());
    }
    let mut updated = None;
    if let Some(parent) = body.parent_id {
        updated = Some(
            state
                .service
                .move_folder(&who.user, &id, parent.as_ref(), &who.trace)
                .await?,
        );
    }
    if let Some(name) = body.name {
        updated = Some(
            state
                .service
                .rename_folder(&who.user, &id, &name, &who.trace)
                .await?,
        );
    }
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

