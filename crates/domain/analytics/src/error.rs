use thiserror::Error;

#[derive(Error, Debug)]
pub enum AnalyticsError {
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Invalid fingerprint hash: {0}")]
    InvalidFingerprint(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Session expired")]
    SessionExpired,

    #[error("Throttle level exceeded")]
    ThrottleLevelExceeded,

    #[error("Behavioral bot detected: {0}")]
    BehavioralBotDetected(String),

    #[error("Feature extraction failed: {0}")]
    FeatureExtractionFailed(String),

    #[error("Anomaly detection failed: {0}")]
    AnomalyDetectionFailed(String),
}

pub type Result<T> = std::result::Result<T, AnalyticsError>;
