//! Gateway HTTP client (`GatewayClient`) and supporting wire types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod auth;
pub mod errors;
mod fetch;
mod identity;
pub mod identity_source;
pub mod manifest;
pub mod manifest_version;
pub mod model_view;
pub mod types;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use systemprompt_identifiers::ValidatedUrl;

pub use errors::GatewayError;
pub use types::{BridgeOAuthClientResponse, HookTokenResponse, WhoamiResponse};

// Why: WSL2's localhost forwarder black-holes IPv6 SYNs and reqwest 0.12 lacks
// happy-eyeballs, so order IPv4 first.
#[derive(Debug)]
pub(crate) struct Ipv4FirstResolver;

impl Resolve for Ipv4FirstResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_owned();
        Box::pin(async move {
            let resolved = tokio::net::lookup_host((host.as_str(), 0)).await?;
            let mut addrs: Vec<SocketAddr> = resolved.collect();
            addrs.sort_by_key(SocketAddr::is_ipv6);
            let iter: Addrs = Box::new(addrs.into_iter());
            Ok(iter)
        })
    }
}

// Why: one pooled client per process, built by the composition root and
// cloned into every `GatewayClient`, so calls share connections without a
// process-global holding the pool.
#[must_use]
pub fn build_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .dns_resolver(Arc::new(Ipv4FirstResolver))
        .pool_max_idle_per_host(8)
        .tcp_nodelay(true)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

#[derive(Debug)]
pub struct GatewayClient {
    base_url: ValidatedUrl,
    http: reqwest::Client,
}

impl GatewayClient {
    #[must_use]
    pub const fn new(base_url: ValidatedUrl, http: reqwest::Client) -> Self {
        Self { base_url, http }
    }

    #[must_use]
    pub const fn base_url(&self) -> &ValidatedUrl {
        &self.base_url
    }

    #[must_use]
    pub fn base_url_str(&self) -> &str {
        self.base_url.as_str()
    }

    pub(super) const fn http(&self) -> &reqwest::Client {
        &self.http
    }

    pub(super) fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url.as_str().trim_end_matches('/'), path)
    }
}

pub(super) fn record_span(resp: &reqwest::Response, started: Instant) {
    let span = tracing::Span::current();
    span.record("status", resp.status().as_u16());
    span.record(
        "latency_ms",
        u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    );
}
