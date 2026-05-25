use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{LogId, SessionId, TraceId, UserId};

use super::{LogLevel, LoggingError};
use crate::attribution::{LogAttributionUnset, platform_owner_id};

/// Mandatory attribution for every log row: who did the work, in which
/// session, on which trace. Bundled so every `LogEntry::new` call carries
/// the full triple instead of relying on hidden defaults.
// Why allow `struct_field_names`: the `_id` suffix is load-bearing here —
// it pairs each field with its typed identifier and matches the LogEntry
// field names so the constructor reads `entry.user_id = actor.user_id`.
#[expect(
    clippy::struct_field_names,
    reason = "the `_id` suffix is load-bearing — it pairs each field with its typed identifier \
              and matches the LogEntry field names so the constructor reads `entry.user_id = \
              actor.user_id`"
)]
#[derive(Debug, Clone)]
pub struct LogActor {
    pub user_id: UserId,
    pub session_id: SessionId,
    pub trace_id: TraceId,
}

impl LogActor {
    #[must_use]
    pub const fn new(user_id: UserId, session_id: SessionId, trace_id: TraceId) -> Self {
        Self {
            user_id,
            session_id,
            trace_id,
        }
    }

    /// Platform telemetry (gateway access logs, OTLP ingest) has no human
    /// originator, so it declares the resolved system-admin owner. Fails
    /// when the runtime has not yet installed the logging attribution; the
    /// caller must propagate the error rather than fabricating a sentinel.
    pub fn platform(trace_id: TraceId) -> Result<Self, LogAttributionUnset> {
        Ok(Self {
            user_id: platform_owner_id()?.clone(),
            session_id: SessionId::system(),
            trace_id,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: LogId,
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub module: String,
    pub message: String,
    pub metadata: Option<serde_json::Value>,
    pub user_id: UserId,
    pub session_id: SessionId,
    pub task_id: Option<systemprompt_identifiers::TaskId>,
    pub trace_id: TraceId,
    pub context_id: Option<systemprompt_identifiers::ContextId>,
    pub client_id: Option<systemprompt_identifiers::ClientId>,
}

impl LogEntry {
    pub fn new(
        level: LogLevel,
        module: impl Into<String>,
        message: impl Into<String>,
        actor: LogActor,
    ) -> Self {
        Self {
            id: LogId::generate(),
            timestamp: Utc::now(),
            level,
            module: module.into(),
            message: message.into(),
            metadata: None,
            user_id: actor.user_id,
            session_id: actor.session_id,
            task_id: None,
            trace_id: actor.trace_id,
            context_id: None,
            client_id: None,
        }
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
