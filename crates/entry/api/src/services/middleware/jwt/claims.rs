//! [`SessionClaims`]: a token's signature and issuer, checked without the
//! session, user or revocation reads, for the two API paths that never
//! authorize on the result — the session middleware's attribution, which
//! attests the session itself, and the rate limiter's bucket choice.
//! Every authorizing path goes through the request authenticator.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::LoaderError;
use systemprompt_identifiers::ExtensionId;
use systemprompt_manifest::Config;
use systemprompt_models::auth::JwtUserContext;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_security::jwt::extract_user_context;
use systemprompt_security::keys::VerificationKeyring;

#[derive(Clone, Debug)]
pub(crate) struct SessionClaims {
    keyring: VerificationKeyring,
    issuer: Arc<str>,
}

impl SessionClaims {
    pub(crate) fn from_kernel(state: &Dependencies) -> Result<Self, LoaderError> {
        let keyring = state
            .get::<VerificationKeyring>()
            .cloned()
            .map_err(|_missing| {
                LoaderError::missing_provider::<VerificationKeyring>(ExtensionId::new("kernel"))
            })?;
        let config = state.get::<Arc<Config>>().map_err(|_missing| {
            LoaderError::missing_provider::<Arc<Config>>(ExtensionId::new("kernel"))
        })?;
        Ok(Self {
            keyring,
            issuer: Arc::from(config.jwt_issuer.as_str()),
        })
    }

    pub(crate) fn verified_claims(&self, token: &str) -> Option<JwtUserContext> {
        extract_user_context(&self.keyring, token, &self.issuer).ok()
    }
}
