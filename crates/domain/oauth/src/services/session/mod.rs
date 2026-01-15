use anyhow::Result;
use axum::http::{HeaderMap, Uri};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use systemprompt_core_analytics::{
    AnalyticsService, CreateAnalyticsSessionInput, FingerprintRepository, SessionAnalytics,
    MAX_SESSIONS_PER_FINGERPRINT,
};
use systemprompt_identifiers::{ClientId, SessionId, SessionSource, UserId};
use systemprompt_traits::{UserEvent, UserEventPublisher, UserProvider};

use crate::services::generation::{generate_anonymous_jwt, JwtSigningParams};

const MAX_SESSION_AGE_SECONDS: i64 = 7 * 24 * 60 * 60;
const SESSION_LOOKUP_TIMEOUT_MS: u64 = 500;

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
    analytics_service: Arc<AnalyticsService>,
    user_provider: Arc<dyn UserProvider>,
    fingerprint_locks: Arc<RwLock<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
    event_publisher: Option<Arc<dyn UserEventPublisher>>,
    fingerprint_repo: Option<Arc<FingerprintRepository>>,
}

impl std::fmt::Debug for SessionCreationService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionCreationService")
            .field("analytics_service", &self.analytics_service)
            .field(
                "event_publisher",
                &self.event_publisher.as_ref().map(|_| "<publisher>"),
            )
            .finish()
    }
}

impl SessionCreationService {
    pub fn new(
        analytics_service: Arc<AnalyticsService>,
        user_provider: Arc<dyn UserProvider>,
    ) -> Self {
        Self {
            analytics_service,
            user_provider,
            fingerprint_locks: Arc::new(RwLock::new(HashMap::new())),
            event_publisher: None,
            fingerprint_repo: None,
        }
    }

    pub fn with_event_publisher(mut self, publisher: Arc<dyn UserEventPublisher>) -> Self {
        self.event_publisher = Some(publisher);
        self
    }

    pub fn with_fingerprint_repo(mut self, repo: Arc<FingerprintRepository>) -> Self {
        self.fingerprint_repo = Some(repo);
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
            .analytics_service
            .extract_analytics(input.headers, input.uri);
        let is_bot = AnalyticsService::is_bot(&analytics);
        let fingerprint = AnalyticsService::compute_fingerprint(&analytics);

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
    ) -> Result<SessionId> {
        let session_id = SessionId::new(format!("sess_{}", Uuid::new_v4()));
        let analytics = self.analytics_service.extract_analytics(headers, None);
        let is_bot = AnalyticsService::is_bot(&analytics);

        let global_config = systemprompt_models::Config::get()?;
        let expires_at = chrono::Utc::now()
            + chrono::Duration::seconds(global_config.jwt_access_token_expiration);

        self.analytics_service
            .create_analytics_session(CreateAnalyticsSessionInput {
                session_id: &session_id,
                user_id: Some(user_id),
                analytics: &analytics,
                session_source,
                is_bot,
                expires_at,
            })
            .await?;

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
        let Some(ref fp_repo) = self.fingerprint_repo else {
            return;
        };

        let _ = fp_repo
            .upsert_fingerprint(
                fingerprint,
                analytics.ip_address.as_deref(),
                analytics.user_agent.as_deref(),
                None,
            )
            .await;
    }

    async fn try_reuse_session_at_limit(
        &self,
        fingerprint: &str,
        client_id: &ClientId,
        jwt_secret: &str,
    ) -> Option<AnonymousSessionInfo> {
        let fp_repo = self.fingerprint_repo.as_ref()?;

        let active_count = fp_repo.count_active_sessions(fingerprint).await.ok()?;
        if active_count < MAX_SESSIONS_PER_FINGERPRINT {
            return None;
        }

        let session_id_str = fp_repo
            .find_reusable_session(fingerprint)
            .await
            .ok()
            .flatten()?;

        let existing_session = self
            .analytics_service
            .find_recent_session_by_fingerprint(fingerprint, MAX_SESSION_AGE_SECONDS)
            .await
            .ok()
            .flatten()?;

        let user_id_str = existing_session.user_id.as_ref()?;
        let user_id = UserId::new(user_id_str.clone());
        let session_id = SessionId::new(session_id_str);

        let config = systemprompt_models::Config::get().ok()?;
        let signing = JwtSigningParams {
            secret: jwt_secret,
            issuer: &config.jwt_issuer,
        };
        let token =
            generate_anonymous_jwt(user_id_str, session_id.as_str(), client_id, &signing).ok()?;

        tracing::debug!(
            fingerprint = %fingerprint,
            session_id = %session_id,
            active_sessions = active_count,
            "Reusing session due to fingerprint session limit"
        );

        Some(AnonymousSessionInfo {
            session_id,
            user_id,
            is_new: false,
            jwt_token: token,
        })
    }

    async fn try_find_existing_session(
        &self,
        fingerprint: &str,
        client_id: &ClientId,
        jwt_secret: &str,
    ) -> Option<AnonymousSessionInfo> {
        let lookup_result = tokio::time::timeout(
            tokio::time::Duration::from_millis(SESSION_LOOKUP_TIMEOUT_MS),
            self.analytics_service
                .find_recent_session_by_fingerprint(fingerprint, MAX_SESSION_AGE_SECONDS),
        )
        .await;

        let existing_session = lookup_result.ok()?.ok()??;
        let user_id_str = existing_session.user_id.as_ref()?;

        let user_id = UserId::new(user_id_str.clone());
        let session_id = SessionId::new(existing_session.session_id.clone());

        let config = systemprompt_models::Config::get().ok()?;
        let signing = JwtSigningParams {
            secret: jwt_secret,
            issuer: &config.jwt_issuer,
        };
        let token = generate_anonymous_jwt(
            user_id_str,
            &existing_session.session_id,
            client_id,
            &signing,
        )
        .ok()?;

        Some(AnonymousSessionInfo {
            session_id,
            user_id,
            is_new: false,
            jwt_token: token,
        })
    }

    async fn create_new_session(
        &self,
        params: SessionCreationParams<'_>,
    ) -> Result<AnonymousSessionInfo> {
        let session_id = SessionId::new(format!("sess_{}", Uuid::new_v4()));

        let anonymous_user = self
            .user_provider
            .create_anonymous(&params.fingerprint)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        let user_id = UserId::new(anonymous_user.id);

        let jwt_expiration_seconds =
            systemprompt_models::Config::get()?.jwt_access_token_expiration;
        let expires_at = chrono::Utc::now() + chrono::Duration::seconds(jwt_expiration_seconds);

        self.analytics_service
            .create_analytics_session(CreateAnalyticsSessionInput {
                session_id: &session_id,
                user_id: Some(&user_id),
                analytics: &params.analytics,
                session_source: params.session_source,
                is_bot: params.is_bot,
                expires_at,
            })
            .await?;

        let config = systemprompt_models::Config::get()?;
        let signing = JwtSigningParams {
            secret: params.jwt_secret,
            issuer: &config.jwt_issuer,
        };
        let token = generate_anonymous_jwt(
            user_id.as_str(),
            session_id.as_str(),
            params.client_id,
            &signing,
        )?;

        self.publish_event(UserEvent::UserCreated {
            user_id: user_id.to_string(),
        });
        self.publish_event(UserEvent::SessionCreated {
            user_id: user_id.to_string(),
            session_id: session_id.to_string(),
        });

        Ok(AnonymousSessionInfo {
            session_id,
            user_id,
            is_new: true,
            jwt_token: token,
        })
    }
}
