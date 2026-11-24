//! `GET /v1/bridge/stream` — the bridge's live event feed.
//!
//! The `/api/v1/stream/*` routes already fan out the same broadcasters, but
//! they sit behind `UserOnlyContextMiddleware`, whose token extractor is
//! browser-shaped: a Bearer JWT or a session cookie, never `x-api-key`. A
//! bridge authenticates the gateway way, so it cannot use them without either
//! adopting the browser flavour or having the middleware widened for it —
//! both worse than one additive route that authenticates the way every other
//! `/v1/bridge/*` endpoint does.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::response::sse::Sse;
use systemprompt_events::standard_keep_alive;
use systemprompt_extension::router::ApiHttpError;
use systemprompt_models::api::ApiError;

use super::BridgeRouteState;
use super::error::authenticate_bridge;

pub async fn handle(state: BridgeRouteState, headers: HeaderMap) -> axum::response::Response {
    let (_claims, user) = match authenticate_bridge(&state.authenticator, &headers).await {
        Ok(pair) => pair,
        Err(e) => return ApiHttpError::from(e).into_response(),
    };
    let Some(subscription) = state.events.subscribe(&user.id).await else {
        tracing::warn!(user_id = %user.id, "bridge event stream refused: per-user connection cap reached");
        return ApiHttpError::from(ApiError::rate_limited(
            "Per-user stream connection limit reached",
        ))
        .into_response();
    };
    tracing::info!(user_id = %user.id, "bridge event stream opened");
    Sse::new(subscription)
        .keep_alive(standard_keep_alive())
        .into_response()
}
