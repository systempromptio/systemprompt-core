use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ContextId, McpExecutionId, MessageId, SkillId, TaskId};

pub use systemprompt_models::{
    CreateContextRequest, UpdateContextRequest, UserContext, UserContextWithStats,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMessage {
    pub message_id: MessageId,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub sequence_number: i32,
    pub parts: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextDetail {
    pub context: UserContext,
    pub messages: Vec<ContextMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContextStateEvent {
    ToolExecutionCompleted {
        context_id: ContextId,
        execution_id: McpExecutionId,
        tool_name: String,
        server_name: String,
        output: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        artifact: Option<super::a2a::Artifact>,
        status: String,
        timestamp: DateTime<Utc>,
    },
    TaskStatusChanged {
        task: super::a2a::Task,
        context_id: ContextId,
        timestamp: DateTime<Utc>,
    },
    ArtifactCreated {
        artifact: super::a2a::Artifact,
        task_id: TaskId,
        context_id: ContextId,
        timestamp: DateTime<Utc>,
    },
    SkillLoaded {
        skill_id: SkillId,
        skill_name: String,
        description: String,
        request_context: systemprompt_models::execution::context::RequestContext,
        tool_name: Option<String>,
        timestamp: DateTime<Utc>,
    },
    ContextCreated {
        context_id: ContextId,
        context: UserContext,
        timestamp: DateTime<Utc>,
    },
    ContextUpdated {
        context_id: ContextId,
        name: String,
        timestamp: DateTime<Utc>,
    },
    ContextDeleted {
        context_id: ContextId,
        timestamp: DateTime<Utc>,
    },
    Heartbeat {
        timestamp: DateTime<Utc>,
    },
    CurrentAgent {
        context_id: ContextId,
        agent_name: Option<String>,
        timestamp: DateTime<Utc>,
    },
}

impl ContextStateEvent {
    pub fn context_id(&self) -> Option<&str> {
        match self {
            Self::ToolExecutionCompleted { context_id, .. } => Some(context_id.as_str()),
            Self::TaskStatusChanged { context_id, .. } => Some(context_id.as_str()),
            Self::ArtifactCreated { context_id, .. } => Some(context_id.as_str()),
            Self::SkillLoaded {
                request_context, ..
            } => Some(request_context.context_id().as_str()),
            Self::ContextCreated { context_id, .. } => Some(context_id.as_str()),
            Self::ContextUpdated { context_id, .. } => Some(context_id.as_str()),
            Self::ContextDeleted { context_id, .. } => Some(context_id.as_str()),
            Self::Heartbeat { .. } => None,
            Self::CurrentAgent { context_id, .. } => Some(context_id.as_str()),
        }
    }

    pub const fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::ToolExecutionCompleted { timestamp, .. } => *timestamp,
            Self::TaskStatusChanged { timestamp, .. } => *timestamp,
            Self::ArtifactCreated { timestamp, .. } => *timestamp,
            Self::SkillLoaded { timestamp, .. } => *timestamp,
            Self::ContextCreated { timestamp, .. } => *timestamp,
            Self::ContextUpdated { timestamp, .. } => *timestamp,
            Self::ContextDeleted { timestamp, .. } => *timestamp,
            Self::Heartbeat { timestamp } => *timestamp,
            Self::CurrentAgent { timestamp, .. } => *timestamp,
        }
    }
}
