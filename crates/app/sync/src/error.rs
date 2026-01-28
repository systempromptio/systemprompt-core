use thiserror::Error;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("API error {status}: {message}")]
    ApiError { status: u16, message: String },

    #[error("Unauthorized - run 'systemprompt cloud login'")]
    Unauthorized,

    #[error("Tenant has no associated app")]
    TenantNoApp,

    #[error("Must run from project root (with infrastructure/ directory)")]
    NotProjectRoot,

    #[error("Command failed: {command}")]
    CommandFailed { command: String },

    #[error("Docker login failed")]
    DockerLoginFailed,

    #[error("Git SHA unavailable")]
    GitShaUnavailable,

    #[error("Missing configuration: {0}")]
    MissingConfig(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Path error: {0}")]
    StripPrefix(#[from] std::path::StripPrefixError),
}

impl SyncError {
    pub const fn is_retryable(&self) -> bool {
        matches!(self, Self::Http(_))
            || matches!(
                self,
                Self::ApiError { status, .. }
                    if *status == 502 || *status == 503 || *status == 504 || *status == 429
            )
    }
}

pub type SyncResult<T> = Result<T, SyncError>;
