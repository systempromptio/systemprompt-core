//! The database-backed window for routers that authenticate inside their
//! handlers.
//!
//! Such a router carries no [`RequestContext`], so the identity window in
//! [`super::global`] sees every request as anonymous and only the per-replica
//! governor would limit it — a budget that multiplies with the replica count.
//! This layer verifies the bearer token's signature and issuer (the handler
//! still performs the full session, user and revocation checks) and charges
//! the same per-user window, so a caller's budget holds across replicas. A
//! request with no bearer JWT, an API key, or a token that does not verify is
//! left to the handler, which authenticates or refuses it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Request, State};
use axum::http::{HeaderMap, header};
use axum::middleware::Next;
use axum::response::Response;
use std::sync::Arc;
use systemprompt_models::RequestContext;
use systemprompt_models::auth::UserType;
use systemprompt_users::API_KEY_PREFIX;

use super::global::{GlobalUserLimit, charge};
use crate::services::middleware::jwt::JwtContextExtractor;

#[derive(Clone, Debug)]
pub(super) struct BearerWindow {
    pub(super) limit: GlobalUserLimit,
    pub(super) jwt: Arc<JwtContextExtractor>,
}

pub fn bearer_jwt(headers: &HeaderMap) -> Option<&str> {
    let raw = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())?;
    let token = raw.strip_prefix("Bearer ")?.trim();
    (!token.is_empty() && !token.starts_with(API_KEY_PREFIX)).then_some(token)
}

pub(super) async fn bearer_user_rate_limit(
    State(window): State<BearerWindow>,
    req: Request,
    next: Next,
) -> Response {
    if req.extensions().get::<RequestContext>().is_some() {
        return next.run(req).await;
    }
    let user_id = bearer_jwt(req.headers())
        .and_then(|token| window.jwt.verified_claims(token))
        .filter(|claims| claims.user_type != UserType::Anon)
        .map(|claims| claims.user_id);
    let Some(user_id) = user_id else {
        return next.run(req).await;
    };
    charge(&window.limit, &user_id, req, next).await
}
