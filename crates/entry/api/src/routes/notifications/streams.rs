//! The streams the caller may see, and publishing to one. A publish is
//! enqueued for routing, so it answers 202.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use systemprompt_identifiers::NotificationStreamId;
use systemprompt_models::RequestContext;
use systemprompt_notifications::service::{PublishOutcome, PublishRequest};

use super::NotificationsState;
use super::ops::{self, StreamList};
use crate::error::ApiHttpError;

pub(super) async fn list(
    State(state): State<NotificationsState>,
    Extension(req_ctx): Extension<RequestContext>,
) -> Result<Json<StreamList>, ApiHttpError> {
    Ok(Json(ops::streams(&state.service, req_ctx.user_id()).await?))
}

pub(super) async fn publish(
    State(state): State<NotificationsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<PublishRequest>,
) -> Result<(StatusCode, Json<PublishOutcome>), ApiHttpError> {
    let stream = NotificationStreamId::try_new(id)?;
    let outcome = state
        .service
        .publish(req_ctx.user_id(), &stream, body)
        .await?;
    Ok((StatusCode::ACCEPTED, Json(outcome)))
}
