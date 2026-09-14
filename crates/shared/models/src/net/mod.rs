//! Shared network timeout constants and outbound-URL validation.
//!
//! Centralised [`Duration`] values for HTTP client configuration, TCP
//! readiness probes, and long-poll image generation, so every caller
//! uses the same tuned timeouts, plus [`validate_outbound_url`] — the
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

// Why: RFC 4291 §2.5.5.2 maps `::ffff:0:0/96` to IPv4 and §2.5.5.1 embeds an
// IPv4 address in the low 32 bits of `::/96`; RFC 6052 `64:ff9b::/96` is the
// NAT64 well-known prefix, through which `64:ff9b::a9fe:a9fe` reaches
// 169.254.169.254. Each embedded IPv4 is judged by the IPv4 table.
fn is_blocked_v6(ip: std::net::Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return is_blocked_v4(v4);
    }
    let segments = ip.segments();
    if let Some(v4) = embedded_v4(&segments) {
        return is_blocked_v4(v4);
    }
    let is_unique_local = (segments[0] & 0xfe00) == 0xfc00;
    let is_link_local = (segments[0] & 0xffc0) == 0xfe80;
    let is_multicast = (segments[0] & 0xff00) == 0xff00;
    ip.is_loopback() || ip.is_unspecified() || is_unique_local || is_link_local || is_multicast
}

fn embedded_v4(segments: &[u16; 8]) -> Option<std::net::Ipv4Addr> {
    let is_nat64 = segments[..6] == [0x64, 0xff9b, 0, 0, 0, 0];
    let is_ipv4_compatible = segments[..6] == [0, 0, 0, 0, 0, 0];
    if !(is_nat64 || is_ipv4_compatible) {
        return None;
    }
    let [a, b] = segments[6].to_be_bytes();
    let [c, d] = segments[7].to_be_bytes();
    Some(std::net::Ipv4Addr::new(a, b, c, d))
}

// Why: RFC 6598 reserves `100.64.0.0/10` for shared carrier-grade NAT, not
// public hosts.
fn is_cgnat_shared_v4(ip: std::net::Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();
    a == 100 && (64..=127).contains(&b)
}

// Why: `0.0.0.0/8` routes to the local host on Linux, `192.0.0.0/24` (RFC
// 6890) and `198.18.0.0/15` (RFC 2544) are IETF-reserved, and `224.0.0.0/4` /
// `240.0.0.0/4` are multicast and reserved — none is a public host.
const fn is_reserved_v4(ip: std::net::Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    a == 0 || (a == 192 && b == 0 && c == 0) || (a == 198 && (b == 18 || b == 19)) || a >= 224
}

fn is_blocked_v4(ip: std::net::Ipv4Addr) -> bool {
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || is_cgnat_shared_v4(ip)
        || is_reserved_v4(ip)
}
