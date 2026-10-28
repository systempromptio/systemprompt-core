//! Bridge session exchange code generation and consumption.
//!
//! A bridge exchange's `purpose` is the caller purpose bound to the credential
//! that authenticated it (an API key's `purpose`), stamped into the bridge JWT.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod exchange;
mod provisioning;

pub use exchange::{
    BridgeExchangeCode, BridgeExchangeRequest, exchange_bridge_session_code, hash_exchange_code,
    issue_bridge_exchange_code,
};
pub use provisioning::{BridgeOAuthClient, provision_bridge_oauth_client};

use crate::error::OauthResult as Result;
use chrono::{Duration as ChronoDuration, Utc};
use http::HeaderMap;
use serde::Serialize;
use std::collections::HashMap;
use std::net::IpAddr;
use systemprompt_identifiers::{
    AccessTokenId, ClientId, PolicyVersion, SessionId, SessionSource, TraceId, UserId, headers,
};
use systemprompt_manifest::Config;
use systemprompt_models::auth::{AuthenticatedUser, JwtAudience, TokenInteraction};
use systemprompt_security::keys::SigningKeys;
use systemprompt_traits::{
    AnalyticsProvider, CreateSessionInput, ExtractSignals, SessionProvider, UserProvider,
};


use crate::services::authenticated_user::load_authenticated_user;
use crate::services::generation::{JwtConfig, JwtSigningParams, generate_jwt};

const DEFAULT_ACCESS_TTL_SECONDS: u64 = 3600;

#[derive(Debug, Clone, Serialize)]
pub struct BridgeAuthResult {
    pub token: String,
    pub ttl: u64,
    pub headers: HashMap<String, String>,
}

#[derive(Debug)]
pub struct BridgeAccessRequest<'a> {
    pub config: &'a Config,
    pub request_headers: &'a HeaderMap,
    pub caller_ip: Option<IpAddr>,
    pub user_id: &'a UserId,
    pub client_id: ClientId,
    pub session_source: SessionSource,
    pub ttl_seconds: u64,
    pub purpose: Option<String>,
    /// Minted for a live host session a person started (the exchange-code
    /// flow); stamps `interaction: interactive` into the bridge JWT.
    pub interactive: bool,
}

impl<'a> BridgeAccessRequest<'a> {
    pub fn bridge(
        config: &'a Config,
        request_headers: &'a HeaderMap,
        caller_ip: Option<IpAddr>,
        user_id: &'a UserId,
    ) -> Self {
        Self {
            config,
            request_headers,
            caller_ip,
            user_id,
            client_id: ClientId::bridge(),
            session_source: SessionSource::Bridge,
            ttl_seconds: DEFAULT_ACCESS_TTL_SECONDS,
            purpose: None,
            interactive: false,
        }
    }

    /// Marks the access as a person's live host session.
    #[must_use]
    pub const fn interactive(mut self) -> Self {
        self.interactive = true;
        self
    }

    pub fn with_purpose(mut self, purpose: Option<String>) -> Self {
        self.purpose = purpose;
        self
    }
}

async fn adopt_or_mint_session(
    sessions: &dyn SessionProvider,
    request_headers: &HeaderMap,
    user_id: &UserId,
) -> SessionId {
    let requested_session = request_headers
        .get(headers::SESSION_ID)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(SessionId::new);
    match requested_session {
        Some(requested) => match sessions.find_session_by_id(&requested).await {
            Ok(Some(existing)) if existing.user_id.as_ref() == Some(user_id) => requested,
            Ok(None) => requested,
            Ok(Some(_)) => {
                tracing::warn!(
                    session_id = %requested,
                    user_id = %user_id,
                    "requested session belongs to a different user; minting a fresh session",
                );
                SessionId::generate()
            },
            Err(e) => {
                tracing::warn!(error = %e, "session ownership lookup failed; minting a fresh session");
                SessionId::generate()
            },
        },
        None => SessionId::generate(),
    }
}

pub async fn issue_bridge_access(
    keys: &SigningKeys,
    analytics: &dyn AnalyticsProvider,
    sessions: &dyn SessionProvider,
    users: &dyn UserProvider,
    request: BridgeAccessRequest<'_>,
) -> Result<BridgeAuthResult> {
    let BridgeAccessRequest {
        config: global_config,
        request_headers,
        caller_ip,
        user_id,
        client_id,
        session_source,
        ttl_seconds,
        purpose,
        interactive,
    } = request;

    let auth_user = load_authenticated_user(users, user_id).await?;

    let session_id = adopt_or_mint_session(sessions, request_headers, user_id).await;
    let trace_id = TraceId::generate();
    let policy_version = PolicyVersion::unversioned();

    let ttl = ChronoDuration::seconds(i64::try_from(ttl_seconds).unwrap_or(i64::MAX));
    let interaction = (interactive && purpose.is_none()).then_some(TokenInteraction::Interactive);
    let config = build_bridge_jwt_config(&auth_user, ttl, purpose, interaction);
    let signing = JwtSigningParams {
        issuer: &global_config.jwt_issuer,
        keys,
    };
    let token = generate_jwt(
        &auth_user,
        config,
        AccessTokenId::generate(),
        &session_id,
        &signing,
    )?;

    let session_analytics = analytics.extract_analytics(
        request_headers,
        ExtractSignals {
            caller_ip,
            ..Default::default()
        },
    );
    let expires_at = Utc::now() + ChronoDuration::seconds(i64::try_from(ttl_seconds).unwrap_or(0));
    sessions
        .create_session(CreateSessionInput {
            session_id: &session_id,
            user_id: Some(user_id),
            analytics: &session_analytics,
            session_source,
            is_bot: false,
            is_ai_crawler: false,
            expires_at,
        })
        .await?;

    let hdrs = build_bridge_headers(&BridgeHeaderParams {
        user_id,
        session_id: &session_id,
        trace_id: &trace_id,
        client_id: &client_id,
        policy_version: &policy_version,
        session_source,
    });

    Ok(BridgeAuthResult {
        token,
        ttl: ttl_seconds,
        headers: hdrs,
    })
}

fn build_bridge_jwt_config(
    auth_user: &AuthenticatedUser,
    ttl: ChronoDuration,
    purpose: Option<String>,
    interaction: Option<TokenInteraction>,
) -> JwtConfig {
    JwtConfig {
        permissions: auth_user.permissions().to_vec(),
        audience: vec![JwtAudience::Bridge, JwtAudience::Mcp],
        expires_in: ttl,
        resource: None,
        plugin_id: None,
        client_id: Some(ClientId::bridge()),
        purpose,
        interaction,
    }
}

struct BridgeHeaderParams<'a> {
    user_id: &'a UserId,
    session_id: &'a SessionId,
    trace_id: &'a TraceId,
    client_id: &'a ClientId,
    policy_version: &'a PolicyVersion,
    session_source: SessionSource,
}

fn build_bridge_headers(params: &BridgeHeaderParams<'_>) -> HashMap<String, String> {
    let mut hdrs = HashMap::new();
    hdrs.insert(headers::USER_ID.to_owned(), params.user_id.to_string());
    hdrs.insert(
        headers::SESSION_ID.to_owned(),
        params.session_id.to_string(),
    );
    hdrs.insert(headers::TRACE_ID.to_owned(), params.trace_id.to_string());
    hdrs.insert(headers::CLIENT_ID.to_owned(), params.client_id.to_string());
    hdrs.insert(
        headers::POLICY_VERSION.to_owned(),
        params.policy_version.to_string(),
    );
    hdrs.insert(
        headers::CALL_SOURCE.to_owned(),
        params.session_source.as_str().to_owned(),
    );
    hdrs
}
