//! The re-check every tool execution makes of the caller it is handed.
//!
//! A [`VerifiedCaller`] reaching the executor is evidence:
//! [`CallerVerification`] decodes its hop token again (RS256 under the
//! deployment's verification keys, the hop `typ`, this deployment's issuer, the
//! one server's audience and expiry) and requires the token's identity to vouch
//! for the caller's context. Anything else is a typed [`CallerRefused`] and the
//! execution is not recorded.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::mcp::VerifiedCaller;
use systemprompt_identifiers::McpServerId;
use systemprompt_security::AuthError;
use systemprompt_security::jwt::decode_mcp_hop;
use systemprompt_security::keys::VerificationKeyring;

#[derive(Debug, thiserror::Error)]
pub enum CallerRefused {
    #[error("the caller's hop token does not verify for this server")]
    Token(#[source] AuthError),
    #[error("the caller's hop token does not vouch for the context it was presented with")]
    NotVouched,
}

/// The server an executor records for, with what it re-verifies callers
/// against.
#[derive(Debug, Clone)]
pub struct ServerCallers {
    pub server: McpServerId,
    pub verification: CallerVerification,
}

/// What a tool execution re-verifies its caller against.
#[derive(Clone)]
pub struct CallerVerification {
    keyring: VerificationKeyring,
    issuer: String,
}

impl std::fmt::Debug for CallerVerification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CallerVerification")
            .field("issuer", &self.issuer)
            .finish_non_exhaustive()
    }
}

impl CallerVerification {
    #[must_use]
    pub fn new(keyring: VerificationKeyring, issuer: &str) -> Self {
        Self {
            keyring,
            issuer: issuer.to_owned(),
        }
    }

    pub fn verify(
        &self,
        server: &McpServerId,
        caller: &VerifiedCaller,
    ) -> Result<(), CallerRefused> {
        let claims = decode_mcp_hop(
            &self.keyring,
            &self.issuer,
            server,
            caller.hop_token().as_str(),
        )
        .map_err(CallerRefused::Token)?;
        if claims.identity.vouches_for(caller.context()) {
            Ok(())
        } else {
            Err(CallerRefused::NotVouched)
        }
    }
}
