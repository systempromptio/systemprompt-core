//! Port liveness probing and reclamation for MCP servers.
//!
//! Provides timeout-bounded loopback probes ([`is_port_in_use`]),
//! cross-platform cleanup of processes holding a port, and retry/backoff
//! helpers that wait for a port to free up before a server binds. The probe
//! timeout guards against kernel-level connect hangs that would otherwise stall
//! startup silently.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::McpDomainResult;
use std::net::{SocketAddr, TcpStream};
use std::process::Command;
use std::time::Duration;

pub const MAX_PORT_CLEANUP_ATTEMPTS: u32 = 5;
pub const PORT_BACKOFF_BASE_MS: u64 = 200;
pub const POST_KILL_DELAY_MS: u64 = 500;

/// Hard cap on a single localhost TCP connect probe. Without a timeout,
/// a stuck `SYN_SENT` (WSL2 / firewall / SYN-blackhole pathologies)
/// blocks the runtime worker indefinitely and the entire MCP startup
/// hangs silently with no log line to show why. 1s is generous for
/// loopback while still failing fast and loud.
const PORT_PROBE_TIMEOUT: Duration = Duration::from_secs(1);

pub async fn prepare_port(port: u16) -> McpDomainResult<()> {
    tracing::debug!(port = port, "Preparing port");

    if is_port_in_use(port) {
        tracing::debug!(port = port, "Port is in use, cleaning up");
        cleanup_port_processes(port).await?;
    }

    tracing::debug!(port = port, "Port is ready");
    Ok(())
}

/// Returns `true` only if a TCP handshake to `127.0.0.1:port` completes within
/// ~1s.
///
/// A refused connection (the listener really isn't there) returns `false`
/// quickly; a kernel-level hang (no SYN/ACK, no RST) returns `false` after the
/// probe timeout with a `warn!` so the operator sees *why* the next startup
/// step decided the port was free.
#[must_use]
pub fn is_port_in_use(port: u16) -> bool {
    let addr: SocketAddr = match format!("127.0.0.1:{port}").parse() {
        Ok(a) => a,
        Err(e) => {
            tracing::error!(port = port, error = %e, "BUG: failed to parse loopback addr for probe");
            return false;
        },
    };
    match TcpStream::connect_timeout(&addr, PORT_PROBE_TIMEOUT) {
        Ok(_) => true,
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => false,
        Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
            tracing::warn!(
                port = port,
                timeout_ms = PORT_PROBE_TIMEOUT.as_millis() as u64,
                "Port probe timed out — no listener accepted, no RST sent. Treating port as free. \
                 If MCP server then fails to bind, a stale half-open socket on this port is the \
                 likely cause."
            );
            false
        },
        Err(e) => {
            tracing::warn!(port = port, error = %e, "Port probe failed; treating port as free");
            false
        },
    }
}

#[must_use]
pub fn is_port_responsive(port: u16) -> bool {
    is_port_in_use(port)
}

#[cfg(unix)]
pub async fn cleanup_port_processes(port: u16) -> McpDomainResult<()> {
    use nix::sys::signal::{self, Signal};
    use nix::unistd::Pid;

    let output = Command::new("lsof")
        .args(["-ti", &format!(":{port}")])
        .output()
        .map_err(|e| {
            crate::error::McpDomainError::Internal(format!(
                "failed to run `lsof -ti :{port}` for port {port}: {e}"
            ))
        })?;

    if !output.stdout.is_empty() {
        let pids = String::from_utf8_lossy(&output.stdout);
        let self_pid = std::process::id() as i32;
        for pid_str in pids.lines() {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                // Never signal ourselves: `lsof` can return this process when it
                // holds the port, and killing the caller is never the intent.
                if pid <= 0 || pid == self_pid {
                    continue;
                }
                tracing::debug!(port = port, pid = pid, "Stopping process on port");

                if let Err(e) = signal::kill(Pid::from_raw(pid), Signal::SIGTERM) {
                    tracing::warn!(pid = pid, error = %e, "Failed to send SIGTERM to port process");
                }

                tokio::time::sleep(Duration::from_millis(100)).await;

                if let Err(e) = signal::kill(Pid::from_raw(pid), Signal::SIGKILL) {
                    tracing::warn!(pid = pid, error = %e, "Failed to send SIGKILL to port process");
                }
            }
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    Ok(())
}

#[cfg(windows)]
pub async fn cleanup_port_processes(port: u16) -> McpDomainResult<()> {
    let output = Command::new("netstat")
        .args(["-ano", "-p", "TCP"])
        .output()
        .map_err(|e| {
            crate::error::McpDomainError::Internal(format!(
                "{}: {e}",
                format!("failed to run `netstat -ano -p TCP` for port {port}")
            ))
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let port_pattern = format!(":{port} ");

    for line in stdout.lines() {
        if line.contains(&port_pattern) {
            if let Some(pid_str) = line.split_whitespace().last() {
                if pid_str.parse::<u32>().is_ok() {
                    tracing::debug!(port = port, pid = %pid_str, "Stopping process on port");

                    if let Err(e) = Command::new("taskkill").args(["/PID", pid_str]).output() {
                        tracing::warn!(pid = %pid_str, error = %e, "Failed to send taskkill to port process");
                    }

                    tokio::time::sleep(Duration::from_millis(100)).await;

                    if let Err(e) = Command::new("taskkill")
                        .args(["/PID", pid_str, "/F"])
                        .output()
                    {
                        tracing::warn!(pid = %pid_str, error = %e, "Failed to force taskkill port process");
                    }
                }
            }
        }
    }

    tokio::time::sleep(Duration::from_millis(200)).await;

    Ok(())
}

pub async fn wait_for_port_release(port: u16) -> McpDomainResult<()> {
    let max_attempts = 10;
    let delay = Duration::from_millis(100);

    for attempt in 1..=max_attempts {
        if !is_port_in_use(port) {
            return Ok(());
        }

        if attempt < max_attempts {
            tokio::time::sleep(delay).await;
        }
    }

    Err(crate::error::McpDomainError::Internal(format!(
        "Port {port} did not become available after {max_attempts} attempts"
    )))
}

pub async fn wait_for_port_release_with_retry(
    port: u16,
    max_cleanup_attempts: u32,
) -> McpDomainResult<()> {
    for cleanup_attempt in 1..=max_cleanup_attempts {
        if !is_port_in_use(port) {
            return Ok(());
        }

        tracing::debug!(
            port = port,
            attempt = cleanup_attempt,
            max_attempts = max_cleanup_attempts,
            "Port still in use, attempting cleanup"
        );

        cleanup_port_processes(port).await?;

        match wait_for_port_release(port).await {
            Ok(()) => return Ok(()),
            Err(_) if cleanup_attempt < max_cleanup_attempts => {
                let backoff =
                    Duration::from_millis(PORT_BACKOFF_BASE_MS * u64::from(cleanup_attempt));
                tokio::time::sleep(backoff).await;
            },
            Err(e) => return Err(e),
        }
    }

    Err(crate::error::McpDomainError::Internal(format!(
        "Port {port} could not be acquired after {max_cleanup_attempts} cleanup attempts"
    )))
}

pub const fn cleanup_port_resources(_port: u16) {}

pub fn find_available_port(start_port: u16, end_port: u16) -> McpDomainResult<u16> {
    for port in start_port..=end_port {
        if !is_port_in_use(port) {
            return Ok(port);
        }
    }

    Err(crate::error::McpDomainError::Internal(format!(
        "No available ports in range {start_port}-{end_port}"
    )))
}
