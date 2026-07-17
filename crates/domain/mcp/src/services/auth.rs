//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::{McpDomainError, McpDomainResult};
use systemprompt_models::auth::{JwtAudience, JwtClaims};
use systemprompt_security::jwt::{ValidationPolicy, decode_rs256_claims};

pub fn validate_jwt_token(
    token: &str,
    issuer: &str,
    audiences: &[JwtAudience],
) -> McpDomainResult<JwtClaims> {
    let policy = ValidationPolicy::issuer_scoped(issuer, audiences);
    decode_rs256_claims(token, &policy).map_err(|e| McpDomainError::Internal(e.to_string()))
}
