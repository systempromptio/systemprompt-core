use std::time::Instant;

use super::JwksClient;
use crate::keys::jwks::{Jwk, Jwks};

pub(in crate::keys) enum CacheProbe {
    Hit(Jwk),
    Miss,
    Expired,
    KidMissRefetchAllowed,
    KidMissRecentlyFetched,
}

#[derive(Clone)]
pub(in crate::keys) struct CachedJwks {
    pub(in crate::keys) jwks: Jwks,
    pub(in crate::keys) expires_at: Instant,
    pub(in crate::keys) last_kid_miss_refetch_at: Option<Instant>,
}

impl JwksClient {
    pub(in crate::keys) fn lookup(&self, issuer: &str, kid: &str) -> CacheProbe {
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
}
