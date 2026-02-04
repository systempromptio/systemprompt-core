use crate::cli_settings::CliConfig;
use crate::interactive::require_confirmation;
use crate::shared::CommandResult;
use anyhow::Result;
use std::sync::Arc;
use systemprompt_logging::CliService;
use systemprompt_models::ProfileBootstrap;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::{ProcessCleanup, ServiceManagementService};

use super::types::CleanupOutput;

const DEFAULT_API_PORT: u16 = 8080;

fn get_api_port() -> u16 {
    ProfileBootstrap::get()
        .map(|p| p.server.port)
        .unwrap_or(DEFAULT_API_PORT)
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
    let service_mgmt = ServiceManagementService::new(Arc::clone(ctx.db_pool()));
    let api_port = get_api_port();

    if !quiet {
        CliService::info("Finding running services...");
    }
    let running_services = service_mgmt.get_running_services_with_pid().await?;

    let api_pid = ProcessCleanup::check_port(api_port);
    let has_services = !running_services.is_empty() || api_pid.is_some();

    if !has_services {
        let message = "No running services found".to_string();
        if !quiet {
            CliService::info(&message);
        }
        let output = CleanupOutput {
            services_cleaned: 0,
            stale_entries_removed: 0,
            dry_run,
            message,
        };
        return Ok(CommandResult::card(output).with_title("Service Cleanup"));
    }

    if dry_run {
        if !quiet {
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
        }

        let service_count = running_services.len() + usize::from(api_pid.is_some());
        let output = CleanupOutput {
            services_cleaned: service_count,
            stale_entries_removed: 0,
            dry_run: true,
            message: format!("Would clean {} service(s)", service_count),
        };
        return Ok(CommandResult::card(output).with_title("Service Cleanup (Dry Run)"));
    }

    let service_count = running_services.len() + usize::from(api_pid.is_some());
    require_confirmation(
        &format!("This will stop {} service(s). Continue?", service_count),
        yes,
        config,
    )?;

    let mut cleaned = 0usize;

    for service in &running_services {
        if let Some(pid) = service.pid {
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
    }

    if !quiet {
        CliService::info("Stopping API server...");
    }
    let api_killed = ProcessCleanup::kill_port(api_port);
    if !api_killed.is_empty() {
        cleaned += 1;
    }

    ProcessCleanup::kill_by_pattern("systemprompt serve api");

    ProcessCleanup::wait_for_port_free(api_port, 3, 1000).await?;

    let stale_count = service_mgmt.cleanup_stale_entries().await.unwrap_or(0) as usize;
    if stale_count > 0 && !quiet {
        CliService::info(&format!("Cleaned {} stale database entries", stale_count));
    }

    let message = if cleaned > 0 {
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
    };

    let output = CleanupOutput {
        services_cleaned: cleaned,
        stale_entries_removed: stale_count,
        dry_run: false,
        message,
    };

    Ok(CommandResult::card(output).with_title("Service Cleanup"))
}
