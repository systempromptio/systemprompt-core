//! Typed error boundary for the `systemprompt-analytics` crate.
//!
//! [`AnalyticsError`] is the canonical error returned from public, non-trait
//! signatures (repositories, services, helpers). It composes [`sqlx::Error`],
//! [`systemprompt_database::RepositoryError`], [`serde_json::Error`] and
//! upstream [`anyhow::Error`] via `#[from]` so `?` propagation works
//! transparently for every internal call site.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AnalyticsError {
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Invalid fingerprint hash: {0}")]
    InvalidFingerprint(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Repository error: {0}")]
    Repository(#[from] systemprompt_database::RepositoryError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Missing field: {0}")]
    MissingField(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Session expired")]
    SessionExpired,

    #[error("Throttle level exceeded")]
    ThrottleLevelExceeded,

    #[error("Behavioral bot detected: {0}")]
    BehavioralBotDetected(String),

    #[error("Anomaly detection failed: {0}")]
    AnomalyDetectionFailed(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl AnalyticsError {
    pub fn missing_field<T: std::fmt::Display>(field: T) -> Self {
        Self::MissingField(field.to_string())
    }

    pub fn invalid_argument<T: Into<String>>(message: T) -> Self {
        Self::InvalidArgument(message.into())
    }
}

pub type Result<T> = std::result::Result<T, AnalyticsError>;
