//! `/api/v1/notifications`, `/api/v1/admin/notifications` and the bridge
//! inbox under `/v1/bridge/notifications` over HTTP.
//!
//! They serve a user's notification feed, seen, read and escalation state,
//! streams, subscriptions and personal rules, and the admin routing
//! simulator.
//!
//! Each handler resolves the caller and calls one operation of the shared
//! [`ops`] layer, which the bridge routes in `bridge` call too, so both
//! surfaces answer with the same shapes. The service
//! authorizes and records every call against the surface it names: the
//! dashboard for a first-party browser session, the API otherwise.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod admin;
pub(crate) mod bridge;
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

use axum::Router;
use axum::routing::{delete, get, post};
use systemprompt_models::RequestContext;
use systemprompt_models::notifications::{SURFACE_API, SURFACE_DASHBOARD};
use systemprompt_notifications::service::{NotificationsService, NotificationsServiceSlot};
use systemprompt_provider_contracts::{Dependencies, MissingDependency};

/// Router-scoped state of the notification routes: the one notification
/// service, read from the host-bound slot in the extension state.
#[derive(Clone)]
pub struct NotificationsState {
    pub(crate) service: Arc<NotificationsService>,
}

impl std::fmt::Debug for NotificationsState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotificationsState").finish_non_exhaustive()
    }
}

impl NotificationsState {
    pub fn from_extension_state(state: &Dependencies) -> Result<Self, MissingDependency> {
        let service = state
            .get::<NotificationsServiceSlot>()?
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

pub fn router(state: NotificationsState) -> Router {
    Router::new()
        .route("/", get(feed::feed))
        .route("/unread", get(feed::unread))
        .route("/read", post(read::mark_read))
        .route("/seen", post(read::mark_seen))
        .route("/{id}/escalate", post(escalate::escalate))
        .route("/streams", get(streams::list))
        .route("/streams/{id}/publish", post(streams::publish))
        .route(
            "/subscriptions",
            get(subscriptions::list).post(subscriptions::create),
        )
        .route("/subscriptions/{id}", delete(subscriptions::remove))
        .route("/rules", get(rules::list).put(rules::replace))
        .with_state(state)
}

pub fn admin_router(state: NotificationsState) -> Router {
    Router::new()
        .route("/simulate", post(admin::simulate))
        .with_state(state)
}
