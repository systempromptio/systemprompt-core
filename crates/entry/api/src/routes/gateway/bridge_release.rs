//! `GET /v1/bridge/latest` and `GET /v1/bridge/download/{platform}` — the feed
//! the desktop bridge's self-updater reads.
//!
//! Release assets live in a private repository that the bridge has no
//! credential for, so the gateway resolves the newest `bridge-v*` release and
//! proxies the bytes. Resolution happening here is also what lets an operator
//! pin or stage a rollout without shipping a new client.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Json;
use axum::body::Body;
use axum::extract::{Path, Query};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::JwtToken;
use systemprompt_models::profile::BridgeReleasesSpec;

use super::messages::extract_credential;
use crate::services::middleware::JwtContextExtractor;

// Why: GitHub caps a release listing at 100, and bridge releases are infrequent
// enough that the newest matching tag is always well inside the first page.
const RELEASE_PAGE_SIZE: u8 = 30;

#[derive(Debug, Deserialize)]
pub struct LatestQuery {
    pub platform: String,
}

/// Mirrors `ReleaseManifest` in the bridge's gateway client.
///
/// Keep the two in lockstep: this is a wire contract with an already-shipped
/// binary, so a renamed field silently breaks every bridge in the field.
#[derive(Debug, Serialize)]
pub struct ReleaseManifest {
    pub version: String,
    pub sha256: String,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    url: String,
    #[serde(default)]
    size: u64,
}

pub async fn latest(
    jwt_extractor: Arc<JwtContextExtractor>,
    headers: HeaderMap,
    Query(query): Query<LatestQuery>,
) -> Result<Json<ReleaseManifest>, (StatusCode, String)> {
    authenticate(&jwt_extractor, &headers).await?;
    let spec = releases_spec()?;

    let asset_name = spec.assets.get(&query.platform).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("no published build for platform {}", query.platform),
        )
    })?;

    let release = resolve_release(&spec).await?;
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

    let version = release
        .tag_name
        .strip_prefix(&spec.tag_prefix)
        .unwrap_or(&release.tag_name)
        .to_owned();
    let sha256 = asset_digest(&spec, &release, asset_name).await?;

    Ok(Json(ReleaseManifest {
        version,
        sha256,
        size: asset.size,
        notes_url: release.html_url.clone(),
    }))
}

pub async fn download(
    jwt_extractor: Arc<JwtContextExtractor>,
    headers: HeaderMap,
    Path(platform): Path<String>,
) -> Result<Response, (StatusCode, String)> {
    authenticate(&jwt_extractor, &headers).await?;
    let spec = releases_spec()?;

    let asset_name = spec.assets.get(&platform).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("no published build for platform {platform}"),
        )
    })?;

    let release = resolve_release(&spec).await?;
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

    // Why: `Accept: application/octet-stream` on the asset *API* url is what
    // makes GitHub serve bytes — without it the response is JSON metadata.
    let upstream = github(&spec, &asset.url)
        .header(header::ACCEPT, "application/octet-stream")
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("asset fetch failed: {e}")))?;

    if !upstream.status().is_success() {
        return Err((
            StatusCode::BAD_GATEWAY,
            format!("asset fetch returned {}", upstream.status()),
        ));
    }

    // Why: streamed rather than buffered — these are tens of megabytes and the
    // gateway must not hold one per updating client in memory.
    let body = Body::from_stream(upstream.bytes_stream());
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{asset_name}\""),
            ),
        ],
        body,
    )
        .into_response())
}

async fn authenticate(
    jwt_extractor: &Arc<JwtContextExtractor>,
    headers: &HeaderMap,
) -> Result<(), (StatusCode, String)> {
    let credential = extract_credential(headers).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "Missing Authorization or x-api-key credential".to_owned(),
        )
    })?;
    jwt_extractor
        .decode_for_gateway(&JwtToken::new(credential))
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;
    Ok(())
}

fn releases_spec() -> Result<BridgeReleasesSpec, (StatusCode, String)> {
    let profile = ProfileBootstrap::get().map_err(|e| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("Profile not ready: {e}"),
        )
    })?;
    profile
        .gateway
        .as_ref()
        .and_then(systemprompt_models::profile::GatewayState::resolved)
        .and_then(|g| g.bridge_releases.clone())
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "bridge releases are not configured on this gateway".to_owned(),
            )
        })
}

// Why: bridge releases are tagged separately from the server's, so an
// unfiltered "latest release" would pick the wrong one.
async fn resolve_release(spec: &BridgeReleasesSpec) -> Result<GhRelease, (StatusCode, String)> {
    if let Some(pinned) = spec.pinned_version.as_deref() {
        let tag = format!("{}{pinned}", spec.tag_prefix);
        let url = format!(
            "https://api.github.com/repos/{}/releases/tags/{tag}",
            spec.repo
        );
        return fetch_json::<GhRelease>(spec, &url).await;
    }

    let url = format!(
        "https://api.github.com/repos/{}/releases?per_page={RELEASE_PAGE_SIZE}",
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
async fn asset_digest(
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

fn github(spec: &BridgeReleasesSpec, url: &str) -> reqwest::RequestBuilder {
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
