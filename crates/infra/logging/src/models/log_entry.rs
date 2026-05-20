use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::LogId;

use super::{LogLevel, LoggingError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: LogId,
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub module: String,
    pub message: String,
    pub metadata: Option<serde_json::Value>,
    pub user_id: systemprompt_identifiers::UserId,
    pub session_id: systemprompt_identifiers::SessionId,
    pub task_id: Option<systemprompt_identifiers::TaskId>,
    pub trace_id: systemprompt_identifiers::TraceId,
    pub context_id: Option<systemprompt_identifiers::ContextId>,
    pub client_id: Option<systemprompt_identifiers::ClientId>,
}

impl LogEntry {
    pub fn new(
        level: LogLevel,
        module: impl Into<String>,
        message: impl Into<String>,
        user_id: systemprompt_identifiers::UserId,
        session_id: systemprompt_identifiers::SessionId,
        trace_id: systemprompt_identifiers::TraceId,
    ) -> Self {
        Self {
            id: LogId::generate(),
            timestamp: Utc::now(),
            level,
            module: module.into(),
            message: message.into(),
            metadata: None,
            user_id,
            session_id,
            task_id: None,
            trace_id,
            context_id: None,
            client_id: None,
        }
    }

    /// Constructor for platform-attributed log rows that have no human originator —
    /// external telemetry ingest (OTLP), gateway access logs emitted by middleware
    /// before any request context is bound, and similar platform-internal events.
    ///
    /// Why this is the only sanctioned `UserId::admin()` call site in the logging
    /// layer: per the security policy every log row must resolve to a real user,
    /// even when that user is the platform owner. Routing these events through a
    /// named constructor makes the platform attribution explicit at the call site
    /// instead of hiding behind a Default.
    #[must_use]
    pub fn platform_event(
        level: LogLevel,
        module: impl Into<String>,
        message: impl Into<String>,
        trace_id: systemprompt_identifiers::TraceId,
    ) -> Self {
        Self::new(
            level,
            module,
            message,
            systemprompt_identifiers::UserId::admin(),
            systemprompt_identifiers::SessionId::system(),
            trace_id,
        )
    }

    #[must_use]
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    #[must_use]
    pub fn with_task_id(mut self, task_id: systemprompt_identifiers::TaskId) -> Self {
        self.task_id = Some(task_id);
        self
    }

    #[must_use]
    pub fn with_context_id(mut self, context_id: systemprompt_identifiers::ContextId) -> Self {
        self.context_id = Some(context_id);
        self
    }

    #[must_use]
    pub fn with_client_id(mut self, client_id: systemprompt_identifiers::ClientId) -> Self {
        self.client_id = Some(client_id);
        self
    }

    pub fn validate(&self) -> Result<(), LoggingError> {
        if self.module.is_empty() {
            return Err(LoggingError::EmptyModuleName);
        }
        if self.message.is_empty() {
            return Err(LoggingError::EmptyMessage);
        }
        if let Some(metadata) = &self.metadata {
            if !metadata.is_object()
                && !metadata.is_array()
                && !metadata.is_string()
                && !metadata.is_null()
            {
                return Err(LoggingError::InvalidMetadata);
            }
        }
        Ok(())
    }
}

impl std::fmt::Display for LogEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let level_str = match self.level {
            LogLevel::Error => "ERROR",
            LogLevel::Warn => "WARN ",
            LogLevel::Info => "INFO ",
            LogLevel::Debug => "DEBUG",
            LogLevel::Trace => "TRACE",
        };

        let timestamp_str = self.timestamp.format("%H:%M:%S");

        if let Some(metadata) = &self.metadata {
            write!(
                f,
                "{} [{}] {}: {} {}",
                timestamp_str,
                level_str,
                self.module,
                self.message,
                serde_json::to_string(metadata).unwrap_or_else(|_| String::new())
            )
        } else {
            write!(
                f,
                "{} [{}] {}: {}",
                timestamp_str, level_str, self.module, self.message
            )
        }
    }
}
