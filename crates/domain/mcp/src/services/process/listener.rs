//! TCP listener lookup: which processes accept connections on a port.
//!
//! The port is a [`NonZeroU16`]: port 0 is "no port", and asking the OS which
//! process holds it matches every unbound or unconnected socket on the host,
//! so it is unrepresentable here rather than checked at each caller. Only
//! sockets in the listening state are reported; a client connected to the
//! port from elsewhere is not its holder.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::McpDomainResult;
use std::num::NonZeroU16;
use std::process::Command;

#[cfg(unix)]
pub fn listener_pids(port: NonZeroU16) -> McpDomainResult<Vec<u32>> {
    let port_filter = format!("-iTCP:{port}");
    let output = Command::new("lsof")
        .args(["-nP", &port_filter, "-sTCP:LISTEN", "-t"])
        .output()
        .map_err(|e| {
            crate::error::McpDomainError::operation(
                format!("failed to run `lsof -nP -iTCP:{port} -sTCP:LISTEN -t` for port {port}"),
                e,
            )
        })?;

    Ok(parse_lsof_pids(&String::from_utf8_lossy(&output.stdout)))
}

#[cfg(windows)]
pub fn listener_pids(port: NonZeroU16) -> McpDomainResult<Vec<u32>> {
    let output = Command::new("netstat")
        .args(["-ano", "-p", "TCP"])
        .output()
        .map_err(|e| {
            crate::error::McpDomainError::operation(
                format!("failed to run `netstat -ano -p TCP` for port {port}"),
                e,
            )
        })?;

    Ok(parse_netstat_listeners(
        &String::from_utf8_lossy(&output.stdout),
        port,
    ))
}

#[must_use]
pub fn parse_lsof_pids(stdout: &str) -> Vec<u32> {
    let mut pids = Vec::new();
    for pid in stdout
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .filter(|pid| *pid != 0)
    {
        if !pids.contains(&pid) {
            pids.push(pid);
        }
    }
    pids
}

#[must_use]
pub fn parse_netstat_listeners(stdout: &str, port: NonZeroU16) -> Vec<u32> {
    let mut pids = Vec::new();
    for line in stdout.lines() {
        let columns: Vec<&str> = line.split_whitespace().collect();
        let [proto, local, _foreign, state, pid] = columns.as_slice() else {
            continue;
        };
        if !proto.eq_ignore_ascii_case("TCP") || *state != "LISTENING" {
            continue;
        }
        let local_port = local
            .rsplit_once(':')
            .and_then(|(_, port)| port.parse::<u16>().ok());
        if local_port != Some(port.get()) {
            continue;
        }
        if let Ok(pid) = pid.parse::<u32>()
            && pid != 0
            && !pids.contains(&pid)
        {
            pids.push(pid);
        }
    }
    pids
}
