//! The run's governance hook token.
//!
//! A plugin-scoped JWT (`aud=hook`, `hook:govern hook:track`, `plugin_id`)
//! minted by [`PluginTokenService`] for the run's user and bound to the run's
//! gateway session; its `jti` is revoked when the run ends.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{AccessTokenId, PluginId, SessionId, UserId};
use systemprompt_oauth::OAuthRepository;
use systemprompt_oauth::services::{
    IssuedPluginToken, JwtSigningParams, PluginTokenService, PluginTokenSubject,
};
use systemprompt_security::keys::SigningKeys;
use systemprompt_users::UserRepository;

use crate::error::{AgentRunsError, AgentRunsResult};

const SECONDS_PER_DAY: i64 = 86_400;

pub(super) struct MintedHookToken {
    pub issued: IssuedPluginToken,
    pub expires_at: DateTime<Utc>,
}

pub(super) struct HookTokenRequest<'a> {
    pub issuer: &'a str,
    pub keys: &'a SigningKeys,
    pub user_id: &'a UserId,
    pub plugin_id: &'a PluginId,
    pub session_id: &'a SessionId,
    pub ttl: chrono::Duration,
}

pub(super) async fn mint_hook_token(
    users: &UserRepository,
    req: HookTokenRequest<'_>,
) -> AgentRunsResult<MintedHookToken> {
    let HookTokenRequest {
        issuer,
        keys,
        user_id,
        plugin_id,
        session_id,
        ttl,
    } = req;
    let user = users
        .find_by_id(user_id)
        .await
        .map_err(|source| AgentRunsError::UserStore {
            step: "load run user",
            source,
        })?
        .ok_or_else(|| AgentRunsError::UserNotFound {
            user_id: user_id.clone(),
        })?;
    let subject = PluginTokenSubject {
        id: user.id,
        username: user.name,
        email: user.email,
    };
    let days = token_days(ttl);
    let expires_at = Utc::now() + chrono::Duration::days(i64::from(days));
    let minted = PluginTokenService::issue(
        subject,
        &JwtSigningParams { issuer, keys },
        plugin_id.clone(),
        days,
        session_id,
    )
    .map_err(|source| AgentRunsError::PluginToken { source })?;
    Ok(MintedHookToken {
        issued: minted,
        expires_at,
    })
}

pub(super) async fn revoke_hook_token(
    oauth: &OAuthRepository,
    jti: &AccessTokenId,
    user_id: &UserId,
    exp: DateTime<Utc>,
) -> AgentRunsResult<()> {
    oauth
        .revoke_jti(jti, user_id, exp)
        .await
        .map_err(|source| AgentRunsError::PluginToken { source })
}

fn token_days(ttl: chrono::Duration) -> u32 {
    let days = (ttl.num_seconds() + SECONDS_PER_DAY - 1) / SECONDS_PER_DAY;
    u32::try_from(days.max(1)).unwrap_or(u32::MAX)
}
