mod creation;
mod lookup;

use anyhow::Result;
use http::{HeaderMap, Uri};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use systemprompt_identifiers::{ClientId, SessionId, SessionSource, UserId};
use systemprompt_traits::{
    AnalyticsProvider, CreateSessionInput, FingerprintProvider, SessionAnalytics, UserEvent,
    UserEventPublisher, UserProvider,
};

const MAX_SESSION_AGE_SECONDS: i64 = 7 * 24 * 60 * 60;

#[derive(Debug, thiserror::Error)]
pub enum SessionCreationError {
    #[error("User not found: {user_id}")]
    UserNotFound { user_id: String },

    #[error("Session creation failed: {0}")]
    Internal(String),
}

struct SessionCreationParams<'a> {
    analytics: SessionAnalytics,
    is_bot: bool,
    fingerprint: String,
    client_id: &'a ClientId,
    jwt_secret: &'a str,
    session_source: SessionSource,
}

#[derive(Debug, Clone)]
pub struct AnonymousSessionInfo {
    pub session_id: SessionId,
    pub user_id: UserId,
    pub is_new: bool,
    pub jwt_token: String,
    pub fingerprint_hash: String,
}

#[derive(Debug, Clone)]
pub struct AuthenticatedSessionInfo {
    pub session_id: SessionId,
}

#[derive(Debug)]
pub struct CreateAnonymousSessionInput<'a> {
    pub headers: &'a HeaderMap,
    pub uri: Option<&'a Uri>,
    pub client_id: &'a ClientId,
    pub jwt_secret: &'a str,
    pub session_source: SessionSource,
}

#[derive(Clone)]
pub struct SessionCreationService {
    analytics_provider: Arc<dyn AnalyticsProvider>,
    user_provider: Arc<dyn UserProvider>,
    fingerprint_locks: Arc<RwLock<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
    event_publisher: Option<Arc<dyn UserEventPublisher>>,
    fingerprint_provider: Option<Arc<dyn FingerprintProvider>>,
}

impl std::fmt::Debug for SessionCreationService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionCreationService")
            .field("analytics_provider", &"<provider>")
            .field(
                "event_publisher",
                &self.event_publisher.as_ref().map(|_| "<publisher>"),
            )
            .finish_non_exhaustive()
    }
}

impl SessionCreationService {
    pub fn new(
        analytics_provider: Arc<dyn AnalyticsProvider>,
        user_provider: Arc<dyn UserProvider>,
    ) -> Self {
        Self {
            analytics_provider,
            user_provider,
            fingerprint_locks: Arc::new(RwLock::new(HashMap::new())),
            event_publisher: None,
            fingerprint_provider: None,
        }
    }

    pub fn with_event_publisher(mut self, publisher: Arc<dyn UserEventPublisher>) -> Self {
        self.event_publisher = Some(publisher);
        self
    }

    pub fn with_fingerprint_provider(mut self, provider: Arc<dyn FingerprintProvider>) -> Self {
        self.fingerprint_provider = Some(provider);
        self
    }

    fn publish_event(&self, event: UserEvent) {
        if let Some(ref publisher) = self.event_publisher {
            publisher.publish_user_event(event);
        }
    }

    pub async fn create_anonymous_session(
        &self,
        input: CreateAnonymousSessionInput<'_>,
    ) -> Result<AnonymousSessionInfo> {
        let analytics = self
            .analytics_provider
            .extract_analytics(input.headers, input.uri);
        let is_bot = analytics.is_bot();
        let fingerprint = analytics.compute_fingerprint();

        let params = SessionCreationParams {
            analytics,
            is_bot,
            fingerprint,
            client_id: input.client_id,
            jwt_secret: input.jwt_secret,
            session_source: input.session_source,
        };
        self.create_session_internal(params).await
    }

    pub async fn create_authenticated_session(
        &self,
        user_id: &UserId,
        headers: &HeaderMap,
        session_source: SessionSource,
    ) -> Result<SessionId, SessionCreationError> {
        let user = self
            .user_provider
            .find_by_id(user_id.as_str())
            .await
            .map_err(|e| SessionCreationError::Internal(e.to_string()))?;

        if user.is_none() {
            return Err(SessionCreationError::UserNotFound {
                user_id: user_id.to_string(),
            });
        }

        let session_id = SessionId::new(format!("sess_{}", Uuid::new_v4()));
        let analytics = self.analytics_provider.extract_analytics(headers, None);
        let is_bot = analytics.is_bot();

        let global_config = systemprompt_models::Config::get()
            .map_err(|e| SessionCreationError::Internal(e.to_string()))?;
        let expires_at = chrono::Utc::now()
            + chrono::Duration::seconds(global_config.jwt_access_token_expiration);

        self.analytics_provider
            .create_session(CreateSessionInput {
                session_id: &session_id,
                user_id: Some(user_id),
                analytics: &analytics,
                session_source,
                is_bot,
                expires_at,
            })
            .await
            .map_err(|e| SessionCreationError::Internal(e.to_string()))?;

        self.publish_event(UserEvent::SessionCreated {
            user_id: user_id.to_string(),
            session_id: session_id.to_string(),
        });

        Ok(session_id)
    }

    async fn create_session_internal(
        &self,
        params: SessionCreationParams<'_>,
    ) -> Result<AnonymousSessionInfo> {
        let _guard = self.acquire_fingerprint_lock(&params.fingerprint).await;

        self.update_fingerprint_if_available(&params.fingerprint, &params.analytics)
            .await;

        if let Some(session) = self
            .try_reuse_session_at_limit(&params.fingerprint, params.client_id, params.jwt_secret)
            .await
        {
            return Ok(session);
        }

        if let Some(session) = self
            .try_find_existing_session(&params.fingerprint, params.client_id, params.jwt_secret)
            .await
        {
            return Ok(session);
        }

        self.create_new_session(params).await
    }

    async fn acquire_fingerprint_lock(
        &self,
        fingerprint: &str,
    ) -> tokio::sync::OwnedMutexGuard<()> {
        let lock = {
            let mut locks = self.fingerprint_locks.write().await;
            Arc::clone(
                locks
                    .entry(fingerprint.to_string())
                    .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(()))),
            )
        };
        lock.lock_owned().await
    }

    async fn update_fingerprint_if_available(
        &self,
        fingerprint: &str,
        analytics: &SessionAnalytics,
    ) {
        let Some(ref fp_provider) = self.fingerprint_provider else {
            return;
        };

        if let Err(e) = fp_provider
            .upsert_fingerprint(
                fingerprint,
                analytics.ip_address.as_deref(),
                analytics.user_agent.as_deref(),
                None,
            )
            .await
        {
            tracing::warn!(error = %e, fingerprint = %fingerprint, "Failed to upsert fingerprint");
        }
    }
}
