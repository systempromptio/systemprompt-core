//! AI session lifecycle and usage accounting owned by users.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_identifiers::SessionId;
use systemprompt_traits::{
    ActiveSession, AiProviderError, AiProviderResult, AiSessionProvider, CreateAiSessionParams,
};

use crate::repository::SessionRepository;
use systemprompt_traits::session_store::{
    CreateSessionParams, SessionAttribution, SessionIdentity, SessionNetwork,
};

#[derive(Debug)]
pub struct UsersAiSessionProvider {
    session_repo: SessionRepository,
}

impl UsersAiSessionProvider {
    pub const fn from_repository(session_repo: SessionRepository) -> Self {
        Self { session_repo }
    }
}

#[async_trait]
impl AiSessionProvider for UsersAiSessionProvider {
    async fn create_session(&self, params: CreateAiSessionParams<'_>) -> AiProviderResult<()> {
        let full_params = CreateSessionParams {
            identity: SessionIdentity {
                session_id: params.session_id,
                user_id: params.user_id,
                session_source: params.session_source,
                fingerprint_hash: None,
            },
            network: SessionNetwork::default(),
            attribution: SessionAttribution::default(),
            is_bot: false,
            is_ai_crawler: false,
            expires_at: params.expires_at,
        };

        self.session_repo
            .create_session(&full_params)
            .await
            .map_err(|e| AiProviderError::Internal(e.into()))
    }

    async fn increment_ai_usage(
        &self,
        session_id: &SessionId,
        tokens: i32,
        cost_microdollars: i64,
    ) -> AiProviderResult<()> {
        self.session_repo
            .increment_ai_usage(session_id, tokens, cost_microdollars)
            .await
            .map_err(|e| AiProviderError::Internal(e.into()))
    }

    async fn find_live_session(
        &self,
        session_id: &SessionId,
    ) -> AiProviderResult<Option<ActiveSession>> {
        let session = self
            .session_repo
            .find_active_by_id(session_id)
            .await
            .map_err(|e| AiProviderError::Internal(e.into()))?;
        Ok(session.map(|row| ActiveSession {
            user_id: row.user_id,
        }))
    }
}
