//! Reading access back (with the grant audit log on `?history=true`), the
//! install's grant subject kinds and subject search (with each user's level
//! on one document when `document_id` is given), ownership transfer and
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
use systemprompt_identifiers::{DocumentId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_models::documents::AccessLevel;
use systemprompt_runtime::documents::active_user_display;
use systemprompt_runtime::documents::views::access_view;

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
    let access = access_view(&state.service, &who.user, &id, query.history, &who.trace)
        .await
        .map(Json)
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
    #[serde(default)]
    document_id: Option<DocumentId>,
}

#[derive(Debug, Serialize)]
pub(super) struct SubjectJson {
    value: String,
    display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    access_level: Option<&'static str>,
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
    let who = caller(&state, &req_ctx).await?;
    let found = state
        .service
        .access()
        .search_subjects(
            &query.kind,
            &query.q,
            query.limit.unwrap_or(MAX_SUBJECT_SEARCH),
        )
        .await?;
    let levels = match query.document_id.as_ref().filter(|_| found.kind == "user") {
        Some(document) => {
            let users: Vec<UserId> = found
                .matches
                .iter()
                .map(|subject| UserId::new(subject.value.clone()))
                .collect();
            let levels = state
                .service
                .levels_on(&who.user, document, &users, &who.trace)
                .await?;
            levels
                .into_iter()
                .map(|level| Some(level.map_or("none", AccessLevel::as_str)))
                .collect()
        },
        None => vec![None; found.matches.len()],
    };
    Ok(Json(SubjectsJson {
        kind: found.kind,
        searchable: found.searchable,
        subjects: found
            .matches
            .into_iter()
            .zip(levels)
            .map(|(subject, access_level)| SubjectJson {
                value: subject.value,
                display_name: subject.display,
                detail: subject.detail,
                access_level,
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
