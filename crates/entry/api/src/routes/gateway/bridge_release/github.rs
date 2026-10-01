//! GitHub release resolution for the bridge feed: picking the newest matching
//! `bridge-v*` release and reading its signed SHA256SUMS entry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::header;
use serde::Deserialize;
use systemprompt_models::services::BridgeReleasesSpec;

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
) -> Result<GhRelease, ReleaseError> {
    if let Some(pinned) = spec.pinned_version.as_deref() {
        let tag = format!("{}{pinned}", spec.tag_prefix);
        let url = format!(
            "{}/repos/{}/releases/tags/{tag}",
            spec.api_base(),
            spec.repo
        );
        return fetch_json::<GhRelease>(http, spec, &url).await;
    }

    let url = format!(
        "{}/repos/{}/releases?per_page={RELEASE_PAGE_SIZE}",
        spec.api_base(),
        spec.repo
    );
    let releases = fetch_json::<Vec<GhRelease>>(http, spec, &url).await?;
    releases
        .into_iter()
        .find(|r| !r.draft && !r.prerelease && r.tag_name.starts_with(&spec.tag_prefix))
        .ok_or_else(|| ReleaseError::NoRelease {
            prefix: spec.tag_prefix.clone(),
            repo: spec.repo.clone(),
        })
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
    sums_url: &str,
    asset_name: &str,
) -> Result<String, ReleaseError> {
    let body = github(http, spec, sums_url)?
        .header(header::ACCEPT, "application/octet-stream")
        .send()
        .await
        .map_err(|source| ReleaseError::Request {
            stage: "SHA256SUMS fetch",
            source,
        })?
        .text()
        .await
        .map_err(|source| ReleaseError::Request {
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
    url: &str,
) -> Result<T, ReleaseError> {
    let resp = github(http, spec, url)?
        .send()
        .await
        .map_err(|source| ReleaseError::Request {
            stage: "github request",
            source,
        })?;
    if !resp.status().is_success() {
        tracing::debug!(url, status = %resp.status(), "github refused the release lookup");
        return Err(ReleaseError::UpstreamStatus {
            stage: "github request",
            status: resp.status(),
        });
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
    url: &str,
) -> Result<reqwest::RequestBuilder, ReleaseError> {
    let mut req = http
        .get(url)
        // Why: GitHub rejects requests that send no User-Agent.
        .header(header::USER_AGENT, "systemprompt-gateway")
        .header("X-GitHub-Api-Version", "2022-11-28");
    if let Some(key) = spec.token_secret.as_deref() {
        req = req.bearer_auth(release_token(key)?);
    }
    Ok(req)
}

fn release_token(key: &str) -> Result<String, ReleaseError> {
    let secrets =
        systemprompt_config::SecretsBootstrap::get().map_err(ReleaseError::SecretsUnavailable)?;
    secrets
        .get(key)
        .cloned()
        .ok_or_else(|| ReleaseError::TokenNotConfigured {
            key: key.to_owned(),
        })
}
