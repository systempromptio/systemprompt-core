//! Session minting for API-key callers of the gateway.
//!
//! A JWT caller gets its session row minted alongside its token, so the
//! `session_id` claim is attested by construction. An API-key caller has no
//! such step, and the gateway now refuses a session id it did not issue — so
//! this endpoint is that step: present the PAT, receive a `user_sessions` row
//! to send as `x-session-id` on `/v1/messages`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::Request;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Extension, Json, Router};
use serde::Serialize;
use std::sync::Arc;
use systemprompt_identifiers::{SessionId, SessionSource};
use systemprompt_oauth::services::SessionCreationService;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{AppContext as _, ExtractSignals};
use systemprompt_users::{API_KEY_PREFIX, ApiKeyService};

use crate::error::ApiHttpError;
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

    let record = ApiKeyService::new(ctx.db_pool())?
        .verify(&presented)
        .await?
        .ok_or_else(|| ApiHttpError::unauthorized("Invalid or revoked API key"))?;

    let analytics = ctx
        .analytics_provider()
        .ok_or_else(|| ApiHttpError::internal_error("Analytics provider unavailable"))?;
    let user_provider = ctx
        .user_provider()
        .ok_or_else(|| ApiHttpError::internal_error("User provider unavailable"))?;

    let caller_ip = client_ip_from_request(&request);
    let session_analytics = analytics.extract_analytics(
        request.headers(),
        ExtractSignals {
            caller_ip,
            ..Default::default()
        },
    );

    let session_id = SessionCreationService::new(Arc::clone(&analytics), user_provider)
        .create_authenticated_session(&record.user_id, &session_analytics, SessionSource::Api)
        .await?;

    Ok((StatusCode::CREATED, Json(MintedSession { session_id })))
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
