//! The bridge Drive routes that carry a local delete or rename to the server.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use systemprompt_files::documents::agents::act_as;
use systemprompt_identifiers::TraceId;
use systemprompt_models::api::ApiError;
use systemprompt_models::documents::{DriveMoveRequest, DriveTrashRequest, DriveTrashResponse};

use super::drive::DriveState;
use super::drive_token::device_user;
use super::error::DocumentsResult;
use super::headers::base_revision;

pub(super) async fn trash(
    State(state): State<DriveState>,
    headers: HeaderMap,
    Json(body): Json<DriveTrashRequest>,
) -> DocumentsResult<Response> {
    let (who, actor) = device_user(&state, &headers).await?;
    let trace = TraceId::generate();
    let base = base_revision(&headers)?
        .ok_or_else(|| ApiError::bad_request("a Drive trash needs If-Match"))?;
    let outcome = act_as(
        actor,
        state
            .service
            .drive_trash(&who, &body.document_id, &base, &trace),
    )
    .await?;
    let status = match outcome {
        DriveTrashResponse::Trashed => StatusCode::OK,
        DriveTrashResponse::Conflict { .. } => StatusCode::CONFLICT,
    };
    Ok((status, Json(outcome)).into_response())
}

pub(super) async fn move_document(
    State(state): State<DriveState>,
    headers: HeaderMap,
    Json(body): Json<DriveMoveRequest>,
) -> DocumentsResult<StatusCode> {
    let (who, actor) = device_user(&state, &headers).await?;
    let trace = TraceId::generate();
    act_as(
        actor,
        state
            .service
            .drive_move(&who, &body.document_id, &body.path, &trace),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
