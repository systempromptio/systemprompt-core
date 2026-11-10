//! Parse-time outbound-URL validation and resolved-address pinning.
//!
//! [`LoopbackPolicy`] is the single switch for loopback reachability: an
//! operator-configured destination may sit on the local host, a destination a
//! user or third party chose may not. Loopback is decided before the block
//! list only when the policy allows it; under [`LoopbackPolicy::Deny`] a
//! `localhost` name is refused outright and a loopback literal falls to the
//! block list like any other reserved address.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::net::IpAddr;

use thiserror::Error;

use super::blocklist::{is_blocked_ip, is_blocked_v4, is_blocked_v6};

/// Rejection reason for an outbound URL or its resolved address.
#[derive(Debug, Error)]
pub enum OutboundUrlError {
    #[error("invalid url: {0}")]
    Parse(#[from] url::ParseError),
    #[error("invalid url: missing host")]
    MissingHost,
    #[error("unsupported url scheme: {0}")]
    Scheme(String),
    #[error("http url only permitted for loopback hosts")]
    NonLoopbackHttp,
    #[error("host {0} is loopback and loopback is not permitted")]
    Loopback(String),
    #[error("host {0} is in a blocked private range")]
    BlockedHost(String),
    #[error("host {host} resolves to blocked address {addr}")]
    BlockedAddress { host: String, addr: IpAddr },
    #[error("host {0} resolved to no address")]
    Unresolved(String),
    #[error("host {0} is the forward proxy and is not a permitted target")]
    ProxyHost(String),
}

/// Whether a destination may be the local host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopbackPolicy {
    Allow,
    Deny,
}

/// A hostname bound to the one address that passed the block list, so a
/// subprocess connects to exactly what was vetted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinnedAddress {
    host: String,
    port: u16,
    addr: IpAddr,
}

impl PinnedAddress {
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }

    #[must_use]
    pub const fn addr(&self) -> IpAddr {
        self.addr
    }

    #[must_use]
    pub fn curl_resolve_entry(&self) -> String {
        match self.addr {
            IpAddr::V4(v4) => format!("{}:{}:{v4}", self.host, self.port),
            IpAddr::V6(v6) => format!("{}:{}:[{v6}]", self.host, self.port),
        }
    }
}

#[must_use]
pub fn is_loopback_host(host: &url::Host<&str>) -> bool {
    match host {
        url::Host::Domain(d) => {
            let name = d.trim_end_matches('.').to_ascii_lowercase();
            name == "localhost" || name.ends_with(".localhost")
        },
        url::Host::Ipv4(ip) => ip.is_loopback(),
        url::Host::Ipv6(ip) => ip.is_loopback(),
    }
}

fn is_trusted(host: &str, trusted_http_hosts: &[impl AsRef<str>]) -> bool {
    !host.is_empty()
        && trusted_http_hosts
            .iter()
            .any(|h| h.as_ref().trim().eq_ignore_ascii_case(host))
}

pub fn validate_outbound_url(
    url: &str,
    loopback: LoopbackPolicy,
) -> Result<url::Url, OutboundUrlError> {
    let no_trust: [&str; 0] = [];
    validate_outbound_url_with_trust(url, &no_trust, loopback)
}

pub fn validate_outbound_url_with_trust(
    url: &str,
    trusted_http_hosts: &[impl AsRef<str>],
    loopback: LoopbackPolicy,
) -> Result<url::Url, OutboundUrlError> {
    let parsed = url::Url::parse(url)?;
    let host = parsed.host().ok_or(OutboundUrlError::MissingHost)?;
    let host_str = parsed.host_str().unwrap_or_default().to_ascii_lowercase();

    let is_loopback = is_loopback_host(&host);
    let loopback_allowed = is_loopback && loopback == LoopbackPolicy::Allow;
    let trusted = is_trusted(&host_str, trusted_http_hosts);

    match parsed.scheme() {
        "https" => {},
        "http" if loopback_allowed || trusted => {},
        "http" => return Err(OutboundUrlError::NonLoopbackHttp),
        scheme => return Err(OutboundUrlError::Scheme(scheme.to_owned())),
    }

    if loopback_allowed || trusted {
        return Ok(parsed);
    }
    if is_loopback {
        return Err(OutboundUrlError::Loopback(host_str));
    }

    let blocked = match host {
        url::Host::Domain(_) => false,
        url::Host::Ipv4(ip) => is_blocked_v4(ip),
        url::Host::Ipv6(ip) => is_blocked_v6(ip),
    };
    if blocked {
        return Err(OutboundUrlError::BlockedHost(host_str));
    }
    Ok(parsed)
}

pub fn pin_resolved(
    url: &url::Url,
    resolved: &[IpAddr],
    trusted_http_hosts: &[impl AsRef<str>],
    loopback: LoopbackPolicy,
) -> Result<PinnedAddress, OutboundUrlError> {
    let host = url.host().ok_or(OutboundUrlError::MissingHost)?;
    let host_str = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let port = url
        .port_or_known_default()
        .ok_or_else(|| OutboundUrlError::Scheme(url.scheme().to_owned()))?;
    let exempt = is_trusted(&host_str, trusted_http_hosts)
        || (loopback == LoopbackPolicy::Allow && is_loopback_host(&host));
    let addr = *resolved
        .first()
        .ok_or_else(|| OutboundUrlError::Unresolved(host_str.clone()))?;
    if !exempt && let Some(blocked) = resolved.iter().find(|a| is_blocked_ip(**a)) {
        return Err(OutboundUrlError::BlockedAddress {
            host: host_str,
            addr: *blocked,
        });
    }
    Ok(PinnedAddress {
        host: host_str,
        port,
        addr,
    })
}
