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
//! 1. JWT signature, issuer, and `aud` contains `hook`.
//! 2. `scope` contains the required permission for the endpoint
//!    ([`Permission::HookGovern`] or [`Permission::HookTrack`]).
//! 3. `plugin_id` claim is present.
//! 4. (Optional) `plugin_id` claim equals the `plugin_id` query parameter on
//!    the request, so a token issued for plugin A can't drive an event into
//!    plugin B.

use jsonwebtoken::{Algorithm, Validation, decode, decode_header};
use systemprompt_models::auth::{JwtAudience, JwtClaims, Permission};

use crate::error::{AuthError, AuthResult};
use crate::keys::authority;

/// Successfully-validated hook token claims, projected to the bits the
/// caller needs to dispatch a govern/track decision.
#[derive(Debug, Clone)]
pub struct ValidatedHookClaims {
    pub plugin_id: String,
    pub subject: String,
    pub scopes: Vec<Permission>,
}

#[derive(Debug)]
pub struct HookTokenValidator {
    issuer: String,
}

impl HookTokenValidator {
    #[must_use]
    pub const fn new(issuer: String) -> Self {
        Self { issuer }
    }

    /// Validate a hook token for the `/api/public/hooks/govern` endpoint.
    pub fn validate_govern(
        &self,
        token: &str,
        request_plugin_id: Option<&str>,
    ) -> AuthResult<ValidatedHookClaims> {
        self.validate(
            token,
            Permission::HookGovern,
            "hook:govern",
            request_plugin_id,
        )
    }

    /// Validate a hook token for the `/api/public/hooks/track` endpoint.
    pub fn validate_track(
        &self,
        token: &str,
        request_plugin_id: Option<&str>,
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
        request_plugin_id: Option<&str>,
    ) -> AuthResult<ValidatedHookClaims> {
        let header = decode_header(token).map_err(AuthError::InvalidToken)?;
        if header.alg != Algorithm::RS256 {
            return Err(AuthError::UnsupportedAlgorithm);
        }
        let kid = header.kid.as_deref().ok_or(AuthError::MissingKid)?;
        let key = authority::decoding_key_for_kid(kid)
            .map_err(|e| AuthError::KeyLookup(e.to_string()))?
            .ok_or_else(|| AuthError::UnknownKid(kid.to_string()))?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[JwtAudience::Hook.as_str()]);

        let token_data =
            decode::<JwtClaims>(token, key, &validation).map_err(AuthError::InvalidToken)?;

        let claims = token_data.claims;

        if !claims.aud.iter().any(|a| matches!(a, JwtAudience::Hook)) {
            return Err(AuthError::HookAudienceMissing);
        }
        if !claims.scope.contains(&required_scope) {
            return Err(AuthError::HookScopeMissing(required_scope_name));
        }
        let plugin_id = claims
            .plugin_id
            .clone()
            .ok_or(AuthError::HookPluginIdMissing)?;
        if let Some(expected) = request_plugin_id
            && expected != plugin_id
        {
            return Err(AuthError::HookPluginIdMismatch {
                expected: expected.to_string(),
                actual: plugin_id,
            });
        }

        Ok(ValidatedHookClaims {
            plugin_id,
            subject: claims.sub,
            scopes: claims.scope,
        })
    }
}
