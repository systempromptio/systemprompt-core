//! AI session lifecycle and usage tracking trait.
//!
//! Dispatched as a trait object (`dyn _`), so it uses `#[async_trait]`;
//! native `async fn` in traits is not yet `dyn`-compatible.
//!
//! `find_live_session` reports a session only while it is neither revoked
//! nor expired, so a caller binding work to a session id learns its owner
//! from the same read that proves the session is still usable.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use systemprompt_identifiers::{SessionId, SessionSource, UserId};

use super::AiProviderResult;
use crate::analytics::ActiveSession;

#[derive(Debug, Clone)]
pub struct CreateAiSessionParams<'a> {
    pub session_id: &'a SessionId,
    pub user_id: Option<&'a UserId>,
    pub session_source: SessionSource,
    pub expires_at: DateTime<Utc>,
}

#[async_trait]
pub trait AiSessionProvider: Send + Sync {
    async fn create_session(&self, params: CreateAiSessionParams<'_>) -> AiProviderResult<()>;

    async fn increment_ai_usage(
        &self,
        session_id: &SessionId,
        tokens: i32,
        cost_microdollars: i64,
    ) -> AiProviderResult<()>;

    async fn find_live_session(
        &self,
        session_id: &SessionId,
    ) -> AiProviderResult<Option<ActiveSession>>;
}

pub type DynAiSessionProvider = Arc<dyn AiSessionProvider>;
