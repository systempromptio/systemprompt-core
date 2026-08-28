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

use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use systemprompt_events::AGUI_BROADCASTER;
use systemprompt_identifiers::{Actor, AgentName, ContextId, JwtToken, SessionId, TraceId};
use systemprompt_models::RequestContext;

use super::messages::extract_credential;
use crate::routes::stream::create_sse_stream;
use crate::services::middleware::JwtContextExtractor;

pub async fn handle(
    jwt_extractor: Arc<JwtContextExtractor>,
    headers: HeaderMap,
) -> axum::response::Response {
    let Some(credential) = extract_credential(&headers) else {
        return (
            StatusCode::UNAUTHORIZED,
            "Missing Authorization or x-api-key credential",
        )
            .into_response();
    };

    let (_claims, user) = match jwt_extractor
        .decode_for_gateway(&JwtToken::new(credential))
        .await
    {
        Ok(pair) => pair,
        Err(e) => return (StatusCode::UNAUTHORIZED, e.to_string()).into_response(),
    };

    let request_context = RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::new("bridge".to_owned()),
    )
    .with_actor(Actor::user(user.id));

    create_sse_stream(request_context, &AGUI_BROADCASTER, "bridge")
        .await
        .into_response()
}
