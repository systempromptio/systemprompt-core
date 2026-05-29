//! Session-scoped analytics counter trait.
//!
//! Dispatched as a trait object (`dyn _`), so it uses `#[async_trait]`;
//! native `async fn` in traits is not yet `dyn`-compatible.

use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_identifiers::SessionId;

pub type SessionAnalyticsResult<T> = Result<T, SessionAnalyticsProviderError>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SessionAnalyticsProviderError {
    #[error("Session not found")]
    SessionNotFound,

    #[error("Internal error: {0}")]
    Internal(String),
}

#[async_trait]
pub trait SessionAnalyticsProvider: Send + Sync {
    async fn increment_task_count(&self, session_id: &SessionId) -> SessionAnalyticsResult<()>;
    async fn increment_message_count(&self, session_id: &SessionId) -> SessionAnalyticsResult<()>;
}

pub type DynSessionAnalyticsProvider = Arc<dyn SessionAnalyticsProvider>;
