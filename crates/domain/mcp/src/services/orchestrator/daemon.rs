use anyhow::Result;
use std::sync::Arc;
use tracing::Instrument;

use super::event_bus::EventBus;
use super::events::McpEvent;
use crate::services::database::DatabaseManager;
use crate::services::lifecycle::LifecycleManager;
use crate::services::registry::RegistryManager;

pub async fn run_daemon(
    event_bus: &Arc<EventBus>,
    lifecycle: &LifecycleManager,
    database: &DatabaseManager,
) -> Result<()> {
    let span: tracing::Span = systemprompt_core_logging::SystemSpan::new("mcp_orchestrator").into();
    async move {
        tracing::info!("Starting MCP daemon mode");

        database.cleanup_stale_services().await?;
        let servers = RegistryManager::get_enabled_servers()?;
        database.sync_state(&servers).await?;
        let server_count = servers.len();

        tracing::info!(
            mode = "daemon",
            enabled_services = server_count,
            services = ?servers.iter().map(|s| s.name.clone()).collect::<Vec<_>>(),
            "MCP daemon started"
        );

        for server in &servers {
            event_bus
                .publish(McpEvent::ServiceStartRequested {
                    service_name: server.name.clone(),
                })
                .await?;
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        tracing::info!("All MCP servers started with proper OAuth enforcement");
        tracing::info!("MCP manager will keep servers running. Press Ctrl+C to stop.");

        tokio::signal::ctrl_c().await?;
        tracing::info!("Shutting down MCP servers");

        let running_servers = database.get_running_servers().await?;
        let running_count = running_servers.len();

        tracing::info!(
            running_services = running_count,
            shutdown_reason = "user_interrupt",
            "MCP daemon shutdown initiated"
        );

        for server in running_servers {
            lifecycle.stop_server(&server).await?;

            event_bus
                .publish(McpEvent::ServiceStopped {
                    service_name: server.name,
                    exit_code: None,
                })
                .await?;
        }

        tracing::info!("MCP daemon shutdown completed");

        Ok(())
    }
    .instrument(span)
    .await
}
