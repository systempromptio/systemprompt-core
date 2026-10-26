//! Validator for plugin-scoped hook JWTs.
//!
//! Hook tokens are minted by the OAuth `client_credentials` grant with
//! `audience=hook`, `scope=hook:govern hook:track`, and a custom `plugin_id`
//! claim. Cowork hook subprocesses present them on `Authorization: Bearer …`
//! when `POSTing` to the gateway's `/api/public/hooks/{govern,track}`
//! endpoints.
//!
//! [`HookTokenValidator`] enforces, in this order:
//!
//! 1. JWT signature, issuer, and `aud` contains `hook`, through the
//!    deployment's [`TokenAuthority`].
//! 2. `scope` contains the required permission for the endpoint
//!    ([`Permission::HookGovern`] or [`Permission::HookTrack`]).
//! 3. `plugin_id` claim is present.
//! 4. (Optional) `plugin_id` claim equals the `plugin_id` query parameter on
//!    the request, so a token issued for plugin A can't drive an event into
//!    plugin B.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::{PluginId, UserId};
use systemprompt_models::auth::{Permission, TokenAuthority};

use crate::error::{AuthError, AuthResult};

#[derive(Debug, Clone)]
pub struct ValidatedHookClaims {
    pub plugin_id: PluginId,
    pub subject: UserId,
    pub scopes: Vec<Permission>,
}

#[derive(Debug)]
pub struct HookTokenValidator {
    authority: Arc<dyn TokenAuthority>,
}

impl HookTokenValidator {
    #[must_use]
    pub fn new(authority: Arc<dyn TokenAuthority>) -> Self {
        Self { authority }
    }

    pub fn validate_govern(
        &self,
        token: &str,
        request_plugin_id: Option<&PluginId>,
    ) -> AuthResult<ValidatedHookClaims> {
        self.validate(
            token,
            Permission::HookGovern,
            "hook:govern",
            request_plugin_id,
        )
    }

    pub fn validate_track(
        &self,
        token: &str,
        request_plugin_id: Option<&PluginId>,
    ) -> AuthResult<ValidatedHookClaims> {
        self.validate(
            token,
            Permission::HookTrack,
            "hook:track",
            request_plugin_id,
        )
    }

    fn validate(
        &self,
        token: &str,
        required_scope: Permission,
        required_scope_name: &'static str,
        request_plugin_id: Option<&PluginId>,
    ) -> AuthResult<ValidatedHookClaims> {
        let claims = self
            .authority
            .verify_hook_token(token)
            .map_err(AuthError::Verification)?;

        if !claims.custom.scope.contains(&required_scope) {
            return Err(AuthError::HookScopeMissing(required_scope_name));
        }
        let plugin_id = claims
            .custom
            .plugin_id
            .clone()
            .ok_or(AuthError::HookPluginIdMissing)?;
        if let Some(expected) = request_plugin_id
            && *expected != plugin_id
        {
            return Err(AuthError::HookPluginIdMismatch {
                expected: expected.clone(),
                actual: plugin_id,
            });
        }

        Ok(ValidatedHookClaims {
            plugin_id,
            subject: UserId::try_new(claims.registered.sub).map_err(AuthError::InvalidSubject)?,
            scopes: claims.custom.scope,
        })
    }
}
