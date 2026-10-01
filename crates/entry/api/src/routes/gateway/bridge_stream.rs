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

use std::sync::Arc;

use axum::http::HeaderMap;
use axum::response::IntoResponse;
use systemprompt_events::AGUI_BROADCASTER;
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId};
use systemprompt_models::RequestContext;

use super::bridge_error::authenticate_bridge;
use crate::error::ApiHttpError;
use crate::routes::stream::create_sse_stream;
use crate::services::middleware::JwtContextExtractor;

pub async fn handle(
    jwt_extractor: Arc<JwtContextExtractor>,
    headers: HeaderMap,
) -> axum::response::Response {
    let (_claims, user) = match authenticate_bridge(&jwt_extractor, &headers).await {
        Ok(pair) => pair,
        Err(e) => return ApiHttpError::from(e).into_response(),
    };

    let request_context = RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::bridge(),
    )
    .with_actor(Actor::user(user.id));

    create_sse_stream(request_context, &AGUI_BROADCASTER, "bridge")
        .await
        .into_response()
}
