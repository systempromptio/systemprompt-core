//! Port liveness probing and reclamation for MCP servers.
//!
//! Provides timeout-bounded loopback probes ([`is_port_in_use`]),
//! reclamation of a port held by this service's own stale process, and
//! retry/backoff helpers that wait for a port to free up before a server
//! binds. A holder is stopped only when the loader's supervision module proves
//! it carries this service's marker; a foreign or unverifiable holder is
//! refused with a typed error. The probe timeout guards against kernel-level
//! connect hangs that would otherwise stall startup silently.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::{McpDomainError, McpDomainResult};
use std::net::SocketAddr;
use std::time::Duration;
use systemprompt_identifiers::ServiceName;
use systemprompt_loader::subprocess::{self, ChildKind, StopOutcome};
use tokio::net::TcpStream;

pub const MAX_PORT_CLEANUP_ATTEMPTS: u32 = 5;
pub const PORT_BACKOFF_BASE_MS: u64 = 200;
pub const POST_KILL_DELAY_MS: u64 = 500;

// Why: Firewalls can blackhole even loopback SYN packets, leaving a blocking
// connect waiting for the operating system's TCP timeout.
const PORT_PROBE_TIMEOUT: Duration = Duration::from_secs(1);
const STALE_HOLDER_GRACE: Duration = Duration::from_secs(5);

pub async fn prepare_port(port: u16, service_name: &ServiceName) -> McpDomainResult<()> {
    tracing::debug!(port = port, service = %service_name, "Preparing port");

    if is_port_in_use(port).await {
        tracing::debug!(port = port, service = %service_name, "Port is in use, cleaning up");
        cleanup_port_processes(port, service_name).await?;
    }

    tracing::debug!(port = port, service = %service_name, "Port is ready");
    Ok(())
}

enum PortHolder {
    Ours,
    Caller,
    Gone,
    Foreign,
    Unverifiable,
}

async fn classify_port_holder(pid: u32, service_name: &ServiceName) -> PortHolder {
    if pid == std::process::id() {
        return PortHolder::Caller;
    }
    if !systemprompt_models::subprocess::identity_verification_supported() {
        return PortHolder::Unverifiable;
    }
    if subprocess::owns(pid, ChildKind::Mcp, service_name).await {
        PortHolder::Ours
    } else if subprocess::is_running(pid).await {
        PortHolder::Foreign
    } else {
        PortHolder::Gone
    }
}

// Why: a loopback connect that neither succeeds nor is refused within the
// probe timeout means no listener accepted and no RST came back; the port is
// treated as free so a stale half-open socket cannot block startup forever.
pub async fn is_port_in_use(port: u16) -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    match tokio::time::timeout(PORT_PROBE_TIMEOUT, TcpStream::connect(addr)).await {
        Ok(Ok(_)) => true,
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::ConnectionRefused => false,
        Ok(Err(e)) => {
            tracing::warn!(port = port, error = %e, "Port probe failed; treating port as free");
            false
        },
        Err(_) => {
            tracing::warn!(
                port = port,
                timeout_ms = PORT_PROBE_TIMEOUT.as_millis() as u64,
                "Port probe timed out — no listener accepted, no RST sent. Treating port as free. \
                 If MCP server then fails to bind, a stale half-open socket on this port is the \
                 likely cause."
            );
            false
        },
    }
}

pub async fn is_port_responsive(port: u16) -> bool {
    is_port_in_use(port).await
}

pub async fn cleanup_port_processes(port: u16, service_name: &ServiceName) -> McpDomainResult<()> {
    for pid in subprocess::pids_listening_on(port).await? {
        match classify_port_holder(pid, service_name).await {
            PortHolder::Caller | PortHolder::Gone => continue,
            PortHolder::Foreign => {
                return Err(McpDomainError::PortOwnedByForeignProcess {
                    port,
                    pid,
                    service: service_name.to_string(),
                });
            },
            PortHolder::Unverifiable => {
                return Err(McpDomainError::PortHolderUnverifiable {
                    port,
                    pid,
                    service: service_name.to_string(),
                });
            },
            PortHolder::Ours => {},
        }

        tracing::debug!(port = port, pid = pid, service = %service_name, "Stopping our stale process on port");
        match subprocess::stop_owned(pid, ChildKind::Mcp, service_name, STALE_HOLDER_GRACE).await? {
            StopOutcome::NotRunning | StopOutcome::Stopped(_) => {},
            StopOutcome::NotOurs => {
                return Err(McpDomainError::PortOwnedByForeignProcess {
                    port,
                    pid,
                    service: service_name.to_string(),
                });
            },
        }
    }

    Ok(())
}

pub async fn wait_for_port_release(port: u16) -> McpDomainResult<()> {
    let max_attempts = 10;
    let delay = Duration::from_millis(100);

    for attempt in 1..=max_attempts {
        if !is_port_in_use(port).await {
            return Ok(());
        }

        if attempt < max_attempts {
            tokio::time::sleep(delay).await;
        }
    }

    Err(McpDomainError::PortNotReleased {
        port,
        attempts: max_attempts,
    })
}

pub async fn wait_for_port_release_with_retry(
    port: u16,
    service_name: &ServiceName,
    max_cleanup_attempts: u32,
) -> McpDomainResult<()> {
    for cleanup_attempt in 1..=max_cleanup_attempts {
        if !is_port_in_use(port).await {
            return Ok(());
        }

        tracing::debug!(
            port = port,
            service = %service_name,
            attempt = cleanup_attempt,
            max_attempts = max_cleanup_attempts,
            "Port still in use, attempting cleanup"
        );

        cleanup_port_processes(port, service_name).await?;

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

    Err(McpDomainError::PortNotReleased {
        port,
        attempts: max_cleanup_attempts,
    })
}
