//! Gateway HTTP client and supporting wire types.
//!
//! `GatewayClient` is the single ingress for bridge → gateway traffic. The
//! struct, shared `reqwest::Client` pool, and URL/tracing helpers live here;
//! read-only fetches are in `fetch`, auth-mutating exchanges are in `auth`.
//! Wire DTOs live in `types`, error taxonomy in `errors`. Manifest parsing
//! and signature verification stay in the `manifest*` siblings.

mod auth;
pub mod errors;
mod fetch;
pub mod manifest;
pub mod manifest_version;
pub mod types;

use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use systemprompt_identifiers::ValidatedUrl;

pub use errors::GatewayError;
pub use types::{BridgeOAuthClientResponse, HookTokenResponse, WhoamiResponse};

// Why: `localhost` resolves to IPv6 `::1` before `127.0.0.1`, but the WSL2
// localhost forwarder relays only IPv4 and black-holes IPv6 SYNs to its
// forwarded ports, so a sequential connect to `::1` stalls the full connect
// timeout before falling back. reqwest 0.12 has no happy-eyeballs option, so
// we install a resolver that returns IPv4 addresses first: loopback (and any
// dual-stack host) connects immediately, with IPv6 retained as a fallback.
// `pub(crate)` so the proxy's upstream client
// (`proxy::server::build_upstream_client`) can install the same resolver — it
// forwards Cowork's MCP + inference to the gateway, so a user-entered
// `localhost` gateway URL would otherwise stall every proxied call on the IPv6
// connect timeout.
#[derive(Debug)]
pub(crate) struct Ipv4FirstResolver;

impl Resolve for Ipv4FirstResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_owned();
        Box::pin(async move {
            let resolved = tokio::net::lookup_host((host.as_str(), 0)).await?;
            let mut addrs: Vec<SocketAddr> = resolved.collect();
            // false (IPv4) sorts before true (IPv6); stable within a family.
            addrs.sort_by_key(SocketAddr::is_ipv6);
            let iter: Addrs = Box::new(addrs.into_iter());
            Ok(iter)
        })
    }
}

static SHARED_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn shared_client() -> reqwest::Client {
    SHARED_CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .dns_resolver(Arc::new(Ipv4FirstResolver))
                .pool_max_idle_per_host(8)
                .tcp_nodelay(true)
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new())
        })
        .clone()
}

#[derive(Debug)]
pub struct GatewayClient {
    base_url: ValidatedUrl,
    http: reqwest::Client,
}

impl GatewayClient {
    #[must_use]
    pub fn new(base_url: ValidatedUrl) -> Self {
        Self {
            base_url,
            http: shared_client(),
        }
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
