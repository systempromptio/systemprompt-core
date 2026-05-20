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

#[derive(Clone)]
struct CachedJwks {
    jwks: Jwks,
    expires_at: Instant,
}

pub struct JwksClient {
    http: Client,
    allowed_hosts: Vec<String>,
    cache: Mutex<LruCache<String, CachedJwks>>,
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
        }
    }

    pub fn with_http_client(mut self, client: Client) -> Self {
        self.http = client;
        self
    }

    pub async fn fetch(&self, issuer: &str, kid: &str) -> Result<Jwk, JwksClientError> {
        if let Some(jwk) = self.lookup_cached(issuer, kid) {
            return Ok(jwk);
        }

        let url = self.build_jwks_url(issuer)?;
        let (jwks, ttl) = self.fetch_remote(&url).await?;

        let cached = CachedJwks {
            jwks: jwks.clone(),
            expires_at: Instant::now() + ttl,
        };
        if let Ok(mut guard) = self.cache.lock() {
            guard.put(issuer.to_string(), cached);
        }

        jwks.keys
            .into_iter()
            .find(|k| k.kid == kid)
            .ok_or_else(|| JwksClientError::KeyNotFound {
                issuer: issuer.to_string(),
                kid: kid.to_string(),
            })
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
        if let Some(jwk) = self.lookup_cached(issuer, kid) {
            return Ok(jwk);
        }

        let url = self.validate_uri(jwks_uri)?;
        let (jwks, ttl) = self.fetch_remote(&url).await?;

        let cached = CachedJwks {
            jwks: jwks.clone(),
            expires_at: Instant::now() + ttl,
        };
        if let Ok(mut guard) = self.cache.lock() {
            guard.put(issuer.to_string(), cached);
        }

        jwks.keys
            .into_iter()
            .find(|k| k.kid == kid)
            .ok_or_else(|| JwksClientError::KeyNotFound {
                issuer: issuer.to_string(),
                kid: kid.to_string(),
            })
    }

    fn lookup_cached(&self, issuer: &str, kid: &str) -> Option<Jwk> {
        let mut guard = self.cache.lock().ok()?;
        let entry = guard.get(issuer)?;
        if entry.expires_at <= Instant::now() {
            guard.pop(issuer);
            return None;
        }
        let found = entry.jwks.keys.iter().find(|k| k.kid == kid).cloned();
        drop(guard);
        found
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
            issuer: raw.to_string(),
            source,
        })?;
        if parsed.scheme() != "https" {
            return Err(JwksClientError::InsecureScheme(raw.to_string()));
        }
        let host = parsed
            .host_str()
            .ok_or_else(|| JwksClientError::HostNotAllowed(raw.to_string()))?
            .to_string();
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
            .map_or(DEFAULT_CACHE_TTL, clamp_ttl);

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
