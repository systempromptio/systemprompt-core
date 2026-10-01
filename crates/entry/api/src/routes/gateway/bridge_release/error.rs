//! Typed failures of the bridge release feed and their HTTP answer.
//!
//! Every bridge in the fleet reads this feed on a timer, so a resolution that
//! keeps failing is a fleet-wide update outage: an upstream failure answers
//! 502/503 through `ApiError`, which logs it at error with its full cause
//! chain while the body carries only the fixed public text. A configuration
//! answer (no build for the platform, no release, no asset) stays a 404.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use systemprompt_config::SecretsBootstrapError;
use systemprompt_loader::ConfigLoadError;
use systemprompt_models::api::ApiError;

use super::super::bridge_error::BridgeError;
use crate::error::ApiHttpError;

#[derive(Debug, thiserror::Error)]
pub enum ReleaseError {
    #[error(transparent)]
    Auth(Box<BridgeError>),
    #[error("services config not ready")]
    ServicesNotReady(#[source] ConfigLoadError),
    #[error("bridge releases are not configured on this gateway")]
    NotConfigured,
    #[error("no published build for platform {platform}")]
    UnknownPlatform { platform: String },
    #[error("no {prefix}* release found in {repo}")]
    NoRelease { prefix: String, repo: String },
    #[error("release {tag} has no asset {asset}")]
    MissingAsset { tag: String, asset: String },
    #[error("release {version} publishes no SHA256SUMS")]
    NoChecksums { version: String },
    #[error("SHA256SUMS has no entry for {asset}")]
    ChecksumMissing { asset: String },
    #[error("{stage} failed")]
    Request {
        stage: &'static str,
        #[source]
        source: reqwest::Error,
    },
    #[error("{stage} returned {status}")]
    UpstreamStatus {
        stage: &'static str,
        status: reqwest::StatusCode,
    },
    #[error("bridge release token secret unavailable")]
    SecretsUnavailable(#[source] SecretsBootstrapError),
    #[error("bridge release token secret {key} is not configured")]
    TokenNotConfigured { key: String },
}

impl ReleaseError {
    #[must_use]
    pub const fn status(&self) -> StatusCode {
        match self {
            Self::Auth(_) => StatusCode::UNAUTHORIZED,
            Self::NotConfigured
            | Self::UnknownPlatform { .. }
            | Self::NoRelease { .. }
            | Self::MissingAsset { .. } => StatusCode::NOT_FOUND,
            Self::NoChecksums { .. }
            | Self::ChecksumMissing { .. }
            | Self::Request { .. }
            | Self::UpstreamStatus { .. } => StatusCode::BAD_GATEWAY,
            Self::ServicesNotReady(_)
            | Self::SecretsUnavailable(_)
            | Self::TokenNotConfigured { .. } => StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    #[must_use]
    pub const fn error_key(&self) -> &'static str {
        match self {
            Self::Auth(_) => "invalid_credential",
            Self::NotConfigured => "bridge_releases_not_configured",
            Self::UnknownPlatform { .. } => "platform_not_published",
            Self::NoRelease { .. } => "release_not_found",
            Self::MissingAsset { .. } => "release_asset_not_found",
            Self::NoChecksums { .. } | Self::ChecksumMissing { .. } => {
                "release_checksum_unavailable"
            },
            Self::Request { .. } | Self::UpstreamStatus { .. } => "release_upstream_failed",
            Self::ServicesNotReady(_)
            | Self::SecretsUnavailable(_)
            | Self::TokenNotConfigured { .. } => "release_feed_not_ready",
        }
    }
}

impl From<BridgeError> for ReleaseError {
    fn from(err: BridgeError) -> Self {
        Self::Auth(Box::new(err))
    }
}

impl IntoResponse for ReleaseError {
    fn into_response(self) -> Response {
        let status = self.status();
        let key = self.error_key();
        let api = match self {
            Self::Auth(inner) => return ApiHttpError::from(*inner).into_response(),
            err if status == StatusCode::NOT_FOUND => {
                ApiError::not_found(err.to_string()).with_error_key(key)
            },
            err => ApiError::service_unavailable("Bridge release feed unavailable")
                .with_error_key(key)
                .with_source(err),
        };
        let mut response = api.into_response();
        // Why: `ErrorCode` has no 502, yet an upstream GitHub failure must still
        // read as a bad gateway to clients, proxies and the access log, so the
        // typed status is restored after the envelope renders.
        *response.status_mut() = status;
        response
    }
}
