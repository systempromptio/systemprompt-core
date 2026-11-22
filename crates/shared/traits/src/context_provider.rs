//! Conversation context provider trait used by chat and agent surfaces.
//!
//! `ContextMaterializer`, `ContextDirectory` and `SessionContextIssuer` are
//! dispatched as trait
//! objects, so they use `#[async_trait]`; native `async fn` in traits is not
//! yet `dyn`-compatible. `ContextProvider` is only used
//! through concrete types and declares native `async` methods.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use systemprompt_identifiers::{ContextId, SessionId, UserId};

use crate::BoxedSource;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ContextProviderError {
    #[error("Context not found: {0}")]
    NotFound(String),

    #[error("Access denied: {0}")]
    AccessDenied(String),

    #[error("Database error: {0}")]
    Database(#[source] BoxedSource),

    #[error("Internal error: {0}")]
    Internal(#[source] BoxedSource),
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

#[derive(Debug, Clone, Copy, Default)]
pub struct ContextStats {
    pub task_count: i64,
    pub message_count: i64,
    pub last_message_at: Option<DateTime<Utc>>,
}

impl ContextWithStats {
    #[must_use]
    pub fn new(
        context_id: ContextId,
        user_id: UserId,
        name: impl Into<String>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            context_id,
            user_id,
            name: name.into(),
            created_at,
            updated_at,
            task_count: 0,
            message_count: 0,
            last_message_at: None,
        }
    }

    #[must_use]
    pub const fn with_stats(mut self, stats: ContextStats) -> Self {
        self.task_count = stats.task_count;
        self.message_count = stats.message_count;
        self.last_message_at = stats.last_message_at;
        self
    }
}

pub trait ContextProvider: Send + Sync {
    fn list_contexts_with_stats(
        &self,
        user_id: &UserId,
    ) -> impl Future<Output = Result<Vec<ContextWithStats>, ContextProviderError>> + Send;

    fn get_context(
        &self,
        context_id: &ContextId,
        user_id: &UserId,
    ) -> impl Future<Output = Result<ContextWithStats, ContextProviderError>> + Send;

    fn create_context(
        &self,
        user_id: &UserId,
        session_id: Option<&SessionId>,
        name: &str,
    ) -> impl Future<Output = Result<ContextId, ContextProviderError>> + Send;

    fn update_context_name(
        &self,
        context_id: &ContextId,
        user_id: &UserId,
        name: &str,
    ) -> impl Future<Output = Result<(), ContextProviderError>> + Send;

    fn delete_context(
        &self,
        context_id: &ContextId,
        user_id: &UserId,
    ) -> impl Future<Output = Result<(), ContextProviderError>> + Send;
}

#[derive(Debug, Clone, Copy)]
pub struct EnsureContextParams<'a> {
    pub context_id: &'a ContextId,
    pub user_id: &'a UserId,
    pub session_id: Option<&'a SessionId>,
    pub name: &'a str,
    pub kind: &'a str,
}

/// Idempotent materialization of derived contexts.
///
/// Boundaries that mint a `ContextId` deterministically (session, an
/// extension's evaluation run, probe) call this so the id resolves to a real
/// `user_contexts` row and joins/audit queries see it. Existing rows are never
/// modified.
///
/// Injected as `dyn ContextMaterializer`, hence `#[async_trait]`.
#[async_trait]
pub trait ContextMaterializer: Send + Sync {
    async fn ensure_context(
        &self,
        params: EnsureContextParams<'_>,
    ) -> Result<(), ContextProviderError>;
}

pub type DynContextMaterializer = Arc<dyn ContextMaterializer>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedContextSummary {
    pub context_id: ContextId,
    pub name: String,
}

/// Lookups of the conversation contexts one user owns.
///
/// For surfaces outside the agent domain that attribute work to a
/// conversation (the document store): a context another user owns reads as
/// absent, so a caller can never learn that it exists.
///
/// Injected as `dyn ContextDirectory`, hence `#[async_trait]`.
#[async_trait]
pub trait ContextDirectory: Send + Sync {
    async fn owned_context(
        &self,
        user_id: &UserId,
        context_id: &ContextId,
    ) -> Result<Option<OwnedContextSummary>, ContextProviderError>;

    async fn owned_names(
        &self,
        user_id: &UserId,
        context_ids: &[ContextId],
    ) -> Result<HashMap<ContextId, String>, ContextProviderError>;
}

pub type DynContextDirectory = Arc<dyn ContextDirectory>;

/// The conversation context a CLI session runs in.
///
/// For the CLI kernel, which issues a session before any command runs and
/// cannot name the agent domain that owns the contexts table: the one context
/// of a user's session with the given name, created on first use and returned
/// unchanged after. Ownership checks go through [`ContextDirectory`].
///
/// Injected as `dyn SessionContextIssuer`, hence `#[async_trait]`.
#[async_trait]
pub trait SessionContextIssuer: Send + Sync {
    async fn session_context(
        &self,
        user_id: &UserId,
        session_id: &SessionId,
        name: &str,
    ) -> Result<ContextId, ContextProviderError>;
}

pub type DynSessionContextIssuer = Arc<dyn SessionContextIssuer>;
