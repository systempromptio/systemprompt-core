use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use systemprompt_identifiers::{
    AgentName, ArtifactId, CategoryId, ContextId, ExecutionStepId, McpExecutionId, MessageId,
    PlaybookId, SessionId, SkillId, SourceId, TaskId, TraceId, UserId,
};
use systemprompt_models::{UserContext, UserContextWithStats};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserContextRow {
    pub context_id: ContextId,
    pub user_id: UserId,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<UserContextRow> for UserContext {
    fn from(row: UserContextRow) -> Self {
        Self {
            context_id: row.context_id,
            user_id: row.user_id,
            name: row.name,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserContextWithStatsRow {
    pub context_id: ContextId,
    pub user_id: UserId,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub task_count: i64,
    pub message_count: i64,
    pub last_message_at: Option<DateTime<Utc>>,
}

impl From<UserContextWithStatsRow> for UserContextWithStats {
    fn from(row: UserContextWithStatsRow) -> Self {
        Self {
            context_id: row.context_id,
            user_id: row.user_id,
            name: row.name,
            created_at: row.created_at,
            updated_at: row.updated_at,
            task_count: row.task_count,
            message_count: row.message_count,
            last_message_at: row.last_message_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TaskRow {
    pub task_id: TaskId,
    pub context_id: ContextId,
    pub status: String,
    pub status_timestamp: Option<DateTime<Utc>>,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub trace_id: Option<TraceId>,
    pub agent_name: Option<AgentName>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub execution_time_ms: Option<i32>,
    pub error_message: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TaskMessage {
    pub id: i32,
    pub task_id: TaskId,
    pub message_id: MessageId,
    pub client_message_id: Option<String>,
    pub role: String,
    pub context_id: Option<ContextId>,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub trace_id: Option<TraceId>,
    pub sequence_number: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
    pub reference_task_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MessagePart {
    pub id: i32,
    pub message_id: MessageId,
    pub task_id: TaskId,
    pub part_kind: String,
    pub sequence_number: i32,
    pub text_content: Option<String>,
    pub file_name: Option<String>,
    pub file_mime_type: Option<String>,
    pub file_uri: Option<String>,
    pub file_bytes: Option<String>,
    pub data_content: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SkillRow {
    pub skill_id: SkillId,
    pub file_path: String,
    pub name: String,
    pub description: String,
    pub instructions: String,
    pub enabled: bool,
    pub tags: Option<Vec<String>>,
    pub category_id: Option<CategoryId>,
    pub source_id: SourceId,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PlaybookRow {
    pub playbook_id: PlaybookId,
    pub file_path: String,
    pub name: String,
    pub description: String,
    pub instructions: String,
    pub enabled: bool,
    pub tags: Option<Vec<String>>,
    pub category: String,
    pub domain: String,
    pub source_id: SourceId,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ArtifactRow {
    pub artifact_id: ArtifactId,
    pub task_id: TaskId,
    pub context_id: Option<ContextId>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub artifact_type: String,
    pub source: Option<String>,
    pub tool_name: Option<String>,
    pub mcp_execution_id: Option<McpExecutionId>,
    pub fingerprint: Option<String>,
    pub skill_id: Option<SkillId>,
    pub skill_name: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ArtifactPartRow {
    pub id: i32,
    pub artifact_id: ArtifactId,
    pub context_id: ContextId,
    pub part_kind: String,
    pub sequence_number: i32,
    pub text_content: Option<String>,
    pub file_name: Option<String>,
    pub file_mime_type: Option<String>,
    pub file_uri: Option<String>,
    pub file_bytes: Option<String>,
    pub data_content: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ExecutionStepBatchRow {
    pub step_id: ExecutionStepId,
    pub task_id: TaskId,
    pub status: String,
    pub content: serde_json::Value,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i32>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PushNotificationConfigRow {
    pub id: String,
    pub task_id: TaskId,
    pub url: String,
    pub endpoint: String,
    pub token: Option<String>,
    pub headers: Option<serde_json::Value>,
    pub authentication: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
