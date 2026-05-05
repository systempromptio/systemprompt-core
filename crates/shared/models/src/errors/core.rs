//! The legacy [`CoreError`] umbrella enum — a closed set of variants
//! covering authentication, session, module, and IO failure modes with
//! HTTP status code mapping helpers.

use systemprompt_identifiers::{SessionId, UserId};

#[derive(Debug, Clone, thiserror::Error)]
pub enum CoreError {
    #[error("Module config missing required field: {field}")]
    MissingConfigField { field: String },

    #[error("Invalid module version: {version}")]
    InvalidVersion { version: String },

    #[error("Module {name} configuration invalid: {reason}")]
    InvalidModuleConfig { name: String, reason: String },

    #[error("Module {name} not found")]
    ModuleNotFound { name: String },

    #[error("Invalid module field {field}: {reason}")]
    InvalidField { field: String, reason: String },

    #[error("Version comparison failed: {reason}")]
    VersionComparisonFailed { reason: String },

    #[error("Authentication failed: {reason}")]
    AuthenticationFailed { reason: String },

    #[error("Invalid or expired token")]
    InvalidToken,

    #[error("Token expired")]
    TokenExpired,

    #[error("Invalid token signature")]
    InvalidSignature,

    #[error("Missing required claim: {claim}")]
    MissingClaim { claim: String },

    #[error("Invalid authorization header")]
    InvalidAuthHeader,

    #[error("Invalid token format")]
    InvalidTokenFormat,

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Forbidden: {reason}")]
    Forbidden { reason: String },

    #[error("User not found: {}", user_id.as_str())]
    UserNotFound { user_id: UserId },

    #[error("Session not found: {}", session_id.as_str())]
    SessionNotFound { session_id: SessionId },

    #[error("Invalid session: {reason}")]
    InvalidSession { reason: String },

    #[error("Session expired")]
    SessionExpired,

    #[error("Cookie not found")]
    CookieNotFound,

    #[error("Invalid cookie format")]
    InvalidCookieFormat,

    #[error("Database error: {reason}")]
    DatabaseError { reason: String },

    #[error("Table not found: {table}")]
    TableNotFound { table: String },

    #[error("Schema error: {message}")]
    SchemaError { message: String },

    #[error("File not found: {path}")]
    FileNotFound { path: String },

    #[error("IO error: {reason}")]
    IoError { reason: String },

    #[error("Internal server error: {reason}")]
    InternalError { reason: String },
}

impl CoreError {
    #[must_use]
    pub fn reason(&self) -> String {
        self.to_string()
    }

    #[must_use]
    pub const fn status_code(&self) -> u16 {
        match self {
            Self::AuthenticationFailed { .. }
            | Self::InvalidToken
            | Self::TokenExpired
            | Self::InvalidSignature
            | Self::Unauthorized
            | Self::SessionExpired => 401,
            Self::MissingClaim { .. }
            | Self::InvalidAuthHeader
            | Self::InvalidTokenFormat
            | Self::InvalidSession { .. }
            | Self::CookieNotFound
            | Self::InvalidCookieFormat
            | Self::MissingConfigField { .. }
            | Self::InvalidVersion { .. }
            | Self::InvalidModuleConfig { .. }
            | Self::InvalidField { .. }
            | Self::VersionComparisonFailed { .. } => 400,
            Self::Forbidden { .. } => 403,
            Self::UserNotFound { .. }
            | Self::SessionNotFound { .. }
            | Self::ModuleNotFound { .. }
            | Self::TableNotFound { .. }
            | Self::FileNotFound { .. } => 404,
            Self::DatabaseError { .. }
            | Self::SchemaError { .. }
            | Self::IoError { .. }
            | Self::InternalError { .. } => 500,
        }
    }

    #[must_use]
    pub const fn is_client_error(&self) -> bool {
        self.status_code() < 500
    }

    #[must_use]
    pub const fn is_server_error(&self) -> bool {
        self.status_code() >= 500
    }

    #[must_use]
    pub const fn is_auth_error(&self) -> bool {
        matches!(
            self,
            Self::AuthenticationFailed { .. }
                | Self::InvalidToken
                | Self::TokenExpired
                | Self::InvalidSignature
                | Self::InvalidAuthHeader
                | Self::InvalidTokenFormat
                | Self::Unauthorized
                | Self::SessionExpired
        )
    }

    #[must_use]
    pub const fn is_permission_error(&self) -> bool {
        matches!(self, Self::Forbidden { .. })
    }

    #[must_use]
    pub const fn is_not_found(&self) -> bool {
        matches!(
            self,
            Self::UserNotFound { .. }
                | Self::SessionNotFound { .. }
                | Self::ModuleNotFound { .. }
                | Self::TableNotFound { .. }
                | Self::FileNotFound { .. }
        )
    }
}

impl From<String> for CoreError {
    fn from(reason: String) -> Self {
        Self::InternalError { reason }
    }
}

impl From<&str> for CoreError {
    fn from(reason: &str) -> Self {
        Self::InternalError {
            reason: reason.to_string(),
        }
    }
}

impl From<std::io::Error> for CoreError {
    fn from(err: std::io::Error) -> Self {
        Self::IoError {
            reason: err.to_string(),
        }
    }
}
