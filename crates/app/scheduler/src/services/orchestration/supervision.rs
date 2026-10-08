//! Port and process stops for service records, over
//! [`systemprompt_loader::subprocess`].
//!
//! A recorded agent or MCP pid is signalled only through
//! [`subprocess::stop_owned`], which proves the live process still carries
//! the service's spawn marker. A port holder is stopped on a service's behalf
//! only when it carries that same marker ([`stop_owned_port_holders`]).
//!
//! The API server is not spawned by us; `infra services serve` stamps itself
//! with the API marker ([`subprocess::stamp_api_server`]).
//! [`stop_api_listeners`] serves the operator commands that name the API port
//! (`services stop api`, `services restart api`, a confirmed `services
//! cleanup`): it signals only a listener carrying that marker, and reports any
//! other listener as [`StopOutcome::NotOurs`] without signalling it. A
//! protected database port is never examined.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_identifiers::ServiceName;
use systemprompt_loader::subprocess::{self, ChildKind, StopOutcome};
use systemprompt_manifest::services::ServiceModule;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApiListenerStop {
    pub pid: u32,
    pub outcome: StopOutcome,
}

pub async fn stop_api_listeners(
    port: u16,
    grace: Duration,
) -> SchedulerResult<Vec<ApiListenerStop>> {
    let service = subprocess::api_server_service();
    let mut stops = Vec::new();
    for pid in port_holders(port).await? {
        let outcome = if !subprocess::is_running(pid).await {
            StopOutcome::NotRunning
        } else if subprocess::owns(pid, ChildKind::Api, &service).await {
            StopOutcome::Stopped(subprocess::terminate_gracefully(pid, grace).await?)
        } else {
            StopOutcome::NotOurs
        };
        stops.push(ApiListenerStop { pid, outcome });
    }
    Ok(stops)
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
