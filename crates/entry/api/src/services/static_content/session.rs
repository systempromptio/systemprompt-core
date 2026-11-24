//! Resolves or provisions a browser `SessionInfo` from JWT cookies.
//!
//! Static-content requests read the same sealed session issuer as the session
//! middleware; with no issuer (the `oauth` member not admitted) no session is
//! minted and the request is refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::net::IpAddr;

use anyhow::{Result, anyhow};
use axum::http::HeaderMap;
use systemprompt_identifiers::{ClientId, JwtToken, SessionId, SessionSource, UserId};
use systemprompt_runtime::AppContext;
use systemprompt_security::TokenExtractor;
use systemprompt_traits::{AppContext as _, ExtractSignals};

use crate::services::middleware::session::session_issuer;

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
    let issuer = session_issuer(ctx).ok_or_else(|| anyhow!("no session issuer is provided"))?;

    if let Ok(token) = TokenExtractor::browser_only().extract(headers)
        && let Some(session) = issuer.verify_browser_token(&token)
    {
        return Ok(SessionInfo {
            session_id: session.session_id,
            user_id: session.user_id,
            is_new: false,
            jwt_token: Some(JwtToken::new(token)),
        });
    }

    let client_id = ClientId::new("sp_web");
    let request_analytics = ctx
        .analytics_provider()
        .extract_analytics(headers, ExtractSignals { uri, caller_ip });
    let session_info = issuer
        .issue_anonymous(&request_analytics, &client_id, SessionSource::Web)
        .await?;

    Ok(SessionInfo {
        session_id: session_info.session_id,
        user_id: session_info.user_id,
        is_new: session_info.is_new,
        jwt_token: Some(session_info.jwt_token),
    })
}
