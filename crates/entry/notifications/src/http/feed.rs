//! The caller's notification feed, newest first and paged by keyset cursor,
//! and the unread count.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Query, State};
use axum::{Extension, Json};
use systemprompt_models::RequestContext;
use systemprompt_models::notifications::NotificationPage;

use super::ops::{self, FeedParams, UnreadCount};
use super::{NotificationsHttpError, NotificationsState};

pub(super) async fn feed(
    State(state): State<NotificationsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Query(params): Query<FeedParams>,
) -> Result<Json<NotificationPage>, NotificationsHttpError> {
    let page = ops::feed(&state.service, req_ctx.user_id(), params).await?;
    Ok(Json(page))
}

pub(super) async fn unread(
    State(state): State<NotificationsState>,
    Extension(req_ctx): Extension<RequestContext>,
) -> Result<Json<UnreadCount>, NotificationsHttpError> {
    Ok(Json(ops::unread(&state.service, req_ctx.user_id()).await?))
}
