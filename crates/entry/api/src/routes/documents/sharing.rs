//! Grants, presence and edit locks on one document.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_files::documents::models::{GrantTarget, LockHolderKind, LockOutcome};
use systemprompt_files::documents::service::{AcquireLockRequest, ShareRequest};
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;

use crate::error::ApiHttpError;
use systemprompt_models::documents::{AccessLevel, PresenceBeacon, PresenceSource, PresenceState};
use systemprompt_runtime::documents::subject_display;
use uuid::Uuid;

use super::DocumentsState;
use super::caller::caller;
use super::error::{DocumentsResult, with_detail};
use super::items::document_id;
use super::views::{GrantJson, LockJson, PresenceJson};

const DEFAULT_LOCK_SECS: u64 = 300;
const MAX_LOCK_SECS: u64 = 3600;

#[derive(Debug, Serialize)]
pub(super) struct GrantsJson {
    grants: Vec<GrantJson>,
}

pub(super) async fn list_grants(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> DocumentsResult<Json<GrantsJson>> {
    let who = caller(&state, &req_ctx).await?;
    let target = GrantTarget::Document(document_id(&id)?);
    let grants = state.service.grants(&who.user, &target, &who.trace).await?;
    Ok(Json(GrantsJson {
        grants: grants.into_iter().map(Into::into).collect(),
    }))
}

#[derive(Debug, Deserialize)]
pub(super) struct ShareBody {
    subject_kind: String,
    subject_value: String,
    level: AccessLevel,
    #[serde(default)]
    valid_until: Option<DateTime<Utc>>,
}

pub(super) async fn share(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<ShareBody>,
) -> DocumentsResult<Response> {
    let who = caller(&state, &req_ctx).await?;
    let display = subject_display(
        &state.user_repository,
        &body.subject_kind,
        &body.subject_value,
    )
    .await
    .map_err(ApiHttpError::from)?;
    let request = ShareRequest {
        target: GrantTarget::Document(document_id(&id)?),
        subject_kind: body.subject_kind,
        subject_value: body.subject_value,
        subject_display: Some(display),
        level: body.level,
        valid_until: body.valid_until,
    };
    let grant = state.service.share(&who.user, request, &who.trace).await?;
    Ok((StatusCode::CREATED, Json(GrantJson::from(grant))).into_response())
}

#[derive(Debug, Deserialize)]
pub(super) struct RevokeQuery {
    grant_id: Uuid,
}

pub(super) async fn revoke(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Query(query): Query<RevokeQuery>,
) -> DocumentsResult<StatusCode> {
    let who = caller(&state, &req_ctx).await?;
    let target = GrantTarget::Document(document_id(&id)?);
    state
        .service
        .revoke(&who.user, &target, query.grant_id, &who.trace)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub(super) struct PresenceBody {
    state: PresenceState,
    #[serde(default)]
    source_ref: Option<String>,
    #[serde(default)]
    // JSON: editor cursor position; the shape belongs to the client's editor.
    cursor: Option<serde_json::Value>,
}

pub(super) async fn touch_presence(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<PresenceBody>,
) -> DocumentsResult<StatusCode> {
    let who = caller(&state, &req_ctx).await?;
    let beacon = PresenceBeacon {
        document_id: document_id(&id)?,
        state: body.state,
        source_kind: PresenceSource::Web,
        source_ref: body
            .source_ref
            .unwrap_or_else(|| req_ctx.session_id().as_str().to_owned()),
        cursor: body.cursor,
    };
    state
        .service
        .touch_presence(&who.user, beacon, &who.trace)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize)]
pub(super) struct PresenceListJson {
    editors: Vec<PresenceJson>,
}

pub(super) async fn presence(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> DocumentsResult<Json<PresenceListJson>> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    let rows = state.service.presence(&who.user, &id, &who.trace).await?;
    Ok(Json(PresenceListJson {
        editors: rows.into_iter().map(Into::into).collect(),
    }))
}

#[derive(Debug, Deserialize)]
pub(super) struct LockBody {
    holder_id: String,
    #[serde(default)]
    ttl_secs: Option<u64>,
}

pub(super) async fn acquire_lock(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<LockBody>,
) -> DocumentsResult<Response> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    if body.holder_id.trim().is_empty() {
        return Err(ApiError::bad_request("holder_id must not be empty").into());
    }
    let ttl = body
        .ttl_secs
        .unwrap_or(DEFAULT_LOCK_SECS)
        .clamp(1, MAX_LOCK_SECS);
    let request = AcquireLockRequest {
        holder_kind: LockHolderKind::Editor,
        holder_id: body.holder_id,
        ttl: Duration::from_secs(ttl),
    };
    let outcome = state
        .service
        .acquire_lock(&who.user, &id, request, &who.trace)
        .await?;
    Ok(match outcome {
        LockOutcome::Acquired(lock) => Json(LockJson::from(lock)).into_response(),
        LockOutcome::Held { current } => {
            let error = ApiError::locked(format!(
                "document {id} is locked by {} holder {}; wait for the lock to be released or to \
                 expire",
                current.holder_kind, current.holder_id
            ))
            .with_error_key("lock_held");
            with_detail(
                &error,
                HeldDetail {
                    lock: LockJson::from(current),
                },
            )
        },
    })
}

#[derive(Debug, Serialize)]
struct HeldDetail {
    lock: LockJson,
}

#[derive(Debug, Deserialize)]
pub(super) struct ReleaseQuery {
    holder_id: String,
}

pub(super) async fn release_lock(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Query(query): Query<ReleaseQuery>,
) -> DocumentsResult<StatusCode> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    let released = state
        .service
        .release_lock(&who.user, &id, &query.holder_id, &who.trace)
        .await?;
    if released {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found(format!("document {id} holds no lock for that holder")).into())
    }
}
