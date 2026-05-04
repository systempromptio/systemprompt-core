use crate::cli_settings::CliConfig;
use crate::interactive::require_confirmation;
use crate::shared::CommandResult;
use anyhow::Result;
use std::sync::Arc;
use systemprompt_config::ProfileBootstrap;
use systemprompt_logging::CliService;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::{ProcessCleanup, ServiceManagementService};

use super::types::CleanupOutput;

const DEFAULT_API_PORT: u16 = 8080;

fn get_api_port() -> u16 {
    ProfileBootstrap::get().map_or(DEFAULT_API_PORT, |p| p.server.port)
}

pub async fn execute(
    yes: bool,
    dry_run: bool,
    config: &CliConfig,
) -> Result<CommandResult<CleanupOutput>> {
    let quiet = config.is_json_output();
    if !quiet {
        CliService::section("Cleaning Up Services");
    }

    let ctx = Arc::new(AppContext::new().await?);
    let service_mgmt = ServiceManagementService::new(ctx.db_pool())?;
    let api_port = get_api_port();

    if !quiet {
        CliService::info("Finding running services...");
    }
    let running_services = service_mgmt.get_running_services_with_pid().await?;
    let api_pid = ProcessCleanup::check_port(api_port);

    if running_services.is_empty() && api_pid.is_none() {
        return Ok(no_services_result(quiet, dry_run));
    }

    if dry_run {
        return Ok(dry_run_result(&running_services, api_pid, api_port, quiet));
    }

    let service_count = running_services.len() + usize::from(api_pid.is_some());
    require_confirmation(
        &format!("This will stop {} service(s). Continue?", service_count),
        yes,
        config,
    )?;

    let mut cleaned = stop_running_services(&running_services, &service_mgmt, quiet).await?;
    cleaned += stop_api_server(api_port, quiet).await?;

    let stale_count = service_mgmt.cleanup_stale_entries().await.unwrap_or(0) as usize;
    if stale_count > 0 && !quiet {
        CliService::info(&format!("Cleaned {} stale database entries", stale_count));
    }

    let message = format_cleanup_message(cleaned, quiet);
    let output = CleanupOutput {
        services_cleaned: cleaned,
        stale_entries_removed: stale_count,
        dry_run: false,
        message,
    };
    Ok(CommandResult::card(output).with_title("Service Cleanup"))
}

fn no_services_result(quiet: bool, dry_run: bool) -> CommandResult<CleanupOutput> {
    let message = "No running services found".to_string();
    if !quiet {
        CliService::info(&message);
    }
    CommandResult::card(CleanupOutput {
        services_cleaned: 0,
        stale_entries_removed: 0,
        dry_run,
        message,
    })
    .with_title("Service Cleanup")
}

fn dry_run_result(
    running_services: &[systemprompt_database::ServiceConfig],
    api_pid: Option<u32>,
    api_port: u16,
    quiet: bool,
) -> CommandResult<CleanupOutput> {
    if !quiet {
        CliService::section("Dry Run - Would clean the following:");
        for service in running_services {
            log_service_state(service);
        }
        if let Some(pid) = api_pid {
            CliService::info(&format!(
                "  [running] API server (PID: {}, port: {})",
                pid, api_port
            ));
        }
        CliService::info("Run without --dry-run to execute cleanup");
    }
    let service_count = running_services.len() + usize::from(api_pid.is_some());
    CommandResult::card(CleanupOutput {
        services_cleaned: service_count,
        stale_entries_removed: 0,
        dry_run: true,
        message: format!("Would clean {} service(s)", service_count),
    })
    .with_title("Service Cleanup (Dry Run)")
}

fn log_service_state(service: &systemprompt_database::ServiceConfig) {
    let Some(pid) = service.pid else { return };
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

async fn stop_running_services(
    running_services: &[systemprompt_database::ServiceConfig],
    service_mgmt: &ServiceManagementService,
    quiet: bool,
) -> Result<usize> {
    let mut cleaned = 0usize;
    for service in running_services {
        let Some(pid) = service.pid else { continue };
        let pid_u32 = pid as u32;
        if !ProcessCleanup::process_exists(pid_u32) {
            if !quiet {
                CliService::info(&format!(
                    "Cleaning stale entry: {} (PID {} not running)",
                    service.name, pid
                ));
            }
            service_mgmt.mark_service_stopped(&service.name).await.ok();
            cleaned += 1;
            continue;
        }
        if !quiet {
            CliService::info(&format!(
                "Stopping {} (PID: {}, port: {})...",
                service.name, pid, service.port
            ));
        }
        service_mgmt.cleanup_orphaned_service(service).await?;
        cleaned += 1;
    }
    Ok(cleaned)
}

async fn stop_api_server(api_port: u16, quiet: bool) -> Result<usize> {
    if !quiet {
        CliService::info("Stopping API server...");
    }
    let api_killed = ProcessCleanup::kill_port(api_port);
    let cleaned = usize::from(!api_killed.is_empty());
    ProcessCleanup::kill_by_pattern("systemprompt serve api");
    ProcessCleanup::wait_for_port_free(api_port, 3, 1000).await?;
    Ok(cleaned)
}

fn format_cleanup_message(cleaned: usize, quiet: bool) -> String {
    if cleaned > 0 {
        let msg = format!("Cleaned up {} services", cleaned);
        if !quiet {
            CliService::success(&msg);
        }
        msg
    } else {
        let msg = "No running services found".to_string();
        if !quiet {
            CliService::info(&msg);
        }
        msg
    }
}
