//! Cross-cutting event publisher traits (logs, user activity, analytics).

use chrono::{DateTime, Utc};

/// Severity level for log events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LogEventLevel {
    /// Hard failure.
    Error,
    /// Recoverable problem.
    Warn,
    /// Informational.
    Info,
    /// Verbose diagnostic.
    Debug,
    /// Highest-volume diagnostic.
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

/// Single structured log entry suitable for downstream sinks.
#[derive(Debug, Clone)]
pub struct LogEventData {
    /// When the event was produced.
    pub timestamp: DateTime<Utc>,
    /// Severity.
    pub level: LogEventLevel,
    /// Module / target the event came from.
    pub module: String,
    /// Free-form message body.
    pub message: String,
}

impl LogEventData {
    /// Construct a [`LogEventData`].
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

/// Publish [`LogEventData`] to whichever sink the runtime has wired up.
pub trait LogEventPublisher: Send + Sync {
    /// Publish a single log event.
    fn publish_log(&self, event: LogEventData);
}

/// User-lifecycle events emitted by the auth and session layers.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum UserEvent {
    /// A new user account was created.
    UserCreated {
        /// Owning user id.
        user_id: String,
    },
    /// An existing user account was modified.
    UserUpdated {
        /// Owning user id.
        user_id: String,
    },
    /// A new session started.
    SessionCreated {
        /// Owning user id.
        user_id: String,
        /// Session identifier.
        session_id: String,
    },
    /// A session ended.
    SessionEnded {
        /// Owning user id.
        user_id: String,
        /// Session identifier.
        session_id: String,
    },
}

/// Publish [`UserEvent`]s to the analytics / activity layer.
pub trait UserEventPublisher: Send + Sync {
    /// Publish a single user event.
    fn publish_user_event(&self, event: UserEvent);
}

/// Cross-cutting analytics events not tied to a specific session.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum AnalyticsEvent {
    /// Generic update marker for triggering recomputation.
    Updated,
    /// An AI request finished and consumed `tokens_used` tokens.
    AiRequestCompleted {
        /// Number of tokens consumed by the request.
        tokens_used: i64,
    },
    /// A user activity record was persisted for `user_id`.
    UserActivityRecorded {
        /// Owning user id.
        user_id: String,
    },
}

/// Publish [`AnalyticsEvent`]s for downstream aggregation.
pub trait AnalyticsEventPublisher: Send + Sync {
    /// Publish a single analytics event.
    fn publish_analytics_event(&self, event: AnalyticsEvent);
}
