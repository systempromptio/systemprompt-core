//! AI session lifecycle and usage tracking trait.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use systemprompt_identifiers::{SessionId, SessionSource, UserId};

use super::AiProviderResult;

/// Input bundle accepted by [`AiSessionProvider::create_session`].
#[derive(Debug, Clone)]
pub struct CreateAiSessionParams<'a> {
    /// Session identifier to persist.
    pub session_id: &'a SessionId,
    /// Owning user, if any.
    pub user_id: Option<&'a UserId>,
    /// Tag describing how the session began.
    pub session_source: SessionSource,
    /// Expiry timestamp.
    pub expires_at: DateTime<Utc>,
}

/// AI session lifecycle and usage tracking.
///
/// `#[async_trait]` is required because the trait is consumed as
/// `Arc<dyn AiSessionProvider>` via [`DynAiSessionProvider`].
#[async_trait]
pub trait AiSessionProvider: Send + Sync {
    /// Report whether `session_id` has a persisted record.
    async fn session_exists(&self, session_id: &SessionId) -> AiProviderResult<bool>;

    /// Persist a new AI session.
    async fn create_session(&self, params: CreateAiSessionParams<'_>) -> AiProviderResult<()>;

    /// Increment the usage counters for `session_id`.
    async fn increment_ai_usage(
        &self,
        session_id: &SessionId,
        tokens: i32,
        cost_microdollars: i64,
    ) -> AiProviderResult<()>;
}

/// Shared `Arc` alias for [`AiSessionProvider`].
pub type DynAiSessionProvider = Arc<dyn AiSessionProvider>;
