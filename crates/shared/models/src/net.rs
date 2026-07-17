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

/// Default per-attempt timeout for a non-streaming AI provider request.
pub const AI_PROVIDER_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// Default timeout for a single MCP tool-call RPC (excludes connection setup).
pub const MCP_TOOL_EXECUTION_TIMEOUT: Duration = Duration::from_secs(30);

/// Operator-supplied allowlist of non-loopback hostnames reachable over plain
/// `http`.
///
/// Comma-separated, case-insensitive, exact domain match only — no globs, no
/// IP, no port. The intended use is sealed-network demos (the air-gap scenario)
/// and behind-the-firewall mock services, where the SSRF guard's default
/// "loopback-only http" rule would otherwise reject a known-trusted internal
/// hostname like `mock-inference`. **Default empty** — operator opts in by
/// naming every host explicitly. Does not loosen the scheme, IP block, or
/// private-range rules for any host outside the allowlist.
pub const TRUSTED_HTTP_HOSTS_ENV: &str = "SYSTEMPROMPT_TRUSTED_HTTP_HOSTS";

/// Parse [`TRUSTED_HTTP_HOSTS_ENV`] into a normalised allowlist.
///
/// Empty/missing → empty vec. Hosts are trimmed and lower-cased; empty
/// entries (from `a,,b` typos) are dropped.
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

/// Validate an operator-configured outbound webhook destination, returning the
/// parsed URL on success.
///
/// Rejects destinations that point at the local host or known private network
/// ranges; these would otherwise let a configured webhook exfiltrate
/// cloud-metadata endpoints (e.g. `169.254.169.254`) or internal services on
/// the same subnet. The scheme must be `https` for production destinations;
/// `http` is allowed only for explicit loopback names used during local
/// development.
pub fn validate_outbound_url(url: &str) -> Result<url::Url, OutboundUrlError> {
    let no_trust: [&str; 0] = [];
    validate_outbound_url_with_trust(url, &no_trust)
}

/// Same as [`validate_outbound_url`], but accepts an explicit allowlist of
/// hostnames the operator has marked as reachable over plain `http`.
///
/// A host in `trusted_http_hosts` is treated like `localhost` for the scheme
/// gate (http accepted) and **also bypasses the private-range IP block** for
/// that hostname's resolution path — the latter matters because in-network
/// hostnames typically resolve to RFC1918 IPs that the standard guard
/// rejects. The IP-blocklist is still enforced for every host *not* in the
/// allowlist.
///
/// Matching is exact, case-insensitive, on the URL's parsed host. IPs in the
/// allowlist are matched literally (allowlist callers should generally use
/// hostnames, not addresses).
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
        url::Host::Ipv6(ip) => {
            // RFC 4291 §2.5.5.2: an ::ffff:0:0/96 address embeds a real IPv4
            // address; treat it as that IPv4 address for SSRF purposes so a
            // hand-crafted v4-mapped URL cannot bypass the v4 block list.
            ip.to_ipv4_mapped().map_or_else(
                || {
                    let segments = ip.segments();
                    let is_unique_local = (segments[0] & 0xfe00) == 0xfc00;
                    let is_link_local = (segments[0] & 0xffc0) == 0xfe80;
                    ip.is_loopback() || ip.is_unspecified() || is_unique_local || is_link_local
                },
                is_blocked_v4,
            )
        },
    };
    if blocked {
        return Err(OutboundUrlError::BlockedHost(
            parsed.host_str().unwrap_or_default().to_owned(),
        ));
    }
    Ok(parsed)
}

/// RFC 6598 carrier-grade NAT range `100.64.0.0/10` — operator-routable but
/// commonly bridges to internal services on cloud-provider managed networks.
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
