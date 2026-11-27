//! `GET /v1/bridge/latest` and `GET /v1/bridge/download/{platform}` — the feed
//! the desktop bridge's self-updater reads.
//!
//! Release assets live in a private repository that the bridge has no
//! credential for, so the gateway resolves the newest `bridge-v*` release this
//! server supports (major.minor no higher than its own version) and proxies
//! the bytes. Resolution happening here is also what lets an operator
//! pin or stage a rollout without shipping a new client.
//!
//! Every bridge in the fleet reads this feed on a timer, so resolution is
//! cached per platform for [`CACHE_TTL`] against a shared HTTP client with
//! connect and read timeouts; misses are single-flight and a `refresh=true`
//! read is honoured at most once per [`REFRESH_INTERVAL`] per platform. A
//! resolution that fails while a previous answer is still held serves that
//! answer — a GitHub blip must not turn into a failed update check for
//! everyone at once.
//!
//! A platform is a key of `bridge_releases.assets`, so the instance's Android
//! app is the `android` key naming the signed APK of the same release; it is
//! served as an Android package and `/latest` carries its checksum. A phone
//! that has no app yet holds no credential, so the one platform the discovery
//! document offers for download (`bridge_instance.clients.android`) is served
//! to an anonymous caller; every other platform requires the bridge
//! credential. An anonymous caller cannot force a refresh.
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
use serde::Deserialize;
use systemprompt_manifest::services::{BridgeReleasesSpec, ServicesView};
use systemprompt_models::bridge::gateway::ReleaseManifest;

mod cache;
mod error;
mod feed;
mod github;
mod upstream;

pub use self::error::ReleaseError;
pub use self::feed::{REFRESH_INTERVAL, ReleaseFeed, ResolvedAsset, ResolvedRelease};
pub use self::github::parse_sha256sums;
pub use self::upstream::{ANDROID_PACKAGE_CONTENT_TYPE, asset_content_type};

use self::github::github;

use super::BridgeRouteState;
use super::error::authenticate_bridge;

pub const CACHE_TTL: Duration = Duration::from_secs(300);

#[derive(Debug, Deserialize)]
pub struct LatestQuery {
    pub platform: String,
    #[serde(default)]
    pub refresh: bool,
}

pub async fn latest(
    state: BridgeRouteState,
    feed: Arc<ReleaseFeed>,
    headers: HeaderMap,
    Query(query): Query<LatestQuery>,
) -> Result<Json<ReleaseManifest>, ReleaseError> {
    let Admitted { spec, caller } = admit(&state, &headers, &query.platform).await?;
    let secrets = &*state.releases;
    let resolved = if query.refresh && caller == Caller::Bridge {
        feed.resolve_fresh(&spec, secrets, &query.platform).await?
    } else {
        feed.resolve(&spec, secrets, &query.platform).await?
    };
    Ok(Json(resolved.manifest))
}

pub async fn download(
    state: BridgeRouteState,
    feed: Arc<ReleaseFeed>,
    headers: HeaderMap,
    Path(platform): Path<String>,
) -> Result<Response, ReleaseError> {
    let Admitted { spec, .. } = admit(&state, &headers, &platform).await?;
    let secrets = &*state.releases;
    let asset = feed.resolve_asset(&spec, secrets, &platform).await?;

    // Why: GitHub's asset API returns JSON metadata unless Accept is
    // application/octet-stream.
    let upstream = github(feed.http(), &spec, secrets, &asset.url)?
        .header(header::ACCEPT, "application/octet-stream")
        .send()
        .await
        .map_err(|source| ReleaseError::Request {
            stage: "asset fetch",
            source,
        })?;

    if !upstream.status().is_success() {
        return Err(ReleaseError::upstream(
            "asset fetch",
            &upstream,
            spec.token_secret.as_deref(),
        ));
    }

    let body = Body::from_stream(upstream.bytes_stream());
    Ok((
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                asset_content_type(&platform).to_owned(),
            ),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", asset.name),
            ),
        ],
        body,
    )
        .into_response())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Caller {
    Bridge,
    Anonymous,
}

struct Admitted {
    spec: BridgeReleasesSpec,
    caller: Caller,
}

async fn admit(
    state: &BridgeRouteState,
    headers: &HeaderMap,
    platform: &str,
) -> Result<Admitted, ReleaseError> {
    let services = state.host.services.view();
    let caller = if serves_anonymously(&services, platform) {
        Caller::Anonymous
    } else {
        authenticate_bridge(&state.authenticator, headers).await?;
        Caller::Bridge
    };
    Ok(Admitted {
        spec: releases_spec(&services)?,
        caller,
    })
}

fn serves_anonymously(services: &ServicesView, platform: &str) -> bool {
    services
        .config()
        .gateway_config()
        .is_some_and(|gateway| gateway.serves_app_download(platform))
}

fn releases_spec(services: &ServicesView) -> Result<BridgeReleasesSpec, ReleaseError> {
    services
        .config()
        .gateway_config()
        .and_then(|g| g.bridge_releases.clone())
        .ok_or(ReleaseError::NotConfigured)
}
