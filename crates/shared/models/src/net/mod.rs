//! Shared network timeout constants and outbound-URL validation.
//!
//! Centralised [`Duration`] values for HTTP client configuration, TCP
//! readiness probes, and long-poll image generation, so every caller
//! uses the same tuned timeouts, the [`BUFFERED_BODY_LIMIT_BYTES`] cap on a
//! buffered proxy request body, plus [`validate_outbound_url`] — the
//! single parse-time SSRF guard applied to every outbound destination.
//!
//! Parse-time validation is a pre-filter, not the enforcement point: a
//! hostname carries no address, so `systemprompt_client::guarded` installs a
//! DNS resolver that re-applies [`is_blocked_ip`] to every address a name
//! resolves to, on the initial request and on every redirect hop. Reach for
//! `systemprompt_client::guarded_client` rather than
//! `reqwest::Client::builder()` for any destination a caller can influence.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod blocklist;
mod outbound;
mod proxy;

pub use blocklist::is_blocked_ip;
pub use outbound::{
    LoopbackPolicy, OutboundUrlError, PinnedAddress, is_loopback_host, pin_resolved,
    validate_outbound_url, validate_outbound_url_with_trust,
};
pub use proxy::{ProxyEnvironment, proxy_host};

use std::time::Duration;

pub const BUFFERED_BODY_LIMIT_BYTES: usize = 8 * 1024 * 1024;
pub const INFERENCE_BODY_LIMIT_BYTES: usize = 32 * 1024 * 1024;

#[must_use]
pub fn gateway_request_body_limit(path: &str) -> usize {
    match path {
        "/v1/messages" | "/v1/messages/count_tokens" | "/v1/chat/completions" | "/v1/responses" => {
            INFERENCE_BODY_LIMIT_BYTES
        },
        _ => BUFFERED_BODY_LIMIT_BYTES,
    }
}

pub const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

pub const HTTP_DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

pub const HTTP_HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(5);

pub const HTTP_AUTH_VERIFY_TIMEOUT: Duration = Duration::from_secs(10);

pub const HTTP_SYNC_DEPLOY_TIMEOUT: Duration = Duration::from_secs(60);

pub const HTTP_STREAM_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

pub const HTTP_KEEPALIVE: Duration = Duration::from_secs(60);

pub const HTTP_POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(90);

pub const AGENT_MONITOR_TCP_TIMEOUT: Duration = Duration::from_secs(15);

pub const AGENT_READINESS_TCP_TIMEOUT: Duration = Duration::from_secs(2);

pub const IMAGE_GEN_LONG_POLL_TIMEOUT: Duration = Duration::from_secs(300);

pub const IMAGE_GEN_OPENAI_TIMEOUT: Duration = Duration::from_secs(120);

pub const AI_PROVIDER_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

pub const MCP_TOOL_EXECUTION_TIMEOUT: Duration = Duration::from_secs(30);

pub const TRUSTED_HTTP_HOSTS_ENV: &str = "SYSTEMPROMPT_TRUSTED_HTTP_HOSTS";

#[must_use]
pub fn trusted_http_hosts_from_env() -> Vec<String> {
    std::env::var(TRUSTED_HTTP_HOSTS_ENV)
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(|s| s.trim().to_ascii_lowercase())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

pub fn trusted_hosts_env_entry(
    lookup: impl Fn(&str) -> Option<String>,
) -> Option<(String, String)> {
    lookup(TRUSTED_HTTP_HOSTS_ENV).map(|trusted| (TRUSTED_HTTP_HOSTS_ENV.to_owned(), trusted))
}
