//! Batch startup of pending MCP servers during reconciliation.
//!
//! [`start_pending_servers`] iterates the enabled set, starting any server not
//! already running and aggregating failures into a single error, then publishes
//! per-server success/failure and overall reconciliation-complete events to the
//! [`EventBus`] and [`StartupEventSender`].

use crate::error::McpDomainResult;
use std::collections::HashSet;
use std::sync::Arc;
use systemprompt_traits::StartupEventSender;

use super::event_bus::EventBus;
use super::events::McpEvent;
use crate::McpServerConfig;
use crate::services::database::DatabaseService;
use crate::services::lifecycle::LifecycleOrchestrator;

pub(super) struct StartPendingServersParams<'a> {
    pub servers: &'a [McpServerConfig],
    pub running_names: &'a HashSet<String>,
    pub lifecycle: &'a LifecycleOrchestrator,
    pub database: &'a DatabaseService,
    pub event_bus: &'a Arc<EventBus>,
    pub events: Option<&'a StartupEventSender>,
}

pub(super) async fn start_pending_servers(
    params: StartPendingServersParams<'_>,
) -> McpDomainResult<usize> {
    let StartPendingServersParams {
        servers,
        running_names,
        lifecycle,
        database,
        event_bus,
        events,
    } = params;
    let mut failed: Vec<(String, String)> = Vec::new();
    let mut started_count = 0;

    for server in servers {
        if running_names.contains(&server.name) {
            started_count += 1;
            continue;
        }

        match start_single_server(server, lifecycle, database, event_bus, events).await {
            Ok(()) => started_count += 1,
            Err(e) => failed.push((server.name.clone(), e.to_string())),
        }
    }

    notify_reconciliation_complete(events, started_count, servers.len());

    if !failed.is_empty() {
        return Err(crate::error::McpDomainError::Internal(format!(
            "Failed to start {} MCP service(s): {}",
            failed.len(),
            failed
                .iter()
                .map(|(name, err)| format!("{name} ({err})"))
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }

    Ok(started_count)
}

fn notify_reconciliation_complete(
    events: Option<&StartupEventSender>,
    running: usize,
    required: usize,
) {
    use systemprompt_traits::StartupEvent;

    if let Some(tx) = events
        && let Err(e) =
            tx.unbounded_send(StartupEvent::McpReconciliationComplete { running, required })
    {
        tracing::warn!(error = %e, "Failed to send reconciliation complete event");
    }
}

async fn start_single_server(
    server: &McpServerConfig,
    lifecycle: &LifecycleOrchestrator,
    database: &DatabaseService,
    event_bus: &Arc<EventBus>,
    events: Option<&StartupEventSender>,
) -> McpDomainResult<()> {
    let start_time = std::time::Instant::now();

    match lifecycle.start_server_with_events(server, events).await {
        Ok(()) => {
            let duration_ms = start_time.elapsed().as_millis() as u64;
            publish_start_success(server, database, event_bus, duration_ms).await
        },
        Err(e) => {
            let duration_ms = start_time.elapsed().as_millis() as u64;
            publish_start_failure(server, event_bus, duration_ms, &e.to_string()).await?;
            Err(e)
        },
    }
}

async fn publish_start_success(
    server: &McpServerConfig,
    database: &DatabaseService,
    event_bus: &Arc<EventBus>,
    duration_ms: u64,
) -> McpDomainResult<()> {
    if let Ok(Some(service_info)) = database.get_service_by_name(&server.name).await {
        event_bus
            .publish(McpEvent::ServiceStartCompleted {
                service_name: server.name.clone(),
                success: true,
                pid: service_info.pid.map(|p| p as u32),
                port: Some(server.port),
                error: None,
                duration_ms,
            })
            .await?;

        event_bus
            .publish(McpEvent::ServiceStarted {
                service_name: server.name.clone(),
                process_id: service_info.pid.unwrap_or(0) as u32,
                port: server.port,
            })
            .await?;
    }
    Ok(())
}

async fn publish_start_failure(
    server: &McpServerConfig,
    event_bus: &Arc<EventBus>,
    duration_ms: u64,
    error_msg: &str,
) -> McpDomainResult<()> {
    event_bus
        .publish(McpEvent::ServiceStartCompleted {
            service_name: server.name.clone(),
            success: false,
            pid: None,
            port: Some(server.port),
            error: Some(error_msg.to_owned()),
            duration_ms,
        })
        .await?;

    event_bus
        .publish(McpEvent::ServiceFailed {
            service_name: server.name.clone(),
            error: error_msg.to_owned(),
        })
        .await?;

    Ok(())
}
