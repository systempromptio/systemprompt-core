use crate::cli_settings::CliConfig;
use crate::interactive::require_confirmation;
use anyhow::Result;
use std::sync::Arc;
use systemprompt_logging::CliService;
use systemprompt_models::ProfileBootstrap;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::{ProcessCleanup, ServiceManagementService};

const DEFAULT_API_PORT: u16 = 8080;

fn get_api_port() -> u16 {
    ProfileBootstrap::get()
        .map(|p| p.server.port)
        .unwrap_or(DEFAULT_API_PORT)
}

pub async fn execute(yes: bool, dry_run: bool, config: &CliConfig) -> Result<()> {
    CliService::section("Cleaning Up Services");

    let ctx = Arc::new(AppContext::new().await?);
    let service_mgmt = ServiceManagementService::new(Arc::clone(ctx.db_pool()));
    let api_port = get_api_port();

    CliService::info("Finding running services...");
    let running_services = service_mgmt.get_running_services_with_pid().await?;

    let api_pid = ProcessCleanup::check_port(api_port);
    let has_services = !running_services.is_empty() || api_pid.is_some();

    if !has_services {
        CliService::info("No running services found");
        return Ok(());
    }

    if dry_run {
        CliService::section("Dry Run - Would clean the following:");
        for service in &running_services {
            if let Some(pid) = service.pid {
                let pid_u32 = pid as u32;
                if ProcessCleanup::process_exists(pid_u32) {
                    CliService::info(&format!(
                        "  [running] {} (PID: {}, port: {})",
                        service.name, pid, service.port
                    ));
                } else {
                    CliService::info(&format!(
                        "  [stale] {} (PID {} not running)",
                        service.name, pid
                    ));
                }
            }
        }
        if let Some(pid) = api_pid {
            CliService::info(&format!(
                "  [running] API server (PID: {}, port: {})",
                pid, api_port
            ));
        }
        CliService::info("Run without --dry-run to execute cleanup");
        return Ok(());
    }

    let service_count = running_services.len() + usize::from(api_pid.is_some());
    require_confirmation(
        &format!("This will stop {} service(s). Continue?", service_count),
        yes,
        config,
    )?;

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
    let api_killed = ProcessCleanup::kill_port(api_port);
    if !api_killed.is_empty() {
        cleaned += 1;
    }

    ProcessCleanup::kill_by_pattern("systemprompt serve api");

    ProcessCleanup::wait_for_port_free(api_port, 3, 1000).await?;

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
