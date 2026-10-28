//! `/api/v1/documents/{id}/collab/*`: live co-editing of HTML and Markdown
//! documents.
//!
//! Protocol v2. Clients join a document's edit session (`edit`, `comment`
//! or `view`) and learn the engine, wire version, model and access level to
//! bind to; they post engine updates, each validated on a server-side fork
//! and refused as `update_rejected` with a `reason` (resync from a fresh
//! join), and leave. Everyone's updates reach them as
//! `document.collab.update` events on their existing AG-UI stream. Binary
//! payloads (snapshots, updates and version vectors) travel as standard
//! base64.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use systemprompt_files::documents::collab::{
    ApplyRequest, Checkpoint, CollabError, CursorRequest, JoinMode, Left,
};
use systemprompt_identifiers::{DocumentEditSessionId, DocumentProposalId, DocumentRevisionId};
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;
use systemprompt_models::documents::AccessLevel;

use super::DocumentsState;
use super::caller::caller;
use super::error::{DocumentsHttpError, DocumentsResult};
use super::items::document_id;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ModeJson {
    Edit,
    Comment,
    View,
}

#[derive(Debug, Deserialize)]
pub(super) struct JoinBody {
    mode: ModeJson,
}

/// Protocol v2 join response: the session state plus the engine, wire
/// version, model and access level the client binds its editor to.
#[derive(Debug, Serialize)]
pub(super) struct JoinedJson {
    protocol: u32,
    session_id: DocumentEditSessionId,
    snapshot_b64: String,
    vv: String,
    seq: i64,
    head_revision: Option<DocumentRevisionId>,
    engine: &'static str,
    wire_version: &'static str,
    model: &'static str,
    model_version: u32,
    level: AccessLevel,
}

const PROTOCOL_VERSION: u32 = 2;

#[derive(Debug, Deserialize)]
pub(super) struct UpdateBody {
    session_id: DocumentEditSessionId,
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
    session_id: DocumentEditSessionId,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum CheckpointKind {
    Unchanged,
    Committed,
    Conflict,
}

#[derive(Debug, Serialize)]
pub(super) struct LeftJson {
    closed: bool,
    checkpoint: CheckpointKind,
    checkpoint_revision: Option<DocumentRevisionId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    head_revision: Option<DocumentRevisionId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    proposal_id: Option<DocumentProposalId>,
}

impl From<Left> for LeftJson {
    fn from(left: Left) -> Self {
        let (checkpoint, checkpoint_revision, head_revision, proposal_id) = match left.checkpoint {
            Checkpoint::Unchanged => (CheckpointKind::Unchanged, None, None, None),
            Checkpoint::Committed(revision) => {
                (CheckpointKind::Committed, Some(revision), None, None)
            },
            Checkpoint::Conflict { head, proposal } => {
                (CheckpointKind::Conflict, None, Some(head), proposal)
            },
        };
        Self {
            closed: left.closed,
            checkpoint,
            checkpoint_revision,
            head_revision,
            proposal_id,
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct CursorBody {
    session_id: DocumentEditSessionId,
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
        ModeJson::Comment => JoinMode::Comment,
        ModeJson::View => JoinMode::View,
    };
    let joined = state
        .collab
        .join(&who.user, &document_id(&id)?, mode, &who.trace)
        .await?;
    Ok(Json(JoinedJson {
        protocol: PROTOCOL_VERSION,
        session_id: joined.session_id,
        snapshot_b64: STANDARD.encode(joined.snapshot),
        vv: STANDARD.encode(joined.version),
        seq: joined.seq,
        head_revision: joined.head_revision,
        engine: joined.engine,
        wire_version: joined.wire_version,
        model: joined.model,
        model_version: joined.model_version,
        level: joined.level,
    }))
}

pub(super) async fn updates(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<UpdateBody>,
) -> DocumentsResult<Json<AppliedJson>> {
    let who = caller(&state, &req_ctx).await?;
    let update = decode(&body.update_b64)?;
    state.service.check_collab_update_limit(update.len())?;
    let request = ApplyRequest {
        document_id: document_id(&id)?,
        session_id: body.session_id,
        version: body.vv.as_deref().map(decode).transpose()?,
        update,
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
        .leave(&who.user, &document_id(&id)?, &body.session_id, &who.trace)
        .await?;
    Ok(Json(LeftJson::from(left)))
}

pub(super) async fn cursor(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<CursorBody>,
) -> DocumentsResult<StatusCode> {
    let who = caller(&state, &req_ctx).await?;
    state
        .service
        .check_cursor_limit(body.cursor.to_string().len())?;
    state
        .collab
        .cursor(
            &who.user,
            CursorRequest {
                document: document_id(&id)?,
                session: body.session_id,
                cursor: body.cursor,
            },
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
        collab_error(error)
    }
}

fn collab_error(error: CollabError) -> DocumentsHttpError {
    type Http = DocumentsHttpError;
    let message = error.to_string();
    match error {
        CollabError::Documents(error) => Http::from(error),
        CollabError::SessionClosed { .. } => {
            Http::from(ApiError::conflict(message).with_error_key("session_closed"))
        },
        CollabError::NotJoined { .. } => {
            Http::from(ApiError::conflict(message).with_error_key("not_joined"))
        },
        CollabError::NotEditing { .. } => {
            Http::from(ApiError::forbidden(message).with_error_key("not_editing"))
        },
        CollabError::InvalidUpdate(source) | CollabError::InvalidVersion(source) => {
            Http::from(ApiError::bad_request(message).with_source(source))
        },
        CollabError::UpdateRejected(rejection) => Http::from(
            ApiError::bad_request(message)
                .with_error_key("update_rejected")
                .with_details(rejection.reason()),
        ),
        CollabError::NotText { .. } => Http::from(ApiError::bad_request(message)),
        CollabError::EngineUnavailable => Http::from(
            ApiError::service_unavailable("no live session engine is enabled")
                .with_error_key("collab_engine_unavailable"),
        ),
        other => Http::from(ApiError::internal("live edit session failed", other)),
    }
}
