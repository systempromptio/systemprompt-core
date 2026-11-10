//! Construction helpers the release feed shares: the timed HTTP client, the
//! server's own version ceiling and the configured-platform check.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_manifest::services::BridgeReleasesSpec;
use systemprompt_models::net::{HTTP_CONNECT_TIMEOUT, HTTP_DEFAULT_TIMEOUT};

use systemprompt_models::bridge::gateway::ReleaseManifest;

use super::error::ReleaseError;

/// One platform's release as the feed resolved it: what `/latest` answers with
/// plus the asset `/download` streams.
#[derive(Debug, Clone)]
pub struct ResolvedRelease {
    pub manifest: ReleaseManifest,
    pub asset: ResolvedAsset,
}

/// Where a platform's asset lives, without the digest.
///
/// Separate because `/download` proxies bytes and never needs `SHA256SUMS`;
/// making it wait on that fetch would turn a missing checksum file into a
/// failed download of a binary that is right there.
#[derive(Debug, Clone)]
pub struct ResolvedAsset {
    pub version: String,
    pub size: u64,
    pub notes_url: Option<String>,
    pub name: String,
    pub url: String,
    pub(super) sums_url: Option<String>,
}

fn pkg_component(digits: &str) -> u64 {
    digits.parse().unwrap_or(0)
}

pub(super) fn server_version() -> semver::Version {
    semver::Version::new(
        pkg_component(env!("CARGO_PKG_VERSION_MAJOR")),
        pkg_component(env!("CARGO_PKG_VERSION_MINOR")),
        pkg_component(env!("CARGO_PKG_VERSION_PATCH")),
    )
}

pub(super) fn http_client() -> Result<reqwest::Client, ReleaseError> {
    reqwest::Client::builder()
        .connect_timeout(HTTP_CONNECT_TIMEOUT)
        .read_timeout(HTTP_DEFAULT_TIMEOUT)
        .build()
        .map_err(|source| ReleaseError::Request {
            stage: "http client build",
            source,
        })
}

pub(super) fn known_platform<'a>(
    spec: &'a BridgeReleasesSpec,
    platform: &str,
) -> Result<&'a String, ReleaseError> {
    spec.assets
        .get(platform)
        .ok_or_else(|| ReleaseError::UnknownPlatform {
            platform: platform.to_owned(),
        })
}
