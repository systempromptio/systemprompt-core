//! The issuer's `OpenID Connect` discovery document, fetched on demand and
//! cached in memory.
//!
//! The document's `issuer` must equal the configured one and every endpoint
//! it names must pass the outbound-URL guard, so a discovery response cannot
//! steer the token exchange or the key fetch somewhere else. A failed refresh
//! keeps serving the last good document.
//!
//! A refresh is single-flight: one caller fetches while the others serve the
//! expired document, or wait for the fetch when there is none yet, so an
//! expiry under a login wave costs the issuer one request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::Deserialize;
use systemprompt_models::net::validate_outbound_url;

use super::error::OidcProviderError;

const DISCOVERY_TTL: Duration = Duration::from_secs(3600);

#[derive(Debug, Clone, Deserialize)]
pub struct OidcDiscovery {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub jwks_uri: String,
}

impl OidcDiscovery {
    pub fn check(&self, expected_issuer: &str) -> Result<(), OidcProviderError> {
        if self.issuer.trim_end_matches('/') != expected_issuer {
            return Err(OidcProviderError::IssuerMismatch {
                expected: expected_issuer.to_owned(),
                discovered: self.issuer.clone(),
            });
        }
        for (field, endpoint) in [
            ("authorization_endpoint", &self.authorization_endpoint),
            ("token_endpoint", &self.token_endpoint),
            ("jwks_uri", &self.jwks_uri),
        ] {
            validate_outbound_url(endpoint)
                .map_err(|source| OidcProviderError::Endpoint { field, source })?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(super) struct DiscoveryCache {
    url: String,
    issuer: String,
    current: Mutex<Option<(Arc<OidcDiscovery>, Instant)>>,
    refresh: tokio::sync::Mutex<()>,
}

impl DiscoveryCache {
    pub(super) fn new(url: String, issuer: String) -> Self {
        Self {
            url,
            issuer,
            current: Mutex::new(None),
            refresh: tokio::sync::Mutex::new(()),
        }
    }

    fn cached(&self) -> Option<(Arc<OidcDiscovery>, Instant)> {
        self.current
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .map(|(doc, at)| (Arc::clone(doc), *at))
    }

    pub(super) async fn current(
        &self,
        http: &reqwest::Client,
    ) -> Result<Arc<OidcDiscovery>, OidcProviderError> {
        let cached = self.cached();
        if let Some((doc, fetched_at)) = &cached
            && fetched_at.elapsed() < DISCOVERY_TTL
        {
            return Ok(Arc::clone(doc));
        }
        let _refresh = match (self.refresh.try_lock(), &cached) {
            (Ok(guard), _) => guard,
            (Err(_busy), Some((doc, _))) => return Ok(Arc::clone(doc)),
            (Err(_busy), None) => self.refresh.lock().await,
        };
        let cached = self.cached();
        if let Some((doc, fetched_at)) = &cached
            && fetched_at.elapsed() < DISCOVERY_TTL
        {
            return Ok(Arc::clone(doc));
        }
        match self.fetch(http).await {
            Ok(doc) => {
                let doc = Arc::new(doc);
                *self.current.lock().unwrap_or_else(PoisonError::into_inner) =
                    Some((Arc::clone(&doc), Instant::now()));
                Ok(doc)
            },
            Err(error) => match cached {
                Some((doc, _)) => {
                    tracing::warn!(%error, url = %self.url, "OIDC discovery refresh failed; keeping the last good document");
                    Ok(doc)
                },
                None => Err(error),
            },
        }
    }

    async fn fetch(&self, http: &reqwest::Client) -> Result<OidcDiscovery, OidcProviderError> {
        let destination =
            validate_outbound_url(&self.url).map_err(|source| OidcProviderError::Endpoint {
                field: "discovery_url",
                source,
            })?;
        let response = http
            .get(destination)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|source| OidcProviderError::Discovery {
                url: self.url.clone(),
                source,
            })?;
        let doc = response.json::<OidcDiscovery>().await.map_err(|source| {
            OidcProviderError::Discovery {
                url: self.url.clone(),
                source,
            }
        })?;
        doc.check(&self.issuer)?;
        Ok(doc)
    }
}
