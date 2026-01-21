use anyhow::Result;
use std::collections::HashSet;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_traits::{StartupEvent, StartupEventSender};
use tracing::Instrument;

use super::event_bus::EventBus;
use super::process_cleanup::{
    detect_and_handle_orphaned_processes, detect_and_handle_stale_binaries,
};
use super::schema_sync::validate_schemas;
use super::server_startup::{start_pending_servers, StartPendingServersParams};
use crate::services::database::DatabaseManager;
use crate::services::lifecycle::LifecycleManager;
use crate::services::process::ProcessManager;
use crate::services::registry::RegistryManager;
use crate::McpServerConfig;

#[derive(Debug)]
pub struct ReconcileParams<'a> {
    pub database: &'a DatabaseManager,
    pub lifecycle: &'a LifecycleManager,
    pub event_bus: &'a Arc<EventBus>,
    pub db_pool: &'a DbPool,
    pub events: Option<&'a StartupEventSender>,
}

pub async fn reconcile(params: ReconcileParams<'_>) -> Result<usize> {
    let ReconcileParams {
        database,
        lifecycle,
        event_bus,
        db_pool,
        events,
    } = params;

    async move {
        database.cleanup_stale_services().await?;
        database.delete_crashed_services().await?;

        let enabled_servers = RegistryManager::get_enabled_servers()?;

        let deleted = database.delete_disabled_services(&enabled_servers).await?;
        if deleted > 0 {
            tracing::info!(count = deleted, "Cleaned up disabled services");
            notify_cleanup(events, deleted, "no longer enabled in configuration");
        }

        validate_schemas(&enabled_servers, db_pool).await?;
        database.sync_state(&enabled_servers).await?;
        cleanup_orphaned_and_stale(database, &enabled_servers, events).await?;

        kill_all_running_servers(database, events).await?;
        let running_names = HashSet::new();
        start_pending_servers(StartPendingServersParams {
            servers: &enabled_servers,
            running_names: &running_names,
            lifecycle,
            database,
            event_bus,
            events,
        })
        .await
    }
    .instrument(systemprompt_logging::SystemSpan::new("mcp_orchestrator").into())
    .await
}

fn notify_cleanup(events: Option<&StartupEventSender>, count: usize, reason: &str) {
    if let Some(tx) = events {
        let _ = tx.send(StartupEvent::McpServiceCleanup {
            name: format!("{} disabled service(s)", count),
            reason: reason.to_string(),
        });
    }
}

async fn cleanup_orphaned_and_stale(
    database: &DatabaseManager,
    servers: &[McpServerConfig],
    events: Option<&StartupEventSender>,
) -> Result<()> {
    let orphaned = detect_and_handle_orphaned_processes(servers, database).await?;
    log_and_notify_cleanup(
        orphaned,
        "orphaned",
        "Killed orphaned MCP processes, will restart fresh",
        events,
    );

    let stale = detect_and_handle_stale_binaries(servers, database).await?;
    log_and_notify_cleanup(
        stale,
        "stale binary",
        "Killed stale MCP processes (binary rebuilt), will restart fresh",
        events,
    );

    Ok(())
}

fn log_and_notify_cleanup(
    count: usize,
    reason: &str,
    message: &str,
    events: Option<&StartupEventSender>,
) {
    if count == 0 {
        return;
    }

    tracing::info!(count = count, message);

    if let Some(tx) = events {
        let _ = tx.send(StartupEvent::McpServiceCleanup {
            name: format!("{} processes", count),
            reason: reason.to_string(),
        });
    }
}

async fn kill_all_running_servers(
    database: &DatabaseManager,
    events: Option<&StartupEventSender>,
) -> Result<()> {
    let running_servers = database.get_running_servers().await?;
    for server in running_servers {
        kill_single_server(database, &server.name, events).await?;
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    Ok(())
}

async fn kill_single_server(
    database: &DatabaseManager,
    server_name: &str,
    events: Option<&StartupEventSender>,
) -> Result<()> {
    if let Ok(Some(service_info)) = database.get_service_by_name(server_name).await {
        if let Some(pid) = service_info.pid {
            if let Some(tx) = events {
                let _ = tx.send(StartupEvent::McpServiceCleanup {
                    name: server_name.to_string(),
                    reason: "Restarting to ensure fresh state".to_string(),
                });
            }
            ProcessManager::terminate_gracefully(pid as u32).ok();
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            ProcessManager::force_kill(pid as u32).ok();
        }
        database.unregister_service(server_name).await.ok();
    }
    Ok(())
}
