//! The handlers of `/api/v1/notifications`, `/api/v1/admin/notifications`
//! and the bridge inbox under `/v1/bridge/notifications`.
//!
//! They serve a user's notification feed, seen, read and escalation state,
//! streams, subscriptions and personal rules, and the admin routing
//! simulator.
//!
//! [`user_routes`], [`admin_routes`] and [`bridge_routes`] are the three
//! route sets the member contributes, each with its conferred shape; the
//! bridge set is placed under the gateway, which attaches the caller's
//! principal.
//!
//! Each handler resolves the caller and calls one operation of the shared
//! [`ops`] layer, which the bridge handlers in `bridge` call too, so both
//! surfaces answer with the same shapes. The service
//! authorizes and records every call against the surface it names: the
//! dashboard for a first-party browser session, the API otherwise.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod admin;
mod bridge;
mod error;
mod escalate;
mod feed;
pub mod ops;
mod read;
mod rules;
mod streams;
mod subscriptions;

pub use error::NotificationsHttpError;

use std::sync::Arc;

use axum::http::Method;
use systemprompt_extension::prelude::*;
use systemprompt_extension::router::{
    ApiBase, BasePath, BodyLimit, GatewayPath, RouteLimits, RouteSet, RouteSpec,
};
use systemprompt_extension::{HandlerVerifier, RouteGroup};
use systemprompt_models::RequestContext;
use systemprompt_models::modules::ApiPaths;
use systemprompt_models::notifications::{SURFACE_API, SURFACE_DASHBOARD};
use systemprompt_notifications::service::{NotificationsService, NotificationsServiceSlot};

/// State of every notification route set: the one notification service,
/// read from the late-bound handle the composition root binds.
#[derive(Clone)]
pub struct NotificationsState {
    pub(crate) service: Arc<NotificationsService>,
}

impl std::fmt::Debug for NotificationsState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotificationsState").finish_non_exhaustive()
    }
}

impl FromState for NotificationsState {
    fn from_state(r: &Resolved<'_>) -> Result<Self, MissingDependency> {
        let slot = r.get::<Arc<NotificationsServiceSlot>>()?;
        let service = slot
            .get()
            .ok_or_else(MissingDependency::of::<NotificationsService>)?;
        Ok(Self {
            service: Arc::clone(service),
        })
    }
}

const fn surface(req_ctx: &RequestContext) -> &'static str {
    if req_ctx.is_browser_session() {
        SURFACE_DASHBOARD
    } else {
        SURFACE_API
    }
}

const LIMITS: RouteLimits = RouteLimits {
    rate: RateLimit::new("notifications", RateBudget::Notifications),
    body: BodyLimit::Default,
};
pub const BRIDGE_LIMITS: RouteLimits = RouteLimits {
    rate: RateLimit::new("bridge_notifications", RateBudget::Notifications).with_bearer_window(),
    body: BodyLimit::Default,
};
const USER: RouteSpec = RouteSpec::new(AuthzPolicy::user(), ContextKind::User, LIMITS);
const ADMIN: RouteSpec = RouteSpec::new(AuthzPolicy::admin(), ContextKind::User, LIMITS);
const BRIDGE: RouteSpec = RouteSpec::new(
    AuthzPolicy::handler_verified(HandlerVerifier::GatewayCredential),
    ContextKind::HandlerResolved,
    BRIDGE_LIMITS,
);

#[must_use]
pub fn user_routes() -> RouteSet {
    RouteSet::at::<NotificationsState>(
        BasePath::Api(ApiBase::Notifications),
        RouteGroup::ContentAndMisc,
    )
    .route(Method::GET, "/", feed::feed, USER)
    .route(Method::GET, "/unread", feed::unread, USER)
    .route(Method::POST, "/read", read::mark_read, USER)
    .route(Method::POST, "/seen", read::mark_seen, USER)
    .route(Method::POST, "/{id}/escalate", escalate::escalate, USER)
    .route(Method::GET, "/streams", streams::list, USER)
    .route(
        Method::POST,
        "/streams/{id}/publish",
        streams::publish,
        USER,
    )
    .route(Method::GET, "/subscriptions", subscriptions::list, USER)
    .route(Method::POST, "/subscriptions", subscriptions::create, USER)
    .route(
        Method::DELETE,
        "/subscriptions/{id}",
        subscriptions::remove,
        USER,
    )
    .route(Method::GET, "/rules", rules::list, USER)
    .route(Method::PUT, "/rules", rules::replace, USER)
    .build()
}

#[must_use]
pub fn admin_routes() -> RouteSet {
    RouteSet::at::<NotificationsState>(
        BasePath::Api(ApiBase::AdminNotifications),
        RouteGroup::ContentAndMisc,
    )
    .route(Method::POST, "/simulate", admin::simulate, ADMIN)
    .build()
}

#[must_use]
pub fn bridge_routes() -> RouteSet {
    let at = ApiPaths::gateway_relative;
    RouteSet::gateway::<NotificationsState>(GatewayPath::BridgeNotifications, RouteGroup::Gateway)
        .route(
            Method::GET,
            at(ApiPaths::BRIDGE_NOTIFICATIONS),
            bridge::feed,
            BRIDGE,
        )
        .route(
            Method::POST,
            at(ApiPaths::BRIDGE_NOTIFICATIONS_READ),
            bridge::mark_read,
            BRIDGE,
        )
        .route(
            Method::POST,
            at(ApiPaths::BRIDGE_NOTIFICATIONS_SEEN),
            bridge::mark_seen,
            BRIDGE,
        )
        .route(
            Method::POST,
            at(ApiPaths::BRIDGE_NOTIFICATIONS_ESCALATE_ROUTE),
            bridge::escalate,
            BRIDGE,
        )
        .route(
            Method::GET,
            at(ApiPaths::BRIDGE_NOTIFICATIONS_STREAMS),
            bridge::streams,
            BRIDGE,
        )
        .route(
            Method::GET,
            at(ApiPaths::BRIDGE_NOTIFICATIONS_SUBSCRIPTIONS),
            bridge::subscriptions,
            BRIDGE,
        )
        .route(
            Method::POST,
            at(ApiPaths::BRIDGE_NOTIFICATIONS_SUBSCRIPTIONS),
            bridge::subscribe,
            BRIDGE,
        )
        .route(
            Method::DELETE,
            at(ApiPaths::BRIDGE_NOTIFICATIONS_SUBSCRIPTION_ROUTE),
            bridge::unsubscribe,
            BRIDGE,
        )
        .build()
}
