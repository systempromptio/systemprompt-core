//! JWT validation provider trait: verification only; no implementation
//! mints a token.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use systemprompt_identifiers::UserId;

use crate::BoxedSource;

pub type JwtResult<T> = Result<T, JwtProviderError>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum JwtProviderError {
    #[error("Invalid token")]
    InvalidToken,

    #[error("Token expired")]
    TokenExpired,

    #[error("Missing audience: {0}")]
    MissingAudience(String),

    #[error("Configuration error: {message}")]
    ConfigurationError { message: String },

    #[error("Internal error: {0}")]
    Internal(#[source] BoxedSource),
}

#[derive(Debug, Clone)]
pub struct AgentJwtClaims {
    pub subject: UserId,
    pub username: String,
    pub user_type: String,
    pub audiences: Vec<String>,
    pub permissions: Vec<String>,
    pub is_admin: bool,
    pub expires_at: i64,
    pub issued_at: i64,
}

impl AgentJwtClaims {
    #[must_use]
    pub fn has_audience(&self, audience: &str) -> bool {
        self.audiences.iter().any(|a| a == audience)
    }

    #[must_use]
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.iter().any(|p| p == permission)
    }
}

pub trait JwtValidationProvider: Send + Sync {
    fn validate_token(&self, token: &str) -> JwtResult<AgentJwtClaims>;

    fn generate_secure_token(&self, prefix: &str) -> String;
}

pub type DynJwtValidationProvider = Arc<dyn JwtValidationProvider>;
