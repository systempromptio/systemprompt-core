//! MCP service reconciliation during server startup.
//!
//! [`reconcile`] reconciles the MCP orchestrator to the required set of
//! enabled managed servers and verifies each is registered and running in the
//! database, failing startup loudly if any required MCP server is missing,
//! since agents depend on their tools. The API's startup phase reaches it
//! through the `mcp` member's process set
//! (`ServiceProcessSet::reconcile`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use systemprompt_database::ServiceRepository;
use systemprompt_identifiers::ServiceName;
use systemprompt_manifest::services::ServiceStatus;
use systemprompt_traits::{Phase, StartupEventExt, StartupEventSender};

use crate::McpServerConfig;
use crate::error::McpDomainError;
use crate::services::orchestrator::McpOrchestrator;
use crate::services::registry::RegistryService;

pub const VERIFY_ATTEMPTS: u32 = 5;
pub const VERIFY_BACKOFF: std::time::Duration = std::time::Duration::from_millis(250);

/// Why startup reconciliation of the MCP servers refused to continue.
#[derive(Debug, thiserror::Error)]
pub enum ReconcileError {
    #[error("the MCP registry could not be read")]
    Registry(#[source] McpDomainError),
    #[error("FATAL: MCP reconciliation failed: {0}\n\nCannot start API without MCP servers.")]
    Reconcile(#[source] McpDomainError),
    #[error(
        "FATAL: {count} required MCP server(s) failed to start: {names}\n\nsystemprompt.io OS \
         cannot operate without MCP servers.\nAgents need tools to function.\n\nBuild missing \
         binaries with:\n  cargo build --bin {bins}\n\nOr build all MCP servers:\n  systemprompt \
         build mcp",
        count = missing.len(),
        names = missing.join(", "),
        bins = missing.join(" --bin ")
    )]
    Missing { missing: Vec<String> },
    #[error(
        "FATAL: MCP services running but not properly registered in database after {attempts} \
         attempts\n\nThis indicates a race condition or database synchronization \
         issue.\nFailed services: {failed}",
        failed = failures.join(", ")
    )]
    Unregistered {
        attempts: u32,
        failures: Vec<String>,
    },
}

pub async fn reconcile(
    orchestrator: &McpOrchestrator,
    registry: &RegistryService,
    services: &ServiceRepository,
    events: Option<&StartupEventSender>,
) -> Result<(), ReconcileError> {
    let required_servers = registry
        .get_managed_servers()
        .map_err(ReconcileError::Registry)?;
    let required_count = required_servers.len();

    let running_count = match orchestrator.reconcile().await {
        Ok(running_count) => running_count,
        Err(e) => {
            events.phase_failed(Phase::McpServers, e.to_string());
            return Err(ReconcileError::Reconcile(e));
        },
    };
    if running_count < required_count {
        return handle_missing_servers(&required_servers, orchestrator, events).await;
    }
    if running_count > 0 {
        verify_database_registration(&required_servers, services, events).await?;
    }
    events.mcp_reconciliation_complete(running_count, required_count);
    Ok(())
}

#[expect(
    clippy::collection_is_never_read,
    reason = "`events` is consumed by `StartupEventExt` trait methods \
              (`events.error(...)`); clippy's `collection_is_never_read` heuristic does not \
              recognise those calls as reads of the `Option`"
)]
pub async fn handle_missing_servers(
    required_servers: &[McpServerConfig],
    orchestrator: &McpOrchestrator,
    events: Option<&StartupEventSender>,
) -> Result<(), ReconcileError> {
    let running_servers = orchestrator
        .get_running_servers()
        .await
        .map_err(ReconcileError::Reconcile)?;
    let running_names: HashSet<String> = running_servers.iter().map(|s| s.name.clone()).collect();

    let missing: Vec<String> = required_servers
        .iter()
        .map(|s| s.name.clone())
        .filter(|name| !running_names.contains(name))
        .collect();

    events.error(
        format!(
            "Server status mismatch: {} servers failed to start: {}",
            missing.len(),
            missing.join(", ")
        ),
        true,
    );

    Err(ReconcileError::Missing { missing })
}

pub async fn verify_database_registration(
    required_servers: &[McpServerConfig],
    services: &ServiceRepository,
    events: Option<&StartupEventSender>,
) -> Result<(), ReconcileError> {
    let mut pending: Vec<&McpServerConfig> = required_servers.iter().collect();
    let mut failures = Vec::new();

    for attempt in 1..=VERIFY_ATTEMPTS {
        let (still_pending, attempt_failures) = verify_once(&pending, services, events).await;
        pending = still_pending;
        failures = attempt_failures;
        if pending.is_empty() {
            return Ok(());
        }
        if attempt < VERIFY_ATTEMPTS {
            tokio::time::sleep(VERIFY_BACKOFF).await;
        }
    }

    events.error(
        format!(
            "Database verification failed for {} service(s): {}",
            failures.len(),
            failures.join(", ")
        ),
        true,
    );
    Err(ReconcileError::Unregistered {
        attempts: VERIFY_ATTEMPTS,
        failures,
    })
}

#[expect(
    clippy::collection_is_never_read,
    reason = "`events` is consumed by StartupEventExt trait methods that clippy does not \
              recognise as reads"
)]
async fn verify_once<'a>(
    servers: &[&'a McpServerConfig],
    services: &ServiceRepository,
    events: Option<&StartupEventSender>,
) -> (Vec<&'a McpServerConfig>, Vec<String>) {
    let mut pending = Vec::new();
    let mut failures = Vec::new();

    for &server in servers {
        let failure = match services
            .find_service_by_name(&ServiceName::new(server.name.as_str()))
            .await
        {
            Ok(Some(service)) if service.status == ServiceStatus::Running => {
                events.mcp_ready(
                    server.name.clone(),
                    service.port as u16,
                    std::time::Duration::ZERO,
                    None,
                );
                continue;
            },
            Ok(Some(service)) => format!("{} (status: {})", server.name, service.status),
            Ok(None) => format!("{} (not in database)", server.name),
            Err(e) => format!("{} (db error: {})", server.name, e),
        };
        pending.push(server);
        failures.push(failure);
    }

    (pending, failures)
}
