//! Test-only `LogRow` shape exposed for unit tests that exercise the row→entry
//! mapping shape. Production read paths use local non-Option `LogRow` structs
//! defined in `repository/operations/queries.rs` and `trace/log_lookup_queries.rs`;
//! they do not go through this type. The struct is preserved because removing
//! it would force a cross-cutting rewrite of the logging test surface; it must
//! not be used in production code.

use chrono::{DateTime, Utc};
use sqlx::FromRow;
use systemprompt_identifiers::{ClientId, ContextId, LogId, SessionId, TaskId, TraceId, UserId};

#[derive(Debug, FromRow)]
pub struct LogRow {
    pub id: LogId,
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub module: String,
    pub message: String,
    pub metadata: Option<String>,
    pub user_id: UserId,
    pub session_id: SessionId,
    pub task_id: Option<TaskId>,
    pub trace_id: TraceId,
    pub context_id: Option<ContextId>,
    pub client_id: Option<ClientId>,
}
