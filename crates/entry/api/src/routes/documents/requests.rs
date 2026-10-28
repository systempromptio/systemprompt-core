//! Access requests: a signed-in user with no level asks a document's owner
//! for access, and a holder of `share` grants or declines it.
//!
//! A caller who cannot open a document is told it does not exist. When the
//! install enables `files.documents.accessRequests`, a signed-in caller's
//! refusal on an existing document also carries `can_request: true`, which
//! tells them the document exists; that disclosure is the price of letting
//! users ask for access, and it is off by default.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::service::{AccessDecision, AccessRequestInput, DecideRequest};
use systemprompt_identifiers::{DocumentAccessRequestId, DocumentId};
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;
use systemprompt_models::documents::AccessLevel;
use systemprompt_runtime::documents::active_user_display;
use systemprompt_runtime::documents::mcp::AccessRequestItem;

use crate::error::ApiHttpError;

use super::DocumentsState;
use super::caller::{Caller, caller};
use super::error::{DocumentsHttpError, DocumentsResult, with_detail};
use super::items::document_id;

#[derive(Debug, Serialize)]
struct CanRequestDetail {
    can_request: bool,
}

pub(super) async fn with_request_hint<T: IntoResponse>(
    state: &DocumentsState,
    who: &Caller,
    document: &DocumentId,
    result: DocumentsResult<T>,
) -> DocumentsResult<Response> {
    match result {
        Ok(value) => Ok(value.into_response()),
        Err(DocumentsHttpError::Service(error))
            if matches!(
                *error,
                DocumentsError::NotFound {
                    what: "document",
                    ..
                }
            ) && state.service.can_request_access(&who.user, document).await =>
        {
            let api = ApiError::not_found(error.to_string());
            Ok(with_detail(&api, CanRequestDetail { can_request: true }))
        },
        Err(error) => Err(error),
    }
}

fn request_error(error: DocumentsError) -> DocumentsHttpError {
    let message = error.to_string();
    let api = match error {
        DocumentsError::AccessRequestOpen { .. } => {
            ApiError::conflict(message).with_error_key("access_request_open")
        },
        DocumentsError::AccessRequestClosed { .. } => {
            ApiError::conflict(message).with_error_key("access_request_closed")
        },
        DocumentsError::AccessRequestNotNeeded { .. } => {
            ApiError::bad_request(message).with_error_key("access_request_not_needed")
        },
        other => return other.into(),
    };
    ApiHttpError::from(api).into()
}

#[derive(Debug, Deserialize)]
pub(super) struct RequestBody {
    level: AccessLevel,
    #[serde(default)]
    message: Option<String>,
}

pub(super) async fn create(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<RequestBody>,
) -> DocumentsResult<Response> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    let request = state
        .service
        .request_access(
            &who.user,
            &id,
            AccessRequestInput {
                level: body.level,
                message: body.message,
            },
        )
        .await
        .map_err(request_error)?;
    Ok((StatusCode::CREATED, Json(AccessRequestItem::from(request))).into_response())
}

fn request_id(raw: &str) -> DocumentsResult<DocumentAccessRequestId> {
    Ok(DocumentAccessRequestId::try_new(raw)?)
}

pub(super) async fn grant(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path((id, rid)): Path<(String, String)>,
) -> DocumentsResult<Json<AccessRequestItem>> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    let rid = request_id(&rid)?;
    let requests = state
        .service
        .access_requests(&who.user, &id, &who.trace)
        .await?;
    let subject_display = match requests.iter().find(|request| request.id == rid) {
        Some(request) => active_user_display(&state.user_repository, &request.requester_user_id)
            .await
            .map_err(ApiHttpError::from)?,
        None => None,
    };
    let decide_request = DecideRequest {
        document: id,
        request: rid,
        decision: AccessDecision::Grant { subject_display },
    };
    decide(&state, &who, decide_request).await
}

pub(super) async fn decline(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path((id, rid)): Path<(String, String)>,
) -> DocumentsResult<Json<AccessRequestItem>> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    let rid = request_id(&rid)?;
    let decide_request = DecideRequest {
        document: id,
        request: rid,
        decision: AccessDecision::Decline,
    };
    decide(&state, &who, decide_request).await
}

async fn decide(
    state: &DocumentsState,
    who: &Caller,
    request: DecideRequest,
) -> DocumentsResult<Json<AccessRequestItem>> {
    let decided = state
        .service
        .decide_access_request(&who.user, request, &who.trace)
        .await
        .map_err(request_error)?;
    Ok(Json(AccessRequestItem::from(decided)))
}
