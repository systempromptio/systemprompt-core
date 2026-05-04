//! Conversation context provider trait used by chat and agent surfaces.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use systemprompt_identifiers::{ContextId, SessionId, UserId};

/// Errors returned by context providers.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ContextProviderError {
    /// The requested context does not exist.
    #[error("Context not found: {0}")]
    NotFound(String),

    /// The caller is not allowed to read or modify the context.
    #[error("Access denied: {0}")]
    AccessDenied(String),

    /// The underlying database call failed.
    #[error("Database error: {0}")]
    Database(String),

    /// Catch-all for unexpected failures.
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Context summary plus message and task counters.
#[derive(Debug, Clone)]
pub struct ContextWithStats {
    /// Context identifier.
    pub context_id: ContextId,
    /// Owning user.
    pub user_id: UserId,
    /// Display name.
    pub name: String,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last-modified timestamp.
    pub updated_at: DateTime<Utc>,
    /// Number of tasks recorded against the context.
    pub task_count: i64,
    /// Number of messages recorded against the context.
    pub message_count: i64,
    /// Timestamp of the most recent message.
    pub last_message_at: Option<DateTime<Utc>>,
}

/// CRUD over conversation contexts.
///
/// `#[async_trait]` is required because the trait is consumed as
/// `Arc<dyn ContextProvider>` via [`DynContextProvider`].
#[async_trait]
pub trait ContextProvider: Send + Sync {
    /// List every context owned by `user_id`, with stats.
    async fn list_contexts_with_stats(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<ContextWithStats>, ContextProviderError>;

    /// Fetch the [`ContextWithStats`] for `context_id` owned by `user_id`.
    async fn get_context(
        &self,
        context_id: &ContextId,
        user_id: &UserId,
    ) -> Result<ContextWithStats, ContextProviderError>;

    /// Create a new context owned by `user_id` with the supplied `name`.
    async fn create_context(
        &self,
        user_id: &UserId,
        session_id: Option<&SessionId>,
        name: &str,
    ) -> Result<ContextId, ContextProviderError>;

    /// Rename `context_id` to `name`.
    async fn update_context_name(
        &self,
        context_id: &ContextId,
        user_id: &UserId,
        name: &str,
    ) -> Result<(), ContextProviderError>;

    /// Delete `context_id`.
    async fn delete_context(
        &self,
        context_id: &ContextId,
        user_id: &UserId,
    ) -> Result<(), ContextProviderError>;
}

/// Shared `Arc` alias for [`ContextProvider`].
pub type DynContextProvider = Arc<dyn ContextProvider>;
