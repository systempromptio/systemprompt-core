//! Resolves or provisions a browser `SessionInfo` from JWT cookies.
//!
//! Static-content requests go through the same shared-trait providers as the
//! session middleware; an absent provider refuses rather than minting a
//! session without request analytics.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::net::IpAddr;

use anyhow::Result;
use axum::http::HeaderMap;
use systemprompt_identifiers::{ClientId, JwtToken, SessionId, SessionSource, UserId};
use systemprompt_oauth::{CreateAnonymousSessionInput, validate_jwt_token};
use systemprompt_runtime::AppContext;
use systemprompt_security::TokenExtractor;
use systemprompt_traits::ExtractSignals;

use crate::services::middleware::session::session_providers;

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub session_id: SessionId,
    pub user_id: UserId,
    pub is_new: bool,
    pub jwt_token: Option<JwtToken>,
}

pub async fn ensure_session(
    headers: &HeaderMap,
    uri: Option<&http::Uri>,
    caller_ip: Option<IpAddr>,
    ctx: &AppContext,
) -> Result<SessionInfo> {
    let config = ctx.config();

    if let Ok(token) = TokenExtractor::browser_only().extract(headers)
        && let Ok(claims) = validate_jwt_token(
            ctx.token_authority().as_ref(),
            &token,
            &config.jwt_audiences,
        )
        && let Some(session_id) = claims.custom.session_id
        && let Ok(user_id) = UserId::try_new(claims.registered.sub)
    {
        return Ok(SessionInfo {
            session_id: SessionId::new(session_id),
            user_id,
            is_new: false,
            jwt_token: Some(JwtToken::new(token)),
        });
    }

    let (analytics, _, session_service) = session_providers(ctx)?;

    let client_id = ClientId::new("sp_web");
    let request_analytics = analytics.extract_analytics(headers, ExtractSignals { uri, caller_ip });
    let session_info = session_service
        .create_anonymous_session(CreateAnonymousSessionInput {
            analytics: &request_analytics,
            client_id: &client_id,
            session_source: SessionSource::Web,
        })
        .await?;

    Ok(SessionInfo {
        session_id: session_info.session_id,
        user_id: session_info.user_id,
        is_new: session_info.is_new,
        jwt_token: Some(session_info.jwt_token),
    })
}
