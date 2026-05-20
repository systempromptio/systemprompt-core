//! `/.well-known/jwks.json` endpoint for the deployment's signing key.
//!
//! The Jwk is loaded lazily from the configured signing-key PEM; when no key
//! is configured the endpoint returns an empty `keys` array so the URL is
//! always reachable.

use std::path::PathBuf;

use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use systemprompt_models::api::ApiError;
use systemprompt_models::modules::ApiPaths;
use systemprompt_runtime::AppContext;
use systemprompt_security::keys::{Jwks, RsaSigningKey};

use crate::services::middleware::RouterExt;

const DEFAULT_SIGNING_KEY_FILENAME: &str = "signing_key.pem";
const JWKS_RATE_LIMIT_PER_SECOND: u64 = 2;

pub fn jwks_router(ctx: &AppContext) -> Router {
    Router::new()
        .route(ApiPaths::WELLKNOWN_JWKS, get(handle_jwks))
        .with_state(ctx.clone())
        .with_rate_limit(&ctx.config().rate_limits, JWKS_RATE_LIMIT_PER_SECOND)
}

async fn handle_jwks(State(ctx): State<AppContext>) -> Result<impl IntoResponse, ApiError> {
    let jwks = load_jwks(&ctx).unwrap_or(Jwks { keys: vec![] });
    Ok(Json(jwks))
}

fn load_jwks(ctx: &AppContext) -> Option<Jwks> {
    let path = resolve_signing_key_path(ctx)?;
    if !path.exists() {
        return None;
    }
    match RsaSigningKey::load_from_pem_file(&path) {
        Ok(key) => Some(key.jwks()),
        Err(err) => {
            tracing::warn!(
                error = %err,
                path = %path.display(),
                "failed to load signing key for JWKS endpoint"
            );
            None
        },
    }
}

fn resolve_signing_key_path(ctx: &AppContext) -> Option<PathBuf> {
    let system_path = &ctx.config().system_path;
    if system_path.is_empty() {
        return None;
    }
    Some(PathBuf::from(system_path).join(DEFAULT_SIGNING_KEY_FILENAME))
}
