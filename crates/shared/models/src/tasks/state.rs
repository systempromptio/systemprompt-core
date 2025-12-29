use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ContextId, MessageId, SessionId, TaskId, TraceId, UserId};

/// Database record for task persistence.
/// This represents the database table structure, NOT the A2A protocol Task
/// entity. For A2A protocol Task, see
/// `crates/modules/agent/src/models/a2a/task.rs`.
#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct TaskRecord {
    pub uuid: TaskId,
    pub context_id: ContextId,
    pub status: String,
    pub status_timestamp: Option<String>,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub trace_id: Option<TraceId>,
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct TaskMessage {
    pub id: i64,
    pub task_uuid: TaskId,
    pub message_id: MessageId,
    pub role: String,
    pub sequence_number: i64,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub trace_id: Option<TraceId>,
    pub metadata: String,
    pub reference_task_ids: Option<Vec<TaskId>>,
    pub created_at: String,
}
