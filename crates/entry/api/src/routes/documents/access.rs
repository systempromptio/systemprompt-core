//! Reading access back (with the grant audit log on `?history=true`), the
//! install's grant subject kinds and subject search, ownership transfer and
//! revoking every grant on a document.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use systemprompt_files::documents::MAX_SUBJECT_SEARCH;
use systemprompt_files::documents::service::TransferRequest;
use systemprompt_identifiers::UserId;
use systemprompt_models::RequestContext;
use systemprompt_runtime::documents::active_user_display;
use systemprompt_runtime::documents::mcp::AccessOutput;

use crate::error::ApiHttpError;

use super::DocumentsState;
use super::caller::caller;
use super::error::{DocumentsHttpError, DocumentsResult};
use super::items::document_id;
use super::requests::with_request_hint;
use super::views::DocumentJson;

#[derive(Debug, Default, Deserialize)]
pub(super) struct HistoryQuery {
    #[serde(default)]
    pub(super) history: bool,
}

pub(super) async fn access(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> DocumentsResult<Response> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    let access = state
        .service
        .access_of(&who.user, &id, query.history, &who.trace)
        .await
        .map(|access| Json(AccessOutput::from(access)))
        .map_err(DocumentsHttpError::from);
    with_request_hint(&state, &who, &id, access).await
}

#[derive(Debug, Deserialize)]
pub(super) struct SubjectsQuery {
    kind: String,
    #[serde(default)]
    q: String,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub(super) struct SubjectJson {
    value: String,
    display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct SubjectsJson {
    kind: String,
    searchable: bool,
    subjects: Vec<SubjectJson>,
}

pub(super) async fn subjects(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Query(query): Query<SubjectsQuery>,
) -> DocumentsResult<Json<SubjectsJson>> {
    caller(&state, &req_ctx).await?;
    let found = state
        .service
        .access()
        .search_subjects(
            &query.kind,
            &query.q,
            query.limit.unwrap_or(MAX_SUBJECT_SEARCH),
        )
        .await?;
    Ok(Json(SubjectsJson {
        kind: found.kind,
        searchable: found.searchable,
        subjects: found
            .matches
            .into_iter()
            .map(|subject| SubjectJson {
                value: subject.value,
                display_name: subject.display,
                detail: subject.detail,
            })
            .collect(),
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
