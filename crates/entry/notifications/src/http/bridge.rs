//! `/v1/bridge/notifications*` — the bridge's view of the user's inbox:
//! feed, mark seen, mark read, escalate, streams and subscriptions.
//!
//! The group is registered into the gateway surface with
//! `register_gateway_route!`: the gateway protects it handler-verified by its
//! credential under the `bridge_notifications` scope and the notifications
//! budget, inside its access log. Every handler authenticates the caller with
//! the [`GatewayCredential`] the gateway hands the factory, as
//! `/v1/bridge/stream` does, and calls the same operations as
//! `/api/v1/notifications`, recording the bridge as the surface.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use systemprompt_api::kernel::GatewayCredential;
use systemprompt_extension::ExtensionContext;
use systemprompt_identifiers::UserId;
use systemprompt_models::ApiPaths;
use systemprompt_models::notifications::{
    Escalate, MarkRead, MarkSeen, Notification, NotificationPage, SURFACE_BRIDGE, SubscriptionDecl,
};
use systemprompt_notifications::service::{NotificationsService, SubscriptionView};
use systemprompt_provider_contracts::MissingDependency;

use super::ops::{self, FeedParams, MarkReadResponse, SeenResponse, StreamList, SubscriptionList};
use super::{NotificationsHttpError, NotificationsState};

#[derive(Clone)]
struct BridgeNotificationsState {
    credential: GatewayCredential,
    service: Arc<NotificationsService>,
}

impl BridgeNotificationsState {
    async fn user(&self, headers: &HeaderMap) -> Result<UserId, NotificationsHttpError> {
        Ok(self.credential.user_id(headers).await?)
    }
}

pub(crate) fn routes(
    ctx: &dyn ExtensionContext,
    credential: GatewayCredential,
) -> Result<Router, MissingDependency> {
    let state = BridgeNotificationsState {
        credential,
        service: NotificationsState::from_extension_state(&ctx.extension_state())?.service,
    };
    Ok(Router::new()
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_NOTIFICATIONS),
            get(feed),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_NOTIFICATIONS_READ),
            post(mark_read),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_NOTIFICATIONS_SEEN),
            post(mark_seen),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_NOTIFICATIONS_ESCALATE_ROUTE),
            post(escalate),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_NOTIFICATIONS_STREAMS),
            get(streams),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_NOTIFICATIONS_SUBSCRIPTIONS),
            get(subscriptions).post(subscribe),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_NOTIFICATIONS_SUBSCRIPTION_ROUTE),
            delete(unsubscribe),
        )
        .with_state(state))
}

async fn feed(
    State(state): State<BridgeNotificationsState>,
    headers: HeaderMap,
    Query(params): Query<FeedParams>,
) -> Result<Json<NotificationPage>, NotificationsHttpError> {
    let user = state.user(&headers).await?;
    Ok(Json(ops::feed(&state.service, &user, params).await?))
}

async fn mark_read(
    State(state): State<BridgeNotificationsState>,
    headers: HeaderMap,
    Json(body): Json<MarkRead>,
) -> Result<Json<MarkReadResponse>, NotificationsHttpError> {
    let user = state.user(&headers).await?;
    Ok(Json(
        ops::mark_read(&state.service, &user, body, SURFACE_BRIDGE).await?,
    ))
}

async fn mark_seen(
    State(state): State<BridgeNotificationsState>,
    headers: HeaderMap,
    Json(body): Json<MarkSeen>,
) -> Result<Json<SeenResponse>, NotificationsHttpError> {
    let user = state.user(&headers).await?;
    Ok(Json(ops::seen(&state.service, &user, body).await?))
}

async fn escalate(
    State(state): State<BridgeNotificationsState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<Escalate>,
) -> Result<Json<Notification>, NotificationsHttpError> {
    let user = state.user(&headers).await?;
    Ok(Json(
        ops::escalate(&state.service, &user, &id, body, SURFACE_BRIDGE).await?,
    ))
}

async fn streams(
    State(state): State<BridgeNotificationsState>,
    headers: HeaderMap,
) -> Result<Json<StreamList>, NotificationsHttpError> {
    let user = state.user(&headers).await?;
    Ok(Json(ops::streams(&state.service, &user).await?))
}

async fn subscriptions(
    State(state): State<BridgeNotificationsState>,
    headers: HeaderMap,
) -> Result<Json<SubscriptionList>, NotificationsHttpError> {
    let user = state.user(&headers).await?;
    Ok(Json(ops::subscriptions(&state.service, &user).await?))
}

async fn subscribe(
    State(state): State<BridgeNotificationsState>,
    headers: HeaderMap,
    Json(body): Json<SubscriptionDecl>,
) -> Result<(StatusCode, Json<SubscriptionView>), NotificationsHttpError> {
    let user = state.user(&headers).await?;
    let view = ops::subscribe(&state.service, &user, body).await?;
    Ok((StatusCode::CREATED, Json(view)))
}

async fn unsubscribe(
    State(state): State<BridgeNotificationsState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, NotificationsHttpError> {
    let user = state.user(&headers).await?;
    ops::unsubscribe(&state.service, &user, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
