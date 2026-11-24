//! A document's invites: list, create and revoke, and the claim a signed-in
//! person makes with an invite's token. Creating returns the token and path
//! once; core delivers nothing, the sharer sends the link. A claim reads the
//! caller's email and whether it is verified from the user store; the service
//! claims only a verified email that is the invite's.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use systemprompt_files::documents::models::DocumentInvite;
use systemprompt_files::documents::service::InviteRequest;
use systemprompt_identifiers::{DocumentId, DocumentInviteId, Email};
use systemprompt_models::RequestContext;
use systemprompt_models::documents::AccessLevel;
use systemprompt_models::modules::ApiPaths;

use super::DocumentsState;
use super::caller::caller;
use super::items::document_id;
use crate::http::error::ApiHttpError;
use systemprompt_documents::http::error::DocumentsResult;

#[derive(Debug, Serialize)]
pub(super) struct InvitesJson {
    invites: Vec<DocumentInvite>,
}

#[derive(Debug, Deserialize)]
pub(super) struct InviteBody {
    email: String,
    level: AccessLevel,
    #[serde(default)]
    days: Option<u32>,
}

#[derive(Debug, Serialize)]
pub(super) struct CreatedInviteJson {
    invite: DocumentInvite,
    token: String,
    path: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct ClaimBody {
    token: String,
}

#[derive(Debug, Serialize)]
pub(super) struct ClaimedJson {
    document_id: DocumentId,
}

pub(super) async fn list_invites(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> DocumentsResult<Json<InvitesJson>> {
    let who = caller(&state, &req_ctx).await?;
    let invites = state
        .service
        .list_invites(&who.user, &document_id(&id)?, &who.trace)
        .await?;
    Ok(Json(InvitesJson { invites }))
}

pub(super) async fn create_invite(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<InviteBody>,
) -> DocumentsResult<Response> {
    let who = caller(&state, &req_ctx).await?;
    let request = InviteRequest {
        email: body.email,
        level: body.level,
        days: body.days,
    };
    let created = state
        .service
        .create_invite(&who.user, &document_id(&id)?, request, &who.trace)
        .await?;
    let path = format!("{}/{}", ApiPaths::PUBLIC_DOCUMENT_LINKS, created.token);
    Ok((
        StatusCode::CREATED,
        Json(CreatedInviteJson {
            invite: created.invite,
            token: created.token,
            path,
        }),
    )
        .into_response())
}

pub(super) async fn revoke_invite(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path((id, invite_id)): Path<(String, String)>,
) -> DocumentsResult<StatusCode> {
    let who = caller(&state, &req_ctx).await?;
    let invite = DocumentInviteId::try_new(invite_id)?;
    state
        .service
        .revoke_invite(&who.user, &document_id(&id)?, &invite, &who.trace)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn claim_invite(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Json(body): Json<ClaimBody>,
) -> DocumentsResult<Json<ClaimedJson>> {
    let who = caller(&state, &req_ctx).await?;
    let verified = state
        .user_repository
        .find_by_id(&who.user.id)
        .await
        .map_err(ApiHttpError::from)?
        .filter(|user| user.email_verified)
        .and_then(|user| Email::try_new(user.email.to_lowercase()).ok());
    let document_id = state
        .service
        .claim_invite(&who.user, &body.token, verified.as_ref(), &who.trace)
        .await?;
    Ok(Json(ClaimedJson { document_id }))
}
