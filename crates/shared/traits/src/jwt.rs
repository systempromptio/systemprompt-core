use std::sync::Arc;
use systemprompt_identifiers::SessionId;

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

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for JwtProviderError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct AgentJwtClaims {
    pub subject: String,
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

#[derive(Debug, Clone)]
pub struct GenerateTokenParams {
    pub user_id: String,
    pub username: String,
    pub user_type: String,
    pub permissions: Vec<String>,
    pub audiences: Vec<String>,
    pub session_id: SessionId,
    pub expires_in_hours: Option<u32>,
}

impl GenerateTokenParams {
    #[must_use]
    pub fn new(
        user_id: impl Into<String>,
        username: impl Into<String>,
        session_id: SessionId,
    ) -> Self {
        Self {
            user_id: user_id.into(),
            username: username.into(),
            user_type: "user".to_string(),
            permissions: Vec::new(),
            audiences: Vec::new(),
            session_id,
            expires_in_hours: None,
        }
    }

    #[must_use]
    pub fn with_user_type(mut self, user_type: impl Into<String>) -> Self {
        self.user_type = user_type.into();
        self
    }

    #[must_use]
    pub fn with_permissions(mut self, permissions: Vec<String>) -> Self {
        self.permissions = permissions;
        self
    }

    #[must_use]
    pub fn with_audiences(mut self, audiences: Vec<String>) -> Self {
        self.audiences = audiences;
        self
    }

    #[must_use]
    pub const fn with_expires_in_hours(mut self, hours: u32) -> Self {
        self.expires_in_hours = Some(hours);
        self
    }
}

pub trait JwtValidationProvider: Send + Sync {
    fn validate_token(&self, token: &str) -> JwtResult<AgentJwtClaims>;

    fn generate_token(&self, params: GenerateTokenParams) -> JwtResult<String>;

    fn generate_secure_token(&self, prefix: &str) -> String;
}

pub type DynJwtValidationProvider = Arc<dyn JwtValidationProvider>;
