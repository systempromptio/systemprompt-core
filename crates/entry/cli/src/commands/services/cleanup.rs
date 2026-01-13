use crate::cli_settings::CliConfig;
use anyhow::Result;
use std::sync::Arc;
use systemprompt_core_logging::CliService;
use systemprompt_core_scheduler::{ProcessCleanup, ServiceManagementService};
use systemprompt_runtime::AppContext;

pub async fn execute(_config: &CliConfig) -> Result<()> {
    CliService::section("Cleaning Up Services");

    let ctx = Arc::new(AppContext::new().await?);
    let service_mgmt = ServiceManagementService::new(Arc::clone(ctx.db_pool()));

    CliService::info("Finding running services...");
    let running_services = service_mgmt.get_running_services_with_pid().await?;

    let mut cleaned = 0;

    for service in &running_services {
        if let Some(pid) = service.pid {
            let pid_u32 = pid as u32;

            if !ProcessCleanup::process_exists(pid_u32) {
                CliService::info(&format!(
                    "Cleaning stale entry: {} (PID {} not running)",
                    service.name, pid
                ));
                service_mgmt.mark_service_stopped(&service.name).await.ok();
                cleaned += 1;
                continue;
            }

            CliService::info(&format!(
                "Stopping {} (PID: {}, port: {})...",
                service.name, pid, service.port
            ));

            service_mgmt.cleanup_orphaned_service(service).await?;
            cleaned += 1;
        }
    }

    CliService::info("Stopping API server...");
    let api_killed = ProcessCleanup::kill_port(8080);
    if !api_killed.is_empty() {
        cleaned += 1;
    }

    ProcessCleanup::kill_by_pattern("systemprompt serve api");

    ProcessCleanup::wait_for_port_free(8080, 3, 1000).await?;

    let stale_count = service_mgmt.cleanup_stale_entries().await.unwrap_or(0);
    if stale_count > 0 {
        CliService::info(&format!("Cleaned {} stale database entries", stale_count));
    }

    if cleaned > 0 {
        CliService::success(&format!("Cleaned up {} services", cleaned));
    } else {
        CliService::info("No running services found");
    }

    Ok(())
}
