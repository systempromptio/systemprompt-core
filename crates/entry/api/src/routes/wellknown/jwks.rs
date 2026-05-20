//! `/.well-known/jwks.json` endpoint for the deployment's signing key.

use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use systemprompt_models::api::ApiError;
use systemprompt_models::modules::ApiPaths;
use systemprompt_runtime::AppContext;
use systemprompt_security::keys::{Jwks, authority};

use crate::services::middleware::RouterExt;

const JWKS_RATE_LIMIT_PER_SECOND: u64 = 2;

pub fn jwks_router(ctx: &AppContext) -> Router {
    Router::new()
        .route(ApiPaths::WELLKNOWN_JWKS, get(handle_jwks))
        .with_state(ctx.clone())
        .with_rate_limit(&ctx.config().rate_limits, JWKS_RATE_LIMIT_PER_SECOND)
}

async fn handle_jwks(State(_ctx): State<AppContext>) -> Result<impl IntoResponse, ApiError> {
    let jwks = match authority::signing_key() {
        Ok(key) => key.jwks(),
        Err(err) => {
            tracing::warn!(error = %err, "failed to load signing key for JWKS endpoint");
            Jwks { keys: vec![] }
        },
    };
    Ok(Json(jwks))
}
