//! `/api/v1/brain` and `/api/v1/admin/brain` over HTTP.
//!
//! The user routes search, read, enter and govern facts, resolve disputes
//! and decide proposals; the admin route simulates a recall for a chosen
//! principal without recording it. Each handler resolves the caller's
//! current roles and calls one operation of the brain service, which
//! authorizes it. With no [`BrainRoutes`], or a profile whose brain is
//! disabled, every path answers 404.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod admin;
mod dto;
mod error;
mod facts;
mod lifecycle;
mod proposals;

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, patch, post};
use systemprompt_brain::service::{BrainCaller, BrainService};
use systemprompt_extension::LoaderError;
use systemprompt_extension::brain::MemoryRecall;
use systemprompt_identifiers::UserId;
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_models::api::ApiError;
use systemprompt_models::modules::ApiPaths;
use systemprompt_traits::UserProvider;

pub use dto::{
    DisputeBody, DisputeCreated, EnterAccepted, FieldsBody, LimitParams, ResolveBody,
    ResolveOutcome, RetractBody, SearchParams, SimulateBody, SimulateResponse, WhyParams,
};
pub use error::BrainHttpError;

use crate::error::ApiHttpError;
use crate::services::middleware::authz::AuthzPolicy;
use crate::services::middleware::{RateLimitState, RouterExt, UserOnlyContextMiddleware};

/// Router-scoped state of the brain routes: the brain service, the recall
/// the admin simulator runs, the brain profile and the user provider that
/// resolves a caller's current roles.
#[derive(Clone)]
pub struct BrainRoutes {
    pub(crate) service: Arc<BrainService>,
    pub(crate) recall: Arc<dyn MemoryRecall>,
    pub(crate) profile: Arc<BrainProfile>,
    pub(crate) users: Arc<dyn UserProvider>,
}

impl std::fmt::Debug for BrainRoutes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrainRoutes").finish_non_exhaustive()
    }
}

impl BrainRoutes {
    #[must_use]
    pub fn new(
        service: Arc<BrainService>,
        recall: Arc<dyn MemoryRecall>,
        profile: Arc<BrainProfile>,
        users: Arc<dyn UserProvider>,
    ) -> Self {
        Self {
            service,
            recall,
            profile,
            users,
        }
    }

    async fn caller(&self, user_id: &UserId) -> Result<BrainCaller, BrainHttpError> {
        let found = self
            .users
            .find_by_id(user_id)
            .await
            .map_err(|source| ApiError::internal("user lookup failed", source))?;
        match found {
            Some(user) if user.is_active => Ok(BrainCaller::new(user.id, user.roles)),
            _ => Err(ApiHttpError::unauthorized("account is not active").into()),
        }
    }
}

fn enabled(routes: Option<BrainRoutes>) -> Option<BrainRoutes> {
    routes.filter(|routes| routes.profile.enabled)
}

async fn disabled() -> BrainHttpError {
    ApiError::not_found("the brain is not enabled")
        .with_error_key("brain_disabled")
        .into()
}

pub fn router(routes: Option<BrainRoutes>) -> Router {
    let Some(state) = enabled(routes) else {
        return Router::new().fallback(disabled);
    };
    Router::new()
        .route("/facts", get(facts::search).post(facts::enter))
        .route("/facts/{id}", get(facts::get))
        .route("/facts/{id}/timeline", get(facts::timeline))
        .route("/facts/{id}/why", get(facts::why))
        .route("/subjects/{kind}/{id}", get(facts::subject))
        .route("/facts/{id}/confirm", post(lifecycle::confirm))
        .route("/facts/{id}/dispute", post(lifecycle::dispute))
        .route("/facts/{id}/retract", post(lifecycle::retract))
        .route("/facts/{id}/erase", post(lifecycle::erase))
        .route("/facts/{id}/pin", post(lifecycle::pin))
        .route("/facts/{id}/unpin", post(lifecycle::unpin))
        .route("/facts/{id}/fields", patch(lifecycle::edit_fields))
        .route("/disputes/{id}/resolve", post(lifecycle::resolve))
        .route("/proposals", get(proposals::list))
        .route("/proposals/{id}/approve", post(proposals::approve))
        .route("/proposals/{id}/reject", post(proposals::reject))
        .with_state(state)
}

pub fn admin_router(routes: Option<BrainRoutes>) -> Router {
    let Some(state) = enabled(routes) else {
        return Router::new().fallback(disabled);
    };
    Router::new()
        .route("/simulate", post(admin::simulate))
        .with_state(state)
}

pub fn mount_brain(
    router_in: Router,
    limits: &RateLimitState,
    user_middleware: &UserOnlyContextMiddleware,
    per_second: u64,
    routes: Option<BrainRoutes>,
) -> Result<Router, LoaderError> {
    Ok(router_in
        .nest(
            ApiPaths::BRAIN,
            router(routes.clone())
                .with_rate_limit(limits, per_second, "brain")?
                .with_auth(user_middleware.clone(), AuthzPolicy::user()),
        )
        .nest(
            ApiPaths::ADMIN_BRAIN,
            admin_router(routes)
                .with_rate_limit(limits, per_second, "brain")?
                .with_auth(user_middleware.clone(), AuthzPolicy::admin()),
        ))
}
