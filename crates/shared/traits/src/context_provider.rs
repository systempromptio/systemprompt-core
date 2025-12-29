//! Context provider trait for accessing user contexts.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
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
    pub context_id: String,
    pub user_id: String,
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
        user_id: &str,
    ) -> Result<Vec<ContextWithStats>, ContextProviderError>;

    async fn get_context(
        &self,
        context_id: &str,
        user_id: &str,
    ) -> Result<ContextWithStats, ContextProviderError>;

    async fn create_context(
        &self,
        user_id: &str,
        session_id: Option<&str>,
        name: &str,
    ) -> Result<String, ContextProviderError>;

    async fn update_context_name(
        &self,
        context_id: &str,
        user_id: &str,
        name: &str,
    ) -> Result<(), ContextProviderError>;

    async fn delete_context(
        &self,
        context_id: &str,
        user_id: &str,
    ) -> Result<(), ContextProviderError>;
}

pub type DynContextProvider = Arc<dyn ContextProvider>;
