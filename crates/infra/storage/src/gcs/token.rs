//! Bearer tokens for Cloud Storage.
//!
//! The storage crate never signs a credential itself: a [`GcsTokenSource`] is
//! injected. [`MetadataServerTokens`] covers workload identity (GKE, Cloud
//! Run, GCE), where the metadata server mints the bound service account's
//! token; a key-file source lives with the composition root, which already
//! links the security crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde::Deserialize;
use tokio::sync::Mutex;
use url::Url;

use super::GcsError;

pub const DEFAULT_METADATA_TOKEN_URL: &str =
    "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";

const EARLY_REFRESH: Duration = Duration::from_secs(60);

/// A source of OAuth bearer tokens for the Cloud Storage JSON API.
///
/// Held as `Arc<dyn GcsTokenSource>` by the backend, so it uses
/// `#[async_trait]` for `dyn`-compatibility.
#[async_trait]
pub trait GcsTokenSource: Send + Sync {
    async fn bearer(&self) -> Result<String, GcsError>;
}

#[derive(Debug, Clone)]
struct CachedToken {
    token: String,
    refresh_at: Instant,
}

#[derive(Debug, Deserialize)]
struct MetadataToken {
    access_token: String,
    expires_in: u64,
}

/// Workload-identity tokens from the GCE metadata server.
///
/// The token is cached and refreshed ahead of expiry (60 s, or half the
/// lifetime for a shorter-lived token). The cache sits behind one async
/// mutex held across the mint, so concurrent callers wait for a single
/// request rather than each minting their own.
#[derive(Debug)]
pub struct MetadataServerTokens {
    endpoint: Url,
    http: reqwest::Client,
    cache: Mutex<Option<CachedToken>>,
}

impl MetadataServerTokens {
    #[must_use]
    pub const fn new(endpoint: Url, http: reqwest::Client) -> Self {
        Self {
            endpoint,
            http,
            cache: Mutex::const_new(None),
        }
    }

    async fn mint(&self) -> Result<CachedToken, GcsError> {
        let response = self
            .http
            .get(self.endpoint.clone())
            .header("Metadata-Flavor", "Google")
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            return Err(GcsError::Status {
                status: status.as_u16(),
                body: response.text().await?,
            });
        }
        let token: MetadataToken = serde_json::from_slice(&response.bytes().await?)?;
        let lifetime = Duration::from_secs(token.expires_in);
        let margin = EARLY_REFRESH.min(lifetime / 2);
        Ok(CachedToken {
            token: token.access_token,
            refresh_at: Instant::now() + lifetime.saturating_sub(margin),
        })
    }
}

#[async_trait]
impl GcsTokenSource for MetadataServerTokens {
    async fn bearer(&self) -> Result<String, GcsError> {
        let mut cache = self.cache.lock().await;
        if let Some(cached) = cache.as_ref()
            && Instant::now() < cached.refresh_at
        {
            return Ok(cached.token.clone());
        }
        let fresh = self.mint().await?;
        let token = fresh.token.clone();
        *cache = Some(fresh);
        drop(cache);
        Ok(token)
    }
}
