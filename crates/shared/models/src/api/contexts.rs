//! Context API models
//!
//! Shared types for user context management used across API endpoints and
//! clients.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use systemprompt_identifiers::{ContextId, UserId};

/// A user context representing a conversation/session workspace
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserContext {
    pub context_id: ContextId,
    pub user_id: UserId,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// User context with additional statistics
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserContextWithStats {
    pub context_id: ContextId,
    pub user_id: UserId,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub task_count: i64,
    pub message_count: i64,
    pub last_message_at: Option<DateTime<Utc>>,
}

/// Request payload for creating a new context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateContextRequest {
    /// Optional name for the context. If not provided, a default name will be
    /// generated.
    pub name: Option<String>,
}

/// Request payload for updating an existing context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateContextRequest {
    pub name: String,
}
