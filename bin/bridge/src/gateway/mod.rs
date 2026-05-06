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

use std::sync::OnceLock;
use std::time::{Duration, Instant};
use systemprompt_identifiers::ValidatedUrl;

pub use errors::GatewayError;
pub use types::{BridgeOAuthClientResponse, HookTokenResponse, WhoamiResponse};

static SHARED_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn shared_client() -> reqwest::Client {
    SHARED_CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .pool_max_idle_per_host(8)
                .tcp_nodelay(true)
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new())
        })
        .clone()
}

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
    pub fn base_url(&self) -> &ValidatedUrl {
        &self.base_url
    }

    #[must_use]
    pub fn base_url_str(&self) -> &str {
        self.base_url.as_str()
    }

    pub(super) fn http(&self) -> &reqwest::Client {
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
