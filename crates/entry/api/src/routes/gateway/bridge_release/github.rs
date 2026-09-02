//! GitHub release resolution for the bridge feed: picking the newest matching
//! `bridge-v*` release and reading its signed SHA256SUMS entry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::{StatusCode, header};
use serde::Deserialize;
use systemprompt_models::profile::BridgeReleasesSpec;

// Why: GitHub caps a release listing at 100, and bridge releases are infrequent
// enough that the newest matching tag is always well inside the first page.
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

// Why: bridge releases are tagged separately from the server's, so an
// unfiltered "latest release" would pick the wrong one.
pub(super) async fn resolve_release(
    spec: &BridgeReleasesSpec,
) -> Result<GhRelease, (StatusCode, String)> {
    if let Some(pinned) = spec.pinned_version.as_deref() {
        let tag = format!("{}{pinned}", spec.tag_prefix);
        let url = format!(
            "{}/repos/{}/releases/tags/{tag}",
            spec.api_base(),
            spec.repo
        );
        return fetch_json::<GhRelease>(spec, &url).await;
    }

    let url = format!(
        "{}/repos/{}/releases?per_page={RELEASE_PAGE_SIZE}",
        spec.api_base(),
        spec.repo
    );
    let releases = fetch_json::<Vec<GhRelease>>(spec, &url).await?;
    releases
        .into_iter()
        .find(|r| !r.draft && !r.prerelease && r.tag_name.starts_with(&spec.tag_prefix))
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("no {}* release found in {}", spec.tag_prefix, spec.repo),
            )
        })
}

// Why: taken from the release's cosign-signed SHA256SUMS rather than computed
// here, so the digest the updater enforces is the one signed at publish time.
pub(super) async fn asset_digest(
    spec: &BridgeReleasesSpec,
    release: &GhRelease,
    asset_name: &str,
) -> Result<String, (StatusCode, String)> {
    let sums = release
        .assets
        .iter()
        .find(|a| a.name == "SHA256SUMS")
        .ok_or_else(|| {
            (
                StatusCode::BAD_GATEWAY,
                format!("release {} publishes no SHA256SUMS", release.tag_name),
            )
        })?;

    let body = github(spec, &sums.url)
        .header(header::ACCEPT, "application/octet-stream")
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("SHA256SUMS fetch: {e}")))?
        .text()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("SHA256SUMS read: {e}")))?;

    parse_sha256sums(&body, asset_name).ok_or_else(|| {
        (
            StatusCode::BAD_GATEWAY,
            format!("SHA256SUMS has no entry for {asset_name}"),
        )
    })
}

// Why: `sha256sum` output is `<hex>␠[␠*]<name>` — the second space or the `*`
// marks binary mode, and both forms appear in the files this reads.
pub fn parse_sha256sums(body: &str, asset_name: &str) -> Option<String> {
    body.lines().find_map(|line| {
        let (digest, name) = line.split_once(char::is_whitespace)?;
        let name = name.trim_start_matches([' ', '*']);
        (name == asset_name && digest.len() == 64).then(|| digest.to_ascii_lowercase())
    })
}

async fn fetch_json<T: serde::de::DeserializeOwned>(
    spec: &BridgeReleasesSpec,
    url: &str,
) -> Result<T, (StatusCode, String)> {
    let resp = github(spec, url)
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("github request: {e}")))?;
    if !resp.status().is_success() {
        return Err((
            StatusCode::BAD_GATEWAY,
            format!("github returned {} for {url}", resp.status()),
        ));
    }
    resp.json::<T>()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("github decode: {e}")))
}

pub(super) fn github(spec: &BridgeReleasesSpec, url: &str) -> reqwest::RequestBuilder {
    let mut req = reqwest::Client::new()
        .get(url)
        // Why: GitHub rejects requests that send no User-Agent.
        .header(header::USER_AGENT, "systemprompt-gateway")
        .header("X-GitHub-Api-Version", "2022-11-28");
    if let Some(token) = spec
        .token_env
        .as_deref()
        .and_then(|k| std::env::var(k).ok())
    {
        req = req.bearer_auth(token);
    }
    req
}
