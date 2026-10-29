//! Raises one notification to the person's attention with a note.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, State};
use axum::{Extension, Json};
use systemprompt_models::RequestContext;
use systemprompt_models::notifications::{Escalate, Notification};

use super::{NotificationsState, ops, surface};
use crate::error::ApiHttpError;

pub(super) async fn escalate(
    State(state): State<NotificationsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<Escalate>,
) -> Result<Json<Notification>, ApiHttpError> {
    let escalated = ops::escalate(
        &state.service,
        req_ctx.user_id(),
        &id,
        body,
        surface(&req_ctx),
    )
    .await?;
    Ok(Json(escalated))
}
