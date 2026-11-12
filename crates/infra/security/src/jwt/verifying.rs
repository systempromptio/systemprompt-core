//! [`VerifyingTokenAuthority`]: a [`TokenAuthority`] that verifies and never
//! signs.
//!
//! A least-privilege child holds only the deployment's public
//! [`VerificationKeyring`]. Its verify operations run the same primitives as
//! [`DeploymentTokenAuthority`](super::DeploymentTokenAuthority) against that
//! keyring; every signing operation is refused with
//! [`TokenError::Signing`] carrying [`SigningUnavailable`], so code built on
//! it fails loudly instead of producing a token.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::McpServerId;
use systemprompt_models::auth::{
    JwtAudience, JwtClaims, McpHopClaims, McpHopIdentity, TokenAuthority, TokenError, TokenResult,
    UserJwtRequest,
};

use super::hop::decode_mcp_hop;
use super::validate::{ValidationPolicy, decode_rs256_claims, decode_session_claims};
use crate::keys::VerificationKeyring;

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("this process holds no signing key; tokens are signed only by the API")]
pub struct SigningUnavailable;

#[derive(Debug, Clone)]
pub struct VerifyingTokenAuthority {
    keyring: VerificationKeyring,
    issuer: String,
}

impl VerifyingTokenAuthority {
    #[must_use]
    pub fn new(keyring: VerificationKeyring, issuer: impl Into<String>) -> Self {
        Self {
            keyring,
            issuer: issuer.into(),
        }
    }
}

fn refused<T>() -> TokenResult<T> {
    Err(TokenError::Signing(Box::new(SigningUnavailable)))
}

impl TokenAuthority for VerifyingTokenAuthority {
    fn verify_session_cookie(&self, token: &str) -> TokenResult<JwtClaims> {
        let policy = ValidationPolicy::session_context(&self.issuer);
        Ok(decode_session_claims(&self.keyring, token, &policy)?)
    }

    fn verify_bearer(&self, token: &str, audiences: &[JwtAudience]) -> TokenResult<JwtClaims> {
        let policy = ValidationPolicy::issuer_scoped(&self.issuer, audiences);
        Ok(decode_rs256_claims(&self.keyring, token, &policy)?)
    }

    fn verify_hook_token(&self, token: &str) -> TokenResult<JwtClaims> {
        let policy = ValidationPolicy::issuer_scoped(&self.issuer, &[JwtAudience::Hook]);
        Ok(decode_rs256_claims(&self.keyring, token, &policy)?)
    }

    // JSON: the trait signs extension-defined claim sets of no fixed schema
    fn sign_claims(&self, _claims: &serde_json::Value) -> TokenResult<String> {
        refused()
    }

    fn mint_user_jwt(&self, _req: &UserJwtRequest) -> TokenResult<String> {
        refused()
    }

    fn mint_mcp_hop(
        &self,
        _server: &McpServerId,
        _identity: &McpHopIdentity,
    ) -> TokenResult<String> {
        refused()
    }

    fn verify_mcp_hop(&self, token: &str, server: &McpServerId) -> TokenResult<McpHopClaims> {
        Ok(decode_mcp_hop(&self.keyring, &self.issuer, server, token)?)
    }
}
