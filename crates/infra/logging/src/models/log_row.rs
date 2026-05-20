use chrono::{DateTime, Utc};
use sqlx::FromRow;
use systemprompt_identifiers::{ClientId, ContextId, LogId, SessionId, TaskId, TraceId, UserId};

use super::{LogEntry, LogLevel, LoggingError};

type Result<T> = std::result::Result<T, LoggingError>;

#[derive(Debug, FromRow)]
pub struct LogRow {
    pub id: LogId,
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub module: String,
    pub message: String,
    pub metadata: Option<String>,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub task_id: Option<TaskId>,
    pub trace_id: Option<TraceId>,
    pub context_id: Option<ContextId>,
    pub client_id: Option<ClientId>,
}

impl LogRow {
    pub fn from_json_row(row: &systemprompt_database::JsonRow) -> Result<Self> {
        let missing = |col: &str| LoggingError::MissingColumn {
            column: col.to_string(),
        };

        let id = row
            .get("id")
            .and_then(|v| v.as_str())
            .map(LogId::new)
            .ok_or_else(|| missing("id"))?;

        let timestamp = row
            .get("timestamp")
            .and_then(systemprompt_database::parse_database_datetime)
            .ok_or_else(|| missing("timestamp"))?;

        let level = row
            .get("level")
            .and_then(|v| v.as_str())
            .ok_or_else(|| missing("level"))?
            .to_string();

        let module = row
            .get("module")
            .and_then(|v| v.as_str())
            .ok_or_else(|| missing("module"))?
            .to_string();

        let message = row
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| missing("message"))?
            .to_string();

        let metadata = row
            .get("metadata")
            .and_then(|v| v.as_str())
            .map(String::from);

        let user_id = row.get("user_id").and_then(|v| v.as_str()).map(UserId::new);

        let session_id = row
            .get("session_id")
            .and_then(|v| v.as_str())
            .map(SessionId::new);

        let task_id = row.get("task_id").and_then(|v| v.as_str()).map(TaskId::new);

        let trace_id = row
            .get("trace_id")
            .and_then(|v| v.as_str())
            .map(TraceId::new);

        let context_id = row
            .get("context_id")
            .and_then(|v| v.as_str())
            .map(ContextId::new);

        let client_id = row
            .get("client_id")
            .and_then(|v| v.as_str())
            .map(ClientId::new);

        Ok(Self {
            id,
            timestamp,
            level,
            module,
            message,
            metadata,
            user_id,
            session_id,
            task_id,
            trace_id,
            context_id,
            client_id,
        })
    }
}

impl From<LogRow> for LogEntry {
    fn from(row: LogRow) -> Self {
        let level = row.level.parse().unwrap_or(LogLevel::Info);

        Self {
            id: row.id,
            timestamp: row.timestamp,
            level,
            module: row.module,
            message: row.message,
            metadata: row.metadata.and_then(|s| {
                serde_json::from_str(&s)
                    .map_err(|e| {
                        tracing::warn!(error = %e, "Malformed log metadata JSON");
                        e
                    })
                    .ok()
            }),
            user_id: row.user_id.unwrap_or_else(UserId::admin),
            session_id: row.session_id.unwrap_or_else(SessionId::system),
            task_id: row.task_id,
            trace_id: row.trace_id.unwrap_or_else(TraceId::system),
            context_id: row.context_id,
            client_id: row.client_id,
        }
    }
}
