//! The minted-token cache, shared by every credential type that mints one.
//!
//! Minting is a round trip to an identity provider in front of the round trip
//! the caller actually wants, so a token is reused until shortly before it
//! expires. The skew is deliberate: a token that expires in flight fails the
//! *user's* request, so it is retired early rather than used to the last
//! second.
//!
//! The cache is also single-flight. Without it, a cold instance taking its
//! first hundred concurrent requests mints a hundred tokens — a burst that
//! identity providers rate-limit and that leaves ninety-nine tokens
//! outstanding. Callers that miss the cache queue behind one per-key lock and
//! re-read the cache when they get it, so exactly one exchange happens.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, OnceLock, PoisonError, RwLock};
use std::time::{Duration, SystemTime};

use super::error::CredentialError;

/// A token is retired this long before it expires.
const EXPIRY_SKEW: Duration = Duration::from_secs(120);

/// Floor for a provider-declared lifetime. Anything shorter would be re-minted
/// on every request anyway, so it is treated as a provider quirk, not a rule.
const MIN_TTL: Duration = Duration::from_secs(60);

/// Ceiling for a provider-declared lifetime, matching the longest assertion
/// lifetime we are willing to sign. A provider claiming more is not trusted
/// to be right about it.
const MAX_TTL: Duration = Duration::from_secs(3600);

#[derive(Debug, Clone)]
struct CachedToken {
    token: String,
    expires_at: SystemTime,
}

#[derive(Default)]
struct TokenCache {
    entries: RwLock<HashMap<String, CachedToken>>,
    mints: tokio::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

fn cache() -> &'static TokenCache {
    static CACHE: OnceLock<TokenCache> = OnceLock::new();
    CACHE.get_or_init(TokenCache::default)
}

/// Clamp a provider-declared lifetime into the range we are willing to cache.
///
/// `None` means the provider declared nothing, which is not an error: the
/// token is simply cached for the longest lifetime we would have asked for.
#[must_use]
pub fn clamp_ttl(declared: Option<u64>) -> Duration {
    match declared {
        None | Some(0) => MAX_TTL,
        Some(secs) => Duration::from_secs(secs).clamp(MIN_TTL, MAX_TTL),
    }
}

fn cached(key: &str) -> Option<String> {
    let guard = cache()
        .entries
        .read()
        .unwrap_or_else(PoisonError::into_inner);
    let token = guard.get(key).and_then(|entry| {
        (entry.expires_at > SystemTime::now() + EXPIRY_SKEW).then(|| entry.token.clone())
    });
    drop(guard);
    token
}

fn store(key: &str, token: &str, ttl: Duration) {
    cache()
        .entries
        .write()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(
            key.to_owned(),
            CachedToken {
                token: token.to_owned(),
                expires_at: SystemTime::now() + ttl,
            },
        );
}

/// Return the cached token for `key`, or run `mint` exactly once to produce
/// one.
///
/// A failed mint is not cached: the next caller retries. That is deliberate —
/// the usual cause is an IAM change an operator is in the middle of making,
/// and caching the failure would outlast the fix.
pub async fn token_for<F, Fut>(key: &str, mint: F) -> Result<String, CredentialError>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<(String, Duration), CredentialError>>,
{
    if let Some(token) = cached(key) {
        return Ok(token);
    }

    // Why: the per-key lock is held in a map that is never pruned. Its keys
    // are secret names, of which a deployment has a handful, so the map is
    // bounded by configuration rather than by traffic.
    let lock = {
        let mut mints = cache().mints.lock().await;
        Arc::clone(mints.entry(key.to_owned()).or_default())
    };
    let _minting = lock.lock().await;

    if let Some(token) = cached(key) {
        return Ok(token);
    }

    let (token, ttl) = mint().await?;
    store(key, &token, ttl);
    Ok(token)
}
