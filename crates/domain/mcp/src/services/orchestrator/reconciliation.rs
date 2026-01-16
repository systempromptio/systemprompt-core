use anyhow::Result;
use std::collections::HashSet;
use std::sync::Arc;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{StartupEvent, StartupEventSender};
use tracing::Instrument;

use super::event_bus::EventBus;
use super::events::McpEvent;
use super::process_cleanup::{
    detect_and_handle_orphaned_processes, detect_and_handle_stale_binaries,
};
use crate::services::database::DatabaseManager;
use crate::services::lifecycle::LifecycleManager;
use crate::services::process::ProcessManager;
use crate::services::registry::RegistryManager;
use crate::services::schema::{SchemaValidationMode, SchemaValidationReport, SchemaValidator};
use crate::McpServerConfig;

#[derive(Debug)]
pub struct ReconcileParams<'a> {
    pub database: &'a DatabaseManager,
    pub lifecycle: &'a LifecycleManager,
    pub event_bus: &'a Arc<EventBus>,
    pub app_context: &'a Arc<AppContext>,
    pub events: Option<&'a StartupEventSender>,
}

pub async fn reconcile(params: ReconcileParams<'_>) -> Result<usize> {
    let ReconcileParams {
        database,
        lifecycle,
        event_bus,
        app_context,
        events,
    } = params;

    async move {
        database.cleanup_stale_services().await?;
        database.delete_crashed_services().await?;

        let enabled_servers = RegistryManager::get_enabled_servers()?;

        let deleted = database.delete_disabled_services(&enabled_servers).await?;
        if deleted > 0 {
            tracing::info!(count = deleted, "Cleaned up disabled services");
            if let Some(tx) = events {
                let _ = tx.send(StartupEvent::McpServiceCleanup {
                    name: format!("{} disabled service(s)", deleted),
                    reason: "no longer enabled in configuration".to_string(),
                });
            }
        }

        validate_schemas(&enabled_servers, app_context).await?;
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
    .instrument(systemprompt_core_logging::SystemSpan::new("mcp_orchestrator").into())
    .await
}

async fn validate_schemas(
    servers: &[McpServerConfig],
    app_context: &Arc<AppContext>,
) -> Result<()> {
    let schema_report = validate_and_migrate_schemas(servers, app_context).await?;

    report_schema_errors(&schema_report)?;

    if schema_report.created > 0 {
        tracing::debug!("Created {} missing tables", schema_report.created);
    }

    Ok(())
}

fn report_schema_errors(report: &SchemaValidationReport) -> Result<()> {
    if report.errors.is_empty() {
        return Ok(());
    }

    for error in &report.errors {
        tracing::error!(error = %error, "Schema validation error");
    }

    Err(anyhow::anyhow!(
        "Schema validation failed with {} errors",
        report.errors.len()
    ))
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
        if let Ok(Some(service_info)) = database.get_service_by_name(&server.name).await {
            if let Some(pid) = service_info.pid {
                if let Some(tx) = events {
                    let _ = tx.send(StartupEvent::McpServiceCleanup {
                        name: server.name.clone(),
                        reason: "Restarting to ensure fresh state".to_string(),
                    });
                }
                ProcessManager::terminate_gracefully(pid as u32).ok();
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                ProcessManager::force_kill(pid as u32).ok();
            }
            database.unregister_service(&server.name).await.ok();
        }
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    Ok(())
}

struct StartPendingServersParams<'a> {
    servers: &'a [McpServerConfig],
    running_names: &'a HashSet<String>,
    lifecycle: &'a LifecycleManager,
    database: &'a DatabaseManager,
    event_bus: &'a Arc<EventBus>,
    events: Option<&'a StartupEventSender>,
}

async fn start_pending_servers(params: StartPendingServersParams<'_>) -> Result<usize> {
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

    if let Some(tx) = events {
        let _ = tx.send(StartupEvent::McpReconciliationComplete {
            running: started_count,
            required: servers.len(),
        });
    }

    if !failed.is_empty() {
        return Err(anyhow::anyhow!(
            "Failed to start {} MCP service(s): {}",
            failed.len(),
            failed
                .iter()
                .map(|(name, err)| format!("{name} ({err})"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    Ok(started_count)
}

async fn start_single_server(
    server: &McpServerConfig,
    lifecycle: &LifecycleManager,
    database: &DatabaseManager,
    event_bus: &Arc<EventBus>,
    events: Option<&StartupEventSender>,
) -> Result<()> {
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
    database: &DatabaseManager,
    event_bus: &Arc<EventBus>,
    duration_ms: u64,
) -> Result<()> {
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
) -> Result<()> {
    event_bus
        .publish(McpEvent::ServiceStartCompleted {
            service_name: server.name.clone(),
            success: false,
            pid: None,
            port: Some(server.port),
            error: Some(error_msg.to_string()),
            duration_ms,
        })
        .await?;

    event_bus
        .publish(McpEvent::ServiceFailed {
            service_name: server.name.clone(),
            error: error_msg.to_string(),
        })
        .await?;

    Ok(())
}

pub async fn validate_and_migrate_schemas(
    servers: &[McpServerConfig],
    app_context: &Arc<AppContext>,
) -> Result<SchemaValidationReport> {
    let validator = create_schema_validator(app_context)?;
    let mut combined_report = SchemaValidationReport::new("all".to_string());

    for server in servers.iter().filter(|s| !s.schemas.is_empty()) {
        validate_server_schemas(server, &validator, &mut combined_report).await;
    }

    Ok(combined_report)
}

fn create_schema_validator(app_context: &Arc<AppContext>) -> Result<SchemaValidator<'_>> {
    use systemprompt_loader::ConfigLoader;

    let services_config = ConfigLoader::load()?;
    let validation_mode =
        SchemaValidationMode::from_string(&services_config.settings.schema_validation_mode);

    Ok(SchemaValidator::new(
        app_context.db_pool().as_ref(),
        validation_mode,
    ))
}

async fn validate_server_schemas(
    server: &McpServerConfig,
    validator: &SchemaValidator<'_>,
    report: &mut SchemaValidationReport,
) {
    let service_path = std::path::Path::new(&server.crate_path);

    match validator
        .validate_and_apply(&server.name, service_path, &server.schemas)
        .await
    {
        Ok(server_report) => {
            log_successful_validation(server, &server_report);
            report.merge(server_report);
        },
        Err(e) => {
            report.errors.push(format!(
                "Schema validation failed for {}: {}",
                server.name, e
            ));
            tracing::error!(
                service_name = %server.name,
                failure_reason = %e,
                "Schema validation failed"
            );
        },
    }
}

fn log_successful_validation(server: &McpServerConfig, report: &SchemaValidationReport) {
    if report.validated > 0 {
        tracing::info!(
            service_name = %server.name,
            validated = report.validated,
            created = report.created,
            "Validated schemas for MCP service"
        );
    }
}
