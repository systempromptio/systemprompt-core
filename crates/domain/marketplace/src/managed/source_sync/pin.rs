//! Resolves a Git repository host in-process and pins the subprocess to the
//! vetted address.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::net::{IpAddr, ToSocketAddrs};
use std::process::Command;

use systemprompt_models::net::{
    LoopbackPolicy, OutboundUrlError, PinnedAddress, ProxyEnvironment, pin_resolved,
    validate_outbound_url,
};

use super::{ManagedError, Result};

const PINNING_GIT: (u32, u32) = (2, 37);

fn refused(error: OutboundUrlError) -> ManagedError {
    super::super::error::invalid_input(
        "Git repository URL is not an allowed outbound target",
        error,
    )
}

pub(super) fn route_repository(repository: &str, loopback: LoopbackPolicy) -> Result<String> {
    let url = validate_outbound_url(repository, loopback).map_err(refused)?;
    let host = url.host_str().unwrap_or_default();
    if let Some(proxy) = ProxyEnvironment::from_env().https_proxy_for(host) {
        return Ok(format!("http.proxy={proxy}"));
    }
    let pinned = pin_url(&url, loopback)?;
    Ok(format!(
        "http.curloptResolve={}",
        pinned.curl_resolve_entry()
    ))
}

fn pin_url(url: &url::Url, loopback: LoopbackPolicy) -> Result<PinnedAddress> {
    let addrs = match url.host() {
        Some(url::Host::Ipv4(ip)) => vec![IpAddr::V4(ip)],
        Some(url::Host::Ipv6(ip)) => vec![IpAddr::V6(ip)],
        Some(url::Host::Domain(name)) => {
            let port = url.port_or_known_default().unwrap_or(443);
            (name, port)
                .to_socket_addrs()
                .map_err(|_unresolved| ManagedError::Unavailable)?
                .map(|addr| addr.ip())
                .collect()
        },
        None => return Err(refused(OutboundUrlError::MissingHost)),
    };
    let no_trust: [&str; 0] = [];
    pin_resolved(url, &addrs, &no_trust, loopback).map_err(refused)
}

fn parse_git_version(output: &str) -> Option<(u32, u32)> {
    let version = output.trim().strip_prefix("git version ")?;
    let mut parts = version.split(|c: char| !c.is_ascii_digit());
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

pub(super) fn require_pinning_git(deadline: std::time::Instant) -> Result<()> {
    let remaining = deadline
        .checked_duration_since(std::time::Instant::now())
        .ok_or(ManagedError::Unavailable)?;
    let mut limits = super::super::git_execution::GitExecutionLimits::default();
    limits.deadline = limits.deadline.min(remaining);
    let output =
        super::super::git_execution::execute(Command::new("git").arg("version"), None, limits)?;
    let text = std::str::from_utf8(&output).map_err(|_corrupt| ManagedError::Integrity)?;
    match parse_git_version(text) {
        Some(version) if version >= PINNING_GIT => Ok(()),
        _ => Err(super::super::error::invalid(
            "Git 2.37 or newer is required to pin the repository address",
        )),
    }
}
