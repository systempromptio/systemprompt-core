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

use axum::body::Body;
use axum::extract::{Path, Query};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use systemprompt_manifest::services::{BridgeReleasesSpec, ServicesView};
use systemprompt_models::ApiPaths;
use systemprompt_models::bridge::gateway::ReleaseManifest;
use systemprompt_runtime::AppContext;

mod error;
mod feed;
mod github;

pub use self::error::ReleaseError;
pub use self::feed::{ReleaseFeed, ResolvedAsset, ResolvedRelease};
pub use self::github::parse_sha256sums;

use self::github::github;

use super::bridge_error::authenticate_bridge;
use crate::services::middleware::JwtContextExtractor;

pub const CACHE_TTL: Duration = Duration::from_secs(300);

#[derive(Debug, Deserialize)]
pub struct LatestQuery {
    pub platform: String,
    #[serde(default)]
    pub refresh: bool,
}

pub async fn latest(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: &AppContext,
    feed: Arc<ReleaseFeed>,
    headers: HeaderMap,
    Query(query): Query<LatestQuery>,
) -> Result<Json<ReleaseManifest>, ReleaseError> {
    authenticate_bridge(&jwt_extractor, &headers).await?;
    let spec = releases_spec(&ctx.services_state().view())?;
    let secrets = ctx.secrets().get();
    let resolved = if query.refresh {
        feed.resolve_fresh(&spec, &secrets, &query.platform).await?
    } else {
        feed.resolve(&spec, &secrets, &query.platform).await?
    };
    Ok(Json(resolved.manifest))
}

pub async fn download(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: &AppContext,
    feed: Arc<ReleaseFeed>,
    headers: HeaderMap,
    Path(platform): Path<String>,
) -> Result<Response, ReleaseError> {
    authenticate_bridge(&jwt_extractor, &headers).await?;
    let spec = releases_spec(&ctx.services_state().view())?;
    let secrets = ctx.secrets().get();
    let asset = feed.resolve_asset(&spec, &secrets, &platform).await?;

    // Why: GitHub's asset API returns JSON metadata unless Accept is
    // application/octet-stream.
    let upstream = github(feed.http(), &spec, &secrets, &asset.url)?
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

fn releases_spec(services: &ServicesView) -> Result<BridgeReleasesSpec, ReleaseError> {
    services
        .config()
        .gateway_config()
        .and_then(|g| g.bridge_releases.clone())
        .ok_or(ReleaseError::NotConfigured)
}

pub(in crate::routes::gateway) fn routes(
    ctx: &AppContext,
    jwt_extractor: &Arc<JwtContextExtractor>,
) -> Router {
    let ctx_latest = ctx.clone();
    let ctx_download = ctx.clone();
    let jwt_latest = Arc::clone(jwt_extractor);
    let jwt_download = Arc::clone(jwt_extractor);
    let feed = Arc::new(ReleaseFeed::default());
    let feed_latest = Arc::clone(&feed);
    let feed_download = feed;

    Router::new()
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_LATEST),
            get(move |headers, query| {
                let extractor = Arc::clone(&jwt_latest);
                let feed = Arc::clone(&feed_latest);
                let ctx = ctx_latest.clone();
                async move { latest(extractor, &ctx, feed, headers, query).await }
            }),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_DOWNLOAD_ROUTE),
            get(move |headers, path| {
                let extractor = Arc::clone(&jwt_download);
                let feed = Arc::clone(&feed_download);
                let ctx = ctx_download.clone();
                async move { download(extractor, &ctx, feed, headers, path).await }
            }),
        )
}
