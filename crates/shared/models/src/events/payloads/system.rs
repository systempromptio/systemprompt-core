use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ContextId;
use systemprompt_traits::ContextWithStats;

use crate::api::contexts::UserContextWithStats;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextCreatedPayload {
    pub context_id: ContextId,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextUpdatedPayload {
    pub context_id: ContextId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextDeletedPayload {
    pub context_id: ContextId,
}

/// A context summary for snapshot events (frontend-facing)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ContextSummary {
    pub context_id: ContextId,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub message_count: i64,
    pub task_count: i64,
}

impl From<UserContextWithStats> for ContextSummary {
    fn from(c: UserContextWithStats) -> Self {
        Self {
            context_id: c.context_id.clone(),
            name: c.name,
            created_at: c.created_at,
            updated_at: c.updated_at,
            message_count: c.message_count,
            task_count: c.task_count,
        }
    }
}

impl From<&UserContextWithStats> for ContextSummary {
    fn from(c: &UserContextWithStats) -> Self {
        Self {
            context_id: c.context_id.clone(),
            name: c.name.clone(),
            created_at: c.created_at,
            updated_at: c.updated_at,
            message_count: c.message_count,
            task_count: c.task_count,
        }
    }
}

impl From<ContextWithStats> for ContextSummary {
    fn from(c: ContextWithStats) -> Self {
        Self {
            context_id: ContextId::new(c.context_id),
            name: c.name,
            created_at: c.created_at,
            updated_at: c.updated_at,
            message_count: c.message_count,
            task_count: c.task_count,
        }
    }
}

impl From<&ContextWithStats> for ContextSummary {
    fn from(c: &ContextWithStats) -> Self {
        Self {
            context_id: ContextId::new(&c.context_id),
            name: c.name.clone(),
            created_at: c.created_at,
            updated_at: c.updated_at,
            message_count: c.message_count,
            task_count: c.task_count,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextsSnapshotPayload {
    pub contexts: Vec<ContextSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectedPayload {
    pub connection_id: String,
}
