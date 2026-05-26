//! Async JWKS fetcher with a bounded LRU cache, HTTPS-only scheme enforcement,
//! and a per-deployment host allowlist.
//!
//! The client is keyed on issuer URL and caches the parsed
//! [`super::jwks::Jwks`] together with an expiry derived from the response's
//! `Cache-Control: max-age=` header (clamped between [`MIN_CACHE_TTL`] and
//! [`MAX_CACHE_TTL`]; falls back to [`DEFAULT_CACHE_TTL`] when the header is
//! missing or unparseable). Callers resolve a `(issuer, kid)` pair to a
//! concrete [`super::jwks::Jwk`]; a cache miss for the `kid` forces a refresh
//! in case the issuer rotated keys mid-window.

mod cache;
mod fetch;

use std::num::NonZeroUsize;
use std::sync::Mutex;
use std::time::Duration;

use lru::LruCache;
use reqwest::Client;

use self::cache::CachedJwks;

pub const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(300);
pub const MIN_CACHE_TTL: Duration = Duration::from_secs(30);
pub const MAX_CACHE_TTL: Duration = Duration::from_secs(3600);
pub const DEFAULT_CACHE_CAPACITY: usize = 32;

/// Minimum interval between unknown-`kid`-triggered JWKS refetches for a
/// single issuer.
///
/// Caps the `DoS` amplification when an attacker spams tokens with random
/// `kid` headers; legitimate rotations are still picked up after at most
/// this delay (well under any sane rotation window).
pub const DEFAULT_MIN_REFRESH_INTERVAL: Duration = Duration::from_secs(10);

pub(super) const WELLKNOWN_JWKS_PATH: &str = "/.well-known/jwks.json";

#[derive(Debug, thiserror::Error)]
pub enum JwksClientError {
    #[error("invalid issuer URL '{issuer}': {source}")]
    InvalidIssuer {
        issuer: String,
        #[source]
        source: url::ParseError,
    },
    #[error("issuer '{0}' must use https scheme")]
    InsecureScheme(String),
    #[error("issuer host '{0}' is not in the trusted allowlist")]
    HostNotAllowed(String),
    #[error("HTTP request to '{url}' failed: {source}")]
    Http {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("non-success status {status} from '{url}'")]
    Status { url: String, status: u16 },
    #[error("failed to parse JWKS body from '{url}': {source}")]
    Decode {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("issuer '{issuer}' has no key with kid '{kid}'")]
    KeyNotFound { issuer: String, kid: String },
}

pub struct JwksClient {
    pub(super) http: Client,
    pub(super) allowed_hosts: Vec<String>,
    pub(super) cache: Mutex<LruCache<String, CachedJwks>>,
    pub(super) min_refresh_interval: Duration,
    pub(super) min_cache_ttl: Duration,
    pub(super) max_cache_ttl: Duration,
    pub(super) default_cache_ttl: Duration,
}

impl std::fmt::Debug for JwksClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwksClient")
            .field("allowed_hosts", &self.allowed_hosts)
            .finish_non_exhaustive()
    }
}

impl JwksClient {
    pub fn new(allowed_hosts: Vec<String>) -> Self {
        Self::with_capacity(allowed_hosts, DEFAULT_CACHE_CAPACITY)
    }

    pub fn with_capacity(allowed_hosts: Vec<String>, capacity: usize) -> Self {
        let cap = NonZeroUsize::new(capacity.max(1)).unwrap_or(NonZeroUsize::MIN);
        Self {
            http: Client::new(),
            allowed_hosts,
            cache: Mutex::new(LruCache::new(cap)),
            min_refresh_interval: DEFAULT_MIN_REFRESH_INTERVAL,
            min_cache_ttl: MIN_CACHE_TTL,
            max_cache_ttl: MAX_CACHE_TTL,
            default_cache_ttl: DEFAULT_CACHE_TTL,
        }
    }

    pub fn with_http_client(mut self, client: Client) -> Self {
        self.http = client;
        self
    }

    /// Override the per-issuer minimum interval between unknown-`kid`
    /// refetches. Set to `Duration::ZERO` to disable the `DoS` guard (tests
    /// only — production callers should keep the default).
    #[must_use]
    pub const fn with_min_refresh_interval(mut self, interval: Duration) -> Self {
        self.min_refresh_interval = interval;
        self
    }

    /// Override the cache TTL bounds and default. Production callers use
    /// the [`MIN_CACHE_TTL`] / [`MAX_CACHE_TTL`] / [`DEFAULT_CACHE_TTL`]
    /// values; tests use shorter values to exercise expiry behaviour
    /// without sleeping.
    #[must_use]
    pub const fn with_cache_ttl(mut self, min: Duration, max: Duration, default: Duration) -> Self {
        self.min_cache_ttl = min;
        self.max_cache_ttl = max;
        self.default_cache_ttl = default;
        self
    }
}

pub fn parse_max_age(header: &str) -> Option<Duration> {
    for directive in header.split(',') {
        let trimmed = directive.trim();
        if let Some(rest) = trimmed.strip_prefix("max-age=") {
            if let Ok(secs) = rest.trim().parse::<u64>() {
                return Some(Duration::from_secs(secs));
            }
        }
    }
    None
}

pub fn clamp_ttl(ttl: Duration) -> Duration {
    ttl.clamp(MIN_CACHE_TTL, MAX_CACHE_TTL)
}
