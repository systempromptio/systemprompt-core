use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LogEventLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl std::str::FromStr for LogEventLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "error" => Ok(Self::Error),
            "warn" | "warning" => Ok(Self::Warn),
            "info" => Ok(Self::Info),
            "debug" => Ok(Self::Debug),
            "trace" => Ok(Self::Trace),
            _ => Err(format!("unknown log level: {s}")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogEventData {
    pub timestamp: DateTime<Utc>,
    pub level: LogEventLevel,
    pub module: String,
    pub message: String,
}

impl LogEventData {
    #[must_use]
    pub fn new(
        timestamp: DateTime<Utc>,
        level: LogEventLevel,
        module: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            timestamp,
            level,
            module: module.into(),
            message: message.into(),
        }
    }
}

pub trait LogEventPublisher: Send + Sync {
    fn publish_log(&self, event: LogEventData);
}

/// Events for user/session changes
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum UserEvent {
    UserCreated { user_id: String },
    UserUpdated { user_id: String },
    SessionCreated { user_id: String, session_id: String },
    SessionEnded { user_id: String, session_id: String },
}

/// Publisher for user-related events
pub trait UserEventPublisher: Send + Sync {
    fn publish_user_event(&self, event: UserEvent);
}

/// Events for analytics updates
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum AnalyticsEvent {
    /// Analytics data has been updated
    Updated,
    /// A new AI request was completed
    AiRequestCompleted { tokens_used: i64 },
    /// User activity was recorded
    UserActivityRecorded { user_id: String },
}

/// Publisher for analytics events
pub trait AnalyticsEventPublisher: Send + Sync {
    fn publish_analytics_event(&self, event: AnalyticsEvent);
}
