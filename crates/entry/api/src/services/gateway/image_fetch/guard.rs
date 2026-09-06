//! SSRF guard for server-side fetches of caller-supplied image URLs.
//!
//! Wraps [`validate_outbound_url_with_trust`] — the same block list the
//! provider registry and the governance webhooks use — and tightens it for the
//! one case it was not written for: a URL chosen by whoever sent the inference
//! request, rather than by an operator. Two things change. Loopback stops being
//! an implicit allow, and a hostname is resolved so the DNS answer is checked
//! against the block list too, since `127.0.0.1.nip.io` is a domain to the
//! parser and an internal address to the socket.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::net::IpAddr;

use systemprompt_models::net::{is_blocked_ip, validate_outbound_url_with_trust};

const DEFAULT_PORT: u16 = 443;

pub(super) fn is_trusted(host: &str, trusted_hosts: &[String]) -> bool {
    trusted_hosts.iter().any(|h| h.eq_ignore_ascii_case(host))
}

// Why: the reason is returned as a plain string rather than a typed error
// because every rejection here is caller fault and the caller wraps them all
// the same way; a taxonomy would have exactly one consumer.
pub(super) async fn checked_url(raw: &str, trusted_hosts: &[String]) -> Result<url::Url, String> {
    let parsed = validate_outbound_url_with_trust(raw, trusted_hosts).map_err(|e| e.to_string())?;
    let host = parsed
        .host_str()
        .ok_or_else(|| "missing host".to_owned())?
        .to_ascii_lowercase();
    if is_trusted(&host, trusted_hosts) {
        return Ok(parsed);
    }
    match parsed.host() {
        Some(url::Host::Ipv4(ip)) => reject_blocked(&host, &[IpAddr::V4(ip)])?,
        Some(url::Host::Ipv6(ip)) => reject_blocked(&host, &[IpAddr::V6(ip)])?,
        Some(url::Host::Domain(_)) => {
            let port = parsed.port_or_known_default().unwrap_or(DEFAULT_PORT);
            reject_blocked(&host, &resolve(&host, port).await?)?;
        },
        None => return Err("missing host".to_owned()),
    }
    Ok(parsed)
}

// Why: the resolution here and the one reqwest performs when it connects are
// two separate lookups, so a record with a one-second TTL can answer publicly
// for the check and internally for the connect. Closing that fully means
// pinning the connection to a vetted address, which reqwest exposes only per
// client; the block list still stops every static internal target, which is
// what the caller-supplied-URL threat actually looks like in practice.
async fn resolve(host: &str, port: u16) -> Result<Vec<IpAddr>, String> {
    let addrs: Vec<IpAddr> = tokio::net::lookup_host((host, port))
        .await
        .map_err(|e| format!("cannot resolve {host}: {e}"))?
        .map(|sa| sa.ip())
        .collect();
    if addrs.is_empty() {
        return Err(format!("cannot resolve {host}"));
    }
    Ok(addrs)
}

// Why: one blocked answer is enough to refuse. A name that resolves to both a
// public and an internal address is a rebinding attempt, not a fallback.
fn reject_blocked(host: &str, addrs: &[IpAddr]) -> Result<(), String> {
    addrs
        .iter()
        .find(|ip| is_blocked_ip(**ip))
        .map_or(Ok(()), |ip| {
            Err(format!("{host} resolves to blocked address {ip}"))
        })
}
