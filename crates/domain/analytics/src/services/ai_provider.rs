use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_identifiers::SessionId;
use systemprompt_traits::{
    AiProviderError, AiProviderResult, AiSessionProvider, CreateAiSessionParams,
};

use crate::repository::{CreateSessionParams, SessionRepository};

#[derive(Debug)]
pub struct AnalyticsAiSessionProvider {
    session_repo: SessionRepository,
}

impl AnalyticsAiSessionProvider {
    pub const fn new(pool: DbPool) -> Self {
        Self {
            session_repo: SessionRepository::new(pool),
        }
    }

    pub const fn from_repository(session_repo: SessionRepository) -> Self {
        Self { session_repo }
    }
}

#[async_trait]
impl AiSessionProvider for AnalyticsAiSessionProvider {
    async fn session_exists(&self, session_id: &SessionId) -> AiProviderResult<bool> {
        self.session_repo
            .exists(session_id)
            .await
            .map_err(|e| AiProviderError::Internal(e.to_string()))
    }

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
            is_bot: false,
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
        cost_cents: i32,
    ) -> AiProviderResult<()> {
        self.session_repo
            .increment_ai_usage(session_id, tokens, cost_cents)
            .await
            .map_err(|e| AiProviderError::Internal(e.to_string()))
    }
}
