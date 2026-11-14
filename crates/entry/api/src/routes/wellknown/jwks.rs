//! `/.well-known/jwks.json` endpoint: the active token key and, during a
//! rotation overlap, the retired key until it expires.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use systemprompt_models::modules::ApiPaths;
use systemprompt_runtime::AppContext;

use crate::kernel::{ApiHttpError, RateLimitState, RouterExt};
use systemprompt_extension::LoaderError;

const JWKS_RATE_LIMIT_PER_SECOND: u64 = 2;

pub fn jwks_router(ctx: &AppContext) -> Result<Router, LoaderError> {
    let limits = RateLimitState::from_context(ctx);
    Router::new()
        .route(ApiPaths::WELLKNOWN_JWKS, get(handle_jwks))
        .with_state(ctx.clone())
        .with_rate_limit(&limits, JWKS_RATE_LIMIT_PER_SECOND, "wellknown_jwks")
}

async fn handle_jwks(State(ctx): State<AppContext>) -> Result<impl IntoResponse, ApiHttpError> {
    Ok(Json(ctx.signing_keys().jwks()))
}
