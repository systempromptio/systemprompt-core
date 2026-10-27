//! Reading access back, the install's grant subject kinds, ownership
//! transfer and revoking every grant on a document.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::UserId;
use systemprompt_models::RequestContext;
use systemprompt_files::documents::service::TransferRequest;
use systemprompt_runtime::documents::active_user_display;
use systemprompt_runtime::documents::mcp::{AccessOutput, GrantItem};

use crate::error::ApiHttpError;

use super::DocumentsState;
use super::caller::caller;
use super::error::DocumentsResult;
use super::items::document_id;
use super::views::DocumentJson;

pub(super) async fn access(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> DocumentsResult<Json<AccessOutput>> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    let access = state.service.access_of(&who.user, &id, &who.trace).await?;
    Ok(Json(AccessOutput {
        document_id: access.document.id,
        owner: access.owner,
        my_access: access.my_access,
        grants: access.grants.into_iter().map(GrantItem::from).collect(),
        inherited: access.inherited.into_iter().map(GrantItem::from).collect(),
    }))
}

#[derive(Debug, Serialize)]
pub(super) struct SubjectKindsJson {
    subject_kinds: Vec<String>,
}

pub(super) async fn subject_kinds(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
) -> DocumentsResult<Json<SubjectKindsJson>> {
    caller(&state, &req_ctx).await?;
    Ok(Json(SubjectKindsJson {
        subject_kinds: state.service.access().subject_kinds(),
    }))
}

#[derive(Debug, Deserialize)]
pub(super) struct TransferBody {
    transfer_to: UserId,
}

pub(super) async fn transfer(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<TransferBody>,
) -> DocumentsResult<Json<DocumentJson>> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    let to_display = active_user_display(&state.user_repository, &body.transfer_to)
        .await
        .map_err(ApiHttpError::from)?;
    let request = TransferRequest {
        to: body.transfer_to,
        to_display,
    };
    let document = state
        .service
        .transfer_ownership(&who.user, &id, request, &who.trace)
        .await?;
    Ok(Json(document.into()))
}

pub(super) async fn revoke_all(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> DocumentsResult<StatusCode> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    state.service.revoke_all(&who.user, &id, &who.trace).await?;
    Ok(StatusCode::NO_CONTENT)
}
