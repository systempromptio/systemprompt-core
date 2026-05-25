//! Async JWKS fetcher with a bounded LRU cache, HTTPS-only scheme enforcement,
//! and a per-deployment host allowlist.
//!
//! The client is keyed on issuer URL and caches the parsed [`Jwks`] together
//! with an expiry derived from the response's `Cache-Control: max-age=` header
//! (clamped between [`MIN_CACHE_TTL`] and [`MAX_CACHE_TTL`]; falls back to
//! [`DEFAULT_CACHE_TTL`] when the header is missing or unparseable). Callers
//! resolve a `(issuer, kid)` pair to a concrete [`Jwk`]; a cache miss for the
//! `kid` forces a refresh in case the issuer rotated keys mid-window.

use std::num::NonZeroUsize;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use lru::LruCache;
use reqwest::Client;
use url::Url;

use super::jwks::{Jwk, Jwks};

pub const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(300);
pub const MIN_CACHE_TTL: Duration = Duration::from_secs(30);
pub const MAX_CACHE_TTL: Duration = Duration::from_secs(3600);
pub const DEFAULT_CACHE_CAPACITY: usize = 32;
/// Minimum interval between unknown-`kid`-triggered JWKS refetches for a
/// single issuer. Caps the DoS amplification when an attacker spams tokens
/// with random `kid` headers; legitimate rotations are still picked up
/// after at most this delay (well under any sane rotation window).
pub const DEFAULT_MIN_REFRESH_INTERVAL: Duration = Duration::from_secs(10);
const WELLKNOWN_JWKS_PATH: &str = "/.well-known/jwks.json";

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

enum CacheProbe {
    Hit(Jwk),
    Miss,
    Expired,
    KidMissRefetchAllowed,
    KidMissRecentlyFetched,
}

#[derive(Clone)]
struct CachedJwks {
    jwks: Jwks,
    expires_at: Instant,
    last_kid_miss_refetch_at: Option<Instant>,
}

pub struct JwksClient {
    http: Client,
    allowed_hosts: Vec<String>,
    cache: Mutex<LruCache<String, CachedJwks>>,
    min_refresh_interval: Duration,
    min_cache_ttl: Duration,
    max_cache_ttl: Duration,
    default_cache_ttl: Duration,
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
    /// refetches. Set to `Duration::ZERO` to disable the DoS guard (tests
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

    pub async fn fetch(&self, issuer: &str, kid: &str) -> Result<Jwk, JwksClientError> {
        match self.lookup(issuer, kid) {
            CacheProbe::Hit(jwk) => return Ok(jwk),
            CacheProbe::KidMissRecentlyFetched => {
                return Err(JwksClientError::KeyNotFound {
                    issuer: issuer.to_string(),
                    kid: kid.to_string(),
                });
            },
            CacheProbe::Miss | CacheProbe::KidMissRefetchAllowed | CacheProbe::Expired => {},
        }

        let url = self.build_jwks_url(issuer)?;
        self.fetch_and_resolve(issuer, &url, kid).await
    }

    /// Same as [`Self::fetch`], but takes an explicit JWKS URI (as configured
    /// on a trusted issuer entry) rather than deriving
    /// `<issuer>/.well-known/jwks.json`. The cache key remains the `issuer`
    /// so two trusted issuers cannot collide even when they share the same
    /// JWKS document host.
    pub async fn fetch_at(
        &self,
        issuer: &str,
        jwks_uri: &str,
        kid: &str,
    ) -> Result<Jwk, JwksClientError> {
        match self.lookup(issuer, kid) {
            CacheProbe::Hit(jwk) => return Ok(jwk),
            CacheProbe::KidMissRecentlyFetched => {
                return Err(JwksClientError::KeyNotFound {
                    issuer: issuer.to_string(),
                    kid: kid.to_string(),
                });
            },
            CacheProbe::Miss | CacheProbe::KidMissRefetchAllowed | CacheProbe::Expired => {},
        }

        let url = self.validate_uri(jwks_uri)?;
        self.fetch_and_resolve(issuer, &url, kid).await
    }

    async fn fetch_and_resolve(
        &self,
        issuer: &str,
        url: &Url,
        kid: &str,
    ) -> Result<Jwk, JwksClientError> {
        let (jwks, ttl) = self.fetch_remote(url).await?;
        let now = Instant::now();
        let kid_present = jwks.keys.iter().any(|k| k.kid == kid);
        let cached = CachedJwks {
            jwks: jwks.clone(),
            expires_at: now + ttl,
            // Only record the kid-miss refetch timestamp when the refetch
            // failed to surface the requested kid. A successful rotation
            // pickup resets the throttle so the next rotation isn't
            // blocked by the previous one.
            last_kid_miss_refetch_at: if kid_present { None } else { Some(now) },
        };
        if let Ok(mut guard) = self.cache.lock() {
            guard.put(issuer.to_owned(), cached);
        }

        jwks.keys
            .into_iter()
            .find(|k| k.kid == kid)
            .ok_or_else(|| JwksClientError::KeyNotFound {
                issuer: issuer.to_owned(),
                kid: kid.to_owned(),
            })
    }

    fn lookup(&self, issuer: &str, kid: &str) -> CacheProbe {
        let Ok(mut guard) = self.cache.lock() else {
            return CacheProbe::Miss;
        };
        let Some(entry) = guard.get(issuer) else {
            return CacheProbe::Miss;
        };
        let now = Instant::now();
        if entry.expires_at <= now {
            guard.pop(issuer);
            return CacheProbe::Expired;
        }
        if let Some(jwk) = entry.jwks.keys.iter().find(|k| k.kid == kid).cloned() {
            return CacheProbe::Hit(jwk);
        }
        match entry.last_kid_miss_refetch_at {
            Some(last) if now.duration_since(last) < self.min_refresh_interval => {
                CacheProbe::KidMissRecentlyFetched
            },
            _ => CacheProbe::KidMissRefetchAllowed,
        }
    }

    fn build_jwks_url(&self, issuer: &str) -> Result<Url, JwksClientError> {
        let mut url = self.validate_uri(issuer)?;
        url.set_path(WELLKNOWN_JWKS_PATH);
        url.set_query(None);
        url.set_fragment(None);
        Ok(url)
    }

    fn validate_uri(&self, raw: &str) -> Result<Url, JwksClientError> {
        let parsed = Url::parse(raw).map_err(|source| JwksClientError::InvalidIssuer {
            issuer: raw.to_owned(),
            source,
        })?;
        #[cfg(feature = "test-jwks-insecure-scheme")]
        let allowed_scheme = parsed.scheme() == "https" || parsed.scheme() == "http";
        #[cfg(not(feature = "test-jwks-insecure-scheme"))]
        let allowed_scheme = parsed.scheme() == "https";
        if !allowed_scheme {
            return Err(JwksClientError::InsecureScheme(raw.to_owned()));
        }
        let host = parsed
            .host_str()
            .ok_or_else(|| JwksClientError::HostNotAllowed(raw.to_owned()))?
            .to_owned();
        if !self
            .allowed_hosts
            .iter()
            .any(|h| h.eq_ignore_ascii_case(&host))
        {
            return Err(JwksClientError::HostNotAllowed(host));
        }
        Ok(parsed)
    }

    async fn fetch_remote(&self, url: &Url) -> Result<(Jwks, Duration), JwksClientError> {
        let response =
            self.http
                .get(url.clone())
                .send()
                .await
                .map_err(|source| JwksClientError::Http {
                    url: url.to_string(),
                    source,
                })?;

        let status = response.status();
        if !status.is_success() {
            return Err(JwksClientError::Status {
                url: url.to_string(),
                status: status.as_u16(),
            });
        }

        let ttl = response
            .headers()
            .get(reqwest::header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok())
            .and_then(parse_max_age)
            .map_or(self.default_cache_ttl, |raw| {
                raw.clamp(self.min_cache_ttl, self.max_cache_ttl)
            });

        let jwks = response
            .json::<Jwks>()
            .await
            .map_err(|source| JwksClientError::Decode {
                url: url.to_string(),
                source,
            })?;

        Ok((jwks, ttl))
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
