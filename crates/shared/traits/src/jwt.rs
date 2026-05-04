//! JWT validation and generation provider trait.

use std::sync::Arc;
use systemprompt_identifiers::{SessionId, UserId};

/// Result alias for [`JwtValidationProvider`] operations.
pub type JwtResult<T> = Result<T, JwtProviderError>;

/// Errors returned by JWT providers.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum JwtProviderError {
    /// The token failed structural or signature validation.
    #[error("Invalid token")]
    InvalidToken,

    /// The token is structurally valid but its `exp` claim has passed.
    #[error("Token expired")]
    TokenExpired,

    /// The token is missing a required audience claim.
    #[error("Missing audience: {0}")]
    MissingAudience(String),

    /// The provider is misconfigured (missing key, bad algorithm, ...).
    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    /// Catch-all for unexpected provider failures.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for JwtProviderError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

/// Decoded JWT claims used by the agent runtime.
#[derive(Debug, Clone)]
pub struct AgentJwtClaims {
    /// `sub` claim — the principal identifier.
    pub subject: String,
    /// Login or display name embedded in the token.
    pub username: String,
    /// User category (`user`, `service`, ...).
    pub user_type: String,
    /// Audience claims (`aud`).
    pub audiences: Vec<String>,
    /// Permission strings carried by the token.
    pub permissions: Vec<String>,
    /// Whether the token grants admin privileges.
    pub is_admin: bool,
    /// Expiry as a unix timestamp.
    pub expires_at: i64,
    /// Issued-at timestamp.
    pub issued_at: i64,
}

impl AgentJwtClaims {
    /// Report whether `audience` appears in the token's audience list.
    #[must_use]
    pub fn has_audience(&self, audience: &str) -> bool {
        self.audiences.iter().any(|a| a == audience)
    }

    /// Report whether `permission` is granted by the token.
    #[must_use]
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.iter().any(|p| p == permission)
    }
}

/// Builder-style payload describing a token to be minted.
#[derive(Debug, Clone)]
pub struct GenerateTokenParams {
    /// Owning user.
    pub user_id: UserId,
    /// Username embedded in the token.
    pub username: String,
    /// User category (`user`, `service`, ...).
    pub user_type: String,
    /// Permissions to embed in the token.
    pub permissions: Vec<String>,
    /// Audience claims to embed.
    pub audiences: Vec<String>,
    /// Session that owns the token.
    pub session_id: SessionId,
    /// Optional override for the default expiry.
    pub expires_in_hours: Option<u32>,
}

impl GenerateTokenParams {
    /// Construct a new params instance with defaults.
    #[must_use]
    pub fn new(user_id: UserId, username: impl Into<String>, session_id: SessionId) -> Self {
        Self {
            user_id,
            username: username.into(),
            user_type: "user".to_string(),
            permissions: Vec::new(),
            audiences: Vec::new(),
            session_id,
            expires_in_hours: None,
        }
    }

    /// Override the user category.
    #[must_use]
    pub fn with_user_type(mut self, user_type: impl Into<String>) -> Self {
        self.user_type = user_type.into();
        self
    }

    /// Replace the permission list.
    #[must_use]
    pub fn with_permissions(mut self, permissions: Vec<String>) -> Self {
        self.permissions = permissions;
        self
    }

    /// Replace the audience list.
    #[must_use]
    pub fn with_audiences(mut self, audiences: Vec<String>) -> Self {
        self.audiences = audiences;
        self
    }

    /// Override token expiry in hours.
    #[must_use]
    pub const fn with_expires_in_hours(mut self, hours: u32) -> Self {
        self.expires_in_hours = Some(hours);
        self
    }
}

/// Validate inbound JWTs and mint new ones.
pub trait JwtValidationProvider: Send + Sync {
    /// Decode and validate `token`, returning its [`AgentJwtClaims`].
    fn validate_token(&self, token: &str) -> JwtResult<AgentJwtClaims>;

    /// Mint a token from the supplied [`GenerateTokenParams`].
    fn generate_token(&self, params: GenerateTokenParams) -> JwtResult<String>;

    /// Generate a random opaque token (for CSRF, refresh, etc.) using
    /// `prefix` as a human-readable namespace.
    fn generate_secure_token(&self, prefix: &str) -> String;
}

/// Shared `Arc` alias for [`JwtValidationProvider`].
pub type DynJwtValidationProvider = Arc<dyn JwtValidationProvider>;
