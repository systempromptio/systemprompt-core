//! The per-platform release cache the feed's two endpoints share.
//!
//! Every bridge in the fleet reads the feed on a timer, so resolution is
//! cached against a shared HTTP client: GitHub sees a couple of dozen calls an
//! hour no matter how large the fleet. A resolution that fails while a
//! previous answer is still held serves that answer — a GitHub blip must not
//! turn into a failed update check for everyone at once.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use axum::http::StatusCode;
use systemprompt_models::services::BridgeReleasesSpec;

use super::github::{asset_digest, resolve_release, sums_url};
use super::{CACHE_TTL, ReleaseManifest};

/// One platform's release as the feed resolved it: what `/latest` answers with
/// plus the asset `/download` streams.
#[derive(Debug, Clone)]
pub struct ResolvedRelease {
    pub manifest: ReleaseManifest,
    pub asset: ResolvedAsset,
}

/// Where a platform's asset lives, without the digest.
///
/// Separate because `/download` proxies bytes and never needs `SHA256SUMS`;
/// making it wait on that fetch would turn a missing checksum file into a
/// failed download of a binary that is right there.
#[derive(Debug, Clone)]
pub struct ResolvedAsset {
    pub version: String,
    pub size: u64,
    pub notes_url: Option<String>,
    pub name: String,
    pub url: String,
    sums_url: Option<String>,
}

#[derive(Debug)]
struct Cache<T>(tokio::sync::RwLock<HashMap<String, (Instant, T)>>);

impl<T> Default for Cache<T> {
    fn default() -> Self {
        Self(tokio::sync::RwLock::new(HashMap::new()))
    }
}

impl<T: Clone + Send + Sync> Cache<T> {
    async fn fresh(&self, key: &str, ttl: Duration) -> Option<T> {
        let held = self.0.read().await;
        held.get(key)
            .filter(|(at, _)| at.elapsed() < ttl)
            .map(|(_, value)| value.clone())
    }

    async fn stale(&self, key: &str) -> Option<T> {
        let held = self.0.read().await;
        held.get(key).map(|(_, value)| value.clone())
    }

    async fn store(&self, key: &str, value: T) {
        let mut held = self.0.write().await;
        held.insert(key.to_owned(), (Instant::now(), value));
    }
}

/// Router-scoped state for the release feed: one HTTP client and the
/// per-platform resolution caches both endpoints read.
#[derive(Debug, Default)]
pub struct ReleaseFeed {
    http: reqwest::Client,
    assets: Cache<ResolvedAsset>,
    digests: Cache<String>,
    ttl: Option<Duration>,
}

impl ReleaseFeed {
    #[must_use]
    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            ttl: Some(ttl),
            ..Self::default()
        }
    }

    pub async fn resolve(
        &self,
        spec: &BridgeReleasesSpec,
        platform: &str,
    ) -> Result<ResolvedRelease, (StatusCode, String)> {
        let asset = self.resolve_asset(spec, platform).await?;
        let sha256 = self.resolve_digest(spec, platform, &asset).await?;
        Ok(ResolvedRelease {
            manifest: ReleaseManifest {
                version: asset.version.clone(),
                sha256,
                size: asset.size,
                notes_url: asset.notes_url.clone(),
            },
            asset,
        })
    }

    pub async fn resolve_asset(
        &self,
        spec: &BridgeReleasesSpec,
        platform: &str,
    ) -> Result<ResolvedAsset, (StatusCode, String)> {
        if let Some(fresh) = self.assets.fresh(platform, self.ttl()).await {
            return Ok(fresh);
        }
        match self.resolve_asset_upstream(spec, platform).await {
            Ok(asset) => {
                self.assets.store(platform, asset.clone()).await;
                Ok(asset)
            },
            Err(err) => match self.assets.stale(platform).await {
                Some(stale) => {
                    tracing::warn!(
                        platform,
                        status = %err.0,
                        detail = %err.1,
                        version = %stale.version,
                        "bridge release resolution failed; serving the last known release"
                    );
                    Ok(stale)
                },
                None => Err(err),
            },
        }
    }

    async fn resolve_digest(
        &self,
        spec: &BridgeReleasesSpec,
        platform: &str,
        asset: &ResolvedAsset,
    ) -> Result<String, (StatusCode, String)> {
        if let Some(fresh) = self.digests.fresh(platform, self.ttl()).await {
            return Ok(fresh);
        }
        let published = asset.sums_url.as_deref().ok_or_else(|| {
            (
                StatusCode::BAD_GATEWAY,
                format!("release {} publishes no SHA256SUMS", asset.version),
            )
        });
        let fetched = match published {
            Ok(url) => asset_digest(&self.http, spec, url, &asset.name).await,
            Err(e) => Err(e),
        };
        match fetched {
            Ok(digest) => {
                self.digests.store(platform, digest.clone()).await;
                Ok(digest)
            },
            Err(err) => match self.digests.stale(platform).await {
                Some(stale) => {
                    tracing::warn!(
                        platform,
                        status = %err.0,
                        detail = %err.1,
                        "SHA256SUMS fetch failed; serving the last known digest"
                    );
                    Ok(stale)
                },
                None => Err(err),
            },
        }
    }

    async fn resolve_asset_upstream(
        &self,
        spec: &BridgeReleasesSpec,
        platform: &str,
    ) -> Result<ResolvedAsset, (StatusCode, String)> {
        let asset_name = spec.assets.get(platform).ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("no published build for platform {platform}"),
            )
        })?;

        let release = resolve_release(&self.http, spec).await?;
        let asset = release
            .assets
            .iter()
            .find(|a| a.name == *asset_name)
            .ok_or_else(|| {
                (
                    StatusCode::NOT_FOUND,
                    format!("release {} has no asset {asset_name}", release.tag_name),
                )
            })?;

        Ok(ResolvedAsset {
            version: release
                .tag_name
                .strip_prefix(&spec.tag_prefix)
                .unwrap_or(&release.tag_name)
                .to_owned(),
            size: asset.size,
            notes_url: release.html_url.clone(),
            name: asset.name.clone(),
            url: asset.url.clone(),
            sums_url: sums_url(&release).map(str::to_owned),
        })
    }

    pub(super) const fn http(&self) -> &reqwest::Client {
        &self.http
    }

    fn ttl(&self) -> Duration {
        self.ttl.unwrap_or(CACHE_TTL)
    }
}
