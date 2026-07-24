//! Minting of long-lived plugin-scoped JWTs (`aud=hook`).
//!
//! [`PluginTokenService`] shapes the claims for hook/governance plugin
//! credentials: `HookGovern` + `HookTrack` scope, a `hook` audience plus a
//! `plugin` resource audience, and the plugin id embedded as the `plugin_id`
//! claim. The caller resolves and authorizes the subject identity (admin
//! check, user lookup) before invoking [`PluginTokenService::issue`]; this
//! module never touches user storage.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use uuid::Uuid;

use systemprompt_identifiers::SessionId;
use systemprompt_models::auth::{AuthenticatedUser, JwtAudience, Permission};

use super::generation::{JwtConfig, JwtSigningParams, generate_access_token_jti, generate_jwt};
use crate::error::OauthResult;

#[derive(Debug, Clone)]
pub struct PluginTokenSubject {
    pub id: Uuid,
    pub username: String,
    pub email: String,
}

#[derive(Debug, Clone)]
pub struct IssuedPluginToken {
    pub token: String,
    pub jti: String,
}

#[derive(Debug, Clone, Copy)]
pub struct PluginTokenService;

impl PluginTokenService {
    /// `session_id` must name a persisted `user_sessions` row for `subject`
    /// that outlives `duration_days`: the governance webhook attests the claim
    /// on every hook call, so an unpersisted id fails on all of them.
    pub fn issue(
        subject: PluginTokenSubject,
        issuer: &str,
        plugin_id: String,
        duration_days: u32,
        session_id: &SessionId,
    ) -> OauthResult<IssuedPluginToken> {
        let permissions = vec![Permission::HookGovern, Permission::HookTrack];
        let authenticated = AuthenticatedUser::new_with_roles(
            subject.id,
            subject.username,
            subject.email,
            permissions.clone(),
            // Why: a hook-scoped (aud=hook) credential authorizes on scope + plugin_id,
            // never roles; carrying the minting admin's roles would be inert privilege.
            Vec::new(),
        );

        let signing = JwtSigningParams { issuer };
        let jti = generate_access_token_jti();

        let config = JwtConfig {
            permissions,
            audience: vec![JwtAudience::Hook],
            expires_in_hours: Some(i64::from(duration_days) * 24),
            resource: Some("plugin".to_owned()),
            plugin_id: Some(plugin_id),
        };

        let token = generate_jwt(&authenticated, config, jti.clone(), session_id, &signing)?;

        Ok(IssuedPluginToken { token, jti })
    }
}
