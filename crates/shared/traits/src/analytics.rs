use async_trait::async_trait;
use chrono::{DateTime, Utc};
use http::{HeaderMap, Uri};
use std::sync::Arc;
use systemprompt_identifiers::{SessionId, SessionSource, UserId};

pub type AnalyticsResult<T> = Result<T, AnalyticsProviderError>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AnalyticsProviderError {
    #[error("Session not found")]
    SessionNotFound,

    #[error("Fingerprint not found")]
    FingerprintNotFound,

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for AnalyticsProviderError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

#[derive(Debug, Clone, Default)]
pub struct SessionAnalytics {
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub device_type: Option<String>,
    pub browser: Option<String>,
    pub os: Option<String>,
    pub fingerprint_hash: Option<String>,
    pub referer: Option<String>,
    pub referrer_url: Option<String>,
    pub referrer_source: Option<String>,
    pub accept_language: Option<String>,
    pub preferred_locale: Option<String>,
    pub screen_width: Option<i32>,
    pub screen_height: Option<i32>,
    pub timezone: Option<String>,
    pub page_url: Option<String>,
    pub landing_page: Option<String>,
    pub entry_url: Option<String>,
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub utm_source: Option<String>,
    pub utm_medium: Option<String>,
    pub utm_campaign: Option<String>,
}

impl SessionAnalytics {
    pub fn is_bot(&self) -> bool {
        self.user_agent.as_ref().is_some_and(|ua| {
            let ua_lower = ua.to_lowercase();
            ua_lower.contains("bot")
                || ua_lower.contains("crawler")
                || ua_lower.contains("spider")
                || ua_lower.contains("headless")
        })
    }

    pub fn compute_fingerprint(&self) -> String {
        use xxhash_rust::xxh64::xxh64;

        if let Some(hash) = &self.fingerprint_hash {
            return hash.clone();
        }

        let data = format!(
            "{}|{}",
            self.user_agent.as_deref().unwrap_or(""),
            self.accept_language
                .as_deref()
                .or(self.preferred_locale.as_deref())
                .unwrap_or("")
        );

        format!("fp_{:016x}", xxh64(data.as_bytes(), 0))
    }
}

#[derive(Debug, Clone)]
pub struct AnalyticsSession {
    pub session_id: String,
    pub user_id: Option<String>,
    pub fingerprint: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct CreateSessionInput<'a> {
    pub session_id: &'a SessionId,
    pub user_id: Option<&'a UserId>,
    pub analytics: &'a SessionAnalytics,
    pub session_source: SessionSource,
    pub is_bot: bool,
    pub expires_at: DateTime<Utc>,
}

#[async_trait]
pub trait AnalyticsProvider: Send + Sync {
    fn extract_analytics(&self, headers: &HeaderMap, uri: Option<&Uri>) -> SessionAnalytics;

    async fn create_session(&self, input: CreateSessionInput<'_>) -> AnalyticsResult<()>;

    async fn find_recent_session_by_fingerprint(
        &self,
        fingerprint: &str,
        max_age_seconds: i64,
    ) -> AnalyticsResult<Option<AnalyticsSession>>;

    async fn find_session_by_id(
        &self,
        session_id: &SessionId,
    ) -> AnalyticsResult<Option<AnalyticsSession>>;

    async fn migrate_user_sessions(
        &self,
        from_user_id: &UserId,
        to_user_id: &UserId,
    ) -> AnalyticsResult<u64>;
}

#[async_trait]
pub trait FingerprintProvider: Send + Sync {
    async fn count_active_sessions(&self, fingerprint: &str) -> AnalyticsResult<i64>;

    async fn find_reusable_session(&self, fingerprint: &str) -> AnalyticsResult<Option<String>>;

    async fn upsert_fingerprint(
        &self,
        fingerprint: &str,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
        screen_info: Option<&str>,
    ) -> AnalyticsResult<()>;
}

pub type DynAnalyticsProvider = Arc<dyn AnalyticsProvider>;
pub type DynFingerprintProvider = Arc<dyn FingerprintProvider>;
