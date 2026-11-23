//! The gateway session mint over the users member's repositories.
//!
//! Every call verifies the presented API key first and then mints, or returns
//! the session bound to, that key's owner only. A key that does not verify,
//! or whose owner no longer exists, mints nothing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::{SessionId, SessionSource};
use systemprompt_traits::{
    CreateSessionInput, GatewaySessionMint, GatewaySessionMintError, MintedGatewaySession,
    SessionAnalytics, SessionProvider,
};
use uuid::Uuid;

use crate::UserError;
use crate::models::UserApiKey;
use crate::repository::{SessionRepository, UserRepository};
use crate::services::{ApiKeyLifetime, ApiKeyService};

/// The users member's [`GatewaySessionMint`].
#[derive(Clone)]
pub struct UsersGatewaySessionMint {
    users: Arc<UserRepository>,
    keys: ApiKeyService,
    sessions: SessionRepository,
    session_ttl: chrono::Duration,
}

impl std::fmt::Debug for UsersGatewaySessionMint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UsersGatewaySessionMint")
            .finish_non_exhaustive()
    }
}

fn store_error(context: &'static str) -> impl FnOnce(UserError) -> GatewaySessionMintError {
    move |source| GatewaySessionMintError::Store {
        context,
        source: source.into(),
    }
}

impl UsersGatewaySessionMint {
    #[must_use]
    pub fn new(
        users: Arc<UserRepository>,
        sessions: SessionRepository,
        lifetime: ApiKeyLifetime,
        session_ttl: chrono::Duration,
    ) -> Self {
        Self {
            keys: ApiKeyService::new(Arc::clone(&users), lifetime),
            users,
            sessions,
            session_ttl,
        }
    }

    async fn verified(
        &self,
        presented: &str,
    ) -> Result<Option<UserApiKey>, GatewaySessionMintError> {
        let Some(record) = self
            .keys
            .verify(presented)
            .await
            .map_err(store_error("api key verification"))?
        else {
            return Ok(None);
        };
        let owner = self
            .users
            .find_by_id(&record.user_id)
            .await
            .map_err(store_error("api key owner lookup"))?;
        Ok(owner.map(|_| record))
    }

    async fn mint(
        &self,
        record: &UserApiKey,
        analytics: &SessionAnalytics,
    ) -> Result<MintedGatewaySession, GatewaySessionMintError> {
        let session_id = SessionId::new(format!("sess_{}", Uuid::new_v4()));
        SessionProvider::create_session(
            &self.sessions,
            CreateSessionInput {
                session_id: &session_id,
                user_id: Some(&record.user_id),
                analytics,
                session_source: SessionSource::Api,
                is_bot: analytics.is_bot,
                is_ai_crawler: analytics.is_ai_crawler,
                expires_at: chrono::Utc::now() + self.session_ttl,
            },
        )
        .await
        .map_err(|source| GatewaySessionMintError::Store {
            context: "session insert",
            source: source.into(),
        })?;
        Ok(MintedGatewaySession {
            api_key_id: record.id.clone(),
            user_id: record.user_id.clone(),
            session_id,
            presented_session_kept: false,
        })
    }

    async fn usable(
        &self,
        record: &UserApiKey,
        session_id: &SessionId,
    ) -> Result<bool, GatewaySessionMintError> {
        let session = SessionProvider::find_active_session_by_id(&self.sessions, session_id)
            .await
            .map_err(|source| GatewaySessionMintError::Store {
                context: "bound session lookup",
                source: source.into(),
            })?;
        Ok(session.is_some_and(|session| {
            session
                .user_id
                .is_some_and(|owner| owner.as_str() == record.user_id.as_str())
        }))
    }
}

#[async_trait]
impl GatewaySessionMint for UsersGatewaySessionMint {
    async fn mint_for_api_key(
        &self,
        presented: &str,
        analytics: &SessionAnalytics,
    ) -> Result<Option<MintedGatewaySession>, GatewaySessionMintError> {
        let Some(record) = self.verified(presented).await? else {
            return Ok(None);
        };
        self.mint(&record, analytics).await.map(Some)
    }

    async fn bind_for_api_key(
        &self,
        presented: &str,
        presented_session: Option<&SessionId>,
        analytics: &SessionAnalytics,
    ) -> Result<Option<MintedGatewaySession>, GatewaySessionMintError> {
        let Some(record) = self.verified(presented).await? else {
            return Ok(None);
        };
        if let Some(session_id) = presented_session
            && self.usable(&record, session_id).await?
        {
            return Ok(Some(MintedGatewaySession {
                api_key_id: record.id,
                user_id: record.user_id,
                session_id: session_id.clone(),
                presented_session_kept: true,
            }));
        }
        let existing = self
            .users
            .find_api_key_session(&record.id)
            .await
            .map_err(store_error("api-key session lookup"))?;
        if let Some(session_id) = existing
            && self.usable(&record, &session_id).await?
        {
            return Ok(Some(MintedGatewaySession {
                api_key_id: record.id,
                user_id: record.user_id,
                session_id,
                presented_session_kept: false,
            }));
        }
        let minted = self.mint(&record, analytics).await?;
        self.users
            .set_api_key_session(&record.id, &minted.session_id)
            .await
            .map_err(store_error("api-key session bind"))?;
        Ok(Some(minted))
    }
}
