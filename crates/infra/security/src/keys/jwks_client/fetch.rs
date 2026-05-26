use std::time::{Duration, Instant};

use url::Url;

use super::cache::{CacheProbe, CachedJwks};
use super::{JwksClient, JwksClientError, WELLKNOWN_JWKS_PATH, parse_max_age};
use crate::keys::jwks::{Jwk, Jwks};

impl JwksClient {
    pub async fn fetch(&self, issuer: &str, kid: &str) -> Result<Jwk, JwksClientError> {
        match self.lookup(issuer, kid) {
            CacheProbe::Hit(jwk) => return Ok(jwk),
            CacheProbe::KidMissRecentlyFetched => {
                return Err(JwksClientError::KeyNotFound {
                    issuer: issuer.to_owned(),
                    kid: kid.to_owned(),
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
                    issuer: issuer.to_owned(),
                    kid: kid.to_owned(),
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
