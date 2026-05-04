//! Session analytics and fingerprinting provider traits.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use http::{HeaderMap, Uri};
use std::sync::Arc;
use systemprompt_identifiers::{SessionId, SessionSource, UserId};

/// Result alias for [`AnalyticsProvider`] / [`FingerprintProvider`].
pub type AnalyticsResult<T> = Result<T, AnalyticsProviderError>;

/// Errors returned by analytics providers.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AnalyticsProviderError {
    /// No session matched the lookup key.
    #[error("Session not found")]
    SessionNotFound,

    /// No fingerprint matched the lookup key.
    #[error("Fingerprint not found")]
    FingerprintNotFound,

    /// Catch-all for unexpected provider failures.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for AnalyticsProviderError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

/// Per-session analytics payload extracted from request metadata.
#[derive(Debug, Clone, Default)]
pub struct SessionAnalytics {
    /// Best-effort client IP address.
    pub ip_address: Option<String>,
    /// Raw `User-Agent` header value.
    pub user_agent: Option<String>,
    /// Coarse device classification.
    pub device_type: Option<String>,
    /// Detected browser family.
    pub browser: Option<String>,
    /// Detected operating system family.
    pub os: Option<String>,
    /// Stable fingerprint hash of the client.
    pub fingerprint_hash: Option<String>,
    /// Raw `Referer` header value.
    pub referer: Option<String>,
    /// Cleaned referrer URL.
    pub referrer_url: Option<String>,
    /// High-level referrer source classification.
    pub referrer_source: Option<String>,
    /// Raw `Accept-Language` header value.
    pub accept_language: Option<String>,
    /// Locale the request prefers.
    pub preferred_locale: Option<String>,
    /// Reported screen width in pixels.
    pub screen_width: Option<i32>,
    /// Reported screen height in pixels.
    pub screen_height: Option<i32>,
    /// IANA timezone reported by the client.
    pub timezone: Option<String>,
    /// Current page URL.
    pub page_url: Option<String>,
    /// Landing page URL for the session.
    pub landing_page: Option<String>,
    /// Initial entry URL that started the session.
    pub entry_url: Option<String>,
    /// IP-derived country code.
    pub country: Option<String>,
    /// IP-derived region.
    pub region: Option<String>,
    /// IP-derived city.
    pub city: Option<String>,
    /// `utm_source` query parameter.
    pub utm_source: Option<String>,
    /// `utm_medium` query parameter.
    pub utm_medium: Option<String>,
    /// `utm_campaign` query parameter.
    pub utm_campaign: Option<String>,
    /// `utm_content` query parameter.
    pub utm_content: Option<String>,
    /// `utm_term` query parameter.
    pub utm_term: Option<String>,
}

const AI_CRAWLER_TOKENS: &[&str] = &[
    "notebooklm",
    "gemini-deep-research",
    "grammarly",
    "chatgpt-user",
    "oai-searchbot",
    "gptbot",
    "perplexitybot",
    "perplexity-user",
    "claudebot",
    "claude-user",
    "claude-web",
    "anthropic-ai",
    "applebot-extended",
    "ccbot",
    "bytespider",
    "amazonbot",
    "youbot",
    "diffbot",
    "cohere-ai",
];

impl SessionAnalytics {
    /// Report whether the recorded user agent matches a known AI crawler.
    pub fn is_ai_crawler(&self) -> bool {
        self.user_agent.as_ref().is_some_and(|ua| {
            let ua_lower = ua.to_lowercase();
            AI_CRAWLER_TOKENS
                .iter()
                .any(|token| ua_lower.contains(token))
        })
    }

    /// Report whether the recorded user agent looks like a non-AI bot.
    pub fn is_bot(&self) -> bool {
        if self.is_ai_crawler() {
            return false;
        }
        self.user_agent.as_ref().is_some_and(|ua| {
            let ua_lower = ua.to_lowercase();
            ua_lower.contains("bot")
                || ua_lower.contains("crawler")
                || ua_lower.contains("spider")
                || ua_lower.contains("headless")
        })
    }

    /// Compute (or return the cached) fingerprint string.
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

/// Persisted session record.
#[derive(Debug, Clone)]
pub struct AnalyticsSession {
    /// Stable session identifier.
    pub session_id: SessionId,
    /// Owning user, if known.
    pub user_id: Option<UserId>,
    /// Cached fingerprint.
    pub fingerprint: Option<String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// Input bundle accepted by [`AnalyticsProvider::create_session`].
#[derive(Debug)]
pub struct CreateSessionInput<'a> {
    /// Session identifier to persist.
    pub session_id: &'a SessionId,
    /// Optional owning user.
    pub user_id: Option<&'a UserId>,
    /// Analytics payload extracted from headers.
    pub analytics: &'a SessionAnalytics,
    /// Tag describing how the session began.
    pub session_source: SessionSource,
    /// Whether the originating client looked like a bot.
    pub is_bot: bool,
    /// Whether the originating client looked like an AI crawler.
    pub is_ai_crawler: bool,
    /// Expiry timestamp.
    pub expires_at: DateTime<Utc>,
}

/// Analytics ingestion contract.
///
/// `#[async_trait]` is required because the trait is consumed as
/// `Arc<dyn AnalyticsProvider>` via [`DynAnalyticsProvider`].
#[async_trait]
pub trait AnalyticsProvider: Send + Sync {
    /// Build a [`SessionAnalytics`] from incoming request metadata.
    fn extract_analytics(&self, headers: &HeaderMap, uri: Option<&Uri>) -> SessionAnalytics;

    /// Persist a new session record.
    async fn create_session(&self, input: CreateSessionInput<'_>) -> AnalyticsResult<()>;

    /// Find a recent session matching `fingerprint`, no older than
    /// `max_age_seconds`.
    async fn find_recent_session_by_fingerprint(
        &self,
        fingerprint: &str,
        max_age_seconds: i64,
    ) -> AnalyticsResult<Option<AnalyticsSession>>;

    /// Look up a session by id.
    async fn find_session_by_id(
        &self,
        session_id: &SessionId,
    ) -> AnalyticsResult<Option<AnalyticsSession>>;

    /// Re-key sessions from `from_user_id` to `to_user_id` (login merge).
    /// Returns the number of rows updated.
    async fn migrate_user_sessions(
        &self,
        from_user_id: &UserId,
        to_user_id: &UserId,
    ) -> AnalyticsResult<u64>;

    /// Mark `session_id` as having converted (signed up, paid, ...).
    async fn mark_session_converted(&self, session_id: &SessionId) -> AnalyticsResult<()>;
}

/// Browser fingerprint persistence and lookup.
///
/// `#[async_trait]` is required because the trait is consumed as
/// `Arc<dyn FingerprintProvider>` via [`DynFingerprintProvider`].
#[async_trait]
pub trait FingerprintProvider: Send + Sync {
    /// Count active sessions associated with `fingerprint`.
    async fn count_active_sessions(&self, fingerprint: &str) -> AnalyticsResult<i64>;

    /// Find a session that can be reused for `fingerprint`, returning its
    /// session id as a string.
    async fn find_reusable_session(&self, fingerprint: &str) -> AnalyticsResult<Option<String>>;

    /// Insert or update fingerprint metadata.
    async fn upsert_fingerprint(
        &self,
        fingerprint: &str,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
        screen_info: Option<&str>,
    ) -> AnalyticsResult<()>;
}

/// Shared `Arc` alias for [`AnalyticsProvider`].
pub type DynAnalyticsProvider = Arc<dyn AnalyticsProvider>;

/// Shared `Arc` alias for [`FingerprintProvider`].
pub type DynFingerprintProvider = Arc<dyn FingerprintProvider>;
