//! AI session lifecycle and usage accounting owned by users.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_identifiers::SessionId;
use systemprompt_traits::{
    ActiveSession, AiProviderError, AiProviderResult, AiSessionProvider, CreateAiSessionParams,
};

use super::SessionRepository;
use systemprompt_traits::session_store::CreateSessionParams;

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
            session_id: params.session_id,
            user_id: params.user_id,
            session_source: params.session_source,
            fingerprint_hash: None,
            ip_address: None,
            user_agent: None,
            device_type: None,
            browser: None,
            os: None,
            country: None,
            region: None,
            city: None,
            preferred_locale: None,
            referrer_source: None,
            referrer_url: None,
            landing_page: None,
            entry_url: None,
            utm_source: None,
            utm_medium: None,
            utm_campaign: None,
            utm_content: None,
            utm_term: None,
            is_bot: false,
            is_ai_crawler: false,
            expires_at: params.expires_at,
        };

        self.session_repo
            .create_session(&full_params)
            .await
            .map_err(|e| AiProviderError::Internal(e.to_string()))
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
            .map_err(|e| AiProviderError::Internal(e.to_string()))
    }

    async fn find_live_session(
        &self,
        session_id: &SessionId,
    ) -> AiProviderResult<Option<ActiveSession>> {
        let session = self
            .session_repo
            .find_active_by_id(session_id)
            .await
            .map_err(|e| AiProviderError::Internal(e.to_string()))?;
        Ok(session.map(|row| ActiveSession {
            user_id: row.user_id,
        }))
    }
}
