//! GitHub release resolution for the bridge feed: picking the newest matching
//! `bridge-v*` release this server supports and reading its signed SHA256SUMS
//! entry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::header;
use serde::Deserialize;
use systemprompt_manifest::Secrets;
use systemprompt_manifest::services::BridgeReleasesSpec;
use systemprompt_models::bridge::instance::BridgeVersionWindow;

use super::error::ReleaseError;

const RELEASE_PAGE_SIZE: u8 = 30;

#[derive(Debug, Deserialize)]
pub(super) struct GhRelease {
    pub(super) tag_name: String,
    #[serde(default)]
    pub(super) html_url: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    pub(super) assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GhAsset {
    pub(super) name: String,
    pub(super) url: String,
    #[serde(default)]
    pub(super) size: u64,
}

pub(super) async fn resolve_release(
    http: &reqwest::Client,
    spec: &BridgeReleasesSpec,
    secrets: &Secrets,
    ceiling: &semver::Version,
) -> Result<GhRelease, ReleaseError> {
    let window = spec.version_window();
    if let Some(pinned) = spec.pinned_version.as_deref() {
        let admitted = semver::Version::parse(pinned).is_ok_and(|v| window.admits(&v));
        if !admitted {
            return Err(ReleaseError::PinnedOutsideWindow {
                pinned: pinned.to_owned(),
                window: describe_window(&window),
            });
        }
        let tag = format!("{}{pinned}", spec.tag_prefix);
        let url = format!(
            "{}/repos/{}/releases/tags/{tag}",
            spec.api_base(),
            spec.repo
        );
        return fetch_json::<GhRelease>(http, spec, secrets, &url).await;
    }

    let url = format!(
        "{}/repos/{}/releases?per_page={RELEASE_PAGE_SIZE}",
        spec.api_base(),
        spec.repo
    );
    let releases = fetch_json::<Vec<GhRelease>>(http, spec, secrets, &url).await?;
    let published: Vec<(semver::Version, GhRelease)> = releases
        .into_iter()
        .filter(|r| !r.draft && !r.prerelease)
        .filter_map(|r| {
            let version =
                semver::Version::parse(r.tag_name.strip_prefix(&spec.tag_prefix)?).ok()?;
            Some((version, r))
        })
        .collect();
    if published.is_empty() {
        return Err(ReleaseError::NoRelease {
            prefix: spec.tag_prefix.clone(),
            repo: spec.repo.clone(),
        });
    }
    published
        .into_iter()
        .filter(|(v, _)| (v.major, v.minor) <= (ceiling.major, ceiling.minor))
        .filter(|(v, _)| window.admits(v))
        .max_by(|(a, _), (b, _)| a.cmp(b))
        .map(|(_, r)| r)
        .ok_or_else(|| ReleaseError::NoCompatibleRelease {
            prefix: spec.tag_prefix.clone(),
            repo: spec.repo.clone(),
            ceiling: ceiling.to_string(),
            window: describe_window(&window),
        })
}

fn describe_window(window: &BridgeVersionWindow) -> String {
    let bound = |v: Option<&semver::Version>| v.map_or_else(|| "*".to_owned(), ToString::to_string);
    format!(
        "[{}, {}]",
        bound(window.min.as_ref()),
        bound(window.max.as_ref())
    )
}

pub(super) fn sums_url(release: &GhRelease) -> Option<&str> {
    release
        .assets
        .iter()
        .find(|a| a.name == "SHA256SUMS")
        .map(|a| a.url.as_str())
}

pub(super) async fn asset_digest(
    http: &reqwest::Client,
    spec: &BridgeReleasesSpec,
    secrets: &Secrets,
    sums_url: &str,
    asset_name: &str,
) -> Result<String, ReleaseError> {
    let resp = github(http, spec, secrets, sums_url)?
        .header(header::ACCEPT, "application/octet-stream")
        .send()
        .await
        .map_err(|source| ReleaseError::Request {
            stage: "SHA256SUMS fetch",
            source,
        })?;
    // Why: GitHub answers a refused or rate-limited asset download with a JSON
    // error body, which would otherwise parse as a SHA256SUMS with no entry.
    if !resp.status().is_success() {
        return Err(ReleaseError::upstream(
            "SHA256SUMS fetch",
            &resp,
            spec.token_secret.as_deref(),
        ));
    }
    let body = resp.text().await.map_err(|source| ReleaseError::Request {
        stage: "SHA256SUMS read",
        source,
    })?;

    parse_sha256sums(&body, asset_name).ok_or_else(|| ReleaseError::ChecksumMissing {
        asset: asset_name.to_owned(),
    })
}

pub fn parse_sha256sums(body: &str, asset_name: &str) -> Option<String> {
    body.lines().find_map(|line| {
        let (digest, name) = line.split_once(char::is_whitespace)?;
        let name = name.trim_start_matches([' ', '*']);
        (name == asset_name && digest.len() == 64).then(|| digest.to_ascii_lowercase())
    })
}

async fn fetch_json<T: serde::de::DeserializeOwned>(
    http: &reqwest::Client,
    spec: &BridgeReleasesSpec,
    secrets: &Secrets,
    url: &str,
) -> Result<T, ReleaseError> {
    let resp = github(http, spec, secrets, url)?
        .send()
        .await
        .map_err(|source| ReleaseError::Request {
            stage: "github request",
            source,
        })?;
    if !resp.status().is_success() {
        tracing::debug!(url, status = %resp.status(), "github refused the release lookup");
        return Err(ReleaseError::upstream(
            "github request",
            &resp,
            spec.token_secret.as_deref(),
        ));
    }
    resp.json::<T>()
        .await
        .map_err(|source| ReleaseError::Request {
            stage: "github decode",
            source,
        })
}

pub(super) fn github(
    http: &reqwest::Client,
    spec: &BridgeReleasesSpec,
    secrets: &Secrets,
    url: &str,
) -> Result<reqwest::RequestBuilder, ReleaseError> {
    let mut req = http
        .get(url)
        // Why: GitHub rejects requests that send no User-Agent.
        .header(header::USER_AGENT, "systemprompt-gateway")
        .header("X-GitHub-Api-Version", "2022-11-28");
    if let Some(key) = spec.token_secret.as_deref() {
        req = req.bearer_auth(release_token(secrets, key)?);
    }
    Ok(req)
}

fn release_token(secrets: &Secrets, key: &str) -> Result<String, ReleaseError> {
    secrets
        .get(key)
        .cloned()
        .ok_or_else(|| ReleaseError::TokenNotConfigured {
            key: key.to_owned(),
        })
}
