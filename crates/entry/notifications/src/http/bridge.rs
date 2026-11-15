//! `/v1/bridge/notifications*` — the bridge's view of the user's inbox:
//! feed, mark seen, mark read, escalate, streams and subscriptions.
//!
//! The set is the member's one gateway route: the gateway verifies the
//! bridge credential under the `bridge_notifications` scope, the
//! notifications budget and the bearer window, inside its access log, and
//! attaches the caller's [`GatewayPrincipal`] to the request. Every handler
//! reads the caller from that principal and calls the same operations as
//! `/api/v1/notifications`, recording the bridge as the surface.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use systemprompt_extension::router::GatewayPrincipal;
use systemprompt_models::notifications::{
    Escalate, MarkRead, MarkSeen, Notification, NotificationPage, SURFACE_BRIDGE, SubscriptionDecl,
};
use systemprompt_notifications::service::SubscriptionView;

use super::ops::{self, FeedParams, MarkReadResponse, SeenResponse, StreamList, SubscriptionList};
use super::{NotificationsHttpError, NotificationsState};

pub(super) async fn feed(
    State(state): State<NotificationsState>,
    Extension(principal): Extension<GatewayPrincipal>,
    Query(params): Query<FeedParams>,
) -> Result<Json<NotificationPage>, NotificationsHttpError> {
    let user = principal.user_id();
    Ok(Json(ops::feed(&state.service, user, params).await?))
}

pub(super) async fn mark_read(
    State(state): State<NotificationsState>,
    Extension(principal): Extension<GatewayPrincipal>,
    Json(body): Json<MarkRead>,
) -> Result<Json<MarkReadResponse>, NotificationsHttpError> {
    let user = principal.user_id();
    Ok(Json(
        ops::mark_read(&state.service, user, body, SURFACE_BRIDGE).await?,
    ))
}

pub(super) async fn mark_seen(
    State(state): State<NotificationsState>,
    Extension(principal): Extension<GatewayPrincipal>,
    Json(body): Json<MarkSeen>,
) -> Result<Json<SeenResponse>, NotificationsHttpError> {
    let user = principal.user_id();
    Ok(Json(ops::seen(&state.service, user, body).await?))
}

pub(super) async fn escalate(
    State(state): State<NotificationsState>,
    Extension(principal): Extension<GatewayPrincipal>,
    Path(id): Path<String>,
    Json(body): Json<Escalate>,
) -> Result<Json<Notification>, NotificationsHttpError> {
    let user = principal.user_id();
    Ok(Json(
        ops::escalate(&state.service, user, &id, body, SURFACE_BRIDGE).await?,
    ))
}

pub(super) async fn streams(
    State(state): State<NotificationsState>,
    Extension(principal): Extension<GatewayPrincipal>,
) -> Result<Json<StreamList>, NotificationsHttpError> {
    let user = principal.user_id();
    Ok(Json(ops::streams(&state.service, user).await?))
}

pub(super) async fn subscriptions(
    State(state): State<NotificationsState>,
    Extension(principal): Extension<GatewayPrincipal>,
) -> Result<Json<SubscriptionList>, NotificationsHttpError> {
    let user = principal.user_id();
    Ok(Json(ops::subscriptions(&state.service, user).await?))
}

pub(super) async fn subscribe(
    State(state): State<NotificationsState>,
    Extension(principal): Extension<GatewayPrincipal>,
    Json(body): Json<SubscriptionDecl>,
) -> Result<(StatusCode, Json<SubscriptionView>), NotificationsHttpError> {
    let user = principal.user_id();
    let view = ops::subscribe(&state.service, user, body).await?;
    Ok((StatusCode::CREATED, Json(view)))
}

pub(super) async fn unsubscribe(
    State(state): State<NotificationsState>,
    Extension(principal): Extension<GatewayPrincipal>,
    Path(id): Path<String>,
) -> Result<StatusCode, NotificationsHttpError> {
    let user = principal.user_id();
    ops::unsubscribe(&state.service, user, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
