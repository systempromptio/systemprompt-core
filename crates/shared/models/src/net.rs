//! Shared network timeout constants and outbound-URL validation.
//!
//! Centralised [`Duration`] values for HTTP client configuration, TCP
//! readiness probes, and long-poll image generation, so every caller
//! uses the same tuned timeouts, plus [`validate_outbound_url`] — the
//! single SSRF guard applied to every operator-configured webhook
//! destination (agent integrations and the governance authz hook).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;
use thiserror::Error;

/// Rejection reason for an operator-configured outbound URL.
#[derive(Debug, Error)]
pub enum OutboundUrlError {
    #[error("invalid url: {0}")]
    Parse(String),
    #[error("unsupported url scheme: {0}")]
    Scheme(String),
    #[error("http url only permitted for loopback hosts")]
    NonLoopbackHttp,
    #[error("host {0} is in a blocked private range")]
    BlockedHost(String),
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

pub fn validate_outbound_url(url: &str) -> Result<url::Url, OutboundUrlError> {
    let no_trust: [&str; 0] = [];
    validate_outbound_url_with_trust(url, &no_trust)
}

pub fn validate_outbound_url_with_trust(
    url: &str,
    trusted_http_hosts: &[impl AsRef<str>],
) -> Result<url::Url, OutboundUrlError> {
    let parsed = url::Url::parse(url).map_err(|e| OutboundUrlError::Parse(e.to_string()))?;
    let host = parsed
        .host()
        .ok_or_else(|| OutboundUrlError::Parse("missing host".to_owned()))?;

    let is_loopback_host = match &host {
        url::Host::Domain(d) => d.eq_ignore_ascii_case("localhost"),
        url::Host::Ipv4(ip) => ip.is_loopback(),
        url::Host::Ipv6(ip) => ip.is_loopback(),
    };

    let host_str = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
    let is_trusted = !host_str.is_empty()
        && trusted_http_hosts
            .iter()
            .any(|h| h.as_ref().eq_ignore_ascii_case(&host_str));

    match parsed.scheme() {
        "https" => {},
        "http" if is_loopback_host || is_trusted => {},
        "http" => return Err(OutboundUrlError::NonLoopbackHttp),
        scheme => return Err(OutboundUrlError::Scheme(scheme.to_owned())),
    }

    if is_loopback_host || is_trusted {
        return Ok(parsed);
    }

    let blocked = match host {
        url::Host::Domain(_) => false,
        url::Host::Ipv4(ip) => is_blocked_v4(ip),
        url::Host::Ipv6(ip) => is_blocked_v6(ip),
    };
    if blocked {
        return Err(OutboundUrlError::BlockedHost(
            parsed.host_str().unwrap_or_default().to_owned(),
        ));
    }
    Ok(parsed)
}

#[must_use]
pub fn is_blocked_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => is_blocked_v4(v4),
        std::net::IpAddr::V6(v6) => is_blocked_v6(v6),
    }
}

// Why: RFC 4291 §2.5.5.2 maps `::ffff:0:0/96` to IPv4, including private IPv4
// addresses.
fn is_blocked_v6(ip: std::net::Ipv6Addr) -> bool {
    ip.to_ipv4_mapped().map_or_else(
        || {
            let segments = ip.segments();
            let is_unique_local = (segments[0] & 0xfe00) == 0xfc00;
            let is_link_local = (segments[0] & 0xffc0) == 0xfe80;
            ip.is_loopback() || ip.is_unspecified() || is_unique_local || is_link_local
        },
        is_blocked_v4,
    )
}

// Why: RFC 6598 reserves `100.64.0.0/10` for shared carrier-grade NAT, not
// public hosts.
fn is_cgnat_shared_v4(ip: std::net::Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();
    a == 100 && (64..=127).contains(&b)
}

fn is_blocked_v4(ip: std::net::Ipv4Addr) -> bool {
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || is_cgnat_shared_v4(ip)
}
