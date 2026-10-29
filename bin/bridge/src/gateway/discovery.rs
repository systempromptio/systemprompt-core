//! The instance discovery client: `GET /.well-known/systemprompt-bridge`.
//!
//! Discovery tells a bridge where an instance mounts its sign-in and identity
//! routes, how it presents itself and which hosts it enables, before any
//! token exists. It carries no trust material. The document is cached per
//! gateway profile, stamped with the gateway that served it; a cache for
//! another gateway is never read. A gateway that does not serve discovery
//! (404) or cannot be reached leaves the compiled brand values in effect.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ValidatedUrl;
use systemprompt_models::ApiPaths;
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::bridge::instance::BridgeDiscovery;

use crate::config::gateways::{self, ActiveProfile, CachedDiscovery, ProfileError};

#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("discovery request to {url}: {source}")]
    Network {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("discovery at {url} answered {status}")]
    Status {
        url: String,
        status: reqwest::StatusCode,
    },
    #[error("discovery at {url} is not a bridge discovery document: {source}")]
    Parse {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("discovery at {url} names `{path}`, which is not a path on the gateway")]
    ForeignPath { url: String, path: String },
    #[error(transparent)]
    Profile(#[from] ProfileError),
    #[error(transparent)]
    Config(#[from] crate::config::ConfigReadError),
}

/// What a refresh found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryOutcome {
    Fetched(Box<BridgeDiscovery>),
    NotServed,
}

#[must_use]
pub fn discovery_url(base: &ValidatedUrl) -> String {
    format!(
        "{}{}",
        base.as_str().trim_end_matches('/'),
        ApiPaths::WELLKNOWN_BRIDGE
    )
}

pub async fn fetch(
    http: &reqwest::Client,
    base: &ValidatedUrl,
) -> Result<Option<BridgeDiscovery>, DiscoveryError> {
    let url = discovery_url(base);
    let response = http
        .get(&url)
        .send()
        .await
        .map_err(|source| DiscoveryError::Network {
            url: url.clone(),
            source,
        })?;
    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !status.is_success() {
        return Err(DiscoveryError::Status { url, status });
    }
    let document =
        response
            .json::<BridgeDiscovery>()
            .await
            .map_err(|source| DiscoveryError::Parse {
                url: url.clone(),
                source,
            })?;
    for path in [&document.device_link_path, &document.identity_path] {
        if !is_gateway_path(path) {
            return Err(DiscoveryError::ForeignPath {
                url,
                path: path.clone(),
            });
        }
    }
    Ok(Some(document))
}

// Why: the paths are appended to the gateway origin, and the identity request
// carries the bearer token, so anything that could re-point the authority
// (`//host`, `@host`, a backslash) is refused.
#[must_use]
pub fn is_gateway_path(path: &str) -> bool {
    path.starts_with('/')
        && !path.starts_with("//")
        && !path
            .chars()
            .any(|c| c == '\\' || c == '@' || c.is_whitespace() || c.is_control())
}

pub async fn refresh_profile(
    http: &reqwest::Client,
    profile: &ActiveProfile,
) -> Result<DiscoveryOutcome, DiscoveryError> {
    let cfg = crate::config::read_file(&profile.config_file)?;
    let gateway = crate::config::gateway_url_or_default(&cfg);
    refresh_into(http, profile, gateway).await
}

pub async fn refresh_for(
    http: &reqwest::Client,
    gateway: &ValidatedUrl,
) -> Result<DiscoveryOutcome, DiscoveryError> {
    let profile = gateways::active()?;
    refresh_into(http, &profile, gateway.clone()).await
}

async fn refresh_into(
    http: &reqwest::Client,
    profile: &ActiveProfile,
    gateway: ValidatedUrl,
) -> Result<DiscoveryOutcome, DiscoveryError> {
    let Some(document) = fetch(http, &gateway).await? else {
        if gateways::read_discovery(profile)
            .ok()
            .flatten()
            .is_some_and(|c| crate::config::same_origin(&c.gateway, &gateway))
        {
            gateways::forget_discovery(profile)?;
        }
        return Ok(DiscoveryOutcome::NotServed);
    };
    gateways::write_discovery(
        profile,
        &CachedDiscovery {
            gateway,
            document: document.clone(),
        },
    )?;
    Ok(DiscoveryOutcome::Fetched(Box::new(document)))
}

pub async fn refresh_active(http: &reqwest::Client) -> Result<DiscoveryOutcome, DiscoveryError> {
    let profile = gateways::active()?;
    refresh_profile(http, &profile).await
}

pub async fn refresh_logged(http: &reqwest::Client, gateway: &ValidatedUrl) {
    match refresh_for(http, gateway).await {
        Ok(DiscoveryOutcome::Fetched(_)) => {
            tracing::debug!(%gateway, "instance discovery refreshed");
        },
        Ok(DiscoveryOutcome::NotServed) => {
            tracing::info!(%gateway, "gateway serves no bridge discovery; using the built-in routes");
        },
        Err(error) => {
            tracing::warn!(%error, "instance discovery unavailable; using cached or built-in routes");
        },
    }
}

#[must_use]
pub fn cached() -> Option<BridgeDiscovery> {
    let profile = gateways::active().ok()?;
    let cfg = crate::config::read_file(&profile.config_file).ok()?;
    cached_in(&profile, &crate::config::gateway_url_or_default(&cfg))
}

#[must_use]
pub fn cached_for(gateway: &ValidatedUrl) -> Option<BridgeDiscovery> {
    cached_in(&gateways::active().ok()?, gateway)
}

fn cached_in(profile: &ActiveProfile, gateway: &ValidatedUrl) -> Option<BridgeDiscovery> {
    match gateways::read_discovery(profile) {
        Ok(Some(cached)) if crate::config::same_origin(&cached.gateway, gateway) => {
            Some(cached.document)
        },
        Ok(_) => None,
        Err(error) => {
            tracing::warn!(%error, "cached instance discovery unreadable; using built-in routes");
            None
        },
    }
}

#[must_use]
pub fn device_link_path_for(gateway: &ValidatedUrl) -> String {
    cached_for(gateway).map_or_else(
        || crate::brand::brand().device_link_path.to_owned(),
        |d| d.device_link_path,
    )
}

#[must_use]
pub fn whoami_path() -> String {
    cached().map_or_else(|| ApiPaths::BRIDGE_WHOAMI.to_owned(), |d| d.identity_path)
}

#[must_use]
pub fn enabled_hosts() -> Option<Vec<HostKind>> {
    cached().map(|d| d.enabled_hosts)
}
