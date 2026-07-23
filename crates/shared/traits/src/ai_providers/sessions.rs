//! AI session lifecycle and usage tracking trait.
//!
//! Dispatched as a trait object (`dyn _`), so it uses `#[async_trait]`;
//! native `async fn` in traits is not yet `dyn`-compatible.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
    /// Must be an idempotent upsert: callers invoke it unconditionally to
    /// guarantee the session row exists before FK-dependent inserts.
    async fn create_session(&self, params: CreateAiSessionParams<'_>) -> AiProviderResult<()>;

    async fn increment_ai_usage(
        &self,
        session_id: &SessionId,
        tokens: i32,
        cost_microdollars: i64,
    ) -> AiProviderResult<()>;
}

pub type DynAiSessionProvider = Arc<dyn AiSessionProvider>;
