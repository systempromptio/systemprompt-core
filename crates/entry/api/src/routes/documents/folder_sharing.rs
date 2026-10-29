//! `/api/v1/document-folders/{id}/grants` and `/{id}/access`: sharing a
//! folder subtree, with the same shapes as the document routes. Grants a
//! folder inherits from its ancestors name the folder they come from.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::{Extension, Json};
use systemprompt_files::documents::models::GrantTarget;
use systemprompt_identifiers::DocumentFolderId;
use systemprompt_models::RequestContext;
use systemprompt_runtime::documents::views::{AccessOutput, folder_access_view};

use super::DocumentsState;
use super::access::HistoryQuery;
use super::caller::caller;
use super::error::DocumentsResult;
use super::sharing::{GrantsJson, RevokeQuery, ShareBody, list_on, revoke_on, share_on};

fn folder_target(raw: &str) -> DocumentsResult<GrantTarget> {
    Ok(GrantTarget::Folder(DocumentFolderId::try_new(raw)?))
}

pub(super) async fn list_grants(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> DocumentsResult<Json<GrantsJson>> {
    let who = caller(&state, &req_ctx).await?;
    list_on(&state, &who, folder_target(&id)?).await
}

pub(super) async fn share(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<ShareBody>,
) -> DocumentsResult<Response> {
    let who = caller(&state, &req_ctx).await?;
    share_on(&state, &who, folder_target(&id)?, body).await
}

pub(super) async fn revoke(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Query(query): Query<RevokeQuery>,
) -> DocumentsResult<StatusCode> {
    let who = caller(&state, &req_ctx).await?;
    revoke_on(&state, &who, folder_target(&id)?, &query).await
}

pub(super) async fn access(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> DocumentsResult<Json<AccessOutput>> {
    let who = caller(&state, &req_ctx).await?;
    let id = DocumentFolderId::try_new(&id)?;
    let access =
        folder_access_view(&state.service, &who.user, &id, query.history, &who.trace).await?;
    Ok(Json(access))
}
