use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ContextId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextListOutput {
    pub contexts: Vec<ContextSummary>,
    pub total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_context_id: Option<ContextId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSummary {
    pub id: ContextId,
    pub name: String,
    pub task_count: i64,
    pub message_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_message_at: Option<DateTime<Utc>>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextDetailOutput {
    pub id: ContextId,
    pub name: String,
    pub task_count: i64,
    pub message_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_message_at: Option<DateTime<Utc>>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextCreatedOutput {
    pub id: ContextId,
    pub name: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextUpdatedOutput {
    pub id: ContextId,
    pub name: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextDeletedOutput {
    pub id: ContextId,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSwitchedOutput {
    pub id: ContextId,
    pub name: String,
    pub message: String,
}
