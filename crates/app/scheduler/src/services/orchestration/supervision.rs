//! Port and process stops for service records, over
//! [`systemprompt_loader::subprocess`].
//!
//! A recorded agent or MCP pid is signalled only through
//! [`subprocess::stop_owned`], which proves the live process still carries
//! the service's spawn marker. A port holder is stopped on a service's behalf
//! only when it carries that same marker ([`stop_owned_port_holders`]).
//!
//! The API server carries no spawn marker. [`stop_port_listeners`] is the one
//! unverified stop: it serves operator commands that name the API port
//! (`services stop api`, `services restart api`, a confirmed
//! `services cleanup`) and signals exactly the listeners on that port, never a
//! protected database port.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_identifiers::ServiceName;
use systemprompt_loader::subprocess::{self, ChildKind, StopOutcome, Termination};
use systemprompt_models::services::ServiceModule;

use crate::error::{SchedulerError, SchedulerResult};

const PROTECTED_PORTS: &[u16] = &[5432, 6432];
const PORT_FREE_POLL: Duration = Duration::from_millis(50);

#[must_use]
pub const fn child_kind(module: ServiceModule) -> ChildKind {
    match module {
        ServiceModule::Agent => ChildKind::Agent,
        ServiceModule::Mcp => ChildKind::Mcp,
    }
}

pub async fn port_holders(port: u16) -> SchedulerResult<Vec<u32>> {
    if port == 0 || PROTECTED_PORTS.contains(&port) {
        return Ok(Vec::new());
    }
    Ok(subprocess::pids_listening_on(port).await?)
}

pub async fn stop_owned_port_holders(
    port: u16,
    kind: ChildKind,
    service: &ServiceName,
    grace: Duration,
) -> SchedulerResult<Vec<u32>> {
    let mut stopped = Vec::new();
    for holder in port_holders(port).await? {
        if let StopOutcome::Stopped(_) =
            subprocess::stop_owned(holder, kind, service, grace).await?
        {
            stopped.push(holder);
        }
    }
    Ok(stopped)
}

pub async fn stop_port_listeners(
    port: u16,
    grace: Duration,
) -> SchedulerResult<Vec<(u32, Termination)>> {
    let mut stopped = Vec::new();
    for holder in port_holders(port).await? {
        let termination = subprocess::terminate_gracefully(holder, grace).await?;
        stopped.push((holder, termination));
    }
    Ok(stopped)
}

pub async fn wait_for_port_free(port: u16, within: Duration) -> SchedulerResult<()> {
    let deadline = tokio::time::Instant::now() + within;
    loop {
        let holders = port_holders(port).await?;
        if holders.is_empty() {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(SchedulerError::PortOccupied { port, holders });
        }
        tokio::time::sleep(PORT_FREE_POLL).await;
    }
}
