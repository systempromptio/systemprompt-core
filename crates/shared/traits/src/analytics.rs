//! Session analytics and fingerprinting provider traits.
//!
//! These traits are dispatched as trait objects (`dyn _`), so they use
//! `#[async_trait]`; native `async fn` in traits is not yet `dyn`-compatible.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use http::{HeaderMap, Uri};
use std::net::IpAddr;
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

/// A single HTTP request reduced to the signals the session pipeline records.
///
/// Produced once per request by an [`AnalyticsProvider`] and passed by
/// reference from there on — the classification verdicts (`is_bot`,
/// `is_ai_crawler`, `skip_tracking`) are decided by the provider, which owns
/// the keyword tables, so no consumer re-derives them.
#[derive(Debug, Clone, Default)]
pub struct SessionAnalytics {
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub device_type: Option<String>,
    pub browser: Option<String>,
    pub os: Option<String>,
    pub fingerprint_hash: Option<String>,
    pub preferred_locale: Option<String>,
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub referrer_source: Option<String>,
    pub referrer_url: Option<String>,
    pub landing_page: Option<String>,
    pub entry_url: Option<String>,
    pub utm_source: Option<String>,
    pub utm_medium: Option<String>,
    pub utm_campaign: Option<String>,
    pub utm_content: Option<String>,
    pub utm_term: Option<String>,
    pub is_bot: bool,
    pub is_ai_crawler: bool,
    /// Whether the session pipeline should suppress the analytics write.
    /// Broader than `is_bot`: also covers bot IP ranges, datacenter ranges,
    /// high-risk countries, and spam referrers.
    pub skip_tracking: bool,
}

impl SessionAnalytics {
    /// Returns the client-supplied fingerprint when present, else derives a
    /// stable one from the user agent and locale.
    pub fn compute_fingerprint(&self) -> String {
        use xxhash_rust::xxh64::xxh64;

        if let Some(hash) = &self.fingerprint_hash {
            return hash.clone();
        }

        let data = format!(
            "{}|{}",
            self.user_agent.as_deref().unwrap_or(""),
            self.preferred_locale.as_deref().unwrap_or("")
        );

        format!("fp_{:016x}", xxh64(data.as_bytes(), 0))
    }
}

#[derive(Debug, Clone)]
pub struct AnalyticsSession {
    pub session_id: SessionId,
    pub user_id: Option<UserId>,
    pub fingerprint: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ActiveSession {
    pub user_id: Option<UserId>,
}

#[derive(Debug)]
pub struct CreateSessionInput<'a> {
    pub session_id: &'a SessionId,
    pub user_id: Option<&'a UserId>,
    pub analytics: &'a SessionAnalytics,
    pub session_source: SessionSource,
    pub is_bot: bool,
    pub is_ai_crawler: bool,
    pub expires_at: DateTime<Utc>,
}

/// Optional request signals for analytics extraction that vary per call site.
/// `GeoIP` and content-routing are supplied by the provider itself, so only the
/// request-scoped inputs live here.
#[derive(Debug, Default, Clone, Copy)]
pub struct ExtractSignals<'a> {
    pub uri: Option<&'a Uri>,
    pub caller_ip: Option<IpAddr>,
}

#[async_trait]
pub trait AnalyticsProvider: Send + Sync {
    fn extract_analytics(
        &self,
        headers: &HeaderMap,
        signals: ExtractSignals<'_>,
    ) -> SessionAnalytics;

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

    async fn find_active_session_by_id(
        &self,
        session_id: &SessionId,
    ) -> AnalyticsResult<Option<ActiveSession>>;

    async fn revoke_session(&self, session_id: &SessionId) -> AnalyticsResult<()>;

    async fn revoke_all_sessions_for_user(&self, user_id: &UserId) -> AnalyticsResult<u64>;

    async fn migrate_user_sessions(
        &self,
        from_user_id: &UserId,
        to_user_id: &UserId,
    ) -> AnalyticsResult<u64>;

    async fn mark_session_converted(&self, session_id: &SessionId) -> AnalyticsResult<()>;
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
