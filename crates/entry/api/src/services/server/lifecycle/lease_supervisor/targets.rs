//! The lease loop's process targets: the processes the configuration wants and
//! the stop of one held process.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::process_ownership::ProcessKey;
use anyhow::Result;
use std::time::Duration;
use systemprompt_database::{ServiceModule, ServiceRepository};
use systemprompt_identifiers::ServiceName;
use systemprompt_loader::subprocess::{self, ChildClaim, ChildKind, SpawnStamp, StopOutcome};
use systemprompt_runtime::AppContext;

pub(super) const RELEASE_MARGIN: Duration = Duration::from_secs(1);

pub(super) async fn stop_held(
    services: &ServiceRepository,
    key: &ProcessKey,
    grace: Duration,
) -> bool {
    let service = match services.find_service_by_name(key.name()).await {
        Ok(service) => service,
        Err(error) => {
            tracing::warn!(service = %key.name(), error = %error, "process row unreadable on shutdown");
            return false;
        },
    };
    let Some(row) = service else {
        tracing::warn!(service = %key.name(), "no pid recorded on shutdown; leaving its lease to expire");
        return false;
    };
    let Some(pid) = row.pid.and_then(|pid| u32::try_from(pid).ok()) else {
        tracing::warn!(service = %key.name(), "no pid recorded on shutdown; leaving its lease to expire");
        return false;
    };
    let kind = match key.module() {
        ServiceModule::Mcp => ChildKind::Mcp,
        ServiceModule::Agent => ChildKind::Agent,
    };
    let claim = ChildClaim::spawned(
        kind,
        key.name(),
        SpawnStamp::recorded(row.pid, row.spawn_nonce.as_ref()),
    );
    match subprocess::stop_owned(pid, &claim, grace).await {
        Ok(StopOutcome::NotRunning | StopOutcome::Stopped(_) | StopOutcome::NotOurs) => true,
        Err(error) => {
            tracing::warn!(service = %key.name(), pid, error = %error, "process survived shutdown stop");
            false
        },
    }
}

pub(crate) async fn desired_processes(ctx: &AppContext) -> Result<Vec<ProcessKey>> {
    let mut wanted: Vec<ProcessKey> = match crate::services::proxy::mcp_registry(ctx) {
        Ok(registry) => registry
            .managed_servers()
            .await?
            .into_iter()
            .map(|server| ProcessKey::new(ServiceModule::Mcp, ServiceName::new(server.name)))
            .collect(),
        Err(_) => Vec::new(),
    };
    wanted.extend(
        super::super::agents::enabled_agents(ctx)?
            .into_iter()
            .map(|agent| ProcessKey::new(ServiceModule::Agent, ServiceName::new(agent.name))),
    );
    Ok(wanted)
}
