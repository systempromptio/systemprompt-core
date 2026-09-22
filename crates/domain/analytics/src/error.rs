//! Typed error boundary for the `systemprompt-analytics` crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::domain_error;

domain_error! {
    pub enum AnalyticsError {
        common: [repository, io, json],

        #[error("session owner: {0}")]
        SessionOwner(#[from] systemprompt_traits::AnalyticsProviderError),

        #[error("Analytics event store failed: {0}")]
        EventStore(#[from] systemprompt_traits::RepositoryError),

        #[error("Session not found: {0}")]
        SessionNotFound(String),

        #[error("Invalid fingerprint hash: {0}")]
        InvalidFingerprint(String),

        #[error("Missing field: {0}")]
        MissingField(String),

        #[error("Invalid argument: {0}")]
        InvalidArgument(String),

        #[error("Reporting baseline rebuild superseded by a newer generation")]
        RebuildSuperseded,

        #[error("Session expired")]
        SessionExpired,

        #[error("Behavioral bot detected: {0}")]
        BehavioralBotDetected(String),

        #[error("Anomaly detection failed: {0}")]
        AnomalyDetectionFailed(String),
    }
}

impl From<sqlx::Error> for AnalyticsError {
    fn from(err: sqlx::Error) -> Self {
        Self::Repository(systemprompt_database::RepositoryError::from(err))
    }
}

impl AnalyticsError {
    pub fn missing_field<T: std::fmt::Display>(field: T) -> Self {
        Self::MissingField(field.to_string())
    }

    pub fn invalid_argument<T: Into<String>>(message: T) -> Self {
        Self::InvalidArgument(message.into())
    }

    pub const fn rebuild_superseded() -> Self {
        Self::RebuildSuperseded
    }

    pub const fn is_rebuild_superseded(&self) -> bool {
        matches!(self, Self::RebuildSuperseded)
    }
}

pub type Result<T> = std::result::Result<T, AnalyticsError>;
