//! The per-platform release cache the feed's two endpoints share.
//!
//! Every bridge in the fleet reads the feed on a timer, so resolution is
//! cached against a shared HTTP client with connect and read timeouts. A miss
//! is single-flight per platform, and a forced refresh is honoured at most
//! once per [`REFRESH_INTERVAL`] per platform, so the number of GitHub calls is
//! bounded by the platform count rather than by fleet size or concurrency. A
//! resolution that fails while a previous answer is still held serves that
//! answer — a GitHub blip must not turn into a failed update check for everyone
//! at once.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::{Duration, Instant};

use systemprompt_manifest::Secrets;
use systemprompt_manifest::services::BridgeReleasesSpec;
use systemprompt_models::bridge::gateway::ReleaseManifest;

use super::CACHE_TTL;
use super::cache::{Cache, RefreshThrottle};
use super::error::ReleaseError;
use super::github::{asset_digest, resolve_release, sums_url};
pub use super::upstream::{ResolvedAsset, ResolvedRelease};
use super::upstream::{http_client, known_platform, server_version};


pub const REFRESH_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy)]
struct Upstream<'a> {
    spec: &'a BridgeReleasesSpec,
    secrets: &'a Secrets,
}

#[derive(Debug, Clone, Copy)]
struct Lookup {
    started: Instant,
    bypass_cache: bool,
}

impl Lookup {
    fn since(self) -> Option<Instant> {
        self.bypass_cache.then_some(self.started)
    }
}

/// Router-scoped state for the release feed: one HTTP client and the
/// per-platform resolution caches both endpoints read.
///
/// The feed only offers releases this server supports: the newest release
/// whose major.minor is no higher than the server's own version, so a bridge
/// published ahead of a deployment is not handed to its clients.
#[derive(Debug)]
pub struct ReleaseFeed {
    http: reqwest::Client,
    assets: Cache<ResolvedAsset>,
    digests: Cache<String>,
    refresh: RefreshThrottle,
    ttl: Option<Duration>,
    ceiling: semver::Version,
}

impl ReleaseFeed {
    pub fn new() -> Result<Self, ReleaseError> {
        Self::with_ceiling(server_version())
    }

    pub fn with_ceiling(ceiling: semver::Version) -> Result<Self, ReleaseError> {
        Ok(Self {
            http: http_client()?,
            assets: Cache::default(),
            digests: Cache::default(),
            refresh: RefreshThrottle::new(REFRESH_INTERVAL),
            ttl: None,
            ceiling,
        })
    }

    pub fn with_ttl(ttl: Duration) -> Result<Self, ReleaseError> {
        Ok(Self {
            ttl: Some(ttl),
            ..Self::new()?
        })
    }

    #[must_use]
    pub const fn ceiling(&self) -> &semver::Version {
        &self.ceiling
    }

    pub async fn resolve(
        &self,
        spec: &BridgeReleasesSpec,
        secrets: &Secrets,
        platform: &str,
    ) -> Result<ResolvedRelease, ReleaseError> {
        self.resolve_with(spec, secrets, platform, false).await
    }

    pub async fn resolve_fresh(
        &self,
        spec: &BridgeReleasesSpec,
        secrets: &Secrets,
        platform: &str,
    ) -> Result<ResolvedRelease, ReleaseError> {
        self.resolve_with(spec, secrets, platform, true).await
    }

    async fn resolve_with(
        &self,
        spec: &BridgeReleasesSpec,
        secrets: &Secrets,
        platform: &str,
        refresh_requested: bool,
    ) -> Result<ResolvedRelease, ReleaseError> {
        known_platform(spec, platform)?;
        let lookup = Lookup {
            started: Instant::now(),
            bypass_cache: refresh_requested && self.refresh.admit(platform).await,
        };
        let asset = self
            .resolve_asset_with(Upstream { spec, secrets }, platform, lookup)
            .await?;
        let sha256 = self
            .resolve_digest(Upstream { spec, secrets }, platform, &asset, lookup)
            .await?;
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
        secrets: &Secrets,
        platform: &str,
    ) -> Result<ResolvedAsset, ReleaseError> {
        known_platform(spec, platform)?;
        let lookup = Lookup {
            started: Instant::now(),
            bypass_cache: false,
        };
        self.resolve_asset_with(Upstream { spec, secrets }, platform, lookup)
            .await
    }

    async fn resolve_asset_with(
        &self,
        upstream: Upstream<'_>,
        platform: &str,
        lookup: Lookup,
    ) -> Result<ResolvedAsset, ReleaseError> {
        let Upstream { spec, secrets } = upstream;
        if !lookup.bypass_cache
            && let Some(fresh) = self.assets.fresh(platform, self.ttl()).await
        {
            return Ok(fresh);
        }
        let _flight = self.assets.flight(platform).await;
        if let Some(fresh) = self
            .assets
            .fresh_since(platform, self.ttl(), lookup.since())
            .await
        {
            return Ok(fresh);
        }
        match self.resolve_asset_upstream(spec, secrets, platform).await {
            Ok(asset) => {
                self.assets.store(platform, asset.clone()).await;
                Ok(asset)
            },
            Err(err) => match self.assets.stale(platform).await {
                Some(stale) => {
                    tracing::warn!(
                        platform,
                        error = %err,
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
        upstream: Upstream<'_>,
        platform: &str,
        asset: &ResolvedAsset,
        lookup: Lookup,
    ) -> Result<String, ReleaseError> {
        let Upstream { spec, secrets } = upstream;
        if !lookup.bypass_cache
            && let Some(fresh) = self.digests.fresh(platform, self.ttl()).await
        {
            return Ok(fresh);
        }
        let _flight = self.digests.flight(platform).await;
        if let Some(fresh) = self
            .digests
            .fresh_since(platform, self.ttl(), lookup.since())
            .await
        {
            return Ok(fresh);
        }
        let published = asset
            .sums_url
            .as_deref()
            .ok_or_else(|| ReleaseError::NoChecksums {
                version: asset.version.clone(),
            });
        let fetched = match published {
            Ok(url) => asset_digest(&self.http, spec, secrets, url, &asset.name).await,
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
                        error = %err,
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
        secrets: &Secrets,
        platform: &str,
    ) -> Result<ResolvedAsset, ReleaseError> {
        let asset_name = known_platform(spec, platform)?;

        let release = resolve_release(&self.http, spec, secrets, &self.ceiling).await?;
        let asset = release
            .assets
            .iter()
            .find(|a| a.name == *asset_name)
            .ok_or_else(|| ReleaseError::MissingAsset {
                tag: release.tag_name.clone(),
                asset: asset_name.clone(),
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
