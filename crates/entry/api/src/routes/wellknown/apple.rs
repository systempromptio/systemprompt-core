//! `/.well-known/apple-app-site-association`, the document by which
//! the instance's domain vouches for its iOS apps.
//!
//! The document is a pure function of the instance's declared
//! [`VouchedApps`](systemprompt_models::association::VouchedApps): no part of
//! the request reaches it. It is unauthenticated because the vendor's
//! verifier fetches it with no credential, and cacheable because it changes
//! only when configuration does. An instance that declares no app of this
//! vendor serves no document and answers 404.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::State;
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use systemprompt_extension::LoaderError;
use systemprompt_loader::ServicesState;
use systemprompt_manifest::services::ServicesConfig;
use systemprompt_models::api::ApiError;
use systemprompt_models::association::AppleAppSiteAssociation;
use systemprompt_models::modules::ApiPaths;
use systemprompt_runtime::AppContext;

use super::{ASSOCIATION_CACHE_CONTROL, ASSOCIATION_RATE_LIMIT_PER_SECOND};
use crate::kernel::{ApiHttpError, RateLimitState, RouterExt};

pub fn apple_app_site_association_router(ctx: &AppContext) -> Result<Router, LoaderError> {
    let limits = RateLimitState::from_context(ctx)?;
    Router::new()
        .route(ApiPaths::WELLKNOWN_APPLE_APP_SITE_ASSOCIATION, get(handle))
        .with_state(ctx.clone())
        .with_rate_limit(
            &limits,
            ASSOCIATION_RATE_LIMIT_PER_SECOND,
            "wellknown_apple_app_site_association",
        )
}

async fn handle(State(ctx): State<AppContext>) -> Result<impl IntoResponse, ApiHttpError> {
    let services = ctx.kernel().state().get::<ServicesState>()?.config();
    let document = apple_app_site_association(&services).ok_or_else(|| {
        ApiError::not_found("this instance vouches for no iOS app")
            .with_error_key("apple_app_site_association_not_configured")
    })?;
    Ok((
        [(header::CACHE_CONTROL, ASSOCIATION_CACHE_CONTROL)],
        Json(document),
    ))
}

#[must_use]
pub fn apple_app_site_association(services: &ServicesConfig) -> Option<AppleAppSiteAssociation> {
    services
        .gateway_config()?
        .vouched_apps()?
        .apple_app_site_association()
}
