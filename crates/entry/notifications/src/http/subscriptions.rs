//! The caller's subscriptions: list, subscribe and unsubscribe.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use systemprompt_models::RequestContext;
use systemprompt_models::notifications::SubscriptionDecl;
use systemprompt_notifications::service::SubscriptionView;

use super::ops::{self, SubscriptionList};
use super::{NotificationsHttpError, NotificationsState};

pub(super) async fn list(
    State(state): State<NotificationsState>,
    Extension(req_ctx): Extension<RequestContext>,
) -> Result<Json<SubscriptionList>, NotificationsHttpError> {
    Ok(Json(
        ops::subscriptions(&state.service, req_ctx.user_id()).await?,
    ))
}

pub(super) async fn create(
    State(state): State<NotificationsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Json(body): Json<SubscriptionDecl>,
) -> Result<(StatusCode, Json<SubscriptionView>), NotificationsHttpError> {
    let view = ops::subscribe(&state.service, req_ctx.user_id(), body).await?;
    Ok((StatusCode::CREATED, Json(view)))
}

pub(super) async fn remove(
    State(state): State<NotificationsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> Result<StatusCode, NotificationsHttpError> {
    ops::unsubscribe(&state.service, req_ctx.user_id(), &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
