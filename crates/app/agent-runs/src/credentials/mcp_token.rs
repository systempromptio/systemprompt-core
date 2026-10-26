//! The run's MCP token.
//!
//! An instance MCP server authenticates a JWT, not a PAT, so a run whose
//! credentials carry a purpose also gets an `aud=mcp` JWT for the run's user
//! with that purpose as its signed `purpose` claim. The MCP server reads the
//! claim as the caller purpose, which is how a service account's credential
//! broker sees `farm:<run_id>`. Its `jti` is revoked when the run ends.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{AccessTokenId, SessionId, UserId};
use systemprompt_models::auth::{JwtAudience, TokenAuthority, UserJwtRequest};
use systemprompt_oauth::services::load_authenticated_user;
use systemprompt_traits::UserProvider;

use crate::error::{AgentRunsError, AgentRunsResult};

pub(super) struct MintedMcpToken {
    pub token: String,
    pub jti: AccessTokenId,
    pub expires_at: DateTime<Utc>,
}

pub(super) struct McpTokenRequest<'a> {
    pub authority: &'a dyn TokenAuthority,
    pub user_id: &'a UserId,
    pub session_id: &'a SessionId,
    pub ttl: chrono::Duration,
    pub purpose: &'a str,
}

pub(super) async fn mint_mcp_token(
    users: &dyn UserProvider,
    req: McpTokenRequest<'_>,
) -> AgentRunsResult<MintedMcpToken> {
    let user = load_authenticated_user(users, req.user_id)
        .await
        .map_err(|source| AgentRunsError::PluginToken { source })?;
    let jti = AccessTokenId::generate();
    let request = UserJwtRequest {
        permissions: user.permissions().to_vec(),
        user,
        session_id: req.session_id.clone(),
        jti: jti.clone(),
        audience: vec![JwtAudience::Mcp],
        ttl: req.ttl,
        client_id: None,
        purpose: Some(req.purpose.to_owned()),
        plugin_id: None,
    };
    let token =
        req.authority
            .mint_user_jwt(&request)
            .map_err(|source| AgentRunsError::PluginToken {
                source: source.into(),
            })?;
    Ok(MintedMcpToken {
        token,
        jti,
        expires_at: Utc::now() + req.ttl,
    })
}
