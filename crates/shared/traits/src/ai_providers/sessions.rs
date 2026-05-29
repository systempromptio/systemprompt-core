//! AI session lifecycle and usage tracking trait.
//!
//! Dispatched as a trait object (`dyn _`), so it uses `#[async_trait]`;
//! native `async fn` in traits is not yet `dyn`-compatible.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use systemprompt_identifiers::{SessionId, SessionSource, UserId};

use super::AiProviderResult;

#[derive(Debug, Clone)]
pub struct CreateAiSessionParams<'a> {
    pub session_id: &'a SessionId,
    pub user_id: Option<&'a UserId>,
    pub session_source: SessionSource,
    pub expires_at: DateTime<Utc>,
}

#[async_trait]
pub trait AiSessionProvider: Send + Sync {
    async fn session_exists(&self, session_id: &SessionId) -> AiProviderResult<bool>;

    async fn create_session(&self, params: CreateAiSessionParams<'_>) -> AiProviderResult<()>;

    async fn increment_ai_usage(
        &self,
        session_id: &SessionId,
        tokens: i32,
        cost_microdollars: i64,
    ) -> AiProviderResult<()>;
}

pub type DynAiSessionProvider = Arc<dyn AiSessionProvider>;
