//! Marks the named notifications, or all of them, read, and records the
//! named ones seen.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::State;
use axum::{Extension, Json};
use systemprompt_models::RequestContext;
use systemprompt_models::notifications::{MarkRead, MarkSeen};

use super::ops::{self, MarkReadResponse, SeenResponse};
use super::{NotificationsState, surface};
use crate::error::ApiHttpError;

pub(super) async fn mark_read(
    State(state): State<NotificationsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Json(body): Json<MarkRead>,
) -> Result<Json<MarkReadResponse>, ApiHttpError> {
    let outcome =
        ops::mark_read(&state.service, req_ctx.user_id(), body, surface(&req_ctx)).await?;
    Ok(Json(outcome))
}

pub(super) async fn mark_seen(
    State(state): State<NotificationsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Json(body): Json<MarkSeen>,
) -> Result<Json<SeenResponse>, ApiHttpError> {
    Ok(Json(
        ops::seen(&state.service, req_ctx.user_id(), body).await?,
    ))
}
