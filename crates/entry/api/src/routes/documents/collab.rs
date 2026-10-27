//! `/api/v1/documents/{id}/collab/*`: live co-editing of HTML and Markdown
//! documents.
//!
//! Clients join a document's edit session, post Loro updates and leave;
//! everyone's updates reach them as `document.collab.update` events on
//! their existing AG-UI stream. Binary payloads (snapshots, updates and
//! version vectors) travel as standard base64.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use systemprompt_files::documents::collab::{ApplyRequest, CollabError, JoinMode};
use systemprompt_identifiers::{DocumentEditSessionId, DocumentRevisionId};
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;

use super::DocumentsState;
use super::caller::caller;
use super::error::{DocumentsHttpError, DocumentsResult};
use super::items::document_id;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ModeJson {
    Edit,
    View,
}

#[derive(Debug, Deserialize)]
pub(super) struct JoinBody {
    mode: ModeJson,
}

#[derive(Debug, Serialize)]
pub(super) struct JoinedJson {
    session_id: DocumentEditSessionId,
    snapshot_b64: String,
    vv: String,
    head_revision: Option<DocumentRevisionId>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpdateBody {
    session_id: String,
    vv: Option<String>,
    update_b64: String,
}

#[derive(Debug, Serialize)]
pub(super) struct AppliedJson {
    seq: i64,
    vv: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    missing_b64: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct LeaveBody {
    session_id: String,
}

#[derive(Debug, Serialize)]
pub(super) struct LeftJson {
    closed: bool,
    checkpoint_revision: Option<DocumentRevisionId>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CursorBody {
    session_id: String,
    // JSON: editor cursor position; the shape belongs to the client's editor.
    cursor: serde_json::Value,
}

pub(super) async fn join(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<JoinBody>,
) -> DocumentsResult<Json<JoinedJson>> {
    let who = caller(&state, &req_ctx).await?;
    let mode = match body.mode {
        ModeJson::Edit => JoinMode::Edit,
        ModeJson::View => JoinMode::View,
    };
    let joined = state
        .collab
        .join(&who.user, &document_id(&id)?, mode, &who.trace)
        .await?;
    Ok(Json(JoinedJson {
        session_id: joined.session_id,
        snapshot_b64: STANDARD.encode(joined.snapshot),
        vv: STANDARD.encode(joined.version),
        head_revision: joined.head_revision,
    }))
}

pub(super) async fn updates(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<UpdateBody>,
) -> DocumentsResult<Json<AppliedJson>> {
    let who = caller(&state, &req_ctx).await?;
    let request = ApplyRequest {
        document_id: document_id(&id)?,
        session_id: DocumentEditSessionId::try_new(body.session_id)?,
        version: body.vv.as_deref().map(decode).transpose()?,
        update: decode(&body.update_b64)?,
    };
    let applied = state.collab.apply(&who.user, request, &who.trace).await?;
    Ok(Json(AppliedJson {
        seq: applied.seq,
        vv: STANDARD.encode(applied.version),
        missing_b64: applied.missing.map(|bytes| STANDARD.encode(bytes)),
    }))
}

pub(super) async fn leave(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<LeaveBody>,
) -> DocumentsResult<Json<LeftJson>> {
    let who = caller(&state, &req_ctx).await?;
    let left = state
        .collab
        .leave(
            &who.user,
            &document_id(&id)?,
            &DocumentEditSessionId::try_new(body.session_id)?,
            &who.trace,
        )
        .await?;
    Ok(Json(LeftJson {
        closed: left.closed,
        checkpoint_revision: left.checkpoint,
    }))
}

pub(super) async fn cursor(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<CursorBody>,
) -> DocumentsResult<StatusCode> {
    let who = caller(&state, &req_ctx).await?;
    state
        .collab
        .cursor(
            &who.user,
            &document_id(&id)?,
            &DocumentEditSessionId::try_new(body.session_id)?,
            body.cursor,
            &who.trace,
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

fn decode(raw: &str) -> DocumentsResult<Vec<u8>> {
    STANDARD.decode(raw).map_err(|error| {
        ApiError::bad_request("payload is not valid base64")
            .with_source(error)
            .into()
    })
}

impl From<CollabError> for DocumentsHttpError {
    fn from(error: CollabError) -> Self {
        let message = error.to_string();
        match error {
            CollabError::Documents(error) => Self::from(error),
            CollabError::SessionClosed { .. } => {
                Self::from(ApiError::conflict(message).with_error_key("session_closed"))
            },
            CollabError::NotJoined { .. } => {
                Self::from(ApiError::conflict(message).with_error_key("not_joined"))
            },
            CollabError::NotEditing { .. } => {
                Self::from(ApiError::forbidden(message).with_error_key("not_editing"))
            },
            CollabError::InvalidUpdate(source) | CollabError::InvalidVersion(source) => {
                Self::from(ApiError::bad_request(message).with_source(source))
            },
            CollabError::NotText { .. } => Self::from(ApiError::bad_request(message)),
            other => Self::from(ApiError::internal("live edit session failed", other)),
        }
    }
}
