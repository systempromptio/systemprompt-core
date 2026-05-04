//! Typed error boundary for the `systemprompt-analytics` crate.
//!
//! [`AnalyticsError`] is the canonical error returned from public, non-trait
//! signatures (repositories, services, helpers). It composes [`sqlx::Error`],
//! [`systemprompt_database::RepositoryError`], [`serde_json::Error`] and
//! upstream [`anyhow::Error`] via `#[from]` so `?` propagation works
//! transparently for every internal call site.

use thiserror::Error;

/// Canonical error type for `systemprompt-analytics` public APIs.
#[derive(Debug, Error)]
pub enum AnalyticsError {
    /// Requested analytics session was not found.
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    /// Provided fingerprint hash was syntactically invalid.
    #[error("Invalid fingerprint hash: {0}")]
    InvalidFingerprint(String),

    /// Underlying `sqlx` driver error (connection, decode, protocol).
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Repository-level error from `systemprompt-database` abstractions.
    #[error("Repository error: {0}")]
    Repository(#[from] systemprompt_database::RepositoryError),

    /// JSON serialisation/deserialisation failure for analytics payloads.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Required field missing from a database row or JSON document.
    #[error("Missing field: {0}")]
    MissingField(String),

    /// Caller supplied an invalid argument to an analytics API.
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    /// Session lifetime has elapsed.
    #[error("Session expired")]
    SessionExpired,

    /// Throttle escalation level exceeded the configured ceiling.
    #[error("Throttle level exceeded")]
    ThrottleLevelExceeded,

    /// Behavioural detector classified traffic as bot activity.
    #[error("Behavioral bot detected: {0}")]
    BehavioralBotDetected(String),

    /// Anomaly detector failed to evaluate metrics.
    #[error("Anomaly detection failed: {0}")]
    AnomalyDetectionFailed(String),

    /// I/O failure (filesystem, `MaxMind` `GeoIP` database, etc.).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Catch-all for upstream errors that have not been narrowed.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl AnalyticsError {
    /// Construct an [`AnalyticsError::MissingField`] from any displayable name.
    pub fn missing_field<T: std::fmt::Display>(field: T) -> Self {
        Self::MissingField(field.to_string())
    }

    /// Construct an [`AnalyticsError::InvalidArgument`] from any string-like
    /// message.
    pub fn invalid_argument<T: Into<String>>(message: T) -> Self {
        Self::InvalidArgument(message.into())
    }
}

/// Convenience alias used by all public, non-trait analytics APIs.
pub type Result<T> = std::result::Result<T, AnalyticsError>;
