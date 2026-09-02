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
use systemprompt_identifiers::JwtToken;
use systemprompt_loader::ServicesBootstrap;
use systemprompt_models::services::BridgeReleasesSpec;

mod github;

pub use self::github::parse_sha256sums;

use self::github::{asset_digest, github, resolve_release};

use super::messages::extract_credential;
use crate::services::middleware::JwtContextExtractor;


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
    let services = ServicesBootstrap::get().map_err(|e| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("Services config not ready: {e}"),
        )
    })?;
    services
        .gateway_config()
        .and_then(|g| g.bridge_releases.clone())
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "bridge releases are not configured on this gateway".to_owned(),
            )
        })
}
