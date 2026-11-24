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
use axum::{Json, Router};
use serde::Serialize;
use systemprompt_extension::router::ApiHttpError;
use systemprompt_gateway::surface::credential::extract_credential;
use systemprompt_identifiers::SessionId;
use systemprompt_traits::{DynGatewaySessionMint, ExtractSignals};
use systemprompt_users::API_KEY_PREFIX;

use std::sync::Arc;

use systemprompt_gateway::surface::views::GatewaySettingsView;
use systemprompt_traits::AnalyticsProvider;

use crate::http::bridge::client_ip;

/// What the public session route reads.
///
/// The session mint conferred on `gateway-http`, the analytics extractor, and
/// the settings view whose trusted proxies resolve the caller's address; on a
/// host that seeds no bridge views it is absent and no address is resolved.
#[derive(Clone)]
pub struct SessionsState {
    pub mint: DynGatewaySessionMint,
    pub analytics: Arc<dyn AnalyticsProvider>,
    pub settings: Option<Arc<GatewaySettingsView>>,
}

impl std::fmt::Debug for SessionsState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionsState").finish_non_exhaustive()
    }
}

#[derive(Debug, Serialize)]
pub struct MintedSession {
    pub session_id: SessionId,
}

pub async fn create_session(
    state: SessionsState,
    request: Request,
) -> Result<(StatusCode, Json<MintedSession>), ApiHttpError> {
    let presented = extract_credential(request.headers())
        .ok_or_else(|| ApiHttpError::unauthorized("Missing x-api-key or Authorization: Bearer"))?;
    if !presented.starts_with(API_KEY_PREFIX) {
        return Err(ApiHttpError::unauthorized(
            "This endpoint authenticates API keys only; JWT callers already carry a session",
        ));
    }

    let mint = &state.mint;
    let analytics = &state.analytics;

    let caller_ip = state
        .settings
        .as_ref()
        .and_then(|settings| client_ip(&request, settings));
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

pub fn public_router(state: SessionsState) -> Router {
    Router::new().route(
        "/sessions",
        post(move |request| {
            let state = state.clone();
            async move { create_session(state, request).await }
        }),
    )
}
