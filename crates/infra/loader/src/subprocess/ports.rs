//! Which processes are listening on a TCP port.
//!
//! The lookup shells out (`lsof` on Unix, `netstat` on Windows) and therefore
//! blocks; [`super::pids_listening_on`] runs it under `spawn_blocking`. Only
//! listening sockets count: a client connection to the port is not a holder.
//! Port 0 means "no port" — asking the OS for its holder would match every
//! unbound socket — so it never reaches the lookup, and a reported pid 0 is
//! dropped.
//! The parsers are pure so their edge cases are testable without a socket.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::Command;

use super::SupervisionError;

#[must_use]
pub fn parse_lsof_pids(stdout: &str) -> Vec<u32> {
    let mut pids: Vec<u32> = stdout
        .lines()
        .filter_map(|line| line.trim().parse().ok())
        .filter(|pid| *pid != 0)
        .collect();
    pids.sort_unstable();
    pids.dedup();
    pids
}

#[must_use]
pub fn parse_netstat_listeners(stdout: &str, port: u16) -> Vec<u32> {
    let suffix = format!(":{port}");
    let mut pids: Vec<u32> = stdout
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            match fields.as_slice() {
                [proto, local, _remote, state, pid]
                    if proto.eq_ignore_ascii_case("TCP")
                        && local.ends_with(&suffix)
                        && state.eq_ignore_ascii_case("LISTENING") =>
                {
                    pid.parse().ok()
                },
                _ => None,
            }
        })
        .filter(|pid| *pid != 0)
        .collect();
    pids.sort_unstable();
    pids.dedup();
    pids
}

#[cfg(unix)]
pub(super) fn listening_pids(port: u16) -> Result<Vec<u32>, SupervisionError> {
    let output = Command::new("lsof")
        .args(["-nP", "-t", &format!("-iTCP:{port}"), "-sTCP:LISTEN"])
        .output()
        .map_err(|source| SupervisionError::Tool {
            tool: "lsof",
            source,
        })?;
    // Why: lsof exits 1 with empty output when nothing matches the filter.
    if !output.status.success() && !output.stdout.is_empty() {
        return Err(SupervisionError::ToolFailed {
            tool: "lsof",
            status: output.status,
        });
    }
    Ok(parse_lsof_pids(&String::from_utf8_lossy(&output.stdout)))
}

#[cfg(windows)]
pub(super) fn listening_pids(port: u16) -> Result<Vec<u32>, SupervisionError> {
    let output = Command::new("netstat")
        .args(["-ano", "-p", "TCP"])
        .output()
        .map_err(|source| SupervisionError::Tool {
            tool: "netstat",
            source,
        })?;
    if !output.status.success() {
        return Err(SupervisionError::ToolFailed {
            tool: "netstat",
            status: output.status,
        });
    }
    Ok(parse_netstat_listeners(
        &String::from_utf8_lossy(&output.stdout),
        port,
    ))
}

#[cfg(not(any(unix, windows)))]
pub(super) fn listening_pids(_port: u16) -> Result<Vec<u32>, SupervisionError> {
    Err(SupervisionError::Unsupported)
}
