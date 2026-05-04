//! Session-scoped analytics counter trait.

use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_identifiers::SessionId;

/// Result alias for [`SessionAnalyticsProvider`] operations.
pub type SessionAnalyticsResult<T> = Result<T, SessionAnalyticsProviderError>;

/// Errors returned by session analytics providers.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SessionAnalyticsProviderError {
    /// No record exists for the requested session.
    #[error("Session not found")]
    SessionNotFound,

    /// Catch-all for unexpected provider failures.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for SessionAnalyticsProviderError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

/// Increment session-scoped analytics counters.
///
/// `#[async_trait]` is required because the trait is consumed as
/// `Arc<dyn SessionAnalyticsProvider>` via [`DynSessionAnalyticsProvider`].
#[async_trait]
pub trait SessionAnalyticsProvider: Send + Sync {
    /// Bump the task counter for `session_id`.
    async fn increment_task_count(&self, session_id: &SessionId) -> SessionAnalyticsResult<()>;
    /// Bump the message counter for `session_id`.
    async fn increment_message_count(&self, session_id: &SessionId) -> SessionAnalyticsResult<()>;
}

/// Shared `Arc` alias for [`SessionAnalyticsProvider`].
pub type DynSessionAnalyticsProvider = Arc<dyn SessionAnalyticsProvider>;
