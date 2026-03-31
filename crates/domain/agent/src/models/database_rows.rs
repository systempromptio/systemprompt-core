use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use systemprompt_identifiers::{
    AgentId, AgentName, ArtifactId, CategoryId, ContextId, ExecutionStepId, McpExecutionId,
    MessageId, SessionId, SkillId, SourceId, TaskId, TraceId, UserId,
};
use systemprompt_models::{UserContext, UserContextWithStats};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub(crate) struct UserContextRow {
    pub(crate) context_id: ContextId,
    pub(crate) user_id: UserId,
    pub(crate) name: String,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
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
pub(crate) struct UserContextWithStatsRow {
    pub(crate) context_id: ContextId,
    pub(crate) user_id: UserId,
    pub(crate) name: String,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) task_count: i64,
    pub(crate) message_count: i64,
    pub(crate) last_message_at: Option<DateTime<Utc>>,
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
pub(crate) struct TaskRow {
    pub(crate) task_id: TaskId,
    pub(crate) context_id: ContextId,
    pub(crate) status: String,
    pub(crate) status_timestamp: Option<DateTime<Utc>>,
    pub(crate) user_id: Option<UserId>,
    pub(crate) session_id: Option<SessionId>,
    pub(crate) trace_id: Option<TraceId>,
    pub(crate) agent_name: Option<AgentName>,
    pub(crate) started_at: Option<DateTime<Utc>>,
    pub(crate) completed_at: Option<DateTime<Utc>>,
    pub(crate) execution_time_ms: Option<i32>,
    pub(crate) error_message: Option<String>,
    pub(crate) metadata: Option<serde_json::Value>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub(crate) struct TaskMessage {
    pub(crate) id: i32,
    pub(crate) task_id: TaskId,
    pub(crate) message_id: MessageId,
    pub(crate) client_message_id: Option<String>,
    pub(crate) role: String,
    pub(crate) context_id: Option<ContextId>,
    pub(crate) user_id: Option<UserId>,
    pub(crate) session_id: Option<SessionId>,
    pub(crate) trace_id: Option<TraceId>,
    pub(crate) sequence_number: i32,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) metadata: Option<serde_json::Value>,
    pub(crate) reference_task_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub(crate) struct MessagePart {
    pub(crate) id: i32,
    pub(crate) message_id: MessageId,
    pub(crate) task_id: TaskId,
    pub(crate) part_kind: String,
    pub(crate) sequence_number: i32,
    pub(crate) text_content: Option<String>,
    pub(crate) file_name: Option<String>,
    pub(crate) file_mime_type: Option<String>,
    pub(crate) file_uri: Option<String>,
    pub(crate) file_bytes: Option<String>,
    pub(crate) data_content: Option<serde_json::Value>,
    pub(crate) metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub(crate) struct SkillRow {
    pub(crate) skill_id: SkillId,
    pub(crate) file_path: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) instructions: String,
    pub(crate) enabled: bool,
    pub(crate) tags: Option<Vec<String>>,
    pub(crate) category_id: Option<CategoryId>,
    pub(crate) source_id: SourceId,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub(crate) struct ArtifactRow {
    pub(crate) artifact_id: ArtifactId,
    pub(crate) task_id: TaskId,
    pub(crate) context_id: Option<ContextId>,
    pub(crate) name: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) artifact_type: String,
    pub(crate) source: Option<String>,
    pub(crate) tool_name: Option<String>,
    pub(crate) mcp_execution_id: Option<McpExecutionId>,
    pub(crate) fingerprint: Option<String>,
    pub(crate) skill_id: Option<SkillId>,
    pub(crate) skill_name: Option<String>,
    pub(crate) metadata: Option<serde_json::Value>,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub(crate) struct ArtifactPartRow {
    pub(crate) id: i32,
    pub(crate) artifact_id: ArtifactId,
    pub(crate) context_id: ContextId,
    pub(crate) part_kind: String,
    pub(crate) sequence_number: i32,
    pub(crate) text_content: Option<String>,
    pub(crate) file_name: Option<String>,
    pub(crate) file_mime_type: Option<String>,
    pub(crate) file_uri: Option<String>,
    pub(crate) file_bytes: Option<String>,
    pub(crate) data_content: Option<serde_json::Value>,
    pub(crate) metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub(crate) struct ExecutionStepBatchRow {
    pub(crate) step_id: ExecutionStepId,
    pub(crate) task_id: TaskId,
    pub(crate) status: String,
    pub(crate) content: serde_json::Value,
    pub(crate) started_at: DateTime<Utc>,
    pub(crate) completed_at: Option<DateTime<Utc>>,
    pub(crate) duration_ms: Option<i32>,
    pub(crate) error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub(crate) struct AgentRow {
    pub(crate) agent_id: AgentId,
    pub(crate) name: String,
    pub(crate) display_name: String,
    pub(crate) description: String,
    pub(crate) version: String,
    pub(crate) system_prompt: Option<String>,
    pub(crate) enabled: bool,
    pub(crate) port: i32,
    pub(crate) endpoint: String,
    pub(crate) dev_only: bool,
    pub(crate) is_primary: bool,
    pub(crate) is_default: bool,
    pub(crate) tags: Option<Vec<String>>,
    pub(crate) category_id: Option<CategoryId>,
    pub(crate) source_id: SourceId,
    pub(crate) provider: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) mcp_servers: Option<Vec<String>>,
    pub(crate) skills: Option<Vec<String>>,
    pub(crate) card_json: serde_json::Value,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub(crate) struct PushNotificationConfigRow {
    pub(crate) id: String,
    pub(crate) task_id: TaskId,
    pub(crate) url: String,
    pub(crate) endpoint: String,
    pub(crate) token: Option<String>,
    pub(crate) headers: Option<serde_json::Value>,
    pub(crate) authentication: Option<serde_json::Value>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}
