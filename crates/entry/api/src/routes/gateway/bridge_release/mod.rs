//! `GET /v1/bridge/latest` and `GET /v1/bridge/download/{platform}` — the feed
//! the desktop bridge's self-updater reads.
//!
//! Release assets live in a private repository that the bridge has no
//! credential for, so the gateway resolves the newest `bridge-v*` release and
//! proxies the bytes. Resolution happening here is also what lets an operator
//! pin or stage a rollout without shipping a new client.
//!
//! Every bridge in the fleet reads this feed on a timer, so resolution is
//! cached per platform for [`CACHE_TTL`] against a shared HTTP client: GitHub
//! sees a couple of dozen calls an hour no matter how large the fleet. A
//! resolution that fails while a previous answer is still held serves that
//! answer — a GitHub blip must not turn into a failed update check for
//! everyone at once.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::body::Body;
use axum::extract::{Path, Query};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::JwtToken;
use systemprompt_loader::ServicesBootstrap;
use systemprompt_models::services::BridgeReleasesSpec;

mod feed;
mod github;

pub use self::feed::{ReleaseFeed, ResolvedAsset, ResolvedRelease};
pub use self::github::parse_sha256sums;

use self::github::github;

use super::messages::extract_credential;
use crate::services::middleware::JwtContextExtractor;

pub const CACHE_TTL: Duration = Duration::from_secs(300);

#[derive(Debug, Deserialize)]
pub struct LatestQuery {
    pub platform: String,
}

/// Mirrors `ReleaseManifest` in the bridge's gateway client.
///
/// Keep the two in lockstep: this is a wire contract with an already-shipped
/// binary, so a renamed field silently breaks every bridge in the field.
#[derive(Debug, Clone, Serialize)]
pub struct ReleaseManifest {
    pub version: String,
    pub sha256: String,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes_url: Option<String>,
}

pub async fn latest(
    jwt_extractor: Arc<JwtContextExtractor>,
    feed: Arc<ReleaseFeed>,
    headers: HeaderMap,
    Query(query): Query<LatestQuery>,
) -> Result<Json<ReleaseManifest>, (StatusCode, String)> {
    authenticate(&jwt_extractor, &headers).await?;
    let spec = releases_spec()?;
    let resolved = feed.resolve(&spec, &query.platform).await?;
    Ok(Json(resolved.manifest))
}

pub async fn download(
    jwt_extractor: Arc<JwtContextExtractor>,
    feed: Arc<ReleaseFeed>,
    headers: HeaderMap,
    Path(platform): Path<String>,
) -> Result<Response, (StatusCode, String)> {
    authenticate(&jwt_extractor, &headers).await?;
    let spec = releases_spec()?;
    let asset = feed.resolve_asset(&spec, &platform).await?;

    // Why: GitHub's asset API returns JSON metadata unless Accept is
    // application/octet-stream.
    let upstream = github(feed.http(), &spec, &asset.url)
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

    let body = Body::from_stream(upstream.bytes_stream());
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", asset.name),
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
