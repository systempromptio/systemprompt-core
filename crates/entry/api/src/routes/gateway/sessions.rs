//! Session minting for API-key callers of the gateway.
//!
//! A JWT caller gets its session row minted alongside its token, so the
//! `session_id` claim is attested by construction. An API-key caller that
//! sends no gateway-issued `x-session-id` lands on the session bound to its
//! key (see `messages::session_binding`); this endpoint is for a caller that
//! wants a distinct session of its own: present the PAT, receive a
//! `user_sessions` row to send as `x-session-id` on `/v1/messages`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::Request;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Extension, Json, Router};
use serde::Serialize;
use systemprompt_identifiers::SessionId;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{AppContext as _, DynGatewaySessionMint, ExtractSignals};
use systemprompt_users::API_KEY_PREFIX;

use crate::kernel::ApiHttpError;
use crate::routes::gateway::messages::extract_credential;
use crate::services::middleware::client_addr::client_ip_from_request;

#[derive(Debug, Serialize)]
pub struct MintedSession {
    pub session_id: SessionId,
}

pub async fn create_session(
    ctx: AppContext,
    request: Request,
) -> Result<(StatusCode, Json<MintedSession>), ApiHttpError> {
    let presented = extract_credential(request.headers())
        .ok_or_else(|| ApiHttpError::unauthorized("Missing x-api-key or Authorization: Bearer"))?;
    if !presented.starts_with(API_KEY_PREFIX) {
        return Err(ApiHttpError::unauthorized(
            "This endpoint authenticates API keys only; JWT callers already carry a session",
        ));
    }

    let mint = ctx
        .kernel()
        .state()
        .get::<DynGatewaySessionMint>()
        .cloned()
        .map_err(|e| ApiHttpError::internal("Gateway session mint unavailable", e))?;
    let analytics = ctx.analytics_provider();

    let caller_ip = client_ip_from_request(&request, ctx.config());
    let session_analytics = analytics.extract_analytics(
        request.headers(),
        ExtractSignals {
            caller_ip,
            ..Default::default()
        },
    );

    let minted = mint
        .mint_for_api_key(&presented, &session_analytics)
        .await
        .map_err(|e| ApiHttpError::internal("Gateway session mint failed", e))?
        .ok_or_else(|| ApiHttpError::unauthorized("Invalid or revoked API key"))?;

    Ok((
        StatusCode::CREATED,
        Json(MintedSession {
            session_id: minted.session_id,
        }),
    ))
}

pub fn public_router(ctx: &AppContext) -> Router {
    let mint_ctx = ctx.clone();
    Router::new()
        .route(
            "/sessions",
            post(move |request| {
                let context = mint_ctx.clone();
                async move { create_session(context, request).await }
            }),
        )
        .layer(Extension(ctx.clone()))
}
