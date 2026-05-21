//! JTI revocation tower layer.
//!
//! Runs after [`crate::services::middleware::context::ContextMiddleware`] has
//! built the [`RequestContext`] and attached it to request extensions. The
//! JWT itself was already validated upstream (signature, audience, expiry);
//! this layer adds the one stateful check JWT validation cannot do — has the
//! token been explicitly revoked?
//!
//! - Anonymous / system contexts (empty `jti`) → no-op.
//! - Cache hit (revoked) → 401 immediately.
//! - Cache hit (fresh negative) → next.
//! - Cache miss → DB lookup, cache the result, then 401 or next.

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::sync::Arc;
use systemprompt_models::RequestContext;
use systemprompt_models::api::{ApiError, ErrorCode};
use systemprompt_oauth::repository::{JtiRevocationCache, OAuthRepository};

#[derive(Clone)]
pub struct JtiRevocationState {
    pub repo: Arc<OAuthRepository>,
    pub cache: Arc<JtiRevocationCache>,
}

impl std::fmt::Debug for JtiRevocationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JtiRevocationState").finish_non_exhaustive()
    }
}

pub async fn jti_revocation_middleware(
    State(state): State<JtiRevocationState>,
    request: Request,
    next: Next,
) -> Response {
    let jti = request
        .extensions()
        .get::<RequestContext>()
        .map(|ctx| ctx.jti().to_string())
        .unwrap_or_default();

    if jti.is_empty() {
        return next.run(request).await;
    }

    if let Some(true) = state.cache.peek(&jti) {
        return token_revoked_response();
    }
    if let Some(false) = state.cache.peek(&jti) {
        return next.run(request).await;
    }

    match state.repo.is_jti_revoked(&jti).await {
        Ok(revoked) => {
            state.cache.record(&jti, revoked);
            if revoked {
                token_revoked_response()
            } else {
                next.run(request).await
            }
        },
        Err(e) => {
            tracing::error!(error = %e, "JTI revocation lookup failed; failing closed");
            ApiError::new(ErrorCode::InternalError, "auth state lookup failed").into_response()
        },
    }
}

fn token_revoked_response() -> Response {
    let mut resp = ApiError::new(ErrorCode::Unauthorized, "Token revoked").into_response();
    *resp.status_mut() = StatusCode::UNAUTHORIZED;
    resp
}
