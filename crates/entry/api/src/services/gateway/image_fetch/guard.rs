//! SSRF guard for server-side fetches of caller-supplied image URLs.
//!
//! Wraps [`validate_outbound_url_with_trust`] — the same block list the
//! provider registry and the governance webhooks use — and tightens it for the
//! one case it was not written for: a URL chosen by whoever sent the inference
//! request, rather than by an operator. Loopback stops being an implicit
//! allow. Hostnames are not resolved here; the client this module builds
//! carries the connect-time resolver guard, so a name is checked against the
//! block list at the moment the socket is opened, on the first request and on
//! every redirect hop.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::net::IpAddr;

use systemprompt_models::net::{
    GuardedClientConfig, is_blocked_ip, validate_outbound_url_with_trust,
};

use super::ImageFetchPolicy;

pub(super) fn is_trusted(host: &str, trusted_hosts: &[String]) -> bool {
    trusted_hosts.iter().any(|h| h.eq_ignore_ascii_case(host))
}

pub(super) fn checked_url(raw: &str, trusted_hosts: &[String]) -> Result<url::Url, String> {
    let parsed = validate_outbound_url_with_trust(raw, trusted_hosts).map_err(|e| e.to_string())?;
    let host = parsed
        .host_str()
        .ok_or_else(|| "missing host".to_owned())?
        .to_ascii_lowercase();
    if is_trusted(&host, trusted_hosts) {
        return Ok(parsed);
    }
    match parsed.host() {
        Some(url::Host::Ipv4(ip)) => reject_blocked(&host, IpAddr::V4(ip))?,
        Some(url::Host::Ipv6(ip)) => reject_blocked(&host, IpAddr::V6(ip))?,
        Some(url::Host::Domain(_)) => {},
        None => return Err("missing host".to_owned()),
    }
    Ok(parsed)
}

pub(super) fn client(policy: &ImageFetchPolicy) -> Result<reqwest::Client, String> {
    let config = GuardedClientConfig::default()
        .with_trusted_hosts(policy.trusted_hosts.clone())
        .with_max_redirects(usize::from(policy.max_redirects))
        .with_timeout(policy.timeout)
        .deny_loopback();
    systemprompt_models::net::guarded_client(&config)
        .map_err(|e| format!("cannot build guarded image client: {e}"))
}

fn reject_blocked(host: &str, addr: IpAddr) -> Result<(), String> {
    if is_blocked_ip(addr) {
        return Err(format!("{host} resolves to blocked address {addr}"));
    }
    Ok(())
}
