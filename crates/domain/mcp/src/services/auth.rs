//! MCP request authentication helpers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::{McpDomainError, McpDomainResult};
use systemprompt_models::auth::{JwtAudience, JwtClaims};
use systemprompt_security::AuthError;
use systemprompt_security::jwt::{ValidationPolicy, decode_rs256_claims};
use systemprompt_security::keys::SigningKeys;

pub fn validate_jwt_token(
    keys: &SigningKeys,
    token: &str,
    issuer: &str,
    audiences: &[JwtAudience],
) -> McpDomainResult<JwtClaims> {
    let policy = ValidationPolicy::issuer_scoped(issuer, audiences);
    decode_rs256_claims(keys, token, &policy).map_err(|e| classify_auth_error(e, issuer))
}

fn classify_auth_error(error: AuthError, issuer: &str) -> McpDomainError {
    if error.is_issuer_mismatch() {
        return McpDomainError::TokenIssuerMismatch {
            expected: issuer.to_owned(),
        };
    }
    McpDomainError::TokenRejected(error)
}
