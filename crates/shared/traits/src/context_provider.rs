use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use systemprompt_identifiers::{ContextId, SessionId, UserId};

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ContextProviderError {
    #[error("Context not found: {0}")]
    NotFound(String),

    #[error("Access denied: {0}")]
    AccessDenied(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Clone)]
pub struct ContextWithStats {
    pub context_id: ContextId,
    pub user_id: UserId,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub task_count: i64,
    pub message_count: i64,
    pub last_message_at: Option<DateTime<Utc>>,
}

#[async_trait]
pub trait ContextProvider: Send + Sync {
    async fn list_contexts_with_stats(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<ContextWithStats>, ContextProviderError>;

    async fn get_context(
        &self,
        context_id: &ContextId,
        user_id: &UserId,
    ) -> Result<ContextWithStats, ContextProviderError>;

    async fn create_context(
        &self,
        user_id: &UserId,
        session_id: Option<&SessionId>,
        name: &str,
    ) -> Result<ContextId, ContextProviderError>;

    async fn update_context_name(
        &self,
        context_id: &ContextId,
        user_id: &UserId,
        name: &str,
    ) -> Result<(), ContextProviderError>;

    async fn delete_context(
        &self,
        context_id: &ContextId,
        user_id: &UserId,
    ) -> Result<(), ContextProviderError>;
}

pub type DynContextProvider = Arc<dyn ContextProvider>;
