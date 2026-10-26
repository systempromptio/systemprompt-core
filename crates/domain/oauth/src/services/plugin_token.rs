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


use systemprompt_identifiers::{AccessTokenId, PluginId, SessionId, UserId};
use systemprompt_models::auth::{
    AuthenticatedUser, JwtAudience, Permission, TokenAuthority, UserJwtRequest,
};

use crate::error::OauthResult;

#[derive(Debug, Clone)]
pub struct PluginTokenSubject {
    pub id: UserId,
    pub username: String,
    pub email: String,
}

#[derive(Debug, Clone)]
pub struct IssuedPluginToken {
    pub token: String,
    pub jti: AccessTokenId,
}

#[derive(Debug, Clone, Copy)]
pub struct PluginTokenService;

impl PluginTokenService {
    pub fn issue(
        subject: PluginTokenSubject,
        authority: &dyn TokenAuthority,
        plugin_id: PluginId,
        duration_days: u32,
        session_id: &SessionId,
    ) -> OauthResult<IssuedPluginToken> {
        let permissions = vec![Permission::HookGovern, Permission::HookTrack];
        let authenticated = AuthenticatedUser::new_with_roles(
            subject.id,
            subject.username,
            subject.email,
            permissions.clone(),
            Vec::new(),
        );

        let jti = AccessTokenId::generate();

        let request = UserJwtRequest {
            user: authenticated,
            session_id: session_id.clone(),
            jti: jti.clone(),
            permissions,
            audience: vec![
                JwtAudience::Hook,
                JwtAudience::Resource("plugin".to_owned()),
            ],
            ttl: chrono::Duration::days(i64::from(duration_days)),
            client_id: None,
            purpose: None,
            plugin_id: Some(plugin_id),
        };
        let token = authority.mint_user_jwt(&request)?;

        Ok(IssuedPluginToken { token, jti })
    }
}
