pub use systemprompt_traits::RepositoryError;

use crate::api::ApiError;

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

    #[error("User not found: {user_id}")]
    UserNotFound { user_id: String },

    #[error("Session not found: {session_id}")]
    SessionNotFound { session_id: String },

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
    pub fn reason(&self) -> String {
        self.to_string()
    }

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

    pub const fn is_client_error(&self) -> bool {
        self.status_code() < 500
    }

    pub const fn is_server_error(&self) -> bool {
        self.status_code() >= 500
    }

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

    pub const fn is_permission_error(&self) -> bool {
        matches!(self, Self::Forbidden { .. })
    }

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

impl From<anyhow::Error> for CoreError {
    fn from(err: anyhow::Error) -> Self {
        Self::InternalError {
            reason: err.to_string(),
        }
    }
}

impl From<sqlx::Error> for CoreError {
    fn from(err: sqlx::Error) -> Self {
        Self::DatabaseError {
            reason: err.to_string(),
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

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("repository error: {0}")]
    Repository(#[from] RepositoryError),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("business logic error: {0}")]
    BusinessLogic(String),

    #[error("external service error: {0}")]
    External(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("forbidden: {0}")]
    Forbidden(String),
}

impl From<ServiceError> for ApiError {
    fn from(err: ServiceError) -> Self {
        match err {
            ServiceError::Repository(e) => e.into(),
            ServiceError::Validation(msg) | ServiceError::BusinessLogic(msg) => {
                Self::bad_request(msg)
            },
            ServiceError::NotFound(msg) => Self::not_found(msg),
            ServiceError::External(msg) => {
                Self::internal_error(format!("External service error: {msg}"))
            },
            ServiceError::Conflict(msg) => Self::conflict(msg),
            ServiceError::Unauthorized(msg) => Self::unauthorized(msg),
            ServiceError::Forbidden(msg) => Self::forbidden(msg),
        }
    }
}

impl From<RepositoryError> for ApiError {
    fn from(err: RepositoryError) -> Self {
        match err {
            RepositoryError::NotFound(msg) => Self::not_found(msg),
            RepositoryError::InvalidData(msg) | RepositoryError::ConstraintViolation(msg) => {
                Self::bad_request(msg)
            },
            RepositoryError::Database(e) => {
                Self::internal_error(format!("Database error: {e}"))
            },
            RepositoryError::Serialization(e) => {
                Self::internal_error(format!("Serialization error: {e}"))
            },
            RepositoryError::Other(e) => Self::internal_error(format!("Error: {e}")),
            _ => Self::internal_error(format!("Repository error: {err}")),
        }
    }
}
