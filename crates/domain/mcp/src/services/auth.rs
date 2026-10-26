//! MCP request authentication helpers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::{McpDomainError, McpDomainResult};
use systemprompt_models::auth::{JwtAudience, JwtClaims, TokenAuthority, TokenError};

pub fn validate_jwt_token(
    authority: &dyn TokenAuthority,
    token: &str,
    audiences: &[JwtAudience],
) -> McpDomainResult<JwtClaims> {
    authority
        .verify_bearer(token, audiences)
        .map_err(|error| match error {
            TokenError::InvalidIssuer => McpDomainError::TokenIssuerMismatch,
            other => McpDomainError::TokenRejected(other),
        })
}
